use leitmotif::{Snapshot, Source, TransitionInfo};
use recite_core::{ChoiceId, LineId, SourceSpan};
use recite_runtime::{
    ConditionEvaluationError, ConditionExpectedType, ConditionValue, DialogueEffectRequest,
    DialogueError, DialogueEvent, EffectAck, preview::PreviewConditionArgument,
    snapshot::DialogueSessionSnapshot,
};

#[derive(Clone, Debug, PartialEq)]
pub(super) enum Operation {
    Next,
    Choose(ChoiceId),
    Immediate,
    Blocking,
    Acknowledge,
    FuelFerry,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct Query {
    pub(super) function: String,
    pub(super) arguments: Vec<PreviewConditionArgument>,
    pub(super) expected: ConditionExpectedType,
    pub(super) result: Result<ConditionValue, ConditionEvaluationError>,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) enum Outcome {
    Delivered(DialogueEvent),
    RuntimeFailed(DialogueError),
    Applied(TransitionInfo),
    Disabled {
        transition: TransitionInfo,
        provenance: Source,
    },
    Acknowledged {
        request: DialogueEffectRequest,
        acknowledgement: EffectAck,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub(super) enum Origin {
    Dialogue(SourceSpan),
    Host(Source),
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct Boundary {
    pub(super) operation: Operation,
    pub(super) origin: Origin,
    pub(super) outcome: Outcome,
    pub(super) queries: Vec<Query>,
    pub(super) host: Snapshot,
    pub(super) session: DialogueSessionSnapshot,
    pub(super) pending: Option<DialogueEffectRequest>,
    pub(super) selected_lines: Vec<LineId>,
}
