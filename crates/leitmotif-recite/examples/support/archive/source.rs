use recite_compiler::compile::{CompileInput, CompileOptions, compile_inputs};
use recite_core::compiled::{
    CompiledAssetId, CompiledDialogue, CompilerVersion, SchemaFingerprint, SourceMapId,
};
use std::error::Error;

pub(super) const PATH: &str = "archive.recite";

// Authored here so the pinned Rust formatter covers the consumer artifact.
// Compilation below validates the real Recite source, without a second parser.
pub(super) const TEXT: &str = concat!(
    ":: archive default\n",
    "! deferred record_visit(archive)\n",
    "! immediate expose_claim(archive)\n",
    "> claim@a90e86884f3aa3bf3ab7\n",
    "  The archivist says the original letter is behind this door.\n",
    "  ? unlock@baa4233d710cc23fd277 requires=(has_key(archive))\n",
    "    Open the archive.\n",
    "    -> unlock\n",
    "  ? leave@630ece67467f4cc775ca\n",
    "    Leave the letter where it is.\n",
    "    -> result\n",
    ":: unlock\n",
    "! blocking unlock_archive(archive)\n",
    "-> result\n",
    ":: result\n",
    ":match archive_access(archive)\n",
    "  :case closed\n",
    "    > closed@4cc0a18fca48337278d1\n",
    "      The archive stays shut.\n",
    "    ! deferred record_result(closed)\n",
    "  :case opened\n",
    "    > opened@4dff358182d02d9090e3\n",
    "      The lock gives, and the archive opens.\n",
    "    ! deferred record_result(opened)\n",
    "  :case denied\n",
    "    > denied@ebb571c54e9fa639e36f\n",
    "      The lock refuses. The archive stays shut.\n",
    "    ! deferred record_result(denied)\n",
    "-> END\n",
);

pub(super) fn compile(text: &str) -> Result<CompiledDialogue, Box<dyn Error>> {
    let report = compile_inputs(
        [CompileInput::new(PATH, text)],
        CompileOptions::new(
            CompilerVersion::new("0.1.0")?,
            CompiledAssetId::new("archive")?,
            SourceMapId::new("archive.map")?,
            SchemaFingerprint::NoSchema,
        ),
    )?;
    if !report.diagnostics.is_empty() {
        return Err(format!("archive source diagnostics: {:?}", report.diagnostics).into());
    }
    let asset = report
        .asset
        .ok_or("archive compiler emitted no asset")?
        .dialogue;
    Ok(asset)
}
