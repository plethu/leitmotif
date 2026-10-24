//! The shared harbour model driven by the actual Recite runtime.
use std::error::Error;

mod harbour_dialogue_owner;

pub use harbour_dialogue_owner::run;

fn main() -> Result<(), Box<dyn Error>> {
    run()
}
