//! A harbour facing a storm: the last oil can light a beacon or fuel a ferry.

mod ferryman;
mod keeper;
#[cfg(test)]
mod tests;

use leitmotif::{
    Domain, Error, Fact, FactHandle, FactKind, Invariant, Model, ModelBuilder, Predicate, State,
    Transition, Value,
};

#[derive(Clone, Copy)]
pub(super) enum Ending {
    Open,
    Sheltered,
    Departed,
}
impl Domain for Ending {
    fn values() -> Vec<Self> {
        vec![Self::Open, Self::Sheltered, Self::Departed]
    }
    fn encode(self) -> Value {
        Value::Symbol(
            match self {
                Self::Open => "open",
                Self::Sheltered => "sheltered",
                Self::Departed => "departed",
            }
            .to_owned(),
        )
    }
}
pub(super) struct TownEnding;
impl Fact for TownEnding {
    type Value = Ending;
    const KEY: &'static str = "harbour.ending";
}
pub(super) struct Oil;
impl Fact for Oil {
    type Value = bool;
    const KEY: &'static str = "harbour.oil_available";
}
struct StormPassed;
impl Fact for StormPassed {
    type Value = bool;
    const KEY: &'static str = "harbour.storm_passed";
    const KIND: FactKind = FactKind::Setting;
}

// Shared inputs are declared once, then explicitly lent to each story.
struct Shared {
    active: Predicate,
    oil: FactHandle<Oil>,
}

/// Complete typed input from a host projection; no missing value is defaulted.
pub(super) struct Projection {
    pub(super) harbour: HarbourProjection,
    pub(super) keeper: KeeperProjection,
    pub(super) ferry: bool,
}
pub(super) struct HarbourProjection {
    pub(super) ending: Ending,
    pub(super) oil: bool,
    pub(super) storm: bool,
}
pub(super) struct KeeperProjection {
    pub(super) warning: bool,
    pub(super) beacon: bool,
}

/// Typed handles lent to a consumer while this owner constructs its model.
pub(super) struct Queries {
    pub(super) ending: FactHandle<TownEnding>,
    pub(super) oil: FactHandle<Oil>,
    pub(super) beacon: FactHandle<keeper::Beacon>,
    pub(super) ferry: FactHandle<ferryman::FerryReady>,
}

/// Separately authored harbour stories and their ending queries.
pub(super) struct Town {
    pub(super) model: Model,
    pub(super) initial: State,
    pub(super) sheltered: Predicate,
    pub(super) departed: Predicate,
    pub(super) unreachable: Predicate,
}

/// Compose the stories with explicit shared inputs.
///
/// # Errors
/// Returns invalid fact, event or invariant declarations.
pub(super) fn town() -> Result<Town, Error> {
    project(&Projection::default())
}

impl Default for Projection {
    fn default() -> Self {
        Self {
            harbour: HarbourProjection {
                ending: Ending::Open,
                oil: true,
                storm: false,
            },
            keeper: KeeperProjection {
                warning: false,
                beacon: false,
            },
            ferry: false,
        }
    }
}

/// Builds the same authored model around a complete typed host assignment.
pub(super) fn project(projection: &Projection) -> Result<Town, Error> {
    project_with(projection, |_| ()).map(|(town, ())| town)
}

/// Lends the existing typed declarations without introducing a second model.
pub(super) fn project_with<Q>(
    projection: &Projection,
    bind: impl FnOnce(&Queries) -> Q,
) -> Result<(Town, Q), Error> {
    let mut builder = ModelBuilder::new("storm_harbour")?;
    let ending = builder.fact::<TownEnding>()?;
    let oil = builder.fact::<Oil>()?;
    let storm = builder.fact::<StormPassed>()?;
    let shared = Shared {
        active: ending.is(Ending::Open),
        oil,
    };
    let keeper = keeper::declare(&mut builder)?;
    let ferryman = ferryman::declare(&mut builder)?;
    let builder = keeper.author(builder, &shared);
    let builder = ferryman.author(builder, &shared, &keeper.warning);
    let model = builder
        .with_transition(
            Transition::new("harbour.shelter")
                .with_guard(shared.active.clone().and(keeper.beacon.is(true)))
                .with_effect(ending.set(Ending::Sheltered)),
        )
        .with_transition(
            Transition::new("harbour.depart")
                .with_guard(shared.active.and(ferryman.ready.is(true)))
                .with_effect(ending.set(Ending::Departed)),
        )
        .with_invariant(Invariant::new(
            "harbour.oil_spent_once",
            keeper.beacon.is(true).and(ferryman.ready.is(true)).negate(),
        ))
        .with_invariant(Invariant::new(
            "harbour.beacon_requires_warning",
            keeper.beacon.is(true).implies(keeper.warning.is(true)),
        ))
        .build()?;
    let queries = Queries {
        ending,
        oil: shared.oil,
        beacon: keeper.beacon,
        ferry: ferryman.ready,
    };
    let initial = model.state([
        queries.ending.set(projection.harbour.ending),
        queries.oil.set(projection.harbour.oil),
        storm.set(projection.harbour.storm),
        keeper.warning.set(projection.keeper.warning),
        queries.beacon.set(projection.keeper.beacon),
        queries.ferry.set(projection.ferry),
    ])?;
    let bound = bind(&queries);
    Ok((
        Town {
            model,
            initial,
            sheltered: queries.ending.is(Ending::Sheltered),
            departed: queries.ending.is(Ending::Departed),
            unreachable: storm.is(true),
        },
        bound,
    ))
}
