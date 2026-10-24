//! Differential checks against the pinned compiler and runtime.
mod immediate_effects;
mod immediate_failures;
mod support;

use std::{cell::RefCell, thread};

use leitmotif::{ModelBuilder, Transition};
use leitmotif_recite::Bindings;
use recite_core::{
    ScalarValue,
    compiled::{CompiledArgument, CompiledConditionCall, CompiledStatementKind},
};
use recite_runtime::{ConditionQuery, DialogueContext, DialogueEvent, choose, next, start_scene};
use support::{Enabled, Mood, Other, Stage, compile};

fn ready_call(argument: CompiledArgument) -> CompiledConditionCall {
    CompiledConditionCall {
        function: "ready".into(),
        args: vec![
            argument,
            CompiledArgument::Value(ScalarValue::Integer(3)),
            CompiledArgument::Value(ScalarValue::Float(0.5)),
            CompiledArgument::Value(ScalarValue::Boolean(true)),
        ],
    }
}

#[test]
fn compiled_boolean_guards_match_runtime_and_preserve_typed_arguments() {
    let asset = compile(concat!(
        ":: start default\n",
        ":if ready(player, 3, 0.5, true) and not ready(\"player\", 3, 0.5, true) or fallback()\n",
        "  > yes@8843fd6f53f020a12b31\n    Yes.\n",
        ":else\n  > no@4dff358182d02d9090e3\n    No.\n",
        "-> END\n"
    ))
    .unwrap();
    let statement = asset
        .statements
        .iter()
        .find(|row| matches!(row.kind, CompiledStatementKind::If { .. }))
        .unwrap();
    let CompiledStatementKind::If { condition, .. } = &statement.kind else {
        panic!("missing if")
    };
    let span = &asset
        .source_maps
        .get(usize::try_from(statement.source_map.as_u32()).unwrap())
        .unwrap()
        .span;
    let mut builder = ModelBuilder::new("host").unwrap();
    let enabled = builder.fact::<Enabled>().unwrap();
    let other = builder.fact::<Other>().unwrap();
    let model = builder.build().unwrap();
    let mut distinct = Bindings::new();
    distinct
        .bind_bool(
            ready_call(CompiledArgument::Identifier("player".into())),
            &enabled,
        )
        .unwrap();
    distinct
        .bind_bool(
            ready_call(CompiledArgument::Value(ScalarValue::String(
                "player".into(),
            ))),
            &other,
        )
        .unwrap();
    distinct
        .bind_bool(
            CompiledConditionCall {
                function: "fallback".into(),
                args: vec![],
            },
            &other,
        )
        .unwrap();
    let guard = distinct.guard(condition, span).unwrap();
    let empty = Bindings::new();
    let missing = empty.guard(condition, span).unwrap_err();
    assert!(matches!(
        missing.kind,
        leitmotif_recite::ErrorKind::Unmapped(_)
    ));
    assert_eq!(
        (missing.provenance.file.as_str(), missing.provenance.line),
        (span.file.as_str(), span.start.line())
    );
    let state = model.state([enabled.set(true), other.set(false)]).unwrap();
    let mut session = start_scene(&asset, None).unwrap();
    assert!(matches!(
        next(&asset, &mut session, &empty.context(&model, &state)),
        Err(recite_runtime::DialogueError::ConditionEvaluationFailed {
            kind: recite_runtime::ConditionEvaluationErrorKind::MissingHandler,
            ..
        })
    ));

    for a in [false, true] {
        for b in [false, true] {
            let state = model.state([enabled.set(a), other.set(b)]).unwrap();
            let context = distinct.context(&model, &state);
            let calls = RefCell::new(Vec::new());
            let recording = |query: ConditionQuery<'_>| {
                calls.borrow_mut().push(query.function().to_owned());
                context.evaluate_condition(query)
            };
            let mut session = start_scene(&asset, None).unwrap();
            let DialogueEvent::Line(line) = next(&asset, &mut session, &recording).unwrap() else {
                panic!("expected branch line")
            };
            assert_eq!(line.text == "Yes.", model.evaluate(&state, &guard).unwrap());
            assert_eq!(calls.borrow().len(), if a && b { 3 } else { 2 });
        }
    }
}

#[test]
fn compiled_enum_scrutinee_matches_runtime_variants() {
    let asset = compile(":: start default\n:match mood()\n  :case tired\n    > tired@8843fd6f53f020a12b31\n      Tired.\n  :case _\n    > rested@4dff358182d02d9090e3\n      Rested.\n-> END\n").unwrap();
    let statement = asset
        .statements
        .iter()
        .find(|row| matches!(row.kind, CompiledStatementKind::Match { .. }))
        .unwrap();
    let CompiledStatementKind::Match { scrutinee, .. } = &statement.kind else {
        panic!("missing match")
    };
    let span = &asset
        .source_maps
        .get(usize::try_from(statement.source_map.as_u32()).unwrap())
        .unwrap()
        .span;
    let mut builder = ModelBuilder::new("host").unwrap();
    let mood_fact = builder.fact::<Stage>().unwrap();
    let enabled = builder.fact::<Enabled>().unwrap();
    let model = builder.build().unwrap();
    let mut bindings = Bindings::new();
    bindings.bind_enum(scrutinee.clone(), &mood_fact).unwrap();
    let tired = bindings.enum_is(scrutinee, "tired", span).unwrap();
    for mood in [Mood::Tired, Mood::Rested] {
        let state = model
            .state([mood_fact.set(mood), enabled.set(true)])
            .unwrap();
        let mut session = start_scene(&asset, None).unwrap();
        let DialogueEvent::Line(line) =
            next(&asset, &mut session, &bindings.context(&model, &state)).unwrap()
        else {
            panic!("expected match line")
        };
        assert_eq!(
            line.text == "Tired.",
            model.evaluate(&state, &tired).unwrap()
        );
    }
    let mut wrong_type = Bindings::new();
    wrong_type.bind_bool(scrutinee.clone(), &enabled).unwrap();
    let state = model
        .state([mood_fact.set(Mood::Tired), enabled.set(true)])
        .unwrap();
    let mut session = start_scene(&asset, None).unwrap();
    assert!(matches!(
        next(&asset, &mut session, &wrong_type.context(&model, &state)),
        Err(recite_runtime::DialogueError::ConditionEvaluationFailed {
            kind: recite_runtime::ConditionEvaluationErrorKind::InvalidResult,
            ..
        })
    ));
    let foreign_model = ModelBuilder::new("foreign").unwrap().build().unwrap();
    let foreign = foreign_model.state([]).unwrap();
    let mut session = start_scene(&asset, None).unwrap();
    assert!(matches!(
        next(&asset, &mut session, &bindings.context(&model, &foreign)),
        Err(recite_runtime::DialogueError::ConditionEvaluationFailed {
            kind: recite_runtime::ConditionEvaluationErrorKind::EvaluationFailed,
            ..
        })
    ));
}

#[test]
fn choice_availability_is_captured_before_the_host_snapshot_changes() {
    let asset = compile(":: start default\n> prompt@8843fd6f53f020a12b31\n  What next?\n  ? enter@4dff358182d02d9090e3 requires=(ready())\n    Enter.\n    -> END\n").unwrap();
    let choice = asset.choices.first().unwrap();
    let span = &asset
        .source_maps
        .get(usize::try_from(choice.source_map.as_u32()).unwrap())
        .unwrap()
        .span;
    let mut builder = ModelBuilder::new("host").unwrap();
    let enabled = builder.fact::<Enabled>().unwrap();
    let model = builder
        .with_transition(Transition::new("disable").with_effect(enabled.set(false)))
        .build()
        .unwrap();
    let initial = model.state([enabled.set(true)]).unwrap();
    let changed = model.apply(&initial, "disable").unwrap().unwrap();
    let mut bindings = Bindings::new();
    bindings
        .bind_bool(
            CompiledConditionCall {
                function: "ready".into(),
                args: vec![],
            },
            &enabled,
        )
        .unwrap();
    let guard = bindings
        .guard(choice.availability_requirement.as_ref().unwrap(), span)
        .unwrap();
    let mut session = start_scene(&asset, None).unwrap();
    let DialogueEvent::Prompt { choices, .. } =
        next(&asset, &mut session, &bindings.context(&model, &initial)).unwrap()
    else {
        panic!("expected prompt")
    };
    assert_eq!(
        choices.first().unwrap().availability.is_available,
        model.evaluate(&initial, &guard).unwrap()
    );
    assert!(!model.evaluate(&changed, &guard).unwrap());
    assert!(matches!(
        choose(
            &asset,
            &mut session,
            choice.id.clone(),
            &bindings.context(&model, &changed)
        )
        .unwrap(),
        DialogueEvent::End { .. }
    ));
}

#[test]
fn mixed_deep_and_wide_compiled_guards_fail_explicitly_on_a_worker_stack() {
    thread::Builder::new().stack_size(2_097_152).spawn(|| {
        let mut expression = "ready()".to_owned();
        for level in 0..128 {
            let siblings = if level % 2 == 0 { " and ready()" } else { " or ready()" };
            expression = format!("({expression}){}", siblings.repeat(16));
        }
        let source = format!(":: start default\n:if {expression}\n  > yes@8843fd6f53f020a12b31\n    Yes.\n:else\n  > no@4dff358182d02d9090e3\n    No.\n-> END\n");
        let asset = compile(&source).unwrap();
        let statement = asset.statements.iter().find(|row| matches!(row.kind, CompiledStatementKind::If { .. })).unwrap();
        let CompiledStatementKind::If { condition, .. } = &statement.kind else { panic!("missing if") };
        let span = &asset.source_maps.get(usize::try_from(statement.source_map.as_u32()).unwrap()).unwrap().span;
        let mut builder = ModelBuilder::new("host").unwrap();
        let enabled = builder.fact::<Enabled>().unwrap();
        let model = builder.build().unwrap();
        let state = model.state([enabled.set(true)]).unwrap();
        let mut bindings = Bindings::new();
        bindings.bind_bool(CompiledConditionCall { function: "ready".into(), args: vec![] }, &enabled).unwrap();
        let mut session = start_scene(&asset, None).unwrap();
        let DialogueEvent::Line(line) = next(&asset, &mut session, &bindings.context(&model, &state)).unwrap() else { panic!("expected line") };
        assert_eq!(line.text, "Yes.");
        let result = bindings.guard(condition, span);
        assert!(result.is_err(), "binary lowering must reject excessive depth");
        if let Err(error) = result {
            assert!(matches!(error.kind, leitmotif_recite::ErrorKind::Unsupported(_)));
            assert_eq!((error.provenance.file.as_str(), error.provenance.line), ("story.recite", 2));
        }
    }).unwrap().join().unwrap();
}

mod blocking_effects;
