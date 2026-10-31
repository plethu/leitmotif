use leitmotif::{
    Domain, Error, Fact, FactHandle, FactKind, Model, ModelBuilder, Snapshot, State, Transition,
    Value,
};

mod reads;
mod restore;

struct Learned;
impl Fact for Learned {
    type Value = bool;
    const KEY: &'static str = "learned";
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Weather {
    Calm,
    Storm,
}

impl Domain for Weather {
    fn values() -> Vec<Self> {
        vec![Self::Calm, Self::Storm]
    }

    fn encode(self) -> Value {
        symbol(match self {
            Self::Calm => "calm",
            Self::Storm => "storm",
        })
    }
}

struct Sky;
impl Fact for Sky {
    type Value = Weather;
    const KEY: &'static str = "sky";
}

struct Hardcore;
impl Fact for Hardcore {
    type Value = bool;
    const KEY: &'static str = "hardcore";
    const KIND: FactKind = FactKind::Setting;
}

struct Harbour {
    model: Model,
    learned: FactHandle<Learned>,
    sky: FactHandle<Sky>,
    hardcore: FactHandle<Hardcore>,
}

fn harbour() -> Result<Harbour, Error> {
    let mut builder = ModelBuilder::new("harbour")?;
    let learned = builder.fact::<Learned>()?;
    let sky = builder.fact::<Sky>()?;
    let hardcore = builder.fact::<Hardcore>()?;
    let model = builder
        .with_transition(
            Transition::new("storm.break")
                .with_guard(sky.is(Weather::Storm))
                .with_effect(sky.set(Weather::Calm))
                .with_effect(learned.set(true)),
        )
        .build()?;
    Ok(Harbour {
        model,
        learned,
        sky,
        hardcore,
    })
}

impl Harbour {
    fn stormy_state(&self) -> Result<State, Error> {
        self.model.state([
            self.learned.set(false),
            self.sky.set(Weather::Storm),
            self.hardcore.set(true),
        ])
    }

    fn stormy_snapshot(&self) -> Result<Snapshot, Error> {
        self.model.snapshot(&self.stormy_state()?)
    }
}

fn symbol(name: &str) -> Value {
    Value::Symbol(name.to_owned())
}
