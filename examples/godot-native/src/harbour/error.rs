use std::{error::Error, fmt, num::TryFromIntError};

use godot::builtin::{VarDictionary, Variant};
use godot::prelude::ToGodot;
use leitmotif::{Snapshot, Source, Value};
use leitmotif_recite::{EffectError, EffectErrorKind};

#[derive(Debug)]
pub(super) struct Difference {
    pub(super) action: String,
    pub(super) source: Option<Source>,
    pub(super) expected: Snapshot,
    pub(super) actual: Snapshot,
    pub(super) expected_enabled: Option<bool>,
    pub(super) actual_enabled: Option<bool>,
    pub(super) verified_prefix_actions: u32,
    pub(super) accepted_host_actions: u32,
}

#[derive(Debug)]
pub(super) enum NativeError {
    RejectedInput(String),
    StaleInput {
        expected: Snapshot,
        actual: Snapshot,
    },
    HostCall {
        operation: String,
        detail: String,
    },
    Mapping {
        binding: String,
        source: Option<Source>,
        detail: String,
    },
    Analysis {
        detail: String,
        source: Option<Source>,
    },
    Dialogue {
        detail: String,
        source: Option<Source>,
    },
    Conformance(Box<Difference>),
}

impl NativeError {
    pub(super) fn rejected(detail: impl fmt::Display) -> Self {
        Self::RejectedInput(detail.to_string())
    }
    pub(super) fn analysis(detail: impl fmt::Display) -> Self {
        Self::Analysis {
            detail: detail.to_string(),
            source: None,
        }
    }
    pub(super) fn dialogue(detail: impl fmt::Display, source: Option<Source>) -> Self {
        Self::Dialogue {
            detail: detail.to_string(),
            source,
        }
    }
    pub(super) fn conformance(difference: Difference) -> Self {
        Self::Conformance(Box::new(difference))
    }
    pub(super) fn host_call(operation: &str, detail: impl fmt::Display) -> Self {
        Self::HostCall {
            operation: operation.into(),
            detail: detail.to_string(),
        }
    }
    pub(super) fn mapping(
        binding: &str,
        source: Option<&Source>,
        detail: impl fmt::Display,
    ) -> Self {
        Self::Mapping {
            binding: binding.into(),
            source: source.cloned(),
            detail: detail.to_string(),
        }
    }
}

impl fmt::Display for NativeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RejectedInput(detail)
            | Self::HostCall { detail, .. }
            | Self::Mapping { detail, .. }
            | Self::Analysis { detail, .. }
            | Self::Dialogue { detail, .. } => formatter.write_str(detail),
            Self::StaleInput { .. } => {
                formatter.write_str("live host differs from the required initial projection")
            }
            Self::Conformance(difference) => {
                write!(formatter, "host diverged after {}", difference.action)
            }
        }
    }
}
impl Error for NativeError {}
impl From<String> for NativeError {
    fn from(detail: String) -> Self {
        Self::rejected(detail)
    }
}
impl From<&str> for NativeError {
    fn from(detail: &str) -> Self {
        Self::rejected(detail)
    }
}
impl From<TryFromIntError> for NativeError {
    fn from(error: TryFromIntError) -> Self {
        Self::rejected(error)
    }
}
impl From<Box<dyn Error>> for NativeError {
    fn from(error: Box<dyn Error>) -> Self {
        let error = match error.downcast::<EffectError>() {
            Ok(error) => {
                let binding = match &error.kind {
                    EffectErrorKind::Unmapped(binding) => binding.as_str(),
                    _ => "effect",
                };
                return Self::mapping(binding, Some(&error.provenance), &error);
            }
            Err(error) => error,
        };
        match error.downcast::<leitmotif::Error>() {
            Ok(error) => Self::from(*error),
            Err(error) => Self::analysis(error),
        }
    }
}
impl From<leitmotif::Error> for NativeError {
    fn from(error: leitmotif::Error) -> Self {
        Self::Analysis {
            detail: error.to_string(),
            source: Some(error.provenance),
        }
    }
}

pub(super) fn encode(error: &NativeError) -> VarDictionary {
    let mut output = VarDictionary::new();
    output.set("ok", false);
    output.set("error", error.to_string());
    let (kind, source) = match error {
        NativeError::RejectedInput(_) => ("rejected_input", None),
        NativeError::StaleInput { expected, actual } => {
            output.set("expected", &snapshot(expected));
            output.set("actual", &snapshot(actual));
            ("rejected_input", None)
        }
        NativeError::HostCall { operation, .. } => {
            output.set("operation", operation.as_str());
            ("host_call", None)
        }
        NativeError::Mapping {
            binding, source, ..
        } => {
            output.set("binding", binding.as_str());
            ("unsupported_mapping", source.as_ref())
        }
        NativeError::Analysis { source, .. } => ("analysis_failure", source.as_ref()),
        NativeError::Dialogue { source, .. } => ("dialogue_failure", source.as_ref()),
        NativeError::Conformance(difference) => {
            output.set("action", difference.action.as_str());
            output.set("expected", &snapshot(&difference.expected));
            output.set("actual", &snapshot(&difference.actual));
            output.set(
                "expected_enabled",
                &difference
                    .expected_enabled
                    .map_or_else(Variant::nil, |value| value.to_variant()),
            );
            output.set(
                "actual_enabled",
                &difference
                    .actual_enabled
                    .map_or_else(Variant::nil, |value| value.to_variant()),
            );
            output.set(
                "verified_prefix_actions",
                i64::from(difference.verified_prefix_actions),
            );
            output.set(
                "accepted_host_actions",
                i64::from(difference.accepted_host_actions),
            );
            ("conformance_failure", difference.source.as_ref())
        }
    };
    output.set("kind", kind);
    if let Some(source) = source {
        output.set("source", &provenance(source));
    }
    output
}

pub(super) fn snapshot(state: &Snapshot) -> VarDictionary {
    let mut output = VarDictionary::new();
    for (key, value) in &state.values {
        let value = match value {
            Value::Bool(value) => value.to_variant(),
            Value::Symbol(value) => value.to_variant(),
        };
        output.set(key.as_str(), &value);
    }
    output
}

pub(super) fn provenance(source: &Source) -> VarDictionary {
    let mut output = VarDictionary::new();
    output.set("file", source.file.as_str());
    output.set("line", i64::from(source.line));
    output.set("column", i64::from(source.column));
    if let Some(file) = &source.portable_file {
        output.set("portable_file", file.as_str());
    }
    output
}
