use leitmotif::{ModelBuilder, Transition};
use leitmotif_recite::{Bindings, DeferredEffects};
use recite_runtime::{DialogueEvent, next, start_scene};

use super::support::{Enabled, compile};

#[test]
fn deferred_requests_apply_only_after_end_from_the_authoritative_queue() {
    let asset = compile(":: start default\n! deferred grant(player)\n-> END\n").unwrap();
    let mut builder = ModelBuilder::new("host").unwrap();
    let enabled = builder.fact::<Enabled>().unwrap();
    let model = builder
        .with_transition(Transition::new("grant").with_effect(enabled.set(true)))
        .build()
        .unwrap();
    let initial = model.state([enabled.set(false)]).unwrap();
    let mut registry = DeferredEffects::new(&model);
    registry
        .bind(asset.effects.first().unwrap(), "grant")
        .unwrap();
    let conditions = Bindings::new();
    let mut session = start_scene(&asset, None).unwrap();
    assert!(registry.apply_at(&session, 0, &initial).is_err());
    let DialogueEvent::End { deferred_effects } =
        next(&asset, &mut session, &conditions.context(&model, &initial)).unwrap()
    else {
        panic!("expected End delivery")
    };
    assert_eq!(session.deferred_effects(), deferred_effects);
    assert!(model.evaluate(&initial, &enabled.is(false)).unwrap());
    let applied = registry.apply_at(&session, 0, &initial).unwrap();
    assert_eq!(applied.request(), deferred_effects.first().unwrap());
    assert!(
        model
            .evaluate(&applied.into_state(), &enabled.is(true))
            .unwrap()
    );
}

mod failures;
mod mixed;
mod ordering;
