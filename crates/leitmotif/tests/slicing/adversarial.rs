use super::{LIMITS, compare, facts, replay};
use leitmotif::{Invariant, ReachabilityConclusion, Transition, Value};

#[test]
fn transitive_guards_and_every_writer_are_retained_in_declaration_order() {
    let (builder, goal, gate, permit, unrelated) = facts().unwrap();
    let model = builder
        .with_transition(Transition::new("noise-before").with_effect(unrelated.set(true)))
        .with_transition(
            Transition::new("blocked")
                .with_guard(gate.is(true))
                .with_effect(goal.set(true)),
        )
        .with_transition(
            Transition::new("unlock")
                .with_guard(permit.is(true))
                .with_effect(gate.set(true)),
        )
        .with_transition(
            Transition::new("noise-between")
                .with_guard(goal.is(false))
                .with_effect(unrelated.set(false)),
        )
        .with_transition(Transition::new("permit").with_effect(permit.set(true)))
        .with_transition(
            Transition::new("direct")
                .with_effect(goal.set(true))
                .with_effect(unrelated.set(true)),
        )
        .with_transition(Transition::new("noise-after").with_effect(unrelated.set(false)))
        .with_invariant(Invariant::new("not-a-premise", goal.is(false)))
        .build()
        .unwrap();
    let initial = model
        .state([
            goal.set(false),
            gate.set(false),
            permit.set(false),
            unrelated.set(false),
        ])
        .unwrap();
    let target = goal.is(true);
    compare(&model, &initial, &target).unwrap();
    let report = model
        .sliced_reachability(&initial, &target, LIMITS)
        .unwrap();
    assert_eq!(report.retained_facts, ["goal", "gate", "permit"]);
    assert_eq!(
        report.retained_transitions,
        ["blocked", "unlock", "permit", "direct"]
    );
    let ReachabilityConclusion::Reached { witness } = &report.conclusion else {
        panic!("direct writer must reach goal")
    };
    assert_eq!(witness.steps.len(), 1);
    assert_eq!(witness.steps.first().unwrap().transition.id, "direct");
    assert_eq!(
        witness.steps.first().unwrap().state.values.get("unrelated"),
        Some(&Value::Bool(true))
    );
    replay(&model, &initial, &target, witness).unwrap();
    assert_eq!(
        serde_json::to_string(&report).unwrap(),
        serde_json::to_string(
            &model
                .sliced_reachability(&initial, &target, LIMITS)
                .unwrap()
        )
        .unwrap()
    );
    let serialized = serde_json::to_value(&report).unwrap();
    assert!(serialized["coverage"].get("discovered_states").is_none());
    assert!(
        serialized["coverage"]
            .get("discovered_projections")
            .is_some()
    );
}

#[test]
fn coeffects_join_the_closure_when_a_later_guard_reads_them() {
    let (builder, goal, gate, permit, unrelated) = facts().unwrap();
    let model = builder
        .with_transition(
            Transition::new("prepare")
                .with_effect(gate.set(true))
                .with_effect(permit.set(false)),
        )
        .with_transition(
            Transition::new("restore")
                .with_guard(unrelated.is(true))
                .with_effect(permit.set(true)),
        )
        .with_transition(Transition::new("signal").with_effect(unrelated.set(true)))
        .with_transition(
            Transition::new("finish")
                .with_guard(gate.is(true).and(permit.is(true)))
                .with_effect(goal.set(true)),
        )
        .build()
        .unwrap();
    let initial = model
        .state([
            goal.set(false),
            gate.set(false),
            permit.set(true),
            unrelated.set(false),
        ])
        .unwrap();
    compare(&model, &initial, &goal.is(true)).unwrap();
    let report = model
        .sliced_reachability(&initial, &goal.is(true), LIMITS)
        .unwrap();
    assert_eq!(
        report.retained_facts,
        ["goal", "gate", "permit", "unrelated"]
    );
    let ReachabilityConclusion::Reached { witness } = report.conclusion else {
        panic!("restored permit must enable ending")
    };
    assert_eq!(witness.steps.len(), 4);
}

#[test]
fn shared_consumable_prevents_both_endings_and_reusable_permit_restores_them() {
    for reusable in [false, true] {
        let (builder, goal, gate, permit, unrelated) = facts().unwrap();
        let mut builder = builder
            .with_transition(
                Transition::new("first")
                    .with_guard(permit.is(true))
                    .with_effect(goal.set(true))
                    .with_effect(permit.set(false)),
            )
            .with_transition(
                Transition::new("second")
                    .with_guard(permit.is(true))
                    .with_effect(gate.set(true))
                    .with_effect(permit.set(false)),
            )
            .with_transition(Transition::new("noise").with_effect(unrelated.set(true)));
        if reusable {
            builder =
                builder.with_transition(Transition::new("release").with_effect(permit.set(true)));
        }
        let model = builder.build().unwrap();
        let initial = model
            .state([
                goal.set(false),
                gate.set(false),
                permit.set(true),
                unrelated.set(true),
            ])
            .unwrap();
        let target = goal.is(true).and(gate.is(true));
        compare(&model, &initial, &target).unwrap();
        let report = model
            .sliced_reachability(&initial, &target, LIMITS)
            .unwrap();
        assert_eq!(report.retained_facts, ["goal", "gate", "permit"]);
        assert_eq!(
            matches!(report.conclusion, ReachabilityConclusion::Reached { .. }),
            reusable
        );
    }
}

#[test]
fn explicit_monitor_fact_encodes_history_without_implicit_event_semantics() {
    let (builder, goal, learned, disclosed_after_learning, unrelated) = facts().unwrap();
    let model = builder
        .with_transition(Transition::new("learn").with_effect(learned.set(true)))
        .with_transition(Transition::new("disclose").with_effect(goal.set(true)))
        .with_transition(
            Transition::new("record_order")
                .with_guard(learned.is(true).and(goal.is(false)))
                .with_effect(goal.set(true))
                .with_effect(disclosed_after_learning.set(true)),
        )
        .with_transition(Transition::new("noise").with_effect(unrelated.set(true)))
        .build()
        .unwrap();
    let initial = model
        .state([
            goal.set(false),
            learned.set(false),
            disclosed_after_learning.set(false),
            unrelated.set(false),
        ])
        .unwrap();
    compare(&model, &initial, &goal.is(true)).unwrap();
    compare(&model, &initial, &disclosed_after_learning.is(true)).unwrap();
    let report = model
        .sliced_reachability(&initial, &disclosed_after_learning.is(true), LIMITS)
        .unwrap();
    let ReachabilityConclusion::Reached { witness } = report.conclusion else {
        panic!("explicit order should be reachable")
    };
    assert_eq!(
        witness
            .steps
            .iter()
            .map(|step| step.transition.id.as_str())
            .collect::<Vec<_>>(),
        ["learn", "record_order"]
    );
}
