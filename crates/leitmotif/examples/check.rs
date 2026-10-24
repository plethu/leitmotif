//! A headless consumer of Rust-authored fixtures, not a model-file loader.

#[path = "support/mystery.rs"]
mod mystery;
#[path = "support/town.rs"]
mod town;

use std::{
    error::Error,
    io::{self, Write},
    process::ExitCode,
};

use clap::{Parser, ValueEnum};
use leitmotif::{
    ClassificationLimits, HypothesisConclusion, HypothesisReport, InvariantConclusion,
    InvariantReport, ReachabilityConclusion, ReachabilityReport, SearchLimits, Trace,
};
use serde::Serialize;

pub use mystery::{Mystery, MysteryEvidence, mystery};
pub use town::{Town, TownVariant, town};

#[derive(Clone, Copy, Debug, ValueEnum)]
enum Scenario {
    Town,
    BlockedEnding,
    Disclosure,
    Mystery,
    RevealedMystery,
    Contradiction,
}

#[derive(Debug, Parser)]
#[command(about = "Check small Rust-authored Leitmotif fixtures (experimental)")]
struct Args {
    #[arg(value_enum, default_value = "town")]
    scenario: Scenario,
    /// Emit an experimental structured report instead of human diagnostics.
    #[arg(long)]
    json: bool,
    /// Maximum distinct admitted states, including the initial state.
    #[arg(long, default_value_t = 1024)]
    max_states: usize,
    /// Maximum transitions from the initial state.
    #[arg(long, default_value_t = 32)]
    max_depth: usize,
    /// Maximum complete assignments examined by the mystery classifier.
    #[arg(long, default_value_t = 1024)]
    max_candidates: usize,
}

#[derive(Serialize)]
#[serde(tag = "analysis", content = "report", rename_all = "snake_case")]
enum Report {
    Town(Box<TownReport>),
    Safety(InvariantReport),
    Hypothesis(HypothesisReport),
}

#[derive(Serialize)]
struct TownReport {
    ending_a: ReachabilityReport,
    ending_d: ReachabilityReport,
    safety: InvariantReport,
    gate_feasibility: HypothesisReport,
}

fn analyze(args: &Args) -> Result<Report, leitmotif::Error> {
    let limits = SearchLimits {
        max_states: args.max_states,
        max_depth: args.max_depth,
    };
    let candidates = ClassificationLimits {
        max_candidates: args.max_candidates,
    };
    match args.scenario {
        Scenario::Town | Scenario::BlockedEnding => {
            let variant = match args.scenario {
                Scenario::BlockedEnding => TownVariant::EvidenceClosesCouncil,
                _ => TownVariant::Valid,
            };
            let fixture = town(variant)?;
            Ok(Report::Town(Box::new(TownReport {
                ending_a: fixture.model.reachability(
                    &fixture.initial,
                    &fixture.ending_a,
                    limits,
                )?,
                ending_d: fixture.model.reachability(
                    &fixture.initial,
                    &fixture.ending_d,
                    limits,
                )?,
                safety: fixture.model.check_invariants(&fixture.initial, limits)?,
                gate_feasibility: fixture
                    .model
                    .classify(&fixture.ending_gate, &[], candidates)?,
            })))
        }
        Scenario::Disclosure => {
            let fixture = town(TownVariant::UnguardedDisclosure)?;
            Ok(Report::Safety(
                fixture.model.check_invariants(&fixture.initial, limits)?,
            ))
        }
        Scenario::Mystery | Scenario::RevealedMystery | Scenario::Contradiction => {
            let evidence = match args.scenario {
                Scenario::RevealedMystery => MysteryEvidence::Complete,
                Scenario::Contradiction => MysteryEvidence::Contradictory,
                _ => MysteryEvidence::Partial,
            };
            let fixture = mystery(evidence)?;
            Ok(Report::Hypothesis(fixture.model.classify(
                &fixture.hypothesis,
                &fixture.premises,
                candidates,
            )?))
        }
    }
}

fn write_trace(output: &mut impl Write, trace: &Trace) -> io::Result<()> {
    for step in &trace.steps {
        writeln!(
            output,
            "  -> {} at {}",
            step.transition.id, step.transition.provenance
        )?;
    }
    Ok(())
}

fn write_reachability(
    output: &mut impl Write,
    label: &str,
    report: &ReachabilityReport,
) -> io::Result<()> {
    let conclusion = match report.conclusion {
        ReachabilityConclusion::Reached { .. } => "reached",
        ReachabilityConclusion::Unreachable => "unreachable in this finite model",
        ReachabilityConclusion::Inconclusive => "inconclusive: unexplored states remain",
    };
    writeln!(
        output,
        "{label}: {conclusion} ({} states evaluated)",
        report.coverage.evaluated_states
    )?;
    if let ReachabilityConclusion::Reached { witness } = &report.conclusion {
        write_trace(output, witness)?;
    }
    Ok(())
}

fn write_safety(output: &mut impl Write, report: &InvariantReport) -> io::Result<()> {
    match &report.conclusion {
        InvariantConclusion::Holds => writeln!(
            output,
            "Invariants: hold across all reachable modeled states"
        ),
        InvariantConclusion::Inconclusive => writeln!(
            output,
            "Invariants: inconclusive; no violation found within limits"
        ),
        InvariantConclusion::Violation { invariant, witness } => {
            writeln!(
                output,
                "Violation: {} at {}",
                invariant.id, invariant.provenance
            )?;
            write_trace(output, witness)
        }
    }
}

fn write_hypothesis(
    output: &mut impl Write,
    label: &str,
    report: &HypothesisReport,
) -> io::Result<()> {
    writeln!(
        output,
        "{label}: {:?} ({} admitted / {} examined; complete: {})",
        report.conclusion,
        report.coverage.satisfying_candidates,
        report.coverage.evaluated_candidates,
        report.coverage.exhausted
    )?;
    if report.conclusion == HypothesisConclusion::Inconsistent {
        writeln!(
            output,
            "  No assignment satisfies the supplied premises together:"
        )?;
        for premise in &report.premises {
            writeln!(output, "  {} at {}", premise.id, premise.provenance)?;
        }
        writeln!(
            output,
            "  This is the full premise set, not a minimized conflict core."
        )?;
    }
    if let Some(witness) = &report.witness {
        writeln!(output, "  Supporting assignment: {:?}", witness.values)?;
    }
    if let Some(counterexample) = &report.counterexample {
        writeln!(
            output,
            "  Counterexample assignment: {:?}",
            counterexample.values
        )?;
    }
    Ok(())
}

fn render(output: &mut impl Write, report: &Report) -> io::Result<()> {
    writeln!(output, "Leitmotif — experimental finite-model analysis")?;
    match report {
        Report::Town(report) => {
            let TownReport {
                ending_a,
                ending_d,
                safety,
                gate_feasibility,
            } = report.as_ref();
            write_reachability(output, "Ending A", ending_a)?;
            write_reachability(output, "Ending D", ending_d)?;
            write_safety(output, safety)?;
            write_hypothesis(
                output,
                "Ending A gate (static feasibility only)",
                gate_feasibility,
            )
        }
        Report::Safety(report) => write_safety(output, report),
        Report::Hypothesis(report) => write_hypothesis(output, "Bo entered the study", report),
    }
}

const fn reach_status(report: &ReachabilityReport) -> u8 {
    match report.conclusion {
        ReachabilityConclusion::Reached { .. } => 0,
        ReachabilityConclusion::Unreachable => 1,
        ReachabilityConclusion::Inconclusive => 2,
    }
}

const fn safety_status(report: &InvariantReport) -> u8 {
    match report.conclusion {
        InvariantConclusion::Holds => 0,
        InvariantConclusion::Violation { .. } => 1,
        InvariantConclusion::Inconclusive => 2,
    }
}

const fn hypothesis_status(report: &HypothesisReport) -> u8 {
    match report.conclusion {
        HypothesisConclusion::Possible
        | HypothesisConclusion::Required
        | HypothesisConclusion::RuledOut => 0,
        HypothesisConclusion::Inconsistent => 1,
        HypothesisConclusion::Inconclusive => 2,
    }
}

fn status(report: &Report) -> u8 {
    match report {
        Report::Town(report) => {
            let TownReport {
                ending_a,
                ending_d,
                safety,
                gate_feasibility,
            } = report.as_ref();
            reach_status(ending_a)
                .max(reach_status(ending_d))
                .max(safety_status(safety))
                .max(hypothesis_status(gate_feasibility))
        }
        Report::Safety(report) => safety_status(report),
        Report::Hypothesis(report) => hypothesis_status(report),
    }
}

fn main() -> Result<ExitCode, Box<dyn Error>> {
    let args = Args::parse();
    let report = analyze(&args)?;
    let mut output = io::stdout().lock();
    if args.json {
        serde_json::to_writer_pretty(&mut output, &report)?;
        writeln!(output)?;
    } else {
        render(&mut output, &report)?;
    }
    Ok(ExitCode::from(status(&report)))
}
