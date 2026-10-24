use super::super::{
    evidence::{ActionSource, Outcome, QueryObservation, owned_argument},
    protocol::Schedule,
};
use super::{Execution, Node, RunResult, RuntimeOperation};
use leitmotif::State;
use recite_core::compiled::{CompiledArgument, CompiledStatementKind};
use recite_runtime::snapshot::snapshot_session;
use recite_runtime::{
    ConditionQuery, ConditionValue, DialogueContext, DialogueEffectMode, DialogueEvent, choose,
    next,
};
use std::cell::RefCell;

impl Execution<'_> {
    pub(super) fn deliver(
        &self,
        node: &mut Node,
        operation: &RuntimeOperation,
        queries: &RefCell<Vec<QueryObservation>>,
    ) -> RunResult<(ActionSource, Outcome)> {
        let context = self
            .scenario
            .conditions
            .context(&self.scenario.model, &node.host);
        let recording = |query: ConditionQuery<'_>| {
            let result = context.evaluate_condition(query);
            queries.borrow_mut().push(QueryObservation {
                function: query.function().into(),
                arguments: query.arguments().into_iter().map(owned_argument).collect(),
                expected: query.expected_type(),
                result: result.clone(),
            });
            result
        };
        let operation_source = self.operation_source(node, operation)?;
        let delivered = if let RuntimeOperation::Choose(id) = operation {
            choose(
                &self.scenario.asset,
                &mut node.session,
                id.clone(),
                &recording,
            )
        } else {
            next(&self.scenario.asset, &mut node.session, &recording)
        };
        let delivered = match delivered {
            Ok(event) => event,
            Err(error) => {
                node.schedule = Schedule::Failed;
                return Ok((operation_source, Outcome::RuntimeFailed(error)));
            }
        };
        let location = if matches!(operation, RuntimeOperation::Choose(_)) {
            operation_source
        } else {
            self.event_source(&delivered)?
        };
        Self::accept(node, &delivered, operation)?;
        Ok((location, Outcome::Delivered(delivered)))
    }
    pub(super) fn check_queries(
        &self,
        state: &State,
        queries: &[QueryObservation],
        outcome: &Outcome,
    ) -> RunResult<()> {
        if queries.iter().any(|query| query.result.is_err())
            && !matches!(outcome, Outcome::RuntimeFailed(_))
        {
            return Err("errored query was paired with a successful runtime outcome".into());
        }
        for query in queries {
            let expected = match query.function.as_str() {
                "has_key" => ConditionValue::Bool(self.scenario.has_key(state)?),
                "archive_access" => {
                    ConditionValue::EnumVariant(self.scenario.access(state)?.symbol().into())
                }
                _ => return Err("fixture queried an undeclared host function".into()),
            };
            if query.arguments != [CompiledArgument::Identifier("archive".into())]
                || query.expected != expected.kind()
                || query
                    .result
                    .as_ref()
                    .is_ok_and(|actual| *actual != expected)
            {
                return Err("runtime query diverged from the typed host projection".into());
            }
        }
        Ok(())
    }

    pub(super) fn accept(
        node: &mut Node,
        event: &DialogueEvent,
        operation: &RuntimeOperation,
    ) -> RunResult<()> {
        match event {
            DialogueEvent::Line(line) => {
                node.selected_lines.push(line.id.clone());
                node.schedule = Schedule::Traverse;
            }
            DialogueEvent::Prompt { line, choices } => {
                if let Some(line) = line {
                    node.selected_lines.push(line.id.clone());
                }
                node.schedule = Schedule::Prompt(choices.clone());
            }
            DialogueEvent::Effect(request) => match request.mode {
                DialogueEffectMode::Immediate => {
                    node.schedule = Schedule::Immediate(request.clone());
                }
                DialogueEffectMode::Blocking => {
                    let outcome = operation.host_outcome();
                    node.schedule = Schedule::Pending {
                        outcome,
                        redelivery: false,
                    };
                }
                DialogueEffectMode::Deferred => {
                    return Err("runtime emitted deferred work before End".into());
                }
            },
            DialogueEvent::End { deferred_effects } => {
                if deferred_effects != node.session.deferred_effects() {
                    return Err("End delivery differs from retained queue".into());
                }
                node.schedule = if deferred_effects.is_empty() {
                    Schedule::Done
                } else {
                    Schedule::Deferred(0)
                };
            }
        }
        Ok(())
    }

    fn operation_source(
        &self,
        node: &Node,
        operation: &RuntimeOperation,
    ) -> RunResult<ActionSource> {
        let source_map = if let RuntimeOperation::Choose(id) = operation {
            self.scenario
                .asset
                .choices
                .iter()
                .find(|choice| choice.id == *id)
                .ok_or("chosen choice absent from source")?
                .source_map
        } else if let Some(request) = node.session.pending_effect() {
            return Ok(ActionSource::Dialogue(request.source_span.clone()));
        } else {
            let snapshot = snapshot_session(&node.session);
            self.scenario
                .asset
                .statements
                .get(usize::try_from(snapshot.next_statement)?)
                .ok_or("next statement absent from source")?
                .source_map
        };
        Ok(ActionSource::Dialogue(
            self.scenario
                .asset
                .source_maps
                .get(usize::try_from(source_map.as_u32())?)
                .ok_or("operation source map missing")?
                .span
                .clone(),
        ))
    }

    fn event_source(&self, event: &DialogueEvent) -> RunResult<ActionSource> {
        let source_map = match event {
            DialogueEvent::Effect(request) => {
                return Ok(ActionSource::Dialogue(request.source_span.clone()));
            }
            DialogueEvent::Line(line)
            | DialogueEvent::Prompt {
                line: Some(line), ..
            } => {
                self.scenario
                    .asset
                    .lines
                    .iter()
                    .find(|candidate| candidate.id == line.id)
                    .ok_or("delivered line absent from asset")?
                    .source_map
            }
            DialogueEvent::End { .. } => {
                self.scenario
                    .asset
                    .statements
                    .iter()
                    .rev()
                    .find(|statement| matches!(statement.kind, CompiledStatementKind::End))
                    .ok_or("fixture has no End")?
                    .source_map
            }
            DialogueEvent::Prompt { line: None, .. } => {
                return Err("fixture prompt has no line".into());
            }
        };
        let span = self
            .scenario
            .asset
            .source_maps
            .get(usize::try_from(source_map.as_u32())?)
            .ok_or("source map missing")?
            .span
            .clone();
        Ok(ActionSource::Dialogue(span))
    }
}
