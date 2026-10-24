//! Typed finite-fact bindings for Recite's compiled conditions.
//!
//! Calls are matched by function and exact typed arguments. Guard lowering
//! validates every branch before analysis; runtime evaluation still short-circuits.
//! The host retains ownership of both the compiled dialogue and finite state.
mod adapter;

pub use adapter::{Bindings, Error, ErrorKind, RuntimeContext};
