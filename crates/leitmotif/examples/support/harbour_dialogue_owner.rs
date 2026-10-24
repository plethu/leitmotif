//! Keep both consumers under the same private fixture boundary.
#[path = "composed_town/mod.rs"]
mod composed_town;
#[path = "../../../leitmotif-recite/examples/support/harbour_dialogue/mod.rs"]
mod harbour_dialogue;

pub use harbour_dialogue::{compiled_asset, run};
