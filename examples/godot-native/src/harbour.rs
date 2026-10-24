#![forbid(unsafe_code)]

#[path = "../../../crates/leitmotif/examples/support/composed_town/mod.rs"]
mod composed_town;

mod dialogue;
mod error;
mod execution;
mod projection;

use error::NativeError;

use godot::builtin::{GString, PackedByteArray, VarDictionary};
use godot::classes::{INode, Node};
use godot::prelude::{Gd, GodotClass, godot_api};

#[derive(GodotClass)]
#[class(init, base=Node)]
struct HarbourNative;

#[godot_api]
impl INode for HarbourNative {}

#[godot_api]
impl HarbourNative {
    #[func]
    fn dialogue(host: Gd<Node>, route: GString, asset: PackedByteArray) -> VarDictionary {
        result(dialogue::check(host, &String::from(route), asset))
    }

    #[func]
    fn audit(host: Gd<Node>) -> VarDictionary {
        result(execution::audit(host))
    }

    #[func]
    fn replay(host: Gd<Node>, query: GString, depth: i64) -> VarDictionary {
        result(execution::replay(host, &String::from(query), depth))
    }
}

type NativeResult<T> = Result<T, NativeError>;

fn result(outcome: NativeResult<VarDictionary>) -> VarDictionary {
    match outcome {
        Ok(mut value) => {
            value.set("ok", true);
            value
        }
        Err(error) => error::encode(&error),
    }
}
