use leitmotif::{Model, Predicate, SearchLimits, Snapshot, State, Trace, TraceStep};
use oxidd::{BooleanFunction, Function, bdd::BDDFunction};
use serde::Serialize;

use super::{Encoding, Failure, Oxidd, Result};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(in super::super) enum Conclusion {
    Reached,
    Unreachable,
    Inconclusive,
}

#[derive(Serialize)]
pub(in super::super) struct Observation {
    id: String,
    /// False is reported only after an exhausted symbolic fixed point.
    pub(in super::super) reachable_violation: Option<bool>,
}

#[derive(Serialize)]
pub(in super::super) struct Outcome {
    pub(in super::super) conclusion: Conclusion,
    pub(in super::super) exhausted: bool,
    pub(in super::super) image_iterations: usize,
    pub(in super::super) reachable_nodes: usize,
    pub(in super::super) relation_nodes_sum: usize,
    pub(in super::super) manager_inner_nodes: usize,
    pub(in super::super) reachable_count_estimate: f64,
    pub(in super::super) witness: Option<Trace>,
    pub(in super::super) invariants: Vec<Observation>,
}

fn recover(
    backend: &Oxidd,
    encoding: &Encoding<'_>,
    layers: &[BDDFunction],
    relations: &[(String, BDDFunction)],
    model: &Model,
    mut terminal: Snapshot,
) -> Result<Trace> {
    let mut steps = Vec::new();
    for predecessor in layers.iter().rev().skip(1) {
        let next = encoding.snapshot(backend, &terminal, true)?;
        let mut found = None;
        for (id, relation) in relations {
            let candidates = (predecessor.and(relation)?).and(&next)?;
            if let Some(valuation) = Oxidd::pick(&candidates, encoding.variables)? {
                found = Some((id.clone(), encoding.decode(&valuation)?));
                break;
            }
        }
        let (id, previous) =
            found.ok_or(Failure::Invalid("symbolic witness has no predecessor"))?;
        let transition = model
            .transitions()
            .find(|transition| transition.id == id)
            .ok_or(Failure::Invalid("unknown witness transition"))?
            .clone();
        steps.push(TraceStep {
            transition,
            state: terminal,
        });
        terminal = previous;
    }
    steps.reverse();
    Ok(Trace {
        initial: terminal,
        steps,
    })
}

pub(in super::super) fn replay(
    model: &Model,
    initial: &State,
    target: &Predicate,
    witness: &Trace,
) -> Result<()> {
    let mut state = initial.clone();
    if model.snapshot(initial)? != witness.initial {
        return Err(Failure::Invalid("symbolic witness initial state mismatch"));
    }
    for step in &witness.steps {
        state = model
            .apply(&state, &step.transition.id)?
            .ok_or(Failure::Invalid(
                "symbolic witness transition is disabled in reference model",
            ))?;
        if model.snapshot(&state)? != step.state {
            return Err(Failure::Invalid(
                "symbolic witness snapshot disagrees with reference model",
            ));
        }
    }
    if !model.evaluate(&state, target)? {
        return Err(Failure::Invalid("symbolic witness missed its target"));
    }
    Ok(())
}

pub(in super::super) fn explore(
    backend: &Oxidd,
    encoding: &Encoding<'_>,
    model: &Model,
    initial: &State,
    target: &Predicate,
    max_depth: usize,
) -> Result<Outcome> {
    // Zero-state reports expose existing typed declarations without exploring
    // successors. Invariants remain observations, never relation constraints.
    let metadata = SearchLimits {
        max_states: 0,
        max_depth: 0,
    };
    let description = model.reachability(initial, target, metadata)?.target;
    let invariants = model.check_invariants(initial, metadata)?.invariants;
    let target_set = encoding.predicate(backend, &description)?;
    let relations = encoding.relations(backend, model)?;
    let initial_set = encoding.snapshot(backend, &model.snapshot(initial)?, false)?;
    let mut visited = initial_set.clone();
    let mut layers = vec![initial_set];
    let mut reached = None;
    let mut exhausted = false;
    let mut iterations = 0_usize;
    for depth in 0..=max_depth {
        let frontier = layers
            .last()
            .ok_or(Failure::Invalid("missing symbolic frontier"))?;
        let candidates = frontier.and(&target_set)?;
        if let Some(valuation) = Oxidd::pick(&candidates, encoding.variables)? {
            reached = Some((depth, encoding.decode(&valuation)?));
            break;
        }
        let mut image = backend.constant(false);
        for (_, relation) in &relations {
            let product = frontier.and(relation)?;
            image = image.or(&backend.exists(&product, &encoding.current)?)?;
        }
        image = backend.rename(&image, &encoding.pairs())?;
        let fresh = image.and(&visited.not()?)?;
        iterations = iterations
            .checked_add(1)
            .ok_or(Failure::Invalid("image counter overflow"))?;
        if !fresh.satisfiable() {
            exhausted = true;
            break;
        }
        if depth == max_depth {
            break;
        }
        visited = visited.or(&fresh)?;
        layers.push(fresh);
    }
    let witness = if let Some((depth, terminal)) = reached {
        let end = depth
            .checked_add(1)
            .ok_or(Failure::Invalid("witness depth overflow"))?;
        let prefix = layers
            .get(..end)
            .ok_or(Failure::Invalid("missing witness layer"))?;
        let steps = recover(backend, encoding, prefix, &relations, model, terminal)?;
        replay(model, initial, target, &steps)?;
        Some(steps)
    } else {
        None
    };
    let mut observations = Vec::new();
    for invariant in invariants {
        let bad = (encoding.predicate(backend, &invariant.predicate)?).not()?;
        let violation = (visited.and(&bad)?).satisfiable();
        observations.push(Observation {
            id: invariant.id,
            reachable_violation: if violation {
                Some(true)
            } else if exhausted {
                Some(false)
            } else {
                None
            },
        });
    }
    let relation_nodes_sum = relations.iter().try_fold(0_usize, |sum, (_, relation)| {
        sum.checked_add(relation.node_count())
            .ok_or(Failure::Invalid("node count overflow"))
    })?;
    Ok(Outcome {
        conclusion: if witness.is_some() {
            Conclusion::Reached
        } else if exhausted {
            Conclusion::Unreachable
        } else {
            Conclusion::Inconclusive
        },
        exhausted,
        image_iterations: iterations,
        reachable_nodes: visited.node_count(),
        relation_nodes_sum,
        manager_inner_nodes: backend.allocated(),
        reachable_count_estimate: encoding.count(&visited)?,
        witness,
        invariants: observations,
    })
}

#[cfg(test)]
#[path = "tests/replay.rs"]
mod tests;
