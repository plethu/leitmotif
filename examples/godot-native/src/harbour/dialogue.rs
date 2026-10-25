mod asset;
mod effects;
mod trace;

#[path = "../../../../crates/leitmotif-recite/examples/support/harbour_dialogue/evidence.rs"]
mod evidence;

#[path = "../../../../crates/leitmotif-recite/examples/support/harbour_dialogue/effect_bindings.rs"]
mod effect_bindings;
#[path = "../../../../crates/leitmotif-recite/examples/support/harbour_dialogue/scenario.rs"]
mod scenario;

use std::{
    cell::{Cell, RefCell},
    error::Error,
    path::Path,
};

use evidence::{Boundary, Operation, Origin, Outcome, Query};
use godot::{
    builtin::{PackedByteArray, VarDictionary},
    classes::Node,
    prelude::Gd,
};
use leitmotif::{Snapshot, Source, State, Value};
use leitmotif_recite::{BlockingEffects, ImmediateEffects};
use recite_core::{ChoiceId, LineId, SourceSpan};
use recite_runtime::{
    ConditionEvaluationError, ConditionQuery, DialogueChoice, DialogueContext, DialogueEvent,
    DialogueSession, choose, next,
    snapshot::{encode_session_messagepack, snapshot_session},
    start_scene,
};

use super::{
    NativeResult,
    error::{Difference, NativeError},
    projection,
};
use scenario::Scenario;

type RunResult<T> = Result<T, Box<dyn Error>>;

#[derive(Clone, Copy)]
enum Route {
    Beacon,
    Ferry,
    Stale,
}
impl Route {
    fn parse(value: &str) -> NativeResult<Self> {
        match value {
            "beacon" => Ok(Self::Beacon),
            "ferry" => Ok(Self::Ferry),
            "stale" => Ok(Self::Stale),
            _ => Err(NativeError::rejected(format!(
                "unsupported harbour dialogue route {value}"
            ))),
        }
    }
    const fn choice(self) -> &'static str {
        match self {
            Self::Ferry => "10000000000000000003",
            Self::Beacon | Self::Stale => "10000000000000000002",
        }
    }
}

#[derive(Default)]
struct Evidence {
    attempted: u32,
    accepted: u32,
    verified: u32,
    boundaries: u32,
    queries: Cell<u32>,
    selected: Vec<LineId>,
    trace: Vec<trace::Recorded>,
    failed_ack: bool,
    dialogue_source: String,
    host_source: String,
}

struct LiveDialogue<'scenario> {
    scenario: &'scenario Scenario,
    host: RefCell<Gd<Node>>,
    oracle: State,
    session: DialogueSession,
    immediate: ImmediateEffects<'scenario>,
    blocking: BlockingEffects<'scenario>,
    evidence: Evidence,
}

pub(super) fn check(
    host: Gd<Node>,
    route: &str,
    asset: PackedByteArray,
) -> NativeResult<VarDictionary> {
    let route = Route::parse(route)?;
    let scenario = Scenario::try_from(asset)?;
    let (immediate, blocking) =
        effect_bindings::effect_bindings(&scenario.town.model, &scenario.asset)?;
    let session =
        start_scene(&scenario.asset, None).map_err(|error| NativeError::dialogue(error, None))?;
    let mut live = LiveDialogue {
        oracle: scenario.town.initial.clone(),
        scenario: &scenario,
        host: RefCell::new(host),
        session,
        immediate,
        blocking,
        evidence: Evidence::default(),
    };
    let actual = projection::capture(&mut live.host.borrow_mut(), &scenario.town.model)?;
    let expected = scenario.town.model.snapshot(&live.oracle)?;
    if actual != expected {
        return Err(NativeError::StaleInput { expected, actual });
    }
    live.run(route)?;
    live.outcome()
}

impl LiveDialogue<'_> {
    fn run(&mut self, route: Route) -> NativeResult<()> {
        loop {
            match self.event(None)? {
                DialogueEvent::Line(_) => {}
                DialogueEvent::Prompt { choices, .. } => self.prompt(route, &choices)?,
                DialogueEvent::Effect(request) => self.immediate_request(&request)?,
                DialogueEvent::End { deferred_effects } if deferred_effects.is_empty() => {
                    return Ok(());
                }
                DialogueEvent::End { .. } => {
                    return Err(NativeError::dialogue(
                        "unexpected deferred queue",
                        self.source(),
                    ));
                }
            }
        }
    }

    fn prompt(&mut self, route: Route, choices: &[DialogueChoice]) -> NativeResult<()> {
        let choice = choices
            .iter()
            .find(|choice| choice.id.as_str() == route.choice() && choice.availability.is_available)
            .ok_or_else(|| {
                NativeError::dialogue("native harbour choice unavailable", self.source())
            })?
            .id
            .clone();
        if matches!(route, Route::Stale) {
            self.external_fuel()?;
        }
        self.select(choice)
    }

    fn event(&mut self, selection: Option<ChoiceId>) -> NativeResult<DialogueEvent> {
        if self.evidence.boundaries >= 24 {
            return Err(NativeError::dialogue(
                "exceeded 24 runtime boundaries",
                self.source(),
            ));
        }
        let expected = self.scenario.town.model.snapshot(&self.oracle)?;
        let span = self.span(selection.as_ref())?;
        let source = Some(source_at(&span));
        let operation = selection
            .as_ref()
            .map_or(Operation::Next, |id| Operation::Choose(id.clone()));
        let queries = RefCell::new(Vec::new());
        let boundary_error = RefCell::new(None);
        let recording = |query: ConditionQuery<'_>| {
            self.evidence
                .queries
                .set(self.evidence.queries.get().saturating_add(1));
            let actual =
                projection::capture(&mut self.host.borrow_mut(), &self.scenario.town.model)
                    .map_err(|error| {
                        let message = error.to_string();
                        *boundary_error.borrow_mut() = Some(error);
                        ConditionEvaluationError::new(message)
                    })?;
            if actual != expected {
                *boundary_error.borrow_mut() = Some(NativeError::conformance(Difference {
                    action: query.function().into(),
                    source: source.clone(),
                    expected: expected.clone(),
                    actual,
                    expected_enabled: None,
                    actual_enabled: None,
                    verified_prefix_actions: self.evidence.verified,
                    accepted_host_actions: self.evidence.accepted,
                }));
                return Err(ConditionEvaluationError::new(
                    "live Godot state changed before query",
                ));
            }
            let result = self
                .scenario
                .conditions
                .context(&self.scenario.town.model, &self.oracle)
                .evaluate_condition(query);
            queries.borrow_mut().push(Query {
                function: query.function().into(),
                arguments: query.arguments().into_iter().map(Into::into).collect(),
                expected: query.expected_type(),
                result: result.clone(),
            });
            result
        };
        let result = match selection {
            Some(id) => choose(&self.scenario.asset, &mut self.session, id, &recording),
            None => next(&self.scenario.asset, &mut self.session, &recording),
        };
        self.evidence.boundaries = self.evidence.boundaries.saturating_add(1);
        if let Some(error) = boundary_error.into_inner() {
            return Err(error);
        }
        let event = result.map_err(|error| NativeError::dialogue(error, source))?;
        match &event {
            DialogueEvent::Line(line)
            | DialogueEvent::Prompt {
                line: Some(line), ..
            } => self.evidence.selected.push(line.id.clone()),
            _ => {}
        }
        self.record(
            operation,
            Origin::Dialogue(span),
            Outcome::Delivered(event.clone()),
            queries.into_inner(),
        )?;
        Ok(event)
    }

    fn verify(&self, action: &str, source: Option<Source>) -> NativeResult<Snapshot> {
        let actual = projection::capture(&mut self.host.borrow_mut(), &self.scenario.town.model)?;
        let expected = self.scenario.town.model.snapshot(&self.oracle)?;
        if actual != expected {
            return Err(NativeError::conformance(Difference {
                action: action.into(),
                source,
                expected,
                actual,
                expected_enabled: None,
                actual_enabled: None,
                verified_prefix_actions: self.evidence.verified,
                accepted_host_actions: self.evidence.accepted,
            }));
        }
        Ok(actual)
    }

    fn span(&self, choice: Option<&ChoiceId>) -> NativeResult<SourceSpan> {
        if let Some(request) = self.session.pending_effect() {
            return Ok(request.source_span.clone());
        }
        let index = if let Some(id) = choice {
            self.scenario
                .asset
                .choices
                .iter()
                .find(|choice| choice.id == *id)
                .ok_or_else(|| NativeError::analysis("missing selected choice"))?
                .source_map
        } else {
            let snapshot = snapshot_session(&self.session);
            self.scenario
                .asset
                .statements
                .get(usize::try_from(snapshot.next_statement)?)
                .or_else(|| self.scenario.asset.statements.last())
                .ok_or_else(|| NativeError::analysis("missing statement source"))?
                .source_map
        };
        Ok(self
            .scenario
            .asset
            .source_maps
            .get(usize::try_from(index.as_u32())?)
            .ok_or_else(|| NativeError::analysis("missing source map"))?
            .span
            .clone())
    }

    fn source(&self) -> Option<Source> {
        self.span(None).ok().map(|span| source_at(&span))
    }

    fn record(
        &mut self,
        operation: Operation,
        origin: Origin,
        outcome: Outcome,
        queries: Vec<Query>,
    ) -> NativeResult<()> {
        let boundary = Boundary {
            operation,
            origin,
            outcome,
            queries,
            host: projection::capture(&mut self.host.borrow_mut(), &self.scenario.town.model)?,
            session: snapshot_session(&self.session),
            pending: self.session.pending_effect().cloned(),
            selected_lines: self.evidence.selected.clone(),
        };
        let session = encode_session_messagepack(&self.session)
            .map_err(|error| NativeError::dialogue(error, self.source()))?;
        self.evidence
            .trace
            .push(trace::Recorded { boundary, session });
        Ok(())
    }

    fn outcome(&self) -> NativeResult<VarDictionary> {
        let final_state = self.verify("final dialogue state", self.source())?;
        let Some(Value::Symbol(ending)) = final_state.values.get("harbour.ending") else {
            return Err(NativeError::analysis("harbour ending is not symbolic"));
        };
        let mut result = VarDictionary::new();
        result.set("ending", ending.as_str());
        result.set("host_actions", i64::from(self.evidence.accepted));
        result.set("attempted_host_actions", i64::from(self.evidence.attempted));
        result.set("queries", i64::from(self.evidence.queries.get()));
        result.set("boundaries", i64::from(self.evidence.boundaries));
        result.set("failed_ack", self.evidence.failed_ack);
        result.set("dialogue_source", self.evidence.dialogue_source.clone());
        result.set("host_source", self.evidence.host_source.clone());
        result.set(
            "selected_lines",
            self.evidence
                .selected
                .iter()
                .map(LineId::as_str)
                .collect::<Vec<_>>()
                .join(","),
        );
        result.set("fingerprint", self.scenario.town.model.fingerprint());
        result.set("trace", &trace::encode(&self.evidence.trace)?);
        Ok(result)
    }
}

fn source_at(span: &SourceSpan) -> Source {
    Source {
        file: span.file.clone(),
        portable_file: (!Path::new(&span.file).is_absolute()).then(|| span.file.clone()),
        line: span.start.line(),
        column: span.start.column(),
    }
}
