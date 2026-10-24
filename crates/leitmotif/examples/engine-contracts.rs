//! A fixture-specific Rust host and offline analysis boundary for engine consumers.
#[path = "support/engine_contracts_fixture.rs"]
mod fixture;

use std::{error::Error, process::ExitCode};

pub use fixture::run;

fn main() -> Result<ExitCode, Box<dyn Error>> {
    run()
}
