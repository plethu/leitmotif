#[path = "../../examples/support/harbour_dialogue/source.rs"]
mod source;

use recite_core::compiled::{
    CompiledDialogue, CompiledMatchPattern, CompiledStatementKind,
    encode_compiled_dialogue_messagepack,
};
use std::{error::Error, fs, path::Path, result::Result as StdResult};

type Result<T> = StdResult<T, Box<dyn Error>>;

/// Write valid, officially encoded counterexamples for the native boundary tests.
///
/// # Errors
/// Returns compiler, encoding or output failures.
pub fn write(output: &Path) -> Result<()> {
    fs::create_dir_all(output)?;
    for (name, text) in [
        ("wrong-effect-argument", source::TEXT.replace("share_warning(keeper)", "share_warning(ferryman)")),
        ("wrong-effect-mode", source::TEXT.replace("! immediate share_warning(keeper)", "! blocking share_warning(keeper)")),
        ("missing-choice", source::TEXT.replace("10000000000000000002", "10000000000000000012")),
        ("no-blocking", source::TEXT.replace("    -> beacon", "    -> result")),
        ("wrong-delivery", source::TEXT.replace("    -> beacon", "    -> harbour")),
        ("unexpected-blocking", source::TEXT.replace("! immediate share_warning(keeper)", "-> beacon")),
        ("no-guards", source::TEXT.replace(" requires=(has_oil(harbour))", "")),
        ("traversal-limit", source::TEXT.replace("! immediate share_warning(keeper)\n", "! immediate share_warning(keeper)\n-> choose\n:: choose\n").replace(" requires=(has_oil(harbour))", "").replace("! blocking light_beacon(keeper)\n:if beacon_lit(keeper)\n  ! immediate shelter(harbour)\n-> result", "! blocking light_beacon(keeper)\n-> choose")),
    ] { save(output, name, &source::compile(&text)?)?; }

    let mut payload = source::compile(source::TEXT)?.into_payload();
    for map in &mut payload.source_maps {
        map.span.end = Some(map.span.start);
    }
    save(
        output,
        "ranged-source",
        &CompiledDialogue::prepare(payload)?,
    )?;

    let mut payload = source::compile(source::TEXT)?.into_payload();
    for statement in &mut payload.statements {
        if let CompiledStatementKind::Prompt { line, .. } = &mut statement.kind {
            *line = None;
        }
    }
    save(
        output,
        "no-prompt-line",
        &CompiledDialogue::prepare(payload)?,
    )?;

    let mut payload = source::compile(source::TEXT)?.into_payload();
    for source in &mut payload.sources {
        source.path = "/external/harbour.recite".into();
    }
    for map in &mut payload.source_maps {
        map.span.file = "/external/harbour.recite".into();
        map.span.end = None;
    }
    save(
        output,
        "absolute-source",
        &CompiledDialogue::prepare(payload)?,
    )?;

    let mut payload = source::compile(source::TEXT)?.into_payload();
    for statement in &mut payload.statements {
        if let CompiledStatementKind::Match { arms, .. } = &mut statement.kind {
            arms.len = 1;
        }
    }
    if let Some(arm) = payload.match_arms.first_mut() {
        arm.pattern = CompiledMatchPattern::Wildcard;
    }
    save(output, "wildcard", &CompiledDialogue::prepare(payload)?)?;
    Ok(())
}

fn save(output: &Path, name: &str, asset: &CompiledDialogue) -> Result<()> {
    fs::write(
        output.join(format!("{name}.recitec")),
        encode_compiled_dialogue_messagepack(asset)?,
    )?;
    Ok(())
}
