use std::error::Error;

use leitmotif::{ModelBuilder, ReachabilityConclusion, SearchLimits, Transition};
use leitmotif_recite::{Bindings, EffectErrorKind, ImmediateEffects};
use recite_core::compiled::CompiledConditionCall;
use recite_runtime::{
    DialogueEvent, LocaleResolution, next, snapshot::snapshot_session, start_scene,
};

use super::support::{Enabled, Other, compile};

pub(super) const STORY: &str = concat!(
    ":: start default\n",
    "! immediate grant(player, \"gift\", 3, 0.5, true)\n",
    ":if ready()\n",
    "  > yes@8843fd6f53f020a12b31\n    Changed.\n",
    ":else\n  > no@4dff358182d02d9090e3\n    Unchanged.\n",
    "-> END\n",
);

#[test]
fn immediate_emission_application_and_next_traversal_are_separate_boundaries() {
    let asset = compile(STORY).unwrap();
    let effect = asset.effects.first().unwrap();
    let mut builder = ModelBuilder::new("host").unwrap();
    let enabled = builder.fact::<Enabled>().unwrap();
    let other = builder.fact::<Other>().unwrap();
    let model = builder
        .with_transition(
            Transition::new("grant")
                .with_guard(enabled.is(false).and(other.is(true)))
                .with_effect(enabled.set(true))
                .with_effect(other.set(false)),
        )
        .build()
        .unwrap();
    let initial = model.state([enabled.set(false), other.set(true)]).unwrap();
    let mut host = initial.clone();
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
    let mut effects = ImmediateEffects::new(&model);
    effects.bind(effect, "grant").unwrap();
    let mut session = start_scene(&asset, None).unwrap();
    let position_before = snapshot_session(&session);
    let host_before = model.snapshot(&host).unwrap();
    let DialogueEvent::Effect(request) =
        next(&asset, &mut session, &conditions.context(&model, &host)).unwrap()
    else {
        panic!("expected immediate request");
    };
    let position_emitted = snapshot_session(&session);
    assert_ne!(
        position_before.next_statement,
        position_emitted.next_statement
    );
    assert_eq!(model.snapshot(&host).unwrap(), host_before);
    assert!(session.pending_effect().is_none());
    assert!(request.id.as_str().starts_with(effect.id.as_str()));
    assert_ne!(request.id, effect.id);
    let source = &asset
        .source_maps
        .get(usize::try_from(effect.source_map.as_u32()).unwrap())
        .unwrap()
        .span;
    assert_eq!(&request.source_span, source);

    let applied = effects.apply(&request, &host).unwrap();
    assert_eq!(applied.request(), &request);
    assert_eq!(applied.transition().id, "grant");
    assert_eq!(snapshot_session(&session), position_emitted);
    host = applied.into_state();
    assert!(
        model
            .evaluate(&host, &enabled.is(true).and(other.is(false)))
            .unwrap()
    );
    let DialogueEvent::Line(line) =
        next(&asset, &mut session, &conditions.context(&model, &host)).unwrap()
    else {
        panic!("expected condition after host application");
    };
    assert_eq!(line.text, "Changed.");

    let report = model
        .reachability(
            &initial,
            &enabled.is(true).and(other.is(false)),
            SearchLimits {
                max_states: 4,
                max_depth: 2,
            },
        )
        .unwrap();
    let ReachabilityConclusion::Reached { witness } = report.conclusion else {
        panic!("host transition must have a witness");
    };
    assert_eq!(witness.steps.len(), 1);
    let step = witness.steps.first().unwrap();
    assert_eq!(step.transition.id, "grant");
    let replayed = effects.apply(&request, &initial).unwrap().into_state();
    assert_eq!(model.snapshot(&replayed).unwrap(), step.state);
    assert_eq!(
        model.snapshot(&replayed).unwrap(),
        model.snapshot(&host).unwrap()
    );
}

#[test]
fn recite_rollback_restores_position_but_cannot_undo_a_committed_host_write() {
    let asset = compile(STORY).unwrap();
    let effect = asset.effects.first().unwrap();
    let mut builder = ModelBuilder::new("host").unwrap();
    let enabled = builder.fact::<Enabled>().unwrap();
    let model = builder
        .with_transition(
            Transition::new("grant")
                .with_guard(enabled.is(false))
                .with_effect(enabled.set(true)),
        )
        .build()
        .unwrap();
    let mut host = model.state([enabled.set(false)]).unwrap();
    let mut effects = ImmediateEffects::new(&model);
    effects.bind(effect, "grant").unwrap();
    let conditions = Bindings::new();
    let mut session = start_scene(&asset, None).unwrap();
    let before = snapshot_session(&session);
    let mut emitted = None;
    let aborted = session.try_traverse(|trial| -> Result<(), Box<dyn Error>> {
        let event = trial.next_with(
            &asset,
            &conditions.context(&model, &host),
            LocaleResolution::new(),
        )?;
        let DialogueEvent::Effect(request) = event else {
            return Err("expected request".into());
        };
        host = effects.apply(&request, &host)?.into_state();
        emitted = Some(request);
        Err("a later operation failed".into())
    });
    assert!(aborted.is_err());
    assert_eq!(snapshot_session(&session), before);
    assert!(model.evaluate(&host, &enabled.is(true)).unwrap());
    let DialogueEvent::Effect(repeated) =
        next(&asset, &mut session, &conditions.context(&model, &host)).unwrap()
    else {
        panic!("rolled-back position must emit the request again");
    };
    assert_eq!(Some(&repeated), emitted.as_ref());
    assert!(matches!(
        effects.apply(&repeated, &host).unwrap_err().kind,
        EffectErrorKind::DisabledTransition(_)
    ));
}

#[test]
fn distinct_typed_signatures_dispatch_to_their_own_transitions() {
    let asset = compile(concat!(
        ":: start default\n",
        "! immediate set(player)\n",
        "! immediate set(\"player\")\n",
        "! immediate finish()\n",
        "-> END\n",
    ))
    .unwrap();
    let mut builder = ModelBuilder::new("typed-effect-dispatch").unwrap();
    let enabled = builder.fact::<Enabled>().unwrap();
    let other = builder.fact::<Other>().unwrap();
    let model = builder
        .with_transition(Transition::new("grant").with_effect(enabled.set(true)))
        .with_transition(Transition::new("revoke").with_effect(enabled.set(false)))
        .with_transition(Transition::new("finish").with_effect(other.set(true)))
        .build()
        .unwrap();
    let mut host = model.state([enabled.set(false), other.set(false)]).unwrap();
    let mut registry = ImmediateEffects::new(&model);
    for (effect, transition) in asset.effects.iter().zip(["grant", "revoke", "finish"]) {
        registry.bind(effect, transition).unwrap();
    }
    let quoted = asset.effects.get(1).unwrap();
    assert!(
        matches!(registry.bind(quoted, "grant").unwrap_err().kind, EffectErrorKind::Duplicate(function) if function == "set")
    );
    let mut session = start_scene(&asset, None).unwrap();
    let conditions = Bindings::new();
    for (transition, expected_enabled, expected_other) in [
        ("grant", true, false),
        ("revoke", false, false),
        ("finish", false, true),
    ] {
        let DialogueEvent::Effect(request) =
            next(&asset, &mut session, &conditions.context(&model, &host)).unwrap()
        else {
            panic!("expected the next declared immediate request");
        };
        let applied = registry.apply(&request, &host).unwrap();
        assert_eq!(applied.transition().id, transition);
        assert_eq!(applied.request(), &request);
        host = applied.into_state();
        let expected = model
            .state([enabled.set(expected_enabled), other.set(expected_other)])
            .unwrap();
        assert_eq!(
            model.snapshot(&host).unwrap(),
            model.snapshot(&expected).unwrap()
        );
    }
    assert!(matches!(
        next(&asset, &mut session, &conditions.context(&model, &host)).unwrap(),
        DialogueEvent::End { .. }
    ));
}
