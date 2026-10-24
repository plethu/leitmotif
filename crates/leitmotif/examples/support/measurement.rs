//! Machine-readable measurements of synthetic cycles and composed harbour stories.

#[path = "composed_town/mod.rs"]
mod composed_town;
#[path = "scaling_town.rs"]
mod scaling_town;

use std::{error::Error, fs, io, time::Instant};

use clap::{Parser, ValueEnum};
use leitmotif::{Model, Predicate, State};
use leitmotif::{ProjectedCoverage, ReachabilityConclusion, SearchCoverage, SearchLimits};
use scaling_town::{Parameters, Resource, Town};
use serde::Serialize;

#[derive(Clone, Copy, Debug, ValueEnum)]
enum ResourceArg {
    Unlimited,
    SinglePermit,
}

#[derive(Clone, Copy, Debug, Serialize, ValueEnum)]
#[serde(rename_all = "snake_case")]
enum Fixture {
    Synthetic,
    Composed,
}

#[derive(Clone, Copy, Debug, Serialize, ValueEnum)]
#[serde(rename_all = "snake_case")]
enum Query {
    Goal,
    Local,
    Unreachable,
    Alternative,
}

#[derive(Clone, Copy, Debug, Serialize, ValueEnum)]
#[serde(rename_all = "snake_case")]
enum Analysis {
    Exhaustive,
    Sliced,
}

#[derive(Debug, Parser)]
#[command(
    about = "Measure a deterministic finite town (peak RSS requires Linux /proc)",
    next_line_help = true,
    max_term_width = 80
)]
struct Args {
    /// Measure synthetic cycles or the separately authored harbour stories.
    #[arg(long, value_enum, default_value = "synthetic")]
    fixture: Fixture,
    /// Independent cyclic story threads (1 through 16).
    #[arg(long)]
    threads: Option<usize>,
    /// States per thread, including its idle state (2 through 8).
    #[arg(long)]
    local_domain: Option<usize>,
    /// Following neighbors that must be idle before a thread starts.
    #[arg(long)]
    coupling: Option<usize>,
    /// Whether threads compete for a single reusable permit.
    #[arg(long, value_enum)]
    resource: Option<ResourceArg>,
    /// Goal: all final phases or shelter. Local: first thread. Alternative: ferry.
    #[arg(long, value_enum, default_value = "unreachable")]
    query: Query,
    /// Enumerate full assignments or query-dependent projections.
    #[arg(long, value_enum, default_value = "exhaustive")]
    analysis: Analysis,
    #[arg(long, default_value_t = 100_000)]
    max_states: usize,
    #[arg(long, default_value_t = 128)]
    max_depth: usize,
}

impl Args {
    fn validate_composed(&self) -> Result<(), Box<dyn Error>> {
        for (specified, flag) in [
            (self.threads.is_some(), "--threads"),
            (self.local_domain.is_some(), "--local-domain"),
            (self.coupling.is_some(), "--coupling"),
            (self.resource.is_some(), "--resource"),
        ] {
            if specified {
                return Err(format!("{flag} applies only to the synthetic fixture").into());
            }
        }
        Ok(())
    }

    fn build_input(&self) -> Result<MeasurementInput, Box<dyn Error>> {
        match self.fixture {
            Fixture::Synthetic => {
                let resource = match self.resource.unwrap_or(ResourceArg::Unlimited) {
                    ResourceArg::Unlimited => Resource::Unlimited,
                    ResourceArg::SinglePermit => Resource::SinglePermit,
                };
                let parameters = Parameters::new(
                    self.threads.unwrap_or(6),
                    self.local_domain.unwrap_or(3),
                    self.coupling.unwrap_or(0),
                    resource,
                )?;
                let town = Town::new(parameters)?;
                let target = match self.query {
                    Query::Goal => town.goal,
                    Query::Local => town.local,
                    Query::Unreachable => town.unreachable,
                    Query::Alternative => {
                        return Err(
                            "alternative ending is available only for the composed fixture".into(),
                        );
                    }
                };
                Ok(MeasurementInput {
                    model: town.model,
                    initial: town.initial,
                    target,
                    parameters: Some(parameters),
                })
            }
            Fixture::Composed => {
                self.validate_composed()?;
                let town = composed_town::town()?;
                let target = match self.query {
                    Query::Goal => town.sheltered,
                    Query::Local => {
                        return Err("local query applies only to the synthetic fixture".into());
                    }
                    Query::Alternative => town.departed,
                    Query::Unreachable => town.unreachable,
                };
                Ok(MeasurementInput {
                    model: town.model,
                    initial: town.initial,
                    target,
                    parameters: None,
                })
            }
        }
    }
}

struct MeasurementInput {
    model: Model,
    initial: State,
    target: Predicate,
    parameters: Option<Parameters>,
}

#[derive(Serialize)]
#[serde(tag = "analysis", rename_all = "snake_case")]
enum MeasurementSearch {
    Exhaustive {
        coverage: SearchCoverage,
    },
    Sliced {
        coverage: ProjectedCoverage,
        slice: Slice,
    },
}

#[derive(Serialize)]
struct Slice {
    retained_facts: Vec<String>,
    retained_transitions: Vec<String>,
}

#[derive(Serialize)]
struct Measurement {
    fixture: Fixture,
    parameters: Option<Parameters>,
    authored_events: usize,
    authored_facts: usize,
    query: Query,
    #[serde(flatten)]
    search: MeasurementSearch,
    conclusion: &'static str,
    witness_length: Option<usize>,
    analysis_seconds: f64,
    peak_rss_bytes: usize,
}

fn peak_rss_bytes() -> Result<usize, Box<dyn Error>> {
    let status = fs::read_to_string("/proc/self/status")
        .map_err(|error| format!("peak RSS requires Linux /proc/self/status: {error}"))?;
    let kilobytes: usize = status
        .lines()
        .find_map(|line| line.strip_prefix("VmHWM:"))
        .and_then(|value| value.split_whitespace().next())
        .ok_or("Linux status omitted VmHWM")?
        .parse()?;
    Ok(kilobytes
        .checked_mul(1024)
        .ok_or("peak RSS exceeds counter capacity")?)
}

/// Measure one selected finite town and write a JSON record to stdout.
///
/// # Errors
/// Returns invalid CLI/model inputs, analysis failures, or output and RSS I/O errors.
pub fn run() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();
    let input = args.build_input()?;
    let start = Instant::now();
    let limits = SearchLimits {
        max_states: args.max_states,
        max_depth: args.max_depth,
    };
    let (conclusion, search) = match args.analysis {
        Analysis::Exhaustive => {
            let report = input
                .model
                .reachability(&input.initial, &input.target, limits)?;
            (
                report.conclusion,
                MeasurementSearch::Exhaustive {
                    coverage: report.coverage,
                },
            )
        }
        Analysis::Sliced => {
            let report = input
                .model
                .sliced_reachability(&input.initial, &input.target, limits)?;
            (
                report.conclusion,
                MeasurementSearch::Sliced {
                    coverage: report.coverage,
                    slice: Slice {
                        retained_facts: report.retained_facts,
                        retained_transitions: report.retained_transitions,
                    },
                },
            )
        }
    };
    let analysis_seconds = start.elapsed().as_secs_f64();
    let (conclusion, witness_length) = match &conclusion {
        ReachabilityConclusion::Reached { witness } => ("reached", Some(witness.steps.len())),
        ReachabilityConclusion::Unreachable => ("unreachable", None),
        ReachabilityConclusion::Inconclusive => ("inconclusive", None),
    };
    let measurement = Measurement {
        fixture: args.fixture,
        authored_events: input.model.transitions().count(),
        authored_facts: input.model.facts().len(),
        parameters: input.parameters,
        query: args.query,
        search,
        conclusion,
        witness_length,
        analysis_seconds,
        peak_rss_bytes: peak_rss_bytes()?,
    };
    serde_json::to_writer(io::stdout().lock(), &measurement)?;
    Ok(())
}
