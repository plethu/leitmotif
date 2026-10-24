use recite_core::EffectId;

use godot::builtin::{PackedByteArray, VarArray, VarDictionary, Variant};
use godot::prelude::ToGodot;
use recite_runtime::{
    ConditionExpectedType, ConditionValue, DialogueEffectArgument, DialogueEffectMode,
    DialogueEffectRequest, DialogueError, DialogueEvent, EffectAck,
    preview::PreviewConditionArgument,
};

use super::super::error::{NativeError, provenance, snapshot};
use super::NativeResult;
use super::{
    evidence::{Boundary, Operation, Origin, Outcome, Query},
    source_at,
};

pub(super) struct Recorded {
    pub(super) boundary: Boundary,
    pub(super) session: Vec<u8>,
}

pub(super) fn encode(records: &[Recorded]) -> NativeResult<VarArray> {
    let mut output = VarArray::new();
    for record in records {
        let boundary = &record.boundary;
        let mut value = VarDictionary::new();
        let operation = match &boundary.operation {
            Operation::Next => "next",
            Operation::Choose(id) => {
                value.set("choice", id.as_str());
                "choose"
            }
            Operation::Immediate => "immediate",
            Operation::Blocking => "blocking",
            Operation::Acknowledge => "acknowledge",
            Operation::FuelFerry => "fuel_ferry",
        };
        value.set("operation", operation);
        value.set("origin", &origin(&boundary.origin));
        value.set("outcome", &outcome(&boundary.outcome));
        value.set("host", &snapshot(&boundary.host));
        value.set(
            "session_messagepack",
            &PackedByteArray::from(record.session.as_slice()),
        );
        value.set("next_statement", i64::from(boundary.session.next_statement));
        let pending = boundary
            .pending
            .as_ref()
            .map_or_else(Variant::nil, |pending| request(pending).to_variant());
        value.set("pending", &pending);
        let mut selected = VarArray::new();
        for line in &boundary.selected_lines {
            selected.push(&line.as_str().to_variant());
        }
        value.set("selected_lines", &selected);
        let mut queries = VarArray::new();
        for observation in &boundary.queries {
            queries.push(&query(observation)?.to_variant());
        }
        value.set("queries", &queries);
        output.push(&value.to_variant());
    }
    Ok(output)
}

fn origin(value: &Origin) -> VarDictionary {
    let mut result = match value {
        Origin::Dialogue(span) => {
            let mut source = provenance(&source_at(span));
            if let Some(end) = span.end {
                source.set("end_line", i64::from(end.line()));
                source.set("end_column", i64::from(end.column()));
            }
            source
        }
        Origin::Host(source) => provenance(source),
    };
    result.set(
        "kind",
        match value {
            Origin::Dialogue(_) => "dialogue",
            Origin::Host(_) => "host",
        },
    );
    result
}

fn outcome(value: &Outcome) -> VarDictionary {
    let mut result = VarDictionary::new();
    let kind = match value {
        Outcome::Delivered(event) => {
            result.set("event", &event_value(event));
            "delivered"
        }
        Outcome::RuntimeFailed(error) => {
            let failure = runtime_failure(error);
            result.set("code", failure.code);
            result.set(
                "effect",
                &failure
                    .effect
                    .map_or_else(Variant::nil, |effect| effect.as_str().to_variant()),
            );
            result.set("error", error.to_string());
            "runtime_failed"
        }
        Outcome::Applied(transition) => {
            result.set("action", transition.id.as_str());
            result.set("source", &provenance(&transition.provenance));
            "applied"
        }
        Outcome::Disabled {
            transition,
            provenance: source,
        } => {
            result.set("action", transition.id.as_str());
            result.set("host_source", &provenance(&transition.provenance));
            result.set("dialogue_source", &provenance(source));
            "disabled"
        }
        Outcome::Acknowledged {
            request: pending,
            acknowledgement,
        } => {
            result.set("request", &request(pending));
            match acknowledgement {
                EffectAck::Completed => {
                    result.set("succeeded", true);
                }
                EffectAck::Failed { reason } => {
                    result.set("succeeded", false);
                    result.set("reason", reason.as_str());
                }
            }
            "acknowledged"
        }
    };
    result.set("kind", kind);
    result
}

fn event_value(event: &DialogueEvent) -> VarDictionary {
    let mut result = VarDictionary::new();
    let kind = match event {
        DialogueEvent::Line(line) => {
            result.set("line", line.id.as_str());
            result.set("text", line.text.as_str());
            "line"
        }
        DialogueEvent::Prompt { line, choices } => {
            if let Some(line) = line {
                result.set("line", line.id.as_str());
                result.set("text", line.text.as_str());
            }
            let mut values = VarArray::new();
            for choice in choices {
                let mut value = VarDictionary::new();
                value.set("id", choice.id.as_str());
                value.set("available", choice.availability.is_available);
                value.set("text", choice.text.as_str());
                values.push(&value.to_variant());
            }
            result.set("choices", &values);
            "prompt"
        }
        DialogueEvent::Effect(effect) => {
            result.set("request", &request(effect));
            "effect"
        }
        DialogueEvent::End { deferred_effects } => {
            let mut effects = VarArray::new();
            for effect in deferred_effects {
                effects.push(&request(effect).to_variant());
            }
            result.set("deferred", &effects);
            "end"
        }
    };
    result.set("kind", kind);
    result
}

fn request(value: &DialogueEffectRequest) -> VarDictionary {
    let mut result = VarDictionary::new();
    result.set("id", value.id.as_str());
    result.set("function", value.function.as_str());
    result.set(
        "mode",
        match value.mode {
            DialogueEffectMode::Immediate => "immediate",
            DialogueEffectMode::Blocking => "blocking",
            DialogueEffectMode::Deferred => "deferred",
        },
    );
    result.set("source", &provenance(&source_at(&value.source_span)));
    let mut arguments = VarArray::new();
    for value in &value.args {
        let (kind, value) = match value {
            DialogueEffectArgument::Identifier(value) => ("identifier", value.to_variant()),
            DialogueEffectArgument::String(value) => ("string", value.to_variant()),
            DialogueEffectArgument::Integer(value) => ("integer", value.to_variant()),
            DialogueEffectArgument::Float(value) => ("float", value.to_variant()),
            DialogueEffectArgument::Boolean(value) => ("boolean", value.to_variant()),
        };
        arguments.push(&argument(kind, &value).to_variant());
    }
    result.set("arguments", &arguments);
    result
}

fn query(value: &Query) -> NativeResult<VarDictionary> {
    let mut result = VarDictionary::new();
    result.set("function", value.function.as_str());
    result.set(
        "expected",
        match value.expected {
            ConditionExpectedType::Bool => "bool",
            ConditionExpectedType::Enum => "enum",
        },
    );
    let mut arguments = VarArray::new();
    for value in &value.arguments {
        let (kind, value) = match value {
            PreviewConditionArgument::Identifier(value) => ("identifier", value.to_variant()),
            PreviewConditionArgument::String(value) => ("string", value.to_variant()),
            PreviewConditionArgument::Integer(value) => ("integer", value.to_variant()),
            PreviewConditionArgument::Float(value) => ("float", value.to_variant()),
            PreviewConditionArgument::Boolean(value) => ("boolean", value.to_variant()),
            _ => {
                return Err(NativeError::mapping(
                    "query argument",
                    None,
                    "unsupported argument kind",
                ));
            }
        };
        arguments.push(&argument(kind, &value).to_variant());
    }
    result.set("arguments", &arguments);
    match &value.result {
        Ok(ConditionValue::Bool(value)) => result.set("value", *value),
        Ok(ConditionValue::EnumVariant(value)) => result.set("value", value.as_str()),
        Err(error) => result.set("error", error.to_string()),
    }
    Ok(result)
}

fn argument(kind: &str, value: &Variant) -> VarDictionary {
    let mut result = VarDictionary::new();
    result.set("kind", kind);
    result.set("value", value);
    result
}

// Keep runtime failure identity independent of Godot allocation so the wire
// classification can be checked without starting an engine process.
struct RuntimeFailure<'a> {
    code: &'static str,
    effect: Option<&'a EffectId>,
}

const fn runtime_failure(error: &DialogueError) -> RuntimeFailure<'_> {
    match error {
        DialogueError::EffectPending { effect } => RuntimeFailure {
            code: "effect_pending",
            effect: Some(effect),
        },
        _ => RuntimeFailure {
            code: "runtime_error",
            effect: None,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_failure_projection_preserves_pending_identity_without_inventing_one_for_other_errors()
     {
        let effect = EffectId::new("pending-oil").unwrap();
        let pending = DialogueError::EffectPending {
            effect: effect.clone(),
        };
        let projected = runtime_failure(&pending);
        assert_eq!(projected.code, "effect_pending");
        assert_eq!(projected.effect, Some(&effect));
        let unknown = DialogueError::UnknownBlock {
            block: "missing".into(),
        };
        let projected = runtime_failure(&unknown);
        assert_eq!(projected.code, "runtime_error");
        assert_eq!(projected.effect, None);
    }
}
