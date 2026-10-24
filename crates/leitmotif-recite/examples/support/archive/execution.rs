use super::{
    Initial, Scenario,
    evidence::Boundary,
    protocol::{Action, HostOutcome, RestoreWindow, Schedule},
};
use leitmotif::State;
use leitmotif_recite::{BlockingEffects, DeferredEffects, ImmediateEffects};
use recite_core::{
    LineId,
    compiled::{CompiledArgument, CompiledEffect, CompiledEffectMode},
};
use recite_runtime::{
    DialogueEffectRequest, DialogueSession, EffectAck, snapshot::snapshot_session, start_scene,
};
use std::{cell::RefCell, error::Error};

mod host;
mod runtime;

type RunResult<T> = Result<T, Box<dyn Error>>;

#[derive(Clone)]
enum RuntimeOperation {
    Next,
    Choose(recite_core::ChoiceId),
    Redeliver(HostOutcome),
}
impl RuntimeOperation {
    const fn host_outcome(&self) -> HostOutcome {
        match self {
            Self::Redeliver(outcome) => *outcome,
            Self::Next | Self::Choose(_) => HostOutcome::Unapplied,
        }
    }
}

enum EnabledAction {
    Deliver(RuntimeOperation),
    ApplyImmediate(DialogueEffectRequest),
    RemoveKey,
    ApplyUnlock,
    Deny,
    Restore {
        window: RestoreWindow,
        outcome: HostOutcome,
    },
    Acknowledge(EffectAck),
    ApplyDeferred(usize),
}
impl EnabledAction {
    fn action(&self) -> Action {
        match self {
            Self::Deliver(RuntimeOperation::Choose(id)) => Action::Choose(id.clone()),
            Self::Deliver(_) => Action::Next,
            Self::ApplyImmediate(_) => Action::ApplyImmediate,
            Self::RemoveKey => Action::RemoveKey,
            Self::ApplyUnlock => Action::ApplyUnlock,
            Self::Deny => Action::Deny,
            Self::Restore { window, .. } => Action::Restore(*window),
            Self::Acknowledge(_) => Action::Acknowledge,
            Self::ApplyDeferred(index) => Action::ApplyDeferred(*index),
        }
    }
}

#[derive(Clone, Debug)]
pub(super) struct Node {
    pub(super) host: State,
    session: DialogueSession,
    pub(super) schedule: Schedule,
    pub(super) trace: Vec<Boundary>,
    selected_lines: Vec<LineId>,
}

pub(super) struct Execution<'scenario> {
    pub(super) scenario: &'scenario Scenario,
    immediate: ImmediateEffects<'scenario>,
    blocking: BlockingEffects<'scenario>,
    deferred: DeferredEffects<'scenario>,
}

impl<'scenario> Execution<'scenario> {
    pub(super) fn new(scenario: &'scenario Scenario) -> RunResult<Self> {
        let mut execution = Self {
            scenario,
            immediate: ImmediateEffects::new(&scenario.model),
            blocking: BlockingEffects::new(&scenario.model),
            deferred: DeferredEffects::new(&scenario.model),
        };
        for effect in &scenario.asset.effects {
            match effect.mode {
                CompiledEffectMode::Immediate => execution.immediate.bind(effect, "expose")?,
                CompiledEffectMode::Blocking => execution.blocking.bind(effect, "unlock")?,
                CompiledEffectMode::Deferred => {
                    let transition = deferred_transition(effect)?;
                    execution.deferred.bind(effect, &transition)?;
                }
            }
        }
        Ok(execution)
    }

    pub(super) fn initial(&self, initial: Initial) -> RunResult<Node> {
        Ok(Node {
            host: self.scenario.initial(initial)?,
            session: start_scene(&self.scenario.asset, None)?,
            schedule: Schedule::Traverse,
            trace: Vec::new(),
            selected_lines: Vec::new(),
        })
    }

    pub(super) fn actions(&self, node: &Node) -> RunResult<Vec<Action>> {
        Ok(self
            .enabled(node)?
            .iter()
            .map(EnabledAction::action)
            .collect())
    }

    fn enabled(&self, node: &Node) -> RunResult<Vec<EnabledAction>> {
        Ok(match &node.schedule {
            Schedule::Traverse => vec![EnabledAction::Deliver(RuntimeOperation::Next)],
            Schedule::Immediate(request) => vec![EnabledAction::ApplyImmediate(request.clone())],
            Schedule::Prompt(choices) => {
                let mut actions: Vec<_> = choices
                    .iter()
                    .filter(|choice| choice.availability.is_available)
                    .map(|choice| {
                        EnabledAction::Deliver(RuntimeOperation::Choose(choice.id.clone()))
                    })
                    .collect();
                if self.scenario.can_remove_key(&node.host)? {
                    actions.push(EnabledAction::RemoveKey);
                }
                actions
            }
            Schedule::Pending {
                outcome,
                redelivery,
            } => self.pending_actions(node, *outcome, *redelivery)?,
            Schedule::Deferred(index) => vec![EnabledAction::ApplyDeferred(*index)],
            Schedule::Done | Schedule::Failed => Vec::new(),
        })
    }

    pub(super) fn step(&self, node: &mut Node, action: &Action) -> RunResult<()> {
        let enabled = self
            .enabled(node)?
            .into_iter()
            .find(|candidate| candidate.action() == *action)
            .ok_or("action is not enabled by the finite host protocol")?;
        let queries = RefCell::new(Vec::new());
        let (location, outcome) = match enabled {
            EnabledAction::Deliver(operation) => self.deliver(node, &operation, &queries)?,
            EnabledAction::ApplyImmediate(request) => self.apply_immediate(node, &request)?,
            EnabledAction::RemoveKey => self.host_action(node, "remove_key")?,
            EnabledAction::Restore { window, outcome } => self.restore(node, window, outcome)?,
            EnabledAction::Deny => self.deny(node)?,
            EnabledAction::ApplyUnlock => self.unlock(node)?,
            EnabledAction::Acknowledge(ack) => Self::acknowledge(node, ack)?,
            EnabledAction::ApplyDeferred(index) => self.apply_deferred(node, index)?,
        };
        let queries = queries.into_inner();
        self.check_queries(&node.host, &queries, &outcome)?;
        node.trace.push(Boundary {
            action: action.clone(),
            source: location,
            outcome,
            queries,
            host: self.scenario.model.snapshot(&node.host)?,
            session: snapshot_session(&node.session),
            pending: node.session.pending_effect().cloned(),
            deferred: node.session.deferred_effects().to_vec(),
            selected_lines: node.selected_lines.clone(),
        });
        Ok(())
    }

    fn pending_actions(
        &self,
        node: &Node,
        outcome: HostOutcome,
        redelivery: bool,
    ) -> RunResult<Vec<EnabledAction>> {
        if redelivery {
            return Ok(vec![EnabledAction::Deliver(RuntimeOperation::Redeliver(
                outcome,
            ))]);
        }
        let mut actions = match outcome {
            HostOutcome::Unapplied => vec![EnabledAction::ApplyUnlock, EnabledAction::Deny],
            HostOutcome::Applied => vec![EnabledAction::Acknowledge(EffectAck::Completed)],
            HostOutcome::Denied => vec![EnabledAction::Acknowledge(EffectAck::Failed {
                reason: "archive host denied access".into(),
            })],
        };
        let window = outcome.restore_window();
        if self.scenario.can_restore(&node.host, window)? {
            actions.push(EnabledAction::Restore { window, outcome });
        }
        Ok(actions)
    }
}

fn deferred_transition(effect: &CompiledEffect) -> RunResult<String> {
    if effect.function == "record_visit" {
        return Ok("record_visit".into());
    }
    match effect.args.first() {
        Some(CompiledArgument::Identifier(access)) => Ok(format!("record_{access}")),
        _ => Err("result declaration has no access argument".into()),
    }
}

#[cfg(test)]
#[path = "../../../tests/archive/execution.rs"]
mod tests;
