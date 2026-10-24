use recite_compiler::compile::{CompileInput, CompileOptions, compile_inputs};
use recite_core::compiled::{
    CompiledAssetId, CompiledDialogue, CompilerVersion, SchemaFingerprint, SourceMapId,
};
use std::error::Error;

pub(super) const PATH: &str = "harbour.recite";
// Rust formatting owns the authored artifact; the actual pinned compiler validates it.
pub(super) const TEXT: &str = concat!(
    ":: harbour default\n",
    "! immediate share_warning(keeper)\n",
    "> decision@10000000000000000001\n",
    "  One flask of oil remains. Light the beacon or fuel the ferry?\n",
    "  ? beacon@10000000000000000002 requires=(has_oil(harbour))\n",
    "    Light the beacon.\n",
    "    -> beacon\n",
    "  ? ferry@10000000000000000003 requires=(has_oil(harbour))\n",
    "    Fuel the ferry.\n",
    "    -> ferry\n",
    ":: beacon\n",
    "! blocking light_beacon(keeper)\n",
    ":if beacon_lit(keeper)\n",
    "  ! immediate shelter(harbour)\n",
    "-> result\n",
    ":: ferry\n",
    "! blocking fuel_ferry(ferryman)\n",
    ":if ferry_ready(ferryman)\n",
    "  ! immediate depart(harbour)\n",
    "-> result\n",
    ":: result\n",
    ":match harbour_ending(harbour)\n",
    "  :case open\n",
    "    > open@10000000000000000004\n",
    "      The requested oil was unavailable. The harbour remains open.\n",
    "  :case sheltered\n",
    "    > sheltered@10000000000000000005\n",
    "      The beacon is lit. The harbour shelters for the storm.\n",
    "  :case departed\n",
    "    > departed@10000000000000000006\n",
    "      The ferry is fuelled. The last boat leaves the harbour.\n",
    "-> END\n",
);

pub(super) fn compile(text: &str) -> Result<CompiledDialogue, Box<dyn Error>> {
    let report = compile_inputs(
        [CompileInput::new(PATH, text)],
        CompileOptions::new(
            CompilerVersion::new("0.1.0")?,
            CompiledAssetId::new("harbour")?,
            SourceMapId::new("harbour.map")?,
            SchemaFingerprint::NoSchema,
        ),
    )?;
    if !report.diagnostics.is_empty() {
        return Err(format!("harbour source diagnostics: {:?}", report.diagnostics).into());
    }
    Ok(report
        .asset
        .ok_or("harbour compiler emitted no asset")?
        .dialogue)
}
