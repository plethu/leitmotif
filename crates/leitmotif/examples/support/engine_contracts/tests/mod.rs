use super::host::{Action, Host};

#[test]
fn host_rejects_unenabled_actions_without_mutation() {
    let mut host = Host::initial();
    let initial = host.snapshot().clone();
    for action in [
        Action::LightBeacon,
        Action::FuelFerry,
        Action::Shelter,
        Action::Depart,
    ] {
        assert!(!host.apply(action));
        assert_eq!(host.snapshot(), &initial);
    }
}

use super::{
    conformance::Replay,
    contract::Contract,
    error::Error,
    host::{Ending, HostSnapshot},
    input::{Limits, Query},
};
use leitmotif::{ReachabilityConclusion, Value};
use serde_json::json;
use std::{
    collections::BTreeSet,
    path::{Component, Path},
};

fn contract() -> Contract {
    Contract::new("reviewed-source-a".into()).unwrap()
}

#[test]
fn every_action_at_every_reachable_state_has_independent_host_agreement() {
    let contract = contract();
    let host = Host::initial();
    assert_eq!(
        host.snapshot().snapshot(),
        contract
            .town
            .model
            .snapshot(&contract.town.initial)
            .unwrap()
    );
    let request = contract.request(host.snapshot().clone(), Query::Sheltered, Limits::COMPLETE);
    let audit = contract.audit(&request, &host).unwrap();
    // Measured structural budget: six states, five declared actions per state.
    assert_eq!(audit.states, 6);
    assert_eq!(audit.cases.len(), 30);
    assert_eq!(
        audit
            .cases
            .iter()
            .filter(|case| case.expected_enabled)
            .count(),
        5
    );
    assert_eq!(
        audit
            .cases
            .iter()
            .filter(|case| !case.expected_enabled)
            .count(),
        25
    );
    let terminal = audit
        .cases
        .iter()
        .filter(|case| case.prefix.len() == 3)
        .collect::<Vec<_>>();
    assert_eq!(terminal.len(), 10);
    assert!(
        terminal
            .iter()
            .all(|case| !case.expected_enabled && case.before == case.expected)
    );
    for case in &audit.cases {
        let mut actual = Host::initial();
        for id in &case.prefix {
            assert!(actual.apply(Action::from_id(id).unwrap()));
        }
        assert_eq!(actual.snapshot().snapshot(), case.before);
        assert_eq!(
            actual.apply(Action::from_id(case.action).unwrap()),
            case.expected_enabled
        );
        assert_eq!(actual.snapshot().snapshot(), case.expected);
    }
    assert_eq!(host.snapshot(), &HostSnapshot::initial());
}

#[test]
fn both_three_action_endings_replay_and_match_offline_analysis() {
    let contract = contract();
    for (query, ending) in [
        (Query::Sheltered, Ending::Sheltered),
        (Query::Departed, Ending::Departed),
    ] {
        let request = contract.request(HostSnapshot::initial(), query, Limits::COMPLETE);
        let direct = contract.analyse(&request).unwrap();
        let wire = serde_json::to_string(&request).unwrap();
        let decoded = contract.decode(&wire).unwrap();
        let offline = contract.analyse(&decoded).unwrap();
        assert_eq!(direct.report, offline.report);
        let ReachabilityConclusion::Reached { witness } = &direct.report.conclusion else {
            panic!("ending must be reachable")
        };
        assert_eq!(witness.steps.len(), 3);
        let mut host = Host::initial();
        let replay = contract.replay(&decoded, &offline, &mut host).unwrap();
        assert!(
            matches!(replay, Replay::Reached { actions: 3, state } if state == host.snapshot().snapshot())
        );
        assert_eq!(host.snapshot().ending, ending);
        assert!(!host.snapshot().oil.is_available());
        assert!(!host.snapshot().storm);
        for action in Action::ALL {
            let before = host.snapshot().clone();
            assert!(!host.apply(action));
            assert_eq!(host.snapshot(), &before);
        }
    }
}

#[test]
fn full_reports_keep_model_and_source_identities_separate_and_resolvable() {
    let first = contract();
    let second = Contract::new("reviewed-source-b".into()).unwrap();
    assert_eq!(
        first.town.model.fingerprint(),
        second.town.model.fingerprint()
    );
    let mut sources = BTreeSet::new();
    for contract in [first, second] {
        let request = contract.request(HostSnapshot::initial(), Query::Sheltered, Limits::COMPLETE);
        let report = contract.analyse(&request).unwrap();
        assert_eq!(report.source_revision, request.source_revision);
        assert_eq!(report.report.initial.values.len(), 6);
        assert_eq!(
            report.report.initial.values.get("harbour.storm_passed"),
            Some(&Value::Bool(false))
        );
        let ReachabilityConclusion::Reached { witness } = report.report.conclusion else {
            panic!("expected trace")
        };
        for step in witness.steps {
            let path = step.transition.provenance.portable_file.unwrap();
            assert!(!Path::new(&path).is_absolute());
            assert!(
                Path::new(&path)
                    .components()
                    .all(|component| !matches!(component, Component::ParentDir))
            );
            assert!(path.contains("examples/support/composed_town/"));
            assert!(step.transition.provenance.line > 0);
            sources.insert(path);
        }
    }
    assert_eq!(sources.len(), 2);
}

#[test]
fn malformed_or_incomplete_projections_never_change_live_host() {
    let contract = contract();
    let request = contract.request(HostSnapshot::initial(), Query::Sheltered, Limits::COMPLETE);
    let base = serde_json::to_value(&request).unwrap();
    let mut variants = Vec::new();
    for key in [
        "harbour.ending",
        "harbour.oil_available",
        "harbour.storm_passed",
        "keeper.warning_shared",
        "keeper.beacon_lit",
        "ferryman.ready",
    ] {
        let mut value = base.clone();
        value["initial"].as_object_mut().unwrap().remove(key);
        variants.push(value);
    }
    for (key, value) in [
        ("harbour.ending", json!("secret")),
        ("harbour.ending", json!(true)),
        ("harbour.oil_available", json!("true")),
        ("harbour.storm_passed", json!(0)),
        ("extra", json!(false)),
    ] {
        let mut mutated = base.clone();
        mutated["initial"][key] = value;
        variants.push(mutated);
    }
    for key in [
        "schema_version",
        "model_id",
        "binding_version",
        "source_revision",
    ] {
        let mut value = base.clone();
        value[key] = json!("unrecognised");
        variants.push(value);
    }
    for value in [json!(-1), json!(1.5), json!(true)] {
        let mut mutated = base.clone();
        mutated["limits"]["max_states"] = value;
        variants.push(mutated);
    }
    let mut behavior = base;
    behavior["lighting_fault"] = json!(true);
    variants.push(behavior);
    let host = Host::initial();
    for value in variants {
        assert!(matches!(
            contract.decode(&serde_json::to_string(&value).unwrap()),
            Err(Error::RejectedInput(_))
        ));
        assert_eq!(host.snapshot(), &HostSnapshot::initial());
    }
    let duplicate = serde_json::to_string(&request).unwrap().replace(
        "\"harbour.oil_available\":true",
        "\"harbour.oil_available\":true,\"harbour.oil_available\":false",
    );
    assert!(matches!(
        contract.decode(&duplicate),
        Err(Error::RejectedInput(_))
    ));
    assert!(matches!(
        contract.decode("not JSON"),
        Err(Error::RejectedInput(_))
    ));
    assert!(matches!(
        Contract::new("  ".into()),
        Err(Error::RejectedInput(_))
    ));
}

#[test]
fn unsupported_queries_and_trace_actions_are_explicit_before_replay_mutation() {
    let contract = contract();
    let mut request = contract.request(HostSnapshot::initial(), Query::Sheltered, Limits::COMPLETE);
    request.query = "arbitrary_script".into();
    assert!(matches!(
        contract.analyse(&request),
        Err(Error::Unsupported(_))
    ));
    request.query = "sheltered".into();
    let mut report = contract.analyse(&request).unwrap();
    let ReachabilityConclusion::Reached { witness } = &mut report.report.conclusion else {
        panic!("expected trace")
    };
    witness.steps.get_mut(1).unwrap().transition.id = "engine.execute_unknown_behavior".into();
    let mut host = Host::initial();
    assert!(matches!(
        contract.replay(&request, &report, &mut host),
        Err(Error::Unsupported(_))
    ));
    assert_eq!(host.snapshot(), &HostSnapshot::initial());
}

#[test]
fn captured_initial_and_report_identity_are_rechecked_before_live_mutation() {
    let contract = contract();
    let request = contract.request(HostSnapshot::initial(), Query::Sheltered, Limits::COMPLETE);
    let analysis = contract.analyse(&request).unwrap();
    let mut host = Host::initial();
    assert!(host.apply(Action::ShareWarning));
    let current = host.snapshot().clone();
    assert!(matches!(
        contract.replay(&request, &analysis, &mut host),
        Err(Error::RejectedInput(_))
    ));
    assert_eq!(host.snapshot(), &current);
    assert!(matches!(
        contract.audit(&request, &host),
        Err(Error::RejectedInput(_))
    ));
    let mut host = Host::initial();
    let mut foreign = contract.analyse(&request).unwrap();
    foreign.source_revision = "different-source".into();
    assert!(matches!(
        contract.replay(&request, &foreign, &mut host),
        Err(Error::RejectedInput(_))
    ));
    assert_eq!(host.snapshot(), &HostSnapshot::initial());
}

#[test]
fn divergent_host_effect_is_not_overwritten_with_the_model_successor() {
    let contract = contract();
    let request = contract.request(HostSnapshot::initial(), Query::Sheltered, Limits::COMPLETE);
    let host = Host::with_lighting_fault(HostSnapshot::initial());
    let Error::ConformanceFailure(failure) = contract.audit(&request, &host).unwrap_err() else {
        panic!("expected divergence")
    };
    assert_eq!(failure.action, "keeper.light_beacon");
    assert_eq!(
        failure.expected.values.get("harbour.oil_available"),
        Some(&Value::Bool(false))
    );
    assert_eq!(
        failure.actual.values.get("harbour.oil_available"),
        Some(&Value::Bool(true))
    );
    assert_eq!(host.snapshot(), &HostSnapshot::initial());
    let analysis = contract.analyse(&request).unwrap();
    let mut live = host;
    let Error::ConformanceFailure(failure) =
        contract.replay(&request, &analysis, &mut live).unwrap_err()
    else {
        panic!("expected live divergence")
    };
    assert_eq!(failure.verified_prefix_actions, 1);
    assert_eq!(failure.accepted_host_actions, 2);
    assert!(
        live.snapshot().beacon.is_lit()
            && live.snapshot().oil.is_available()
            && live.snapshot().warning
    );
    assert_eq!(failure.actual, live.snapshot().snapshot());
}

#[test]
fn truncated_analysis_and_unreachable_query_are_distinct_without_mutation() {
    let contract = contract();
    for limits in [
        Limits {
            max_states: 0,
            max_depth: 3,
        },
        Limits {
            max_states: 2,
            max_depth: 3,
        },
        Limits {
            max_states: 6,
            max_depth: 1,
        },
    ] {
        let request = contract.request(HostSnapshot::initial(), Query::Sheltered, limits);
        let report = contract.analyse(&request).unwrap();
        assert_eq!(
            report.report.conclusion,
            ReachabilityConclusion::Inconclusive
        );
        let mut host = Host::initial();
        assert!(matches!(
            contract.replay(&request, &report, &mut host).unwrap(),
            Replay::Inconclusive
        ));
        assert_eq!(host.snapshot(), &HostSnapshot::initial());
    }
    let request = contract.request(
        HostSnapshot::initial(),
        Query::StormPassed,
        Limits::COMPLETE,
    );
    let report = contract.analyse(&request).unwrap();
    assert_eq!(
        report.report.conclusion,
        ReachabilityConclusion::Unreachable
    );
    assert!(report.report.coverage.exhausted);
    assert_eq!(report.report.coverage.evaluated_states, 6);
    assert_eq!(report.report.coverage.discovered_states, 6);
    let mut host = Host::initial();
    assert!(matches!(
        contract.replay(&request, &report, &mut host).unwrap(),
        Replay::Unreachable
    ));
    assert_eq!(host.snapshot(), &HostSnapshot::initial());
}

#[test]
fn every_report_context_field_and_witness_initial_are_checked_before_actions() {
    use super::contract::Analysis;
    let contract = contract();
    let request = contract.request(HostSnapshot::initial(), Query::Sheltered, Limits::COMPLETE);
    let mutations: [fn(&mut Analysis); 9] = [
        |report| report.source_revision = "other-source".into(),
        |report| report.binding_version = "other-binding",
        |report| report.schema_version = "other-schema",
        |report| report.query = Query::Departed,
        |report| report.report.model_id = "other-model".into(),
        |report| {
            report
                .report
                .initial
                .values
                .insert("keeper.warning_shared".into(), Value::Bool(true));
        },
        |report| report.report.coverage.limits.max_states = 5,
        |report| report.report.coverage.limits.max_depth = 2,
        |report| {
            let ReachabilityConclusion::Reached { witness } = &mut report.report.conclusion else {
                panic!("expected real ending witness")
            };
            witness
                .initial
                .values
                .insert("keeper.warning_shared".into(), Value::Bool(true));
        },
    ];
    for mutate in mutations {
        let mut report = contract.analyse(&request).unwrap();
        mutate(&mut report);
        let mut host = Host::initial();
        assert!(matches!(
            contract.replay(&request, &report, &mut host),
            Err(Error::RejectedInput(_))
        ));
        assert_eq!(host.snapshot(), &HostSnapshot::initial());
    }
}

#[test]
fn captured_mid_story_state_roundtrips_spent_oil_and_a_lit_beacon() {
    let contract = contract();
    let mut host = Host::initial();
    assert!(host.apply(Action::ShareWarning));
    assert!(host.apply(Action::LightBeacon));
    let captured = host.snapshot().clone();
    let request = contract.request(captured.clone(), Query::Sheltered, Limits::COMPLETE);
    let encoded = serde_json::to_string(&request).unwrap();
    let decoded = contract.decode(&encoded).unwrap();
    assert_eq!(decoded.initial, captured);
    let audit = contract.audit(&decoded, &host).unwrap();
    assert_eq!(audit.states, 2);
    assert_eq!(audit.cases.len(), 10);
    let analysis = contract.analyse(&decoded).unwrap();
    assert!(matches!(
        contract.replay(&decoded, &analysis, &mut host).unwrap(),
        Replay::Reached { actions: 1, .. }
    ));
    assert_eq!(host.snapshot().ending, Ending::Sheltered);
    assert!(!host.snapshot().oil.is_available());
    assert!(host.snapshot().beacon.is_lit());
}

#[test]
fn dropped_witness_prefix_is_rejected_by_actual_host_guards_without_mutation() {
    let contract = contract();
    let request = contract.request(HostSnapshot::initial(), Query::Sheltered, Limits::COMPLETE);
    let mut analysis = contract.analyse(&request).unwrap();
    let ReachabilityConclusion::Reached { witness } = &mut analysis.report.conclusion else {
        panic!("expected real witness")
    };
    // Simulate a consumer losing the first delivered action, retaining the real remainder.
    assert_eq!(
        witness.steps.remove(0).transition.id,
        "keeper.share_warning"
    );
    let mut host = Host::initial();
    let Error::ConformanceFailure(failure) =
        contract.replay(&request, &analysis, &mut host).unwrap_err()
    else {
        panic!("expected rejected host command")
    };
    assert_eq!(failure.action, "keeper.light_beacon");
    assert!(failure.expected_enabled);
    assert!(!failure.actual_enabled);
    assert_eq!(failure.verified_prefix_actions, 0);
    assert_eq!(failure.accepted_host_actions, 0);
    assert_eq!(host.snapshot(), &HostSnapshot::initial());
}
