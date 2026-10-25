//! Compare independent host execution with model oracles; never hydrate successors.
use std::collections::{BTreeMap, BTreeSet, VecDeque};

use leitmotif::{Model, ReachabilityConclusion, Snapshot, Source, State, Value};
use serde::Serialize;

use super::super::composed_town;
use super::{
    contract::{Analysis, Contract},
    error::{Difference, Error, Operation},
    host::{Action, Host},
    input::Request,
};

#[derive(Debug, Serialize)]
pub(super) struct Case {
    /// Replay these authored actions against host-owned initial state to reach this case.
    pub(super) prefix: Vec<String>,
    pub(super) before: Snapshot,
    pub(super) action: &'static str,
    pub(super) provenance: Source,
    pub(super) expected_enabled: bool,
    pub(super) expected: Snapshot,
}
impl Case {
    fn verify(&self, host: &Host, accepted: bool, operation: Operation) -> Result<(), Error> {
        let actual = host.snapshot().snapshot();
        if self.expected_enabled == accepted && self.expected == actual {
            return Ok(());
        }
        let accepted_host_actions = self
            .prefix
            .len()
            .checked_add(usize::from(accepted))
            .ok_or_else(|| Error::Unsupported("host action counter capacity".into()))?;
        Err(Error::ConformanceFailure(Box::new(Difference {
            action: self.action.into(),
            provenance: self.provenance.clone(),
            expected_enabled: self.expected_enabled,
            actual_enabled: accepted,
            expected: self.expected.clone(),
            actual,
            operation,
            verified_prefix_actions: self.prefix.len(),
            accepted_host_actions,
        })))
    }
}
#[derive(Debug, Serialize)]
pub(super) struct Audit {
    pub(super) states: usize,
    pub(super) cases: Vec<Case>,
}
#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(super) enum Replay {
    Reached { actions: usize, state: Snapshot },
    Unreachable,
    Inconclusive,
}
struct Node {
    state: State,
    host: Host,
    prefix: Vec<String>,
}
struct Probe {
    case: Case,
    successor: Option<State>,
    host: Host,
}
type Visited = BTreeSet<BTreeMap<String, Value>>;

fn probe(model: &Model, node: &Node, action: Action) -> Result<Probe, Error> {
    let transition = model
        .transitions()
        .find(|transition| transition.id == action.id())
        .ok_or_else(|| Error::Unsupported(format!("model action {}", action.id())))?;
    let before = model.snapshot(&node.state)?;
    let successor = model.apply(&node.state, action.id())?;
    let expected = match &successor {
        Some(next) => model.snapshot(next)?,
        None => before.clone(),
    };
    let mut host = node.host.clone();
    let accepted = host.apply(action);
    let case = Case {
        prefix: node.prefix.clone(),
        before,
        action: action.id(),
        provenance: transition.provenance.clone(),
        expected_enabled: successor.is_some(),
        expected,
    };
    case.verify(&host, accepted, Operation::Audit)?;
    Ok(Probe {
        case,
        successor,
        host,
    })
}
fn admit(
    model: &Model,
    seen: &mut Visited,
    capacity: usize,
    probe: Probe,
) -> Result<(Case, Option<Node>), Error> {
    let Probe {
        case,
        successor,
        host,
    } = probe;
    let Some(state) = successor else {
        return Ok((case, None));
    };
    if !seen.insert(model.snapshot(&state)?.values) {
        return Ok((case, None));
    }
    if seen.len() > capacity {
        return Err(Error::Unsupported(
            "host graph exceeds declared finite domains".into(),
        ));
    }
    let mut prefix = case.prefix.clone();
    prefix.push(case.action.into());
    Ok((
        case,
        Some(Node {
            state,
            host,
            prefix,
        }),
    ))
}
impl Contract {
    fn validate_live(&self, request: &Request, host: &Host) -> Result<(), Error> {
        self.validate(request)?;
        if host.snapshot() != &request.initial {
            return Err(Error::RejectedInput(
                "live host differs from the captured initial projection".into(),
            ));
        }
        Ok(())
    }
    pub(super) fn audit(&self, request: &Request, host: &Host) -> Result<Audit, Error> {
        self.validate_live(request, host)?;
        let town = composed_town::project(&request.initial.projection())?;
        for transition in town.model.transitions() {
            if Action::from_id(&transition.id).is_none() {
                return Err(Error::Unsupported(format!("action {}", transition.id)));
            }
        }
        let capacity = town.model.facts().iter().try_fold(1usize, |size, fact| {
            size.checked_mul(fact.domain.len())
                .ok_or_else(|| Error::Unsupported("finite host domain capacity".into()))
        })?;
        let mut frontier = VecDeque::from([Node {
            state: town.initial.clone(),
            host: host.clone(),
            prefix: Vec::new(),
        }]);
        let mut seen = BTreeSet::from([town.model.snapshot(&town.initial)?.values]);
        let mut cases = Vec::new();
        while let Some(node) = frontier.pop_front() {
            for action in Action::ALL {
                let (case, successor) = admit(
                    &town.model,
                    &mut seen,
                    capacity,
                    probe(&town.model, &node, action)?,
                )?;
                frontier.extend(successor);
                cases.push(case);
            }
        }
        Ok(Audit {
            states: seen.len(),
            cases,
        })
    }
    pub(super) fn replay(
        &self,
        request: &Request,
        analysis: &Analysis,
        host: &mut Host,
    ) -> Result<Replay, Error> {
        self.validate_live(request, host)?;
        let query = self.validate(request)?;
        if analysis.source_revision != request.source_revision
            || analysis.binding_version != request.binding_version
            || analysis.schema_version != request.schema_version
            || analysis.query != query
            || analysis.report.model_id != request.model_id
            || analysis.report.initial != request.initial.snapshot()
            || analysis.report.coverage.limits.max_states != request.limits.max_states
            || analysis.report.coverage.limits.max_depth != request.limits.max_depth
        {
            return Err(Error::RejectedInput(
                "report does not belong to this request".into(),
            ));
        }
        let witness = match &analysis.report.conclusion {
            ReachabilityConclusion::Reached { witness } => witness,
            ReachabilityConclusion::Unreachable => return Ok(Replay::Unreachable),
            ReachabilityConclusion::Inconclusive => return Ok(Replay::Inconclusive),
        };
        if witness.initial != request.initial.snapshot() {
            return Err(Error::RejectedInput(
                "witness initial projection differs".into(),
            ));
        }
        let actions = witness
            .steps
            .iter()
            .map(|step| {
                Action::from_id(&step.transition.id)
                    .ok_or_else(|| Error::Unsupported(format!("action {}", step.transition.id)))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut prefix = Vec::new();
        for (step, action) in witness.steps.iter().zip(actions) {
            let case = Case {
                prefix: prefix.clone(),
                before: host.snapshot().snapshot(),
                action: action.id(),
                provenance: step.transition.provenance.clone(),
                expected_enabled: true,
                expected: step.state.clone(),
            };
            let accepted = host.apply(action);
            case.verify(host, accepted, Operation::Replay)?;
            prefix.push(action.id().into());
        }
        Ok(Replay::Reached {
            actions: witness.steps.len(),
            state: host.snapshot().snapshot(),
        })
    }
}
