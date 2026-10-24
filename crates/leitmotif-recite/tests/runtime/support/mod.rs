use std::error::Error;

use leitmotif::{Domain, Fact, Value};
use recite_compiler::compile::{CompileInput, CompileOptions, compile_inputs};
use recite_core::compiled::{
    CompiledAssetId, CompiledDialogue, CompilerVersion, SchemaFingerprint, SourceMapId,
};

pub(super) struct Enabled;
impl Fact for Enabled {
    type Value = bool;
    const KEY: &'static str = "enabled";
}
pub(super) struct Other;
impl Fact for Other {
    type Value = bool;
    const KEY: &'static str = "other";
}
#[derive(Clone, Copy)]
pub(super) enum Mood {
    Tired,
    Rested,
}
impl Domain for Mood {
    fn values() -> Vec<Self> {
        vec![Self::Tired, Self::Rested]
    }
    fn encode(self) -> Value {
        Value::Symbol(
            match self {
                Self::Tired => "tired",
                Self::Rested => "rested",
            }
            .into(),
        )
    }
}
pub(super) struct Stage;
impl Fact for Stage {
    type Value = Mood;
    const KEY: &'static str = "mood";
}

pub(super) fn compile(source: &str) -> Result<CompiledDialogue, Box<dyn Error>> {
    let report = compile_inputs(
        [CompileInput::new("story.recite", source)],
        CompileOptions::new(
            CompilerVersion::new("0.1.0")?,
            CompiledAssetId::new("story")?,
            SourceMapId::new("story.map")?,
            SchemaFingerprint::NoSchema,
        ),
    )?;
    if !report.diagnostics.is_empty() {
        return Err(format!("{:?}", report.diagnostics).into());
    }
    Ok(report.asset.ok_or("compiler emitted no asset")?.dialogue)
}
