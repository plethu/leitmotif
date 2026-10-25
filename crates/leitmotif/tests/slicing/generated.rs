use super::{compare, facts};
use leitmotif::{Predicate, Transition};

#[test]
fn generated_boolean_models_match_full_verdicts_distances_and_concrete_replay() {
    // Each bit controls an independent semantic choice, not a random seed.
    // Enumerate initial assignments too: projection representatives need not be defaults.
    for configuration in 0_u16..128 {
        let (mut builder, goal, gate, permit, unrelated) = facts().unwrap();
        let guard = match configuration & 3 {
            0 => Predicate::constant(true),
            1 => gate.is(true),
            2 => gate.is(true).or(permit.is(false)),
            _ => gate.is(true).and(permit.is(true)).negate(),
        };
        builder = builder
            .with_transition(Transition::new("noise").with_effect(unrelated.set(true)))
            .with_transition(
                Transition::new("write")
                    .with_guard(guard)
                    .with_effect(goal.set(true)),
            )
            .with_transition(
                Transition::new("gate")
                    .with_guard(permit.is(configuration & 4 != 0))
                    .with_effect(gate.set(true)),
            )
            .with_transition(
                Transition::new("permit")
                    .with_guard(gate.is(configuration & 8 != 0))
                    .with_effect(permit.set(false)),
            );
        if configuration & 16 != 0 {
            builder = builder.with_transition(
                Transition::new("reset")
                    .with_effect(goal.set(false))
                    .with_effect(permit.set(true)),
            );
        }
        if configuration & 32 != 0 {
            builder = builder.with_transition(
                Transition::new("alternative")
                    .with_guard(unrelated.is(true))
                    .with_effect(goal.set(true)),
            );
        }
        if configuration & 64 != 0 {
            builder = builder.with_transition(Transition::new("empty").with_guard(goal.is(true)));
        }
        let model = builder.build().unwrap();
        for assignment in 0_u8..16 {
            let initial = model
                .state([
                    goal.set(assignment & 1 != 0),
                    gate.set(assignment & 2 != 0),
                    permit.set(assignment & 4 != 0),
                    unrelated.set(assignment & 8 != 0),
                ])
                .unwrap();
            for target in [
                goal.is(true),
                goal.is(false).and(gate.is(true)),
                goal.is(true).or(permit.is(false)).negate(),
            ] {
                compare(&model, &initial, &target).unwrap();
            }
        }
    }
}
