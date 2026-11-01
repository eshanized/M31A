//! Integration tests for the M31A command line interface.
//!
//! Tests CLI argument parsing, doctor checks, setup wizard workflows, and subcommand routing.

use std::process::Command;
use tempfile::tempdir;

fn m31a_bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_m31a"))
}

#[test]
fn test_cli_version_and_help() {
    // 1. --version
    let output = m31a_bin().arg("--version").output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("0.1.13") || stdout.contains("m31a"));

    // 2. --help
    let output = m31a_bin().arg("--help").output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Usage: m31a") || stdout.contains("Commands:"));
    assert!(stdout.contains("exec"));
    assert!(stdout.contains("doctor"));
    assert!(stdout.contains("presets"));
}

#[test]
fn test_cli_presets_subcommand() {
    // List presets
    let output = m31a_bin().arg("presets").output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Security Presets"));
    assert!(stdout.contains("read-only"));
    assert!(stdout.contains("standard"));
    assert!(stdout.contains("airgapped"));

    // Inspect specific preset
    let output = m31a_bin().args(["presets", "airgapped"]).output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Airgapped"));
    assert!(stdout.contains("Sandbox Policy"));
}

#[test]
fn test_cli_trust_management() {
    let tmp = tempdir().unwrap();
    let tmp_path = tmp.path().to_str().unwrap();

    // Trust add
    let output = m31a_bin()
        .args(["trust", "add", tmp_path])
        .output()
        .unwrap();
    assert!(output.status.success());

    // Trust list
    let output = m31a_bin().args(["trust", "list"]).output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains(tmp_path) || stdout.contains("Trusted Directories"));

    // Trust audit
    let output = m31a_bin()
        .args(["trust", "audit", tmp_path])
        .output()
        .unwrap();
    assert!(output.status.success());
}

#[test]
fn test_cli_doctor_diagnostics() {
    let output = m31a_bin().arg("doctor").output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("M31A Doctor Diagnostics Report"));
    assert!(stdout.contains("Git Workspace Diagnostics"));
    assert!(stdout.contains("Sandboxing & Isolation Engine"));
}

#[test]
fn test_cli_ps_command() {
    let output = m31a_bin().arg("ps").output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("ID") || stdout.contains("No managed background processes"));
}

#[test]
fn test_cli_apply_patch_file() {
    let tmp = tempdir().unwrap();
    let file_path = tmp.path().join("source.txt");
    std::fs::write(&file_path, "version = 1\n").unwrap();

    let patch_content = r#"*** Begin Patch
*** Update File: source.txt
@@
-version = 1
+version = 2
*** End Patch
"#;
    let patch_file = tmp.path().join("change.patch");
    std::fs::write(&patch_file, patch_content).unwrap();

    let output = m31a_bin()
        .current_dir(tmp.path())
        .args(["apply-patch", patch_file.to_str().unwrap()])
        .output()
        .unwrap();

    assert!(output.status.success());
    let updated = std::fs::read_to_string(&file_path).unwrap();
    assert_eq!(updated, "version = 2\n");
}
