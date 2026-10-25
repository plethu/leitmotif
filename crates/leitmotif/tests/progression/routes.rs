use super::Location;
use super::{Fixture, LIMITS, Place};
use leitmotif::{
    ModelBuilder, PermittedEnding, ProgressionConclusion, ProgressionQuery, Transition,
};

#[test]
fn a_cycle_with_an_escape_is_not_a_trap_but_a_closed_branch_is() {
    let mut builder = ModelBuilder::new("progression").unwrap();
    let location = builder.fact::<Location>().unwrap();
    let model = builder
        .with_transition(
            Transition::new("enter_loop")
                .with_guard(location.is(Place::Start))
                .with_effect(location.set(Place::Loop)),
        )
        .with_transition(
            Transition::new("return")
                .with_guard(location.is(Place::Loop))
                .with_effect(location.set(Place::Start)),
        )
        .with_transition(
            Transition::new("exit")
                .with_guard(location.is(Place::Start))
                .with_effect(location.set(Place::Exit)),
        )
        .with_transition(
            Transition::new("trap")
                .with_guard(location.is(Place::Start))
                .with_effect(location.set(Place::Trap)),
        )
        .with_transition(
            Transition::new("stay_trapped")
                .with_guard(location.is(Place::Trap))
                .with_effect(location.set(Place::Trap)),
        )
        .build()
        .unwrap();
    let initial = model.state([location.set(Place::Start)]).unwrap();
    let query = ProgressionQuery::new(
        location.is(Place::Exit),
        [PermittedEnding::new("finish", location.is(Place::Exit))],
    )
    .unwrap();
    let report = model.progression(&initial, &query, LIMITS).unwrap();
    assert!(report.coverage.exhausted);
    assert_eq!(report.coverage.evaluated_states, 4);
    let ProgressionConclusion::Trap {
        entry,
        trapped_states,
    } = report.conclusion
    else {
        panic!("expected the closed branch")
    };
    assert_eq!(trapped_states, 1);
    assert_eq!(entry.steps.len(), 1);
    let step = entry.steps.first().unwrap();
    assert_eq!(step.transition.id, "trap");
    let entered = model.apply(&initial, &step.transition.id).unwrap().unwrap();
    assert_eq!(model.snapshot(&entered).unwrap(), step.state);
    assert!(model.evaluate(&entered, &location.is(Place::Trap)).unwrap());
}

#[test]
fn escapable_cycles_and_converging_routes_have_no_traps() {
    let f = Fixture::new(
        Place::Start,
        &[
            ("loop", Place::Start, Place::Loop),
            ("back", Place::Loop, Place::Start),
            ("other", Place::Start, Place::Trap),
            ("join", Place::Trap, Place::Loop),
            ("finish", Place::Start, Place::Exit),
        ],
    )
    .unwrap();
    let query = f.query(&[Place::Exit], &[("finish", Place::Exit)]).unwrap();
    let report = f.model.progression(&f.initial, &query, LIMITS).unwrap();
    assert_eq!(report.conclusion, ProgressionConclusion::NoTraps);
    assert!(report.coverage.exhausted);
    assert_eq!(report.coverage.discovered_states, 4);
}

#[test]
fn intentional_failure_can_finish_after_another_ending_is_locked_out() {
    let f = Fixture::new(
        Place::Start,
        &[("accept_failure", Place::Start, Place::Failure)],
    )
    .unwrap();
    let query = f
        .query(
            &[Place::Exit, Place::Failure],
            &[
                ("success", Place::Exit),
                ("intentional_failure", Place::Failure),
            ],
        )
        .unwrap();
    let report = f.model.progression(&f.initial, &query, LIMITS).unwrap();
    assert_eq!(report.conclusion, ProgressionConclusion::NoTraps);
    assert_eq!(
        report
            .permitted_endings
            .iter()
            .map(|ending| ending.id.as_str())
            .collect::<Vec<_>>(),
        ["success", "intentional_failure"]
    );
    assert_eq!(report.coverage.evaluated_states, 2);
}

#[test]
fn routes_cannot_pass_through_a_disallowed_terminal() {
    let f = Fixture::new(
        Place::Start,
        &[
            ("fail", Place::Start, Place::Failure),
            ("restart", Place::Failure, Place::Exit),
        ],
    )
    .unwrap();
    let query = f
        .query(&[Place::Exit, Place::Failure], &[("success", Place::Exit)])
        .unwrap();
    let report = f.model.progression(&f.initial, &query, LIMITS).unwrap();
    assert_eq!(report.coverage.discovered_states, 2);
    assert!(report.coverage.exhausted);
    let ProgressionConclusion::Trap {
        entry,
        trapped_states,
    } = report.conclusion
    else {
        panic!("terminal failure cannot be traversed")
    };
    assert!(entry.steps.is_empty());
    assert_eq!(trapped_states, 1);
}

#[test]
fn closed_cycles_and_initial_dead_ends_produce_empty_entry_witnesses() {
    for edges in [
        vec![],
        vec![
            ("loop", Place::Start, Place::Loop),
            ("back", Place::Loop, Place::Start),
        ],
    ] {
        let expected = if edges.is_empty() { 1 } else { 2 };
        let f = Fixture::new(Place::Start, &edges).unwrap();
        let query = f.query(&[Place::Exit], &[("finish", Place::Exit)]).unwrap();
        let report = f.model.progression(&f.initial, &query, LIMITS).unwrap();
        let ProgressionConclusion::Trap {
            entry,
            trapped_states,
        } = report.conclusion
        else {
            panic!("expected an initial progression trap")
        };
        assert!(entry.steps.is_empty());
        assert_eq!(entry.initial, f.model.snapshot(&f.initial).unwrap());
        assert_eq!(trapped_states, expected);
    }
}

#[test]
fn empty_permitted_sets_exclude_terminals_from_the_trap_count() {
    let edges = [
        ("step", Place::Start, Place::Loop),
        ("end", Place::Loop, Place::Exit),
        ("after", Place::Exit, Place::Trap),
    ];
    let f = Fixture::new(Place::Start, &edges).unwrap();
    let report = f
        .model
        .progression(&f.initial, &f.query(&[Place::Exit], &[]).unwrap(), LIMITS)
        .unwrap();
    assert!(matches!(
        report.conclusion,
        ProgressionConclusion::Trap {
            trapped_states: 2,
            ..
        }
    ));
    let terminal = f.model.state([f.location.set(Place::Exit)]).unwrap();
    let report = f
        .model
        .progression(&terminal, &f.query(&[Place::Exit], &[]).unwrap(), LIMITS)
        .unwrap();
    assert_eq!(report.conclusion, ProgressionConclusion::NoTraps);
    assert_eq!(report.coverage.evaluated_states, 1);
}
