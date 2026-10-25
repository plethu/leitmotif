use super::{LIMITS, facts, replay};
use leitmotif::{
    Domain, ErrorKind, Fact, FactKind, ModelBuilder, Predicate, ReachabilityConclusion,
    SearchLimits, Transition, Value,
};

#[test]
fn selected_projection_noop_does_not_truncate_at_zero_depth_or_one_state() {
    let (builder, goal, gate, permit, unrelated) = facts().unwrap();
    let model = builder
        .with_transition(
            Transition::new("noop")
                .with_effect(goal.set(false))
                .with_effect(unrelated.set(true)),
        )
        .build()
        .unwrap();
    let initial = model
        .state([
            goal.set(false),
            gate.set(false),
            permit.set(false),
            unrelated.set(false),
        ])
        .unwrap();
    let report = model
        .sliced_reachability(
            &initial,
            &goal.is(true),
            SearchLimits {
                max_states: 1,
                max_depth: 0,
            },
        )
        .unwrap();
    assert_eq!(report.conclusion, ReachabilityConclusion::Unreachable);
    assert!(report.coverage.exhausted);
    assert!(!report.coverage.state_limit_hit);
    assert!(!report.coverage.depth_limit_hit);
    assert_eq!(report.coverage.evaluated_projections, 1);
}

#[test]
fn constant_queries_respect_zero_and_positive_budgets_without_retaining_facts() {
    let (builder, goal, gate, permit, unrelated) = facts().unwrap();
    let model = builder
        .with_transition(Transition::new("change").with_effect(goal.set(true)))
        .build()
        .unwrap();
    let initial = model
        .state([
            goal.set(false),
            gate.set(false),
            permit.set(true),
            unrelated.set(true),
        ])
        .unwrap();
    for truth in [false, true] {
        for budget in [0, 1] {
            let target = Predicate::constant(truth);
            let report = model
                .sliced_reachability(
                    &initial,
                    &target,
                    SearchLimits {
                        max_states: budget,
                        max_depth: 0,
                    },
                )
                .unwrap();
            assert!(report.retained_facts.is_empty());
            assert!(report.retained_transitions.is_empty());
            assert_eq!(report.coverage.discovered_projections, budget);
            match report.conclusion {
                ReachabilityConclusion::Reached { witness } => {
                    assert!(truth && budget == 1);
                    assert!(witness.steps.is_empty());
                    replay(&model, &initial, &target, &witness).unwrap();
                }
                ReachabilityConclusion::Unreachable => {
                    assert!(!truth && budget == 1);
                    assert!(report.coverage.exhausted);
                }
                ReachabilityConclusion::Inconclusive => {
                    assert_eq!(budget, 0);
                    assert!(report.coverage.state_limit_hit);
                }
            }
        }
    }
}

#[test]
fn projected_and_full_budgets_can_give_different_evidence_without_false_proofs() {
    let (builder, goal, gate, permit, unrelated) = facts().unwrap();
    let model = builder
        .with_transition(Transition::new("noise").with_effect(unrelated.set(true)))
        .with_transition(Transition::new("open").with_effect(gate.set(true)))
        .with_transition(
            Transition::new("finish")
                .with_guard(gate.is(true))
                .with_effect(goal.set(true)),
        )
        .build()
        .unwrap();
    let initial = model
        .state([
            goal.set(false),
            gate.set(false),
            permit.set(false),
            unrelated.set(false),
        ])
        .unwrap();
    let limits = SearchLimits {
        max_states: 3,
        max_depth: 2,
    };
    assert_eq!(
        model
            .reachability(&initial, &goal.is(true), limits)
            .unwrap()
            .conclusion,
        ReachabilityConclusion::Inconclusive
    );
    let report = model
        .sliced_reachability(&initial, &goal.is(true), limits)
        .unwrap();
    let ReachabilityConclusion::Reached { witness } = report.conclusion else {
        panic!("projection admits ending")
    };
    replay(&model, &initial, &goal.is(true), &witness).unwrap();
    for limits in [
        SearchLimits {
            max_states: 2,
            max_depth: 8,
        },
        SearchLimits {
            max_states: 8,
            max_depth: 1,
        },
    ] {
        let report = model
            .sliced_reachability(&initial, &goal.is(true), limits)
            .unwrap();
        assert_eq!(report.conclusion, ReachabilityConclusion::Inconclusive);
        assert!(!report.coverage.exhausted);
        assert!(report.coverage.state_limit_hit || report.coverage.depth_limit_hit);
    }
}

#[derive(Clone, Copy)]
enum SettingValue {
    Declared,
    Invalid,
}
impl Domain for SettingValue {
    fn values() -> Vec<Self> {
        vec![Self::Declared]
    }
    fn encode(self) -> Value {
        Value::Symbol(
            match self {
                Self::Declared => "declared",
                Self::Invalid => "invalid",
            }
            .into(),
        )
    }
}
struct Setting;
impl Fact for Setting {
    type Value = SettingValue;
    const KEY: &'static str = "setting";
    const KIND: FactKind = FactKind::Setting;
}

#[test]
fn validation_covers_dead_branches_zero_budget_and_foreign_initials() {
    let mut builder = ModelBuilder::new("validation").unwrap();
    let setting = builder.fact::<Setting>().unwrap();
    let model = builder.build().unwrap();
    let initial = model.state([setting.set(SettingValue::Declared)]).unwrap();
    let mut other = ModelBuilder::new("foreign").unwrap();
    let foreign = other.fact::<Setting>().unwrap();
    let other = other.build().unwrap();
    let foreign_initial = other.state([foreign.set(SettingValue::Declared)]).unwrap();
    for max_states in [0, 1] {
        let limits = SearchLimits {
            max_states,
            max_depth: 0,
        };
        for target in [
            Predicate::constant(true).or(foreign.is(SettingValue::Declared)),
            Predicate::constant(false).and(foreign.is(SettingValue::Declared)),
        ] {
            assert!(matches!(
                model
                    .sliced_reachability(&initial, &target, limits)
                    .unwrap_err()
                    .kind,
                ErrorKind::ForeignHandle(_)
            ));
        }
        let target = Predicate::constant(true).or(setting.is(SettingValue::Invalid));
        assert!(matches!(
            model
                .sliced_reachability(&initial, &target, limits)
                .unwrap_err()
                .kind,
            ErrorKind::InvalidValue { .. }
        ));
        assert_eq!(
            model
                .sliced_reachability(&foreign_initial, &Predicate::constant(true), limits)
                .unwrap_err()
                .kind,
            ErrorKind::ForeignState
        );
    }
    let report = model
        .sliced_reachability(&initial, &setting.is(SettingValue::Declared), LIMITS)
        .unwrap();
    assert_eq!(report.retained_facts, ["setting"]);
    assert!(report.retained_transitions.is_empty());
}
