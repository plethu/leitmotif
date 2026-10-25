use super::{
    Error, composed_town,
    host::{Action, Harbour, Host, Lighting},
    verification::{self, Replay},
};
use leitmotif::{ReachabilityConclusion, SearchLimits};

const LIMITS: SearchLimits = SearchLimits {
    max_states: 6,
    max_depth: 3,
};

#[test]
fn every_scheduled_action_agrees_at_all_six_states_with_fixed_cost() {
    let initial = Harbour::initial();
    let town = composed_town::project(&initial.projection()).unwrap();
    assert_eq!(verification::audit(&town, &initial).unwrap(), (6, 30));
}
#[test]
fn both_endings_replay_three_independent_actions() {
    let town = composed_town::town().unwrap();
    for target in [&town.sheltered, &town.departed] {
        let report = town
            .model
            .reachability(&town.initial, target, LIMITS)
            .unwrap();
        let ReachabilityConclusion::Reached { witness } = &report.conclusion else {
            panic!("expected witness")
        };
        let expected = witness.steps.last().unwrap().state.clone();
        let mut host = Host::new(Harbour::initial(), Lighting::SpendOil);
        assert_eq!(
            verification::replay(&town, &report, &mut host).unwrap(),
            Replay::Reached {
                actions: 3,
                state: expected
            }
        );
        let terminal = host.state().unwrap().clone();
        for action in Action::ALL {
            assert!(!host.apply(action).unwrap());
            assert_eq!(host.state().unwrap(), &terminal);
        }
    }
}
#[test]
fn incomplete_and_unreachable_analysis_have_distinct_effect_free_outcomes() {
    let town = composed_town::town().unwrap();
    for (target, limits, expected) in [
        (&town.unreachable, LIMITS, Replay::Unreachable),
        (
            &town.sheltered,
            SearchLimits {
                max_states: 1,
                max_depth: 0,
            },
            Replay::Inconclusive,
        ),
    ] {
        let report = town
            .model
            .reachability(&town.initial, target, limits)
            .unwrap();
        let mut host = Host::new(Harbour::initial(), Lighting::SpendOil);
        assert_eq!(
            verification::replay(&town, &report, &mut host).unwrap(),
            expected
        );
        assert_eq!(host.state().unwrap(), &Harbour::initial());
    }
}
#[test]
fn stale_snapshot_and_changed_model_identity_fail_before_effects() {
    let town = composed_town::town().unwrap();
    let mut report = town
        .model
        .reachability(&town.initial, &town.sheltered, LIMITS)
        .unwrap();
    let mut host = Host::new(Harbour::initial(), Lighting::SpendOil);
    assert!(host.apply(Action::ShareWarning).unwrap());
    let before = host.state().unwrap().clone();
    assert!(matches!(
        verification::replay(&town, &report, &mut host),
        Err(Error::StaleSnapshot)
    ));
    assert_eq!(host.state().unwrap(), &before);
    report.model_id = "different model".into();
    let mut host = Host::new(Harbour::initial(), Lighting::SpendOil);
    assert!(matches!(
        verification::replay(&town, &report, &mut host),
        Err(Error::StaleSnapshot)
    ));
    assert_eq!(host.state().unwrap(), &Harbour::initial());
}
#[test]
fn bad_lighting_retains_actual_effects_and_reports_verified_prefix() {
    let town = composed_town::town().unwrap();
    let report = town
        .model
        .reachability(&town.initial, &town.sheltered, LIMITS)
        .unwrap();
    let mut host = Host::new(Harbour::initial(), Lighting::PreserveOil);
    let Err(Error::Divergence(difference)) = verification::replay(&town, &report, &mut host) else {
        panic!("expected divergence")
    };
    assert_eq!(difference.action, "keeper.light_beacon");
    assert_eq!(difference.verified_prefix, 1);
    assert_eq!(difference.accepted_actions, 2);
    assert!(difference.accepted);
    assert!(difference.source.file.contains("composed_town/keeper.rs"));
    assert_ne!(difference.actual, difference.expected);
    assert_eq!(host.state().unwrap().snapshot(), difference.actual);
    assert!(difference.to_string().contains("keeper.light_beacon"));
}
#[test]
fn unknown_later_action_is_resolved_before_any_effect() {
    let town = composed_town::town().unwrap();
    let mut report = town
        .model
        .reachability(&town.initial, &town.sheltered, LIMITS)
        .unwrap();
    let ReachabilityConclusion::Reached { witness } = &mut report.conclusion else {
        panic!("expected witness")
    };
    witness.steps.last_mut().unwrap().transition.id = "missing.action".into();
    let mut host = Host::new(Harbour::initial(), Lighting::SpendOil);
    assert!(matches!(
        verification::replay(&town, &report, &mut host),
        Err(Error::Unsupported(_))
    ));
    assert_eq!(host.state().unwrap(), &Harbour::initial());
}
#[test]
fn executable_runs_the_fixed_fixture() {
    verification::run().unwrap();
}

#[test]
fn malformed_later_snapshot_and_initial_trace_fail_before_any_effect() {
    let town = composed_town::town().unwrap();
    let report = town
        .model
        .reachability(&town.initial, &town.sheltered, LIMITS)
        .unwrap();
    for corrupt_initial in [false, true] {
        let mut changed = report.clone();
        let ReachabilityConclusion::Reached { witness } = &mut changed.conclusion else {
            panic!("expected witness")
        };
        if corrupt_initial {
            witness.initial.values.clear();
        } else {
            witness.steps.last_mut().unwrap().state.values.clear();
        }
        let mut host = Host::new(Harbour::initial(), Lighting::SpendOil);
        assert!(matches!(
            verification::replay(&town, &changed, &mut host),
            Err(Error::InvalidWitness)
        ));
        assert_eq!(host.state().unwrap(), &Harbour::initial());
    }
}

#[test]
fn empty_and_valid_prefix_witnesses_do_not_claim_the_ending() {
    let town = composed_town::town().unwrap();
    let report = town
        .model
        .reachability(&town.initial, &town.sheltered, LIMITS)
        .unwrap();
    for length in [0, 1, 2] {
        let mut changed = report.clone();
        let ReachabilityConclusion::Reached { witness } = &mut changed.conclusion else {
            panic!("expected witness")
        };
        witness.steps.truncate(length);
        let mut host = Host::new(Harbour::initial(), Lighting::SpendOil);
        assert!(matches!(
            verification::replay(&town, &changed, &mut host),
            Err(Error::InvalidWitness)
        ));
        assert_eq!(host.state().unwrap(), &Harbour::initial());
    }
}

#[test]
fn changed_target_must_be_declared_and_satisfied_before_effects() {
    let town = composed_town::town().unwrap();
    let report = town
        .model
        .reachability(&town.initial, &town.sheltered, LIMITS)
        .unwrap();
    let departed = town
        .model
        .reachability(&town.initial, &town.departed, LIMITS)
        .unwrap();
    for target in [
        departed.target,
        leitmotif::PredicateDescription {
            expression: leitmotif::PredicateExpression::Constant(true),
            provenance: report.target.provenance.clone(),
        },
    ] {
        let mut changed = report.clone();
        changed.target = target;
        let mut host = Host::new(Harbour::initial(), Lighting::SpendOil);
        assert!(matches!(
            verification::replay(&town, &changed, &mut host),
            Err(Error::InvalidWitness)
        ));
        assert_eq!(host.state().unwrap(), &Harbour::initial());
    }
}

#[test]
fn initially_satisfied_ending_accepts_a_genuine_empty_witness() {
    let mut host = Host::new(Harbour::initial(), Lighting::SpendOil);
    for action in [Action::ShareWarning, Action::LightBeacon, Action::Shelter] {
        assert!(host.apply(action).unwrap());
    }
    let before = host.state().unwrap().clone();
    let town = composed_town::project(&before.projection()).unwrap();
    let report = town
        .model
        .reachability(&town.initial, &town.sheltered, LIMITS)
        .unwrap();
    assert_eq!(
        verification::replay(&town, &report, &mut host).unwrap(),
        Replay::Reached {
            actions: 0,
            state: before.snapshot()
        }
    );
    assert_eq!(host.state().unwrap(), &before);
}

#[test]
fn audit_reports_guard_disagreement_from_independently_captured_states() {
    let initial = Harbour::initial();
    let mut warned = Host::new(initial.clone(), Lighting::SpendOil);
    assert!(warned.apply(Action::ShareWarning).unwrap());
    let town = composed_town::project(&warned.state().unwrap().projection()).unwrap();
    let Err(Error::Divergence(difference)) = verification::audit(&town, &initial) else {
        panic!("expected independently captured state disagreement")
    };
    assert_eq!(difference.action, "keeper.share_warning");
    assert!(!difference.expected_enabled);
    assert!(difference.accepted);
    assert_eq!(difference.verified_prefix, 0);
}

#[test]
fn a_report_cannot_replay_against_a_different_town_initial_state() {
    let mut town = composed_town::town().unwrap();
    let report = town
        .model
        .reachability(&town.initial, &town.sheltered, LIMITS)
        .unwrap();
    town.initial = town
        .model
        .apply(&town.initial, Action::ShareWarning.id())
        .unwrap()
        .unwrap();
    let mut host = Host::new(Harbour::initial(), Lighting::SpendOil);
    assert!(matches!(
        verification::replay(&town, &report, &mut host),
        Err(Error::InvalidWitness)
    ));
    assert_eq!(host.state().unwrap(), &Harbour::initial());
}

#[test]
fn a_known_but_disabled_witness_action_fails_before_effects() {
    let town = composed_town::town().unwrap();
    let mut report = town
        .model
        .reachability(&town.initial, &town.sheltered, LIMITS)
        .unwrap();
    let ReachabilityConclusion::Reached { witness } = &mut report.conclusion else {
        panic!("expected witness")
    };
    witness.steps.first_mut().unwrap().transition.id = Action::Shelter.id().into();
    let mut host = Host::new(Harbour::initial(), Lighting::SpendOil);
    assert!(matches!(
        verification::replay(&town, &report, &mut host),
        Err(Error::InvalidWitness)
    ));
    assert_eq!(host.state().unwrap(), &Harbour::initial());
}

#[test]
fn divergence_display_identifies_missing_and_unexpected_facts_and_retains_evidence() {
    let town = composed_town::town().unwrap();
    let report = town
        .model
        .reachability(&town.initial, &town.sheltered, LIMITS)
        .unwrap();
    let mut host = Host::new(Harbour::initial(), Lighting::PreserveOil);
    let Err(Error::Divergence(mut difference)) = verification::replay(&town, &report, &mut host)
    else {
        panic!("expected divergence")
    };
    difference.actual.values.remove("keeper.beacon_lit");
    difference
        .actual
        .values
        .insert("unexpected.fact".into(), leitmotif::Value::Bool(true));
    let display = difference.to_string();
    assert!(display.starts_with(
        "keeper.light_beacon\n  harbour.oil_available: expected Bool(false), actual Bool(true)\n"
    ));
    assert!(display.contains("keeper.beacon_lit: expected Bool(true), actual <missing>"));
    assert!(display.contains("unexpected.fact: expected <missing>, actual Bool(true)"));
    assert!(display.contains("\n  source: "));
    assert!(display.contains("composed_town/keeper.rs"));
    assert!(display.contains("\n  enabled: expected true, accepted true\n"));
    assert!(display.contains("\n  expected snapshot:\n"));
    assert!(display.contains("\n  actual snapshot:\n"));
    assert!(display.contains("    ferryman.ready: Bool(false)\n"));
    assert!(display.ends_with("execution: verified prefix 1, accepted actions 2"));
}
