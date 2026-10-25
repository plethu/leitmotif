use std::path::Path;

use base16ct::lower;
use serde::Serialize;
use serde_json::Value as JsonValue;
use sha2::{Digest, Sha256};

use super::{
    Error, ErrorKind, FactInfo, Model, NamedPredicate, PredicateDescription, PredicateExpression,
    Source, TransitionInfo,
};

#[derive(Serialize)]
struct ModelDefinition<'a> {
    context: &'a str,
    facts: &'a [FactInfo],
    transitions: Vec<&'a TransitionInfo>,
    invariants: Vec<&'a NamedPredicate>,
}

impl Model {
    pub(super) fn semantic_id(&self) -> Result<String, Error> {
        Ok(format!(
            "sha256:{}",
            lower::encode_string(&Sha256::digest(self.canonical_definition()?))
        ))
    }

    fn canonical_definition(&self) -> Result<Vec<u8>, Error> {
        let definition = ModelDefinition {
            context: &self.context,
            facts: &self.facts,
            transitions: self.transitions().collect(),
            invariants: self.invariants.iter().map(|item| &item.info).collect(),
        };
        let mut value = serde_json::to_value(definition).map_err(encoding_error)?;
        omit_provenance(&mut value);
        serde_json::to_vec(&value).map_err(encoding_error)
    }

    pub(super) fn portable_sources(&mut self) {
        let Some(root) = &self.source_root else {
            return;
        };
        for fact in &mut self.facts {
            fact.provenance.relativize(root);
        }
        for transition in &mut self.transitions {
            transition.info.provenance.relativize(root);
            relativize_predicate(&mut transition.info.guard, root);
            for effect in &mut transition.info.effects {
                effect.provenance.relativize(root);
            }
        }
        for invariant in &mut self.invariants {
            relativize_named(&mut invariant.info, root);
        }
    }

    pub(super) fn portable_predicate(
        &self,
        mut predicate: PredicateDescription,
    ) -> PredicateDescription {
        if let Some(root) = &self.source_root {
            relativize_predicate(&mut predicate, root);
        }
        predicate
    }

    pub(super) fn portable_named(&self, predicate: &mut NamedPredicate) {
        if let Some(root) = &self.source_root {
            relativize_named(predicate, root);
        }
    }
}

fn relativize_named(predicate: &mut NamedPredicate, root: &Path) {
    predicate.provenance.relativize(root);
    relativize_predicate(&mut predicate.predicate, root);
}

fn relativize_predicate(predicate: &mut PredicateDescription, root: &Path) {
    predicate.provenance.relativize(root);
    match &mut predicate.expression {
        PredicateExpression::Constant(_) => {}
        PredicateExpression::Eq(assignment) => assignment.provenance.relativize(root),
        PredicateExpression::And(left, right) | PredicateExpression::Or(left, right) => {
            relativize_predicate(left, root);
            relativize_predicate(right, root);
        }
        PredicateExpression::Not(inner) => relativize_predicate(inner, root),
    }
}

fn omit_provenance(value: &mut JsonValue) {
    match value {
        JsonValue::Object(fields) => {
            fields.remove("provenance");
            // Feature unification may enable serde_json/preserve_order.
            fields.sort_keys();
            for field in fields.values_mut() {
                omit_provenance(field);
            }
        }
        JsonValue::Array(items) => {
            for item in items {
                omit_provenance(item);
            }
        }
        _ => {}
    }
}

fn encoding_error(_: serde_json::Error) -> Error {
    Error::new(
        ErrorKind::InvalidModel("model definition could not be encoded"),
        Source::caller(),
    )
}

#[cfg(test)]
#[path = "reproducibility/tests/hash_cost.rs"]
mod tests;
