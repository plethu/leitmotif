use std::path::Path;

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
            "sha256:{:x}",
            Sha256::digest(self.canonical_definition()?)
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
mod tests {
    use crate::{Fact, ModelBuilder, Transition};

    struct Flag;
    impl Fact for Flag {
        type Value = bool;
        const KEY: &'static str = "flag";
    }

    #[test]
    fn fixed_model_bounds_canonical_hash_input() {
        let mut builder = ModelBuilder::new("hash-cost").unwrap();
        let flag = builder.fact::<Flag>().unwrap();
        for index in 0..128 {
            builder = builder.with_transition(
                Transition::new(format!("event-{index:03}")).with_effect(flag.set(true)),
            );
        }
        let model = builder.build().unwrap();
        let encoded = model.canonical_definition().unwrap();
        // Measured at 19,239 canonical bytes for this fixed 128-transition model.
        // This bounds serialized bytes fed to linear SHA-256, not elapsed time.
        assert!(encoded.len() <= 19_239);
        assert_eq!(
            model.fingerprint,
            "sha256:f78c60f50987a1ecfa0d2ff12662edf5ab05b573fe180e0769236c098d4b419f"
        );
    }
}
