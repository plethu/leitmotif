use std::{fmt, marker::PhantomData, sync::Arc};

use super::{Domain, Fact, Source, Value};

#[derive(Clone, Debug)]
pub(super) struct Identity(Arc<()>);

impl Identity {
    pub(super) fn new() -> Self {
        Self(Arc::new(()))
    }

    pub(super) fn matches(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

/// A typed handle owned by one model builder; cloning preserves identity.
pub struct FactHandle<F: Fact> {
    pub(super) owner: Identity,
    pub(super) index: usize,
    pub(super) marker: PhantomData<fn() -> F>,
}

impl<F: Fact> fmt::Debug for FactHandle<F> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("FactHandle")
            .field("fact", &F::KEY)
            .field("index", &self.index)
            .finish_non_exhaustive()
    }
}

impl<F: Fact> Clone for FactHandle<F> {
    fn clone(&self) -> Self {
        Self {
            owner: self.owner.clone(),
            index: self.index,
            marker: PhantomData,
        }
    }
}

impl<F: Fact> FactHandle<F> {
    /// Builds an equality predicate, validated when compiled or queried.
    #[must_use]
    #[track_caller]
    pub fn is(&self, value: F::Value) -> Predicate {
        Predicate {
            provenance: Source::caller(),
            kind: PredicateKind::Eq(self.set(value)),
        }
    }

    /// Builds an assignment, validated when used in a state or transition.
    #[must_use]
    #[track_caller]
    pub fn set(&self, value: F::Value) -> Assignment {
        Assignment {
            owner: self.owner.clone(),
            index: self.index,
            fact: F::KEY.to_owned(),
            value: value.encode(),
            provenance: Source::caller(),
        }
    }
}

/// A symbolic fact assignment; its identity cannot be forged by callers.
#[derive(Clone, Debug)]
pub struct Assignment {
    pub(super) owner: Identity,
    pub(super) index: usize,
    pub(super) fact: String,
    pub(super) value: Value,
    pub(super) provenance: Source,
}

impl Assignment {
    /// Stable key of the assigned fact.
    #[must_use]
    pub fn fact(&self) -> &str {
        &self.fact
    }

    /// Symbolic value awaiting validation.
    #[must_use]
    pub const fn value(&self) -> &Value {
        &self.value
    }

    /// Assignment authoring location.
    #[must_use]
    pub const fn provenance(&self) -> &Source {
        &self.provenance
    }
}

/// An inspectable symbolic predicate. No implicit facts or premises are added.
#[derive(Clone, Debug)]
pub struct Predicate {
    /// Predicate operation and children.
    pub kind: PredicateKind,
    /// Location where this operation was authored.
    pub provenance: Source,
}

/// The complete supported finite predicate language.
#[derive(Clone, Debug)]
pub enum PredicateKind {
    /// Literal truth value.
    Constant(bool),
    /// Equality with one fact value.
    Eq(Assignment),
    /// Conjunction.
    And(Box<Predicate>, Box<Predicate>),
    /// Disjunction.
    Or(Box<Predicate>, Box<Predicate>),
    /// Negation.
    Not(Box<Predicate>),
}

impl Predicate {
    /// Creates a literal predicate.
    #[must_use]
    #[track_caller]
    pub fn constant(value: bool) -> Self {
        Self {
            kind: PredicateKind::Constant(value),
            provenance: Source::caller(),
        }
    }

    /// Conjoins two predicates.
    #[must_use]
    #[track_caller]
    pub fn and(self, other: Self) -> Self {
        Self {
            kind: PredicateKind::And(Box::new(self), Box::new(other)),
            provenance: Source::caller(),
        }
    }

    /// Disjoins two predicates.
    #[must_use]
    #[track_caller]
    pub fn or(self, other: Self) -> Self {
        Self {
            kind: PredicateKind::Or(Box::new(self), Box::new(other)),
            provenance: Source::caller(),
        }
    }

    /// Negates a predicate.
    #[must_use]
    #[track_caller]
    pub fn negate(self) -> Self {
        Self {
            kind: PredicateKind::Not(Box::new(self)),
            provenance: Source::caller(),
        }
    }

    /// Material implication, represented as `Not(self) Or other`.
    #[must_use]
    #[track_caller]
    pub fn implies(self, other: Self) -> Self {
        self.negate().or(other)
    }
}
