use leitmotif::{Predicate, ReachabilityConclusion, SearchLimits};

use super::{Failure, replay};
use crate::fixture::scaling_town::{Parameters, Resource, Town};

#[test]
fn reference_replay_rejects_corrupted_witnesses() {
    let town = Town::new(Parameters::new(1, 3, 0, Resource::Unlimited).unwrap()).unwrap();
    let report = town
        .model
        .reachability(
            &town.initial,
            &town.goal,
            SearchLimits {
                max_states: 32,
                max_depth: 8,
            },
        )
        .unwrap();
    let ReachabilityConclusion::Reached { witness } = report.conclusion else {
        panic!("fixture must reach goal");
    };
    replay(&town.model, &town.initial, &town.goal, &witness).unwrap();
    let mut changed = witness.clone();
    changed.initial = witness.steps.last().unwrap().state.clone();
    assert!(matches!(
        replay(&town.model, &town.initial, &town.goal, &changed),
        Err(Failure::Invalid("symbolic witness initial state mismatch"))
    ));
    let mut changed = witness.clone();
    changed.steps.first_mut().unwrap().state = witness.initial.clone();
    assert!(matches!(
        replay(&town.model, &town.initial, &town.goal, &changed),
        Err(Failure::Invalid(
            "symbolic witness snapshot disagrees with reference model"
        ))
    ));
    let mut changed = witness.clone();
    changed.steps.swap(0, 1);
    assert!(matches!(
        replay(&town.model, &town.initial, &town.goal, &changed),
        Err(Failure::Invalid(
            "symbolic witness transition is disabled in reference model"
        ))
    ));
    assert!(matches!(
        replay(
            &town.model,
            &town.initial,
            &Predicate::constant(false),
            &witness
        ),
        Err(Failure::Invalid("symbolic witness missed its target"))
    ));
}
