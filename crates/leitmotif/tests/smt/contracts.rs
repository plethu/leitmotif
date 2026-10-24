use super::{Bit, options};
use leitmotif::{
    ClassificationLimits, Domain, ErrorKind, Fact, FactKind, Invariant, ModelBuilder, Predicate,
    Premise, Transition, Value,
    smt::{Conclusion, CoreDependency, Error, Options},
};
use std::{path::Path, time::Duration};

#[derive(Clone, Copy)]
enum Choice {
    Boolean,
    Symbol,
    Invalid,
}
impl Domain for Choice {
    fn values() -> Vec<Self> {
        vec![Self::Boolean, Self::Symbol]
    }
    fn encode(self) -> Value {
        match self {
            Self::Boolean => Value::Bool(false),
            Self::Symbol => Value::Symbol("|λ| ) (assert false) \"quoted\"\n".into()),
            Self::Invalid => Value::Symbol("invalid".into()),
        }
    }
}
struct Setting;
impl Fact for Setting {
    type Value = Choice;
    const KEY: &'static str = "setting.) (assert false)";
    const KIND: FactKind = FactKind::Setting;
}

#[test]
fn finite_settings_mixed_encodings_and_domain_cores_match_ordinary_classification() {
    let mut builder = ModelBuilder::new("settings").unwrap();
    let setting = builder.fact::<Setting>().unwrap();
    let model = builder.build().unwrap();
    let excluded = setting
        .is(Choice::Boolean)
        .negate()
        .and(setting.is(Choice::Symbol).negate());
    let premises = [Premise::new("both_values_excluded", excluded)];
    let report = model
        .classify_smt(&setting.is(Choice::Symbol), &premises, &options().unwrap())
        .unwrap();
    let Conclusion::Inconsistent { core } = report.conclusion else {
        panic!("finite domain must exclude a third value")
    };
    assert!(core.dependencies.iter().any(|dependency| matches!(dependency, CoreDependency::Domain { fact } if fact.key == Setting::KEY)));
    assert!(core.dependencies.iter().any(|dependency| matches!(dependency, CoreDependency::Premise { premise } if premise.id == "both_values_excluded" && premise.provenance.portable_file.is_some())));
    assert!(
        !core
            .dependencies
            .iter()
            .any(|dependency| matches!(dependency, CoreDependency::Hypothesis { .. }))
    );
    for premise in [setting.is(Choice::Boolean), setting.is(Choice::Symbol)] {
        let premises = [Premise::new("selected", premise)];
        let hypothesis = setting.is(Choice::Symbol);
        let reference = model
            .classify(
                &hypothesis,
                &premises,
                ClassificationLimits { max_candidates: 2 },
            )
            .unwrap();
        let report = model
            .classify_smt(&hypothesis, &premises, &options().unwrap())
            .unwrap();
        assert_eq!(
            super::verdict(&report.conclusion),
            Some(reference.conclusion)
        );
        assert!(report.solver.version_response.contains("5.3.1"));
    }
}

#[test]
fn transitions_and_invariants_do_not_become_implicit_premises() {
    let mut builder = ModelBuilder::new("scope").unwrap();
    let bit = builder.fact::<Bit<0>>().unwrap();
    let model = builder
        .with_transition(Transition::new("write").with_effect(bit.set(true)))
        .with_invariant(Invariant::new("false", bit.is(false)))
        .build()
        .unwrap();
    let report = model
        .classify_smt(&bit.is(true), &[], &options().unwrap())
        .unwrap();
    assert!(matches!(report.conclusion, Conclusion::Possible { .. }));
    assert!(report.premises.is_empty());
}

#[test]
fn empty_models_constants_and_auxiliary_hypothesis_cores_are_explicit() {
    let model = ModelBuilder::new("empty").unwrap().build().unwrap();
    for truth in [false, true] {
        let report = model
            .classify_smt(&Predicate::constant(truth), &[], &options().unwrap())
            .unwrap();
        let (snapshot, core) = match report.conclusion {
            Conclusion::Required { witness, core } => {
                assert!(truth);
                (witness, core)
            }
            Conclusion::RuledOut {
                counterexample,
                core,
            } => {
                assert!(!truth);
                (counterexample, core)
            }
            other => panic!("constant result unexpected: {other:?}"),
        };
        assert!(snapshot.values.is_empty());
        assert_eq!(
            core.dependencies,
            [CoreDependency::Hypothesis { supports: !truth }]
        );
        assert_eq!(report.checks.len(), 2);
        let serialized = serde_json::to_value(&report.checks).unwrap();
        assert!(serialized.is_array());
    }
    let report = model
        .classify_smt(
            &Predicate::constant(true),
            &[Premise::new("false", Predicate::constant(false))],
            &options().unwrap(),
        )
        .unwrap();
    assert!(matches!(report.conclusion, Conclusion::Inconsistent { .. }));
}

#[test]
fn full_validation_precedes_deployment_even_in_dead_boolean_branches() {
    let mut builder = ModelBuilder::new("validation").unwrap();
    let setting = builder.fact::<Setting>().unwrap();
    let model = builder.build().unwrap();
    let mut foreign = ModelBuilder::new("foreign").unwrap();
    let foreign = foreign.fact::<Setting>().unwrap();
    let unavailable =
        Options::new("/definitely/missing/leitmotif-z3", Duration::from_millis(1)).unwrap();
    let invalid = Predicate::constant(true).or(setting.is(Choice::Invalid));
    assert!(
        matches!(model.classify_smt(&invalid, &[], &unavailable), Err(Error::Query(error)) if matches!(error.kind, ErrorKind::InvalidValue { .. }))
    );
    let invalid = Predicate::constant(false).and(foreign.is(Choice::Boolean));
    assert!(
        matches!(model.classify_smt(&invalid, &[], &unavailable), Err(Error::Query(error)) if matches!(error.kind, ErrorKind::ForeignHandle(_)))
    );
    for premises in [
        vec![Premise::new(" ", Predicate::constant(true))],
        vec![
            Premise::new("same", Predicate::constant(true)),
            Premise::new("same", Predicate::constant(true)),
        ],
        vec![Premise::new("invalid", invalid)],
    ] {
        assert!(matches!(
            model.classify_smt(&Predicate::constant(true), &premises, &unavailable),
            Err(Error::Query(_))
        ));
    }
    assert!(
        matches!(model.classify_smt(&Predicate::constant(true), &[], &unavailable), Err(Error::Deployment { executable, .. }) if executable == Path::new("/definitely/missing/leitmotif-z3"))
    );
}

#[test]
fn options_require_representable_positive_whole_milliseconds() {
    for duration in [
        Duration::ZERO,
        Duration::from_nanos(1),
        Duration::from_micros(1500),
        Duration::from_millis(u64::from(u32::MAX)),
        Duration::from_millis(u64::from(u32::MAX) + 1),
    ] {
        assert!(matches!(
            Options::new("z3", duration),
            Err(Error::InvalidOptions(_))
        ));
    }
    assert!(matches!(
        Options::new("", Duration::from_millis(1)),
        Err(Error::InvalidOptions(_))
    ));
    let options = Options::new("z3", Duration::from_millis(u64::from(u32::MAX - 1))).unwrap();
    assert_eq!(options.soft_timeout_ms(), u32::MAX - 1);
    assert_eq!(options.executable(), Path::new("z3"));
}

#[cfg(unix)]
#[test]
fn non_utf8_executable_is_rejected_by_the_options_owner() {
    use std::{ffi::OsString, os::unix::ffi::OsStringExt};
    assert!(matches!(
        Options::new(OsString::from_vec(vec![0xff]), Duration::from_millis(1)),
        Err(Error::InvalidOptions(_))
    ));
}
