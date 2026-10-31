use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

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
    pub(super) source_root: Option<PathBuf>,
    pub(super) fingerprint: String,
    pub(super) owner: Identity,
    pub(super) facts: Vec<FactInfo>,
    pub(super) transitions: Vec<CompiledTransition>,
    pub(super) invariants: Vec<CompiledInvariant>,
}

impl Model {
    pub(super) fn compile(builder: ModelBuilder) -> Result<Self, Error> {
        let mut model = Self {
            context: builder.context,
            source_root: builder.source_root,
            fingerprint: String::new(),
            owner: builder.owner,
            facts: builder.facts,
            transitions: Vec::new(),
            invariants: Vec::new(),
        };
        let mut seen = BTreeMap::new();
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
        model.fingerprint = model.semantic_id()?;
        model.portable_sources();
        Ok(model)
    }

    /// Model's single named context.
    #[must_use]
    pub fn context(&self) -> &str {
        &self.context
    }

    /// SHA-256 identity of this model's ordered semantic declarations.
    /// Excludes source locations, runtime handle ownership and analysis inputs.
    /// Retain the model source, lockfile and toolchain to reproduce a check.
    #[must_use]
    pub fn fingerprint(&self) -> &str {
        &self.fingerprint
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

    /// Restores a state owned by this model from a snapshot, such as a host save.
    ///
    /// Compatibility is decided by fact keys and domains alone, not the model
    /// fingerprint, so saves survive unrelated model edits. Migrate
    /// `snapshot.values` before restoring when declarations change.
    ///
    /// # Errors
    /// Returns one [`ErrorKind::SnapshotMismatch`] listing every missing fact,
    /// unknown key and out-of-domain value. No unspecified value is defaulted.
    #[track_caller]
    pub fn restore(&self, snapshot: &Snapshot) -> Result<State, Error> {
        let provenance = Source::caller();
        let mut values = Vec::with_capacity(self.facts.len());
        let mut missing = Vec::new();
        let mut invalid = Vec::new();
        for fact in &self.facts {
            match snapshot.values.get(&fact.key) {
                None => missing.push(fact.key.clone()),
                Some(value) if !fact.domain.contains(value) => {
                    invalid.push((fact.key.clone(), value.clone()));
                }
                Some(value) => values.push(value.clone()),
            }
        }
        let matched = values.len().saturating_add(invalid.len());
        if missing.is_empty() && invalid.is_empty() && snapshot.values.len() == matched {
            return Ok(State {
                owner: self.owner.clone(),
                values,
            });
        }
        let declared: BTreeSet<&str> = self.facts.iter().map(|fact| fact.key.as_str()).collect();
        let unknown: Vec<String> = snapshot
            .values
            .keys()
            .filter(|key| !declared.contains(key.as_str()))
            .cloned()
            .collect();
        missing.sort_unstable();
        invalid.sort_unstable();
        Err(Error::new(
            ErrorKind::SnapshotMismatch {
                missing: missing.into(),
                unknown: unknown.into(),
                invalid: invalid.into(),
            },
            provenance,
        ))
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
