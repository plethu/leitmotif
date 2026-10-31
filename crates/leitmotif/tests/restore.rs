//! Contract tests for restoring saved states from snapshots.

use leitmotif::{
    Domain, Error, ErrorKind, Fact, FactHandle, FactKind, Model, ModelBuilder, Snapshot,
    Transition, Value,
};

struct Learned;
impl Fact for Learned {
    type Value = bool;
    const KEY: &'static str = "learned";
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Weather {
    Calm,
    Storm,
}

impl Domain for Weather {
    fn values() -> Vec<Self> {
        vec![Self::Calm, Self::Storm]
    }

    fn encode(self) -> Value {
        symbol(match self {
            Self::Calm => "calm",
            Self::Storm => "storm",
        })
    }
}

struct Sky;
impl Fact for Sky {
    type Value = Weather;
    const KEY: &'static str = "sky";
}

struct Hardcore;
impl Fact for Hardcore {
    type Value = bool;
    const KEY: &'static str = "hardcore";
    const KIND: FactKind = FactKind::Setting;
}

struct Harbour {
    model: Model,
    learned: FactHandle<Learned>,
    sky: FactHandle<Sky>,
    hardcore: FactHandle<Hardcore>,
}

fn harbour() -> Result<Harbour, Error> {
    let mut builder = ModelBuilder::new("harbour")?;
    let learned = builder.fact::<Learned>()?;
    let sky = builder.fact::<Sky>()?;
    let hardcore = builder.fact::<Hardcore>()?;
    let model = builder
        .with_transition(
            Transition::new("storm.break")
                .with_guard(sky.is(Weather::Storm))
                .with_effect(sky.set(Weather::Calm))
                .with_effect(learned.set(true)),
        )
        .build()?;
    Ok(Harbour {
        model,
        learned,
        sky,
        hardcore,
    })
}

impl Harbour {
    fn stormy_snapshot(&self) -> Result<Snapshot, Error> {
        let state = self.model.state([
            self.learned.set(false),
            self.sky.set(Weather::Storm),
            self.hardcore.set(true),
        ])?;
        self.model.snapshot(&state)
    }
}

fn symbol(name: &str) -> Value {
    Value::Symbol(name.to_owned())
}

fn mismatch(missing: &[&str], unknown: &[&str], invalid: &[(&str, Value)]) -> ErrorKind {
    let keys = |keys: &[&str]| keys.iter().map(|&key| key.to_owned()).collect();
    ErrorKind::SnapshotMismatch {
        missing: keys(missing),
        unknown: keys(unknown),
        invalid: invalid
            .iter()
            .map(|(key, value)| ((*key).to_owned(), value.clone()))
            .collect(),
    }
}

#[test]
fn restored_state_round_trips_and_replays() {
    let harbour = harbour().unwrap();
    let saved = harbour.stormy_snapshot().unwrap();
    let restored = harbour.model.restore(&saved).unwrap();
    assert_eq!(harbour.model.snapshot(&restored).unwrap(), saved);
    assert!(
        harbour
            .model
            .evaluate(&restored, &harbour.sky.is(Weather::Storm))
            .unwrap()
    );
    let calmed = harbour
        .model
        .apply(&restored, "storm.break")
        .unwrap()
        .unwrap();
    assert!(
        harbour
            .model
            .evaluate(
                &calmed,
                &harbour.learned.is(true).and(harbour.hardcore.is(true))
            )
            .unwrap()
    );
}

#[test]
fn snapshot_restores_into_a_separately_built_identical_model() {
    let original = harbour().unwrap();
    let saved = original.stormy_snapshot().unwrap();
    let reloaded = harbour().unwrap();
    let foreign = original.model.restore(&saved).unwrap();
    assert_eq!(
        reloaded.model.snapshot(&foreign).unwrap_err().kind,
        ErrorKind::ForeignState
    );
    let restored = reloaded.model.restore(&saved).unwrap();
    assert_eq!(reloaded.model.snapshot(&restored).unwrap(), saved);
    assert!(
        reloaded
            .model
            .evaluate(&restored, &reloaded.sky.is(Weather::Storm))
            .unwrap()
    );
}

#[test]
fn snapshot_json_form_is_pinned_and_round_trips() {
    let saved = harbour().unwrap().stormy_snapshot().unwrap();
    let json = serde_json::to_string(&saved).unwrap();
    assert_eq!(
        json,
        r#"{"values":{"hardcore":{"type":"bool","value":true},"learned":{"type":"bool","value":false},"sky":{"type":"symbol","value":"storm"}}}"#
    );
    assert_eq!(serde_json::from_str::<Snapshot>(&json).unwrap(), saved);
}

#[test]
fn missing_facts_including_settings_are_rejected_in_key_order() {
    let harbour = harbour().unwrap();
    let mut saved = harbour.stormy_snapshot().unwrap();
    saved.values.remove("learned");
    saved.values.remove("hardcore");
    let error = harbour.model.restore(&saved).unwrap_err();
    assert_eq!(error.kind, mismatch(&["hardcore", "learned"], &[], &[]));
}

#[test]
fn unknown_key_is_rejected() {
    let harbour = harbour().unwrap();
    let mut saved = harbour.stormy_snapshot().unwrap();
    saved.values.insert("tide".to_owned(), Value::Bool(true));
    let error = harbour.model.restore(&saved).unwrap_err();
    assert_eq!(error.kind, mismatch(&[], &["tide"], &[]));
}

#[test]
fn symbol_outside_its_domain_is_rejected() {
    let harbour = harbour().unwrap();
    let mut saved = harbour.stormy_snapshot().unwrap();
    saved.values.insert("sky".to_owned(), symbol("fog"));
    let error = harbour.model.restore(&saved).unwrap_err();
    assert_eq!(error.kind, mismatch(&[], &[], &[("sky", symbol("fog"))]));
}

#[test]
fn bool_for_a_symbol_fact_is_rejected() {
    let harbour = harbour().unwrap();
    let mut saved = harbour.stormy_snapshot().unwrap();
    saved.values.insert("sky".to_owned(), Value::Bool(true));
    let error = harbour.model.restore(&saved).unwrap_err();
    assert_eq!(
        error.kind,
        mismatch(&[], &[], &[("sky", Value::Bool(true))])
    );
}

#[test]
fn every_mismatch_is_reported_together_in_key_order() {
    let harbour = harbour().unwrap();
    let mut saved = harbour.stormy_snapshot().unwrap();
    saved.values.remove("learned");
    saved.values.insert("tide".to_owned(), Value::Bool(true));
    saved.values.insert("anchor".to_owned(), Value::Bool(false));
    saved.values.insert("sky".to_owned(), symbol("fog"));
    saved.values.insert("hardcore".to_owned(), symbol("yes"));
    let restore_line = line!() + 1;
    let error = harbour.model.restore(&saved).unwrap_err();
    assert_eq!(error.provenance.line, restore_line);
    assert_eq!(
        error.kind,
        mismatch(
            &["learned"],
            &["anchor", "tide"],
            &[("hardcore", symbol("yes")), ("sky", symbol("fog"))]
        )
    );
}
