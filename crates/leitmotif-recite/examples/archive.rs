//! Run the archive-access integration fixture from a source checkout.
#[path = "support/archive/mod.rs"]
mod archive;

pub use archive::run;
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    run()
}

#[cfg(test)]
#[path = "../tests/archive/consumer.rs"]
mod consumer;
