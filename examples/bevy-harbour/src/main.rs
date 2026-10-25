//! Explicit authoring/CI analysis of a headless Bevy-owned harbour.
mod fixture;
pub use fixture::run;
use std::error::Error;
fn main() -> Result<(), Box<dyn Error>> {
    run()
}
