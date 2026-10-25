use std::cell::RefCell;

use leitmotif_recite::{EffectErrorKind, ImmediateEffects};
use recite_runtime::{
    ConditionQuery, ConditionValue, DialogueContext, DialogueEvent, next,
    snapshot::snapshot_session, start_scene,
};

use super::super::support::compile;

mod host;
use host::{Host, STORY};

#[test]
fn mixed_modes_preserve_queued_state_queries_and_captured_choice() {
    let asset = compile(STORY).unwrap();
    let host = Host::new().unwrap();
    let mut state = host.initial.clone();
    let mut immediate = ImmediateEffects::new(&host.model);
    immediate
        .bind(
            asset
                .effects
                .iter()
                .find(|effect| effect.function == "cue")
                .unwrap(),
            "cue",
        )
        .unwrap();
    let mut session = start_scene(&asset, None).unwrap();
    let DialogueEvent::Effect(cue) = next(
        &asset,
        &mut session,
        &host.conditions.context(&host.model, &state),
    )
    .unwrap() else {
        panic!("expected later immediate request")
    };
    assert_eq!(cue.function, "cue");
    assert_eq!(session.deferred_effects().len(), 1);
    assert_eq!(
        session.deferred_effects().first().unwrap().function,
        "grant"
    );
    assert_eq!(
        host.model.snapshot(&state).unwrap(),
        host.model.snapshot(&host.initial).unwrap()
    );
    state = immediate.apply(&cue, &state).unwrap().into_state();
    assert!(
        host.model
            .evaluate(&state, &host.other.is(true).and(host.enabled.is(false)))
            .unwrap()
    );
    let queries = RefCell::new(Vec::new());
    let context = host.conditions.context(&host.model, &state);
    let recording = |query: ConditionQuery<'_>| {
        let function = query.function().to_owned();
        let result = context.evaluate_condition(query);
        queries.borrow_mut().push((
            function,
            result.as_ref().cloned().map_err(ToString::to_string),
        ));
        result
    };
    let DialogueEvent::Prompt { choices, .. } = next(&asset, &mut session, &recording).unwrap()
    else {
        panic!("expected captured prompt")
    };
    assert_eq!(
        *queries.borrow(),
        vec![
            ("ready".into(), Ok(ConditionValue::Bool(false))),
            ("ready".into(), Ok(ConditionValue::Bool(false)))
        ]
    );
    let choice = choices.first().unwrap();
    assert!(choice.availability.is_available);
    assert_eq!(
        session
            .deferred_effects()
            .iter()
            .map(|request| request.function.as_str())
            .collect::<Vec<_>>(),
        ["grant", "record_else"]
    );
}

#[test]
fn reached_end_list_preserves_sources_and_host_ordered_application() {
    let asset = compile(STORY).unwrap();
    let host = Host::new().unwrap();
    let deferred = host.deferred(&asset).unwrap();
    let (session, mut state, delivered) = host.finish_reached_path(&asset).unwrap();
    assert_eq!(delivered, session.deferred_effects());
    assert_eq!(
        delivered
            .iter()
            .map(|request| request.function.as_str())
            .collect::<Vec<_>>(),
        ["grant", "record_else", "finish"]
    );
    assert!(
        host.model
            .evaluate(
                &state,
                &host
                    .enabled
                    .is(false)
                    .and(host.visited.is(false))
                    .and(host.finished.is(false))
            )
            .unwrap()
    );
    let ended = snapshot_session(&session);
    for (position, request) in delivered.iter().enumerate() {
        let compiled = asset
            .effects
            .iter()
            .find(|effect| effect.function == request.function)
            .unwrap();
        assert_eq!(request.id, compiled.id);
        let source = &asset
            .source_maps
            .get(usize::try_from(compiled.source_map.as_u32()).unwrap())
            .unwrap()
            .span;
        assert_eq!(&request.source_span, source);
        let receipt = deferred.apply_at(&session, position, &state).unwrap();
        assert_eq!(receipt.request(), request);
        assert_eq!(receipt.transition().id, request.function);
        state = receipt.into_state();
        assert_eq!(snapshot_session(&session), ended);
    }
    assert!(
        host.model
            .evaluate(
                &state,
                &host
                    .enabled
                    .is(true)
                    .and(host.other.is(true))
                    .and(host.visited.is(true))
                    .and(host.finished.is(true))
            )
            .unwrap()
    );
}

#[test]
fn premature_host_application_changes_the_query_prompt_and_reached_queue() {
    let asset = compile(STORY).unwrap();
    let host = Host::new().unwrap();
    let deferred = host.deferred(&asset).unwrap();
    let mut immediate = ImmediateEffects::new(&host.model);
    immediate
        .bind(
            asset
                .effects
                .iter()
                .find(|effect| effect.function == "cue")
                .unwrap(),
            "cue",
        )
        .unwrap();
    let mut early = start_scene(&asset, None).unwrap();
    let DialogueEvent::Effect(cue) = next(
        &asset,
        &mut early,
        &host.conditions.context(&host.model, &host.initial),
    )
    .unwrap() else {
        panic!("expected immediate request")
    };
    let state = immediate.apply(&cue, &host.initial).unwrap().into_state();
    assert!(matches!(
        deferred.apply_at(&early, 0, &state).unwrap_err().kind,
        EffectErrorKind::NotEnded
    ));
    let mut correct = early.clone();
    let DialogueEvent::Prompt {
        choices: correct_choices,
        ..
    } = next(
        &asset,
        &mut correct,
        &host.conditions.context(&host.model, &state),
    )
    .unwrap()
    else {
        panic!("expected correct prompt")
    };
    // Bypass the bridge deliberately to reproduce the invalid early-commit policy.
    let premature = host.model.apply(&state, "grant").unwrap().unwrap();
    let DialogueEvent::Prompt {
        choices: early_choices,
        ..
    } = next(
        &asset,
        &mut early,
        &host.conditions.context(&host.model, &premature),
    )
    .unwrap()
    else {
        panic!("expected altered prompt")
    };
    assert!(correct_choices.first().unwrap().availability.is_available);
    assert!(!early_choices.first().unwrap().availability.is_available);
    assert_eq!(
        correct
            .deferred_effects()
            .iter()
            .map(|request| request.function.as_str())
            .collect::<Vec<_>>(),
        ["grant", "record_else"]
    );
    assert_eq!(
        early
            .deferred_effects()
            .iter()
            .map(|request| request.function.as_str())
            .collect::<Vec<_>>(),
        ["grant", "impossible"]
    );
    assert_ne!(early_choices, correct_choices);
    assert_ne!(early.deferred_effects(), correct.deferred_effects());
}
