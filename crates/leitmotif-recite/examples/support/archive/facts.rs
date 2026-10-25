use leitmotif::{Domain, Fact, FactKind, Value};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Access {
    Closed,
    Opened,
    Denied,
}
impl Domain for Access {
    fn values() -> Vec<Self> {
        vec![Self::Closed, Self::Opened, Self::Denied]
    }
    fn encode(self) -> Value {
        Value::Symbol(self.symbol().to_owned())
    }
}
impl Access {
    pub(super) const fn symbol(self) -> &'static str {
        match self {
            Self::Closed => "closed",
            Self::Opened => "opened",
            Self::Denied => "denied",
        }
    }
}

pub(super) struct Key;
impl Fact for Key {
    type Value = bool;
    const KEY: &'static str = "archive.key";
}
pub(super) struct Door;
impl Fact for Door {
    type Value = Access;
    const KEY: &'static str = "archive.access";
}
pub(super) struct Exposed;
impl Fact for Exposed {
    type Value = bool;
    const KEY: &'static str = "claim.exposed";
}
pub(super) struct Truth;
impl Fact for Truth {
    type Value = bool;
    const KEY: &'static str = "claim.truth";
    const KIND: FactKind = FactKind::Setting;
}
pub(super) struct Belief;
impl Fact for Belief {
    type Value = bool;
    const KEY: &'static str = "claim.belief";
    const KIND: FactKind = FactKind::Setting;
}
pub(super) struct Visit;
impl Fact for Visit {
    type Value = bool;
    const KEY: &'static str = "archive.visit_recorded";
}
#[derive(Clone)]
pub(super) enum VisitResult {
    Unrecorded,
    Recorded(Access),
}
impl Domain for VisitResult {
    fn values() -> Vec<Self> {
        let mut values = vec![Self::Unrecorded];
        values.extend(Access::values().into_iter().map(Self::Recorded));
        values
    }
    fn encode(self) -> Value {
        match self {
            Self::Unrecorded => Value::Symbol("unrecorded".into()),
            Self::Recorded(access) => access.encode(),
        }
    }
}
pub(super) struct ResultRecorded;
impl Fact for ResultRecorded {
    type Value = VisitResult;
    const KEY: &'static str = "archive.result_recorded";
}
pub(super) struct KeyRemoval;
impl Fact for KeyRemoval {
    type Value = bool;
    const KEY: &'static str = "protocol.key_removal_available";
}
pub(super) struct RestoreBefore;
impl Fact for RestoreBefore {
    type Value = bool;
    const KEY: &'static str = "protocol.restore_before_available";
}
pub(super) struct RestoreAfter;
impl Fact for RestoreAfter {
    type Value = bool;
    const KEY: &'static str = "protocol.restore_after_available";
}
