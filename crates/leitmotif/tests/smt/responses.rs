use super::Bit;
use leitmotif::{
    ModelBuilder, Predicate, Premise,
    smt::{Conclusion, Error, IncompleteReason, KnownAssignment, Options},
};
use std::{
    env, error::Error as StdError, fs, io::ErrorKind, os::unix::fs::symlink, path::PathBuf,
    process, time::Duration,
};

struct FixtureSolver {
    directory: PathBuf,
    executable: PathBuf,
}
impl FixtureSolver {
    fn new(mode: &str) -> Result<Self, Box<dyn StdError>> {
        for index in 0..1000 {
            let directory =
                env::temp_dir().join(format!("leitmotif-smt-{}-{index}", process::id()));
            match fs::create_dir(&directory) {
                Ok(()) => {
                    let executable = directory.join(mode);
                    let fixture = Self {
                        directory,
                        executable,
                    };
                    symlink(
                        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/smt_solver.py"),
                        &fixture.executable,
                    )?;
                    return Ok(fixture);
                }
                Err(error) if error.kind() == ErrorKind::AlreadyExists => {}
                Err(error) => return Err(error.into()),
            }
        }
        Err("could not allocate fixture directory".into())
    }
    fn options(&self) -> Result<Options, Error> {
        Options::new(&self.executable, Duration::from_millis(100))
    }
}
impl Drop for FixtureSolver {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.executable);
        let _ = fs::remove_dir(&self.directory);
    }
}

#[test]
fn timeout_unknown_and_partial_assignment_polarities_remain_distinct() {
    let mut builder = ModelBuilder::new("responses").unwrap();
    let bit = builder.fact::<Bit<0>>().unwrap();
    let model = builder.build().unwrap();
    for mode in [
        "timeout",
        "timeout-token",
        "unknown",
        "second-support",
        "second-oppose",
    ] {
        let fixture = FixtureSolver::new(mode).unwrap();
        let report = model
            .classify_smt(&bit.is(true), &[], &fixture.options().unwrap())
            .unwrap();
        let Conclusion::Incomplete { reason, known } = report.conclusion else {
            panic!("incomplete fixture must remain incomplete")
        };
        if mode.starts_with("timeout") {
            assert_eq!(reason, IncompleteReason::Timeout);
        } else {
            assert!(
                matches!(reason, IncompleteReason::Unknown { response } if response.contains("incomplete fixture"))
            );
        }
        match (mode, known) {
            ("second-support", Some(KnownAssignment::Supporting(snapshot))) => assert_eq!(
                snapshot.values.get("a"),
                Some(&leitmotif::Value::Bool(true))
            ),
            ("second-oppose", Some(KnownAssignment::Opposing(snapshot))) => assert_eq!(
                snapshot.values.get("a"),
                Some(&leitmotif::Value::Bool(false))
            ),
            ("timeout" | "timeout-token" | "unknown", None) => assert_eq!(report.checks.len(), 1),
            combination => panic!("wrong known-evidence polarity: {combination:?}"),
        }
    }
}

#[test]
fn invalid_ordinals_cardinality_identifiers_and_stale_second_models_are_rejected() {
    let mut builder = ModelBuilder::new("responses").unwrap();
    let bit = builder.fact::<Bit<0>>().unwrap();
    let model = builder.build().unwrap();
    for mode in [
        "prefix",
        "negative",
        "overflow",
        "fraction",
        "out-of-domain",
        "missing",
        "foreign",
        "stale",
    ] {
        let fixture = FixtureSolver::new(mode).unwrap();
        assert!(
            matches!(
                model.classify_smt(&bit.is(true), &[], &fixture.options().unwrap()),
                Err(Error::InvalidAssignment(_))
            ),
            "{mode}"
        );
    }
    let fixture = FixtureSolver::new("bad-premise").unwrap();
    assert!(matches!(
        model.classify_smt(
            &bit.is(true),
            &[Premise::new("requires_true", bit.is(true))],
            &fixture.options().unwrap()
        ),
        Err(Error::InvalidAssignment(_))
    ));
}

#[test]
fn shuffled_complete_assignments_are_valid_but_duplicate_variables_are_not() {
    let mut builder = ModelBuilder::new("responses").unwrap();
    builder.fact::<Bit<0>>().unwrap();
    builder.fact::<Bit<1>>().unwrap();
    let model = builder.build().unwrap();
    let fixture = FixtureSolver::new("shuffled").unwrap();
    let report = model
        .classify_smt(&Predicate::constant(true), &[], &fixture.options().unwrap())
        .unwrap();
    assert!(
        matches!(report.conclusion, Conclusion::Required { witness, .. } if witness.values.len() == 2)
    );
    let fixture = FixtureSolver::new("duplicate").unwrap();
    assert!(matches!(
        model.classify_smt(&Predicate::constant(true), &[], &fixture.options().unwrap()),
        Err(Error::InvalidAssignment(_))
    ));
}

#[test]
fn unsupported_protocol_and_invalid_cores_are_typed_failures() {
    let model = ModelBuilder::new("responses").unwrap().build().unwrap();
    for mode in [
        "unsupported",
        "unsupported-config",
        "malformed",
        "bad-reason",
        "bad-core",
        "duplicate-core",
    ] {
        let fixture = FixtureSolver::new(mode).unwrap();
        let result = model.classify_smt(
            &Predicate::constant(true),
            &[Premise::new("premise", Predicate::constant(false))],
            &fixture.options().unwrap(),
        );
        match (mode, result) {
            ("unsupported" | "unsupported-config", Err(Error::Unsupported { .. }))
            | ("malformed" | "bad-reason", Err(Error::Protocol { .. }))
            | ("bad-core" | "duplicate-core", Err(Error::InvalidCore(_))) => {}
            other => panic!("wrong failure category: {other:?}"),
        }
    }
}

#[test]
fn both_info_commands_require_complete_expected_responses_and_preserve_error_kinds() {
    use rsmt2::errors::{Error as SolverError, ErrorKind as SolverErrorKind};
    let model = ModelBuilder::new("info_responses")
        .unwrap()
        .build()
        .unwrap();
    for command in ["version", "reason"] {
        for case in [
            "unsupported",
            "error",
            "atom",
            "extra",
            "trailing",
            "unclosed",
            "malformed-escape",
            "unsupported-extra",
            "wrong-flag",
            "unquoted",
            "unicode-atom",
            "unicode-unquoted",
            "unicode-header",
            "unicode-prefix",
            "unicode-trailer",
            "unicode-extra",
            "unicode-error-header",
            "unicode-unsupported",
            "unicode-error",
        ] {
            let fixture = FixtureSolver::new(&format!("{command}-{case}")).unwrap();
            let result =
                model.classify_smt(&Predicate::constant(true), &[], &fixture.options().unwrap());
            match (case, result) {
                ("unsupported", Err(Error::Unsupported { source, .. })) => assert!(matches!(
                    source.downcast_ref::<SolverError>().unwrap().kind(),
                    SolverErrorKind::Unsupported
                )),
                ("error", Err(Error::Protocol { source, .. })) => assert!(
                    matches!(source.downcast_ref::<SolverError>().unwrap().kind(), SolverErrorKind::SolverError(message) if message == "info unavailable")
                ),
                ("unicode-error", Err(Error::Protocol { source, .. })) => assert!(
                    matches!(source.downcast_ref::<SolverError>().unwrap().kind(), SolverErrorKind::SolverError(message) if message == "δ unavailable")
                ),
                (_, Err(Error::Protocol { .. })) => {}
                other => {
                    panic!("invalid {command} info response accepted or misclassified: {other:?}")
                }
            }
        }
    }
}

#[test]
fn quoted_info_fields_support_smt_doubled_quotes_parentheses_and_unicode() {
    let model = ModelBuilder::new("escaped_info").unwrap().build().unwrap();
    let fixture = FixtureSolver::new("version-escaped").unwrap();
    let report = model
        .classify_smt(&Predicate::constant(true), &[], &fixture.options().unwrap())
        .unwrap();
    assert_eq!(
        report.solver.version_response,
        "(:version \"fixture \"\"quoted\"\" ) λ\")"
    );
    assert!(matches!(report.conclusion, Conclusion::Required { .. }));
    let fixture = FixtureSolver::new("reason-escaped").unwrap();
    let report = model
        .classify_smt(&Predicate::constant(true), &[], &fixture.options().unwrap())
        .unwrap();
    assert!(
        matches!(report.conclusion, Conclusion::Incomplete { reason: IncompleteReason::Unknown { response }, known: None } if response == "(:reason-unknown \"fixture \"\"quoted\"\" ) λ\")")
    );
}
