use leitmotif::{
    Domain, Fact, FactHandle, FactKind, Invariant, Model, ModelBuilder, SearchLimits, State,
    Transition, Value,
};
use std::collections::BTreeMap;

use oxidd::{BooleanFunction, bdd::BDDFunction};

use super::{Encoding, Order, Oxidd, explore};

use super::search::Conclusion;

#[derive(Clone, Copy)]
enum Phase {
    Idle,
    Ready,
    Done,
}
impl Domain for Phase {
    fn values() -> Vec<Self> {
        vec![Self::Idle, Self::Ready, Self::Done]
    }
    fn encode(self) -> Value {
        Value::Symbol(
            match self {
                Self::Idle => "idle",
                Self::Ready => "ready",
                Self::Done => "done",
            }
            .to_owned(),
        )
    }
}
struct Progress;
impl Fact for Progress {
    type Value = Phase;
    const KEY: &'static str = "progress";
}
struct Flag;
impl Fact for Flag {
    type Value = bool;
    const KEY: &'static str = "flag";
}
struct Setting;
impl Fact for Setting {
    type Value = bool;
    const KEY: &'static str = "setting";
    const KIND: FactKind = FactKind::Setting;
}

struct Contract {
    model: Model,
    progress: FactHandle<Progress>,
    flag: FactHandle<Flag>,
    setting: FactHandle<Setting>,
}

fn contract() -> Contract {
    let mut builder = ModelBuilder::new("encoding_contract").unwrap();
    let progress = builder.fact::<Progress>().unwrap();
    let flag = builder.fact::<Flag>().unwrap();
    let setting = builder.fact::<Setting>().unwrap();
    let model = builder
        .with_transition(
            Transition::new("advance")
                .with_guard(progress.is(Phase::Idle).and(setting.is(true)))
                .with_effect(progress.set(Phase::Ready))
                .with_effect(flag.set(true)),
        )
        .with_transition(
            Transition::new("finish")
                .with_guard(progress.is(Phase::Ready).and(flag.is(true)))
                .with_effect(progress.set(Phase::Done)),
        )
        .with_invariant(Invariant::new("flag_stays_false", flag.is(false)))
        .build()
        .unwrap();
    Contract {
        model,
        progress,
        flag,
        setting,
    }
}

fn complete_assignments(fixture: &Contract) -> Vec<State> {
    let mut states = Vec::new();
    for phase in Phase::values() {
        for flag in [false, true] {
            for configured in [false, true] {
                states.push(
                    fixture
                        .model
                        .state([
                            fixture.progress.set(phase),
                            fixture.flag.set(flag),
                            fixture.setting.set(configured),
                        ])
                        .unwrap(),
                );
            }
        }
    }
    states
}

fn check_relations(backend: &Oxidd, encoding: &Encoding<'_>, fixture: &Contract) {
    let states = complete_assignments(fixture);
    for (id, relation) in encoding.relations(backend, &fixture.model).unwrap() {
        for state in &states {
            check_successor(backend, encoding, &fixture.model, state, &id, &relation);
        }
    }
}

fn check_successor(
    backend: &Oxidd,
    encoding: &Encoding<'_>,
    model: &Model,
    state: &State,
    id: &str,
    relation: &BDDFunction,
) {
    let current = encoding
        .snapshot(backend, &model.snapshot(state).unwrap(), false)
        .unwrap();
    let image = backend
        .exists(&current.and(relation).unwrap(), &encoding.current)
        .unwrap();
    let image = backend.rename(&image, &encoding.pairs()).unwrap();
    let Some(expected) = model.apply(state, id).unwrap() else {
        assert!(!image.satisfiable());
        return;
    };
    let expected = encoding
        .snapshot(backend, &model.snapshot(&expected).unwrap(), false)
        .unwrap();
    let difference = (image.equiv(&expected).unwrap()).not().unwrap();
    assert!(!difference.satisfiable());
}

fn exhaustive(backend: &Oxidd, order: Order) {
    let fixture = contract();
    let Contract {
        model,
        progress,
        flag,
        setting,
    } = &fixture;
    let encoding = Encoding::new(model, order).unwrap();
    check_relations(backend, &encoding, &fixture);
    // All 3×2×2 legal states, with every primed bit free, normalize to 12.
    let count = encoding
        .count(&encoding.legal(backend, false).unwrap())
        .unwrap();
    assert!((count - 12.0).abs() < f64::EPSILON);
    let invalid = vec![true; usize::try_from(encoding.variables).unwrap()];
    assert!(encoding.decode(&invalid).is_err());
    let mut invalid_cube = backend.constant(true);
    for id in 0..encoding.variables {
        invalid_cube = invalid_cube.and(&backend.variable(id).unwrap()).unwrap();
    }
    for prime in [false, true] {
        assert!(
            !((invalid_cube)
                .and(&encoding.legal(backend, prime).unwrap())
                .unwrap())
            .satisfiable()
        );
    }
    let initial = model
        .state([
            progress.set(Phase::Idle),
            flag.set(false),
            setting.set(true),
        ])
        .unwrap();
    let outcome = explore(
        backend,
        &encoding,
        model,
        &initial,
        &progress.is(Phase::Done),
        4,
    )
    .unwrap();
    assert_eq!(outcome.conclusion, Conclusion::Reached);
    assert_eq!(
        outcome
            .witness
            .as_ref()
            .unwrap()
            .steps
            .iter()
            .map(|step| step.transition.id.as_str())
            .collect::<Vec<_>>(),
        ["advance", "finish"]
    );
    let explicit = model
        .check_invariants(
            &initial,
            SearchLimits {
                max_states: 32,
                max_depth: 4,
            },
        )
        .unwrap();
    assert!(matches!(
        explicit.conclusion,
        leitmotif::InvariantConclusion::Violation { .. }
    ));
    assert_eq!(
        outcome.invariants.first().unwrap().reachable_violation,
        Some(true)
    );
}

#[test]
fn oxidd_encoding_matches_every_small_reference_assignment() {
    exhaustive(&Oxidd::new(8, 10_000).unwrap(), Order::Paired);
    exhaustive(&Oxidd::new(8, 10_000).unwrap(), Order::Banked);
}

use super::super::scaling_town::{Parameters, Resource, Town};
use leitmotif::{Predicate, ReachabilityConclusion};

fn compare_town(backend: &Oxidd, encoding: &Encoding<'_>, town: &Town) {
    let limits = SearchLimits {
        max_states: 1024,
        max_depth: 64,
    };
    let reference = town
        .model
        .reachability(&town.initial, &town.unreachable, limits)
        .unwrap();
    assert!(reference.coverage.exhausted);
    let closure = explore(
        backend,
        encoding,
        &town.model,
        &town.initial,
        &town.unreachable,
        64,
    )
    .unwrap();
    assert_eq!(closure.conclusion, Conclusion::Unreachable);
    assert!(closure.exhausted);
    let expected = f64::from(u32::try_from(reference.coverage.discovered_states).unwrap());
    assert!((closure.reachable_count_estimate - expected).abs() < f64::EPSILON);
    for target in [&town.goal, &town.local] {
        let reference = town
            .model
            .reachability(&town.initial, target, limits)
            .unwrap();
        let symbolic = explore(backend, encoding, &town.model, &town.initial, target, 64).unwrap();
        match reference.conclusion {
            ReachabilityConclusion::Reached { witness } => {
                assert_eq!(symbolic.conclusion, Conclusion::Reached);
                assert_eq!(symbolic.witness.unwrap().steps.len(), witness.steps.len());
            }
            ReachabilityConclusion::Unreachable => {
                assert_eq!(symbolic.conclusion, Conclusion::Unreachable);
            }
            ReachabilityConclusion::Inconclusive => panic!("small reference fixture truncated"),
        }
    }
    let truncated = explore(
        backend,
        encoding,
        &town.model,
        &town.initial,
        &town.unreachable,
        0,
    )
    .unwrap();
    assert_eq!(truncated.conclusion, Conclusion::Inconclusive);
    assert!(!truncated.exhausted);
    let initial_target = explore(
        backend,
        encoding,
        &town.model,
        &town.initial,
        &Predicate::constant(true),
        0,
    )
    .unwrap();
    assert_eq!(initial_target.conclusion, Conclusion::Reached);
    assert!(initial_target.witness.unwrap().steps.is_empty());
}

#[test]
fn small_sparse_and_coupled_towns_match_reference_in_both_orders() {
    for (threads, domain) in [(2, 2), (2, 3), (3, 2), (3, 3), (4, 2), (4, 3)] {
        for resource in [Resource::Unlimited, Resource::SinglePermit] {
            let coupling = if resource == Resource::SinglePermit {
                threads - 1
            } else {
                0
            };
            let town =
                Town::new(Parameters::new(threads, domain, coupling, resource).unwrap()).unwrap();
            for order in [Order::Paired, Order::Banked] {
                let encoding = Encoding::new(&town.model, order).unwrap();
                compare_town(
                    &Oxidd::new(encoding.variables, 100_000).unwrap(),
                    &encoding,
                    &town,
                );
            }
        }
    }
}

#[test]
fn oxidd_resource_limit_is_an_error_never_an_unreachable_conclusion() {
    assert!(matches!(
        Oxidd::new(16, 4),
        Err(super::Failure::Allocation(_))
    ));
    let fixture = contract();
    let encoding = Encoding::new(&fixture.model, Order::Paired).unwrap();
    let backend = Oxidd::new(encoding.variables, 16).unwrap();
    let state = complete_assignments(&fixture).pop().unwrap();
    let result = explore(
        &backend,
        &encoding,
        &fixture.model,
        &state,
        &fixture.progress.is(Phase::Done),
        4,
    );
    assert!(matches!(result, Err(super::Failure::Allocation(_))));
}

#[test]
fn fixed_sparse_work_exhausts_within_its_measured_symbolic_budget() {
    let town = Town::new(Parameters::new(2, 3, 0, Resource::Unlimited).unwrap()).unwrap();
    let encoding = Encoding::new(&town.model, Order::Paired).unwrap();
    let outcome = explore(
        &Oxidd::new(encoding.variables, 100_000).unwrap(),
        &encoding,
        &town.model,
        &town.initial,
        &town.unreachable,
        64,
    )
    .unwrap();
    assert_eq!(outcome.conclusion, Conclusion::Unreachable);
    assert!(outcome.exhausted);
    assert_eq!(outcome.image_iterations, 5);
    assert_eq!(outcome.reachable_nodes, 8);
    assert_eq!(outcome.relation_nodes_sum, 114);
    assert!((outcome.reachable_count_estimate - 9.0).abs() < f64::EPSILON);
}

fn check_boolean_predicate(backend: &Oxidd, encoding: &Encoding<'_>, fixture: &Contract) {
    let query = fixture
        .progress
        .is(Phase::Idle)
        .or(fixture.flag.is(true))
        .and(fixture.setting.is(true).negate());
    let states = complete_assignments(fixture);
    for state in states {
        let description = fixture
            .model
            .reachability(
                &state,
                &query,
                SearchLimits {
                    max_states: 0,
                    max_depth: 0,
                },
            )
            .unwrap()
            .target;
        let predicate = encoding.predicate(backend, &description).unwrap();
        let assignment = encoding
            .snapshot(backend, &fixture.model.snapshot(&state).unwrap(), false)
            .unwrap();
        let actual = (predicate.and(&assignment).unwrap()).satisfiable();
        assert_eq!(actual, fixture.model.evaluate(&state, &query).unwrap());
    }
}

#[test]
fn boolean_query_operators_match_the_reference_on_every_assignment() {
    let fixture = contract();
    for order in [Order::Paired, Order::Banked] {
        let encoding = Encoding::new(&fixture.model, order).unwrap();
        check_boolean_predicate(
            &Oxidd::new(encoding.variables, 10_000).unwrap(),
            &encoding,
            &fixture,
        );
    }
}

#[test]
fn invariant_safety_requires_exhaustion_and_truncation_stays_unknown() {
    let fixture = contract();
    let encoding = Encoding::new(&fixture.model, Order::Paired).unwrap();
    let backend = Oxidd::new(encoding.variables, 10_000).unwrap();
    for (phase, depth, expected) in [(Phase::Idle, 0, None), (Phase::Done, 8, Some(false))] {
        let initial = fixture
            .model
            .state([
                fixture.progress.set(phase),
                fixture.flag.set(false),
                fixture.setting.set(true),
            ])
            .unwrap();
        let result = explore(
            &backend,
            &encoding,
            &fixture.model,
            &initial,
            &Predicate::constant(false),
            depth,
        )
        .unwrap();
        assert_eq!(
            result.invariants.first().unwrap().reachable_violation,
            expected
        );
    }
}

#[test]
fn encoding_rejects_an_empty_universe_and_incomplete_snapshot() {
    let model = ModelBuilder::new("empty").unwrap().build().unwrap();
    assert!(Encoding::new(&model, Order::Paired).is_err());
    let fixture = contract();
    let encoding = Encoding::new(&fixture.model, Order::Paired).unwrap();
    let backend = Oxidd::new(encoding.variables, 10_000).unwrap();
    assert!(
        encoding
            .snapshot(
                &backend,
                &leitmotif::Snapshot {
                    values: BTreeMap::new()
                },
                false
            )
            .is_err()
    );
}
