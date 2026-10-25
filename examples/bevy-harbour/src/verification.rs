//! Analysis runs explicitly outside Update; the schedule only executes game actions.
use super::{
    Difference, Error,
    composed_town::{self, Town},
    host::{Action, Harbour, Host, Lighting},
};
use leitmotif::{
    Model, Predicate, ReachabilityConclusion, ReachabilityReport, SearchLimits, Snapshot, State,
};
use std::collections::{BTreeSet, VecDeque};

#[derive(Debug, PartialEq, Eq)]
pub(super) enum Replay {
    Reached { actions: usize, state: Snapshot },
    Unreachable,
    Inconclusive,
}

fn verify(
    model: &Model,
    state: &State,
    action: Action,
    host: &mut Host,
    prefix: usize,
) -> Result<Option<State>, Error> {
    let transition = model
        .transitions()
        .find(|transition| transition.id == action.id())
        .ok_or_else(|| Error::Unsupported(action.id().into()))?;
    let successor = model.apply(state, action.id())?;
    let expected = model.snapshot(successor.as_ref().unwrap_or(state))?;
    let accepted = host.apply(action)?;
    let actual = host.state()?.snapshot();
    if accepted != successor.is_some() || actual != expected {
        return Err(Error::Divergence(Box::new(Difference {
            action: action.id(),
            source: transition.provenance.clone(),
            expected_enabled: successor.is_some(),
            accepted,
            expected,
            actual,
            verified_prefix: prefix,
            accepted_actions: prefix
                .checked_add(usize::from(accepted))
                .ok_or(Error::Budget)?,
        })));
    }
    Ok(successor)
}

/// Reconstruct branches from game-owned snapshots, never from model successors.
pub(super) fn audit(town: &Town, initial: &Harbour) -> Result<(usize, usize), Error> {
    let mut seen = BTreeSet::from([initial.snapshot().values]);
    let mut frontier = VecDeque::from([(town.initial.clone(), initial.clone(), 0usize)]);
    let mut cases = 0usize;
    for transition in town.model.transitions() {
        Action::parse(&transition.id)?;
    }
    while let Some((state, live, depth)) = frontier.pop_front() {
        for action in Action::ALL {
            let mut host = Host::new(live.clone(), Lighting::SpendOil);
            let next = verify(&town.model, &state, action, &mut host, depth)?;
            cases = cases.checked_add(1).ok_or(Error::Budget)?;
            let Some(next) = next else {
                continue;
            };
            let live = host.state()?.clone();
            if !seen.insert(live.snapshot().values) {
                continue;
            }
            if seen.len() > 6 {
                return Err(Error::Budget);
            }
            frontier.push_back((next, live, depth.checked_add(1).ok_or(Error::Budget)?));
        }
    }
    Ok((seen.len(), cases))
}

// Use the model's public report projection to resolve only this fixture's typed
// targets. The one-state, zero-depth bound never explores a successor.
fn resolve_target<'a>(town: &'a Town, report: &ReachabilityReport) -> Result<&'a Predicate, Error> {
    for target in [&town.sheltered, &town.departed, &town.unreachable] {
        let declared = town.model.reachability(
            &town.initial,
            target,
            SearchLimits {
                max_states: 1,
                max_depth: 0,
            },
        )?;
        if declared.target == report.target {
            return Ok(target);
        }
    }
    Err(Error::InvalidWitness)
}

pub(super) fn replay(
    town: &Town,
    report: &ReachabilityReport,
    host: &mut Host,
) -> Result<Replay, Error> {
    if host.state()?.snapshot() != report.initial || report.model_id != town.model.fingerprint() {
        return Err(Error::StaleSnapshot);
    }
    let target = resolve_target(town, report)?;
    let witness = match &report.conclusion {
        ReachabilityConclusion::Reached { witness } => witness,
        ReachabilityConclusion::Unreachable => return Ok(Replay::Unreachable),
        ReachabilityConclusion::Inconclusive => return Ok(Replay::Inconclusive),
    };
    // Resolve the complete mapping before any live effects.
    let actions = witness
        .steps
        .iter()
        .map(|step| Action::parse(&step.transition.id))
        .collect::<Result<Vec<_>, _>>()?;
    if witness.initial != report.initial || report.initial != town.model.snapshot(&town.initial)? {
        return Err(Error::InvalidWitness);
    }
    let mut expected = town.initial.clone();
    for (step, action) in witness.steps.iter().zip(&actions) {
        expected = town
            .model
            .apply(&expected, action.id())?
            .ok_or(Error::InvalidWitness)?;
        if town.model.snapshot(&expected)? != step.state {
            return Err(Error::InvalidWitness);
        }
    }
    if !town.model.evaluate(&expected, target)? {
        return Err(Error::InvalidWitness);
    }
    let mut state = town.initial.clone();
    for (prefix, action) in actions.into_iter().enumerate() {
        state = verify(&town.model, &state, action, host, prefix)?
            .ok_or_else(|| Error::Unsupported(action.id().into()))?;
    }
    Ok(Replay::Reached {
        actions: witness.steps.len(),
        state: host.state()?.snapshot(),
    })
}

pub(super) fn run() -> Result<(), Error> {
    let initial = Harbour::initial();
    let town = composed_town::town()?;
    let projected = composed_town::project(&initial.projection())?;
    if projected.model.snapshot(&projected.initial)? != town.model.snapshot(&town.initial)? {
        return Err(Error::StaleSnapshot);
    }
    let (states, cases) = audit(&projected, &initial)?;
    println!(
        "Bevy harbour: {states} reachable states; {cases} independent scheduled action checks"
    );
    for (label, target, limits) in [
        (
            "sheltered",
            &town.sheltered,
            SearchLimits {
                max_states: 6,
                max_depth: 3,
            },
        ),
        (
            "departed",
            &town.departed,
            SearchLimits {
                max_states: 6,
                max_depth: 3,
            },
        ),
        (
            "storm_passed",
            &town.unreachable,
            SearchLimits {
                max_states: 6,
                max_depth: 3,
            },
        ),
        (
            "bounded shelter",
            &town.sheltered,
            SearchLimits {
                max_states: 1,
                max_depth: 0,
            },
        ),
    ] {
        let report = town.model.reachability(&town.initial, target, limits)?;
        let outcome = replay(
            &town,
            &report,
            &mut Host::new(initial.clone(), Lighting::SpendOil),
        )?;
        println!("{label}: {outcome:?}");
    }
    let report = town.model.reachability(
        &town.initial,
        &town.sheltered,
        SearchLimits {
            max_states: 6,
            max_depth: 3,
        },
    )?;
    let mut faulty = Host::new(initial, Lighting::PreserveOil);
    match replay(&town, &report, &mut faulty) {
        Err(Error::Divergence(difference)) => {
            println!("deliberate missing oil effect detected: {difference}");
        }
        Err(error) => return Err(error),
        Ok(_) => {
            return Err(Error::Unsupported(
                "deliberate fault was not detected".into(),
            ));
        }
    }
    Ok(())
}
