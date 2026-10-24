use super::{Bit, compare};
use leitmotif::{Domain, Fact, ModelBuilder, Predicate, Transition, Value};

#[derive(Clone, Copy)]
enum Phase {
    Idle,
    Ready,
    Done,
}
impl Domain for Phase {
    fn values() -> Vec<Self> {
        vec![Self::Idle, Self::Ready, Self::Done]
    }
    fn encode(self) -> Value {
        Value::Symbol(
            match self {
                Self::Idle => "idle",
                Self::Ready => "ready",
                Self::Done => "done",
            }
            .into(),
        )
    }
}
struct Local;
impl Fact for Local {
    type Value = Phase;
    const KEY: &'static str = "local";
}

#[test]
fn generated_three_state_graphs_preserve_cycles_alternatives_and_unreachable_endings() {
    for edges in 0_u8..64 {
        let mut builder = ModelBuilder::new("finite_domains").unwrap();
        let local = builder.fact::<Local>().unwrap();
        let gate = builder.fact::<Bit<1>>().unwrap();
        let noise = builder.fact::<Bit<3>>().unwrap();
        for (bit, (from, to)) in [
            (Phase::Idle, Phase::Ready),
            (Phase::Ready, Phase::Done),
            (Phase::Done, Phase::Idle),
            (Phase::Idle, Phase::Done),
            (Phase::Ready, Phase::Idle),
            (Phase::Done, Phase::Ready),
        ]
        .into_iter()
        .enumerate()
        {
            if edges & (1 << bit) == 0 {
                continue;
            }
            let guard = if edges & 4 == 0 {
                Predicate::constant(true)
            } else {
                gate.is(true)
            };
            builder = builder.with_transition(
                Transition::new(format!("edge.{bit}"))
                    .with_guard(local.is(from).and(guard))
                    .with_effect(local.set(to))
                    .with_effect(noise.set(true)),
            );
        }
        let model = builder
            .with_transition(Transition::new("grant").with_effect(gate.set(true)))
            .with_transition(Transition::new("revoke").with_effect(gate.set(false)))
            .with_transition(
                Transition::new("noise")
                    .with_guard(local.is(Phase::Ready))
                    .with_effect(noise.set(false)),
            )
            .build()
            .unwrap();
        for (phase, enabled) in Phase::values()
            .into_iter()
            .flat_map(|phase| [false, true].map(|enabled| (phase, enabled)))
        {
            let initial = model
                .state([local.set(phase), gate.set(enabled), noise.set(false)])
                .unwrap();
            for target in Phase::values() {
                compare(&model, &initial, &local.is(target)).unwrap();
            }
        }
    }
}
