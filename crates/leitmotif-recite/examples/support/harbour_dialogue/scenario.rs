use leitmotif_recite::Bindings;
use recite_core::compiled::{CompiledArgument, CompiledConditionCall, CompiledDialogue};

use super::super::composed_town::{self, Projection, Town};
use super::RunResult;

pub(super) struct Scenario {
    pub(super) town: Town,
    pub(super) asset: CompiledDialogue,
    pub(super) conditions: Bindings,
}

impl Scenario {
    pub(super) fn from_asset(asset: CompiledDialogue) -> RunResult<Self> {
        let (town, conditions) = composed_town::project_with(&Projection::default(), |facts| {
            let mut conditions = Bindings::new();
            conditions.bind_bool(call("has_oil", "harbour"), &facts.oil)?;
            conditions.bind_bool(call("beacon_lit", "keeper"), &facts.beacon)?;
            conditions.bind_bool(call("ferry_ready", "ferryman"), &facts.ferry)?;
            conditions.bind_enum(call("harbour_ending", "harbour"), &facts.ending)?;
            Ok::<_, leitmotif_recite::Error>(conditions)
        })?;
        Ok(Self {
            town,
            asset,
            conditions: conditions?,
        })
    }
}

pub(super) fn call(function: &str, argument: &str) -> CompiledConditionCall {
    CompiledConditionCall {
        function: function.into(),
        args: vec![CompiledArgument::Identifier(argument.into())],
    }
}
