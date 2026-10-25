//! Reproducible optional SMT versus finite-enumeration measurements.
#[path = "support/smt_comparison.rs"]
mod comparison;
pub use comparison::{Mystery, MysteryEvidence, mystery, run};
use std::error::Error;
fn main() -> Result<(), Box<dyn Error>> {
    run()
}
