use leitmotif::{Snapshot, Source, TransitionInfo};
use recite_core::{LineId, ScalarValue, SourceSpan, compiled::CompiledArgument};
use recite_runtime::{
    ConditionArgument, ConditionEvaluationError, ConditionExpectedType, ConditionValue,
    DialogueEffectRequest, DialogueError, DialogueEvent, EffectAck,
    snapshot::DialogueSessionSnapshot,
};

use super::protocol::Action;

#[derive(Clone, Debug, PartialEq)]
pub(super) struct QueryObservation {
    pub(super) function: String,
    pub(super) arguments: Vec<CompiledArgument>,
    pub(super) expected: ConditionExpectedType,
    pub(super) result: Result<ConditionValue, ConditionEvaluationError>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum ActionSource {
    Dialogue(SourceSpan),
    Host(Source),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct DisabledEffect {
    pub(super) transition: String,
    pub(super) provenance: Source,
    pub(super) diagnostic: String,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) enum Outcome {
    Delivered(DialogueEvent),
    RuntimeFailed(DialogueError),
    HostApplied(TransitionInfo),
    HostDisabled {
        failure: DisabledEffect,
        denial: TransitionInfo,
    },
    Restored(TransitionInfo),
    Acknowledged {
        request: DialogueEffectRequest,
        acknowledgement: EffectAck,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct Boundary {
    pub(super) action: Action,
    pub(super) source: ActionSource,
    pub(super) outcome: Outcome,
    pub(super) queries: Vec<QueryObservation>,
    pub(super) host: Snapshot,
    pub(super) session: DialogueSessionSnapshot,
    pub(super) pending: Option<DialogueEffectRequest>,
    pub(super) deferred: Vec<DialogueEffectRequest>,
    pub(super) selected_lines: Vec<LineId>,
}

pub(super) fn owned_argument(argument: ConditionArgument<'_>) -> CompiledArgument {
    match argument {
        ConditionArgument::Identifier(value) => CompiledArgument::Identifier(value.into()),
        ConditionArgument::String(value) => {
            CompiledArgument::Value(ScalarValue::String(value.into()))
        }
        ConditionArgument::Integer(value) => CompiledArgument::Value(ScalarValue::Integer(value)),
        ConditionArgument::Float(value) => CompiledArgument::Value(ScalarValue::Float(value)),
        ConditionArgument::Boolean(value) => CompiledArgument::Value(ScalarValue::Boolean(value)),
    }
}
