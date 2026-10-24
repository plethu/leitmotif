//! Scoped testimony in an original tide-plate mystery.
#[path = "support/mystery/mod.rs"]
mod mystery;
pub use mystery::run;
use std::error::Error;
fn main() -> Result<(), Box<dyn Error>> {
    run()
}
