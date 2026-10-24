use super::super::composed_town::{self, Projection};
use super::*;
use evidence::{Origin, Outcome};
use leitmotif::{ReachabilityConclusion, SearchLimits, Value};
use recite_core::LineId;
use recite_runtime::{DialogueEvent, EffectAck};

#[test]
fn compiled_dialogue_uses_the_five_existing_host_actions() {
    let asset = source::compile(source::TEXT).unwrap();
    assert_eq!(asset.effects.len(), 5);
    assert_eq!(asset.choices.len(), 2);
}

#[test]
fn ordinary_routes_reach_both_existing_endings_through_real_runtime() {
    let scenario = Scenario::new().unwrap();
    for (route, target, line) in [
        (
            Route::Beacon,
            &scenario.town.sheltered,
            "10000000000000000005",
        ),
        (
            Route::Ferry,
            &scenario.town.departed,
            "10000000000000000006",
        ),
    ] {
        let execution = demonstrate(&scenario, route).unwrap();
        assert!(execution.ended());
        assert!(
            scenario
                .town
                .model
                .evaluate(&execution.host, target)
                .unwrap()
        );
        let applied: Vec<_> = execution
            .trace
            .iter()
            .filter_map(|boundary| match &boundary.outcome {
                Outcome::Applied(transition) => Some(transition.id.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(applied.len(), 3);
        assert_eq!(applied[0], "keeper.share_warning");
        let final_boundary = execution.trace.last().unwrap();
        assert_eq!(
            final_boundary
                .selected_lines
                .iter()
                .map(LineId::as_str)
                .collect::<Vec<_>>(),
            ["10000000000000000001", line]
        );
        assert!(final_boundary.pending.is_none());
        assert!(
            matches!(&final_boundary.outcome, Outcome::Delivered(DialogueEvent::End { deferred_effects }) if deferred_effects.is_empty())
        );
        assert!(execution.trace.iter().any(|boundary| matches!(
            boundary.outcome,
            Outcome::Acknowledged {
                acknowledgement: EffectAck::Completed,
                ..
            }
        )));
        println!(
            "{route:?}: boundaries={}, queries={}",
            execution.trace.len(),
            execution
                .trace
                .iter()
                .map(|boundary| boundary.queries.len())
                .sum::<usize>()
        );
    }
}

#[test]
fn captured_beacon_choice_rechecks_oil_and_failed_ack_never_shelters() {
    let scenario = Scenario::new().unwrap();
    let execution = demonstrate(&scenario, Route::CapturedBeaconAfterFuel).unwrap();
    assert!(
        !scenario
            .town
            .model
            .evaluate(&execution.host, &scenario.town.sheltered)
            .unwrap()
    );
    assert!(
        !scenario
            .town
            .model
            .evaluate(&execution.host, &scenario.town.departed)
            .unwrap()
    );
    let failed = execution
        .trace
        .iter()
        .position(|boundary| matches!(boundary.outcome, Outcome::Disabled { .. }))
        .unwrap();
    let disabled = &execution.trace[failed];
    let Outcome::Disabled {
        transition,
        provenance,
    } = &disabled.outcome
    else {
        panic!("disabled outcome expected")
    };
    assert_eq!(transition.id, "keeper.light_beacon");
    assert!(
        transition
            .provenance
            .file
            .contains("composed_town/keeper.rs")
    );
    assert_eq!(provenance.file, source::PATH);
    assert!(provenance.line > 0);
    assert_eq!(disabled.host.values["ferryman.ready"], Value::Bool(true));
    assert_eq!(
        disabled.host.values["keeper.beacon_lit"],
        Value::Bool(false)
    );
    assert_eq!(execution.trace[failed - 1].host, disabled.host);
    let request = disabled.pending.as_ref().unwrap();
    let redelivery = &execution.trace[failed + 1];
    assert!(
        matches!(&redelivery.outcome, Outcome::RuntimeFailed(recite_runtime::DialogueError::EffectPending { effect }) if effect == &request.id)
    );
    assert_eq!(redelivery.pending.as_ref(), Some(request));
    assert!(execution.trace.iter().any(|boundary| matches!(&boundary.outcome, Outcome::Acknowledged { request: acknowledged, acknowledgement: EffectAck::Failed { .. } } if acknowledged.id == request.id)));
    let final_boundary = execution.trace.last().unwrap();
    assert_eq!(
        final_boundary
            .selected_lines
            .iter()
            .map(LineId::as_str)
            .collect::<Vec<_>>(),
        ["10000000000000000001", "10000000000000000004"]
    );
    assert!(
        execution
            .trace
            .iter()
            .any(|boundary| matches!(boundary.origin, Origin::Host(_)))
    );
    println!(
        "stale: boundaries={}, queries={}",
        execution.trace.len(),
        execution
            .trace
            .iter()
            .map(|boundary| boundary.queries.len())
            .sum::<usize>()
    );
}

#[test]
fn host_model_fingerprint_and_six_state_five_action_contract_are_unchanged() {
    let scenario = Scenario::new().unwrap();
    let baseline = composed_town::town().unwrap();
    assert_eq!(
        scenario.town.model.fingerprint(),
        baseline.model.fingerprint()
    );
    assert_eq!(
        scenario.town.model.fingerprint(),
        "sha256:f999232901e39df083d73a0f8013f02c20c053c0b0efbafa1f212b9e70ee0b8f"
    );
    assert_eq!(scenario.town.model.facts().len(), 6);
    assert_eq!(scenario.town.model.transitions().count(), 5);
    let report = scenario
        .town
        .model
        .reachability(
            &scenario.town.initial,
            &scenario.town.unreachable,
            SearchLimits {
                max_states: 6,
                max_depth: 3,
            },
        )
        .unwrap();
    assert_eq!(report.conclusion, ReachabilityConclusion::Unreachable);
    assert!(report.coverage.exhausted);
    assert_eq!(report.coverage.evaluated_states, 6);
}

#[test]
fn query_evidence_preserves_exact_arguments_types_and_live_values() {
    use recite_runtime::preview::PreviewConditionArgument;
    let scenario = Scenario::new().unwrap();
    for route in [Route::Beacon, Route::Ferry, Route::CapturedBeaconAfterFuel] {
        let execution = demonstrate(&scenario, route).unwrap();
        for boundary in &execution.trace {
            for query in &boundary.queries {
                let (argument, fact) = match query.function.as_str() {
                    "has_oil" => ("harbour", "harbour.oil_available"),
                    "beacon_lit" => ("keeper", "keeper.beacon_lit"),
                    "ferry_ready" => ("ferryman", "ferryman.ready"),
                    "harbour_ending" => ("harbour", "harbour.ending"),
                    _ => panic!("unexpected query"),
                };
                assert_eq!(
                    query.arguments,
                    [PreviewConditionArgument::Identifier(argument.into())]
                );
                let (kind, expected) = expected_condition(&boundary.host.values[fact]);
                assert_eq!(query.expected, kind);
                assert_eq!(query.result, Ok(expected));
            }
        }
    }
}

#[test]
fn missing_and_wrong_type_bindings_keep_source_linked_errors_and_replay() {
    use leitmotif_recite::{Bindings, ErrorKind};
    use recite_runtime::{ConditionEvaluationErrorKind, DialogueError};
    for wrong_type in [false, true] {
        let (town, conditions) = composed_town::project_with(&Projection::default(), |facts| {
            let mut bindings = Bindings::new();
            if wrong_type {
                bindings
                    .bind_enum(scenario::call("has_oil", "harbour"), &facts.ending)
                    .unwrap();
            }
            bindings
        })
        .unwrap();
        let scenario = Scenario {
            town,
            conditions,
            asset: source::compile(source::TEXT).unwrap(),
        };
        let choice = scenario.asset.choices.first().unwrap();
        let span =
            &scenario.asset.source_maps[usize::try_from(choice.source_map.as_u32()).unwrap()].span;
        let error = scenario
            .conditions
            .guard(choice.availability_requirement.as_ref().unwrap(), span)
            .unwrap_err();
        assert!(matches!(
            (&error.kind, wrong_type),
            (ErrorKind::WrongType(_), true) | (ErrorKind::Unmapped(_), false)
        ));
        assert_eq!(error.provenance.file, source::PATH);
        assert_eq!(error.provenance.line, span.start.line());
        let mut execution = Execution::fresh(&scenario).unwrap();
        for operation in [Operation::Next, Operation::Immediate, Operation::Next] {
            execution.step(operation).unwrap();
        }
        let boundary = execution.trace.last().unwrap();
        let expected_kind = if wrong_type {
            ConditionEvaluationErrorKind::InvalidResult
        } else {
            ConditionEvaluationErrorKind::MissingHandler
        };
        assert!(
            matches!(&boundary.outcome, Outcome::RuntimeFailed(DialogueError::ConditionEvaluationFailed { function, kind, .. }) if function == "has_oil" && *kind == expected_kind)
        );
        assert!(boundary.queries.iter().all(|query| query.result.is_err()));
        assert_eq!(
            boundary.host.values["keeper.warning_shared"],
            Value::Bool(true)
        );
        assert!(
            matches!(&boundary.origin, Origin::Dialogue(span) if span.file == source::PATH && span.start.line() > 0)
        );
        assert!(!execution.ended());
        assert!(execution.step(Operation::Next).is_err());
        runtime::replay(&scenario, &execution.trace).unwrap();
    }
}

#[test]
fn fresh_replay_rejects_changed_host_query_or_selected_line_evidence() {
    let scenario = Scenario::new().unwrap();
    let execution = demonstrate(&scenario, Route::Beacon).unwrap();
    let mut changed = execution.trace.clone();
    changed
        .first_mut()
        .unwrap()
        .host
        .values
        .insert("harbour.oil_available".into(), Value::Bool(false));
    assert!(runtime::replay(&scenario, &changed).is_err());
    let mut changed = execution.trace.clone();
    changed
        .iter_mut()
        .find(|boundary| !boundary.queries.is_empty())
        .unwrap()
        .queries
        .clear();
    assert!(runtime::replay(&scenario, &changed).is_err());
    let mut changed = execution.trace;
    changed.last_mut().unwrap().selected_lines.clear();
    assert!(runtime::replay(&scenario, &changed).is_err());
}

#[test]
fn disabled_protocol_operations_do_not_write_host_state() {
    let scenario = Scenario::new().unwrap();
    let mut execution = Execution::fresh(&scenario).unwrap();
    assert!(execution.choice("unknown").is_err());
    for operation in [
        Operation::Immediate,
        Operation::Blocking,
        Operation::Acknowledge,
        Operation::FuelFerry,
    ] {
        assert!(execution.step(operation).is_err());
        assert_eq!(
            scenario.town.model.snapshot(&execution.host).unwrap(),
            scenario
                .town
                .model
                .snapshot(&scenario.town.initial)
                .unwrap()
        );
        assert!(execution.trace.is_empty());
    }
    for operation in [Operation::Next, Operation::Immediate, Operation::Next] {
        execution.step(operation).unwrap();
    }
    assert!(execution.choice("unknown").is_err());
    assert!(execution.step(Operation::Next).is_err());
    let choice = execution.choice("10000000000000000002").unwrap();
    execution.step(Operation::Choose(choice)).unwrap();
    assert!(execution.step(Operation::Acknowledge).is_err());
    execution.step(Operation::Blocking).unwrap();
    assert!(execution.step(Operation::Blocking).is_err());
}

#[test]
fn fixed_interaction_cost_preserves_both_endings_and_stale_rejection() {
    let scenario = Scenario::new().unwrap();
    for (route, boundaries, applications) in [
        (Route::Beacon, 11, 3),
        (Route::Ferry, 11, 3),
        (Route::CapturedBeaconAfterFuel, 10, 2),
    ] {
        let execution = demonstrate(&scenario, route).unwrap();
        assert!(execution.ended());
        assert_eq!(execution.trace.len(), boundaries);
        assert_eq!(
            execution
                .trace
                .iter()
                .map(|boundary| boundary.queries.len())
                .sum::<usize>(),
            4
        );
        assert_eq!(
            execution
                .trace
                .iter()
                .filter(|boundary| matches!(boundary.outcome, Outcome::Applied(_)))
                .count(),
            applications
        );
        assert!(
            execution
                .trace
                .iter()
                .all(|boundary| boundary.host.values.len() == 6)
        );
    }
}

#[test]
fn actual_compiler_rejects_invalid_harbour_source() {
    assert!(source::compile(":: harbour default\n:if (\n").is_err());
}

fn expected_condition(
    value: &Value,
) -> (
    recite_runtime::ConditionExpectedType,
    recite_runtime::ConditionValue,
) {
    use recite_runtime::{ConditionExpectedType, ConditionValue};
    match value {
        Value::Bool(value) => (ConditionExpectedType::Bool, ConditionValue::Bool(*value)),
        Value::Symbol(value) => (
            ConditionExpectedType::Enum,
            ConditionValue::EnumVariant(value.clone()),
        ),
    }
}

#[test]
fn authored_effect_signatures_and_modes_are_rejected_with_dialogue_source() {
    use leitmotif_recite::{EffectError, EffectErrorKind};
    for (original, replacement, wrong_mode) in [
        ("share_warning(keeper)", "unknown_effect(keeper)", false),
        ("share_warning(keeper)", "share_warning(\"keeper\")", false),
        ("share_warning(keeper)", "share_warning(ferryman)", false),
        (
            "! immediate share_warning",
            "! blocking share_warning",
            true,
        ),
        (
            "! immediate share_warning",
            "! deferred share_warning",
            true,
        ),
    ] {
        let mut scenario = Scenario::new().unwrap();
        scenario.asset = source::compile(&source::TEXT.replace(original, replacement)).unwrap();
        let Err(error) = Execution::fresh(&scenario) else {
            panic!("unsupported effect accepted")
        };
        let error = error.downcast_ref::<EffectError>().unwrap();
        assert!(matches!(
            (&error.kind, wrong_mode),
            (EffectErrorKind::WrongMode { .. }, true) | (EffectErrorKind::Unmapped(_), false)
        ));
        assert_eq!(error.provenance.file, source::PATH);
        assert_eq!(error.provenance.line, 2);
    }
}

#[test]
fn captured_choices_keep_their_actual_availability_while_host_oil_changes() {
    let scenario = Scenario::new().unwrap();
    let execution = demonstrate(&scenario, Route::CapturedBeaconAfterFuel).unwrap();
    let prompt = execution
        .trace
        .iter()
        .find(|boundary| {
            matches!(
                boundary.outcome,
                Outcome::Delivered(DialogueEvent::Prompt { .. })
            )
        })
        .unwrap();
    let Outcome::Delivered(DialogueEvent::Prompt { choices, .. }) = &prompt.outcome else {
        panic!("prompt expected")
    };
    assert!(
        choices
            .iter()
            .all(|choice| choice.availability.is_available)
    );
    assert_eq!(
        prompt.host.values["harbour.oil_available"],
        Value::Bool(true)
    );
    let chosen = execution
        .trace
        .iter()
        .find(|boundary| matches!(boundary.operation, Operation::Choose(_)))
        .unwrap();
    assert_eq!(
        chosen.host.values["harbour.oil_available"],
        Value::Bool(false)
    );
    assert!(chosen.queries.is_empty());
    assert!(
        matches!(&chosen.outcome, Outcome::Delivered(DialogueEvent::Effect(request)) if request.function == "light_beacon")
    );
}

#[test]
fn repeated_pending_traversal_is_explicitly_bounded_without_host_mutation() {
    let scenario = Scenario::new().unwrap();
    let mut execution = Execution::fresh(&scenario).unwrap();
    for operation in [Operation::Next, Operation::Immediate, Operation::Next] {
        execution.step(operation).unwrap();
    }
    let choice = execution.choice("10000000000000000002").unwrap();
    execution.step(Operation::Choose(choice)).unwrap();
    let unchanged = scenario.town.model.snapshot(&execution.host).unwrap();
    for _ in execution.trace.len()..24 {
        execution.step(Operation::Next).unwrap();
    }
    assert_eq!(execution.trace.len(), 24);
    assert!(execution.step(Operation::Next).is_err());
    assert_eq!(
        scenario.town.model.snapshot(&execution.host).unwrap(),
        unchanged
    );
}

#[test]
fn human_demonstration_checks_actual_runtime_paths_and_finite_reference() {
    run().unwrap();
}

#[test]
fn authored_continuation_changes_cannot_pass_the_scripted_route_contract() {
    for (text, route, expected) in [
        (
            source::TEXT.replace(":if beacon_lit(keeper)", ":if ferry_ready(ferryman)"),
            Route::Beacon,
            "harbour runtime reached an unexpected ending",
        ),
        (
            source::TEXT
                .replace(
                    ":if ferry_ready(ferryman)\n  ! immediate depart(harbour)\n",
                    "",
                )
                .replace(":if beacon_lit(keeper)", ":if ferry_ready(ferryman)")
                .replace(
                    "! immediate shelter(harbour)",
                    "! immediate depart(harbour)",
                ),
            Route::CapturedBeaconAfterFuel,
            "harbour runtime reached an unexpected ending",
        ),
        (
            source::TEXT.replace(
                "-> END\n",
                "> extra@10000000000000000007\n  Another harbour report.\n-> END\n",
            ),
            Route::Beacon,
            "harbour interaction did not finish",
        ),
    ] {
        let mut scenario = Scenario::new().unwrap();
        scenario.asset = source::compile(&text).unwrap();
        let Err(error) = demonstrate(&scenario, route) else {
            panic!("changed continuation passed the original route contract")
        };
        assert_eq!(error.to_string(), expected);
    }
}

#[test]
fn unknown_choice_is_rejected_before_runtime_or_host_mutation() {
    let scenario = Scenario::new().unwrap();
    let mut execution = Execution::fresh(&scenario).unwrap();
    for operation in [Operation::Next, Operation::Immediate, Operation::Next] {
        execution.step(operation).unwrap();
    }
    let before = execution.trace.clone();
    let host = scenario.town.model.snapshot(&execution.host).unwrap();
    let unknown = "10000000000000000099";
    assert_eq!(
        execution.choice(unknown).unwrap_err().to_string(),
        "requested harbour choice is unavailable"
    );
    let error = execution
        .step(Operation::Choose(
            recite_core::ChoiceId::new(unknown).unwrap(),
        ))
        .unwrap_err();
    assert_eq!(error.to_string(), "choice source missing");
    assert_eq!(execution.trace, before);
    assert_eq!(scenario.town.model.snapshot(&execution.host).unwrap(), host);
    execution
        .step(Operation::Choose(
            execution.choice("10000000000000000002").unwrap(),
        ))
        .unwrap();
    assert!(matches!(
        execution.trace.last().unwrap().outcome,
        Outcome::Delivered(DialogueEvent::Effect(_))
    ));
}

#[test]
fn finite_reference_rejects_different_model_identity_and_incomplete_exploration() {
    use leitmotif::{Fact, ModelBuilder};

    struct OtherSignal;
    impl Fact for OtherSignal {
        type Value = bool;
        const KEY: &'static str = "other.signal";
    }
    let scenario = Scenario::new().unwrap();
    let mut builder = ModelBuilder::new("other_harbour").unwrap();
    let signal = builder.fact::<OtherSignal>().unwrap();
    let model = builder.build().unwrap();
    let different = composed_town::Town {
        initial: model.state([signal.set(false)]).unwrap(),
        model,
        sheltered: signal.is(true),
        departed: signal.is(true),
        unreachable: signal.is(true),
    };
    let finite = different
        .model
        .reachability(
            &different.initial,
            &different.unreachable,
            SearchLimits {
                max_states: 6,
                max_depth: 3,
            },
        )
        .unwrap();
    assert!(finite.coverage.exhausted);
    assert_ne!(
        different.model.fingerprint(),
        scenario.town.model.fingerprint()
    );
    assert_eq!(
        check_reference(&different, &scenario)
            .unwrap_err()
            .to_string(),
        "dialogue changed the existing finite harbour contract"
    );

    let mut reached = composed_town::town().unwrap();
    reached.unreachable = reached.sheltered.clone();
    assert_eq!(
        reached.model.fingerprint(),
        scenario.town.model.fingerprint()
    );
    let finite = reached
        .model
        .reachability(
            &reached.initial,
            &reached.unreachable,
            SearchLimits {
                max_states: 6,
                max_depth: 3,
            },
        )
        .unwrap();
    assert!(matches!(
        finite.conclusion,
        ReachabilityConclusion::Reached { .. }
    ));
    assert!(!finite.coverage.exhausted);
    assert_eq!(
        check_reference(&reached, &scenario)
            .unwrap_err()
            .to_string(),
        "dialogue changed the existing finite harbour contract"
    );
    check_reference(&composed_town::town().unwrap(), &scenario).unwrap();
}
