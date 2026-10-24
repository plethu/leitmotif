//! Run the archive-access integration fixture from a source checkout.
#[path = "support/archive/mod.rs"]
mod archive;

pub use archive::{run, run_cli};
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    run_cli()
}
