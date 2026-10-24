use std::path::PathBuf;
use std::{collections::BTreeSet, marker::PhantomData};

use super::{
    Assignment, Domain, Error, ErrorKind, Fact, FactHandle, FactInfo, Model, Predicate, Source,
    Value, predicate::Identity,
};

/// A named transition. Its guard defaults to true; effects are simultaneous.
#[derive(Clone, Debug)]
pub struct Transition {
    pub(super) id: String,
    pub(super) guard: Predicate,
    pub(super) effects: Vec<Assignment>,
    pub(super) provenance: Source,
}

impl Transition {
    /// Creates a transition. Empty and duplicate IDs are rejected at build time.
    #[must_use]
    #[track_caller]
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            guard: Predicate::constant(true),
            effects: Vec::new(),
            provenance: Source::caller(),
        }
    }

    /// Replaces the guard evaluated against the pre-state.
    #[must_use]
    pub fn with_guard(mut self, guard: Predicate) -> Self {
        self.guard = guard;
        self
    }

    /// Appends a simultaneous effect. Duplicate writes are rejected at build time.
    #[must_use]
    pub fn with_effect(mut self, effect: Assignment) -> Self {
        self.effects.push(effect);
        self
    }
}

/// A named property checked over reachable states, without pruning exploration.
#[derive(Clone, Debug)]
pub struct Invariant {
    pub(super) id: String,
    pub(super) predicate: Predicate,
    pub(super) provenance: Source,
}

impl Invariant {
    /// Creates an invariant. Its name and predicate are validated at build time.
    #[must_use]
    #[track_caller]
    pub fn new(id: impl Into<String>, predicate: Predicate) -> Self {
        Self {
            id: id.into(),
            predicate,
            provenance: Source::caller(),
        }
    }
}

/// An explicit named assumption used only for hypothesis classification.
#[derive(Clone, Debug)]
pub struct Premise {
    pub(super) id: String,
    pub(super) predicate: Predicate,
    pub(super) provenance: Source,
}

impl Premise {
    /// Creates a premise. Classification validates its name and predicate.
    #[must_use]
    #[track_caller]
    pub fn new(id: impl Into<String>, predicate: Predicate) -> Self {
        Self {
            id: id.into(),
            predicate,
            provenance: Source::caller(),
        }
    }
}

/// Authoring context for one finite model.
#[derive(Debug)]
pub struct ModelBuilder {
    pub(super) context: String,
    pub(super) source_root: Option<PathBuf>,
    pub(super) owner: Identity,
    pub(super) facts: Vec<FactInfo>,
    pub(super) transitions: Vec<Transition>,
    pub(super) invariants: Vec<Invariant>,
}

impl ModelBuilder {
    /// Starts a named context.
    ///
    /// # Errors
    /// Returns an error for an empty or whitespace-only context name.
    #[track_caller]
    pub fn new(context: impl Into<String>) -> Result<Self, Error> {
        let context = context.into();
        validate_name(&context, "context", &Source::caller())?;
        Ok(Self {
            context,
            source_root: None,
            owner: Identity::new(),
            facts: Vec::new(),
            transitions: Vec::new(),
            invariants: Vec::new(),
        })
    }

    /// Configures the checkout root for portable report source references.
    /// Original compiler locations are always retained. Paths outside this root
    /// retain their original location without a relative alias.
    #[must_use]
    pub fn with_source_root(mut self, root: impl Into<PathBuf>) -> Self {
        self.source_root = Some(root.into());
        self
    }

    /// Registers a fact and validates its explicitly finite encoding domain.
    ///
    /// # Errors
    /// Rejects empty or duplicate keys, empty domains, duplicate encodings and
    /// empty symbolic encodings. Failure does not register the fact.
    #[track_caller]
    pub fn fact<F: Fact>(&mut self) -> Result<FactHandle<F>, Error> {
        let provenance = Source::caller();
        validate_name(F::KEY, "fact", &provenance)?;
        if self.facts.iter().any(|fact| fact.key == F::KEY) {
            return Err(Error::new(
                ErrorKind::DuplicateId {
                    kind: "fact",
                    id: F::KEY.to_owned(),
                },
                provenance,
            ));
        }
        let mut seen = BTreeSet::new();
        let mut domain = Vec::new();
        for value in F::Value::values() {
            let encoded = value.encode();
            let reason = match &encoded {
                Value::Symbol(symbol) if symbol.trim().is_empty() => {
                    Some("empty symbolic encoding")
                }
                _ if !seen.insert(encoded.clone()) => Some("duplicate encoding"),
                _ => None,
            };
            if let Some(reason) = reason {
                return Err(Error::new(
                    ErrorKind::InvalidDomain {
                        fact: F::KEY.to_owned(),
                        reason,
                    },
                    provenance,
                ));
            }
            domain.push(encoded);
        }
        if domain.is_empty() {
            return Err(Error::new(
                ErrorKind::InvalidDomain {
                    fact: F::KEY.to_owned(),
                    reason: "empty domain",
                },
                provenance,
            ));
        }
        let index = self.facts.len();
        self.facts.push(FactInfo {
            key: F::KEY.to_owned(),
            kind: F::KIND,
            domain,
            provenance,
        });
        Ok(FactHandle {
            owner: self.owner.clone(),
            index,
            marker: PhantomData,
        })
    }

    /// Appends a transition, preserving deterministic authoring order.
    #[must_use]
    pub fn with_transition(mut self, transition: Transition) -> Self {
        self.transitions.push(transition);
        self
    }

    /// Appends an invariant, preserving deterministic diagnostic order.
    #[must_use]
    pub fn with_invariant(mut self, invariant: Invariant) -> Self {
        self.invariants.push(invariant);
        self
    }

    /// Compiles all predicates and effects into checked fact indices.
    ///
    /// # Errors
    /// Rejects invalid names, duplicate transition/invariant IDs, foreign
    /// handles, undeclared values, duplicate effects and setting writes.
    pub fn build(self) -> Result<Model, Error> {
        Model::compile(self)
    }
}

pub(super) fn validate_name(
    name: &str,
    kind: &'static str,
    provenance: &Source,
) -> Result<(), Error> {
    if name.trim().is_empty() {
        return Err(Error::new(ErrorKind::EmptyName(kind), provenance.clone()));
    }
    Ok(())
}

pub(super) fn unique_name(
    seen: &mut BTreeSet<String>,
    name: &str,
    kind: &'static str,
    provenance: &Source,
) -> Result<(), Error> {
    validate_name(name, kind, provenance)?;
    if !seen.insert(name.to_owned()) {
        return Err(Error::new(
            ErrorKind::DuplicateId {
                kind,
                id: name.to_owned(),
            },
            provenance.clone(),
        ));
    }
    Ok(())
}
