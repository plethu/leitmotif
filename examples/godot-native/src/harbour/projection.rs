use std::collections::BTreeMap;

use godot::builtin::{GString, VarDictionary};
use godot::classes::Node;
use godot::prelude::Gd;
use leitmotif::{Model, Snapshot, Value};

use super::{NativeResult, error::NativeError};

pub(super) fn capture(host: &mut Gd<Node>, model: &Model) -> NativeResult<Snapshot> {
    let returned = host
        .try_call("Capture", &[])
        .map_err(|error| NativeError::host_call("Capture", error))?;
    let facts = returned.try_to::<VarDictionary>().map_err(|error| {
        NativeError::mapping(
            "",
            None,
            format!("Capture must return a Dictionary: {error}"),
        )
    })?;
    let mut values = BTreeMap::new();
    for fact in model.facts() {
        let input = facts.get(fact.key.as_str()).ok_or_else(|| {
            NativeError::mapping(&fact.key, Some(&fact.provenance), "missing fact")
        })?;
        let value =
            match fact.domain.first() {
                Some(Value::Bool(_)) => Value::Bool(input.try_to::<bool>().map_err(|error| {
                    NativeError::mapping(&fact.key, Some(&fact.provenance), error)
                })?),
                Some(Value::Symbol(_)) => Value::Symbol(
                    input
                        .try_to::<GString>()
                        .map_err(|error| {
                            NativeError::mapping(&fact.key, Some(&fact.provenance), error)
                        })?
                        .to_string(),
                ),
                None => {
                    return Err(NativeError::mapping(
                        &fact.key,
                        Some(&fact.provenance),
                        "empty finite domain",
                    ));
                }
            };
        if !fact.domain.contains(&value) {
            return Err(NativeError::mapping(
                &fact.key,
                Some(&fact.provenance),
                "unsupported finite value",
            ));
        }
        values.insert(fact.key.clone(), value);
    }
    if facts.len() != model.facts().len() {
        return Err(NativeError::mapping(
            "",
            None,
            "Capture must contain exactly the six harbour facts",
        ));
    }
    Ok(Snapshot { values })
}

pub(super) fn reset(host: &mut Gd<Node>) -> NativeResult<()> {
    host.try_call("Reset", &[])
        .map_err(|error| NativeError::host_call("Reset", error))?;
    Ok(())
}

pub(super) fn apply(host: &mut Gd<Node>, action: &str) -> NativeResult<bool> {
    use godot::prelude::ToGodot;

    host.try_call("Perform", &[action.to_variant()])
        .map_err(|error| NativeError::host_call("Perform", format!("{action}: {error}")))?
        .try_to::<bool>()
        .map_err(|error| {
            NativeError::mapping(action, None, format!("Perform must return bool: {error}"))
        })
}
