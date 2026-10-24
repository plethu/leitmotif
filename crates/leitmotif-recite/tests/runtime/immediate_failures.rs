use std::error::Error;

use leitmotif::{ErrorKind, Fact, FactKind, ModelBuilder, Predicate, Transition};
use leitmotif_recite::{Bindings, EffectErrorKind, ImmediateEffects};
use recite_core::{
    ScalarValue,
    compiled::{CompiledArgument, CompiledConditionCall, CompiledEffectMode},
};
use recite_runtime::{
    DialogueEffectMode, DialogueEvent, next, snapshot::snapshot_session, start_scene,
};

use super::{
    immediate_effects::STORY,
    support::{Enabled, Other, compile},
};

#[test]
fn unmapped_runtime_requests_leave_host_unchanged_without_rewinding_dialogue() {
    let asset = compile(STORY).unwrap();
    let effect = asset.effects.first().unwrap();
    let mut builder = ModelBuilder::new("host").unwrap();
    let enabled = builder.fact::<Enabled>().unwrap();
    let model = builder
        .with_transition(Transition::new("grant").with_effect(enabled.set(true)))
        .build()
        .unwrap();
    let host = model.state([enabled.set(false)]).unwrap();
    let snapshot = model.snapshot(&host).unwrap();
    let mut conditions = Bindings::new();
    conditions
        .bind_bool(
            CompiledConditionCall {
                function: "ready".into(),
                args: vec![],
            },
            &enabled,
        )
        .unwrap();
    let mut session = start_scene(&asset, None).unwrap();
    let before = snapshot_session(&session);
    let DialogueEvent::Effect(request) =
        next(&asset, &mut session, &conditions.context(&model, &host)).unwrap()
    else {
        panic!("expected request")
    };
    let emitted = snapshot_session(&session);
    assert_ne!(before.next_statement, emitted.next_statement);
    for (index, argument) in [
        (
            0,
            CompiledArgument::Value(ScalarValue::String("player".into())),
        ),
        (1, CompiledArgument::Identifier("gift".into())),
        (2, CompiledArgument::Value(ScalarValue::Float(3.0))),
        (3, CompiledArgument::Value(ScalarValue::Float(0.75))),
        (4, CompiledArgument::Value(ScalarValue::Boolean(false))),
    ] {
        let mut changed = effect.clone();
        changed.args[index] = argument;
        let mut registry = ImmediateEffects::new(&model);
        registry.bind(&changed, "grant").unwrap();
        let failure = registry.apply(&request, &host).unwrap_err();
        assert!(matches!(failure.kind, EffectErrorKind::Unmapped(_)));
        assert_eq!(failure.provenance.file, request.source_span.file);
        assert_eq!(failure.provenance.line, request.source_span.start.line());
        assert_eq!(
            failure.provenance.column,
            request.source_span.start.column()
        );
        assert_eq!(model.snapshot(&host).unwrap(), snapshot);
        assert_eq!(snapshot_session(&session), emitted);
    }
    for change_function in [true, false] {
        let mut changed = effect.clone();
        if change_function {
            changed.function.push_str("_other");
        } else {
            changed.args.pop();
        }
        let mut registry = ImmediateEffects::new(&model);
        registry.bind(&changed, "grant").unwrap();
        assert!(matches!(
            registry.apply(&request, &host).unwrap_err().kind,
            EffectErrorKind::Unmapped(_)
        ));
    }
    let empty = ImmediateEffects::new(&model);
    assert!(matches!(
        empty.apply(&request, &host).unwrap_err().kind,
        EffectErrorKind::Unmapped(_)
    ));
    let DialogueEvent::Line(line) =
        next(&asset, &mut session, &conditions.context(&model, &host)).unwrap()
    else {
        panic!("expected following branch")
    };
    assert_eq!(line.text, "Unchanged.");
}

#[test]
fn disabled_and_foreign_host_failures_preserve_the_pre_state_and_runtime_position() {
    let asset = compile(STORY).unwrap();
    let effect = asset.effects.first().unwrap();
    let mut builder = ModelBuilder::new("host").unwrap();
    let enabled = builder.fact::<Enabled>().unwrap();
    let model = builder
        .with_transition(
            Transition::new("grant")
                .with_guard(enabled.is(true))
                .with_effect(enabled.set(false)),
        )
        .build()
        .unwrap();
    let host = model.state([enabled.set(false)]).unwrap();
    let mut registry = ImmediateEffects::new(&model);
    registry.bind(effect, "grant").unwrap();
    let conditions = Bindings::new();
    let mut session = start_scene(&asset, None).unwrap();
    let DialogueEvent::Effect(request) =
        next(&asset, &mut session, &conditions.context(&model, &host)).unwrap()
    else {
        panic!("expected request")
    };
    let emitted = snapshot_session(&session);
    let before = model.snapshot(&host).unwrap();
    assert!(
        matches!(registry.apply(&request, &host).unwrap_err().kind, EffectErrorKind::DisabledTransition(id) if id == "grant")
    );
    assert_eq!(model.snapshot(&host).unwrap(), before);
    let foreign_model = ModelBuilder::new("foreign").unwrap().build().unwrap();
    let foreign = foreign_model.state([]).unwrap();
    let failure = registry.apply(&request, &foreign).unwrap_err();
    assert!(
        matches!(&failure.kind, EffectErrorKind::Host(error) if matches!(error.kind, ErrorKind::ForeignState))
    );
    assert!(failure.source().and_then(Error::source).is_some());
    assert_eq!(snapshot_session(&session), emitted);
    assert_eq!(model.snapshot(&host).unwrap(), before);
}

#[test]
fn invalid_registration_and_other_effect_modes_are_explicit_without_losing_valid_bindings() {
    let asset = compile(STORY).unwrap();
    let effect = asset.effects.first().unwrap();
    let mut builder = ModelBuilder::new("host").unwrap();
    let enabled = builder.fact::<Enabled>().unwrap();
    let model = builder
        .with_transition(Transition::new("grant").with_effect(enabled.set(true)))
        .build()
        .unwrap();
    let host = model.state([enabled.set(false)]).unwrap();
    let mut registry = ImmediateEffects::new(&model);
    let unknown = registry.bind(effect, "missing").unwrap_err();
    assert!(matches!(unknown.kind, EffectErrorKind::UnknownTransition(id) if id == "missing"));
    registry.bind(effect, "grant").unwrap();
    let mut distinct = effect.clone();
    distinct.args.pop();
    registry.bind(&distinct, "grant").unwrap();
    let duplicate = registry.bind(effect, "grant").unwrap_err();
    assert!(matches!(duplicate.kind, EffectErrorKind::Duplicate(name) if name == "grant"));
    assert!(duplicate.provenance.file.ends_with("immediate_failures.rs"));
    let mut session = start_scene(&asset, None).unwrap();
    let conditions = Bindings::new();
    let DialogueEvent::Effect(request) =
        next(&asset, &mut session, &conditions.context(&model, &host)).unwrap()
    else {
        panic!("expected request")
    };
    for (compiled_mode, runtime_mode) in [
        (CompiledEffectMode::Deferred, DialogueEffectMode::Deferred),
        (CompiledEffectMode::Blocking, DialogueEffectMode::Blocking),
    ] {
        let mut wrong = effect.clone();
        wrong.mode = compiled_mode;
        assert!(matches!(
            registry.bind(&wrong, "grant").unwrap_err().kind,
            EffectErrorKind::NotImmediate
        ));
        let mut wrong = request.clone();
        wrong.mode = runtime_mode;
        assert!(matches!(
            registry.apply(&wrong, &host).unwrap_err().kind,
            EffectErrorKind::NotImmediate
        ));
    }
    let applied = registry.apply(&request, &host).unwrap();
    assert_eq!(applied.request(), &request);
    assert!(
        model
            .evaluate(&applied.into_state(), &enabled.is(true))
            .unwrap()
    );
}

struct Setting;
impl Fact for Setting {
    type Value = bool;
    const KEY: &'static str = "setting";
    const KIND: FactKind = FactKind::Setting;
}

#[test]
fn host_declarations_reject_setting_writes_and_duplicate_simultaneous_assignments() {
    let mut builder = ModelBuilder::new("settings").unwrap();
    let setting = builder.fact::<Setting>().unwrap();
    let invalid = builder
        .with_transition(
            Transition::new("grant")
                .with_guard(Predicate::constant(false))
                .with_effect(setting.set(true)),
        )
        .build()
        .unwrap_err();
    assert!(matches!(invalid.kind, ErrorKind::SettingWrite(_)));
    let mut builder = ModelBuilder::new("duplicates").unwrap();
    let other = builder.fact::<Other>().unwrap();
    let invalid = builder
        .with_transition(
            Transition::new("grant")
                .with_effect(other.set(false))
                .with_effect(other.set(true)),
        )
        .build()
        .unwrap_err();
    assert!(matches!(invalid.kind, ErrorKind::DuplicateWrite(_)));
}
