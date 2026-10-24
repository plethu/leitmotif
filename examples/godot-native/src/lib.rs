//! Experimental native Godot consumer of the composed harbour.

mod harbour;

#[allow(
    unsafe_code,
    reason = "godot-rust requires an unsafe ExtensionLibrary impl and generates the native ABI entry point"
)]
mod entry;
