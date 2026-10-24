mod effect_bindings;
mod evidence;
mod runtime;
mod scenario;
mod source;
#[cfg(test)]
mod tests;

use super::composed_town;
use evidence::{Operation, Outcome};
use leitmotif::SearchLimits;
use recite_core::compiled::encode_compiled_dialogue_messagepack;
use recite_runtime::DialogueEvent;
use runtime::Execution;
use scenario::Scenario;
use std::error::Error;

type RunResult<T> = Result<T, Box<dyn Error>>;

impl Scenario {
    fn new() -> RunResult<Self> {
        Self::from_asset(source::compile(source::TEXT)?)
    }
}

#[derive(Clone, Copy, Debug)]
enum Route {
    Beacon,
    Ferry,
    CapturedBeaconAfterFuel,
}

fn demonstrate(scenario: &Scenario, route: Route) -> RunResult<Execution<'_>> {
    let mut execution = Execution::fresh(scenario)?;
    execution.step(Operation::Next)?;
    execution.step(Operation::Immediate)?;
    execution.step(Operation::Next)?;
    let choice = execution.choice(match route {
        Route::Ferry => "10000000000000000003",
        Route::Beacon | Route::CapturedBeaconAfterFuel => "10000000000000000002",
    })?;
    if matches!(route, Route::CapturedBeaconAfterFuel) {
        execution.step(Operation::FuelFerry)?;
    }
    execution.step(Operation::Choose(choice))?;
    execution.step(Operation::Blocking)?;
    // A request remains pending, including after disabled application, until acknowledgement.
    execution.step(Operation::Next)?;
    execution.step(Operation::Acknowledge)?;
    execution.step(Operation::Next)?;
    if matches!(
        execution.trace.last().map(|boundary| &boundary.outcome),
        Some(Outcome::Delivered(DialogueEvent::Effect(_)))
    ) {
        execution.step(Operation::Immediate)?;
        execution.step(Operation::Next)?;
    }
    execution.step(Operation::Next)?;
    if !execution.ended() {
        return Err("harbour interaction did not finish".into());
    }
    let reached = match route {
        Route::Beacon => scenario
            .town
            .model
            .evaluate(&execution.host, &scenario.town.sheltered)?,
        Route::Ferry => scenario
            .town
            .model
            .evaluate(&execution.host, &scenario.town.departed)?,
        Route::CapturedBeaconAfterFuel => {
            !scenario
                .town
                .model
                .evaluate(&execution.host, &scenario.town.sheltered)?
                && !scenario
                    .town
                    .model
                    .evaluate(&execution.host, &scenario.town.departed)?
        }
    };
    if !reached {
        return Err("harbour runtime reached an unexpected ending".into());
    }
    runtime::replay(scenario, &execution.trace)?;
    Ok(execution)
}

fn check_reference(reference: &composed_town::Town, scenario: &Scenario) -> RunResult<()> {
    let finite = reference.model.reachability(
        &reference.initial,
        &reference.unreachable,
        SearchLimits {
            max_states: 6,
            max_depth: 3,
        },
    )?;
    if reference.model.fingerprint() != scenario.town.model.fingerprint()
        || !finite.coverage.exhausted
    {
        return Err("dialogue changed the existing finite harbour contract".into());
    }
    Ok(())
}

/// Run the bounded harbour runtime interactions and fresh-session replay.
///
/// # Errors
/// Returns compiler, binding, runtime or replay failures.
pub fn run() -> RunResult<()> {
    let scenario = Scenario::new()?;
    let reference = composed_town::town()?;
    check_reference(&reference, &scenario)?;
    println!("Harbour dialogue: real Recite runtime over the existing six-fact model");
    println!("Only three scripted interactions are checked; session state is separate.");
    println!("Model: {}", scenario.town.model.fingerprint());
    for (route, label) in [
        (Route::Beacon, "Beacon: sheltered"),
        (Route::Ferry, "Ferry: departed"),
        (
            Route::CapturedBeaconAfterFuel,
            "Captured beacon after ferry: open; failed acknowledgement",
        ),
    ] {
        let execution = demonstrate(&scenario, route)?;
        let applications = execution
            .trace
            .iter()
            .filter(|boundary| matches!(boundary.outcome, Outcome::Applied(_)))
            .count();
        println!(
            "{label}\n  {} boundaries, {applications} host actions; fresh replay matched",
            execution.trace.len()
        );
        for boundary in &execution.trace {
            if let Outcome::Disabled {
                transition,
                provenance,
            } = &boundary.outcome
            {
                println!(
                    "  {} disabled at {}:{}",
                    transition.id, provenance.file, provenance.line
                );
                println!("  host declaration: {}", transition.provenance);
            }
        }
    }
    Ok(())
}

/// Compile the sole authored harbour dialogue for an engine consumer.
///
/// # Errors
/// Returns compiler diagnostics or official compiled-asset encoding failures.
pub fn compiled_asset() -> RunResult<Vec<u8>> {
    let asset = source::compile(source::TEXT)?;
    Ok(encode_compiled_dialogue_messagepack(&asset)?)
}
