use leitmotif::{ReachabilityReport, SearchLimits};
use serde::Serialize;

use super::super::composed_town::{self, Town};
use super::{
    error::Error,
    host::HostSnapshot,
    input::{BINDING, Limits, Query, Request, SCHEMA},
};

pub(super) struct Contract {
    pub(super) town: Town,
    source_revision: String,
}
#[derive(Debug, Serialize)]
pub(super) struct Analysis {
    pub(super) schema_version: &'static str,
    pub(super) binding_version: &'static str,
    pub(super) source_revision: String,
    pub(super) query: Query,
    pub(super) report: ReachabilityReport,
}
impl Contract {
    pub(super) fn new(source_revision: String) -> Result<Self, Error> {
        if source_revision.trim().is_empty() {
            return Err(Error::RejectedInput("source revision is empty".into()));
        }
        Ok(Self {
            town: composed_town::town()?,
            source_revision,
        })
    }
    pub(super) fn request(&self, initial: HostSnapshot, query: Query, limits: Limits) -> Request {
        Request {
            schema_version: SCHEMA.into(),
            model_id: self.town.model.fingerprint().into(),
            binding_version: BINDING.into(),
            source_revision: self.source_revision.clone(),
            query: query.id().into(),
            limits,
            initial,
        }
    }
    pub(super) fn validate(&self, request: &Request) -> Result<Query, Error> {
        for (label, actual, expected) in [
            ("schema", request.schema_version.as_str(), SCHEMA),
            (
                "model",
                request.model_id.as_str(),
                self.town.model.fingerprint(),
            ),
            ("binding", request.binding_version.as_str(), BINDING),
            (
                "source revision",
                request.source_revision.as_str(),
                self.source_revision.as_str(),
            ),
        ] {
            if actual != expected {
                return Err(Error::RejectedInput(format!(
                    "{label} identity mismatch: expected {expected}, received {actual}"
                )));
            }
        }
        Query::parse(&request.query)
            .ok_or_else(|| Error::Unsupported(format!("query {}", request.query)))
    }
    pub(super) fn decode(&self, input: &str) -> Result<Request, Error> {
        let request = serde_json::from_str(input)?;
        self.validate(&request)?;
        Ok(request)
    }
    pub(super) fn analyse(&self, request: &Request) -> Result<Analysis, Error> {
        let query = self.validate(request)?;
        let town = composed_town::project(&request.initial.projection())?;
        let target = match query {
            Query::Sheltered => &town.sheltered,
            Query::Departed => &town.departed,
            Query::StormPassed => &town.unreachable,
        };
        let report = town.model.reachability(
            &town.initial,
            target,
            SearchLimits {
                max_states: request.limits.max_states,
                max_depth: request.limits.max_depth,
            },
        )?;
        Ok(Analysis {
            schema_version: SCHEMA,
            binding_version: BINDING,
            source_revision: self.source_revision.clone(),
            query,
            report,
        })
    }
}
