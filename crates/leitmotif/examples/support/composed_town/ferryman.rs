use super::{Shared, keeper::Warning};
use leitmotif::{Error, Fact, FactHandle, ModelBuilder, Transition};

pub(in super::super) struct FerryReady;
impl Fact for FerryReady {
    type Value = bool;
    const KEY: &'static str = "ferryman.ready";
}

pub(super) struct Ferryman {
    pub(super) ready: FactHandle<FerryReady>,
}

pub(super) fn declare(builder: &mut ModelBuilder) -> Result<Ferryman, Error> {
    Ok(Ferryman {
        ready: builder.fact::<FerryReady>()?,
    })
}

impl Ferryman {
    pub(super) fn author(
        &self,
        builder: ModelBuilder,
        shared: &Shared,
        warning: &FactHandle<Warning>,
    ) -> ModelBuilder {
        builder.with_transition(
            Transition::new("ferryman.fuel_ferry")
                .with_guard(
                    shared
                        .active
                        .clone()
                        .and(warning.is(true))
                        .and(shared.oil.is(true)),
                )
                .with_effect(self.ready.set(true))
                .with_effect(shared.oil.set(false)),
        )
    }
}
