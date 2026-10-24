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
