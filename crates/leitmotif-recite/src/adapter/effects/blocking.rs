use leitmotif::{Model, State};
use recite_core::compiled::{CompiledEffect, CompiledEffectMode};
use recite_runtime::{DialogueEffectMode, DialogueSession};

use super::super::error::caller;
use super::{
    AppliedEffect, EffectError, EffectErrorKind,
    registry::{Registry, runtime_mode},
};

/// Blocking requests mapped to host transitions, with Recite owning pending work.
/// Host application, acknowledgement and continuation remain separate operations.
pub struct BlockingEffects<'model> {
    registry: Registry<'model>,
}

impl<'model> BlockingEffects<'model> {
    /// Creates an empty registry bound to this model's declarations.
    #[must_use]
    pub const fn new(model: &'model Model) -> Self {
        Self {
            registry: Registry::new(model),
        }
    }

    /// Maps an exact blocking compiled call to a declared host transition.
    ///
    /// # Errors
    /// Rejects other modes, duplicate signatures and undeclared transition IDs.
    #[track_caller]
    pub fn bind(&mut self, effect: &CompiledEffect, transition: &str) -> Result<(), EffectError> {
        let provenance = caller();
        if effect.mode != CompiledEffectMode::Blocking {
            return Err(EffectError {
                kind: EffectErrorKind::WrongMode {
                    expected: DialogueEffectMode::Blocking,
                    actual: runtime_mode(effect.mode),
                },
                provenance,
            });
        }
        self.registry.bind(effect, transition, provenance)
    }

    /// Applies the exact live pending request borrowed from the Recite session.
    /// Commit the returned host state before calling Recite's `acknowledge_effect`
    /// with that request ID and an `EffectAck`, then continue dialogue separately.
    /// Both acknowledgement variants resume the same continuation without host mutation.
    ///
    /// Restoring a session may re-emit a previously applied request. The host must
    /// reconcile its saved assignment before deciding whether application is needed;
    /// this registry does not track deliveries or guarantee exactly-once execution.
    ///
    /// # Errors
    /// Rejects absent pending work, unmapped calls, disabled guards and invalid states.
    /// Neither success nor failure acknowledges the request or changes the session.
    #[track_caller]
    pub fn apply_pending<'request>(
        &'request self,
        session: &'request DialogueSession,
        state: &State,
    ) -> Result<AppliedEffect<'request>, EffectError> {
        let Some(request) = session.pending_effect() else {
            return Err(EffectError {
                kind: EffectErrorKind::NoPending,
                provenance: caller(),
            });
        };
        self.registry.apply(request, state)
    }
}
