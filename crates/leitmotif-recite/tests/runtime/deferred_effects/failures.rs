use leitmotif::{ErrorKind, ModelBuilder, Transition};
use leitmotif_recite::{Bindings, DeferredEffects, EffectErrorKind};
use recite_core::compiled::CompiledEffectMode;
use recite_runtime::{
    DialogueEffectMode, DialogueEvent, next, snapshot::snapshot_session, start_scene,
};

use super::super::support::{Enabled, Other, compile};

#[test]
fn host_stop_on_failure_preserves_the_committed_prefix_and_leaves_the_suffix_unattempted() {
    let asset = compile(
        ":: start default\n! deferred grant()\n! deferred middle()\n! deferred suffix()\n-> END\n",
    )
    .unwrap();
    let mut builder = ModelBuilder::new("partial-deferred").unwrap();
    let enabled = builder.fact::<Enabled>().unwrap();
    let other = builder.fact::<Other>().unwrap();
    let model = builder
        .with_transition(Transition::new("grant").with_effect(enabled.set(true)))
        .with_transition(
            Transition::new("middle")
                .with_guard(enabled.is(false))
                .with_effect(enabled.set(false)),
        )
        .with_transition(Transition::new("suffix").with_effect(other.set(true)))
        .build()
        .unwrap();
    let initial = model.state([enabled.set(false), other.set(false)]).unwrap();
    let conditions = Bindings::new();
    let mut session = start_scene(&asset, None).unwrap();
    let DialogueEvent::End {
        deferred_effects: delivered,
    } = next(&asset, &mut session, &conditions.context(&model, &initial)).unwrap()
    else {
        panic!("expected End")
    };
    assert_eq!(delivered.len(), 3);
    let ended = snapshot_session(&session);
    for bind_middle in [false, true] {
        let mut registry = DeferredEffects::new(&model);
        for effect in asset
            .effects
            .iter()
            .filter(|effect| effect.function != "middle" || bind_middle)
        {
            registry.bind(effect, &effect.function).unwrap();
        }
        let mut state = initial.clone();
        let mut committed = Vec::new();
        let mut failed = None;
        // This is the host's explicit ordered, stop-on-first-failure policy.
        for (position, request) in delivered.iter().enumerate() {
            match registry.apply_at(&session, position, &state) {
                Ok(receipt) => {
                    assert_eq!(receipt.request(), request);
                    state = receipt.into_state();
                    committed.push(position);
                }
                Err(error) => {
                    failed = Some((position, error));
                    break;
                }
            }
        }
        assert_eq!(committed, [0]);
        let (position, error) = failed.unwrap();
        assert_eq!(position, 1);
        if bind_middle {
            assert!(matches!(error.kind, EffectErrorKind::DisabledTransition(_)));
        } else {
            assert!(matches!(error.kind, EffectErrorKind::Unmapped(_)));
        }
        assert_eq!(
            error.provenance.line,
            delivered.get(1).unwrap().source_span.start.line()
        );
        let expected = model.state([enabled.set(true), other.set(false)]).unwrap();
        assert_eq!(
            model.snapshot(&state).unwrap(),
            model.snapshot(&expected).unwrap()
        );
        assert_eq!(snapshot_session(&session), ended);
        assert_eq!(session.deferred_effects(), delivered);
    }
}

#[test]
fn nonended_sessions_invalid_positions_and_wrong_modes_are_explicit() {
    let asset = compile(":: start default\n! deferred grant()\n-> END\n").unwrap();
    let mut builder = ModelBuilder::new("deferred-boundaries").unwrap();
    let enabled = builder.fact::<Enabled>().unwrap();
    let model = builder
        .with_transition(Transition::new("grant").with_effect(enabled.set(true)))
        .build()
        .unwrap();
    let state = model.state([enabled.set(false)]).unwrap();
    let mut registry = DeferredEffects::new(&model);
    let effect = asset.effects.first().unwrap();
    for (mode, expected) in [
        (CompiledEffectMode::Immediate, DialogueEffectMode::Immediate),
        (CompiledEffectMode::Blocking, DialogueEffectMode::Blocking),
    ] {
        let mut wrong = effect.clone();
        wrong.mode = mode;
        assert!(
            matches!(registry.bind(&wrong, "grant").unwrap_err().kind, EffectErrorKind::WrongMode { expected: DialogueEffectMode::Deferred, actual } if actual == expected)
        );
    }
    assert!(matches!(
        registry.bind(effect, "unknown").unwrap_err().kind,
        EffectErrorKind::UnknownTransition(_)
    ));
    registry.bind(effect, "grant").unwrap();
    assert!(matches!(
        registry.bind(effect, "grant").unwrap_err().kind,
        EffectErrorKind::Duplicate(_)
    ));
    let conditions = Bindings::new();
    let mut session = start_scene(&asset, None).unwrap();
    assert!(matches!(
        registry.apply_at(&session, 0, &state).unwrap_err().kind,
        EffectErrorKind::NotEnded
    ));
    assert!(matches!(
        next(&asset, &mut session, &conditions.context(&model, &state)).unwrap(),
        DialogueEvent::End { .. }
    ));
    for position in [1, usize::MAX] {
        assert!(
            matches!(registry.apply_at(&session, position, &state).unwrap_err().kind, EffectErrorKind::InvalidDeferredPosition { position: actual, len: 1 } if actual == position)
        );
    }
    let foreign_model = ModelBuilder::new("foreign").unwrap().build().unwrap();
    let foreign = foreign_model.state([]).unwrap();
    assert!(
        matches!(registry.apply_at(&session, 0, &foreign).unwrap_err().kind, EffectErrorKind::Host(error) if matches!(error.kind, ErrorKind::ForeignState))
    );
    assert!(model.evaluate(&state, &enabled.is(false)).unwrap());
    assert!(
        model
            .evaluate(
                &registry.apply_at(&session, 0, &state).unwrap().into_state(),
                &enabled.is(true)
            )
            .unwrap()
    );
}
