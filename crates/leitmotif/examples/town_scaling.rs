//! Machine-readable measurements of finite town fixtures.

#[path = "support/measurement.rs"]
mod measurement;

pub use measurement::run;
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    run()
}
