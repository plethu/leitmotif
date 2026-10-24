use godot::builtin::PackedByteArray;
use recite_core::compiled::decode_compiled_dialogue_messagepack;

use leitmotif_recite::{Error as BindingError, ErrorKind as BindingErrorKind};
use recite_core::{
    SourceSpan,
    compiled::{
        CompiledConditionCall, CompiledMatchPattern, CompiledStatementKind, MatchArmRange,
        SourceMapIndex,
    },
};

use super::{NativeError, NativeResult, Scenario};

pub(super) fn validate(scenario: &Scenario) -> NativeResult<()> {
    for choice in &scenario.asset.choices {
        if let Some(guard) = &choice.availability_requirement {
            scenario
                .conditions
                .guard(guard, span(scenario, choice.source_map)?)
                .map_err(|error| mapping(&error))?;
        }
    }
    for statement in &scenario.asset.statements {
        let source = span(scenario, statement.source_map)?;
        match &statement.kind {
            CompiledStatementKind::If { condition, .. } => {
                scenario
                    .conditions
                    .guard(condition, source)
                    .map_err(|error| mapping(&error))?;
            }
            CompiledStatementKind::Match { scrutinee, arms } => {
                validate_match(scenario, scrutinee, *arms, source)?;
            }
            _ => {}
        }
    }
    Ok(())
}

fn span(scenario: &Scenario, index: SourceMapIndex) -> NativeResult<&SourceSpan> {
    Ok(&scenario
        .asset
        .source_maps
        .get(usize::try_from(index.as_u32())?)
        .ok_or_else(|| NativeError::analysis("missing compiled source map"))?
        .span)
}

fn mapping(error: &BindingError) -> NativeError {
    let binding = match &error.kind {
        BindingErrorKind::Unmapped(binding)
        | BindingErrorKind::WrongType(binding)
        | BindingErrorKind::Duplicate(binding)
        | BindingErrorKind::UnknownVariant(binding) => binding.as_str(),
        BindingErrorKind::NonSymbolicDomain | BindingErrorKind::Unsupported(_) => "condition",
    };
    NativeError::mapping(binding, Some(&error.provenance), error)
}

fn validate_match(
    scenario: &Scenario,
    scrutinee: &CompiledConditionCall,
    arms: MatchArmRange,
    source: &SourceSpan,
) -> NativeResult<()> {
    // harbour_ending is this fixture's sole enum binding; `open` validates
    // its result type even for an asset containing only a wildcard arm.
    scenario
        .conditions
        .enum_is(scrutinee, "open", source)
        .map_err(|error| mapping(&error))?;
    let start = usize::try_from(arms.start.as_u32())?;
    let end = start
        .checked_add(usize::try_from(arms.len)?)
        .ok_or_else(|| NativeError::analysis("match arm range overflow"))?;
    for arm in scenario
        .asset
        .match_arms
        .get(start..end)
        .ok_or_else(|| NativeError::analysis("missing compiled match arms"))?
    {
        if let CompiledMatchPattern::Variant(variant) = &arm.pattern {
            scenario
                .conditions
                .enum_is(scrutinee, variant, span(scenario, arm.source_map)?)
                .map_err(|error| mapping(&error))?;
        }
    }
    Ok(())
}

impl TryFrom<PackedByteArray> for Scenario {
    type Error = NativeError;

    fn try_from(bytes: PackedByteArray) -> NativeResult<Self> {
        let asset = decode_compiled_dialogue_messagepack(bytes.as_slice())
            .map_err(NativeError::analysis)?;
        let scenario = Self::from_asset(asset)?;
        validate(&scenario)?;
        // The registries borrow their model. Validate all effect mappings now;
        // traversal constructs its own scoped registries from this owned scenario.
        super::effect_bindings::effect_bindings(&scenario.town.model, &scenario.asset)?;
        Ok(scenario)
    }
}
