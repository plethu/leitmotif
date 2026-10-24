//! Process contracts for Linux town measurements, including peak RSS.

#![cfg(target_os = "linux")]

use std::{
    env,
    error::Error,
    path::Path,
    process::{Command, Output},
};

use serde_json::Value;

fn run(args: &[&str], expected_status: i32) -> Result<Output, Box<dyn Error>> {
    let test_executable = env::current_exe()?;
    let profile = test_executable
        .parent()
        .and_then(Path::parent)
        .ok_or("test executable has no profile directory")?;
    let output = Command::new(profile.join("examples").join("town_scaling"))
        .args(args)
        .output()?;
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

fn measurement(local_domain: &str, args: &[&str]) -> Result<Value, Box<dyn Error>> {
    let mut small_town = vec!["--threads", "2", "--local-domain", local_domain];
    small_town.extend_from_slice(args);
    let output = run(&small_town, 0)?;
    if !output.stderr.is_empty() {
        return Err(format!("unexpected diagnostics: {:?}", output.stderr).into());
    }
    let report: Value = serde_json::from_slice(&output.stdout)?;
    let elapsed = report
        .get("analysis_seconds")
        .and_then(Value::as_f64)
        .ok_or("missing analysis duration")?;
    if elapsed < 0.0 || !elapsed.is_finite() {
        return Err("analysis duration must be finite and nonnegative".into());
    }
    if report
        .get("peak_rss_bytes")
        .and_then(Value::as_u64)
        .ok_or("missing peak RSS")?
        == 0
    {
        return Err("peak RSS must be positive".into());
    }
    Ok(report)
}

#[test]
fn unreachable_measurement_reports_an_exhausted_graph_and_process_metrics() {
    let report = measurement("2", &["--max-states", "4", "--max-depth", "2"]).unwrap();
    assert_eq!(report["parameters"]["threads"], 2);
    assert_eq!(report["parameters"]["local_domain"], 2);
    assert_eq!(report["parameters"]["coupling"], 0);
    assert_eq!(report["parameters"]["resource"], "unlimited");
    assert_eq!(report["query"], "unreachable");
    assert_eq!(report["conclusion"], "unreachable");
    assert!(report.get("witness_length").unwrap().is_null());
    let coverage = &report["coverage"];
    assert_eq!(coverage["discovered_states"], 4);
    assert_eq!(coverage["evaluated_states"], 4);
    assert_eq!(coverage["deepest_evaluated"], 2);
    assert_eq!(coverage["exhausted"], true);
    assert_eq!(coverage["limits"]["max_states"], 4);
    assert_eq!(coverage["limits"]["max_depth"], 2);
    assert_eq!(coverage["state_limit_hit"], false);
    assert_eq!(coverage["depth_limit_hit"], false);
}

#[test]
fn reached_goal_reports_a_witness_length_without_claiming_exhaustion() {
    let report = measurement("2", &["--query", "goal"]).unwrap();
    assert_eq!(report["query"], "goal");
    assert_eq!(report["conclusion"], "reached");
    assert_eq!(report["witness_length"], 2);
    assert_eq!(report["coverage"]["discovered_states"], 4);
    assert_eq!(report["coverage"]["evaluated_states"], 4);
    assert_eq!(report["coverage"]["exhausted"], false);
}

#[test]
fn resource_and_coupling_arguments_restrict_the_measured_graph() {
    for (local_domain, args, states) in [
        (
            "2",
            vec!["--resource", "single-permit", "--query", "goal"],
            3,
        ),
        ("2", vec!["--coupling", "1", "--query", "goal"], 3),
        // A middle phase must keep the permit held, rather than enabling another start.
        (
            "3",
            vec!["--resource", "single-permit", "--query", "goal"],
            5,
        ),
    ] {
        let report = measurement(local_domain, &args).unwrap();
        assert_eq!(report["conclusion"], "unreachable");
        assert!(report.get("witness_length").unwrap().is_null());
        assert_eq!(report["coverage"]["discovered_states"], states);
        assert_eq!(report["coverage"]["evaluated_states"], states);
        assert_eq!(report["coverage"]["exhausted"], true);
    }
}

#[test]
fn truncated_measurements_keep_the_requested_limit_and_inconclusive_conclusion() {
    for (flag, limit, coverage_flag, states) in [
        ("--max-states", "max_states", "state_limit_hit", 1),
        ("--max-depth", "max_depth", "depth_limit_hit", 3),
    ] {
        let report = measurement("2", &[flag, "1"]).unwrap();
        assert_eq!(report["conclusion"], "inconclusive", "{flag}");
        assert!(report.get("witness_length").unwrap().is_null());
        assert_eq!(report["coverage"]["exhausted"], false, "{flag}");
        assert_eq!(report["coverage"][coverage_flag], true, "{flag}");
        assert_eq!(report["coverage"]["limits"][limit], 1, "{flag}");
        assert_eq!(report["coverage"]["discovered_states"], states, "{flag}");
        assert_eq!(report["coverage"]["evaluated_states"], states, "{flag}");
    }
}

#[test]
fn invalid_parameters_fail_without_emitting_measurement_json() {
    for (args, diagnostic) in [
        (vec!["--threads", "0"], "threads must be between 1 and 16"),
        (vec!["--threads", "17"], "threads must be between 1 and 16"),
        (
            vec!["--local-domain", "1"],
            "local domain must be between 2 and 8",
        ),
        (
            vec!["--local-domain", "9"],
            "local domain must be between 2 and 8",
        ),
        (
            vec!["--threads", "2", "--coupling", "2"],
            "coupling must be smaller than the thread count",
        ),
    ] {
        let output = run(&args, 1).unwrap();
        assert!(output.stdout.is_empty(), "{args:?}: {:?}", output.stdout);
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(stderr.contains(diagnostic), "{args:?}: {stderr}");
    }
    let output = run(&["--resource", "unknown"], 2).unwrap();
    assert!(output.stdout.is_empty());
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("invalid value")
    );
}
