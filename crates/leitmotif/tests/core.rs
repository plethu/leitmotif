//! Contract tests for the retained finite-model core.

use std::panic::Location;

use leitmotif::{
    ClassificationLimits, Domain, ErrorKind, Fact, FactHandle, FactKind, HypothesisConclusion,
    Invariant, InvariantConclusion, Model, ModelBuilder, Predicate, PredicateKind, Premise,
    ReachabilityConclusion, SearchLimits, Source, Transition, Value,
};

struct Learned;
impl Fact for Learned {
    type Value = bool;
    const KEY: &'static str = "learned";
}

struct Ending;
impl Fact for Ending {
    type Value = bool;
    const KEY: &'static str = "ending";
}

struct Locked;
impl Fact for Locked {
    type Value = bool;
    const KEY: &'static str = "locked";
    const KIND: FactKind = FactKind::Setting;
}

#[derive(Clone)]
enum Colour {
    Red,
    Blue,
    Undeclared,
}

impl Domain for Colour {
    fn values() -> Vec<Self> {
        vec![Self::Red, Self::Blue]
    }

    fn encode(self) -> Value {
        Value::Symbol(
            match self {
                Self::Red => "red",
                Self::Blue => "blue",
                Self::Undeclared => "undeclared",
            }
            .to_owned(),
        )
    }
}

struct Paint;
impl Fact for Paint {
    type Value = Colour;
    const KEY: &'static str = "paint";
}

const SEARCH: SearchLimits = SearchLimits {
    max_states: 32,
    max_depth: 16,
};
const CLASSIFY: ClassificationLimits = ClassificationLimits { max_candidates: 32 };

#[test]
fn composed_impossible_gate_is_exhaustively_unreachable() {
    let mut builder = ModelBuilder::new("town").unwrap();
    let learned = builder.fact::<Learned>().unwrap();
    let ending = builder.fact::<Ending>().unwrap();
    let impossible = learned.is(true).and(learned.is(false));
    let model = builder
        .with_transition(
            Transition::new("impossible")
                .with_guard(impossible)
                .with_effect(ending.set(true)),
        )
        .build()
        .unwrap();
    let initial = model
        .state([learned.set(false), ending.set(false)])
        .unwrap();
    let report = model
        .reachability(&initial, &ending.is(true), SEARCH)
        .unwrap();
    assert_eq!(report.conclusion, ReachabilityConclusion::Unreachable);
    assert!(report.coverage.exhausted);
    assert_eq!(report.coverage.evaluated_states, 1);
    assert_eq!(report.context, "town");
    assert_eq!(
        report.schema_version,
        leitmotif::EXPERIMENTAL_SCHEMA_VERSION
    );
}

#[test]
fn alternative_ending_has_shortest_deterministic_full_witness() {
    let mut builder = ModelBuilder::new("town").unwrap();
    let learned = builder.fact::<Learned>().unwrap();
    let ending = builder.fact::<Ending>().unwrap();
    let locked = builder.fact::<Locked>().unwrap();
    let model = builder
        .with_transition(Transition::new("learn").with_effect(learned.set(true)))
        .with_transition(
            Transition::new("long-ending")
                .with_guard(learned.is(true))
                .with_effect(ending.set(true)),
        )
        .with_transition(
            Transition::new("alternative")
                .with_guard(locked.is(false))
                .with_effect(ending.set(true)),
        )
        .build()
        .unwrap();
    let initial = model
        .state([learned.set(false), ending.set(false), locked.set(false)])
        .unwrap();
    let target = ending.is(true);
    let report = model.reachability(&initial, &target, SEARCH).unwrap();
    let repeated = model.reachability(&initial, &target, SEARCH).unwrap();
    assert_eq!(report, repeated);
    assert_eq!(
        serde_json::to_string(&report).unwrap(),
        serde_json::to_string(&repeated).unwrap()
    );
    let ReachabilityConclusion::Reached { witness } = report.conclusion else {
        panic!("expected reachable ending");
    };
    assert_eq!(witness.steps.len(), 1);
    let step = witness.steps.first().expect("missing witness step");
    assert_eq!(step.transition.id, "alternative");
    assert!(step.transition.provenance.file.ends_with("core.rs"));
    assert!(step.transition.provenance.line > 0);
    assert!(step.transition.provenance.column > 0);
    assert_eq!(step.state.values.len(), 3);
    assert_eq!(step.state.values.get("learned"), Some(&Value::Bool(false)));
    assert_eq!(step.state.values.get("ending"), Some(&Value::Bool(true)));
    assert_eq!(step.state.values.get("locked"), Some(&Value::Bool(false)));
    let replay = model
        .apply(&initial, &step.transition.id)
        .unwrap()
        .expect("disabled replay transition");
    assert_eq!(model.snapshot(&replay).unwrap(), step.state);
    assert!(model.evaluate(&replay, &target).unwrap());
}

#[test]
fn invariant_violation_replays_and_does_not_prune_reachability() {
    let mut builder = ModelBuilder::new("truth").unwrap();
    let learned = builder.fact::<Learned>().unwrap();
    let ending = builder.fact::<Ending>().unwrap();
    let truthful = learned.is(false);
    let model = builder
        .with_transition(Transition::new("learn").with_effect(learned.set(true)))
        .with_transition(
            Transition::new("finish")
                .with_guard(learned.is(true))
                .with_effect(ending.set(true)),
        )
        .with_invariant(Invariant::new("truthful", truthful.clone()))
        .build()
        .unwrap();
    let initial = model
        .state([learned.set(false), ending.set(false)])
        .unwrap();
    let report = model.check_invariants(&initial, SEARCH).unwrap();
    let InvariantConclusion::Violation { invariant, witness } = report.conclusion else {
        panic!("expected invariant violation");
    };
    assert_eq!(invariant.id, "truthful");
    assert!(invariant.provenance.file.ends_with("core.rs"));
    assert_eq!(witness.steps.len(), 1);
    let mut replay = initial.clone();
    for step in &witness.steps {
        replay = model
            .apply(&replay, &step.transition.id)
            .unwrap()
            .expect("disabled trace step");
        assert_eq!(model.snapshot(&replay).unwrap(), step.state);
    }
    assert!(!model.evaluate(&replay, &truthful).unwrap());
    let reachable = model
        .reachability(&initial, &ending.is(true), SEARCH)
        .unwrap();
    assert!(matches!(
        reachable.conclusion,
        ReachabilityConclusion::Reached { .. }
    ));
}

#[test]
fn initial_violations_and_initial_targets_need_no_transition() {
    let mut builder = ModelBuilder::new("initial").unwrap();
    let learned = builder.fact::<Learned>().unwrap();
    let model = builder
        .with_invariant(Invariant::new("never", learned.is(false)))
        .build()
        .unwrap();
    let initial = model.state([learned.set(true)]).unwrap();
    let limits = SearchLimits {
        max_states: 1,
        max_depth: 0,
    };
    let reached = model
        .reachability(&initial, &learned.is(true), limits)
        .unwrap();
    let ReachabilityConclusion::Reached { witness } = reached.conclusion else {
        panic!("initial target missed");
    };
    assert!(witness.steps.is_empty());
    let violated = model.check_invariants(&initial, limits).unwrap();
    let InvariantConclusion::Violation { witness, .. } = violated.conclusion else {
        panic!("initial violation missed");
    };
    assert!(witness.steps.is_empty());
}

#[test]
fn bounds_report_distinct_unexplored_successors() {
    let mut builder = ModelBuilder::new("bounds").unwrap();
    let learned = builder.fact::<Learned>().unwrap();
    let model = builder
        .with_transition(Transition::new("learn").with_effect(learned.set(true)))
        .with_invariant(Invariant::new("not-yet", learned.is(false)))
        .build()
        .unwrap();
    let initial = model.state([learned.set(false)]).unwrap();
    for limits in [
        SearchLimits {
            max_states: 1,
            max_depth: 5,
        },
        SearchLimits {
            max_states: 5,
            max_depth: 0,
        },
    ] {
        let report = model
            .reachability(&initial, &learned.is(true), limits)
            .unwrap();
        assert_eq!(report.conclusion, ReachabilityConclusion::Inconclusive);
        assert!(!report.coverage.exhausted);
        assert!(report.coverage.state_limit_hit || report.coverage.depth_limit_hit);
        let invariants = model.check_invariants(&initial, limits).unwrap();
        assert_eq!(invariants.conclusion, InvariantConclusion::Inconclusive);
    }
    let zero = model
        .reachability(
            &initial,
            &learned.is(false),
            SearchLimits {
                max_states: 0,
                max_depth: 0,
            },
        )
        .unwrap();
    assert_eq!(zero.conclusion, ReachabilityConclusion::Inconclusive);
    assert_eq!(zero.coverage.discovered_states, 0);
    assert_eq!(zero.coverage.evaluated_states, 0);
    assert_eq!(zero.coverage.deepest_evaluated, None);
}

#[test]
fn cycles_and_exact_bounds_can_exhaust_without_enumerating_paths() {
    let mut builder = ModelBuilder::new("cycle").unwrap();
    let learned = builder.fact::<Learned>().unwrap();
    let model = builder
        .with_transition(Transition::new("up").with_effect(learned.set(true)))
        .with_transition(Transition::new("down").with_effect(learned.set(false)))
        .with_invariant(Invariant::new(
            "tautology",
            learned.is(true).or(learned.is(false)),
        ))
        .build()
        .unwrap();
    let initial = model.state([learned.set(false)]).unwrap();
    let limits = SearchLimits {
        max_states: 2,
        max_depth: 1,
    };
    let report = model
        .reachability(&initial, &Predicate::constant(false), limits)
        .unwrap();
    assert_eq!(report.conclusion, ReachabilityConclusion::Unreachable);
    assert!(report.coverage.exhausted);
    assert!(!report.coverage.depth_limit_hit);
    assert!(!report.coverage.state_limit_hit);
    assert_eq!(report.coverage.evaluated_states, 2);
    assert_eq!(
        model.check_invariants(&initial, limits).unwrap().conclusion,
        InvariantConclusion::Holds
    );
}

#[test]
fn guard_uses_pre_state_and_effects_produce_complete_post_state() {
    let mut builder = ModelBuilder::new("simultaneous").unwrap();
    let learned = builder.fact::<Learned>().unwrap();
    let ending = builder.fact::<Ending>().unwrap();
    let model = builder
        .with_transition(
            Transition::new("both")
                .with_guard(learned.is(false).and(ending.is(false)))
                .with_effect(learned.set(true))
                .with_effect(ending.set(true)),
        )
        .build()
        .unwrap();
    let initial = model
        .state([learned.set(false), ending.set(false)])
        .unwrap();
    let next = model
        .apply(&initial, "both")
        .unwrap()
        .expect("guard did not use pre-state");
    assert!(
        model
            .evaluate(&next, &learned.is(true).and(ending.is(true)))
            .unwrap()
    );
    assert!(model.apply(&next, "both").unwrap().is_none());
    assert!(
        model
            .evaluate(&initial, &learned.is(false).and(ending.is(false)))
            .unwrap()
    );
    assert!(
        matches!(model.apply(&initial, "unknown"), Err(error) if matches!(error.kind, ErrorKind::UnknownTransition(_)))
    );
}

#[test]
fn all_facts_must_be_assigned_once_including_settings() {
    let mut builder = ModelBuilder::new("complete").unwrap();
    let learned = builder.fact::<Learned>().unwrap();
    let locked = builder.fact::<Locked>().unwrap();
    let model = builder.build().unwrap();
    assert!(
        matches!(model.state([learned.set(false)]), Err(error) if error.kind == ErrorKind::MissingAssignment("locked".to_owned()))
    );
    assert!(
        matches!(model.state([learned.set(false), locked.set(true), learned.set(false)]), Err(error) if error.kind == ErrorKind::DuplicateWrite("learned".to_owned()))
    );
    let initial = model.state([locked.set(true), learned.set(false)]).unwrap();
    assert_eq!(
        model.snapshot(&initial).unwrap().values.get("locked"),
        Some(&Value::Bool(true))
    );
}

#[test]
fn foreign_handles_are_rejected_even_in_dead_predicate_branches() {
    let mut first = ModelBuilder::new("same-name").unwrap();
    let local = first.fact::<Learned>().unwrap();
    let mut second = ModelBuilder::new("same-name").unwrap();
    let foreign = second.fact::<Learned>().unwrap();
    let model = first.build().unwrap();
    let initial = model.state([local.set(false)]).unwrap();
    let query = Predicate::constant(false).and(foreign.is(true));
    assert!(
        matches!(model.evaluate(&initial, &query), Err(error) if matches!(error.kind, ErrorKind::ForeignHandle(_)))
    );
    assert!(
        matches!(model.state([foreign.set(false)]), Err(error) if matches!(error.kind, ErrorKind::ForeignHandle(_)))
    );
    assert!(
        matches!(model.reachability(&initial, &query, SEARCH), Err(error) if matches!(error.kind, ErrorKind::ForeignHandle(_)))
    );
    assert!(
        matches!(model.classify(&query, &[], ClassificationLimits { max_candidates: 0 }), Err(error) if matches!(error.kind, ErrorKind::ForeignHandle(_)))
    );
    assert!(
        matches!(second.with_transition(Transition::new("foreign").with_effect(local.set(true))).build(), Err(error) if matches!(error.kind, ErrorKind::ForeignHandle(_)))
    );
}

#[test]
fn foreign_states_fail_every_state_consuming_operation() {
    let mut first = ModelBuilder::new("same").unwrap();
    let first_handle = first.fact::<Learned>().unwrap();
    let first = first.build().unwrap();
    let mut second = ModelBuilder::new("same").unwrap();
    let handle = second.fact::<Learned>().unwrap();
    let second = second.build().unwrap();
    let foreign = first.state([first_handle.set(false)]).unwrap();
    assert_eq!(
        second.snapshot(&foreign).unwrap_err().kind,
        ErrorKind::ForeignState
    );
    assert_eq!(
        second
            .evaluate(&foreign, &handle.is(false))
            .unwrap_err()
            .kind,
        ErrorKind::ForeignState
    );
    assert_eq!(
        second.apply(&foreign, "anything").unwrap_err().kind,
        ErrorKind::ForeignState
    );
    assert_eq!(
        second
            .reachability(&foreign, &handle.is(false), SEARCH)
            .unwrap_err()
            .kind,
        ErrorKind::ForeignState
    );
    assert_eq!(
        second.check_invariants(&foreign, SEARCH).unwrap_err().kind,
        ErrorKind::ForeignState
    );
}

#[test]
fn duplicate_keys_transition_ids_effects_and_settings_are_rejected() {
    let mut builder = ModelBuilder::new("duplicate").unwrap();
    let learned = builder.fact::<Learned>().unwrap();
    assert!(
        matches!(builder.fact::<Learned>(), Err(error) if matches!(error.kind, ErrorKind::DuplicateId { kind: "fact", .. }))
    );
    assert!(
        matches!(builder.with_transition(Transition::new("again")).with_transition(Transition::new("again")).build(), Err(error) if matches!(error.kind, ErrorKind::DuplicateId { kind: "transition", .. }))
    );
    let mut builder = ModelBuilder::new("writes").unwrap();
    let handle = builder.fact::<Learned>().unwrap();
    assert!(
        matches!(builder.with_transition(Transition::new("twice").with_effect(handle.set(true)).with_effect(handle.set(false))).build(), Err(error) if matches!(error.kind, ErrorKind::DuplicateWrite(_)))
    );
    let mut builder = ModelBuilder::new("settings").unwrap();
    let setting = builder.fact::<Locked>().unwrap();
    assert!(
        matches!(builder.with_transition(Transition::new("setting").with_guard(Predicate::constant(false)).with_effect(setting.set(true))).build(), Err(error) if matches!(error.kind, ErrorKind::SettingWrite(_)))
    );
    assert_eq!(learned.set(false).fact(), "learned");
}

#[test]
fn symbolic_domains_are_finite_and_undeclared_values_fail_at_every_boundary() {
    let mut builder = ModelBuilder::new("colours").unwrap();
    let paint = builder.fact::<Paint>().unwrap();
    let model = builder.build().unwrap();
    assert_eq!(
        model.facts().first().expect("missing paint").domain.len(),
        2
    );
    let initial = model.state([paint.set(Colour::Red)]).unwrap();
    assert!(
        matches!(model.state([paint.set(Colour::Undeclared)]), Err(error) if matches!(error.kind, ErrorKind::InvalidValue { .. }))
    );
    assert!(
        matches!(model.evaluate(&initial, &paint.is(Colour::Undeclared)), Err(error) if matches!(error.kind, ErrorKind::InvalidValue { .. }))
    );
    assert!(
        matches!(model.classify(&paint.is(Colour::Undeclared), &[], CLASSIFY), Err(error) if matches!(error.kind, ErrorKind::InvalidValue { .. }))
    );
    let mut builder = ModelBuilder::new("invalid-effect").unwrap();
    let paint = builder.fact::<Paint>().unwrap();
    assert!(
        matches!(builder.with_transition(Transition::new("bad").with_effect(paint.set(Colour::Undeclared))).build(), Err(error) if matches!(error.kind, ErrorKind::InvalidValue { .. }))
    );
}

#[derive(Clone)]
struct Empty;
impl Domain for Empty {
    fn values() -> Vec<Self> {
        Vec::new()
    }
    fn encode(self) -> Value {
        Value::Bool(false)
    }
}
impl Fact for Empty {
    type Value = Self;
    const KEY: &'static str = "empty";
}

#[derive(Clone)]
struct Duplicate;
impl Domain for Duplicate {
    fn values() -> Vec<Self> {
        vec![Self, Self]
    }
    fn encode(self) -> Value {
        Value::Bool(false)
    }
}
impl Fact for Duplicate {
    type Value = Self;
    const KEY: &'static str = "duplicate";
}

#[derive(Clone)]
struct Blank;
impl Domain for Blank {
    fn values() -> Vec<Self> {
        vec![Self]
    }
    fn encode(self) -> Value {
        Value::Symbol(String::new())
    }
}
impl Fact for Blank {
    type Value = Self;
    const KEY: &'static str = "blank";
}

#[test]
fn invalid_domains_fail_without_registering_partial_facts() {
    let mut builder = ModelBuilder::new("domains").unwrap();
    assert!(
        matches!(builder.fact::<Empty>(), Err(error) if matches!(error.kind, ErrorKind::InvalidDomain { reason: "empty domain", .. }))
    );
    assert!(
        matches!(builder.fact::<Duplicate>(), Err(error) if matches!(error.kind, ErrorKind::InvalidDomain { reason: "duplicate encoding", .. }))
    );
    assert!(
        matches!(builder.fact::<Blank>(), Err(error) if matches!(error.kind, ErrorKind::InvalidDomain { reason: "empty symbolic encoding", .. }))
    );
    let valid = builder.fact::<Learned>().unwrap();
    let model = builder.build().unwrap();
    assert_eq!(model.facts().len(), 1);
    assert!(
        model
            .evaluate(&model.state([valid.set(true)]).unwrap(), &valid.is(true))
            .unwrap()
    );
}

#[test]
fn hypotheses_use_only_explicit_premises_and_check_consistency_first() {
    let mut builder = ModelBuilder::new("hypotheses").unwrap();
    let learned = builder.fact::<Learned>().unwrap();
    let model = builder
        .with_invariant(Invariant::new("not-imported", learned.is(false)))
        .build()
        .unwrap();
    let hypothesis = learned.is(true);
    let possible = model.classify(&hypothesis, &[], CLASSIFY).unwrap();
    assert_eq!(possible.conclusion, HypothesisConclusion::Possible);
    assert!(possible.witness.is_some());
    assert!(possible.counterexample.is_some());
    assert_eq!(possible.coverage.evaluated_candidates, 2);
    let required = model
        .classify(
            &hypothesis,
            &[Premise::new("known", learned.is(true))],
            CLASSIFY,
        )
        .unwrap();
    assert_eq!(required.conclusion, HypothesisConclusion::Required);
    assert!(required.coverage.exhausted);
    assert!(required.counterexample.is_none());
    let ruled_out = model
        .classify(
            &hypothesis,
            &[Premise::new("unknown", learned.is(false))],
            CLASSIFY,
        )
        .unwrap();
    assert_eq!(ruled_out.conclusion, HypothesisConclusion::RuledOut);
    assert!(ruled_out.coverage.exhausted);
    let contradictory = [
        Premise::new("yes", learned.is(true)),
        Premise::new("no", learned.is(false)),
    ];
    for hypothesis in [
        hypothesis,
        Predicate::constant(true),
        Predicate::constant(false),
    ] {
        let inconsistent = model
            .classify(&hypothesis, &contradictory, CLASSIFY)
            .unwrap();
        assert_eq!(inconsistent.conclusion, HypothesisConclusion::Inconsistent);
        assert_eq!(inconsistent.coverage.satisfying_candidates, 0);
        assert!(inconsistent.witness.is_none());
        assert!(inconsistent.counterexample.is_none());
    }
}

#[test]
fn classification_budgets_distinguish_existence_from_exhaustive_claims() {
    let mut builder = ModelBuilder::new("candidate-bounds").unwrap();
    let learned = builder.fact::<Learned>().unwrap();
    let model = builder.build().unwrap();
    let partial = ClassificationLimits { max_candidates: 1 };
    let possible = model.classify(&learned.is(false), &[], partial).unwrap();
    assert_eq!(possible.conclusion, HypothesisConclusion::Possible);
    assert!(possible.witness.is_some());
    assert!(possible.counterexample.is_none());
    assert!(!possible.coverage.exhausted);
    let inconclusive = model.classify(&learned.is(true), &[], partial).unwrap();
    assert_eq!(inconclusive.conclusion, HypothesisConclusion::Inconclusive);
    assert!(inconclusive.counterexample.is_some());
    let premise_not_seen = model
        .classify(
            &learned.is(true),
            &[Premise::new("true", learned.is(true))],
            partial,
        )
        .unwrap();
    assert_eq!(
        premise_not_seen.conclusion,
        HypothesisConclusion::Inconclusive
    );
    assert_eq!(premise_not_seen.coverage.satisfying_candidates, 0);
    let zero = model
        .classify(
            &learned.is(true),
            &[],
            ClassificationLimits { max_candidates: 0 },
        )
        .unwrap();
    assert_eq!(zero.conclusion, HypothesisConclusion::Inconclusive);
    assert_eq!(zero.coverage.evaluated_candidates, 0);
    assert!(!zero.coverage.exhausted);
    let exact = model
        .classify(
            &Predicate::constant(true),
            &[],
            ClassificationLimits { max_candidates: 2 },
        )
        .unwrap();
    assert_eq!(exact.conclusion, HypothesisConclusion::Required);
    assert!(exact.coverage.exhausted);
}

#[test]
fn cartesian_enumeration_covers_every_symbol_and_setting_combination() {
    let mut builder = ModelBuilder::new("product").unwrap();
    let paint = builder.fact::<Paint>().unwrap();
    let locked = builder.fact::<Locked>().unwrap();
    let model = builder.build().unwrap();
    let hypothesis = paint.is(Colour::Blue).and(locked.is(true));
    let report = model
        .classify(&hypothesis, &[], ClassificationLimits { max_candidates: 4 })
        .unwrap();
    assert_eq!(report.coverage.evaluated_candidates, 4);
    assert_eq!(report.coverage.satisfying_candidates, 4);
    assert!(report.coverage.exhausted);
    assert_eq!(report.conclusion, HypothesisConclusion::Possible);
    let witness = report.witness.expect("missing Cartesian witness");
    assert_eq!(
        witness.values.get("paint"),
        Some(&Value::Symbol("blue".to_owned()))
    );
    assert_eq!(witness.values.get("locked"), Some(&Value::Bool(true)));
}

#[test]
fn zero_fact_model_has_one_empty_assignment_and_a_closed_reachable_state() {
    let model = ModelBuilder::new("empty-model").unwrap().build().unwrap();
    let initial = model.state([]).unwrap();
    let report = model
        .classify(
            &Predicate::constant(true),
            &[],
            ClassificationLimits { max_candidates: 1 },
        )
        .unwrap();
    assert_eq!(report.conclusion, HypothesisConclusion::Required);
    assert_eq!(report.coverage.evaluated_candidates, 1);
    assert!(report.coverage.exhausted);
    assert!(
        report
            .witness
            .expect("missing empty assignment")
            .values
            .is_empty()
    );
    let unreachable = model
        .reachability(
            &initial,
            &Predicate::constant(false),
            SearchLimits {
                max_states: 1,
                max_depth: 0,
            },
        )
        .unwrap();
    assert_eq!(unreachable.conclusion, ReachabilityConclusion::Unreachable);
    assert!(unreachable.coverage.exhausted);
}

#[test]
fn names_and_premise_identifiers_are_checked() {
    assert!(
        matches!(ModelBuilder::new("  "), Err(error) if matches!(error.kind, ErrorKind::EmptyName("context")))
    );
    let mut builder = ModelBuilder::new("names").unwrap();
    let learned = builder.fact::<Learned>().unwrap();
    let model = builder.build().unwrap();
    let duplicate = [
        Premise::new("same", learned.is(true)),
        Premise::new("same", learned.is(false)),
    ];
    assert!(
        matches!(model.classify(&learned.is(true), &duplicate, CLASSIFY), Err(error) if matches!(error.kind, ErrorKind::DuplicateId { kind: "premise", .. }))
    );
    let blank = [Premise::new("", learned.is(true))];
    assert!(
        matches!(model.classify(&learned.is(true), &blank, CLASSIFY), Err(error) if matches!(error.kind, ErrorKind::EmptyName("premise")))
    );
    assert!(
        matches!(ModelBuilder::new("transition").unwrap().with_transition(Transition::new("")).build(), Err(error) if matches!(error.kind, ErrorKind::EmptyName("transition")))
    );
    assert!(
        matches!(ModelBuilder::new("invariant").unwrap().with_invariant(Invariant::new("", Predicate::constant(true))).build(), Err(error) if matches!(error.kind, ErrorKind::EmptyName("invariant")))
    );
}

#[test]
fn predicates_remain_inspectable_and_implication_uses_shared_evaluator() {
    let mut builder = ModelBuilder::new("inspection").unwrap();
    let learned = builder.fact::<Learned>().unwrap();
    let ending = builder.fact::<Ending>().unwrap();
    let predicate = learned.is(true).implies(ending.is(true));
    assert!(matches!(predicate.kind, PredicateKind::Or(_, _)));
    assert!(predicate.provenance.file.ends_with("core.rs"));
    let model = builder.build().unwrap();
    let report = model
        .classify(
            &predicate,
            &[
                Premise::new("learned", learned.is(true)),
                Premise::new("unfinished", ending.is(false)),
            ],
            CLASSIFY,
        )
        .unwrap();
    assert_eq!(report.conclusion, HypothesisConclusion::RuledOut);
    assert!(report.hypothesis.provenance.file.ends_with("core.rs"));
    assert_eq!(report.premises.len(), 2);
    let assignment = learned.set(true);
    assert_eq!(assignment.value(), &Value::Bool(true));
    assert!(assignment.provenance().file.ends_with("core.rs"));
}

#[cfg(test)]
#[track_caller]
fn assert_caller_sources(model: &Model, handle: &FactHandle<Learned>) {
    let caller = Location::caller();
    let expected = Source {
        portable_file: Some(caller.file().to_owned()),
        file: caller.file().to_owned(),
        line: caller.line(),
        column: caller.column(),
    };
    assert_eq!(handle.is(true).provenance, expected);
    assert_eq!(*handle.set(true).provenance(), expected);
    assert_eq!(Predicate::constant(true).provenance, expected);
    assert_eq!(model.state([]).unwrap_err().provenance, expected);
    let state = model.state([handle.set(false)]).unwrap();
    assert_eq!(
        model.apply(&state, "missing").unwrap_err().provenance,
        expected
    );
    let error = ModelBuilder::new("").unwrap_err();
    assert_eq!(error.provenance, expected);
}

#[test]
fn tracked_sources_include_exact_external_line_and_column() {
    let mut builder = ModelBuilder::new("provenance").unwrap();
    let handle = builder.fact::<Learned>().unwrap();
    let model = builder.build().unwrap();
    assert_caller_sources(&model, &handle);
}

#[test]
fn compilation_rejects_foreign_guards_invariants_and_premises() {
    let mut other = ModelBuilder::new("other").unwrap();
    let foreign = other.fact::<Learned>().unwrap();
    let guard_error = ModelBuilder::new("guard")
        .unwrap()
        .with_transition(Transition::new("bad").with_guard(foreign.is(true)))
        .build()
        .unwrap_err();
    assert!(matches!(guard_error.kind, ErrorKind::ForeignHandle(_)));
    let invariant_error = ModelBuilder::new("invariant")
        .unwrap()
        .with_invariant(Invariant::new("bad", foreign.is(true)))
        .build()
        .unwrap_err();
    assert!(matches!(invariant_error.kind, ErrorKind::ForeignHandle(_)));
    let model = ModelBuilder::new("premise").unwrap().build().unwrap();
    let premise_error = model
        .classify(
            &Predicate::constant(false),
            &[Premise::new("foreign", foreign.is(true))],
            ClassificationLimits { max_candidates: 0 },
        )
        .unwrap_err();
    assert!(matches!(premise_error.kind, ErrorKind::ForeignHandle(_)));
    let duplicate = ModelBuilder::new("duplicate-invariant")
        .unwrap()
        .with_invariant(Invariant::new("same", Predicate::constant(true)))
        .with_invariant(Invariant::new("same", Predicate::constant(false)))
        .build()
        .unwrap_err();
    assert!(matches!(
        duplicate.kind,
        ErrorKind::DuplicateId {
            kind: "invariant",
            ..
        }
    ));
}

#[test]
fn classification_reports_are_deterministic_and_partial_inconsistency_is_unproven() {
    let mut builder = ModelBuilder::new("consistent-evidence").unwrap();
    let handle = builder.fact::<Learned>().unwrap();
    let model = builder.build().unwrap();
    let hypothesis = handle.is(true);
    let premises = [
        Premise::new("true", handle.is(true)),
        Premise::new("false", handle.is(false)),
    ];
    let partial = model
        .classify(
            &hypothesis,
            &premises,
            ClassificationLimits { max_candidates: 1 },
        )
        .unwrap();
    assert_eq!(partial.conclusion, HypothesisConclusion::Inconclusive);
    assert!(!partial.coverage.exhausted);
    let first = model.classify(&hypothesis, &premises, CLASSIFY).unwrap();
    let second = model.classify(&hypothesis, &premises, CLASSIFY).unwrap();
    assert_eq!(first, second);
    assert_eq!(
        serde_json::to_string(&first).unwrap(),
        serde_json::to_string(&second).unwrap()
    );
    assert_eq!(first.conclusion, HypothesisConclusion::Inconsistent);
}
