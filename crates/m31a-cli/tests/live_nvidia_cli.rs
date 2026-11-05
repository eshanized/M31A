//! Live CLI integration tests for M31A targeting real NVIDIA Nemotron 3 Ultra.
//!
//! Target:
//! - Binary: `m31a`
//! - Model: nvidia/nemotron-3-ultra-550b-a55b
//! - Provider: NVIDIA NIM
//!
//! Gated by:
//! - Cargo feature: `live-nvidia-tests`
//! - Environment variable: `M31A_RUN_LIVE_NVIDIA_TESTS=1`

use m31a_provider::live_test_support::{NVIDIA_NEMOTRON_LIGHTNING, load_live_test_api_key};
use std::process::Command;
use std::time::Duration;
use tempfile::tempdir;

fn m31a_bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_m31a"))
}

fn run_command_with_timeout(mut cmd: Command, timeout: Duration) -> std::process::Output {
    let mut child = cmd
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("failed to spawn child process");

    let start = std::time::Instant::now();
    loop {
        match child.try_wait().expect("try_wait failed") {
            Some(_) => {
                return child
                    .wait_with_output()
                    .expect("failed to wait with output");
            }
            None => {
                if start.elapsed() >= timeout {
                    let _ = child.kill();
                    let _ = child.wait();
                    panic!("CLI process timed out after {:?}", timeout);
                }
                std::thread::sleep(Duration::from_millis(50));
            }
        }
    }
}

#[test]
#[ignore = "Live NVIDIA NIM integration test requiring M31A_RUN_LIVE_NVIDIA_TESTS=1"]
fn test_real_live_cli_exec_nemotron_ultra() {
    let api_key = load_live_test_api_key();
    let tmp = tempdir().expect("tempdir");

    // Authorize workspace under zero-trust policy
    let trust_output = run_command_with_timeout(
        {
            let mut cmd = m31a_bin();
            cmd.env("M31A_HOME", tmp.path().join(".m31a")).args([
                "trust",
                "add",
                tmp.path().to_str().unwrap(),
            ]);
            cmd
        },
        Duration::from_secs(15),
    );
    assert!(trust_output.status.success());

    let start = std::time::Instant::now();
    let mut exec_cmd = m31a_bin();
    exec_cmd
        .current_dir(tmp.path())
        .env("NVIDIA_API_KEY", &api_key)
        .env("M31A_HOME", tmp.path().join(".m31a"))
        .args([
            "-m",
            NVIDIA_NEMOTRON_LIGHTNING,
            "exec",
            "What is 100 divided by 4? Answer with just the numeric result.",
        ]);

    let output = run_command_with_timeout(exec_cmd, Duration::from_secs(60));

    let elapsed = start.elapsed();
    println!("Live CLI execution completed in {} ms", elapsed.as_millis());

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // Verify process succeeded
    assert!(
        output.status.success(),
        "CLI exec failed with exit status {:?}\nstderr:\n{}\nstdout:\n{}",
        output.status.code(),
        stderr,
        stdout
    );

    // Verify non-empty output and exact semantic answer token
    assert!(!stdout.trim().is_empty(), "CLI stdout must not be empty");
    let contains_exact_25 = stdout.split_whitespace().any(|word| {
        let cleaned = word.trim_matches(|c: char| !c.is_alphanumeric());
        cleaned == "25"
    });
    assert!(
        contains_exact_25,
        "Expected exact answer token '25' in stdout, got:\n{}",
        stdout
    );

    // Verify no secret leakage
    assert!(
        !stdout.contains(&api_key),
        "CLI output must not leak NVIDIA_API_KEY"
    );
    assert!(
        !stderr.contains(&api_key),
        "CLI stderr must not leak NVIDIA_API_KEY"
    );
}

#[test]
#[ignore = "Live NVIDIA NIM integration test requiring M31A_RUN_LIVE_NVIDIA_TESTS=1"]
fn test_real_live_cli_invalid_key_error_propagation() {
    m31a_provider::live_test_support::ensure_live_test_opt_in();
    let tmp = tempdir().expect("tempdir");

    let trust_output = run_command_with_timeout(
        {
            let mut cmd = m31a_bin();
            cmd.env("M31A_HOME", tmp.path().join(".m31a")).args([
                "trust",
                "add",
                tmp.path().to_str().unwrap(),
            ]);
            cmd
        },
        Duration::from_secs(15),
    );
    assert!(trust_output.status.success());

    let bad_key = "nvapi-invalid-bogus-key-for-cli-test";
    let mut exec_cmd = m31a_bin();
    exec_cmd
        .current_dir(tmp.path())
        .env("NVIDIA_API_KEY", bad_key)
        .env("M31A_HOME", tmp.path().join(".m31a"))
        .args(["-m", NVIDIA_NEMOTRON_LIGHTNING, "exec", "Say hello."]);

    let output = run_command_with_timeout(exec_cmd, Duration::from_secs(30));

    // Must fail honestly
    assert!(
        !output.status.success(),
        "CLI must fail when given an invalid API key"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let combined = format!("{stdout}\n{stderr}");

    assert!(
        combined.contains("401")
            || combined.contains("Unauthorized")
            || combined.contains("API Error")
            || combined.contains("error"),
        "Error output must indicate honest failure: {combined}"
    );

    // Verify bad key is not echoed
    assert!(
        !combined.contains(bad_key),
        "Error output must not echo sensitive keys"
    );
}
