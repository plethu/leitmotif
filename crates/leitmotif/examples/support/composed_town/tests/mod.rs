use super::*;
use leitmotif::{ErrorKind, InvariantConclusion, ReachabilityConclusion, SearchLimits};

const LIMITS: SearchLimits = SearchLimits {
    max_states: 128,
    max_depth: 16,
};

#[test]
fn both_endings_have_source_linked_replayable_witnesses() {
    let town = town().unwrap();
    let safety = town.model.check_invariants(&town.initial, LIMITS).unwrap();
    assert_eq!(safety.conclusion, InvariantConclusion::Holds);
    assert!(safety.coverage.exhausted);
    for target in [&town.sheltered, &town.departed] {
        let report = town
            .model
            .reachability(&town.initial, target, LIMITS)
            .unwrap();
        let ReachabilityConclusion::Reached { witness } = report.conclusion else {
            panic!("ending must be reachable");
        };
        assert_eq!(witness.steps.len(), 3);
        let mut state = town.initial.clone();
        for step in witness.steps {
            assert!(step.transition.provenance.file.contains("composed_town/"));
            state = town
                .model
                .apply(&state, &step.transition.id)
                .unwrap()
                .unwrap();
            assert_eq!(town.model.snapshot(&state).unwrap(), step.state);
        }
        assert!(town.model.evaluate(&state, target).unwrap());
        assert!(
            town.model.transitions().all(|event| town
                .model
                .apply(&state, &event.id)
                .unwrap()
                .is_none())
        );
    }
}

#[test]
fn a_keepers_warning_changes_the_ferrymans_actions_and_oil_is_shared() {
    let town = town().unwrap();
    assert!(
        town.model
            .apply(&town.initial, "ferryman.fuel_ferry")
            .unwrap()
            .is_none()
    );
    let warned = town
        .model
        .apply(&town.initial, "keeper.share_warning")
        .unwrap()
        .unwrap();
    for (first, blocked) in [
        ("keeper.light_beacon", "ferryman.fuel_ferry"),
        ("ferryman.fuel_ferry", "keeper.light_beacon"),
    ] {
        let spent = town.model.apply(&warned, first).unwrap().unwrap();
        assert!(town.model.apply(&spent, blocked).unwrap().is_none());
        assert!(town.model.apply(&spent, first).unwrap().is_none());
    }
}

#[test]
fn duplicate_and_conflicting_modules_fail_at_the_offending_source() {
    struct ConflictingOil;
    impl Fact for ConflictingOil {
        type Value = Ending;
        const KEY: &'static str = Oil::KEY;
        const KIND: FactKind = FactKind::Setting;
    }
    let mut builder = ModelBuilder::new("conflicting_modules").unwrap();
    let original_line = line!() + 1;
    let oil = builder.fact::<Oil>().unwrap();
    let duplicate_line = line!() + 1;
    let error = builder.fact::<ConflictingOil>().unwrap_err();
    let ErrorKind::DuplicateId { kind, id, original } = &error.kind else {
        panic!("duplicate fact expected");
    };
    assert_eq!((*kind, id.as_str()), ("fact", Oil::KEY));
    assert_eq!(original.line, original_line);
    assert_eq!(original.file, error.provenance.file);
    assert_eq!(error.provenance.line, duplicate_line);
    assert!(
        error
            .provenance
            .file
            .ends_with("composed_town/tests/mod.rs")
    );
    // A rejected declaration does not replace the original mutable resource.
    let model = builder
        .with_transition(Transition::new("original.consume").with_effect(oil.set(false)))
        .build()
        .unwrap();
    assert_eq!(model.facts().len(), 1);
    assert_eq!(model.facts().first().unwrap().kind, FactKind::Mutable);

    let duplicate_line = line!() + 1;
    let conflicting = Transition::new("keeper.share_warning").with_effect(oil.set(false));
    let original = Transition::new("keeper.share_warning");
    let original_line = line!() - 1;
    let error = ModelBuilder::new("duplicate_events")
        .unwrap()
        .with_transition(original)
        .with_transition(conflicting)
        .build()
        .unwrap_err();
    let ErrorKind::DuplicateId { kind, id, original } = &error.kind else {
        panic!("duplicate event expected");
    };
    assert_eq!((*kind, id.as_str()), ("transition", "keeper.share_warning"));
    assert_eq!(original.line, original_line);
    assert_eq!(error.provenance.line, duplicate_line);
}

#[test]
fn authored_events_are_distinct_from_the_exhausted_state_graph() {
    let town = town().unwrap();
    let report = town
        .model
        .reachability(
            &town.initial,
            &town.unreachable,
            SearchLimits {
                max_states: 6,
                max_depth: 3,
            },
        )
        .unwrap();
    assert_eq!(report.conclusion, ReachabilityConclusion::Unreachable);
    assert!(report.coverage.exhausted);
    assert_eq!(town.model.transitions().count(), 5);
    assert_eq!(town.model.facts().len(), 6);
    assert_eq!(report.coverage.evaluated_states, 6);
    assert_eq!(report.coverage.discovered_states, 6);
    assert_eq!(report.coverage.deepest_evaluated, Some(3));
}
