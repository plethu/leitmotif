use super::Shared;
use leitmotif::{Error, Fact, FactHandle, ModelBuilder, Transition};

pub(super) struct Warning;
impl Fact for Warning {
    type Value = bool;
    const KEY: &'static str = "keeper.warning_shared";
}
pub(in super::super) struct Beacon;
impl Fact for Beacon {
    type Value = bool;
    const KEY: &'static str = "keeper.beacon_lit";
}

pub(super) struct Keeper {
    pub(super) warning: FactHandle<Warning>,
    pub(super) beacon: FactHandle<Beacon>,
}

pub(super) fn declare(builder: &mut ModelBuilder) -> Result<Keeper, Error> {
    Ok(Keeper {
        warning: builder.fact::<Warning>()?,
        beacon: builder.fact::<Beacon>()?,
    })
}

impl Keeper {
    pub(super) fn author(&self, builder: ModelBuilder, shared: &Shared) -> ModelBuilder {
        builder
            .with_transition(
                Transition::new("keeper.share_warning")
                    .with_guard(shared.active.clone().and(self.warning.is(false)))
                    .with_effect(self.warning.set(true)),
            )
            .with_transition(
                Transition::new("keeper.light_beacon")
                    .with_guard(
                        shared
                            .active
                            .clone()
                            .and(self.warning.is(true))
                            .and(shared.oil.is(true)),
                    )
                    .with_effect(self.beacon.set(true))
                    .with_effect(shared.oil.set(false)),
            )
    }
}
