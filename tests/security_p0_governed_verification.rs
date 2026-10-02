//! P0-02 regression: verification commands execute through the governed
//! process boundary — sanitized env, workspace containment, no shell.

use m31a::ids::{MissionId, TaskId};
use m31a::verification::executor::{approve_verification_command, execute_governed_verification};
use m31a::verification::runners::VerificationRunner;
use m31a::verification::runners::{CompilerRunner, StaticAnalysisRunner, TestRunner};

fn workspace_root() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn ids() -> (MissionId, TaskId) {
    (MissionId::new(), TaskId::new())
}

// --- Approval semantics -------------------------------------------------

#[test]
fn shell_injection_strings_are_rejected() {
    let ws = workspace_root();
    for cmd in [
        "cargo check; rm -rf /tmp/x",
        "cargo test | tee /tmp/out",
        "cargo check && curl evil.example.com",
        "cargo check $(whoami)",
        "cargo `whoami`",
        "cargo check > /tmp/x",
        "cargo check < /etc/passwd",
        "cargo $HOME",
    ] {
        assert!(
            approve_verification_command(cmd, &ws, vec![]).is_err(),
            "{cmd} must be rejected"
        );
    }
}

#[test]
fn workspace_escape_is_rejected() {
    // A bare `.git` directory is not an execution root and fails closed.
    // (Roots under `.m31a/worktrees/*` are legitimate runtime execution
    // roots and must approve — covered in the executor unit tests.)
    let ws = workspace_root();
    let protected = ws.join(".git");
    assert!(approve_verification_command("cargo check", &protected, vec![]).is_err());
}

#[test]
fn credential_and_loader_env_are_rejected() {
    let ws = workspace_root();
    for key in [
        "NVIDIA_API_KEY",
        "API_KEY_NVIDIA",
        "GITHUB_TOKEN",
        "AWS_SECRET_ACCESS_KEY",
        "LD_PRELOAD",
        "LD_LIBRARY_PATH",
        "PYTHONPATH",
        "GIT_DIR",
    ] {
        let err = approve_verification_command(
            "cargo test",
            &ws,
            vec![(key.to_string(), "x".to_string())],
        )
        .unwrap_err();
        assert!(err.contains("forbidden"), "{key}: {err}");
    }
}

#[test]
fn benign_commands_still_approve() {
    let ws = workspace_root();
    for cmd in [
        "cargo check",
        "cargo test --lib",
        "cargo clippy -- -D warnings",
    ] {
        assert!(
            approve_verification_command(cmd, &ws, vec![]).is_ok(),
            "{cmd} must approve"
        );
    }
}

// --- Governed execution properties ----------------------------------------

#[tokio::test]
async fn verification_child_gets_sanitized_environment() {
    unsafe {
        std::env::set_var(
            "NVIDIA_API_KEY",
            "nvapi-should-never-reach-child-1234567890",
        );
        std::env::set_var("LD_PRELOAD", "/tmp/evil.so");
    }
    // `cargo --version` prints nothing secret, but the authoritative property
    // is the builder map below; this proves governed execution works at all.
    let out = execute_governed_verification(
        "cargo --version",
        &workspace_root(),
        std::time::Duration::from_secs(60),
        vec![],
        None,
    )
    .await
    .expect("governed cargo --version must succeed");
    assert_eq!(out.exit_code, 0);
    assert!(out.stdout.contains("cargo"));
    unsafe {
        std::env::remove_var("NVIDIA_API_KEY");
        std::env::remove_var("LD_PRELOAD");
    }

    let builder = m31a::process::env::EnvironmentBuilder::new(workspace_root());
    let map = builder.build_map();
    for key in [
        "NVIDIA_API_KEY",
        "API_KEY_NVIDIA",
        "GITHUB_TOKEN",
        "LD_PRELOAD",
        "LD_LIBRARY_PATH",
        "PYTHONPATH",
        "GIT_DIR",
    ] {
        assert!(!map.contains_key(key), "sanitized env must lack {key}");
    }
}

#[tokio::test]
async fn runners_reject_injection_and_escape() {
    let (mid, tid) = ids();
    let ws = workspace_root();
    // Injection via configured command surfaces as a governed Err, not a shell.
    let bad = CompilerRunner::with_command("cargo check; echo PWNED");
    assert!(
        bad.execute(mid, tid, &ws, "snap").await.is_err(),
        "injection command must fail closed"
    );
    let bad_test = TestRunner::new().with_command("cargo test | tee /tmp/pwned".to_string());
    assert!(bad_test.execute(mid, tid, &ws, "snap").await.is_err());
    // Protected-path workspace roots fail closed instead of executing.
    let outside = ws.join(".git");
    let runner = CompilerRunner::new();
    assert!(runner.execute(mid, tid, &outside, "snap").await.is_err());
}

#[tokio::test]
async fn custom_commands_work_when_permitted() {
    let (mid, tid) = ids();
    // Explicitly permitted benign custom command still executes.
    let runner = StaticAnalysisRunner::with_command("cargo --version".to_string());
    let check = runner
        .execute(mid, tid, &workspace_root(), "snap")
        .await
        .expect("permitted custom command must execute");
    let _ = check;
}

#[tokio::test]
async fn verification_output_remains_usable() {
    let (mid, tid) = ids();
    // Simulated outputs still parse (no subprocess needed for parsing logic).
    let runner = CompilerRunner::with_simulated(0, "", "    Finished dev profile\n");
    let check = runner
        .execute(mid, tid, &workspace_root(), "snap")
        .await
        .expect("simulated compiler output must parse");
    assert!(check.status.is_passed());
}
