//! Progression traps are relative to terminal stopping boundaries and permitted endings.

use leitmotif::{
    Domain, Error, Fact, FactHandle, Model, ModelBuilder, PermittedEnding, Predicate,
    ProgressionQuery, SearchLimits, State, Transition, Value,
};

mod bounds;
mod reports;
mod routes;
mod scale;
mod validation;

#[derive(Clone, Copy)]
enum Place {
    Start,
    Loop,
    Exit,
    Trap,
    Failure,
    Undeclared,
}
impl Domain for Place {
    fn values() -> Vec<Self> {
        vec![
            Self::Start,
            Self::Loop,
            Self::Exit,
            Self::Trap,
            Self::Failure,
        ]
    }
    fn encode(self) -> Value {
        Value::Symbol(
            match self {
                Self::Start => "start",
                Self::Loop => "loop",
                Self::Exit => "exit",
                Self::Trap => "trap",
                Self::Failure => "failure",
                Self::Undeclared => "undeclared",
            }
            .into(),
        )
    }
}
struct Location;
impl Fact for Location {
    type Value = Place;
    const KEY: &'static str = "location";
}

const LIMITS: SearchLimits = SearchLimits {
    max_states: 16,
    max_depth: 16,
};

struct Fixture {
    model: Model,
    initial: State,
    location: FactHandle<Location>,
}
impl Fixture {
    fn new(initial: Place, edges: &[(&str, Place, Place)]) -> Result<Self, Error> {
        let mut builder = ModelBuilder::new("progression")?;
        let location = builder.fact::<Location>()?;
        for (id, from, to) in edges {
            builder = builder.with_transition(
                Transition::new(*id)
                    .with_guard(location.is(*from))
                    .with_effect(location.set(*to)),
            );
        }
        let model = builder.build()?;
        Ok(Self {
            initial: model.state([location.set(initial)])?,
            model,
            location,
        })
    }

    fn query(
        &self,
        terminal: &[Place],
        endings: &[(&str, Place)],
    ) -> Result<ProgressionQuery, Error> {
        let terminal = terminal
            .iter()
            .fold(Predicate::constant(false), |predicate, place| {
                predicate.or(self.location.is(*place))
            });
        ProgressionQuery::new(
            terminal,
            endings
                .iter()
                .map(|(name, place)| PermittedEnding::new(*name, self.location.is(*place))),
        )
    }
}
