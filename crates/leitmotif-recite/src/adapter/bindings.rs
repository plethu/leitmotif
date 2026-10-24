use leitmotif::{Domain, Fact, FactHandle, Predicate, Source, Value};
use recite_core::compiled::CompiledConditionCall;
use recite_runtime::ConditionExpectedType;

use super::{Error, ErrorKind, error::caller};

pub(super) struct Binding {
    pub(super) call: CompiledConditionCall,
    pub(super) kind: ConditionExpectedType,
    pub(super) cases: Vec<(Value, Predicate)>,
    pub(super) provenance: Source,
}

/// Exact compiled calls bound to typed finite facts, independent of game engines.
#[derive(Default)]
pub struct Bindings {
    pub(super) entries: Vec<Binding>,
}

impl Bindings {
    /// Creates an empty binding registry.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Binds a Boolean-returning compiled call to a Boolean fact.
    ///
    /// # Errors
    /// Rejects duplicate exact calls.
    #[track_caller]
    pub fn bind_bool<F: Fact<Value = bool>>(
        &mut self,
        call: CompiledConditionCall,
        fact: &FactHandle<F>,
    ) -> Result<(), Error> {
        self.insert(Binding {
            call,
            kind: ConditionExpectedType::Bool,
            cases: vec![
                (Value::Bool(false), fact.is(false)),
                (Value::Bool(true), fact.is(true)),
            ],
            provenance: caller(),
        })
    }

    /// Binds an enum-returning compiled call to a symbol-encoded finite fact.
    ///
    /// # Errors
    /// Rejects duplicate calls and domains containing non-symbolic values.
    #[track_caller]
    pub fn bind_enum<F: Fact>(
        &mut self,
        call: CompiledConditionCall,
        fact: &FactHandle<F>,
    ) -> Result<(), Error> {
        let provenance = caller();
        let mut cases = Vec::new();
        for value in F::Value::values() {
            let encoded = value.clone().encode();
            if !matches!(encoded, Value::Symbol(_)) {
                return Err(Error {
                    kind: ErrorKind::NonSymbolicDomain,
                    provenance,
                });
            }
            cases.push((encoded, fact.is(value)));
        }
        self.insert(Binding {
            call,
            kind: ConditionExpectedType::Enum,
            cases,
            provenance,
        })
    }

    fn insert(&mut self, binding: Binding) -> Result<(), Error> {
        if self.entries.iter().any(|entry| entry.call == binding.call) {
            return Err(Error {
                kind: ErrorKind::Duplicate(binding.call.function),
                provenance: binding.provenance,
            });
        }
        self.entries.push(binding);
        Ok(())
    }

    pub(super) fn lookup(
        &self,
        call: &CompiledConditionCall,
        expected: ConditionExpectedType,
        provenance: &Source,
    ) -> Result<&Binding, Error> {
        let binding = self
            .entries
            .iter()
            .find(|entry| entry.call == *call)
            .ok_or_else(|| Error {
                kind: ErrorKind::Unmapped(call.function.clone()),
                provenance: provenance.clone(),
            })?;
        if binding.kind != expected {
            return Err(Error {
                kind: ErrorKind::WrongType(call.function.clone()),
                provenance: provenance.clone(),
            });
        }
        Ok(binding)
    }
}
