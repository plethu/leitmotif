//! Local bounded symbolic-town comparison; no supported backend API.
mod fixture;
pub use fixture::run;
use std::error::Error;
fn main() -> Result<(), Box<dyn Error>> {
    run()
}
