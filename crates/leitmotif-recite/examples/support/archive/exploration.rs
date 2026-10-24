use std::{collections::VecDeque, error::Error};

use leitmotif::Snapshot;
use recite_runtime::DialogueEvent;

use super::{
    Initial, Scenario,
    evidence::{Boundary, Outcome},
    execution::{Execution, Node},
    protocol::{Action, Limits, PROTOCOL, Schedule},
};

#[derive(Clone, Debug, PartialEq)]
pub(super) enum Conclusion {
    Violation { witness: Vec<Boundary> },
    RuntimeFailed { witness: Vec<Boundary> },
    Exhaustive,
    Inconclusive,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct Report {
    pub(super) property: &'static str,
    pub(super) schema_version: &'static str,
    pub(super) tool_version: &'static str,
    pub(super) recite_revision: &'static str,
    pub(super) model_id: String,
    pub(super) initial: Snapshot,
    pub(super) protocol: &'static str,
    pub(super) limits: Limits,
    pub(super) conclusion: Conclusion,
    pub(super) evaluated_nodes: usize,
    pub(super) generated_nodes: usize,
    pub(super) peak_frontier: usize,
    pub(super) completed_paths: Vec<Vec<Boundary>>,
}

pub(super) fn explore(
    scenario: &Scenario,
    initial: Initial,
    limits: Limits,
) -> Result<Report, Box<dyn Error>> {
    let execution = Execution::new(scenario)?;
    let root = execution.initial(initial)?;
    let mut report = Report {
        property: super::model::PROPERTY,
        schema_version: leitmotif::EXPERIMENTAL_SCHEMA_VERSION,
        tool_version: env!("CARGO_PKG_VERSION"),
        recite_revision: "6ddf13bb944dc7b1f0528654a2e70e243e54bb6f",
        model_id: scenario.model.fingerprint().into(),
        initial: scenario.model.snapshot(&root.host)?,
        protocol: PROTOCOL,
        limits,
        conclusion: Conclusion::Inconclusive,
        evaluated_nodes: 0,
        generated_nodes: 1,
        peak_frontier: 1,
        completed_paths: Vec::new(),
    };
    let mut frontier = VecDeque::from([root]);
    let mut truncated = false;
    while let Some(node) = frontier.pop_front() {
        if report.evaluated_nodes >= limits.max_nodes {
            truncated = true;
            break;
        }
        report.evaluated_nodes = report
            .evaluated_nodes
            .checked_add(1)
            .ok_or("node count overflow")?;
        if matches!(node.schedule, Schedule::Failed) {
            report.conclusion = Conclusion::RuntimeFailed {
                witness: node.trace,
            };
            return Ok(report);
        }
        // The invariant applies to every visited host assignment, including the
        // pending boundary before the runtime delivers any following line.
        if scenario.violation(&node.host)? {
            report.conclusion = Conclusion::Violation {
                witness: node.trace,
            };
            return Ok(report);
        }
        if matches!(node.schedule, Schedule::Done) {
            replay(scenario, initial, &node.trace)?;
            report.completed_paths.push(node.trace);
            continue;
        }
        let actions = execution.actions(&node)?;
        if node.trace.len() >= limits.max_depth {
            truncated |= !actions.is_empty();
            continue;
        }
        for action in actions {
            let mut successor = node.clone();
            execution.step(&mut successor, &action)?;
            frontier.push_back(successor);
            report.generated_nodes = report
                .generated_nodes
                .checked_add(1)
                .ok_or("constructed node count overflow")?;
            report.peak_frontier = report.peak_frontier.max(frontier.len());
        }
    }
    if !truncated {
        report.conclusion = Conclusion::Exhaustive;
    }
    Ok(report)
}

fn replay_node(
    scenario: &Scenario,
    initial: Initial,
    trace: &[Boundary],
) -> Result<Node, Box<dyn Error>> {
    let execution = Execution::new(scenario)?;
    let mut node = execution.initial(initial)?;
    for (index, expected) in trace.iter().enumerate() {
        execution.step(&mut node, &expected.action)?;
        if node.trace.last() != Some(expected) {
            return Err(format!("boundary {index} differs during actual runtime replay").into());
        }
    }
    Ok(node)
}

pub(super) fn replay(
    scenario: &Scenario,
    initial: Initial,
    trace: &[Boundary],
) -> Result<(), Box<dyn Error>> {
    replay_node(scenario, initial, trace).map(|_| ())
}

// This separate demonstration extends an already valid violating prefix. It is
// not used to decide the invariant verdict or weaken an exploration limit.
pub(super) fn show_opened_continuation(
    scenario: &Scenario,
    initial: Initial,
    witness: &[Boundary],
) -> Result<Vec<Boundary>, Box<dyn Error>> {
    let execution = Execution::new(scenario)?;
    let mut node = replay_node(scenario, initial, witness)?;
    execution.step(&mut node, &Action::Acknowledge)?;
    execution.step(&mut node, &Action::Next)?;
    let opened = matches!(node.trace.last().map(|boundary| &boundary.outcome), Some(Outcome::Delivered(DialogueEvent::Line(line))) if line.id.as_str() == "4dff358182d02d9090e3");
    if !opened || !scenario.violation(&node.host)? {
        return Err("runtime continuation did not preserve the witnessed opening violation".into());
    }
    Ok(node.trace)
}
