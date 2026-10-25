//! Contract tests share the exact source paths used by the archive executable.
#[path = "archive/mod.rs"]
mod archive;

pub use archive::{run, run_cli};

#[path = "../../tests/archive/consumer.rs"]
mod consumer;
