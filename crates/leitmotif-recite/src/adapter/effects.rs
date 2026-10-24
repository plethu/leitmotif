use leitmotif::{Model, Source, State, TransitionInfo};
use recite_core::{
    ScalarValue,
    compiled::{CompiledArgument, CompiledEffect, CompiledEffectMode},
};
use recite_runtime::{DialogueEffectArgument, DialogueEffectMode, DialogueEffectRequest};
use thiserror::Error;

use super::error::{caller, source};

struct Binding<'model> {
    function: String,
    args: Vec<DialogueEffectArgument>,
    transition: &'model TransitionInfo,
}

/// Immediate request signatures mapped to declared transitions of one host model.
/// The registry borrows the model and never owns dialogue traversal or host mutation.
pub struct ImmediateEffects<'model> {
    model: &'model Model,
    bindings: Vec<Binding<'model>>,
}

impl<'model> ImmediateEffects<'model> {
    /// Creates an empty registry bound to this model's declarations.
    #[must_use]
    pub const fn new(model: &'model Model) -> Self {
        Self {
            model,
            bindings: Vec::new(),
        }
    }

    /// Maps an immediate compiled call, with exact typed arguments, to a host transition.
    /// Emitted runtime request IDs are preserved as opaque identities, not binding keys.
    ///
    /// # Errors
    /// Rejects other modes, duplicate signatures, and undeclared transition IDs.
    #[track_caller]
    pub fn bind(&mut self, effect: &CompiledEffect, transition: &str) -> Result<(), EffectError> {
        let provenance = caller();
        if effect.mode != CompiledEffectMode::Immediate {
            return Err(EffectError {
                kind: EffectErrorKind::NotImmediate,
                provenance,
            });
        }
        let args = effect.args.iter().map(runtime_argument).collect::<Vec<_>>();
        if self
            .bindings
            .iter()
            .any(|binding| binding.function == effect.function && binding.args == args)
        {
            return Err(EffectError {
                kind: EffectErrorKind::Duplicate(effect.function.clone()),
                provenance,
            });
        }
        let declared = self
            .model
            .transitions()
            .find(|candidate| candidate.id == transition)
            .ok_or_else(|| EffectError {
                kind: EffectErrorKind::UnknownTransition(transition.to_owned()),
                provenance,
            })?;
        self.bindings.push(Binding {
            function: effect.function.clone(),
            args,
            transition: declared,
        });
        Ok(())
    }

    /// Applies the mapped transition without changing the supplied state or any dialogue session.
    /// The host must commit the returned state before its next traversal call.
    /// Recite session rollback cannot undo a state already committed by the host.
    ///
    /// # Errors
    /// Rejects non-immediate or unmapped requests, disabled guards, and invalid host states.
    /// Failure leaves the input state untouched and does not rewind dialogue that emitted the request.
    pub fn apply<'request>(
        &'request self,
        request: &'request DialogueEffectRequest,
        state: &State,
    ) -> Result<AppliedEffect<'request>, EffectError> {
        let at_request = |kind| EffectError {
            kind,
            provenance: source(&request.source_span),
        };
        if request.mode != DialogueEffectMode::Immediate {
            return Err(at_request(EffectErrorKind::NotImmediate));
        }
        let binding = matching_binding(self.bindings.iter(), request)
            .ok_or_else(|| at_request(EffectErrorKind::Unmapped(request.function.clone())))?;
        let successor = self
            .model
            .apply(state, &binding.transition.id)
            .map_err(|error| at_request(EffectErrorKind::Host(Box::new(error))))?
            .ok_or_else(|| {
                at_request(EffectErrorKind::DisabledTransition(
                    binding.transition.id.clone(),
                ))
            })?;
        Ok(AppliedEffect {
            request,
            transition: binding.transition,
            state: successor,
        })
    }
}

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

/// Explicit failures at the immediate-request/host-transition boundary.
#[derive(Debug, Error)]
pub enum EffectErrorKind {
    /// This registry accepts immediate requests only.
    #[error("effect is not immediate")]
    NotImmediate,
    /// An exact effect signature was registered more than once.
    #[error("duplicate effect binding {0}")]
    Duplicate(String),
    /// Registration names no declared transition in the registry's model.
    #[error("undeclared host transition {0}")]
    UnknownTransition(String),
    /// No exact function-and-typed-arguments binding exists.
    #[error("unmapped immediate effect {0}")]
    Unmapped(String),
    /// The mapped transition's guard is false in the supplied pre-state.
    #[error("host transition is disabled: {0}")]
    DisabledTransition(String),
    /// The model rejected application; no successor state was produced.
    #[error("host transition failed: {0}")]
    Host(#[source] Box<leitmotif::Error>),
}

fn runtime_argument(argument: &CompiledArgument) -> DialogueEffectArgument {
    match argument {
        CompiledArgument::Identifier(value) => DialogueEffectArgument::Identifier(value.clone()),
        CompiledArgument::Value(ScalarValue::String(value)) => {
            DialogueEffectArgument::String(value.clone())
        }
        CompiledArgument::Value(ScalarValue::Integer(value)) => {
            DialogueEffectArgument::Integer(*value)
        }
        CompiledArgument::Value(ScalarValue::Float(value)) => DialogueEffectArgument::Float(*value),
        CompiledArgument::Value(ScalarValue::Boolean(value)) => {
            DialogueEffectArgument::Boolean(*value)
        }
    }
}

fn matching_binding<'binding>(
    mut bindings: impl Iterator<Item = &'binding Binding<'binding>>,
    request: &DialogueEffectRequest,
) -> Option<&'binding Binding<'binding>> {
    bindings.find(|binding| binding.function == request.function && binding.args == request.args)
}

#[cfg(test)]
mod tests;
