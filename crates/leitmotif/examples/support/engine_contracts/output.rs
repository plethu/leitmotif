use leitmotif::ReachabilityConclusion;
use serde::Serialize;

use super::{
    conformance::{Audit, Replay},
    contract::Analysis,
    error::{Difference, Error},
    input::SCHEMA,
};

#[derive(Serialize)]
pub(super) struct Output {
    schema_version: &'static str,
    #[serde(flatten)]
    outcome: Outcome,
}
#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum Outcome {
    Checked {
        analysis: Analysis,
        conformance: Audit,
        replay: Replay,
    },
    Inconclusive {
        analysis: Analysis,
        conformance: Audit,
    },
    RejectedInput {
        diagnostic: String,
    },
    Unsupported {
        diagnostic: String,
    },
    ConformanceFailure {
        evidence: Box<Difference>,
    },
    AnalysisError {
        diagnostic: String,
    },
}
impl Output {
    pub(super) fn checked(analysis: Analysis, conformance: Audit, replay: Replay) -> Self {
        let outcome = if matches!(
            analysis.report.conclusion,
            ReachabilityConclusion::Inconclusive
        ) {
            Outcome::Inconclusive {
                analysis,
                conformance,
            }
        } else {
            Outcome::Checked {
                analysis,
                conformance,
                replay,
            }
        };
        Self {
            schema_version: SCHEMA,
            outcome,
        }
    }
    pub(super) fn error(error: Error) -> Self {
        let outcome = match error {
            Error::RejectedInput(diagnostic) => Outcome::RejectedInput { diagnostic },
            Error::Unsupported(diagnostic) => Outcome::Unsupported { diagnostic },
            Error::ConformanceFailure(evidence) => Outcome::ConformanceFailure { evidence },
            Error::Model(error) => Outcome::AnalysisError {
                diagnostic: error.to_string(),
            },
        };
        Self {
            schema_version: SCHEMA,
            outcome,
        }
    }
    pub(super) const fn status(&self) -> u8 {
        match &self.outcome {
            Outcome::Checked { analysis, .. } => match analysis.report.conclusion {
                ReachabilityConclusion::Reached { .. } => 0,
                ReachabilityConclusion::Unreachable => 1,
                ReachabilityConclusion::Inconclusive => 2,
            },
            Outcome::ConformanceFailure { .. } => 1,
            Outcome::Inconclusive { .. }
            | Outcome::RejectedInput { .. }
            | Outcome::Unsupported { .. } => 2,
            Outcome::AnalysisError { .. } => 3,
        }
    }
}
