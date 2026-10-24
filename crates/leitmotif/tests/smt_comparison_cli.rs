//! Process contracts for optional SMT comparison measurements.
#![cfg(feature = "smt")]
use serde_json::Value;
use std::{
    env,
    error::Error,
    path::Path,
    process::{Command, Output},
};

fn run(args: &[&str], status: i32) -> Result<Output, Box<dyn Error>> {
    let executable = env::current_exe()?;
    let profile = executable
        .parent()
        .and_then(Path::parent)
        .ok_or("missing test profile")?;
    let output = Command::new(profile.join("examples/smt_comparison"))
        .args(args)
        .output()?;
    if output.status.code() != Some(status) {
        return Err(format!(
            "{args:?}: got {}; stderr={}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }
    Ok(output)
}

#[test]
fn authored_mystery_and_prerequisites_agree_across_both_engines() {
    let cases = ["mystery", "prerequisites"]
        .into_iter()
        .flat_map(|fixture| {
            ["possible", "required", "ruled-out", "inconsistent"].map(|query| (fixture, query))
        });
    for (fixture, query) in cases {
        for engine in ["enumeration", "smt"] {
            let output = run(
                &["--fixture", fixture, "--query", query, "--engine", engine],
                0,
            )
            .unwrap();
            assert!(output.stderr.is_empty());
            let measurement: Value = serde_json::from_slice(&output.stdout).unwrap();
            assert_eq!(measurement["engine"], engine);
            assert!(
                measurement["analysis_seconds"]
                    .as_f64()
                    .unwrap()
                    .is_finite()
            );
            let conclusion = if engine == "smt" {
                &measurement["report"]["conclusion"]["kind"]
            } else {
                &measurement["report"]["conclusion"]
            };
            assert_eq!(conclusion.as_str().unwrap(), query.replace('-', "_"));
            if engine == "smt" {
                assert!(measurement["report"].get("coverage").is_none());
                assert!(measurement["report"]["checks"].as_array().unwrap().len() <= 2);
                assert!(
                    measurement["report"]["solver"]["version_response"]
                        .as_str()
                        .unwrap()
                        .contains("5.3.1")
                );
            }
        }
    }
}

#[test]
fn invalid_dimensions_options_and_deployment_leave_stdout_empty() {
    for args in [
        vec!["--facts", "0"],
        vec!["--facts", "25"],
        vec!["--fixture", "mystery", "--facts", "3"],
        vec!["--engine", "smt", "--soft-timeout-ms", "0"],
        vec![
            "--engine",
            "smt",
            "--solver",
            "/definitely/missing/leitmotif-z3",
        ],
    ] {
        let output = run(&args, 1).unwrap();
        assert!(output.stdout.is_empty());
        assert!(!output.stderr.is_empty());
    }
    let output = run(&["--query", "unsupported"], 2).unwrap();
    assert!(output.stdout.is_empty());
}

#[test]
fn incomplete_enumeration_and_eighty_column_help_remain_inspectable() {
    let output = run(&["--max-candidates", "0"], 0).unwrap();
    let measurement: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(measurement["report"]["conclusion"], "inconclusive");
    assert_eq!(measurement["report"]["coverage"]["exhausted"], false);
    let output = run(&["--help"], 0).unwrap();
    let help = String::from_utf8(output.stdout).unwrap();
    assert!(help.lines().all(|line| line.chars().count() <= 80));
    assert!(help.contains("--soft-timeout-ms <SOFT_TIMEOUT_MS>"));
    assert!(help.contains("enumeration, smt"));
}

#[test]
fn fixed_prerequisite_query_matches_its_measured_two_check_budget() {
    let reference = run(
        &[
            "--facts",
            "12",
            "--query",
            "required",
            "--engine",
            "enumeration",
        ],
        0,
    )
    .unwrap();
    let reference: Value = serde_json::from_slice(&reference.stdout).unwrap();
    assert_eq!(reference["report"]["conclusion"], "required");
    assert_eq!(
        reference["report"]["coverage"]["evaluated_candidates"],
        4096
    );
    assert_eq!(reference["report"]["coverage"]["exhausted"], true);
    let report = run(
        &["--facts", "12", "--query", "required", "--engine", "smt"],
        0,
    )
    .unwrap();
    let report: Value = serde_json::from_slice(&report.stdout).unwrap();
    assert_eq!(report["report"]["conclusion"]["kind"], "required");
    assert_eq!(
        report["report"]["model_id"],
        reference["report"]["model_id"]
    );
    assert_eq!(report["report"]["checks"].as_array().unwrap().len(), 2);
    let witness = report["report"]["conclusion"]["witness"]["values"]
        .as_object()
        .unwrap();
    assert_eq!(witness.len(), 12);
    assert!(
        witness
            .values()
            .all(|value| value["type"] == "bool" && value["value"] == true)
    );
}
