use std::iter;

use leitmotif::{
    Domain, Fact, ModelBuilder, PermittedEnding, ProgressionConclusion, ProgressionQuery,
    SearchLimits, Transition, Value,
};

#[derive(Clone)]
struct RingPlace(usize);
impl Domain for RingPlace {
    fn values() -> Vec<Self> {
        (0..66).map(Self).collect()
    }
    fn encode(self) -> Value {
        Value::Symbol(self.0.to_string())
    }
}
struct RingLocation;
impl Fact for RingLocation {
    type Value = RingPlace;
    const KEY: &'static str = "ring.location";
}

#[test]
fn fixed_ring_model_preserves_cycle_escape_and_replays_the_trap_entry() {
    let mut builder = ModelBuilder::new("progression-ring").unwrap();
    let location = builder.fact::<RingLocation>().unwrap();
    for (from, to) in (0..64).zip((1..64).chain(iter::once(0))) {
        builder = builder.with_transition(
            Transition::new(format!("ring.{from}"))
                .with_guard(location.is(RingPlace(from)))
                .with_effect(location.set(RingPlace(to))),
        );
    }
    let model = builder
        .with_transition(
            Transition::new("finish")
                .with_guard(location.is(RingPlace(0)))
                .with_effect(location.set(RingPlace(64))),
        )
        .with_transition(
            Transition::new("trap")
                .with_guard(location.is(RingPlace(0)))
                .with_effect(location.set(RingPlace(65))),
        )
        .with_transition(
            Transition::new("closed")
                .with_guard(location.is(RingPlace(65)))
                .with_effect(location.set(RingPlace(65))),
        )
        .build()
        .unwrap();
    let initial = model.state([location.set(RingPlace(0))]).unwrap();
    let ending = location.is(RingPlace(64));
    let query =
        ProgressionQuery::new(ending.clone(), [PermittedEnding::new("finish", ending)]).unwrap();
    let report = model
        .progression(
            &initial,
            &query,
            SearchLimits {
                max_states: 66,
                max_depth: 63,
            },
        )
        .unwrap();
    assert!(report.coverage.exhausted);
    assert_eq!(report.coverage.discovered_states, 66);
    assert_eq!(report.coverage.evaluated_states, 66);
    let ProgressionConclusion::Trap {
        entry,
        trapped_states,
    } = report.conclusion
    else {
        panic!("only the closed branch is trapped")
    };
    assert_eq!(trapped_states, 1);
    assert_eq!(entry.steps.len(), 1);
    let step = entry.steps.first().unwrap();
    assert_eq!(step.transition.id, "trap");
    let replayed = model.apply(&initial, &step.transition.id).unwrap().unwrap();
    assert_eq!(model.snapshot(&replayed).unwrap(), step.state);
    assert!(
        model
            .evaluate(&replayed, &location.is(RingPlace(65)))
            .unwrap()
    );
}
