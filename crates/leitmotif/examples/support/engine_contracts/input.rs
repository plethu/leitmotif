//! Versioned requests are projections of engine-owned state, not authored models.
use serde::{Deserialize, Serialize};

use super::host::HostSnapshot;

pub(super) const SCHEMA: &str = "leitmotif.harbour.engine.experimental.v1";
pub(super) const BINDING: &str = "harbour.six-facts-five-actions.v1";

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Limits {
    pub(super) max_states: usize,
    pub(super) max_depth: usize,
}
impl Limits {
    pub(super) const COMPLETE: Self = Self {
        max_states: 6,
        max_depth: 3,
    };
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Request {
    pub(super) schema_version: String,
    pub(super) model_id: String,
    pub(super) binding_version: String,
    pub(super) source_revision: String,
    pub(super) query: String,
    pub(super) limits: Limits,
    pub(super) initial: HostSnapshot,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Query {
    Sheltered,
    Departed,
    StormPassed,
}
impl Query {
    pub(super) fn parse(value: &str) -> Option<Self> {
        match value {
            "sheltered" => Some(Self::Sheltered),
            "departed" => Some(Self::Departed),
            "storm_passed" => Some(Self::StormPassed),
            _ => None,
        }
    }
    pub(super) const fn id(self) -> &'static str {
        match self {
            Self::Sheltered => "sheltered",
            Self::Departed => "departed",
            Self::StormPassed => "storm_passed",
        }
    }
}
