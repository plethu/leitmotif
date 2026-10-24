use super::claims::{Attitude, Belief, ClaimId, Context, EvidenceId, Exposure, Person};
use leitmotif::{Domain, Fact, FactHandle, FactKind, Model, ModelBuilder, State, Value};

impl Domain for Person {
    fn values() -> Vec<Self> {
        vec![Self::Mira, Self::Orin]
    }
    fn encode(self) -> Value {
        Value::Symbol(
            match self {
                Self::Mira => "mira",
                Self::Orin => "orin",
            }
            .to_owned(),
        )
    }
}
struct Culprit;
impl Fact for Culprit {
    type Value = Person;
    const KEY: &'static str = "tide_plate.culprit";
    const KIND: FactKind = FactKind::Setting;
}
struct LockedAtDusk;
impl Fact for LockedAtDusk {
    type Value = bool;
    const KEY: &'static str = "cabinet.dusk.locked";
}
struct LockedAtDawn;
impl Fact for LockedAtDawn {
    type Value = bool;
    const KEY: &'static str = "cabinet.dawn.locked";
}

pub(super) struct Story {
    pub(super) model: Model,
    pub(super) truth: State,
    pub(super) beliefs: [Belief; 2],
    pub(super) exposures: Vec<Exposure>,
    culprit: FactHandle<Culprit>,
    dusk: FactHandle<LockedAtDusk>,
    dawn: FactHandle<LockedAtDawn>,
}
impl Story {
    pub(super) fn new() -> Result<Self, leitmotif::Error> {
        let mut builder = ModelBuilder::new("missing_tide_plate")?;
        let culprit = builder.fact::<Culprit>()?;
        let dusk = builder.fact::<LockedAtDusk>()?;
        let dawn = builder.fact::<LockedAtDawn>()?;
        let model = builder.build()?;
        let truth = model.state([culprit.set(Person::Mira), dusk.set(true), dawn.set(false)])?;
        Ok(Self {
            model,
            truth,
            culprit,
            dusk,
            dawn,
            beliefs: [
                Belief {
                    holder: Person::Orin,
                    claim: ClaimId::MiraAccusesOrin,
                    attitude: Attitude::Accepts,
                },
                Belief {
                    holder: Person::Mira,
                    claim: ClaimId::DuskOpen,
                    attitude: Attitude::Rejects,
                },
            ],
            exposures: vec![
                Exposure::new(
                    Person::Orin,
                    EvidenceId::Testimony(ClaimId::MiraAccusesOrin),
                ),
                Exposure::new(Person::Orin, EvidenceId::Testimony(ClaimId::OrinSawCoat)),
            ],
        })
    }
    pub(super) fn culprit(&self, person: Person) -> leitmotif::Predicate {
        self.culprit.is(person)
    }
    pub(super) fn locked(&self, context: Context, value: bool) -> leitmotif::Predicate {
        match context {
            Context::Dusk => self.dusk.is(value),
            Context::Dawn => self.dawn.is(value),
        }
    }
}
