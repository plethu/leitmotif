#[path = "../../../crates/leitmotif/examples/support/scaling_town.rs"]
mod scaling_town;
#[path = "symbolic/mod.rs"]
mod symbolic;

use std::{error::Error, time::Instant};

use clap::{Parser, ValueEnum};
use leitmotif::{ReachabilityConclusion, SearchLimits};
use scaling_town::{Parameters, Resource, Town};
use serde::Serialize;
use symbolic::{Encoding, Order, Outcome, Oxidd, explore, replay};

#[derive(Clone, Copy, Debug, Serialize, ValueEnum)]
#[serde(rename_all = "snake_case")]
enum Engine {
    Explicit,
    Oxidd,
}

#[derive(Clone, Copy, Debug, Serialize, ValueEnum)]
#[serde(rename_all = "snake_case")]
enum Family {
    Sparse,
    Coupled,
}

#[derive(Clone, Copy, Debug, Serialize, ValueEnum)]
#[serde(rename_all = "snake_case")]
enum Query {
    Goal,
    Unreachable,
    Local,
}

#[derive(Parser)]
#[command(
    about = "Compare bounded private symbolic-town experiments",
    next_line_help = true,
    max_term_width = 80
)]
struct Args {
    /// Explicit reference or the optional `OxiDD` experiment.
    #[arg(long, value_enum)]
    backend: Engine,
    /// Independent cycles, or globally coupled cycles sharing one permit.
    #[arg(long, value_enum, default_value = "sparse")]
    family: Family,
    /// Story threads (1 through 16).
    #[arg(long, default_value_t = 4)]
    threads: usize,
    /// States per thread (2 through 8), including its idle state.
    #[arg(long, default_value_t = 3)]
    local_domain: usize,
    /// Current/next bit placement; explicit exploration ignores this choice.
    #[arg(long, value_enum, default_value = "paired")]
    order: Order,
    /// Goal: all final phases. Local: first thread. Unreachable: closed setting.
    #[arg(long, value_enum, default_value = "goal")]
    query: Query,
    /// Maximum transition depth (0 through 256); truncation is inconclusive.
    #[arg(long, default_value_t = 128)]
    max_depth: usize,
    /// Explicit-explorer state budget; the BDD manager has a node budget.
    #[arg(long, default_value_t = 100_000)]
    max_states: usize,
    /// Fixed `OxiDD` inner-node capacity, separate from external process limits.
    #[arg(long, default_value_t = 1_000_000)]
    node_capacity: usize,
}

#[derive(Serialize)]
struct Measurement {
    backend: Engine,
    family: Family,
    parameters: Parameters,
    order: Order,
    query: Query,
    model_id: String,
    variables: u32,
    analysis_seconds: f64,
    node_count_definition: &'static str,
    result: serde_json::Value,
}

fn symbolic_search(
    backend: &Oxidd,
    encoding: &Encoding<'_>,
    town: &Town,
    target: &leitmotif::Predicate,
    depth: usize,
) -> Result<Outcome, Box<dyn Error>> {
    Ok(explore(
        backend,
        encoding,
        &town.model,
        &town.initial,
        target,
        depth,
    )?)
}

fn explicit(
    town: &Town,
    target: &leitmotif::Predicate,
    args: &Args,
) -> Result<(serde_json::Value, &'static str), Box<dyn Error>> {
    let report = town.model.reachability(
        &town.initial,
        target,
        SearchLimits {
            max_states: args.max_states,
            max_depth: args.max_depth,
        },
    )?;
    let (conclusion, witness) = match report.conclusion {
        ReachabilityConclusion::Reached { witness } => {
            replay(&town.model, &town.initial, target, &witness)?;
            ("reached", Some(witness))
        }
        ReachabilityConclusion::Unreachable => ("unreachable", None),
        ReachabilityConclusion::Inconclusive => ("inconclusive", None),
    };
    Ok((
        serde_json::json!({ "conclusion": conclusion, "coverage": report.coverage, "witness": witness }),
        "explicit complete assignments",
    ))
}

/// Runs one experiment process. External CPU/address-space/time limits belong to
/// the measurement wrapper; a killed process never yields an unreachable result.
///
/// # Errors
/// Rejects invalid fixture dimensions or reports model/BDD allocation failures.
pub fn run() -> Result<(), Box<dyn Error>> {
    println!("{}", serde_json::to_string(&measure(&Args::parse())?)?);
    Ok(())
}

fn measure(args: &Args) -> Result<Measurement, Box<dyn Error>> {
    if args.max_depth > 256 {
        return Err("experiment depth must be at most 256".into());
    }
    if args.node_capacity < 4 {
        return Err("node capacity must be at least 4".into());
    }
    let (coupling, resource) = match args.family {
        Family::Sparse => (0, Resource::Unlimited),
        Family::Coupled => (
            args.threads.checked_sub(1).ok_or("empty town")?,
            Resource::SinglePermit,
        ),
    };
    let parameters = Parameters::new(args.threads, args.local_domain, coupling, resource)?;
    let town = Town::new(parameters)?;
    let target = match args.query {
        Query::Goal => &town.goal,
        Query::Local => &town.local,
        Query::Unreachable => &town.unreachable,
    };
    let encoding = Encoding::new(&town.model, args.order)?;
    let start = Instant::now();
    let (result, definition) = match args.backend {
        Engine::Explicit => explicit(&town, target, args)?,
        Engine::Oxidd => {
            let backend = Oxidd::new(encoding.variables, args.node_capacity)?;
            (
                serde_json::to_value(symbolic_search(
                    &backend,
                    &encoding,
                    &town,
                    target,
                    args.max_depth,
                )?)?,
                "OxiDD function nodes include terminals; manager_inner_nodes separately counts allocated inner nodes",
            )
        }
    };
    Ok(Measurement {
        backend: args.backend,
        family: args.family,
        parameters,
        order: args.order,
        query: args.query,
        model_id: town.model.fingerprint().to_owned(),
        variables: encoding.variables,
        analysis_seconds: start.elapsed().as_secs_f64(),
        node_count_definition: definition,
        result,
    })
}

#[cfg(test)]
#[path = "symbolic/tests/fixture.rs"]
mod tests;
