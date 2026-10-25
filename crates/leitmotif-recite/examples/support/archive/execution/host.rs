use super::super::{
    evidence::{ActionSource, DisabledEffect, Outcome},
    protocol::{HostOutcome, RestoreWindow, Schedule},
};
use super::{Execution, Node, RunResult};
use leitmotif::TransitionInfo;
use leitmotif_recite::EffectErrorKind;
use recite_runtime::{
    DialogueEffectRequest, EffectAck, acknowledge_effect,
    snapshot::{decode_session_messagepack, encode_session_messagepack},
};

impl Execution<'_> {
    pub(super) fn apply_immediate(
        &self,
        node: &mut Node,
        request: &DialogueEffectRequest,
    ) -> RunResult<(ActionSource, Outcome)> {
        let applied = self.immediate.apply(request, &node.host)?;
        let location = ActionSource::Dialogue(applied.request().source_span.clone());
        let transition = applied.transition().clone();
        node.host = applied.into_state();
        node.schedule = Schedule::Traverse;
        Ok((location, Outcome::HostApplied(transition)))
    }
    pub(super) fn unlock(&self, node: &mut Node) -> RunResult<(ActionSource, Outcome)> {
        let request = node.session.pending_effect().ok_or("no blocking request")?;
        let location = ActionSource::Dialogue(request.source_span.clone());
        let outcome = match self.blocking.apply_pending(&node.session, &node.host) {
            Ok(applied) => {
                let transition = applied.transition().clone();
                node.host = applied.into_state();
                node.schedule = Schedule::Pending {
                    outcome: HostOutcome::Applied,
                    redelivery: false,
                };
                Outcome::HostApplied(transition)
            }
            Err(error) => {
                let diagnostic = error.to_string();
                let EffectErrorKind::DisabledTransition(id) = error.kind else {
                    return Err(Box::new(error));
                };
                let failure = DisabledEffect {
                    transition: id,
                    provenance: error.provenance,
                    diagnostic,
                };
                let denial = self.host_transition(node, "deny")?;
                node.schedule = Schedule::Pending {
                    outcome: HostOutcome::Denied,
                    redelivery: false,
                };
                Outcome::HostDisabled { failure, denial }
            }
        };
        Ok((location, outcome))
    }
    pub(super) fn acknowledge(
        node: &mut Node,
        ack: EffectAck,
    ) -> RunResult<(ActionSource, Outcome)> {
        let request = node.session.pending_effect().ok_or("no blocking request")?;
        let location = ActionSource::Dialogue(request.source_span.clone());
        let request = request.clone();
        acknowledge_effect(&mut node.session, request.id.clone(), ack.clone())?;
        node.schedule = Schedule::Traverse;
        Ok((
            location,
            Outcome::Acknowledged {
                request,
                acknowledgement: ack,
            },
        ))
    }
    pub(super) fn apply_deferred(
        &self,
        node: &mut Node,
        index: usize,
    ) -> RunResult<(ActionSource, Outcome)> {
        let applied = self.deferred.apply_at(&node.session, index, &node.host)?;
        let location = ActionSource::Dialogue(applied.request().source_span.clone());
        let transition = applied.transition().clone();
        node.host = applied.into_state();
        let next_index = index.checked_add(1).ok_or("deferred position overflow")?;
        node.schedule = if next_index == node.session.deferred_effects().len() {
            Schedule::Done
        } else {
            Schedule::Deferred(next_index)
        };
        Ok((location, Outcome::HostApplied(transition)))
    }
    pub(super) fn host_action(
        &self,
        node: &mut Node,
        id: &str,
    ) -> RunResult<(ActionSource, Outcome)> {
        let declared = self.host_transition(node, id)?;
        Ok((
            ActionSource::Host(declared.provenance.clone()),
            Outcome::HostApplied(declared),
        ))
    }
    pub(super) fn deny(&self, node: &mut Node) -> RunResult<(ActionSource, Outcome)> {
        let result = self.host_action(node, "deny")?;
        node.schedule = Schedule::Pending {
            outcome: HostOutcome::Denied,
            redelivery: false,
        };
        Ok(result)
    }
    pub(super) fn restore(
        &self,
        node: &mut Node,
        window: RestoreWindow,
        outcome: HostOutcome,
    ) -> RunResult<(ActionSource, Outcome)> {
        let declared = self.host_transition(node, window.transition())?;
        let saved = encode_session_messagepack(&node.session)?;
        node.session = decode_session_messagepack(&self.scenario.asset, &saved)?;
        node.schedule = Schedule::Pending {
            outcome,
            redelivery: true,
        };
        Ok((
            ActionSource::Host(declared.provenance.clone()),
            Outcome::Restored(declared),
        ))
    }
    fn host_transition(&self, node: &mut Node, id: &str) -> RunResult<TransitionInfo> {
        let transition = self
            .scenario
            .model
            .transitions()
            .find(|transition| transition.id == id)
            .ok_or("fixture transition missing")?
            .clone();
        node.host = self
            .scenario
            .model
            .apply(&node.host, id)?
            .ok_or("fixture host transition disabled")?;
        Ok(transition)
    }
}
