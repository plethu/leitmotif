use std::collections::{BTreeMap, BTreeSet, VecDeque};

use godot::builtin::VarDictionary;
use godot::classes::Node;
use godot::prelude::Gd;
use leitmotif::{
    Model, ReachabilityConclusion, SearchLimits, Snapshot, State, TransitionInfo, Value,
};

use super::composed_town::{self, Town};
use super::error::{Difference, NativeError};
use super::{NativeResult, projection};

struct Position {
    state: State,
    prefix: Vec<String>,
}
#[derive(Clone, Copy)]
struct Progress {
    verified: u32,
    accepted: u32,
}

pub(super) fn audit(mut host: Gd<Node>) -> NativeResult<VarDictionary> {
    let town = composed_town::town()?;
    let mut seen = BTreeSet::from([town.model.snapshot(&town.initial)?.values]);
    let mut frontier = VecDeque::from([Position {
        state: town.initial.clone(),
        prefix: Vec::new(),
    }]);
    let mut cases = 0i64;
    let mut actions = 0i64;
    while let Some(position) = frontier.pop_front() {
        for transition in town.model.transitions() {
            projection::reset(&mut host)?;
            validate_initial(&mut host, &town)?;
            let prefix_actions = replay_prefix(&mut host, &town, &position.prefix)?;
            actions = actions
                .checked_add(i64::from(prefix_actions))
                .ok_or_else(|| NativeError::analysis("host action counter overflow"))?;
            let successor = town.model.apply(&position.state, &transition.id)?;
            let expected = match &successor {
                Some(state) => town.model.snapshot(state)?,
                None => town.model.snapshot(&position.state)?,
            };
            let accepted = projection::apply(&mut host, &transition.id)?;
            actions = actions
                .checked_add(1)
                .ok_or_else(|| NativeError::analysis("host action counter overflow"))?;
            verify(
                &mut host,
                &town.model,
                &expected,
                transition,
                (successor.is_some(), accepted),
                Progress {
                    verified: prefix_actions,
                    accepted: prefix_actions
                        .checked_add(u32::from(accepted))
                        .ok_or_else(|| NativeError::analysis("host action counter overflow"))?,
                },
            )?;
            cases = cases
                .checked_add(1)
                .ok_or_else(|| NativeError::analysis("audit counter overflow"))?;
            if let Some(state) = successor {
                admit(
                    &mut seen,
                    &mut frontier,
                    expected,
                    state,
                    &position.prefix,
                    &transition.id,
                );
            }
        }
    }
    let mut result = VarDictionary::new();
    result.set("states", i64::try_from(seen.len())?);
    result.set("cases", cases);
    result.set("host_actions", actions);
    result.set("fingerprint", town.model.fingerprint());
    Ok(result)
}

pub(super) fn replay(mut host: Gd<Node>, query: &str, depth: i64) -> NativeResult<VarDictionary> {
    let town = composed_town::town()?;
    let query = match query {
        "sheltered" => &town.sheltered,
        "departed" => &town.departed,
        "storm-passed" => &town.unreachable,
        _ => {
            return Err(NativeError::rejected(format!(
                "unsupported harbour query {query}"
            )));
        }
    };
    let depth = usize::try_from(depth)
        .map_err(|error| NativeError::rejected(format!("invalid search depth: {error}")))?;
    if depth > 3 {
        return Err(NativeError::rejected(
            "harbour search depth exceeds three transitions",
        ));
    }
    validate_initial(&mut host, &town)?;
    let report = town.model.reachability(
        &town.initial,
        query,
        SearchLimits {
            max_states: 6,
            max_depth: depth,
        },
    )?;
    let mut result = VarDictionary::new();
    let outcome = match report.conclusion {
        ReachabilityConclusion::Reached { witness } => {
            for (index, step) in witness.steps.iter().enumerate() {
                let accepted = projection::apply(&mut host, &step.transition.id)?;
                let verified = u32::try_from(index)?;
                verify(
                    &mut host,
                    &town.model,
                    &step.state,
                    &step.transition,
                    (true, accepted),
                    Progress {
                        verified,
                        accepted: verified
                            .checked_add(u32::from(accepted))
                            .ok_or_else(|| NativeError::analysis("host action counter overflow"))?,
                    },
                )?;
            }
            result.set("actions", i64::try_from(witness.steps.len())?);
            "reached"
        }
        ReachabilityConclusion::Unreachable => "unreachable",
        ReachabilityConclusion::Inconclusive => "inconclusive",
    };
    result.set("outcome", outcome);
    result.set("fingerprint", town.model.fingerprint());
    Ok(result)
}

fn validate_initial(host: &mut Gd<Node>, town: &Town) -> NativeResult<()> {
    let expected = town.model.snapshot(&town.initial)?;
    let actual = projection::capture(host, &town.model)?;
    if actual == expected {
        Ok(())
    } else {
        Err(NativeError::StaleInput { expected, actual })
    }
}

fn verify(
    host: &mut Gd<Node>,
    model: &Model,
    expected: &Snapshot,
    transition: &TransitionInfo,
    enabled: (bool, bool),
    progress: Progress,
) -> NativeResult<()> {
    let actual = projection::capture(host, model)?;
    if enabled.0 != enabled.1 || actual != *expected {
        return Err(NativeError::conformance(Difference {
            action: transition.id.clone(),
            source: Some(transition.provenance.clone()),
            expected: expected.clone(),
            actual,
            expected_enabled: Some(enabled.0),
            actual_enabled: Some(enabled.1),
            verified_prefix_actions: progress.verified,
            accepted_host_actions: progress.accepted,
        }));
    }
    Ok(())
}

fn replay_prefix(host: &mut Gd<Node>, town: &Town, prefix: &[String]) -> NativeResult<u32> {
    let mut state = town.initial.clone();
    for (index, action) in prefix.iter().enumerate() {
        let transition = town
            .model
            .transitions()
            .find(|transition| transition.id == *action)
            .ok_or_else(|| NativeError::analysis("missing prefix transition"))?;
        state = town
            .model
            .apply(&state, action)?
            .ok_or_else(|| NativeError::analysis("disabled model prefix"))?;
        let accepted = projection::apply(host, action)?;
        let verified = u32::try_from(index)?;
        verify(
            host,
            &town.model,
            &town.model.snapshot(&state)?,
            transition,
            (true, accepted),
            Progress {
                verified,
                accepted: verified
                    .checked_add(u32::from(accepted))
                    .ok_or_else(|| NativeError::analysis("host action counter overflow"))?,
            },
        )?;
    }
    Ok(u32::try_from(prefix.len())?)
}

fn admit(
    seen: &mut BTreeSet<BTreeMap<String, Value>>,
    frontier: &mut VecDeque<Position>,
    snapshot: Snapshot,
    state: State,
    prefix: &[String],
    action: &str,
) {
    if seen.insert(snapshot.values) {
        let mut prefix = prefix.to_vec();
        prefix.push(action.into());
        frontier.push_back(Position { state, prefix });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn revisited_snapshot_is_not_reenqueued() {
        let town = composed_town::town().unwrap();
        let mut seen = BTreeSet::new();
        let mut frontier = VecDeque::new();
        for _ in 0..2 {
            admit(
                &mut seen,
                &mut frontier,
                town.model.snapshot(&town.initial).unwrap(),
                town.initial.clone(),
                &[],
                "keeper.share_warning",
            );
        }
        assert_eq!(frontier.len(), 1);
        assert_eq!(frontier.front().unwrap().prefix, ["keeper.share_warning"]);
    }
}
