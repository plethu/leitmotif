use leitmotif::{ProgressionConclusion, SearchLimits};

use super::{Fixture, Place};

#[test]
fn incomplete_graphs_are_inconclusive_even_when_a_closed_trap_was_seen() {
    let f = Fixture::new(
        Place::Start,
        &[
            ("trap", Place::Start, Place::Trap),
            ("other", Place::Start, Place::Loop),
            ("end", Place::Loop, Place::Exit),
        ],
    )
    .unwrap();
    let query = f.query(&[Place::Exit], &[("finish", Place::Exit)]).unwrap();
    for limits in [
        SearchLimits {
            max_states: 0,
            max_depth: 16,
        },
        SearchLimits {
            max_states: 1,
            max_depth: 16,
        },
        SearchLimits {
            max_states: 3,
            max_depth: 16,
        },
        SearchLimits {
            max_states: 16,
            max_depth: 0,
        },
        SearchLimits {
            max_states: 16,
            max_depth: 1,
        },
    ] {
        let report = f.model.progression(&f.initial, &query, limits).unwrap();
        assert_eq!(report.conclusion, ProgressionConclusion::Inconclusive);
        assert!(!report.coverage.exhausted);
        assert_eq!(report.coverage.limits, limits);
        assert_eq!(report.initial, f.model.snapshot(&f.initial).unwrap());
    }
}

#[test]
fn exact_bounds_exhaust_a_cycle_without_demanding_an_extra_state_or_depth() {
    let f = Fixture::new(
        Place::Start,
        &[
            ("step", Place::Start, Place::Loop),
            ("back", Place::Loop, Place::Start),
            ("end", Place::Loop, Place::Exit),
        ],
    )
    .unwrap();
    let query = f.query(&[Place::Exit], &[("finish", Place::Exit)]).unwrap();
    let report = f
        .model
        .progression(
            &f.initial,
            &query,
            SearchLimits {
                max_states: 3,
                max_depth: 2,
            },
        )
        .unwrap();
    assert_eq!(report.conclusion, ProgressionConclusion::NoTraps);
    assert!(report.coverage.exhausted);
    assert!(!report.coverage.state_limit_hit);
    assert!(!report.coverage.depth_limit_hit);
    assert_eq!(report.coverage.deepest_evaluated, Some(2));
    let terminal = f.model.state([f.location.set(Place::Exit)]).unwrap();
    let report = f
        .model
        .progression(
            &terminal,
            &query,
            SearchLimits {
                max_states: 1,
                max_depth: 0,
            },
        )
        .unwrap();
    assert_eq!(report.conclusion, ProgressionConclusion::NoTraps);
    assert!(report.coverage.exhausted);
}
