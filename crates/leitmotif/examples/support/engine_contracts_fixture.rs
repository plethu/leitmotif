//! Compose the existing harbour model with one independent engine-contract consumer.
#[path = "composed_town/mod.rs"]
mod composed_town;
#[path = "engine_contracts/mod.rs"]
mod engine_contracts;

pub use engine_contracts::run;
