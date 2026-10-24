use std::collections::BTreeSet;

use super::chronology::{
    Before, Chronology, DisclosureRule, LIMITS, Occurrence, OrderError, Performance,
};
use leitmotif::{InvariantConclusion, SearchLimits, State, Value};

#[test]
fn repetitions_are_distinct_and_unordered() {
    let chronology = Chronology::story().unwrap();
    assert!(chronology.requires_before(Occurrence::LockA, Occurrence::Conversation));
    assert!(!chronology.requires_before(Occurrence::LockA, Occurrence::LockB));
    assert!(!chronology.requires_before(Occurrence::LockB, Occurrence::LockA));
}

fn follow(performance: &Performance, actions: &[&str]) -> State {
    let mut state = performance.initial.clone();
    for action in actions {
        state = performance
            .model
            .apply(&state, action)
            .unwrap()
            .expect("enabled action");
    }
    state
}
fn assert_flag(performance: &Performance, state: &State, key: &str, value: bool) {
    assert_eq!(
        performance.model.snapshot(state).unwrap().values.get(key),
        Some(&Value::Bool(value))
    );
}

#[test]
fn late_flashback_changes_audience_exposure_without_rewriting_learning() {
    let performance = Performance::new(DisclosureRule::RequiresAcquisition).unwrap();
    let before = follow(
        &performance,
        &["conversation", "acquire_lock_a", "disclose_lock_a"],
    );
    assert_flag(&performance, &before, "orin.knows.lock_a", true);
    assert_flag(&performance, &before, "audience.saw.lock_a", false);
    let after = performance
        .model
        .apply(&before, "flashback_lock_a")
        .unwrap()
        .unwrap();
    assert_flag(&performance, &after, "audience.saw.lock_a", true);
    assert_flag(&performance, &after, "orin.knows.lock_a", true);
    assert_flag(&performance, &after, "orin.knows.lock_b", false);
    assert!(
        Chronology::story()
            .unwrap()
            .requires_before(Occurrence::LockA, Occurrence::Conversation)
    );
    // One authored action slot cannot silently create another occurrence.
    assert!(
        performance
            .model
            .apply(&after, "flashback_lock_a")
            .unwrap()
            .is_none()
    );
}

#[test]
fn audience_revelation_does_not_license_character_disclosure() {
    let performance = Performance::new(DisclosureRule::RequiresAcquisition).unwrap();
    let state = follow(&performance, &["conversation", "flashback_lock_a"]);
    assert_flag(&performance, &state, "audience.saw.lock_a", true);
    assert_flag(&performance, &state, "orin.knows.lock_a", false);
    assert!(
        performance
            .model
            .apply(&state, "disclose_lock_a")
            .unwrap()
            .is_none()
    );
}

#[test]
fn early_disclosure_has_replayable_source_linked_evidence() {
    let performance = Performance::new(DisclosureRule::Unchecked).unwrap();
    let report = performance
        .model
        .check_invariants(&performance.initial, LIMITS)
        .unwrap();
    let InvariantConclusion::Violation { invariant, witness } = report.conclusion else {
        panic!("expected violation")
    };
    assert_eq!(invariant.id, "lock_a_disclosure_requires_acquisition");
    assert!(
        invariant
            .provenance
            .file
            .ends_with("chronology/performance.rs")
    );
    assert!(invariant.provenance.line > 0);
    assert_eq!(
        witness
            .steps
            .iter()
            .map(|step| step.transition.id.as_str())
            .collect::<Vec<_>>(),
        ["conversation", "disclose_lock_a"]
    );
    let mut state = performance.initial.clone();
    assert_eq!(witness.initial, performance.model.snapshot(&state).unwrap());
    for step in &witness.steps {
        state = performance
            .model
            .apply(&state, &step.transition.id)
            .unwrap()
            .unwrap();
        assert_eq!(performance.model.snapshot(&state).unwrap(), step.state);
        assert!(
            step.transition
                .provenance
                .file
                .ends_with("chronology/performance.rs")
        );
    }
    assert_flag(&performance, &state, "orin.knows.lock_a", false);
    assert_flag(&performance, &state, "orin.disclosed.lock_a", true);
}

#[test]
fn acquiring_one_repetition_does_not_license_the_other() {
    let performance = Performance::new(DisclosureRule::RequiresAcquisition).unwrap();
    for (acquire, allowed, denied) in [
        ("acquire_lock_a", "disclose_lock_a", "disclose_lock_b"),
        ("acquire_lock_b", "disclose_lock_b", "disclose_lock_a"),
    ] {
        let state = follow(&performance, &["conversation", acquire]);
        assert!(performance.model.apply(&state, allowed).unwrap().is_some());
        assert!(performance.model.apply(&state, denied).unwrap().is_none());
        assert!(performance.model.apply(&state, acquire).unwrap().is_none());
    }
}

#[test]
fn independent_historical_occurrences_allow_both_presentation_orders() {
    let performance = Performance::new(DisclosureRule::RequiresAcquisition).unwrap();
    let forward = follow(
        &performance,
        &["conversation", "flashback_lock_a", "flashback_lock_b"],
    );
    let backward = follow(
        &performance,
        &["conversation", "flashback_lock_b", "flashback_lock_a"],
    );
    assert_eq!(
        performance.model.snapshot(&forward).unwrap(),
        performance.model.snapshot(&backward).unwrap()
    );
    for key in ["audience.saw.lock_a", "audience.saw.lock_b"] {
        assert_flag(&performance, &forward, key, true);
    }
    for key in ["orin.knows.lock_a", "orin.knows.lock_b"] {
        assert_flag(&performance, &forward, key, false);
    }
}

#[test]
fn all_six_world_orders_cross_check_the_partial_relation() {
    let a = Occurrence::LockA;
    let b = Occurrence::LockB;
    let c = Occurrence::Conversation;
    let chronology = Chronology::story().unwrap();
    let permutations = [
        [a, b, c],
        [a, c, b],
        [b, a, c],
        [b, c, a],
        [c, a, b],
        [c, b, a],
    ];
    let compatible: Vec<_> = permutations
        .into_iter()
        .filter(|order| {
            order.iter().enumerate().all(|(i, left)| {
                order
                    .iter()
                    .enumerate()
                    .all(|(j, right)| !chronology.requires_before(*left, *right) || i < j)
            })
        })
        .collect();
    assert_eq!(compatible, [[a, b, c], [b, a, c]]);
}

#[test]
fn incompatible_order_retains_the_entire_source_linked_cycle() {
    let a = Occurrence::LockA;
    let b = Occurrence::LockB;
    let c = Occurrence::Conversation;
    let edges = [Before::new(a, b), Before::new(b, c), Before::new(c, a)];
    let error = Chronology::new(edges).err().unwrap();
    let OrderError::Cycle { chain } = error else {
        panic!("expected cycle")
    };
    assert_eq!(chain.len(), 3);
    for (actual, expected) in chain.iter().zip(edges) {
        assert_eq!(
            (actual.earlier, actual.later),
            (expected.earlier, expected.later)
        );
        assert_eq!(actual.source, expected.source);
        assert!(
            actual
                .source
                .file()
                .ends_with("tests/mystery/chronology.rs")
        );
    }
    assert!(
        matches!(Chronology::new([Before::new(a,a)]), Err(OrderError::Cycle { chain }) if chain.len() == 1)
    );
    let original = Before::new(a, b);
    let repeated = Before::new(a, b);
    let OrderError::Duplicate {
        original: left,
        repeated: right,
    } = Chronology::new([original, repeated]).err().unwrap()
    else {
        panic!("expected duplicate")
    };
    assert_eq!(left.source, original.source);
    assert_eq!(right.source, repeated.source);
    assert_ne!(left.source.line(), right.source.line());
}

#[test]
fn transitive_order_is_not_truth_inheritance() {
    let a = Occurrence::LockA;
    let b = Occurrence::LockB;
    let c = Occurrence::Conversation;
    let chronology = Chronology::new([Before::new(a, b), Before::new(b, c)]).unwrap();
    assert!(chronology.requires_before(a, c));
    assert!(!chronology.requires_before(c, a));
    assert!(!chronology.requires_before(a, a));
    // Disconnected and converging edges must not invent the missing relation.
    let disconnected = Chronology::new([Before::new(a, c), Before::new(b, c)]).unwrap();
    assert!(!disconnected.requires_before(a, b));
}

#[test]
fn incomplete_disclosure_check_cannot_claim_safety() {
    let performance = Performance::new(DisclosureRule::RequiresAcquisition).unwrap();
    for limits in [
        SearchLimits {
            max_states: 0,
            max_depth: 7,
        },
        SearchLimits {
            max_states: 2,
            max_depth: 7,
        },
        SearchLimits {
            max_states: 128,
            max_depth: 1,
        },
    ] {
        let report = performance
            .model
            .check_invariants(&performance.initial, limits)
            .unwrap();
        assert_eq!(report.conclusion, InvariantConclusion::Inconclusive);
        assert!(!report.coverage.exhausted);
    }
}

#[test]
fn fixed_disclosure_search_cost_and_safety() {
    // Measured first: seven one-shot actions admit 37 distinct guarded states.
    let performance = Performance::new(DisclosureRule::RequiresAcquisition).unwrap();
    let report = performance
        .model
        .check_invariants(&performance.initial, LIMITS)
        .unwrap();
    assert_eq!(report.conclusion, InvariantConclusion::Holds);
    assert!(report.coverage.exhausted);
    assert_eq!(report.coverage.discovered_states, 37);
    assert_eq!(report.coverage.evaluated_states, 37);
    assert_eq!(report.coverage.deepest_evaluated, Some(7));
    let completed = follow(
        &performance,
        &[
            "conversation",
            "acquire_lock_a",
            "acquire_lock_b",
            "disclose_lock_a",
            "disclose_lock_b",
            "flashback_lock_a",
            "flashback_lock_b",
        ],
    );
    assert!(
        performance
            .model
            .snapshot(&completed)
            .unwrap()
            .values
            .values()
            .all(|value| *value == Value::Bool(true))
    );
}

#[test]
fn action_sources_identify_distinct_authored_occurrences() {
    let performance = Performance::new(DisclosureRule::Unchecked).unwrap();
    let declarations: BTreeSet<_> = performance
        .model
        .transitions()
        .map(|transition| {
            assert!(
                transition
                    .provenance
                    .file
                    .ends_with("chronology/performance.rs")
            );
            (&transition.provenance.file, transition.provenance.line)
        })
        .collect();
    assert_eq!(declarations.len(), 7);
}
