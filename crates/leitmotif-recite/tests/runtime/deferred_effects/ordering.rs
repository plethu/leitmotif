use leitmotif::{ModelBuilder, Transition};
use leitmotif_recite::{Bindings, DeferredEffects};
use recite_core::ChoiceId;
use recite_runtime::{DialogueEvent, choose, next, start_scene};

use super::super::support::{Enabled, compile};

#[test]
fn repeated_authored_requests_are_distinct_occurrences_in_host_commit_order() {
    let asset = compile(concat!(
        ":: start default\n-> loop\n",
        ":: loop\n! deferred set()\n",
        "? repeat@11111111111111111111\n  Repeat.\n  -> again\n",
        "? finish@22222222222222222222\n  Finish.\n  -> END\n",
        ":: again\n! deferred reset()\n-> loop\n",
    ))
    .unwrap();
    let mut builder = ModelBuilder::new("repeated-deferred").unwrap();
    let enabled = builder.fact::<Enabled>().unwrap();
    let model = builder
        .with_transition(Transition::new("set").with_effect(enabled.set(true)))
        .with_transition(Transition::new("reset").with_effect(enabled.set(false)))
        .build()
        .unwrap();
    let mut state = model.state([enabled.set(false)]).unwrap();
    let mut registry = DeferredEffects::new(&model);
    for effect in &asset.effects {
        registry.bind(effect, &effect.function).unwrap();
    }
    let conditions = Bindings::new();
    let mut session = start_scene(&asset, None).unwrap();
    assert!(matches!(
        next(&asset, &mut session, &conditions.context(&model, &state)).unwrap(),
        DialogueEvent::Prompt { .. }
    ));
    assert!(matches!(
        choose(
            &asset,
            &mut session,
            ChoiceId::new("11111111111111111111").unwrap(),
            &conditions.context(&model, &state)
        )
        .unwrap(),
        DialogueEvent::Prompt { .. }
    ));
    let DialogueEvent::End {
        deferred_effects: delivered,
    } = choose(
        &asset,
        &mut session,
        ChoiceId::new("22222222222222222222").unwrap(),
        &conditions.context(&model, &state),
    )
    .unwrap()
    else {
        panic!("expected End")
    };
    assert_eq!(
        delivered
            .iter()
            .map(|request| request.function.as_str())
            .collect::<Vec<_>>(),
        ["set", "reset", "set"]
    );
    assert_eq!(delivered.first().unwrap(), delivered.last().unwrap());
    assert_eq!(delivered, session.deferred_effects());
    assert!(model.evaluate(&state, &enabled.is(false)).unwrap());
    let host_transaction = "scene-transaction-1";
    let mut committed = Vec::new();
    let mut observed = Vec::new();
    for (position, request) in delivered.iter().enumerate() {
        let applied = registry.apply_at(&session, position, &state).unwrap();
        assert_eq!(applied.request(), request);
        state = applied.into_state();
        committed.push((host_transaction, position, request.id.clone()));
        observed.push(model.evaluate(&state, &enabled.is(true)).unwrap());
    }
    assert_eq!(observed, [true, false, true]);
    let first = committed.first().unwrap();
    let last = committed.last().unwrap();
    assert_eq!(first.0, last.0);
    assert_eq!(first.2, last.2);
    assert_ne!(first.1, last.1);
    assert!(model.evaluate(&state, &enabled.is(true)).unwrap());
}
