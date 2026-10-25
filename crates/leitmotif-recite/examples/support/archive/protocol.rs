use recite_core::ChoiceId;
use recite_runtime::{DialogueChoice, DialogueEffectRequest};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RestoreWindow {
    BeforeApplication,
    AfterCommit,
}
impl RestoreWindow {
    pub(super) const fn transition(self) -> &'static str {
        match self {
            Self::BeforeApplication => "restore_before",
            Self::AfterCommit => "restore_after",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum HostOutcome {
    Unapplied,
    Applied,
    Denied,
}
impl HostOutcome {
    pub(super) const fn restore_window(self) -> RestoreWindow {
        match self {
            Self::Unapplied => RestoreWindow::BeforeApplication,
            Self::Applied | Self::Denied => RestoreWindow::AfterCommit,
        }
    }
}

// This is the fixture host's scheduling state, not a replacement runtime session.
// Every request and enabled choice is checked against the actual session/event.
#[derive(Clone, Debug)]
pub(super) enum Schedule {
    Traverse,
    Immediate(DialogueEffectRequest),
    Prompt(Vec<DialogueChoice>),
    Pending {
        outcome: HostOutcome,
        redelivery: bool,
    },
    Deferred(usize),
    Done,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Action {
    Next,
    ApplyImmediate,
    Choose(ChoiceId),
    RemoveKey,
    ApplyUnlock,
    Deny,
    Restore(RestoreWindow),
    Acknowledge,
    ApplyDeferred(usize),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Limits {
    pub(super) max_nodes: usize,
    pub(super) max_depth: usize,
}

pub(super) const PROTOCOL: &str = concat!(
    "key removal at most once while the displayed prompt is pending,
",
    "  before choosing; one restore before application and one after commitment;
",
    "  redelivery immediately after restore; Applied -> Completed, Denied -> Failed;
",
    "  deferred occurrences applied in reached order after End"
);
