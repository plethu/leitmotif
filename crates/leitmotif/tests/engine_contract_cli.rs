//! Actual offline process statuses and full serialized report boundaries.
use std::{
    env,
    error::Error,
    fs,
    io::Write,
    path::Path,
    process::{self, Command, Output, Stdio},
};

use serde_json::{Value, json};

type TestResult<T> = Result<T, Box<dyn Error>>;
fn run(args: &[&str], input: Option<&[u8]>) -> TestResult<Output> {
    let test = env::current_exe()?;
    let profile = test
        .parent()
        .and_then(Path::parent)
        .ok_or("missing profile directory")?;
    let path = profile
        .join("examples")
        .join(format!("engine-contracts{}", env::consts::EXE_SUFFIX));
    let mut child = Command::new(path)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    if let Some(input) = input {
        child
            .stdin
            .take()
            .ok_or("missing child input pipe")?
            .write_all(input)?;
    }
    Ok(child.wait_with_output()?)
}
fn request(query: &str) -> TestResult<Value> {
    let output = run(
        &[
            "--source-revision",
            "fixture-source",
            "request",
            "--query",
            query,
        ],
        None,
    )?;
    if !output.status.success() || !output.stderr.is_empty() {
        return Err(format!("request failed: {output:?}").into());
    }
    Ok(serde_json::from_slice(&output.stdout)?)
}
fn check(value: &Value, extra: &[&str], status: i32, kind: &str) -> TestResult<Value> {
    let mut args = vec!["--source-revision", "fixture-source", "check", "-"];
    args.extend_from_slice(extra);
    let output = run(&args, Some(&serde_json::to_vec(value)?))?;
    if output.status.code() != Some(status) || !output.stderr.is_empty() {
        return Err(format!("check expected {status}: {output:?}").into());
    }
    let report: Value = serde_json::from_slice(&output.stdout)?;
    if report.get("kind").and_then(Value::as_str) != Some(kind) {
        return Err(format!("expected {kind}: {report}").into());
    }
    Ok(report)
}
#[test]
fn complete_projection_process_replays_both_endings_and_checks_all_bindings() {
    for (query, ending) in [("sheltered", "sheltered"), ("departed", "departed")] {
        let input = request(query).unwrap();
        let output = check(&input, &[], 0, "checked").unwrap();
        assert_eq!(input["initial"].as_object().unwrap().len(), 6);
        assert_eq!(output["conformance"]["states"], 6);
        assert_eq!(output["conformance"]["cases"].as_array().unwrap().len(), 30);
        assert_eq!(
            output["analysis"]["report"]["conclusion"]["kind"],
            "reached"
        );
        assert_eq!(output["analysis"]["report"]["model_id"], input["model_id"]);
        assert_eq!(output["analysis"]["source_revision"], "fixture-source");
        assert_eq!(output["replay"]["actions"], 3);
        assert_eq!(
            output["replay"]["state"]["values"]["harbour.ending"]["value"],
            ending
        );
    }
}
#[test]
fn process_rejected_unsupported_inconclusive_and_failed_outcomes_are_nonzero() {
    let input = request("sheltered").unwrap();
    check(&input, &["--lighting-fault"], 1, "conformance_failure").unwrap();
    let unreachable = request("storm-passed").unwrap();
    let output = check(&unreachable, &[], 1, "checked").unwrap();
    assert_eq!(
        output["analysis"]["report"]["conclusion"]["kind"],
        "unreachable"
    );
    let mut incomplete = input.clone();
    incomplete["limits"]["max_states"] = json!(0);
    let output = check(&incomplete, &[], 2, "inconclusive").unwrap();
    assert_eq!(
        output["analysis"]["report"]["conclusion"]["kind"],
        "inconclusive"
    );
    let mut unknown = input.clone();
    unknown["query"] = json!("run_engine_script");
    check(&unknown, &[], 2, "unsupported").unwrap();
    let mut rejected = input;
    rejected["initial"]
        .as_object_mut()
        .unwrap()
        .remove("harbour.storm_passed");
    check(&rejected, &[], 2, "rejected_input").unwrap();
    let malformed = run(
        &["--source-revision", "fixture-source", "check", "-"],
        Some(b"not JSON"),
    )
    .unwrap();
    assert_eq!(malformed.status.code(), Some(2));
    let error: Value = serde_json::from_slice(&malformed.stdout).unwrap();
    assert_eq!(error["kind"], "rejected_input");
    let help = run(&["--help"], None).unwrap();
    assert!(help.status.success());
    assert!(
        String::from_utf8(help.stdout)
            .unwrap()
            .contains("--source-revision")
    );
}

#[test]
fn file_backed_projection_and_missing_file_have_distinct_process_outcomes() {
    let input = request("sheltered").unwrap();
    let path = env::temp_dir().join(format!("leitmotif-engine-contract-{}.json", process::id()));
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .unwrap();
    file.write_all(&serde_json::to_vec(&input).unwrap())
        .unwrap();
    drop(file);
    let result = run(
        &[
            "--source-revision",
            "fixture-source",
            "check",
            path.to_str().unwrap(),
        ],
        None,
    );
    fs::remove_file(&path).unwrap();
    let output = result.unwrap();
    assert_eq!(output.status.code(), Some(0));
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["kind"], "checked");
    let missing = run(
        &[
            "--source-revision",
            "fixture-source",
            "check",
            path.to_str().unwrap(),
        ],
        None,
    )
    .unwrap();
    assert_eq!(missing.status.code(), Some(2));
    let value: Value = serde_json::from_slice(&missing.stdout).unwrap();
    assert_eq!(value["kind"], "rejected_input");
}
