use leitmotif::{Source, State};
use leitmotif_recite::EffectErrorKind;
use recite_core::ChoiceId;
use recite_runtime::{
    ConditionEvaluationError, ConditionQuery, DialogueEffectMode, DialogueEffectRequest,
    DialogueEvent, EffectAck, acknowledge_effect, next,
};

use super::super::{error::Difference, projection};
use super::{
    LiveDialogue, NativeError, NativeResult,
    evidence::{Operation, Origin, Outcome},
    source_at,
};

impl LiveDialogue<'_> {
    pub(super) fn immediate_request(
        &mut self,
        request: &DialogueEffectRequest,
    ) -> NativeResult<()> {
        let source = Some(source_at(&request.source_span));
        if request.mode != DialogueEffectMode::Immediate {
            return Err(NativeError::dialogue("unexpected delivery mode", source));
        }
        self.verify(&request.function, source.clone())?;
        let receipt = self
            .immediate
            .apply(request, &self.oracle)
            .map_err(|error| NativeError::dialogue(error.to_string(), Some(error.provenance)))?;
        let action = receipt.transition().id.clone();
        let successor = receipt.into_state();
        self.perform(&action, Some(successor), source, Operation::Immediate)
    }

    pub(super) fn select(&mut self, choice: ChoiceId) -> NativeResult<()> {
        let DialogueEvent::Effect(request) = self.event(Some(choice))? else {
            return Err(NativeError::dialogue(
                "choice did not emit a blocking request",
                self.source(),
            ));
        };
        let source = Some(source_at(&request.source_span));
        if request.mode != DialogueEffectMode::Blocking {
            return Err(NativeError::dialogue(
                "choice emitted wrong delivery mode",
                source,
            ));
        }
        self.verify(&request.function, source.clone())?;
        let (action, successor) = match self.blocking.apply_pending(&self.session, &self.oracle) {
            Ok(receipt) => (receipt.transition().id.clone(), Some(receipt.into_state())),
            Err(error) => {
                let EffectErrorKind::DisabledTransition(action) = error.kind else {
                    return Err(NativeError::dialogue(
                        error.to_string(),
                        Some(error.provenance),
                    ));
                };
                self.evidence.dialogue_source = error.provenance.to_string();
                let transition = self
                    .scenario
                    .town
                    .model
                    .transitions()
                    .find(|transition| transition.id == action)
                    .ok_or_else(|| NativeError::analysis("disabled transition missing"))?;
                self.evidence.host_source = transition.provenance.to_string();
                (action, None)
            }
        };
        let acknowledgement = if successor.is_some() {
            EffectAck::Completed
        } else {
            self.evidence.failed_ack = true;
            EffectAck::Failed {
                reason: "oil already committed to ferry".into(),
            }
        };
        self.perform(&action, successor, source.clone(), Operation::Blocking)?;
        let span = self.span(None)?;
        let blocked = next(
            &self.scenario.asset,
            &mut self.session,
            &|_: ConditionQuery<'_>| {
                Err(ConditionEvaluationError::new(
                    "pending effect unexpectedly queried host",
                ))
            },
        );
        let outcome = match blocked {
            Ok(event) => Outcome::Delivered(event),
            Err(error) => Outcome::RuntimeFailed(error),
        };
        self.evidence.boundaries = self.evidence.boundaries.saturating_add(1);
        self.record(Operation::Next, Origin::Dialogue(span), outcome, Vec::new())?;
        acknowledge_effect(
            &mut self.session,
            request.id.clone(),
            acknowledgement.clone(),
        )
        .map_err(|error| NativeError::dialogue(error, source))?;
        self.record(
            Operation::Acknowledge,
            Origin::Dialogue(request.source_span.clone()),
            Outcome::Acknowledged {
                request,
                acknowledgement,
            },
            Vec::new(),
        )
    }

    pub(super) fn external_fuel(&mut self) -> NativeResult<()> {
        let action = "ferryman.fuel_ferry";
        self.verify(action, None)?;
        let successor = self
            .scenario
            .town
            .model
            .apply(&self.oracle, action)?
            .ok_or_else(|| NativeError::analysis("external reference transition disabled"))?;
        self.perform(action, Some(successor), None, Operation::FuelFerry)
    }

    fn perform(
        &mut self,
        action: &str,
        successor: Option<State>,
        source: Option<Source>,
        operation: Operation,
    ) -> NativeResult<()> {
        let expected_enabled = successor.is_some();
        let expected = self
            .scenario
            .town
            .model
            .snapshot(successor.as_ref().unwrap_or(&self.oracle))?;
        let accepted = projection::apply(&mut self.host.borrow_mut(), action)?;
        self.evidence.attempted = self.evidence.attempted.saturating_add(1);
        if accepted {
            self.evidence.accepted = self.evidence.accepted.saturating_add(1);
        }
        let actual = projection::capture(&mut self.host.borrow_mut(), &self.scenario.town.model)?;
        if accepted != expected_enabled || actual != expected {
            return Err(NativeError::conformance(Difference {
                action: action.into(),
                source,
                expected,
                actual,
                expected_enabled: Some(expected_enabled),
                actual_enabled: Some(accepted),
                verified_prefix_actions: self.evidence.verified,
                accepted_host_actions: self.evidence.accepted,
            }));
        }
        let transition = self
            .scenario
            .town
            .model
            .transitions()
            .find(|transition| transition.id == action)
            .cloned()
            .ok_or_else(|| NativeError::analysis("missing applied transition"))?;
        let origin = Origin::Host(transition.provenance.clone());
        let outcome = if let Some(state) = successor {
            self.oracle = state;
            self.evidence.verified = self.evidence.verified.saturating_add(1);
            Outcome::Applied(transition)
        } else {
            Outcome::Disabled {
                provenance: source.unwrap_or_else(|| transition.provenance.clone()),
                transition,
            }
        };
        self.record(operation, origin, outcome, Vec::new())
    }
}
