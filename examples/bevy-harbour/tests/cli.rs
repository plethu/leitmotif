//! Exercise the shipped headless executable and its report boundary.
use std::process::Command;

#[test]
fn headless_executable_reports_audited_outcomes_and_the_deliberate_fault() {
    let output = Command::new(env!("CARGO_BIN_EXE_bevy-harbour"))
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("6 reachable states; 30 independent scheduled action checks"));
    assert!(stdout.contains("sheltered: Reached { actions: 3"));
    assert!(stdout.contains("departed: Reached { actions: 3"));
    assert!(stdout.contains("storm_passed: Unreachable"));
    assert!(stdout.contains("bounded shelter: Inconclusive"));
    assert!(stdout.contains("deliberate missing oil effect detected: keeper.light_beacon"));
    assert!(
        stdout.contains("\n  harbour.oil_available: expected Bool(false), actual Bool(true)\n")
    );
    assert!(stdout.contains("\n  source: "));
    assert!(stdout.contains("\n  expected snapshot:\n"));
    assert!(stdout.contains("\n  actual snapshot:\n"));
    assert!(stdout.contains("\n  execution: verified prefix 1, accepted actions 2"));
    assert!(output.stderr.is_empty());
}
