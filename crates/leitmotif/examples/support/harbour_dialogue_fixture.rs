//! The shared harbour model driven by the actual Recite runtime.
use std::{error::Error, fs, path::PathBuf};

use clap::Parser;

mod harbour_dialogue_owner;

pub use harbour_dialogue_owner::{compiled_asset, run};

#[derive(Parser)]
#[command(about = "Run the harbour dialogue or compile its asset for an engine host")]
struct Cli {
    /// Write the officially encoded dialogue asset instead of running the fixture.
    #[arg(long)]
    compiled_asset: Option<PathBuf>,
}

fn main() -> Result<(), Box<dyn Error>> {
    match Cli::parse().compiled_asset {
        Some(path) => fs::write(path, compiled_asset()?)?,
        None => run()?,
    }
    Ok(())
}
