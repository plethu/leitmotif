//! Typed guard bindings and unsupported compiled-expression boundaries.
use std::thread;

use leitmotif::{Fact, ModelBuilder};
use leitmotif_recite::Bindings;
use recite_core::{
    SourcePosition, SourceSpan,
    compiled::{CompiledConditionCall, CompiledConditionExpression, MAX_COMPILED_CONDITION_DEPTH},
};
struct Flag;
impl Fact for Flag {
    type Value = bool;
    const KEY: &'static str = "flag";
}
#[test]
fn boolean_guard_uses_the_bound_typed_fact() {
    let mut builder = ModelBuilder::new("host").unwrap();
    let flag = builder.fact::<Flag>().unwrap();
    let model = builder.build().unwrap();
    let call = CompiledConditionCall {
        function: "ready".into(),
        args: vec![],
    };
    let mut bindings = Bindings::new();
    bindings.bind_bool(call.clone(), &flag).unwrap();
    let span = SourceSpan::point("story.recite", SourcePosition::new(2, 1).unwrap());
    let guard = bindings
        .guard(&CompiledConditionExpression::Call(call), &span)
        .unwrap();
    for value in [false, true] {
        let state = model.state([flag.set(value)]).unwrap();
        assert_eq!(model.evaluate(&state, &guard).unwrap(), value);
    }
}

#[derive(Clone)]
enum Mood {
    Tired,
    Rested,
}
impl leitmotif::Domain for Mood {
    fn values() -> Vec<Self> {
        vec![Self::Tired, Self::Rested]
    }
    fn encode(self) -> leitmotif::Value {
        leitmotif::Value::Symbol(
            match self {
                Self::Tired => "tired",
                Self::Rested => "rested",
            }
            .into(),
        )
    }
}
struct Stage;
impl Fact for Stage {
    type Value = Mood;
    const KEY: &'static str = "stage";
}

#[test]
fn binding_and_lowering_errors_are_explicit_and_source_linked() {
    use leitmotif_recite::ErrorKind;
    let mut builder = ModelBuilder::new("host").unwrap();
    let flag = builder.fact::<Flag>().unwrap();
    let stage = builder.fact::<Stage>().unwrap();
    let call = CompiledConditionCall {
        function: "ready".into(),
        args: vec![],
    };
    let enum_call = CompiledConditionCall {
        function: "mood".into(),
        args: vec![],
    };
    let span = SourceSpan::point("story.recite", SourcePosition::new(8, 3).unwrap());
    let mut bindings = Bindings::new();
    let missing = bindings
        .guard(&CompiledConditionExpression::Call(call.clone()), &span)
        .unwrap_err();
    assert!(matches!(missing.kind, ErrorKind::Unmapped(_)));
    assert_eq!(
        (
            missing.provenance.file.as_str(),
            missing.provenance.line,
            missing.provenance.column
        ),
        ("story.recite", 8, 3)
    );
    bindings.bind_bool(call.clone(), &flag).unwrap();
    assert!(matches!(
        bindings.bind_bool(call.clone(), &flag).unwrap_err().kind,
        ErrorKind::Duplicate(_)
    ));
    assert!(matches!(
        bindings
            .bind_enum(enum_call.clone(), &flag)
            .unwrap_err()
            .kind,
        ErrorKind::NonSymbolicDomain
    ));
    bindings.bind_enum(enum_call.clone(), &stage).unwrap();
    assert!(matches!(
        bindings
            .guard(&CompiledConditionExpression::Call(enum_call.clone()), &span)
            .unwrap_err()
            .kind,
        ErrorKind::WrongType(_)
    ));
    assert!(matches!(
        bindings.enum_is(&call, "tired", &span).unwrap_err().kind,
        ErrorKind::WrongType(_)
    ));
    assert!(matches!(
        bindings
            .enum_is(&enum_call, "missing", &span)
            .unwrap_err()
            .kind,
        ErrorKind::UnknownVariant(_)
    ));
    for expression in [
        CompiledConditionExpression::And(vec![]),
        CompiledConditionExpression::Or(vec![]),
    ] {
        let error = bindings.guard(&expression, &span).unwrap_err();
        assert!(matches!(error.kind, ErrorKind::Unsupported(_)));
        assert_eq!(error.provenance.line, 8);
    }
    let mut expression = CompiledConditionExpression::Call(call);
    for _ in 0..=MAX_COMPILED_CONDITION_DEPTH {
        expression = CompiledConditionExpression::Not(Box::new(expression));
    }
    assert!(matches!(
        bindings.guard(&expression, &span).unwrap_err().kind,
        ErrorKind::Unsupported(_)
    ));
}

#[test]
fn fixed_wide_guard_bounds_lowered_tree_size_and_depth() {
    let mut builder = ModelBuilder::new("host").unwrap();
    let flag = builder.fact::<Flag>().unwrap();
    let model = builder.build().unwrap();
    let call = CompiledConditionCall {
        function: "ready".into(),
        args: vec![],
    };
    let mut bindings = Bindings::new();
    bindings.bind_bool(call.clone(), &flag).unwrap();
    let span = SourceSpan::point("story.recite", SourcePosition::new(2, 1).unwrap());
    let expression =
        CompiledConditionExpression::And(vec![CompiledConditionExpression::Call(call); 1025]);
    let guard = bindings.guard(&expression, &span).unwrap();
    let mut pending = vec![(&guard, 1_usize)];
    let mut measured = Vec::new();
    while let Some((predicate, depth)) = pending.pop() {
        measured.push(depth);
        match &predicate.kind {
            leitmotif::PredicateKind::And(left, right)
            | leitmotif::PredicateKind::Or(left, right) => {
                pending.push((left, depth.saturating_add(1)));
                pending.push((right, depth.saturating_add(1)));
            }
            leitmotif::PredicateKind::Not(inner) => pending.push((inner, depth.saturating_add(1))),
            leitmotif::PredicateKind::Constant(_) | leitmotif::PredicateKind::Eq(_) => {}
        }
    }
    // Measured at 2,049 nodes and 12 levels for this fixed 1,025-call group.
    // Bounds prevent duplicated output or linear nesting; no runtime ceiling is claimed.
    assert!(measured.len() <= 2049);
    assert!(measured.iter().max().copied().unwrap() <= 12);
    for value in [false, true] {
        let state = model.state([flag.set(value)]).unwrap();
        assert_eq!(model.evaluate(&state, &guard).unwrap(), value);
    }
}

#[test]
fn lowered_binary_depth_accepts_128_edges_and_rejects_129() {
    thread::Builder::new()
        .stack_size(2_097_152)
        .spawn(|| {
            let mut builder = ModelBuilder::new("host").unwrap();
            let flag = builder.fact::<Flag>().unwrap();
            let model = builder.build().unwrap();
            let call = CompiledConditionCall {
                function: "ready".into(),
                args: vec![],
            };
            let mut bindings = Bindings::new();
            bindings.bind_bool(call.clone(), &flag).unwrap();
            let span = SourceSpan::point("story.recite", SourcePosition::new(2, 1).unwrap());
            for unary_edges in [125, 126] {
                let mut nested = CompiledConditionExpression::Call(call.clone());
                for _ in 0..unary_edges {
                    nested = CompiledConditionExpression::Not(Box::new(nested));
                }
                let mut children = vec![nested];
                children.extend(vec![CompiledConditionExpression::Call(call.clone()); 4]);
                let expression = CompiledConditionExpression::Or(children);
                let result = bindings.guard(&expression, &span);
                if unary_edges == 126 {
                    assert!(matches!(
                        result.unwrap_err().kind,
                        leitmotif_recite::ErrorKind::Unsupported(
                            "lowered binary predicate depth exceeds 128 edges"
                        )
                    ));
                    continue;
                }
                let guard = result.unwrap();
                for value in [false, true] {
                    let state = model.state([flag.set(value)]).unwrap();
                    assert!(model.evaluate(&state, &guard).unwrap());
                }
            }
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn exact_call_keys_reject_scalar_kind_value_and_arity_mismatches() {
    use recite_core::{ScalarValue, compiled::CompiledArgument};
    let mut builder = ModelBuilder::new("host").unwrap();
    let flag = builder.fact::<Flag>().unwrap();
    let call = CompiledConditionCall {
        function: "ready".into(),
        args: vec![
            CompiledArgument::Identifier("actor".into()),
            CompiledArgument::Value(ScalarValue::String("name".into())),
            CompiledArgument::Value(ScalarValue::Integer(3)),
            CompiledArgument::Value(ScalarValue::Float(0.5)),
            CompiledArgument::Value(ScalarValue::Boolean(true)),
        ],
    };
    let span = SourceSpan::point("story.recite", SourcePosition::new(2, 1).unwrap());
    let mut bindings = Bindings::new();
    bindings.bind_bool(call.clone(), &flag).unwrap();
    for (index, replacement) in [
        (0, CompiledArgument::Identifier("other".into())),
        (
            0,
            CompiledArgument::Value(ScalarValue::String("actor".into())),
        ),
        (
            1,
            CompiledArgument::Value(ScalarValue::String("other".into())),
        ),
        (2, CompiledArgument::Value(ScalarValue::Integer(4))),
        (2, CompiledArgument::Value(ScalarValue::Float(3.0))),
        (3, CompiledArgument::Value(ScalarValue::Float(0.75))),
        (4, CompiledArgument::Value(ScalarValue::Boolean(false))),
    ] {
        let mut changed = call.clone();
        changed.args[index] = replacement;
        assert!(matches!(
            bindings
                .guard(&CompiledConditionExpression::Call(changed), &span)
                .unwrap_err()
                .kind,
            leitmotif_recite::ErrorKind::Unmapped(_)
        ));
    }
    let mut changed = call;
    changed.args.pop();
    assert!(matches!(
        bindings
            .guard(&CompiledConditionExpression::Call(changed), &span)
            .unwrap_err()
            .kind,
        leitmotif_recite::ErrorKind::Unmapped(_)
    ));
}

#[test]
fn unmapped_calls_are_rejected_even_in_short_circuited_branches() {
    let mut builder = ModelBuilder::new("host").unwrap();
    let flag = builder.fact::<Flag>().unwrap();
    let ready = CompiledConditionCall {
        function: "ready".into(),
        args: vec![],
    };
    let missing = CompiledConditionExpression::Call(CompiledConditionCall {
        function: "missing".into(),
        args: vec![],
    });
    let span = SourceSpan::point("story.recite", SourcePosition::new(2, 1).unwrap());
    let mut bindings = Bindings::new();
    bindings.bind_bool(ready.clone(), &flag).unwrap();
    let ready = CompiledConditionExpression::Call(ready);
    let known_false = CompiledConditionExpression::And(vec![
        ready.clone(),
        CompiledConditionExpression::Not(Box::new(ready.clone())),
    ]);
    let known_true = CompiledConditionExpression::Or(vec![
        ready.clone(),
        CompiledConditionExpression::Not(Box::new(ready)),
    ]);
    for expression in [
        CompiledConditionExpression::And(vec![known_false, missing.clone()]),
        CompiledConditionExpression::Or(vec![known_true, missing]),
    ] {
        assert!(matches!(
            bindings.guard(&expression, &span).unwrap_err().kind,
            leitmotif_recite::ErrorKind::Unmapped(_)
        ));
    }
}
