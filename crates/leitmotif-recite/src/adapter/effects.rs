use leitmotif::{Source, State, TransitionInfo};
use recite_runtime::{DialogueEffectMode, DialogueEffectRequest};
use thiserror::Error;

mod blocking;
mod immediate;
mod registry;

pub use blocking::BlockingEffects;
pub use immediate::ImmediateEffects;

/// Evidence of one successful host application, retaining the exact emitted request.
/// This is not a delivery ledger: applying a request again evaluates the transition again.
#[derive(Debug)]
pub struct AppliedEffect<'request> {
    request: &'request DialogueEffectRequest,
    transition: &'request TransitionInfo,
    state: State,
}

impl<'request> AppliedEffect<'request> {
    /// The original runtime identity, typed arguments and full dialogue source span.
    #[must_use]
    pub const fn request(&self) -> &'request DialogueEffectRequest {
        self.request
    }

    /// The declared host transition, including its guard, assignments and Rust source.
    #[must_use]
    pub const fn transition(&self) -> &'request TransitionInfo {
        self.transition
    }

    /// Transfers the resulting state to its host owner.
    #[must_use]
    pub fn into_state(self) -> State {
        self.state
    }
}

/// A registration or application failure linked to its Rust or dialogue source.
#[derive(Debug, Error)]
#[error("{kind} at {provenance}")]
pub struct EffectError {
    /// The failure category, retaining any underlying model error.
    #[source]
    pub kind: EffectErrorKind,
    /// Binding declaration or emitted request's dialogue span start.
    pub provenance: Source,
}

/// Explicit failures at a request/host-transition boundary.
#[derive(Debug, Error)]
pub enum EffectErrorKind {
    /// A mode-specific registry received another delivery mode.
    #[error("expected {expected:?} effect, received {actual:?}")]
    WrongMode {
        /// Mode accepted by this registry.
        expected: DialogueEffectMode,
        /// Mode of the supplied declaration or request.
        actual: DialogueEffectMode,
    },
    /// The session has no live pending blocking request to apply.
    #[error("no blocking effect is pending")]
    NoPending,
    /// An exact effect signature was registered more than once.
    #[error("duplicate effect binding {0}")]
    Duplicate(String),
    /// Registration names no declared transition in the registry's model.
    #[error("undeclared host transition {0}")]
    UnknownTransition(String),
    /// No exact function-and-typed-arguments binding exists.
    #[error("unmapped effect {0}")]
    Unmapped(String),
    /// The mapped transition's guard is false in the supplied pre-state.
    #[error("host transition is disabled: {0}")]
    DisabledTransition(String),
    /// The model rejected application; no successor state was produced.
    #[error("host transition failed: {0}")]
    Host(#[source] Box<leitmotif::Error>),
}
