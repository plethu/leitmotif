//! Local bounded symbolic-town comparison; no supported backend API.
#[path = "support/symbolic_fixture.rs"]
mod symbolic_fixture;
use std::error::Error;
pub use symbolic_fixture::run;
fn main() -> Result<(), Box<dyn Error>> {
    run()
}
