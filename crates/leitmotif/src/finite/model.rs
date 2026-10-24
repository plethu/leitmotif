use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;

use super::{
    Assignment, Error, ErrorKind, FactKind, ModelBuilder, NamedPredicate, Predicate, Snapshot,
    Source, TransitionInfo, Value,
    author::unique_name,
    compile::{CompiledInvariant, CompiledTransition, describe},
    predicate::Identity,
};

/// Validated finite fact declaration available for model inspection.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct FactInfo {
    /// Stable application-defined key.
    pub key: String,
    /// Whether transitions may write the fact.
    pub kind: FactKind,
    /// Complete validated finite encoding domain, in declared order.
    pub domain: Vec<Value>,
    /// Registration source.
    pub provenance: Source,
}

/// An opaque complete assignment bound to exactly one compiled model.
#[derive(Clone, Debug)]
pub struct State {
    pub(super) owner: Identity,
    pub(super) values: Vec<Value>,
}

/// A checked indexed finite model in one named context.
#[derive(Debug)]
pub struct Model {
    pub(super) context: String,
    pub(super) owner: Identity,
    pub(super) facts: Vec<FactInfo>,
    pub(super) transitions: Vec<CompiledTransition>,
    pub(super) invariants: Vec<CompiledInvariant>,
}

impl Model {
    pub(super) fn compile(builder: ModelBuilder) -> Result<Self, Error> {
        let mut model = Self {
            context: builder.context,
            owner: builder.owner,
            facts: builder.facts,
            transitions: Vec::new(),
            invariants: Vec::new(),
        };
        let mut seen = BTreeSet::new();
        for transition in builder.transitions {
            let compiled = model.compile_transition(transition, &mut seen)?;
            model.transitions.push(compiled);
        }
        seen.clear();
        for invariant in builder.invariants {
            unique_name(&mut seen, &invariant.id, "invariant", &invariant.provenance)?;
            let predicate = model.compile_predicate(&invariant.predicate)?;
            model.invariants.push(CompiledInvariant {
                info: NamedPredicate {
                    id: invariant.id,
                    predicate: describe(&invariant.predicate),
                    provenance: invariant.provenance,
                },
                predicate,
            });
        }
        Ok(model)
    }

    /// Model's single named context.
    #[must_use]
    pub fn context(&self) -> &str {
        &self.context
    }

    /// Finite fact declarations in registration order.
    #[must_use]
    pub fn facts(&self) -> &[FactInfo] {
        &self.facts
    }

    /// Source-linked transition declarations in deterministic execution order.
    pub fn transitions(&self) -> impl Iterator<Item = &TransitionInfo> {
        self.transitions.iter().map(|transition| &transition.info)
    }

    /// Creates a complete initial state, including explicit settings.
    ///
    /// # Errors
    /// Rejects foreign handles, invalid encodings, duplicate assignments and
    /// missing facts. No unspecified value is defaulted.
    #[track_caller]
    pub fn state(&self, assignments: impl IntoIterator<Item = Assignment>) -> Result<State, Error> {
        let provenance = Source::caller();
        let mut assigned = BTreeMap::new();
        for assignment in assignments {
            self.validate_assignment(&assignment)?;
            if assigned
                .insert(assignment.index, assignment.value)
                .is_some()
            {
                return Err(Error::new(
                    ErrorKind::DuplicateWrite(assignment.fact),
                    assignment.provenance,
                ));
            }
        }
        let mut values = Vec::new();
        for (index, fact) in self.facts.iter().enumerate() {
            let value = assigned.remove(&index).ok_or_else(|| {
                Error::new(
                    ErrorKind::MissingAssignment(fact.key.clone()),
                    provenance.clone(),
                )
            })?;
            values.push(value);
        }
        Ok(State {
            owner: self.owner.clone(),
            values,
        })
    }

    /// Obtains a full snapshot after validating state ownership.
    ///
    /// # Errors
    /// Rejects a state from another model.
    #[track_caller]
    pub fn snapshot(&self, state: &State) -> Result<Snapshot, Error> {
        self.validate_state(state)?;
        Ok(self.snapshot_values(&state.values))
    }

    /// Evaluates a typed predicate on a checked state using the analysis evaluator.
    ///
    /// # Errors
    /// Rejects foreign states, foreign handles and invalid query values.
    #[track_caller]
    pub fn evaluate(&self, state: &State, predicate: &Predicate) -> Result<bool, Error> {
        self.validate_state(state)?;
        self.compile_predicate(predicate)?.evaluate(&state.values)
    }

    /// Replays one transition. A false guard returns `None`.
    ///
    /// # Errors
    /// Rejects foreign states and unknown transition IDs.
    #[track_caller]
    pub fn apply(&self, state: &State, transition_id: &str) -> Result<Option<State>, Error> {
        self.validate_state(state)?;
        let provenance = Source::caller();
        let transition = self
            .transitions
            .iter()
            .find(|transition| transition.info.id == transition_id)
            .ok_or_else(|| {
                Error::new(
                    ErrorKind::UnknownTransition(transition_id.to_owned()),
                    provenance,
                )
            })?;
        Ok(transition.apply(&state.values)?.map(|values| State {
            owner: self.owner.clone(),
            values,
        }))
    }

    #[track_caller]
    pub(super) fn validate_state(&self, state: &State) -> Result<(), Error> {
        if !self.owner.matches(&state.owner) {
            return Err(Error::new(ErrorKind::ForeignState, Source::caller()));
        }
        Ok(())
    }

    pub(super) fn snapshot_values(&self, values: &[Value]) -> Snapshot {
        Snapshot {
            values: self
                .facts
                .iter()
                .zip(values)
                .map(|(fact, value)| (fact.key.clone(), value.clone()))
                .collect(),
        }
    }
}
