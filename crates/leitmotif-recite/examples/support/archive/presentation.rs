use leitmotif::{Snapshot, Source, Value};
use recite_runtime::DialogueEvent;

use super::{
    Initial,
    evidence::{ActionSource, Boundary, Outcome},
    exploration::{Conclusion, Report},
    protocol::Action,
};

pub(super) fn report_summary(label: &str, reports: &[(Initial, Report)]) -> String {
    let exhaustive = reports
        .iter()
        .filter(|(_, report)| report.conclusion == Conclusion::Exhaustive)
        .count();
    let violations = reports
        .iter()
        .filter(|(_, report)| matches!(report.conclusion, Conclusion::Violation { .. }))
        .count();
    let runtime_errors = reports
        .iter()
        .filter(|(_, report)| matches!(report.conclusion, Conclusion::RuntimeFailed { .. }))
        .count();
    let inconclusive = reports
        .len()
        .saturating_sub(exhaustive)
        .saturating_sub(violations)
        .saturating_sub(runtime_errors);
    let completed = reports
        .iter()
        .map(|(_, report)| report.completed_paths.len())
        .sum::<usize>();
    let nodes = reports
        .iter()
        .map(|(_, report)| report.evaluated_nodes)
        .sum::<usize>();
    format!(
        "{label}: {violations} violations; {exhaustive} exhausted; {inconclusive} inconclusive\n  {runtime_errors} runtime errors; {} initial assignments\n  {nodes} evaluated runtime-tree nodes; {completed} completed paths\n",
        reports.len()
    )
}

pub(super) fn witness_summary(initial: Initial, witness: &[Boundary]) -> String {
    let mut lines = vec![format!(
        "Failing runtime sequence (key={}, truth={}, belief={}):",
        initial.has_key, initial.truth, initial.belief
    )];
    for (index, boundary) in witness.iter().enumerate() {
        lines.push(format!(
            "  {:02} {} -> {}",
            index.saturating_add(1),
            action_name(&boundary.action),
            outcome_name(&boundary.outcome)
        ));
        let location = match &boundary.source {
            ActionSource::Dialogue(span) => format!(
                "{}:{}:{}",
                span.file,
                span.start.line(),
                span.start.column()
            ),
            ActionSource::Host(source) => source_location(source),
        };
        lines.push(format!("     at {location}"));
        if let Outcome::HostApplied(transition) = &boundary.outcome {
            lines.push(format!("     transition {}", transition.id));
            lines.push(format!(
                "       at {}",
                source_location(&transition.provenance)
            ));
        }
        lines.push(format!(
            "     key={} access={} exposed={} visit={} result={}",
            value(&boundary.host, "archive.key"),
            value(&boundary.host, "archive.access"),
            value(&boundary.host, "claim.exposed"),
            value(&boundary.host, "archive.visit_recorded"),
            value(&boundary.host, "archive.result_recorded")
        ));
    }
    lines.push("  Full typed boundaries were compared by replay through the real runtime.".into());
    lines.join("\n")
}

fn source_location(source: &Source) -> String {
    let file = source.portable_file.as_deref().unwrap_or(&source.file);
    format!("{file}:{}:{}", source.line, source.column)
}

fn value(snapshot: &Snapshot, key: &str) -> String {
    match snapshot.values.get(key) {
        Some(Value::Bool(value)) => value.to_string(),
        Some(Value::Symbol(value)) => value.clone(),
        None => "missing".into(),
    }
}

const fn action_name(action: &Action) -> &'static str {
    match action {
        Action::Next => "next",
        Action::ApplyImmediate => "apply immediate",
        Action::Choose(_) => "choose",
        Action::RemoveKey => "remove key",
        Action::ApplyUnlock => "apply unlock",
        Action::Deny => "host denial",
        Action::Restore(_) => "restore",
        Action::Acknowledge => "acknowledge",
        Action::ApplyDeferred(_) => "apply deferred",
    }
}

fn outcome_name(outcome: &Outcome) -> String {
    match outcome {
        Outcome::Delivered(DialogueEvent::Effect(request)) => {
            format!("{:?} {}", request.mode, request.function)
        }
        Outcome::Delivered(DialogueEvent::Prompt { .. }) => "captured prompt".into(),
        Outcome::Delivered(DialogueEvent::Line(line)) => format!("line {}", line.id.as_str()),
        Outcome::Delivered(DialogueEvent::End { .. }) => "End".into(),
        Outcome::RuntimeFailed(error) => format!("runtime error: {error}"),
        Outcome::HostApplied(transition) => transition.id.clone(),
        Outcome::HostDisabled { .. } => "unlock rejected; denial committed".into(),
        Outcome::Restored(_) => "pending request restored".into(),
        Outcome::Acknowledged {
            acknowledgement, ..
        } => format!("{acknowledgement:?}"),
    }
}

pub(super) fn initial_summary(snapshot: &Snapshot) -> String {
    snapshot
        .values
        .iter()
        .map(|(key, value)| {
            let value = match value {
                Value::Bool(value) => value.to_string(),
                Value::Symbol(value) => value.clone(),
            };
            format!("  {key} = {value}")
        })
        .collect::<Vec<_>>()
        .join(
            "
",
        )
}
