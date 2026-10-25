use leitmotif::{Model, Source};
use leitmotif_recite::{BlockingEffects, EffectError, EffectErrorKind, ImmediateEffects};
use recite_core::compiled::{CompiledArgument, CompiledDialogue, CompiledEffectMode};
use std::path::Path;

use super::RunResult;

pub(super) fn effect_bindings<'model>(
    model: &'model Model,
    asset: &CompiledDialogue,
) -> RunResult<(ImmediateEffects<'model>, BlockingEffects<'model>)> {
    let mut immediate = ImmediateEffects::new(model);
    let mut blocking = BlockingEffects::new(model);
    for effect in &asset.effects {
        let span = &asset
            .source_maps
            .get(usize::try_from(effect.source_map.as_u32())?)
            .ok_or("effect source map missing")?
            .span;
        let provenance = Source {
            file: span.file.clone(),
            portable_file: (!Path::new(&span.file).is_absolute()).then(|| span.file.clone()),
            line: span.start.line(),
            column: span.start.column(),
        };
        let (transition, actor, mode) = match effect.function.as_str() {
            "share_warning" => (
                "keeper.share_warning",
                "keeper",
                CompiledEffectMode::Immediate,
            ),
            "light_beacon" => (
                "keeper.light_beacon",
                "keeper",
                CompiledEffectMode::Blocking,
            ),
            "fuel_ferry" => (
                "ferryman.fuel_ferry",
                "ferryman",
                CompiledEffectMode::Blocking,
            ),
            "shelter" => ("harbour.shelter", "harbour", CompiledEffectMode::Immediate),
            "depart" => ("harbour.depart", "harbour", CompiledEffectMode::Immediate),
            _ => {
                return Err(EffectError {
                    kind: EffectErrorKind::Unmapped(effect.function.clone()),
                    provenance,
                }
                .into());
            }
        };
        if effect.args != [CompiledArgument::Identifier(actor.into())] {
            return Err(EffectError {
                kind: EffectErrorKind::Unmapped(effect.function.clone()),
                provenance,
            }
            .into());
        }
        let registered = if mode == CompiledEffectMode::Immediate {
            immediate.bind(effect, transition)
        } else {
            blocking.bind(effect, transition)
        };
        registered.map_err(|mut error| {
            error.provenance = provenance;
            error
        })?;
    }
    Ok((immediate, blocking))
}
