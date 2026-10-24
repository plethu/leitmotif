use std::{panic::Location, path::Path};

use leitmotif::Source;
use recite_core::SourceSpan;
use thiserror::Error;

/// A binding or lowering failure with its author-visible source.
#[derive(Debug, Error)]
#[error("{kind} at {provenance}")]
pub struct Error {
    /// Recoverable failure category.
    pub kind: ErrorKind,
    /// Dialogue span start, or Rust binding declaration for registration failures.
    pub provenance: Source,
}

/// Failures are explicit; unsupported guards never become false conditions.
#[derive(Debug, Error)]
pub enum ErrorKind {
    /// No exact function-and-arguments binding exists.
    #[error("unmapped condition call {0}")]
    Unmapped(String),
    /// The same call was registered twice.
    #[error("duplicate condition binding {0}")]
    Duplicate(String),
    /// Boolean and enum condition result kinds cannot be interchanged.
    #[error("wrong condition result type for {0}")]
    WrongType(String),
    /// Enum facts require exclusively symbolic encodings.
    #[error("enum binding requires a symbolic finite domain")]
    NonSymbolicDomain,
    /// The requested case is not in the bound finite enum domain.
    #[error("undeclared enum variant {0}")]
    UnknownVariant(String),
    /// Malformed or over-depth compiled expressions are outside the supported contract.
    #[error("unsupported compiled expression: {0}")]
    Unsupported(&'static str),
}

pub(super) fn source(span: &SourceSpan) -> Source {
    Source {
        file: span.file.clone(),
        portable_file: (!Path::new(&span.file).is_absolute()).then(|| span.file.clone()),
        line: span.start.line(),
        column: span.start.column(),
    }
}

#[track_caller]
pub(super) fn caller() -> Source {
    let location = Location::caller();
    Source {
        file: location.file().to_owned(),
        portable_file: (!Path::new(location.file()).is_absolute())
            .then(|| location.file().to_owned()),
        line: location.line(),
        column: location.column(),
    }
}
