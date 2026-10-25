//! The authoring process emits official assets without embedding a compiler in Godot.
use std::{
    env, fs,
    path::{Path, PathBuf},
    process::{self, Command},
};

use recite_core::compiled::{
    decode_compiled_dialogue_messagepack, encode_compiled_dialogue_messagepack,
};

struct OutputDirectory(PathBuf);
impl Drop for OutputDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn authoring_cli_roundtrips_asset_and_reports_invalid_arguments_and_write_failures() {
    let executable = env::current_exe()
        .unwrap()
        .parent()
        .and_then(Path::parent)
        .ok_or("missing profile directory")
        .unwrap()
        .join("examples")
        .join(format!("harbour-dialogue{}", env::consts::EXE_SUFFIX));
    let directory =
        OutputDirectory(env::temp_dir().join(format!("leitmotif-harbour-asset-{}", process::id())));
    fs::create_dir(&directory.0).unwrap();
    let asset_path = directory.0.join("harbour dialogue.recitec");
    let produced = Command::new(&executable)
        .arg("--compiled-asset")
        .arg(&asset_path)
        .output()
        .unwrap();
    assert!(produced.status.success(), "{produced:?}");
    assert!(produced.stdout.is_empty() && produced.stderr.is_empty());
    let bytes = fs::read(&asset_path).unwrap();
    let asset = decode_compiled_dialogue_messagepack(&bytes).unwrap();
    assert_eq!(asset.effects.len(), 5);
    assert_eq!(asset.choices.len(), 2);
    assert_eq!(encode_compiled_dialogue_messagepack(&asset).unwrap(), bytes);
    let write_failure = Command::new(&executable)
        .arg("--compiled-asset")
        .arg(&directory.0)
        .output()
        .unwrap();
    assert!(!write_failure.status.success());
    assert!(!write_failure.stderr.is_empty());
    assert_eq!(fs::read(&asset_path).unwrap(), bytes);
    for args in [["--compiled-asset"].as_slice(), ["--unknown"].as_slice()] {
        let invalid = Command::new(&executable).args(args).output().unwrap();
        assert_eq!(invalid.status.code(), Some(2));
        assert!(invalid.stdout.is_empty());
    }
    let default_run = Command::new(executable).output().unwrap();
    assert!(default_run.status.success(), "{default_run:?}");
    assert!(
        String::from_utf8(default_run.stdout)
            .unwrap()
            .contains("Captured beacon after ferry: open")
    );
}
