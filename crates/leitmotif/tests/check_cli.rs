//! Process-level contracts for the documented example command.

use std::{
    env,
    error::Error,
    path::Path,
    process::{Command, Output},
};

use serde_json::Value;

type TestResult<T> = Result<T, Box<dyn Error>>;

fn run(args: &[&str], expected_status: i32) -> TestResult<Output> {
    // Cargo's normal test build compiles examples alongside integration tests.
    // Deriving the profile directory also follows custom targets and coverage builds.
    let test_executable = env::current_exe()?;
    let profile = test_executable
        .parent()
        .and_then(Path::parent)
        .ok_or("test executable has no profile directory")?;
    let executable = profile
        .join("examples")
        .join(format!("check{}", env::consts::EXE_SUFFIX));
    let output = Command::new(&executable).args(args).output()?;
    if output.status.code() != Some(expected_status) {
        return Err(format!(
            "{args:?}: expected status {expected_status}, got {}; stdout={}, stderr={}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }
    Ok(output)
}

fn json(args: &[&str], expected_status: i32) -> TestResult<Value> {
    let mut args = args.to_vec();
    args.push("--json");
    let output = run(&args, expected_status)?;
    if !output.stderr.is_empty() {
        return Err(format!("{args:?}: unexpected stderr: {:?}", output.stderr).into());
    }
    // Parsing the whole stream rejects banners, trailing diagnostics and extra reports.
    Ok(serde_json::from_slice(&output.stdout)?)
}

#[test]
fn documented_scenarios_render_their_conclusions_and_exit_codes() {
    let scenarios: &[(&str, i32, &[&str])] = &[
        (
            "town",
            0,
            &[
                "Ending A: reached",
                "Ending D: reached",
                "Invariants: hold across all reachable modeled states",
                "Ending A gate (static feasibility only): Possible",
            ],
        ),
        (
            "blocked-ending",
            1,
            &[
                "Ending A: unreachable in this finite model",
                "Ending D: reached",
                "Ending A gate (static feasibility only): Possible",
            ],
        ),
        (
            "disclosure",
            1,
            &[
                "Violation: mara.truthful_requires_learning at",
                "support/town.rs:",
                "-> mara.testify at",
            ],
        ),
        ("mystery", 0, &["Bo entered the study: Possible"]),
        ("revealed-mystery", 0, &["Bo entered the study: RuledOut"]),
        (
            "contradiction",
            1,
            &[
                "Bo entered the study: Inconsistent",
                "No assignment satisfies the supplied premises together",
                "only_ada_had_key at",
                "no_passage at",
                "ada_did_not_enter at",
                "full premise set, not a minimized conflict core",
            ],
        ),
    ];
    for (scenario, status, conclusions) in scenarios {
        let output = run(&[scenario], *status).unwrap();
        assert!(output.stderr.is_empty(), "{scenario}: {:?}", output.stderr);
        let text = String::from_utf8(output.stdout).unwrap();
        for conclusion in *conclusions {
            assert!(
                text.contains(conclusion),
                "{scenario}: missing {conclusion:?} in {text}"
            );
        }
    }
}

#[test]
fn town_json_contains_reachable_endings_safety_and_static_feasibility() {
    let document = json(&["town"], 0).unwrap();
    assert_eq!(document["analysis"], "town");
    let report = &document["report"];
    for ending in ["ending_a", "ending_d"] {
        assert_eq!(
            report[ending]["schema_version"],
            leitmotif::EXPERIMENTAL_SCHEMA_VERSION
        );
        assert_eq!(report[ending]["context"], "town");
        assert_eq!(report[ending]["conclusion"]["kind"], "reached");
        assert!(
            !report[ending]["conclusion"]["witness"]["steps"]
                .as_array()
                .expect("missing witness steps")
                .is_empty()
        );
    }
    assert_eq!(report["safety"]["conclusion"]["kind"], "holds");
    assert_eq!(report["safety"]["coverage"]["exhausted"], true);
    assert_eq!(report["gate_feasibility"]["conclusion"], "possible");
}

#[test]
fn blocked_ending_json_distinguishes_unreachability_from_static_feasibility() {
    let document = json(&["blocked-ending"], 1).unwrap();
    assert_eq!(document["analysis"], "town");
    let report = &document["report"];
    assert_eq!(report["ending_a"]["conclusion"]["kind"], "unreachable");
    assert_eq!(report["ending_a"]["coverage"]["exhausted"], true);
    assert_eq!(report["ending_d"]["conclusion"]["kind"], "reached");
    assert_eq!(report["gate_feasibility"]["conclusion"], "possible");
}

#[test]
fn disclosure_json_preserves_source_linked_violation_evidence() {
    let document = json(&["disclosure"], 1).unwrap();
    assert_eq!(document["analysis"], "safety");
    let conclusion = &document["report"]["conclusion"];
    assert_eq!(conclusion["kind"], "violation");
    assert_eq!(
        conclusion["invariant"]["id"],
        "mara.truthful_requires_learning"
    );
    assert!(
        conclusion["invariant"]["provenance"]["file"]
            .as_str()
            .expect("missing invariant source")
            .ends_with("support/town.rs")
    );
    let steps = conclusion["witness"]["steps"]
        .as_array()
        .expect("missing violation steps");
    assert_eq!(steps.len(), 1);
    let step = steps.first().expect("missing violating transition");
    assert_eq!(step["transition"]["id"], "mara.testify");
    assert_eq!(step["state"]["values"]["mara.learned"]["value"], false);
    assert_eq!(step["state"]["values"]["mara.claim"]["value"], "truthful");
}

#[test]
fn mystery_json_distinguishes_possible_ruled_out_and_inconsistent() {
    for (scenario, status, conclusion, candidates, premises) in [
        ("mystery", 0, "possible", 3, 1),
        ("revealed-mystery", 0, "ruled_out", 1, 2),
        ("contradiction", 1, "inconsistent", 0, 3),
    ] {
        let document = json(&[scenario], status).unwrap();
        assert_eq!(document["analysis"], "hypothesis", "{scenario}");
        let report = &document["report"];
        assert_eq!(report["conclusion"], conclusion, "{scenario}");
        assert_eq!(report["coverage"]["exhausted"], true, "{scenario}");
        assert_eq!(
            report["coverage"]["satisfying_candidates"], candidates,
            "{scenario}"
        );
        assert_eq!(
            report["premises"]
                .as_array()
                .expect("missing premises")
                .len(),
            premises,
            "{scenario}"
        );
        assert_eq!(
            report.get("witness").unwrap().is_null(),
            scenario != "mystery",
            "{scenario}"
        );
        assert_eq!(
            report.get("counterexample").unwrap().is_null(),
            scenario == "contradiction",
            "{scenario}"
        );
    }
}

#[test]
fn truncated_search_is_inconclusive_in_text_and_json() {
    let args = ["town", "--max-states", "1"];
    let output = run(&args, 2).unwrap();
    assert!(output.stderr.is_empty());
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("Ending A: inconclusive: unexplored states remain"));
    assert!(text.contains("Invariants: inconclusive; no violation found within limits"));
    let document = json(&args, 2).unwrap();
    for analysis in ["ending_a", "ending_d", "safety"] {
        let report = &document["report"][analysis];
        assert_eq!(report["conclusion"]["kind"], "inconclusive");
        assert_eq!(report["coverage"]["limits"]["max_states"], 1);
        assert_eq!(report["coverage"]["evaluated_states"], 1);
        assert_eq!(report["coverage"]["exhausted"], false);
        assert_eq!(report["coverage"]["state_limit_hit"], true);
    }
}

#[test]
fn inconclusive_classification_takes_precedence_over_a_failed_ending() {
    let args = ["blocked-ending", "--max-candidates", "0"];
    let output = run(&args, 2).unwrap();
    assert!(output.stderr.is_empty());
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("Ending A: unreachable in this finite model"));
    assert!(text.contains("Ending A gate (static feasibility only): Inconclusive"));
    let document = json(&args, 2).unwrap();
    let report = &document["report"];
    assert_eq!(report["ending_a"]["conclusion"]["kind"], "unreachable");
    assert_eq!(report["ending_d"]["conclusion"]["kind"], "reached");
    assert_eq!(report["gate_feasibility"]["conclusion"], "inconclusive");
    assert_eq!(
        report["gate_feasibility"]["coverage"]["limits"]["max_candidates"],
        0
    );
    assert_eq!(
        report["gate_feasibility"]["coverage"]["evaluated_candidates"],
        0
    );
    assert_eq!(report["gate_feasibility"]["coverage"]["exhausted"], false);
}

#[test]
fn invalid_arguments_report_usage_on_stderr_without_polluting_json_stdout() {
    for (args, diagnostic) in [
        (vec!["unknown-scenario", "--json"], "invalid value"),
        (vec!["town", "--unknown", "--json"], "unexpected argument"),
        (
            vec!["town", "--max-states", "invalid", "--json"],
            "invalid value",
        ),
        (vec!["town", "--max-depth", "--json"], "a value is required"),
    ] {
        let output = run(&args, 2).unwrap();
        assert!(output.stdout.is_empty(), "{args:?}: {:?}", output.stdout);
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(stderr.contains(diagnostic), "{args:?}: {stderr}");
    }
}
