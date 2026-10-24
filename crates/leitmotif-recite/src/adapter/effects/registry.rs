use leitmotif::{Model, Source, State, TransitionInfo};
use recite_core::{
    ScalarValue,
    compiled::{CompiledArgument, CompiledEffect, CompiledEffectMode},
};
use recite_runtime::{DialogueEffectArgument, DialogueEffectMode, DialogueEffectRequest};

use super::super::error::source;
use super::{AppliedEffect, EffectError, EffectErrorKind};

struct Binding<'model> {
    function: String,
    args: Vec<DialogueEffectArgument>,
    transition: &'model TransitionInfo,
}

pub(super) struct Registry<'model> {
    model: &'model Model,
    bindings: Vec<Binding<'model>>,
}

impl<'model> Registry<'model> {
    pub(super) const fn new(model: &'model Model) -> Self {
        Self {
            model,
            bindings: Vec::new(),
        }
    }

    pub(super) fn bind(
        &mut self,
        effect: &CompiledEffect,
        transition: &str,
        provenance: Source,
    ) -> Result<(), EffectError> {
        let args = effect.args.iter().map(runtime_argument).collect::<Vec<_>>();
        if self
            .bindings
            .iter()
            .any(|binding| binding.function == effect.function && binding.args == args)
        {
            return Err(EffectError {
                kind: EffectErrorKind::Duplicate(effect.function.clone()),
                provenance,
            });
        }
        let declared = self
            .model
            .transitions()
            .find(|candidate| candidate.id == transition)
            .ok_or_else(|| EffectError {
                kind: EffectErrorKind::UnknownTransition(transition.to_owned()),
                provenance,
            })?;
        self.bindings.push(Binding {
            function: effect.function.clone(),
            args,
            transition: declared,
        });
        Ok(())
    }

    pub(super) fn apply<'request>(
        &'request self,
        request: &'request DialogueEffectRequest,
        state: &State,
    ) -> Result<AppliedEffect<'request>, EffectError> {
        let at_request = |kind| EffectError {
            kind,
            provenance: source(&request.source_span),
        };
        let binding = matching_binding(self.bindings.iter(), request)
            .ok_or_else(|| at_request(EffectErrorKind::Unmapped(request.function.clone())))?;
        let successor = self
            .model
            .apply(state, &binding.transition.id)
            .map_err(|error| at_request(EffectErrorKind::Host(Box::new(error))))?
            .ok_or_else(|| {
                at_request(EffectErrorKind::DisabledTransition(
                    binding.transition.id.clone(),
                ))
            })?;
        Ok(AppliedEffect {
            request,
            transition: binding.transition,
            state: successor,
        })
    }
}

pub(super) const fn runtime_mode(mode: CompiledEffectMode) -> DialogueEffectMode {
    match mode {
        CompiledEffectMode::Immediate => DialogueEffectMode::Immediate,
        CompiledEffectMode::Blocking => DialogueEffectMode::Blocking,
        CompiledEffectMode::Deferred => DialogueEffectMode::Deferred,
    }
}

fn runtime_argument(argument: &CompiledArgument) -> DialogueEffectArgument {
    match argument {
        CompiledArgument::Identifier(value) => DialogueEffectArgument::Identifier(value.clone()),
        CompiledArgument::Value(ScalarValue::String(value)) => {
            DialogueEffectArgument::String(value.clone())
        }
        CompiledArgument::Value(ScalarValue::Integer(value)) => {
            DialogueEffectArgument::Integer(*value)
        }
        CompiledArgument::Value(ScalarValue::Float(value)) => DialogueEffectArgument::Float(*value),
        CompiledArgument::Value(ScalarValue::Boolean(value)) => {
            DialogueEffectArgument::Boolean(*value)
        }
    }
}

fn matching_binding<'binding>(
    mut bindings: impl Iterator<Item = &'binding Binding<'binding>>,
    request: &DialogueEffectRequest,
) -> Option<&'binding Binding<'binding>> {
    bindings.find(|binding| binding.function == request.function && binding.args == request.args)
}

#[cfg(test)]
mod tests;
