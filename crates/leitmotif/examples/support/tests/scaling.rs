//! Known finite graphs and replayable witnesses for the scaling fixture.

use super::{Parameters, Resource, Town};
use leitmotif::{ReachabilityConclusion, SearchLimits};

const LIMITS: SearchLimits = SearchLimits {
    max_states: 1024,
    max_depth: 64,
};

#[test]
fn independent_threads_have_the_cartesian_state_count_and_shortest_witness() {
    let town = Town::new(Parameters::new(2, 3, 0, Resource::Unlimited).unwrap()).unwrap();
    let graph = town
        .model
        .reachability(&town.initial, &town.unreachable, LIMITS)
        .unwrap();
    assert_eq!(graph.conclusion, ReachabilityConclusion::Unreachable);
    assert!(graph.coverage.exhausted);
    assert_eq!(graph.coverage.discovered_states, 9);
    assert_eq!(graph.coverage.evaluated_states, 9);
    let report = town
        .model
        .reachability(&town.initial, &town.goal, LIMITS)
        .unwrap();
    let ReachabilityConclusion::Reached { witness } = report.conclusion else {
        panic!("independent threads must reach their final phases");
    };
    assert_eq!(witness.steps.len(), 4);
    let mut state = town.initial.clone();
    assert_eq!(town.model.snapshot(&state).unwrap(), witness.initial);
    for step in witness.steps {
        state = town
            .model
            .apply(&state, &step.transition.id)
            .unwrap()
            .expect("enabled witness step");
        assert_eq!(town.model.snapshot(&state).unwrap(), step.state);
    }
    assert!(town.model.evaluate(&state, &town.goal).unwrap());
}

#[test]
fn completed_thread_cycles_return_to_the_same_full_state() {
    let town = Town::new(Parameters::new(2, 3, 0, Resource::SinglePermit).unwrap()).unwrap();
    let mut state = town.initial.clone();
    for transition in ["town.thread.0.0", "town.thread.0.1", "town.thread.0.2"] {
        state = town
            .model
            .apply(&state, transition)
            .unwrap()
            .expect("enabled cycle step");
    }
    assert_eq!(
        town.model.snapshot(&state).unwrap(),
        town.model.snapshot(&town.initial).unwrap()
    );
}

#[test]
fn coupling_and_a_single_permit_reduce_the_reachable_graph() {
    for (coupling, resource, states) in [
        (0, Resource::Unlimited, 27),
        (1, Resource::Unlimited, 19),
        (2, Resource::Unlimited, 7),
        (0, Resource::SinglePermit, 7),
        (2, Resource::SinglePermit, 7),
    ] {
        let town = Town::new(Parameters::new(3, 3, coupling, resource).unwrap()).unwrap();
        let report = town
            .model
            .reachability(&town.initial, &town.unreachable, LIMITS)
            .unwrap();
        assert_eq!(report.conclusion, ReachabilityConclusion::Unreachable);
        assert!(report.coverage.exhausted);
        assert_eq!(
            report.coverage.discovered_states, states,
            "coupling={coupling}, resource={resource:?}"
        );
        assert_eq!(report.coverage.evaluated_states, states);
        if resource == Resource::SinglePermit || coupling == 2 {
            let goal = town
                .model
                .reachability(&town.initial, &town.goal, LIMITS)
                .unwrap();
            assert_eq!(goal.conclusion, ReachabilityConclusion::Unreachable);
        }
    }
}

#[test]
fn one_permit_serializes_starts_and_is_returned_after_completion() {
    let town = Town::new(Parameters::new(2, 3, 0, Resource::SinglePermit).unwrap()).unwrap();
    let started = town
        .model
        .apply(&town.initial, "town.thread.0.0")
        .unwrap()
        .unwrap();
    assert!(
        town.model
            .apply(&started, "town.thread.1.0")
            .unwrap()
            .is_none()
    );
    let finishing = town
        .model
        .apply(&started, "town.thread.0.1")
        .unwrap()
        .unwrap();
    let released = town
        .model
        .apply(&finishing, "town.thread.0.2")
        .unwrap()
        .unwrap();
    assert!(
        town.model
            .apply(&released, "town.thread.1.0")
            .unwrap()
            .is_some()
    );
}

#[test]
fn truncated_graphs_never_become_exhaustive_unreachability_claims() {
    let town = Town::new(Parameters::new(2, 3, 0, Resource::Unlimited).unwrap()).unwrap();
    for limits in [
        SearchLimits {
            max_states: 4,
            max_depth: 64,
        },
        SearchLimits {
            max_states: 1024,
            max_depth: 1,
        },
    ] {
        let report = town
            .model
            .reachability(&town.initial, &town.unreachable, limits)
            .unwrap();
        assert_eq!(report.conclusion, ReachabilityConclusion::Inconclusive);
        assert!(!report.coverage.exhausted);
        assert!(report.coverage.state_limit_hit || report.coverage.depth_limit_hit);
    }
}

#[test]
fn supported_domain_sizes_and_thread_counts_are_independent() {
    for domain in 2..=8 {
        let town =
            Town::new(Parameters::new(16, domain, 15, Resource::Unlimited).unwrap()).unwrap();
        let report = town
            .model
            .reachability(&town.initial, &town.unreachable, LIMITS)
            .unwrap();
        assert!(report.coverage.exhausted);
        assert_eq!(report.coverage.evaluated_states, 1 + 16 * (domain - 1));
        for fact in town
            .model
            .facts()
            .iter()
            .filter(|fact| fact.key.starts_with("town.thread."))
        {
            assert_eq!(fact.domain.len(), domain);
        }
    }
}

#[test]
fn invalid_parameters_are_rejected_before_constructing_a_model() {
    for (threads, domain, coupling) in [(0, 3, 0), (17, 3, 0), (2, 1, 0), (2, 9, 0), (2, 3, 2)] {
        assert!(Parameters::new(threads, domain, coupling, Resource::Unlimited).is_err());
    }
}

#[test]
fn fixed_independent_workload_exhausts_within_its_measured_state_budget() {
    // Six three-state threads measured 729 admitted and evaluated states.
    // An exact budget also catches false truncation at the admission boundary.
    let town = Town::new(Parameters::new(6, 3, 0, Resource::Unlimited).unwrap()).unwrap();
    let limits = SearchLimits {
        max_states: 729,
        max_depth: 12,
    };
    let report = town
        .model
        .reachability(&town.initial, &town.unreachable, limits)
        .unwrap();
    assert_eq!(report.conclusion, ReachabilityConclusion::Unreachable);
    assert_eq!(report.coverage.discovered_states, 729);
    assert_eq!(report.coverage.evaluated_states, 729);
    assert_eq!(report.coverage.deepest_evaluated, Some(12));
    assert!(report.coverage.exhausted);
    assert!(!report.coverage.state_limit_hit);
    assert!(!report.coverage.depth_limit_hit);
}
