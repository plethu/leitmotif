use leitmotif::{Snapshot, Source};
use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Error)]
pub(super) enum Error {
    #[error("request rejected: {0}")]
    RejectedInput(String),
    #[error("unsupported mapping: {0}")]
    Unsupported(String),
    #[error("host/model disagreement at {0:?}")]
    ConformanceFailure(Box<Difference>),
    #[error(transparent)]
    Model(#[from] leitmotif::Error),
}
#[derive(Debug, Serialize)]
pub(super) struct Difference {
    pub(super) action: String,
    pub(super) provenance: Source,
    pub(super) expected_enabled: bool,
    pub(super) actual_enabled: bool,
    pub(super) expected: Snapshot,
    pub(super) actual: Snapshot,
    pub(super) operation: Operation,
    /// Actions verified before the mismatching command. Its effects may already exist.
    pub(super) verified_prefix_actions: usize,
    /// Commands actually accepted by this host, including the mismatching command.
    pub(super) accepted_host_actions: usize,
}
impl From<serde_json::Error> for Error {
    fn from(error: serde_json::Error) -> Self {
        Self::RejectedInput(error.to_string())
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Operation {
    Audit,
    Replay,
}
