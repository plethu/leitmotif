use super::{Initial, ModelVariant, Scenario};

#[test]
fn captured_key_removal_fails_in_runtime_then_disappears_after_guard_correction() {
    use super::{
        exploration::{Conclusion, explore, replay},
        protocol::Limits,
    };
    let limits = Limits {
        max_nodes: 1000,
        max_depth: 32,
    };
    for initial in Initial::all() {
        let faulty = Scenario::new(ModelVariant::Faulty).unwrap();
        let failure = explore(&faulty, initial, limits).unwrap();
        if initial.has_key {
            let Conclusion::Violation { witness } = &failure.conclusion else {
                panic!("expected faulty runtime witness")
            };
            replay(&faulty, initial, witness).unwrap();
            let continued =
                super::exploration::show_opened_continuation(&faulty, initial, witness).unwrap();
            replay(&faulty, initial, &continued).unwrap();
        } else {
            assert_eq!(failure.conclusion, Conclusion::Exhaustive);
        }
        let corrected = Scenario::new(ModelVariant::Corrected).unwrap();
        let report = explore(&corrected, initial, limits).unwrap();
        assert_eq!(report.conclusion, Conclusion::Exhaustive);
        assert_eq!(report.model_id, corrected.model.fingerprint());
        assert_eq!(
            report.initial,
            corrected
                .model
                .snapshot(&corrected.initial(initial).unwrap())
                .unwrap()
        );
        assert_eq!(report.limits, limits);
        assert!(report.protocol.contains("one restore"));
    }
}

#[test]
fn a_depth_limit_after_host_unlock_still_reports_the_already_observed_violation() {
    use super::{
        exploration::{Conclusion, explore},
        protocol::Limits,
    };
    let faulty = Scenario::new(ModelVariant::Faulty).unwrap();
    let report = explore(
        &faulty,
        Initial {
            has_key: true,
            truth: false,
            belief: false,
        },
        Limits {
            max_nodes: 1000,
            max_depth: 6,
        },
    )
    .unwrap();
    assert!(matches!(report.conclusion, Conclusion::Violation { .. }));
}

#[test]
fn measured_corrected_tree_retains_every_history_with_bounded_growth() {
    use super::{exploration::explore, protocol::Limits};
    let scenario = Scenario::new(ModelVariant::Corrected).unwrap();
    let report = explore(
        &scenario,
        Initial {
            has_key: true,
            truth: false,
            belief: true,
        },
        Limits {
            max_nodes: 1000,
            max_depth: 32,
        },
    )
    .unwrap();
    // Measured before choosing budgets: 123 evaluated/constructed nodes,
    // peak frontier 17, and 18 distinct completed histories. No deduplication.
    assert_eq!(
        report.conclusion,
        super::exploration::Conclusion::Exhaustive
    );
    assert_eq!(report.completed_paths.len(), 18);
    assert!(report.evaluated_nodes <= 123);
    assert!(report.generated_nodes <= 123);
    assert!(report.peak_frontier <= 17);
}

#[test]
fn truncation_is_inconclusive_and_preserves_full_initial_assignment() {
    use super::{
        exploration::{Conclusion, explore},
        protocol::Limits,
    };
    let scenario = Scenario::new(ModelVariant::Corrected).unwrap();
    let initial = Initial {
        has_key: true,
        truth: true,
        belief: false,
    };
    for limits in [
        Limits {
            max_nodes: 0,
            max_depth: 32,
        },
        Limits {
            max_nodes: 1,
            max_depth: 32,
        },
        Limits {
            max_nodes: 1000,
            max_depth: 0,
        },
        Limits {
            max_nodes: 1000,
            max_depth: 5,
        },
    ] {
        let report = explore(&scenario, initial, limits).unwrap();
        assert_eq!(report.conclusion, Conclusion::Inconclusive);
        assert_eq!(
            report.initial,
            scenario
                .model
                .snapshot(&scenario.initial(initial).unwrap())
                .unwrap()
        );
        assert_eq!(report.limits, limits);
        assert_eq!(report.property, super::model::PROPERTY);
        assert!(
            include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml"))
                .contains(report.recite_revision)
        );
    }
}

#[test]
fn opened_continuation_rejects_host_denial_and_safe_keyed_unlock_prefixes() {
    use super::{
        evidence::Outcome,
        execution::Execution,
        exploration::{Conclusion, explore, replay, show_opened_continuation},
        facts::Access,
        protocol::{Action, Limits},
    };
    use recite_runtime::{ConditionValue, DialogueEvent};

    let scenario = Scenario::new(ModelVariant::Corrected).unwrap();
    let initial = Initial {
        has_key: true,
        truth: false,
        belief: true,
    };
    let report = explore(
        &scenario,
        initial,
        Limits {
            max_nodes: 1000,
            max_depth: 32,
        },
    )
    .unwrap();
    assert_eq!(report.conclusion, Conclusion::Exhaustive);
    let execution = Execution::new(&scenario).unwrap();
    for (host_action, expected_access) in [
        (Action::Deny, Access::Denied),
        (Action::ApplyUnlock, Access::Opened),
    ] {
        let path = report
            .completed_paths
            .iter()
            .find(|path| {
                path.iter().any(|boundary| boundary.action == host_action)
                    && path.iter().all(|boundary| {
                        !matches!(boundary.action, Action::RemoveKey | Action::Restore(_))
                    })
            })
            .unwrap();
        let last = path
            .iter()
            .position(|boundary| boundary.action == host_action)
            .unwrap();
        let prefix = path.get(..=last).unwrap();
        replay(&scenario, initial, prefix).unwrap();
        let mut node = execution.initial(initial).unwrap();
        for boundary in prefix {
            execution.step(&mut node, &boundary.action).unwrap();
        }
        assert_eq!(scenario.access(&node.host).unwrap(), expected_access);
        assert!(scenario.has_key(&node.host).unwrap());
        assert!(!scenario.violation(&node.host).unwrap());

        // Prove the prefix permits the helper's exact continuation through the real runtime.
        execution.step(&mut node, &Action::Acknowledge).unwrap();
        execution.step(&mut node, &Action::Next).unwrap();
        let delivered = node.trace.last().unwrap();
        assert!(matches!(
            &delivered.outcome,
            Outcome::Delivered(DialogueEvent::Line(_))
        ));
        assert!(delivered.queries.iter().any(|query| {
            query.function == "archive_access"
                && query.result == Ok(ConditionValue::EnumVariant(expected_access.symbol().into()))
        }));
        let error = show_opened_continuation(&scenario, initial, prefix).unwrap_err();
        assert_eq!(
            error.to_string(),
            "runtime continuation did not preserve the witnessed opening violation"
        );
    }
}
