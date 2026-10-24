use godot::init::{ExtensionLibrary, gdextension};

struct HarbourExtension;

// SAFETY: The fixture loads this extension once into Godot 4.6 on its main
// thread. The default godot-rust safeguards remain enabled. No callbacks retain
// engine objects across unload or send them to worker threads. This module owns
// only godot-rust's required native entry point; host implementation forbids unsafe.
#[gdextension]
unsafe impl ExtensionLibrary for HarbourExtension {}
