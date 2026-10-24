mod cli;
mod conformance;
mod contract;
mod error;
mod host;
mod input;
mod output;
#[cfg(test)]
mod tests;

use std::{error::Error, process::ExitCode};

/// Run the fixture-specific offline projection check or emit a complete request.
///
/// # Errors
/// Returns file and output failures. Input and conformance failures have typed wire outcomes.
pub fn run() -> Result<ExitCode, Box<dyn Error>> {
    cli::run()
}
