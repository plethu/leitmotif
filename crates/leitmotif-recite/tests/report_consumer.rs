//! The CI consumer is a separate process which imports no Rust model types.
use std::{env, error::Error, path::Path, process::Command};

#[test]
fn archive_reports_reach_the_real_ci_consumer() -> Result<(), Box<dyn Error>> {
    let test = env::current_exe()?;
    let profile = test
        .parent()
        .and_then(Path::parent)
        .ok_or("missing profile directory")?;
    let producer = profile
        .join("examples")
        .join(format!("archive{}", env::consts::EXE_SUFFIX));
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .ok_or("missing workspace root")?;
    let output = Command::new("python")
        .arg(root.join("tests/tooling/archive_consumer_fixture.py"))
        .arg(producer)
        .output()?;
    if !output.status.success() {
        return Err(format!(
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }
    Ok(())
}
