//! An experimental display projection: runtime sessions remain private to replay.
use std::error::Error;

use leitmotif::{Snapshot, Source};
use serde::Serialize;

use super::{
    evidence::{ActionSource, Boundary, Outcome},
    exploration::{Conclusion, Report},
    presentation::{action_name, outcome_name},
    source,
};

pub(super) const SCHEMA: &str = "leitmotif.archive-ci.experimental.v1";

#[derive(Serialize)]
pub(super) struct Export<'a> {
    schema_version: &'static str,
    tool_version: &'static str,
    virtual_source: VirtualSource,
    #[serde(flatten)]
    result: ResultView<'a>,
}
#[derive(Serialize)]
struct VirtualSource {
    path: &'static str,
    content: &'static str,
}
#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum ResultView<'a> {
    Analysed { runs: Vec<Run<'a>> },
    Unsupported { message: String, source: Location },
}
#[derive(Serialize)]
struct Run<'a> {
    model_id: &'a str,
    property: &'a str,
    protocol: &'a str,
    recite_revision: &'a str,
    initial: &'a Snapshot,
    limits: Limits,
    coverage: Coverage,
    conclusion: &'static str,
    witness: Vec<Step<'a>>,
}
#[derive(Serialize)]
struct Limits {
    max_nodes: usize,
    max_depth: usize,
}
#[derive(Serialize)]
struct Coverage {
    evaluated_nodes: usize,
    generated_nodes: usize,
    peak_frontier: usize,
    completed_paths: usize,
}
#[derive(Serialize)]
struct Step<'a> {
    action: String,
    outcome: String,
    source: Location,
    declaration: Option<Location>,
    host: &'a Snapshot,
}
#[derive(Serialize)]
pub(super) struct Location {
    kind: &'static str,
    path: String,
    line: u32,
    column: u32,
}
impl Location {
    pub(super) fn repository(source: &Source) -> Result<Self, Box<dyn Error>> {
        Ok(Self {
            kind: "repository",
            path: source
                .portable_file
                .clone()
                .ok_or("source has no portable repository path")?,
            line: source.line,
            column: source.column,
        })
    }
    fn boundary(boundary: &Boundary) -> Result<Self, Box<dyn Error>> {
        match &boundary.source {
            ActionSource::Host(source) => Self::repository(source),
            ActionSource::Dialogue(span) => Ok(Self {
                kind: "virtual",
                path: span.file.clone(),
                line: span.start.line(),
                column: span.start.column(),
            }),
        }
    }
}
impl<'a> Export<'a> {
    const fn new(result: ResultView<'a>) -> Self {
        Self {
            schema_version: SCHEMA,
            tool_version: env!("CARGO_PKG_VERSION"),
            virtual_source: VirtualSource {
                path: source::PATH,
                content: source::TEXT,
            },
            result,
        }
    }
    pub(super) fn analysed(
        reports: &'a [(super::Initial, Report)],
    ) -> Result<Self, Box<dyn Error>> {
        let runs = reports
            .iter()
            .map(|(_, report)| Run::new(report))
            .collect::<Result<_, _>>()?;
        Ok(Self::new(ResultView::Analysed { runs }))
    }
    pub(super) const fn unsupported(message: String, source: Location) -> Self {
        Self::new(ResultView::Unsupported { message, source })
    }
}
impl<'a> Run<'a> {
    fn new(report: &'a Report) -> Result<Self, Box<dyn Error>> {
        let (conclusion, witness) = match &report.conclusion {
            Conclusion::Exhaustive => ("exhaustive", &[][..]),
            Conclusion::Inconclusive => ("inconclusive", &[][..]),
            Conclusion::Violation { witness } => ("violation", witness.as_slice()),
            Conclusion::RuntimeFailed { witness } => ("runtime_failed", witness.as_slice()),
        };
        Ok(Self {
            model_id: &report.model_id,
            property: report.property,
            protocol: report.protocol,
            recite_revision: report.recite_revision,
            initial: &report.initial,
            limits: Limits {
                max_nodes: report.limits.max_nodes,
                max_depth: report.limits.max_depth,
            },
            coverage: Coverage {
                evaluated_nodes: report.evaluated_nodes,
                generated_nodes: report.generated_nodes,
                peak_frontier: report.peak_frontier,
                completed_paths: report.completed_paths.len(),
            },
            conclusion,
            witness: witness
                .iter()
                .map(|boundary| {
                    Ok(Step {
                        action: format!("{}: {:?}", action_name(&boundary.action), boundary.action),
                        outcome: outcome_name(&boundary.outcome),
                        source: Location::boundary(boundary)?,
                        declaration: match &boundary.outcome {
                            Outcome::HostApplied(transition) | Outcome::Restored(transition) => {
                                Some(Location::repository(&transition.provenance)?)
                            }
                            Outcome::HostDisabled { failure, .. } => {
                                Some(Location::repository(&failure.provenance)?)
                            }
                            Outcome::Delivered(_)
                            | Outcome::RuntimeFailed(_)
                            | Outcome::Acknowledged { .. } => None,
                        },
                        host: &boundary.host,
                    })
                })
                .collect::<Result<_, Box<dyn Error>>>()?,
        })
    }
}
