//! Cold-process comparison of optional solver classification with finite enumeration.
#[path = "mystery.rs"]
mod mystery;
mod prerequisites;

use clap::{Parser, ValueEnum};
use leitmotif::{
    ClassificationLimits, HypothesisReport, Model, Predicate, Premise,
    smt::{Options, Report},
};
pub use mystery::{Mystery, MysteryEvidence, mystery};
use prerequisites::Prerequisites;
use serde::Serialize;
use std::{
    error::Error,
    io,
    path::PathBuf,
    time::{Duration, Instant},
};

#[derive(Clone, Copy, Debug, ValueEnum)]
enum Engine {
    Enumeration,
    Smt,
}
#[derive(Clone, Copy, Debug, Serialize, ValueEnum)]
#[serde(rename_all = "snake_case")]
enum Fixture {
    Prerequisites,
    Mystery,
}
#[derive(Clone, Copy, Debug, Serialize, ValueEnum)]
#[serde(rename_all = "snake_case")]
enum Query {
    Possible,
    Required,
    RuledOut,
    Inconsistent,
}
#[derive(Debug, Parser)]
#[command(
    about = "Compare finite enumeration with optional Z3 classification",
    next_line_help = true,
    max_term_width = 80
)]
struct Args {
    /// Exhaust finite assignments or use the optional Z3 experiment.
    #[arg(long, value_enum, default_value = "enumeration")]
    engine: Engine,
    /// Synthetic prerequisites or the authored study mystery.
    #[arg(long, value_enum, default_value = "prerequisites")]
    fixture: Fixture,
    /// Synthetic Boolean facts (1 through 24); invalid for the mystery fixture.
    #[arg(long)]
    facts: Option<usize>,
    /// Evidence pattern and corresponding hypothesis.
    #[arg(long, value_enum, default_value = "required")]
    query: Query,
    /// Enumeration budget, independent of solver checks.
    #[arg(long, default_value_t = 16_777_216)]
    max_candidates: usize,
    /// Explicit solver executable; run through mise to use the pinned release.
    #[arg(long, default_value = "z3")]
    solver: PathBuf,
    /// Soft timeout for each solver check; excludes startup, I/O and cleanup.
    #[arg(long, default_value_t = 1000)]
    soft_timeout_ms: u32,
}
struct Input {
    model: Model,
    hypothesis: Predicate,
    premises: Vec<Premise>,
}
impl Args {
    fn input(&self) -> Result<Input, Box<dyn Error>> {
        match self.fixture {
            Fixture::Prerequisites => {
                let fixture = Prerequisites::new(self.facts.unwrap_or(12), self.query)?;
                Ok(Input {
                    model: fixture.model,
                    hypothesis: fixture.hypothesis,
                    premises: fixture.premises,
                })
            }
            Fixture::Mystery => {
                if self.facts.is_some() {
                    return Err("--facts applies only to prerequisites".into());
                }
                let evidence = match self.query {
                    Query::Possible => MysteryEvidence::Partial,
                    Query::Required | Query::RuledOut => MysteryEvidence::Complete,
                    Query::Inconsistent => MysteryEvidence::Contradictory,
                };
                let fixture = mystery(evidence)?;
                Ok(Input {
                    model: fixture.model,
                    hypothesis: if matches!(self.query, Query::Required) {
                        fixture.hypothesis.negate()
                    } else {
                        fixture.hypothesis
                    },
                    premises: fixture.premises,
                })
            }
        }
    }
}
#[derive(Serialize)]
#[serde(tag = "engine", content = "report", rename_all = "snake_case")]
enum ResultReport {
    Enumeration(HypothesisReport),
    Smt(Report),
}
#[derive(Serialize)]
struct Measurement {
    fixture: Fixture,
    query: Query,
    authored_facts: usize,
    analysis_seconds: f64,
    #[serde(flatten)]
    result: ResultReport,
}
/// Measure one classification in a fresh process and emit its complete evidence as JSON.
///
/// # Errors
/// Returns invalid fixture/options, solver failures or serialization errors.
pub fn run() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();
    let input = args.input()?;
    let start = Instant::now();
    let result = match args.engine {
        Engine::Enumeration => ResultReport::Enumeration(input.model.classify(
            &input.hypothesis,
            &input.premises,
            ClassificationLimits {
                max_candidates: args.max_candidates,
            },
        )?),
        Engine::Smt => ResultReport::Smt(input.model.classify_smt(
            &input.hypothesis,
            &input.premises,
            &Options::new(
                args.solver,
                Duration::from_millis(u64::from(args.soft_timeout_ms)),
            )?,
        )?),
    };
    let measurement = Measurement {
        fixture: args.fixture,
        query: args.query,
        authored_facts: input.model.facts().len(),
        analysis_seconds: start.elapsed().as_secs_f64(),
        result,
    };
    serde_json::to_writer(io::stdout().lock(), &measurement)?;
    Ok(())
}
