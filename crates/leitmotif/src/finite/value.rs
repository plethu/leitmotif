use serde::Serialize;

/// A concrete finite-domain encoding. Symbols must be nonempty.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum Value {
    /// A boolean value.
    Bool(bool),
    /// An application-defined symbolic value.
    Symbol(String),
}

/// An explicitly finite domain with unique, nonempty encodings.
///
/// `values` must enumerate the whole domain, deterministically. `encode` must
/// agree with that enumeration. Registration validates the declared encodings;
/// assignments and queries validate their encodings against that declaration.
pub trait Domain: Clone {
    /// Enumerates every possible value, in deterministic order.
    fn values() -> Vec<Self>;
    /// Encodes a value without depending on mutable external state.
    fn encode(self) -> Value;
}

impl Domain for bool {
    fn values() -> Vec<Self> {
        vec![false, true]
    }

    fn encode(self) -> Value {
        Value::Bool(self)
    }
}

/// Whether transitions may change a fact.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FactKind {
    /// A fact transitions may write.
    #[default]
    Mutable,
    /// A fact fixed for the duration of a trace.
    Setting,
}

/// A typed fact declaration. Keys are stable and unique within a model.
pub trait Fact: 'static {
    /// The explicitly finite value domain.
    type Value: Domain;
    /// A nonempty, stable application identifier.
    const KEY: &'static str;
    /// Transition write policy; initial assignments are always required.
    const KIND: FactKind = FactKind::Mutable;
}
