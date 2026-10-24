//! Machine-readable measurements of a synthetic, cyclic town family.

#[path = "support/scaling_town.rs"]
mod scaling_town;

use std::{error::Error, fs, io, time::Instant};

use clap::{Parser, ValueEnum};
use leitmotif::{ReachabilityConclusion, SearchCoverage, SearchLimits};
pub use scaling_town::{Parameters, Resource, Town};
use serde::Serialize;

#[derive(Clone, Copy, Debug, ValueEnum)]
enum ResourceArg {
    Unlimited,
    SinglePermit,
}

#[derive(Clone, Copy, Debug, Serialize, ValueEnum)]
#[serde(rename_all = "snake_case")]
enum Query {
    Goal,
    Unreachable,
}

#[derive(Debug, Parser)]
#[command(about = "Measure a deterministic synthetic town (peak RSS requires Linux /proc)")]
struct Args {
    /// Independent cyclic story threads (1 through 16).
    #[arg(long, default_value_t = 6)]
    threads: usize,
    /// States per thread, including its idle state (2 through 8).
    #[arg(long, default_value_t = 3)]
    local_domain: usize,
    /// Following neighbors that must be idle before a thread starts.
    #[arg(long, default_value_t = 0)]
    coupling: usize,
    /// Whether threads compete for a single reusable permit.
    #[arg(long, value_enum, default_value = "unlimited")]
    resource: ResourceArg,
    /// Goal requires all threads in their final phase; unreachable asks for a closed setting.
    #[arg(long, value_enum, default_value = "unreachable")]
    query: Query,
    #[arg(long, default_value_t = 100_000)]
    max_states: usize,
    #[arg(long, default_value_t = 128)]
    max_depth: usize,
}

#[derive(Serialize)]
struct Measurement {
    parameters: Parameters,
    query: Query,
    coverage: SearchCoverage,
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

fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();
    let resource = match args.resource {
        ResourceArg::Unlimited => Resource::Unlimited,
        ResourceArg::SinglePermit => Resource::SinglePermit,
    };
    let parameters = Parameters::new(args.threads, args.local_domain, args.coupling, resource)?;
    let town = Town::new(parameters)?;
    let query = match args.query {
        Query::Goal => &town.goal,
        Query::Unreachable => &town.unreachable,
    };
    let start = Instant::now();
    let report = town.model.reachability(
        &town.initial,
        query,
        SearchLimits {
            max_states: args.max_states,
            max_depth: args.max_depth,
        },
    )?;
    let analysis_seconds = start.elapsed().as_secs_f64();
    let (conclusion, witness_length) = match &report.conclusion {
        ReachabilityConclusion::Reached { witness } => ("reached", Some(witness.steps.len())),
        ReachabilityConclusion::Unreachable => ("unreachable", None),
        ReachabilityConclusion::Inconclusive => ("inconclusive", None),
    };
    let measurement = Measurement {
        parameters,
        query: args.query,
        coverage: report.coverage,
        conclusion,
        witness_length,
        analysis_seconds,
        peak_rss_bytes: peak_rss_bytes()?,
    };
    serde_json::to_writer(io::stdout().lock(), &measurement)?;
    Ok(())
}
