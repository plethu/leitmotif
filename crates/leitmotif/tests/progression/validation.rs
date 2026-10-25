use leitmotif::{
    ErrorKind, PermittedEnding, Predicate, ProgressionConclusion, ProgressionQuery, SearchLimits,
};

use super::{Fixture, LIMITS, Place};

#[test]
fn visited_nonterminal_ending_matches_return_source_linked_counterexamples() {
    let f = Fixture::new(Place::Start, &[("step", Place::Start, Place::Loop)]).unwrap();
    let line = line!().checked_add(1).unwrap();
    let ending = PermittedEnding::new("not_finished", f.location.is(Place::Loop));
    let query = ProgressionQuery::new(f.location.is(Place::Exit), [ending]).unwrap();
    let error = f.model.progression(&f.initial, &query, LIMITS).unwrap_err();
    assert!(error.provenance.file.ends_with("progression/validation.rs"));
    assert_eq!(error.provenance.line, line);
    let ErrorKind::NonterminalEnding {
        ending,
        counterexample,
    } = error.kind
    else {
        panic!("expected a query mismatch")
    };
    assert_eq!(ending, "not_finished");
    let reached = f.model.apply(&f.initial, "step").unwrap().unwrap();
    assert_eq!(*counterexample, f.model.snapshot(&reached).unwrap());
    let report = f
        .model
        .progression(
            &f.initial,
            &query,
            SearchLimits {
                max_states: 0,
                max_depth: 0,
            },
        )
        .unwrap();
    assert_eq!(report.conclusion, ProgressionConclusion::Inconclusive);
}

#[test]
fn ending_names_must_be_nonblank_and_unique_but_predicates_may_overlap() {
    let terminal = Predicate::constant(true);
    assert!(matches!(
        ProgressionQuery::new(
            terminal.clone(),
            [PermittedEnding::new(" ", terminal.clone())]
        )
        .unwrap_err()
        .kind,
        ErrorKind::EmptyName("permitted ending")
    ));
    let first_line = line!().checked_add(1).unwrap();
    let first = PermittedEnding::new("same", terminal.clone());
    let second_line = line!().checked_add(1).unwrap();
    let second = PermittedEnding::new("same", terminal.clone());
    let error = ProgressionQuery::new(terminal.clone(), [first, second]).unwrap_err();
    assert_eq!(error.provenance.line, second_line);
    assert!(
        matches!(error.kind, ErrorKind::DuplicateId { original, .. } if original.line == first_line)
    );
    let f = Fixture::new(Place::Exit, &[]).unwrap();
    let query = ProgressionQuery::new(
        terminal.clone(),
        [
            PermittedEnding::new("a", terminal.clone()),
            PermittedEnding::new("b", terminal),
        ],
    )
    .unwrap();
    let report = f.model.progression(&f.initial, &query, LIMITS).unwrap();
    assert_eq!(report.conclusion, ProgressionConclusion::NoTraps);
    assert_eq!(report.permitted_endings.len(), 2);
}

#[test]
fn predicates_and_initial_state_are_validated_before_exploration() {
    let f = Fixture::new(Place::Start, &[]).unwrap();
    let foreign = Fixture::new(Place::Start, &[]).unwrap();
    for query in [
        ProgressionQuery::new(foreign.location.is(Place::Exit), []).unwrap(),
        ProgressionQuery::new(
            Predicate::constant(true),
            [PermittedEnding::new(
                "foreign",
                foreign.location.is(Place::Exit),
            )],
        )
        .unwrap(),
    ] {
        assert!(matches!(
            f.model
                .progression(&f.initial, &query, LIMITS)
                .unwrap_err()
                .kind,
            ErrorKind::ForeignHandle(_)
        ));
    }
    let query = f.query(&[Place::Exit], &[("finish", Place::Exit)]).unwrap();
    assert!(matches!(
        f.model
            .progression(&foreign.initial, &query, LIMITS)
            .unwrap_err()
            .kind,
        ErrorKind::ForeignState
    ));
}

#[test]
fn undeclared_values_are_rejected_even_in_unvisited_ending_predicates() {
    let f = Fixture::new(Place::Start, &[]).unwrap();
    for query in [
        ProgressionQuery::new(f.location.is(Place::Undeclared), []).unwrap(),
        ProgressionQuery::new(
            Predicate::constant(true),
            [
                PermittedEnding::new("valid", Predicate::constant(true)),
                PermittedEnding::new("invalid", f.location.is(Place::Undeclared)),
            ],
        )
        .unwrap(),
    ] {
        assert!(matches!(
            f.model
                .progression(
                    &f.initial,
                    &query,
                    SearchLimits {
                        max_states: 0,
                        max_depth: 0
                    }
                )
                .unwrap_err()
                .kind,
            ErrorKind::InvalidValue { .. }
        ));
    }
}

#[test]
fn semantic_ending_validation_checks_only_query_reachable_states() {
    let f = Fixture::new(
        Place::Start,
        &[
            ("finish", Place::Start, Place::Exit),
            ("after", Place::Exit, Place::Trap),
        ],
    )
    .unwrap();
    let query = f
        .query(&[Place::Exit], &[("off_graph", Place::Trap)])
        .unwrap();
    let report = f.model.progression(&f.initial, &query, LIMITS).unwrap();
    assert!(matches!(
        report.conclusion,
        ProgressionConclusion::Trap {
            trapped_states: 1,
            ..
        }
    ));
    assert_eq!(report.coverage.evaluated_states, 2);
}
