use std::{fmt, panic::Location};

use serde::Serialize;
use thiserror::Error;

use super::Value;

/// Authoring location captured by caller-tracked constructors.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Source {
    /// Source file name supplied by Rust.
    pub file: String,
    /// One-based source line.
    pub line: u32,
    /// One-based source column.
    pub column: u32,
}

impl Source {
    #[track_caller]
    pub(super) fn caller() -> Self {
        let location = Location::caller();
        Self {
            file: location.file().to_owned(),
            line: location.line(),
            column: location.column(),
        }
    }
}

impl fmt::Display for Source {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}:{}:{}", self.file, self.line, self.column)
    }
}

/// A validation or query failure linked to its authoring location.
#[derive(Clone, Debug, PartialEq, Eq, Error)]
#[error("{kind} at {provenance}")]
pub struct Error {
    /// Machine-matchable failure category.
    pub kind: ErrorKind,
    /// Location of the invalid declaration, assignment, or query.
    pub provenance: Source,
}

impl Error {
    pub(super) const fn new(kind: ErrorKind, provenance: Source) -> Self {
        Self { kind, provenance }
    }
}

/// Recoverable errors from checked authoring and analysis.
#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum ErrorKind {
    /// Contexts, fact keys and named declarations must be nonempty.
    #[error("empty {0} name")]
    EmptyName(&'static str),
    /// A declared domain is empty, repeats encodings, or contains empty symbols.
    #[error("invalid domain for fact {fact}: {reason}")]
    InvalidDomain {
        /// Stable fact key.
        fact: String,
        /// Explanation of the invalid declaration.
        reason: &'static str,
    },
    /// Stable identifiers must be unique in their declaration category.
    #[error("duplicate {kind} identifier {id}")]
    DuplicateId {
        /// Declaration category.
        kind: &'static str,
        /// Repeated identifier.
        id: String,
    },
    /// A predicate or assignment used a handle from another builder.
    #[error("foreign handle for fact {0}")]
    ForeignHandle(String),
    /// States belong to exactly one compiled model.
    #[error("state belongs to another model")]
    ForeignState,
    /// Replay requested a transition not present in this model.
    #[error("unknown transition {0}")]
    UnknownTransition(String),
    /// An encoding was not among the fact's declared domain values.
    #[error("invalid value {value:?} for fact {fact}")]
    InvalidValue {
        /// Stable fact key.
        fact: String,
        /// Rejected encoding.
        value: Value,
    },
    /// Initial states require every fact exactly once.
    #[error("missing initial assignment for fact {0}")]
    MissingAssignment(String),
    /// A state or transition assigned the same fact twice.
    #[error("duplicate assignment for fact {0}")]
    DuplicateWrite(String),
    /// Settings cannot be transition effects.
    #[error("transition writes setting {0}")]
    SettingWrite(String),
    /// Checked internal consistency failed instead of panicking.
    #[error("invalid compiled model: {0}")]
    InvalidModel(&'static str),
    /// A representable counter was exhausted.
    #[error("analysis counter capacity exhausted")]
    Capacity,
}
