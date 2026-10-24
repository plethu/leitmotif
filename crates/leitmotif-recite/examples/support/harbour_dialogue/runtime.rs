mod effects;

use leitmotif::{Source, State, TransitionInfo};
use leitmotif_recite::{BlockingEffects, EffectError, EffectErrorKind, ImmediateEffects};
use recite_core::{
    ChoiceId, LineId, SourceSpan,
    compiled::{CompiledArgument, CompiledEffectMode},
};
use recite_runtime::{
    ConditionQuery, DialogueChoice, DialogueContext, DialogueEffectMode, DialogueEffectRequest,
    DialogueError, DialogueEvent, DialogueSession, choose, next, snapshot::snapshot_session,
    start_scene,
};
use std::cell::RefCell;

use super::{
    RunResult,
    evidence::{Boundary, Operation, Origin, Outcome, Query},
    scenario::Scenario,
};

#[derive(Clone, Copy)]
enum HostOutcome {
    Unapplied,
    Applied,
    Disabled,
}
enum Phase {
    Traverse,
    Immediate(DialogueEffectRequest),
    Prompt(Vec<DialogueChoice>),
    Pending(HostOutcome),
    Done,
    Failed,
}

pub(super) struct Execution<'scenario> {
    scenario: &'scenario Scenario,
    session: DialogueSession,
    pub(super) host: State,
    immediate: ImmediateEffects<'scenario>,
    blocking: BlockingEffects<'scenario>,
    phase: Phase,
    selected_lines: Vec<LineId>,
    pub(super) trace: Vec<Boundary>,
}

impl<'scenario> Execution<'scenario> {
    pub(super) fn fresh(scenario: &'scenario Scenario) -> RunResult<Self> {
        let mut immediate = ImmediateEffects::new(&scenario.town.model);
        let mut blocking = BlockingEffects::new(&scenario.town.model);
        for effect in &scenario.asset.effects {
            let span = &scenario
                .asset
                .source_maps
                .get(usize::try_from(effect.source_map.as_u32())?)
                .ok_or("effect source map missing")?
                .span;
            let provenance = Source {
                file: span.file.clone(),
                portable_file: Some(span.file.clone()),
                line: span.start.line(),
                column: span.start.column(),
            };
            let (transition, actor, mode) = match effect.function.as_str() {
                "share_warning" => (
                    "keeper.share_warning",
                    "keeper",
                    CompiledEffectMode::Immediate,
                ),
                "light_beacon" => (
                    "keeper.light_beacon",
                    "keeper",
                    CompiledEffectMode::Blocking,
                ),
                "fuel_ferry" => (
                    "ferryman.fuel_ferry",
                    "ferryman",
                    CompiledEffectMode::Blocking,
                ),
                "shelter" => ("harbour.shelter", "harbour", CompiledEffectMode::Immediate),
                "depart" => ("harbour.depart", "harbour", CompiledEffectMode::Immediate),
                _ => {
                    return Err(EffectError {
                        kind: EffectErrorKind::Unmapped(effect.function.clone()),
                        provenance,
                    }
                    .into());
                }
            };
            if effect.args != [CompiledArgument::Identifier(actor.into())] {
                return Err(EffectError {
                    kind: EffectErrorKind::Unmapped(effect.function.clone()),
                    provenance,
                }
                .into());
            }
            let registered = if mode == CompiledEffectMode::Immediate {
                immediate.bind(effect, transition)
            } else {
                blocking.bind(effect, transition)
            };
            registered.map_err(|mut error| {
                error.provenance = provenance;
                error
            })?;
        }
        Ok(Self {
            scenario,
            session: start_scene(&scenario.asset, None)?,
            host: scenario.town.initial.clone(),
            immediate,
            blocking,
            phase: Phase::Traverse,
            selected_lines: Vec::new(),
            trace: Vec::new(),
        })
    }

    pub(super) fn choice(&self, id: &str) -> RunResult<ChoiceId> {
        let Phase::Prompt(choices) = &self.phase else {
            return Err("no displayed harbour prompt".into());
        };
        choices
            .iter()
            .find(|choice| choice.id.as_str() == id && choice.availability.is_available)
            .map(|choice| choice.id.clone())
            .ok_or_else(|| "requested harbour choice is unavailable".into())
    }

    pub(super) const fn ended(&self) -> bool {
        matches!(self.phase, Phase::Done)
    }

    pub(super) fn step(&mut self, operation: Operation) -> RunResult<()> {
        if self.trace.len() >= 24 {
            return Err("harbour interaction exceeds its 24-boundary limit".into());
        }
        let queries = RefCell::new(Vec::new());
        let (origin, outcome) = match &operation {
            Operation::Next if matches!(self.phase, Phase::Traverse | Phase::Pending(_)) => {
                self.deliver(None, &queries)?
            }
            Operation::Choose(id) if matches!(self.phase, Phase::Prompt(_)) => {
                self.deliver(Some(id), &queries)?
            }
            Operation::Immediate => self.apply_immediate()?,
            Operation::Blocking => self.apply_blocking()?,
            Operation::Acknowledge => self.acknowledge()?,
            Operation::FuelFerry if matches!(self.phase, Phase::Prompt(_)) => {
                let transition = self.transition("ferryman.fuel_ferry")?;
                self.host = self
                    .scenario
                    .town
                    .model
                    .apply(&self.host, &transition.id)?
                    .ok_or("external ferry action disabled")?;
                (
                    Origin::Host(transition.provenance.clone()),
                    Outcome::Applied(transition),
                )
            }
            _ => return Err("operation is not enabled at this harbour boundary".into()),
        };
        self.trace.push(Boundary {
            operation,
            origin,
            outcome,
            queries: queries.into_inner(),
            host: self.scenario.town.model.snapshot(&self.host)?,
            session: snapshot_session(&self.session),
            pending: self.session.pending_effect().cloned(),
            selected_lines: self.selected_lines.clone(),
        });
        Ok(())
    }

    fn deliver(
        &mut self,
        selection: Option<&ChoiceId>,
        queries: &RefCell<Vec<Query>>,
    ) -> RunResult<(Origin, Outcome)> {
        let span = self.operation_span(selection)?;
        let context = self
            .scenario
            .conditions
            .context(&self.scenario.town.model, &self.host);
        let recording = |query: ConditionQuery<'_>| {
            let result = context.evaluate_condition(query);
            queries.borrow_mut().push(Query {
                function: query.function().into(),
                arguments: query.arguments().into_iter().map(Into::into).collect(),
                expected: query.expected_type(),
                result: result.clone(),
            });
            result
        };
        let event = match selection {
            Some(id) => choose(
                &self.scenario.asset,
                &mut self.session,
                id.clone(),
                &recording,
            ),
            None => next(&self.scenario.asset, &mut self.session, &recording),
        };
        let event = match event {
            Ok(event) => event,
            Err(error) => {
                if !matches!(error, DialogueError::EffectPending { .. }) {
                    self.phase = Phase::Failed;
                }
                return Ok((Origin::Dialogue(span), Outcome::RuntimeFailed(error)));
            }
        };
        match &event {
            DialogueEvent::Line(line) => {
                self.selected_lines.push(line.id.clone());
                self.phase = Phase::Traverse;
            }
            DialogueEvent::Prompt { line, choices } => {
                if let Some(line) = line {
                    self.selected_lines.push(line.id.clone());
                }
                self.phase = Phase::Prompt(choices.clone());
            }
            DialogueEvent::Effect(request) => match request.mode {
                DialogueEffectMode::Immediate => self.phase = Phase::Immediate(request.clone()),
                DialogueEffectMode::Blocking => {
                    if !matches!(self.phase, Phase::Pending(_)) {
                        self.phase = Phase::Pending(HostOutcome::Unapplied);
                    }
                }
                DialogueEffectMode::Deferred => return Err("unexpected deferred delivery".into()),
            },
            DialogueEvent::End { deferred_effects } => {
                if !deferred_effects.is_empty() {
                    return Err("unexpected deferred queue".into());
                }
                self.phase = Phase::Done;
            }
        }
        Ok((Origin::Dialogue(span), Outcome::Delivered(event)))
    }

    fn transition(&self, id: &str) -> RunResult<TransitionInfo> {
        self.scenario
            .town
            .model
            .transitions()
            .find(|transition| transition.id == id)
            .cloned()
            .ok_or_else(|| "harbour transition missing".into())
    }

    fn operation_span(&self, selection: Option<&ChoiceId>) -> RunResult<SourceSpan> {
        if let Some(request) = self.session.pending_effect() {
            return Ok(request.source_span.clone());
        }
        let index = if let Some(id) = selection {
            self.scenario
                .asset
                .choices
                .iter()
                .find(|choice| choice.id == *id)
                .ok_or("choice source missing")?
                .source_map
        } else {
            let snapshot = snapshot_session(&self.session);
            self.scenario
                .asset
                .statements
                .get(usize::try_from(snapshot.next_statement)?)
                .or_else(|| self.scenario.asset.statements.last())
                .ok_or("statement source missing")?
                .source_map
        };
        Ok(self
            .scenario
            .asset
            .source_maps
            .get(usize::try_from(index.as_u32())?)
            .ok_or("source map missing")?
            .span
            .clone())
    }
}

pub(super) fn replay(scenario: &Scenario, evidence: &[Boundary]) -> RunResult<()> {
    let mut fresh = Execution::fresh(scenario)?;
    for boundary in evidence {
        fresh.step(boundary.operation.clone())?;
        if fresh.trace.last() != Some(boundary) {
            return Err("fresh harbour replay diverged".into());
        }
    }
    Ok(())
}
