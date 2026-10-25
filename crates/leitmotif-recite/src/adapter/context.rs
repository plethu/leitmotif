use leitmotif::{Model, State, Value};
use recite_core::{ScalarValue, compiled::CompiledArgument};
use recite_runtime::{
    ConditionArgument, ConditionEvaluationError, ConditionEvaluationErrorKind, ConditionQuery,
    ConditionValue, DialogueContext,
};

use crate::Bindings;

/// Borrowed runtime context over exactly the finite state supplied by the host.
/// Queries do not mutate the model or state.
pub struct RuntimeContext<'a> {
    bindings: &'a Bindings,
    model: &'a Model,
    state: &'a State,
}

impl Bindings {
    /// Borrows a host state for use by the real Recite runtime.
    #[must_use]
    pub const fn context<'a>(&'a self, model: &'a Model, state: &'a State) -> RuntimeContext<'a> {
        RuntimeContext {
            bindings: self,
            model,
            state,
        }
    }
}

impl DialogueContext for RuntimeContext<'_> {
    fn evaluate_condition(
        &self,
        query: ConditionQuery<'_>,
    ) -> Result<ConditionValue, ConditionEvaluationError> {
        let binding = self
            .bindings
            .entries
            .iter()
            .find(|entry| {
                entry.call.function == query.function()
                    && entry.call.args.len() == query.arguments().len()
                    && entry
                        .call
                        .args
                        .iter()
                        .zip(query.arguments())
                        .all(|(compiled, argument)| same_argument(compiled, argument))
            })
            .ok_or_else(|| {
                ConditionEvaluationError::with_kind(
                    ConditionEvaluationErrorKind::MissingHandler,
                    format!("unmapped condition {}", query.function()),
                )
            })?;
        if binding.kind != query.expected_type() {
            return Err(ConditionEvaluationError::with_kind(
                ConditionEvaluationErrorKind::InvalidResult,
                format!(
                    "wrong result type for {} at {}",
                    query.function(),
                    binding.provenance
                ),
            ));
        }
        for (value, predicate) in &binding.cases {
            if self
                .model
                .evaluate(self.state, predicate)
                .map_err(|error| ConditionEvaluationError::new(error.to_string()))?
            {
                return match value {
                    Value::Bool(value) => Ok(ConditionValue::Bool(*value)),
                    Value::Symbol(value) => Ok(ConditionValue::EnumVariant(value.clone())),
                };
            }
        }
        Err(ConditionEvaluationError::new(
            "state value outside bound finite domain",
        ))
    }
}

fn same_argument(compiled: &CompiledArgument, argument: ConditionArgument<'_>) -> bool {
    match (compiled, argument) {
        (CompiledArgument::Identifier(left), ConditionArgument::Identifier(right)) => left == right,
        (CompiledArgument::Value(ScalarValue::String(left)), ConditionArgument::String(right)) => {
            left == right
        }
        (
            CompiledArgument::Value(ScalarValue::Integer(left)),
            ConditionArgument::Integer(right),
        ) => *left == right,
        (CompiledArgument::Value(ScalarValue::Float(left)), ConditionArgument::Float(right)) => {
            ScalarValue::Float(*left) == ScalarValue::Float(right)
        }
        (
            CompiledArgument::Value(ScalarValue::Boolean(left)),
            ConditionArgument::Boolean(right),
        ) => *left == right,
        _ => false,
    }
}
