use super::super::Error as QueryError;
use rsmt2::errors::{Error as SolverError, ErrorKind as SolverErrorKind};
use std::{error::Error as StdError, path::PathBuf};
use thiserror::Error;

/// Query, deployment and response failures of the experimental SMT capability.
#[derive(Debug, Error)]
pub enum Error {
    /// Ordinary finite query validation failed before starting the solver.
    #[error(transparent)]
    Query(#[from] QueryError),
    /// The executable or soft timeout cannot be represented safely.
    #[error("invalid SMT options: {0}")]
    InvalidOptions(&'static str),
    /// The requested solver executable could not be started or initialized.
    #[error("cannot start SMT executable {executable}: {source}")]
    Deployment {
        /// Explicit executable supplied by the caller.
        executable: PathBuf,
        /// Original deployment or initialization failure.
        #[source]
        source: Box<dyn StdError + Send>,
    },
    /// The solver rejected a command as unsupported; no semantic verdict follows.
    #[error("SMT command unsupported during {operation}: {source}")]
    Unsupported {
        /// Operation that received the unsupported response.
        operation: &'static str,
        /// Original solver response.
        #[source]
        source: Box<dyn StdError + Send>,
    },
    /// Solver communication, parsing or cleanup failed.
    #[error("SMT failure during {operation}: {source}")]
    Protocol {
        /// Operation that failed.
        operation: &'static str,
        /// Original transport or parsing failure.
        #[source]
        source: Box<dyn StdError + Send>,
    },
    /// A returned assignment was incomplete, outside the domain or semantically false.
    #[error("invalid SMT assignment: {0}")]
    InvalidAssignment(String),
    /// An unsat core contained an unknown or repeated generated identifier.
    #[error("invalid SMT core identifier: {0}")]
    InvalidCore(String),
}

pub(super) fn protocol(operation: &'static str, source: SolverError) -> Error {
    if matches!(source.kind(), SolverErrorKind::Unsupported) {
        Error::Unsupported {
            operation,
            source: Box::new(source),
        }
    } else {
        Error::Protocol {
            operation,
            source: Box::new(source),
        }
    }
}
