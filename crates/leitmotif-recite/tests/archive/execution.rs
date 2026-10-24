use super::*;
use crate::archive::{
    evidence::{Outcome, QueryObservation},
    facts::Access,
    model::ModelVariant,
    source,
};
use leitmotif_recite::{Bindings, EffectError, EffectErrorKind};
use recite_core::compiled::CompiledArgument;
use recite_runtime::{ConditionExpectedType, ConditionValue, DialogueEvent};

fn prompt(execution: &Execution<'_>, initial: Initial) -> RunResult<Node> {
    let mut node = execution.initial(initial)?;
    for action in [Action::Next, Action::ApplyImmediate, Action::Next] {
        execution.step(&mut node, &action)?;
    }
    Ok(node)
}

#[test]
fn invalid_actions_and_unmapped_effects_are_not_recovered_as_host_denial() {
    let scenario = Scenario::new(ModelVariant::Corrected).unwrap();
    let mut execution = Execution::new(&scenario).unwrap();
    let initial = Initial {
        has_key: true,
        truth: false,
        belief: true,
    };
    let mut node = execution.initial(initial).unwrap();
    let original = scenario.model.snapshot(&node.host).unwrap();
    assert!(execution.step(&mut node, &Action::ApplyUnlock).is_err());
    assert_eq!(scenario.model.snapshot(&node.host).unwrap(), original);
    assert!(node.trace.is_empty());
    node = prompt(&execution, initial).unwrap();
    let choice = execution
        .actions(&node)
        .unwrap()
        .into_iter()
        .find(
            |action| matches!(action, Action::Choose(id) if id.as_str() == "baa4233d710cc23fd277"),
        )
        .unwrap();
    execution.step(&mut node, &choice).unwrap();
    execution.blocking = BlockingEffects::new(&scenario.model);
    let before = snapshot_session(&node.session);
    let error = execution.step(&mut node, &Action::ApplyUnlock).unwrap_err();
    assert!(matches!(
        error.downcast_ref::<EffectError>().unwrap().kind,
        EffectErrorKind::Unmapped(_)
    ));
    assert_eq!(snapshot_session(&node.session), before);
    assert_eq!(scenario.access(&node.host).unwrap(), Access::Closed);
}

#[test]
fn query_conformance_rejects_wrong_names_arguments_expected_types_values_and_fail_open() {
    let scenario = Scenario::new(ModelVariant::Corrected).unwrap();
    let execution = Execution::new(&scenario).unwrap();
    let initial = Initial {
        has_key: true,
        truth: false,
        belief: true,
    };
    let node = prompt(&execution, initial).unwrap();
    let boundary = node.trace.last().unwrap();
    let query = boundary.queries.first().unwrap();
    let mut name = query.clone();
    name.function = "unknown".into();
    let mut args = query.clone();
    args.arguments = vec![CompiledArgument::Identifier("elsewhere".into())];
    let mut expected = query.clone();
    expected.expected = ConditionExpectedType::Enum;
    let mut result = query.clone();
    result.result = Ok(ConditionValue::Bool(false));
    for changed in [name, args, expected, result] {
        assert!(
            execution
                .check_queries(&node.host, &[changed], &boundary.outcome)
                .is_err()
        );
    }
    let mut unmapped = Scenario::new(ModelVariant::Corrected).unwrap();
    unmapped.conditions = Bindings::new();
    let unmapped_execution = Execution::new(&unmapped).unwrap();
    let failed = prompt(&unmapped_execution, initial).unwrap();
    let errors: Vec<QueryObservation> = failed.trace.last().unwrap().queries.clone();
    assert!(
        unmapped_execution
            .check_queries(&failed.host, &errors, &boundary.outcome)
            .is_err()
    );
}

#[test]
fn real_empty_end_and_corrupted_end_delivery_have_explicit_conformance_outcomes() {
    let mut scenario = Scenario::new(ModelVariant::Corrected).unwrap();
    scenario.asset = source::compile(":: archive default\n-> END\n").unwrap();
    let execution = Execution::new(&scenario).unwrap();
    let mut node = execution
        .initial(Initial {
            has_key: false,
            truth: false,
            belief: false,
        })
        .unwrap();
    execution.step(&mut node, &Action::Next).unwrap();
    assert!(matches!(node.schedule, Schedule::Done));
    assert!(matches!(
        node.trace.last().unwrap().outcome,
        Outcome::Delivered(DialogueEvent::End { .. })
    ));
    let scenario = Scenario::new(ModelVariant::Corrected).unwrap();
    let execution = Execution::new(&scenario).unwrap();
    let mut node = prompt(
        &execution,
        Initial {
            has_key: false,
            truth: false,
            belief: false,
        },
    )
    .unwrap();
    let leave = execution
        .actions(&node)
        .unwrap()
        .into_iter()
        .find(|action| matches!(action, Action::Choose(_)))
        .unwrap();
    execution.step(&mut node, &leave).unwrap();
    execution.step(&mut node, &Action::Next).unwrap();
    let bad = DialogueEvent::End {
        deferred_effects: vec![],
    };
    assert!(Execution::accept(&mut node, &bad, &RuntimeOperation::Next).is_err());
}

#[test]
fn malformed_authored_source_is_rejected_by_the_real_compiler() {
    let error = source::compile(":: archive default\n:if (\n  -> END\n").unwrap_err();
    assert!(error.to_string().contains("archive source diagnostics"));
    assert!(error.to_string().contains("archive.recite"));
}
