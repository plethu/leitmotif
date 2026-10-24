use std::error::Error as StdError;

use super::facts::{
    Access, Belief, Door, Exposed, Key, KeyRemoval, RestoreAfter, RestoreBefore, ResultRecorded,
    Truth, Visit, VisitResult,
};
use leitmotif::{
    Domain, Error, FactHandle, Invariant, Model, ModelBuilder, Predicate, State, Transition,
};
use leitmotif_recite::Bindings;
use recite_core::compiled::{CompiledArgument, CompiledConditionCall, CompiledDialogue};

use super::source;

pub(super) const PROPERTY: &str = "opening_requires_current_key";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ModelVariant {
    Faulty,
    Corrected,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Initial {
    pub(super) has_key: bool,
    pub(super) truth: bool,
    pub(super) belief: bool,
}
impl Initial {
    pub(super) fn all() -> Vec<Self> {
        let mut assignments = Vec::new();
        for has_key in [false, true] {
            for (truth, belief) in [(false, false), (false, true), (true, false), (true, true)] {
                assignments.push(Self {
                    has_key,
                    truth,
                    belief,
                });
            }
        }
        assignments
    }
}

pub(super) struct Scenario {
    pub(super) model: Model,
    pub(super) asset: CompiledDialogue,
    pub(super) conditions: Bindings,
    opening_policy: Predicate,
    removal_policy: Predicate,
    key: FactHandle<Key>,
    door: FactHandle<Door>,
    exposed: FactHandle<Exposed>,
    truth: FactHandle<Truth>,
    belief: FactHandle<Belief>,
    visit: FactHandle<Visit>,
    result: FactHandle<ResultRecorded>,
    removal: FactHandle<KeyRemoval>,
    restore_before: FactHandle<RestoreBefore>,
    restore_after: FactHandle<RestoreAfter>,
}

impl Scenario {
    pub(super) fn new(variant: ModelVariant) -> Result<Self, Box<dyn StdError>> {
        let mut builder = ModelBuilder::new("archive.access")?;
        let key = builder.fact::<Key>()?;
        let door = builder.fact::<Door>()?;
        let exposed = builder.fact::<Exposed>()?;
        let truth = builder.fact::<Truth>()?;
        let belief = builder.fact::<Belief>()?;
        let visit = builder.fact::<Visit>()?;
        let result = builder.fact::<ResultRecorded>()?;
        let removal = builder.fact::<KeyRemoval>()?;
        let restore_before = builder.fact::<RestoreBefore>()?;
        let restore_after = builder.fact::<RestoreAfter>()?;
        let unlock_guard = match variant {
            ModelVariant::Faulty => door.is(Access::Closed),
            ModelVariant::Corrected => door.is(Access::Closed).and(key.is(true)),
        };
        let opening_policy = door.is(Access::Opened).implies(key.is(true));
        let removal_policy = removal.is(true).and(key.is(true));
        builder = builder
            .with_transition(
                Transition::new("expose")
                    .with_guard(exposed.is(false))
                    .with_effect(exposed.set(true)),
            )
            .with_transition(
                Transition::new("remove_key")
                    .with_guard(removal_policy.clone())
                    .with_effect(key.set(false))
                    .with_effect(removal.set(false)),
            )
            .with_transition(
                Transition::new("restore_before")
                    .with_guard(restore_before.is(true))
                    .with_effect(restore_before.set(false)),
            )
            .with_transition(
                Transition::new("restore_after")
                    .with_guard(restore_after.is(true))
                    .with_effect(restore_after.set(false)),
            )
            .with_transition(
                Transition::new("unlock")
                    .with_guard(unlock_guard)
                    .with_effect(door.set(Access::Opened)),
            )
            .with_transition(
                Transition::new("deny")
                    .with_guard(door.is(Access::Closed))
                    .with_effect(door.set(Access::Denied)),
            )
            .with_transition(
                Transition::new("record_visit")
                    .with_guard(visit.is(false))
                    .with_effect(visit.set(true)),
            )
            .with_invariant(Invariant::new(PROPERTY, opening_policy.clone()));
        for access in Access::values() {
            builder = builder.with_transition(
                Transition::new(format!("record_{}", access.symbol()))
                    .with_guard(door.is(access).and(result.is(VisitResult::Unrecorded)))
                    .with_effect(result.set(VisitResult::Recorded(access))),
            );
        }
        let model = builder.build()?;
        let mut conditions = Bindings::new();
        conditions.bind_bool(call("has_key"), &key)?;
        conditions.bind_enum(call("archive_access"), &door)?;
        let asset = source::compile(source::TEXT)?;
        Ok(Self {
            model,
            asset,
            conditions,
            opening_policy,
            removal_policy,
            key,
            door,
            exposed,
            truth,
            belief,
            visit,
            result,
            removal,
            restore_before,
            restore_after,
        })
    }

    pub(super) fn initial(&self, initial: Initial) -> Result<State, Error> {
        self.model.state([
            self.key.set(initial.has_key),
            self.door.set(Access::Closed),
            self.exposed.set(false),
            self.truth.set(initial.truth),
            self.belief.set(initial.belief),
            self.visit.set(false),
            self.result.set(VisitResult::Unrecorded),
            self.removal.set(true),
            self.restore_before.set(true),
            self.restore_after.set(true),
        ])
    }

    pub(super) fn access(&self, state: &State) -> Result<Access, Error> {
        if self.model.evaluate(state, &self.door.is(Access::Opened))? {
            Ok(Access::Opened)
        } else if self.model.evaluate(state, &self.door.is(Access::Denied))? {
            Ok(Access::Denied)
        } else {
            Ok(Access::Closed)
        }
    }

    pub(super) fn has_key(&self, state: &State) -> Result<bool, Error> {
        self.model.evaluate(state, &self.key.is(true))
    }
    pub(super) fn violation(&self, state: &State) -> Result<bool, Error> {
        Ok(!self.model.evaluate(state, &self.opening_policy)?)
    }
    pub(super) fn can_remove_key(&self, state: &State) -> Result<bool, Error> {
        self.model.evaluate(state, &self.removal_policy)
    }
    pub(super) fn can_restore(
        &self,
        state: &State,
        window: super::protocol::RestoreWindow,
    ) -> Result<bool, Error> {
        let predicate = if window == super::protocol::RestoreWindow::AfterCommit {
            self.restore_after.is(true)
        } else {
            self.restore_before.is(true)
        };
        self.model.evaluate(state, &predicate)
    }
}

fn call(function: &str) -> CompiledConditionCall {
    CompiledConditionCall {
        function: function.into(),
        args: vec![CompiledArgument::Identifier("archive".into())],
    }
}
