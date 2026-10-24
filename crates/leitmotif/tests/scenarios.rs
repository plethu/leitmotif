//! Behavioral acceptance fixtures expressed through the consumer-facing API.

#[path = "../examples/support/mystery.rs"]
mod mystery;
#[path = "../examples/support/town.rs"]
mod town;

use leitmotif::{
    ClassificationLimits, HypothesisConclusion, InvariantConclusion, ReachabilityConclusion,
    SearchLimits,
};
pub use mystery::{Mystery, MysteryEvidence, mystery};
pub use town::{Town, TownVariant, town};

const LIMITS: SearchLimits = SearchLimits {
    max_states: 1024,
    max_depth: 32,
};
const CANDIDATES: ClassificationLimits = ClassificationLimits {
    max_candidates: 1024,
};

#[test]
fn intersecting_stories_reach_both_endings_without_confusing_lies_and_truth() {
    let f = town(TownVariant::Valid).unwrap();
    let safety = f.model.check_invariants(&f.initial, LIMITS).unwrap();
    assert_eq!(safety.conclusion, InvariantConclusion::Holds);
    assert!(safety.coverage.exhausted);
    for target in [&f.ending_a, &f.ending_d] {
        let report = f.model.reachability(&f.initial, target, LIMITS).unwrap();
        let ReachabilityConclusion::Reached { witness } = report.conclusion else {
            panic!("ending must be reachable");
        };
        let mut state = f.initial.clone();
        for step in witness.steps {
            state = f
                .model
                .apply(&state, &step.transition.id)
                .unwrap()
                .expect("witness guard enabled");
            assert_eq!(f.model.snapshot(&state).unwrap(), step.state);
        }
        assert!(f.model.evaluate(&state, target).unwrap());
    }
    let lie = f.model.apply(&f.initial, "mara.pretend").unwrap().unwrap();
    assert_eq!(
        f.model.check_invariants(&lie, LIMITS).unwrap().conclusion,
        InvariantConclusion::Holds
    );
}

#[test]
fn a_satisfiable_gate_can_be_unreachable_while_an_alternative_ending_survives() {
    let f = town(TownVariant::EvidenceClosesCouncil).unwrap();
    assert_eq!(
        f.model
            .classify(&f.ending_gate, &[], CANDIDATES)
            .unwrap()
            .conclusion,
        HypothesisConclusion::Possible
    );
    let report = f
        .model
        .reachability(&f.initial, &f.ending_a, LIMITS)
        .unwrap();
    assert_eq!(report.conclusion, ReachabilityConclusion::Unreachable);
    assert!(report.coverage.exhausted);
    assert!(matches!(
        f.model
            .reachability(&f.initial, &f.ending_d, LIMITS)
            .unwrap()
            .conclusion,
        ReachabilityConclusion::Reached { .. }
    ));
}

#[test]
fn a_missing_learning_guard_produces_a_source_linked_replayable_violation() {
    let f = town(TownVariant::UnguardedDisclosure).unwrap();
    let report = f.model.check_invariants(&f.initial, LIMITS).unwrap();
    let InvariantConclusion::Violation { invariant, witness } = report.conclusion else {
        panic!("missing guard should violate invariant");
    };
    assert_eq!(invariant.id, "mara.truthful_requires_learning");
    assert_eq!(witness.steps.len(), 1);
    let step = witness.steps.first().unwrap();
    assert_eq!(step.transition.id, "mara.testify");
    assert!(step.transition.provenance.file.ends_with("support/town.rs"));
    let state = f
        .model
        .apply(&f.initial, &step.transition.id)
        .unwrap()
        .unwrap();
    assert_eq!(f.model.snapshot(&state).unwrap(), step.state);
}

#[test]
fn evidence_can_preserve_ambiguity_then_exclude_a_hypothesis() {
    let partial = mystery(MysteryEvidence::Partial).unwrap();
    let report = partial
        .model
        .classify(&partial.hypothesis, &partial.premises, CANDIDATES)
        .unwrap();
    assert_eq!(report.conclusion, HypothesisConclusion::Possible);
    assert!(report.witness.is_some());
    assert!(report.counterexample.is_some());
    assert_eq!(report.coverage.satisfying_candidates, 3);
    let complete = mystery(MysteryEvidence::Complete).unwrap();
    let report = complete
        .model
        .classify(&complete.hypothesis, &complete.premises, CANDIDATES)
        .unwrap();
    assert_eq!(report.conclusion, HypothesisConclusion::RuledOut);
    assert_eq!(report.coverage.satisfying_candidates, 1);
}

#[test]
fn contradictory_premises_do_not_establish_every_hypothesis() {
    let f = mystery(MysteryEvidence::Contradictory).unwrap();
    let report = f
        .model
        .classify(&f.hypothesis, &f.premises, CANDIDATES)
        .unwrap();
    assert_eq!(report.conclusion, HypothesisConclusion::Inconsistent);
    assert!(report.coverage.exhausted);
    assert!(report.witness.is_none());
    assert!(report.counterexample.is_none());
}

#[test]
fn a_budget_cannot_turn_a_missing_witness_into_a_proof() {
    let f = town(TownVariant::Valid).unwrap();
    let report = f
        .model
        .reachability(
            &f.initial,
            &f.ending_a,
            SearchLimits {
                max_states: 1,
                max_depth: 32,
            },
        )
        .unwrap();
    assert_eq!(report.conclusion, ReachabilityConclusion::Inconclusive);
    assert!(!report.coverage.exhausted);
    let mystery = mystery(MysteryEvidence::Contradictory).unwrap();
    let report = mystery
        .model
        .classify(
            &mystery.hypothesis,
            &mystery.premises,
            ClassificationLimits { max_candidates: 1 },
        )
        .unwrap();
    assert_eq!(report.conclusion, HypothesisConclusion::Inconclusive);
}

#[test]
fn repeated_reports_are_byte_identical() {
    let f = town(TownVariant::Valid).unwrap();
    let first = f
        .model
        .reachability(&f.initial, &f.ending_a, LIMITS)
        .unwrap();
    let second = f
        .model
        .reachability(&f.initial, &f.ending_a, LIMITS)
        .unwrap();
    assert_eq!(
        serde_json::to_string(&first).unwrap(),
        serde_json::to_string(&second).unwrap()
    );
}

#[test]
fn missing_initial_values_and_unknown_replay_steps_point_to_the_consumer() {
    let fixture = town(TownVariant::Valid).unwrap();
    let missing = fixture.model.state([]).unwrap_err();
    assert!(missing.provenance.file.ends_with("scenarios.rs"));
    let unknown = fixture
        .model
        .apply(&fixture.initial, "not.a.transition")
        .unwrap_err();
    assert!(unknown.provenance.file.ends_with("scenarios.rs"));
}
