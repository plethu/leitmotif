use std::path::{Component, Path, PathBuf};
use std::{fmt, panic::Location};

use serde::Serialize;
use thiserror::Error;

use super::Value;

/// Authoring location captured by caller-tracked constructors.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Source {
    /// Source file name supplied by Rust.
    pub file: String,
    /// Checkout-relative path when a configured root contains the original file.
    /// Relative compiler paths are already portable; external absolute paths remain absent.
    pub portable_file: Option<String>,
    /// One-based source line.
    pub line: u32,
    /// One-based source column.
    pub column: u32,
}

impl Source {
    pub(super) fn relativize(&mut self, root: &Path) {
        let path = Path::new(&self.file);
        self.portable_file = if path.is_absolute() {
            path.strip_prefix(root).ok().and_then(portable_suffix)
        } else {
            Some(self.file.clone())
        };
    }

    #[track_caller]
    pub(super) fn caller() -> Self {
        let location = Location::caller();
        Self {
            file: location.file().to_owned(),
            portable_file: (!Path::new(location.file()).is_absolute())
                .then(|| location.file().to_owned()),
            line: location.line(),
            column: location.column(),
        }
    }
}

fn portable_suffix(path: &Path) -> Option<String> {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => normalized.push(part),
            Component::CurDir => {}
            Component::ParentDir if normalized.pop() => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    Some(normalized.to_string_lossy().into_owned())
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
    #[error("duplicate {kind} identifier {id}; first declared at {original}")]
    DuplicateId {
        /// Declaration category.
        kind: &'static str,
        /// Repeated identifier.
        id: String,
        /// Location of the original declaration.
        original: Box<Source>,
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
