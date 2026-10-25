//! Explicit authoring/CI analysis of a headless Bevy-owned harbour.
#[path = "../../../crates/leitmotif/examples/support/composed_town/mod.rs"]
mod composed_town;
#[path = "host.rs"]
mod host;
#[cfg(test)]
#[path = "tests/mod.rs"]
mod tests;
#[path = "verification.rs"]
mod verification;

use leitmotif::{Snapshot, Source};
use std::collections::BTreeSet;
use std::error::Error as StdError;
use std::fmt::{Display, Formatter, Result as FmtResult};
use thiserror::Error;

#[derive(Debug, Error)]
enum Error {
    #[error(transparent)]
    Model(#[from] leitmotif::Error),
    #[error("unsupported authored action: {0}")]
    Unsupported(String),
    #[error("the game's harbour entity is absent")]
    MissingHarbour,
    #[error("the update schedule did not execute the pending action")]
    MissingExecution,
    #[error("live state differs from the captured analysis snapshot")]
    StaleSnapshot,
    #[error("host/model divergence: {0}")]
    Divergence(Box<Difference>),
    #[error("analysis witness does not match the authored model")]
    InvalidWitness,
    #[error("reachable graph exceeds the six-state fixture budget")]
    Budget,
}
#[derive(Debug)]
struct Difference {
    action: &'static str,
    source: Source,
    expected_enabled: bool,
    accepted: bool,
    expected: Snapshot,
    actual: Snapshot,
    verified_prefix: usize,
    accepted_actions: usize,
}
/// Run the fixed headless conformance demonstration.
///
/// # Errors
/// Returns projection, model or independently executed host failures.
pub fn run() -> Result<(), Box<dyn StdError>> {
    verification::run()?;
    Ok(())
}

impl Display for Difference {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        writeln!(f, "{}", self.action)?;
        let keys: BTreeSet<_> = self
            .expected
            .values
            .keys()
            .chain(self.actual.values.keys())
            .collect();
        for key in keys {
            let expected = self.expected.values.get(key);
            let actual = self.actual.values.get(key);
            if expected != actual {
                let expected = expected.map_or_else(|| "<missing>".into(), |v| format!("{v:?}"));
                let actual = actual.map_or_else(|| "<missing>".into(), |v| format!("{v:?}"));
                writeln!(f, "  {key}: expected {expected}, actual {actual}")?;
            }
        }
        writeln!(
            f,
            "  source: {}:{}:{}",
            self.source.file, self.source.line, self.source.column
        )?;
        writeln!(
            f,
            "  enabled: expected {}, accepted {}",
            self.expected_enabled, self.accepted
        )?;
        for (label, snapshot) in [("expected", &self.expected), ("actual", &self.actual)] {
            writeln!(f, "  {label} snapshot:")?;
            for (key, value) in &snapshot.values {
                writeln!(f, "    {key}: {value:?}")?;
            }
        }
        write!(
            f,
            "  execution: verified prefix {}, accepted actions {}",
            self.verified_prefix, self.accepted_actions
        )
    }
}
