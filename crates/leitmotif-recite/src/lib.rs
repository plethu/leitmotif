//! Typed finite host bindings for Recite guards and immediate requests.
//!
//! Calls are matched by function and exact typed arguments. Guard lowering
//! validates every branch before analysis; runtime evaluation still short-circuits.
//! Immediate requests map to declared host transitions without owning traversal.
//! The host retains ownership of both the compiled dialogue and finite state.
mod adapter;

pub use adapter::{
    AppliedEffect, Bindings, EffectError, EffectErrorKind, Error, ErrorKind, ImmediateEffects,
    RuntimeContext,
};
