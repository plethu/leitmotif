//! Generate controlled compiled inputs for the actual native-engine test driver.
mod fixtures;

use clap::Parser;
use std::{error::Error, path::PathBuf};

pub use fixtures::write;

#[derive(Parser)]
struct Args {
    output: PathBuf,
}

fn main() -> Result<(), Box<dyn Error>> {
    write(&Args::parse().output)
}
