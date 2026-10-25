use super::{Initial, ModelVariant, Scenario};

#[test]
fn real_missing_bindings_are_recorded_with_queries_and_cannot_be_safe_exhaustion() {
    use super::{
        evidence::Outcome,
        exploration::{Conclusion, explore, replay},
        protocol::Limits,
    };
    use leitmotif_recite::Bindings;
    use recite_runtime::{ConditionEvaluationErrorKind, DialogueError};
    let mut scenario = Scenario::new(ModelVariant::Corrected).unwrap();
    scenario.conditions = Bindings::new();
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
    let Conclusion::RuntimeFailed { witness } = report.conclusion else {
        panic!("missing binding must be an error outcome")
    };
    let boundary = witness.last().unwrap();
    assert!(matches!(
        &boundary.outcome,
        Outcome::RuntimeFailed(DialogueError::ConditionEvaluationFailed {
            kind: ConditionEvaluationErrorKind::MissingHandler,
            ..
        })
    ));
    assert_eq!(
        boundary
            .queries
            .first()
            .unwrap()
            .result
            .as_ref()
            .unwrap_err()
            .kind(),
        ConditionEvaluationErrorKind::MissingHandler
    );
    assert_eq!(
        boundary.host.values.get("archive.key"),
        Some(&leitmotif::Value::Bool(true))
    );
    replay(&scenario, initial, &witness).unwrap();
}

#[test]
fn full_boundary_replay_rejects_tampered_host_pending_queue_queries_lines_and_sources() {
    use super::{
        evidence::{ActionSource, Outcome},
        exploration::{Conclusion, explore, replay},
        protocol::Limits,
    };
    use leitmotif::Value;
    enum Component {
        Host,
        Pending,
        Queue,
        Lines,
        Source,
        Queries,
        Event,
    }

    let scenario = Scenario::new(ModelVariant::Faulty).unwrap();
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
    let Conclusion::Violation { witness } = report.conclusion else {
        panic!("expected witness")
    };
    for component in [
        Component::Host,
        Component::Pending,
        Component::Queue,
        Component::Lines,
        Component::Source,
        Component::Queries,
        Component::Event,
    ] {
        let mut changed = witness.clone();
        let last = changed.last_mut().unwrap();
        match component {
            Component::Host => {
                last.host
                    .values
                    .insert("claim.truth".into(), Value::Bool(true));
            }
            Component::Pending => last.pending = None,
            Component::Queue => last.deferred.clear(),
            Component::Lines => last.selected_lines.clear(),
            Component::Source => {
                let ActionSource::Dialogue(span) = &mut last.source else {
                    panic!("expected request source")
                };
                span.file.push_str(".different");
            }
            Component::Queries => {
                let prompt = changed
                    .iter_mut()
                    .find(|boundary| !boundary.queries.is_empty())
                    .unwrap();
                prompt.queries.clear();
            }
            Component::Event => {
                let prompt = changed
                    .iter_mut()
                    .find(|boundary| matches!(boundary.outcome, Outcome::Delivered(_)))
                    .unwrap();
                prompt.outcome = Outcome::Delivered(recite_runtime::DialogueEvent::End {
                    deferred_effects: vec![],
                });
            }
        }
        assert!(replay(&scenario, initial, &changed).is_err());
    }
}

#[test]
fn complete_paths_preserve_claim_settings_and_reached_deferred_order_at_each_boundary() {
    use super::{
        evidence::Outcome,
        exploration::{Conclusion, explore, replay},
        protocol::{Action, Limits, RestoreWindow},
    };
    use leitmotif::Value;
    use recite_runtime::DialogueEvent;
    let scenario = Scenario::new(ModelVariant::Corrected).unwrap();
    let initial = Initial {
        has_key: true,
        truth: false,
        belief: true,
    };
    let limits = Limits {
        max_nodes: 1000,
        max_depth: 32,
    };
    let report = explore(&scenario, initial, limits).unwrap();
    assert_eq!(report, explore(&scenario, initial, limits).unwrap());
    assert_eq!(report.conclusion, Conclusion::Exhaustive);
    let mut denied = false;
    let mut before_restore = false;
    let mut after_restore = false;
    for path in &report.completed_paths {
        replay(&scenario, initial, path).unwrap();
        for boundary in path {
            assert_eq!(
                boundary.host.values.get("claim.truth"),
                Some(&Value::Bool(false))
            );
            assert_eq!(
                boundary.host.values.get("claim.belief"),
                Some(&Value::Bool(true))
            );
            denied |= matches!(boundary.outcome, Outcome::HostDisabled { .. });
            before_restore |= boundary.action == Action::Restore(RestoreWindow::BeforeApplication);
            after_restore |= boundary.action == Action::Restore(RestoreWindow::AfterCommit);
            if let Outcome::Delivered(DialogueEvent::End { deferred_effects }) = &boundary.outcome {
                assert_eq!(deferred_effects.len(), 2);
                assert_eq!(deferred_effects.first().unwrap().function, "record_visit");
                assert_eq!(deferred_effects.get(1).unwrap().function, "record_result");
                assert_eq!(*deferred_effects, boundary.deferred);
                assert_eq!(
                    boundary.host.values.get("archive.visit_recorded"),
                    Some(&Value::Bool(false))
                );
                assert_eq!(
                    boundary.host.values.get("archive.result_recorded"),
                    Some(&Value::Symbol("unrecorded".into()))
                );
            }
        }
        let completed = path.last().unwrap();
        assert_eq!(
            completed.host.values.get("archive.visit_recorded"),
            Some(&Value::Bool(true))
        );
        let access = completed.host.values.get("archive.access").unwrap();
        let expected_line = match access {
            Value::Symbol(value) if value == "closed" => "4cc0a18fca48337278d1",
            Value::Symbol(value) if value == "opened" => "4dff358182d02d9090e3",
            Value::Symbol(value) if value == "denied" => "ebb571c54e9fa639e36f",
            _ => panic!("invalid access state"),
        };
        assert_eq!(
            completed
                .selected_lines
                .iter()
                .map(recite_core::LineId::as_str)
                .collect::<Vec<_>>(),
            ["a90e86884f3aa3bf3ab7", expected_line]
        );
        let Value::Symbol(access) = access else {
            panic!("access is symbolic")
        };
        assert_eq!(
            completed.deferred.get(1).unwrap().args,
            [recite_runtime::DialogueEffectArgument::Identifier(
                access.clone()
            )]
        );
        assert_eq!(
            completed.host.values.get("archive.access"),
            completed.host.values.get("archive.result_recorded")
        );
    }
    assert!(denied && before_restore && after_restore);
}

#[test]
fn direct_host_denial_with_a_present_key_selects_only_denied_output_and_result() {
    use super::{
        evidence::Outcome,
        exploration::{Conclusion, explore},
        protocol::{Action, Limits},
    };
    use leitmotif::Value;
    use recite_runtime::EffectAck;
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
    assert_eq!(report.conclusion, Conclusion::Exhaustive);
    let path = report
        .completed_paths
        .iter()
        .find(|path| {
            path.iter().any(|boundary| {
                boundary.action == Action::Deny
                    && boundary.host.values.get("archive.key") == Some(&Value::Bool(true))
            })
        })
        .unwrap();
    assert!(path.iter().any(|boundary| matches!(
        &boundary.outcome,
        Outcome::Acknowledged {
            acknowledgement: EffectAck::Failed { .. },
            ..
        }
    )));
    let completed = path.last().unwrap();
    assert_eq!(
        completed.host.values.get("archive.key"),
        Some(&Value::Bool(true))
    );
    assert_eq!(
        completed.host.values.get("archive.access"),
        Some(&Value::Symbol("denied".into()))
    );
    assert_eq!(
        completed.host.values.get("archive.result_recorded"),
        Some(&Value::Symbol("denied".into()))
    );
    assert_eq!(
        completed.selected_lines.last().unwrap().as_str(),
        "ebb571c54e9fa639e36f"
    );
}

#[test]
fn restore_after_host_commit_reemits_the_same_request_without_reapplication() {
    use super::{
        evidence::Outcome,
        exploration::explore,
        protocol::{Action, Limits, RestoreWindow},
    };
    use recite_runtime::DialogueEvent;
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
    let path = report
        .completed_paths
        .iter()
        .find(|path| {
            path.iter()
                .any(|boundary| boundary.action == Action::Restore(RestoreWindow::AfterCommit))
        })
        .unwrap();
    let index = path
        .iter()
        .position(|boundary| boundary.action == Action::Restore(RestoreWindow::AfterCommit))
        .unwrap();
    let restored = path.get(index).unwrap();
    let following = path.get(index.saturating_add(1)).unwrap();
    let Outcome::Delivered(DialogueEvent::Effect(reemitted)) = &following.outcome else {
        panic!("restored request must be redelivered")
    };
    assert_eq!(Some(reemitted), restored.pending.as_ref());
    assert!(
        !path
            .iter()
            .skip(index)
            .any(|later| later.action == Action::ApplyUnlock)
    );
    assert_eq!(restored.host, following.host);
}
