use leitmotif::{Model, State};
use recite_core::compiled::{CompiledEffect, CompiledEffectMode};
use recite_runtime::{DialogueEffectMode, DialogueEffectRequest};

use super::super::error::{caller, source};
use super::{
    AppliedEffect, EffectError, EffectErrorKind,
    registry::{Registry, runtime_mode},
};

/// Immediate request signatures mapped to declared transitions of one host model.
/// The registry borrows the model and never owns dialogue traversal or host mutation.
pub struct ImmediateEffects<'model> {
    registry: Registry<'model>,
}

impl<'model> ImmediateEffects<'model> {
    /// Creates an empty registry bound to this model's declarations.
    #[must_use]
    pub const fn new(model: &'model Model) -> Self {
        Self {
            registry: Registry::new(model),
        }
    }

    /// Maps an immediate compiled call, with exact typed arguments, to a host transition.
    /// Emitted runtime request IDs are opaque identities, not binding keys.
    ///
    /// # Errors
    /// Rejects other modes, duplicate signatures, and undeclared transition IDs.
    #[track_caller]
    pub fn bind(&mut self, effect: &CompiledEffect, transition: &str) -> Result<(), EffectError> {
        let provenance = caller();
        if effect.mode != CompiledEffectMode::Immediate {
            return Err(EffectError {
                kind: EffectErrorKind::WrongMode {
                    expected: DialogueEffectMode::Immediate,
                    actual: runtime_mode(effect.mode),
                },
                provenance,
            });
        }
        self.registry.bind(effect, transition, provenance)
    }

    /// Applies the mapped transition without changing the supplied state or any session.
    /// The host must commit the returned state before its next traversal call.
    /// Recite rollback cannot undo an already committed host state.
    ///
    /// # Errors
    /// Rejects non-immediate/unmapped requests, disabled guards and invalid host states.
    /// Failure leaves the input state untouched and does not rewind dialogue.
    pub fn apply<'request>(
        &'request self,
        request: &'request DialogueEffectRequest,
        state: &State,
    ) -> Result<AppliedEffect<'request>, EffectError> {
        if request.mode != DialogueEffectMode::Immediate {
            return Err(EffectError {
                kind: EffectErrorKind::WrongMode {
                    expected: DialogueEffectMode::Immediate,
                    actual: request.mode,
                },
                provenance: source(&request.source_span),
            });
        }
        self.registry.apply(request, state)
    }
}
