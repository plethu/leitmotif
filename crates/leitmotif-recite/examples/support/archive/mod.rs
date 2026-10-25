mod cli;
mod evidence;
mod execution;
mod exploration;
mod export;
mod facts;
mod model;
mod presentation;
mod protocol;
mod source;

use model::{Initial, ModelVariant, Scenario};
use std::error::Error;

/// Runs the original archive-access integration fixture.
///
/// # Errors
/// Returns compilation, host-model or runtime failures from the fixture.
pub fn run() -> Result<(), Box<dyn Error>> {
    let limits = protocol::Limits {
        max_nodes: 1000,
        max_depth: 32,
    };
    let faulty = Scenario::new(ModelVariant::Faulty)?;
    let corrected = Scenario::new(ModelVariant::Corrected)?;
    let summaries = [
        ("Faulty", analyse_all(&faulty, limits)?),
        ("Corrected", analyse_all(&corrected, limits)?),
    ];
    let [(_, faulty_reports), _] = &summaries;
    let [(_, first_report), ..] = faulty_reports;
    let (witness_initial, witness) = required_violation(faulty_reports)?;
    let witness = exploration::show_opened_continuation(&faulty, witness_initial, witness)?;
    exploration::replay(&faulty, witness_initial, &witness)?;
    println!("Archive access: finite runtime integration fixture\n");
    println!("\nEach model checks all key/truth/belief Boolean combinations.\n");
    for (label, reports) in &summaries {
        print!("{}", presentation::report_summary(label, reports));
        let [(_, report), ..] = reports;
        println!("  Model ID:\n  {}\n", report.model_id);
    }

    println!("Property: {}", first_report.property);
    println!(
        "Tool: {}; schema: {}",
        first_report.tool_version, first_report.schema_version
    );
    println!("Recite revision:\n  {}", first_report.recite_revision);
    println!(
        "Limits per initial assignment: {} evaluated nodes; {} actions deep",
        first_report.limits.max_nodes, first_report.limits.max_depth
    );
    println!("Scope: {}", first_report.protocol);
    println!("Host declarations:\n  crates/leitmotif-recite/examples/support/archive/model.rs");
    println!(
        "First complete assignment:\n{}",
        presentation::initial_summary(&first_report.initial)
    );
    println!();
    println!(
        "{}",
        presentation::witness_summary(witness_initial, &witness)
    );
    println!("\nExhaustion covers only the stated finite host protocol, with no deduplication.");
    Ok(())
}

fn analyse_all(
    scenario: &Scenario,
    limits: protocol::Limits,
) -> Result<[(Initial, exploration::Report); 8], Box<dyn Error>> {
    Initial::all()
        .into_iter()
        .map(|initial| {
            exploration::explore(scenario, initial, limits).map(|report| (initial, report))
        })
        .collect::<Result<Vec<_>, _>>()?
        .try_into()
        .map_err(|_| "archive requires exactly eight initial assignments".into())
}

fn required_violation(
    reports: &[(Initial, exploration::Report)],
) -> Result<(Initial, &[evidence::Boundary]), Box<dyn Error>> {
    reports
        .iter()
        .find_map(|(initial, report)| match &report.conclusion {
            exploration::Conclusion::Violation { witness } => Some((*initial, witness.as_slice())),
            exploration::Conclusion::Exhaustive
            | exploration::Conclusion::Inconclusive
            | exploration::Conclusion::RuntimeFailed { .. } => None,
        })
        .ok_or_else(|| "faulty demonstration did not produce its required violation".into())
}

#[cfg(test)]
#[path = "../../../tests/archive/model.rs"]
mod model_tests;

#[cfg(test)]
#[path = "../../../tests/archive/exploration.rs"]
mod exploration_tests;

#[cfg(test)]
#[path = "../../../tests/archive/replay.rs"]
mod replay_tests;

/// Runs the human demonstration or experimental JSON producer.
///
/// # Errors
/// Returns invalid model, source, runtime, or output diagnostics.
pub fn run_cli() -> Result<(), Box<dyn Error>> {
    cli::run()
}
