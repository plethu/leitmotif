use leitmotif_recite::EffectErrorKind;
use recite_runtime::{EffectAck, acknowledge_effect};

use super::super::{
    RunResult,
    evidence::{Origin, Outcome},
};
use super::{Execution, HostOutcome, Phase};

impl Execution<'_> {
    pub(super) fn apply_immediate(&mut self) -> RunResult<(Origin, Outcome)> {
        let Phase::Immediate(request) = &self.phase else {
            return Err("no immediate harbour request".into());
        };
        let applied = self.immediate.apply(request, &self.host)?;
        let origin = Origin::Dialogue(applied.request().source_span.clone());
        let transition = applied.transition().clone();
        self.host = applied.into_state();
        self.phase = Phase::Traverse;
        Ok((origin, Outcome::Applied(transition)))
    }

    pub(super) fn apply_blocking(&mut self) -> RunResult<(Origin, Outcome)> {
        if !matches!(self.phase, Phase::Pending(HostOutcome::Unapplied)) {
            return Err("blocking request was already handled or is absent".into());
        }
        let origin = Origin::Dialogue(
            self.session
                .pending_effect()
                .ok_or("pending request absent")?
                .source_span
                .clone(),
        );
        match self.blocking.apply_pending(&self.session, &self.host) {
            Ok(applied) => {
                let transition = applied.transition().clone();
                self.host = applied.into_state();
                self.phase = Phase::Pending(HostOutcome::Applied);
                Ok((origin, Outcome::Applied(transition)))
            }
            Err(error) => {
                let EffectErrorKind::DisabledTransition(id) = error.kind else {
                    return Err(error.into());
                };
                let transition = self.transition(&id)?;
                self.phase = Phase::Pending(HostOutcome::Disabled);
                Ok((
                    origin,
                    Outcome::Disabled {
                        transition,
                        provenance: error.provenance,
                    },
                ))
            }
        }
    }

    pub(super) fn acknowledge(&mut self) -> RunResult<(Origin, Outcome)> {
        let acknowledgement = match self.phase {
            Phase::Pending(HostOutcome::Applied) => EffectAck::Completed,
            Phase::Pending(HostOutcome::Disabled) => EffectAck::Failed {
                reason: "the oil was committed to the other route".into(),
            },
            _ => return Err("no handled harbour effect to acknowledge".into()),
        };
        let request = self
            .session
            .pending_effect()
            .ok_or("pending request absent")?
            .clone();
        acknowledge_effect(
            &mut self.session,
            request.id.clone(),
            acknowledgement.clone(),
        )?;
        self.phase = Phase::Traverse;
        Ok((
            Origin::Dialogue(request.source_span.clone()),
            Outcome::Acknowledged {
                request,
                acknowledgement,
            },
        ))
    }
}
