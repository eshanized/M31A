//! Remediation Test Suite: CLI Binary Execution & Output Contracts (BLK-01, BLK-06, CLI-01).
//!
//! Verifies:
//! 1. `m31a --version` outputs package version and exits with code 0.
//! 2. `m31a agent list --output json` returns the canonical 8 roles from AGT-01.
//! 3. `m31a capability list` runs against real capability registry and exits 0.
//! 4. `m31a mission list` runs against persistence and exits 0.
//! 5. `m31a doctor --output json` produces valid JSON report.
//! 6. `m31a tui` handles headless/non-interactive invocation gracefully.

use std::process::Command;
use tempfile::tempdir;

#[test]
fn test_cli_binary_version() {
    let bin_path = env!("CARGO_BIN_EXE_m31a");
    let output = Command::new(bin_path)
        .arg("version")
        .output()
        .expect("Failed to execute m31a binary");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("m31a"));
}

#[test]
fn test_cli_binary_canonical_agent_roles() {
    let temp_dir = tempdir().unwrap();
    let bin_path = env!("CARGO_BIN_EXE_m31a");
    let output = Command::new(bin_path)
        .args([
            "--workspace",
            temp_dir.path().to_str().unwrap(),
            "agent",
            "list",
            "--output",
            "json",
        ])
        .output()
        .expect("Failed to execute m31a binary");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("Valid JSON output");

    let roles = parsed["roles"]
        .as_array()
        .expect("Roles array present")
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect::<Vec<_>>();

    // Phase 27.5 (R-01): CLI lists the registered built-in roles from the
    // role registry (single existence authority), not a hardcoded subset.
    let expected = vec![
        "planner",
        "researcher",
        "architect",
        "implementer",
        "reviewer",
        "verifier",
        "diagnostician",
        "integrator",
        "discovery_analyst",
        "stack_researcher",
        "features_researcher",
        "architecture_researcher",
        "pitfalls_researcher",
        "security_researcher",
        "deployment_researcher",
        "synthesizer",
        "auditor",
        "release_certifier",
    ];

    assert_eq!(
        roles, expected,
        "BLK-06: CLI must output registry-registered AGT-01 roles"
    );
}

#[test]
fn test_cli_binary_capability_list() {
    let temp_dir = tempdir().unwrap();
    let bin_path = env!("CARGO_BIN_EXE_m31a");
    let output = Command::new(bin_path)
        .args([
            "--workspace",
            temp_dir.path().to_str().unwrap(),
            "capability",
            "list",
            "--output",
            "json",
        ])
        .output()
        .expect("Failed to execute m31a binary");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("Valid JSON output");
    assert!(parsed.get("capability_count").is_some());
}

#[test]
fn test_cli_binary_mission_list_in_workspace() {
    let temp_dir = tempdir().unwrap();
    let bin_path = env!("CARGO_BIN_EXE_m31a");

    let output = Command::new(bin_path)
        .args([
            "--workspace",
            temp_dir.path().to_str().unwrap(),
            "mission",
            "list",
            "--output",
            "json",
        ])
        .output()
        .expect("Failed to execute m31a binary");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("Valid JSON output");
    assert!(parsed["missions"].is_array());
}

#[test]
fn test_cli_binary_doctor_diagnostics() {
    let temp_dir = tempdir().unwrap();
    let bin_path = env!("CARGO_BIN_EXE_m31a");
    let output = Command::new(bin_path)
        .args([
            "--workspace",
            temp_dir.path().to_str().unwrap(),
            "doctor",
            "--output",
            "json",
        ])
        .output()
        .expect("Failed to execute m31a binary");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("Valid JSON output");
    assert!(parsed.get("overall_status").is_some() || parsed.get("checks").is_some());
}

#[test]
fn test_cli_binary_tui_headless_fallback() {
    let temp_dir = tempdir().unwrap();
    let bin_path = env!("CARGO_BIN_EXE_m31a");

    // In a test subprocess without a tty, running `tui` should not panic
    let output = Command::new(bin_path)
        .args(["--workspace", temp_dir.path().to_str().unwrap(), "tui"])
        .output()
        .expect("Failed to execute m31a binary");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Interactive Cockpit")
            || stdout.contains("Terminal does not support raw mode")
    );
}
