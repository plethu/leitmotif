//! Typed finite host bindings for Recite guards and host effect requests.
//!
//! Calls are matched by function and exact typed arguments. Guard lowering
//! validates every branch before analysis; runtime evaluation still short-circuits.
//! Effect requests map to declared host transitions without owning traversal.
//! Recite retains pending blocking requests until the host explicitly acknowledges them.
//! The host retains ownership of both the compiled dialogue and finite state.
mod adapter;

pub use adapter::{
    AppliedEffect, Bindings, BlockingEffects, EffectError, EffectErrorKind, Error, ErrorKind,
    ImmediateEffects, RuntimeContext,
};
