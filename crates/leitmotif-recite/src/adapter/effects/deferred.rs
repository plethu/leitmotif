use leitmotif::{Model, State};
use recite_core::compiled::{CompiledEffect, CompiledEffectMode};
use recite_runtime::{DialogueEffectMode, DialogueSession};

use super::super::error::caller;
use super::{
    AppliedEffect, EffectError, EffectErrorKind,
    registry::{Registry, runtime_mode},
};

/// Deferred requests mapped to host transitions after Recite ends the scene.
/// The host owns application order, commits, and recovery from a failed position.
pub struct DeferredEffects<'model> {
    registry: Registry<'model>,
}

impl<'model> DeferredEffects<'model> {
    /// Creates an empty registry bound to this model's declarations.
    #[must_use]
    pub const fn new(model: &'model Model) -> Self {
        Self {
            registry: Registry::new(model),
        }
    }

    /// Maps an exact deferred compiled call to a declared host transition.
    ///
    /// # Errors
    /// Rejects other modes, duplicate signatures and undeclared transition IDs.
    #[track_caller]
    pub fn bind(&mut self, effect: &CompiledEffect, transition: &str) -> Result<(), EffectError> {
        let provenance = caller();
        if effect.mode != CompiledEffectMode::Deferred {
            return Err(EffectError {
                kind: EffectErrorKind::WrongMode {
                    expected: DialogueEffectMode::Deferred,
                    actual: runtime_mode(effect.mode),
                },
                provenance,
            });
        }
        self.registry.bind(effect, transition, provenance)
    }

    /// Applies one occurrence from an ended session's authoritative retained list.
    /// The host handles `End`, iterates its delivered list in order, and commits
    /// each returned state before advancing to the next position. On failure,
    /// prior host commits remain; this method neither rolls back nor applies a suffix.
    ///
    /// Repeated occurrences can have identical request IDs. Pair queue positions
    /// with the host's transaction identity when recording progress. This registry
    /// does not track commits, suppress duplicates or enforce a scheduling policy.
    ///
    /// # Errors
    /// Rejects sessions that have not ended, positions outside the list, unmapped
    /// calls, disabled transitions and invalid host states. It never changes the session.
    #[track_caller]
    pub fn apply_at<'request>(
        &'request self,
        session: &'request DialogueSession,
        position: usize,
        state: &State,
    ) -> Result<AppliedEffect<'request>, EffectError> {
        if !session.is_ended() {
            return Err(EffectError {
                kind: EffectErrorKind::NotEnded,
                provenance: caller(),
            });
        }
        let Some(request) = session.deferred_effects().get(position) else {
            return Err(EffectError {
                kind: EffectErrorKind::InvalidDeferredPosition {
                    position,
                    len: session.deferred_effects().len(),
                },
                provenance: caller(),
            });
        };
        self.registry.apply(request, state)
    }
}
