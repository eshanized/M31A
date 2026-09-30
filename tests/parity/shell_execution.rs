//! Parity: shell execution (direct argv vs shell string).
//!
//! Direct argv execution is the preferred safe path on every host: program
//! plus argument vector, never shell-interpreted. Shell-string execution is
//! explicitly authorized, policy-gated, and resolves the interpreter per
//! host (sh on Unix, cmd.exe/PowerShell on Windows).

use std::time::Duration;

use m31a::platform::shell as platform_shell;
use m31a::process::supervisor::ProcessSupervisor;

/// Direct argv: arguments with spaces survive intact, no splitting.
#[tokio::test]
async fn parity_shell_direct_argv_preserves_spaces() {
    let supervisor = ProcessSupervisor::default();
    let cwd = std::env::temp_dir();
    let out = supervisor
        .run_command(
            "echo",
            &["hello world".to_string()],
            &cwd,
            None,
            Some(Duration::from_secs(10)),
            None,
        )
        .await
        .expect("echo must run");
    assert!(
        out.stdout.contains("hello world"),
        "EQUIVALENT: argv with spaces is one argument, got {:?}",
        out.stdout
    );
}

/// Direct argv: metacharacters are data, never syntax.
#[tokio::test]
async fn parity_shell_direct_argv_no_injection() {
    let supervisor = ProcessSupervisor::default();
    let cwd = std::env::temp_dir();
    let payload = "a; echo PWNED";
    let out = supervisor
        .run_command(
            "echo",
            &[payload.to_string()],
            &cwd,
            None,
            Some(Duration::from_secs(10)),
            None,
        )
        .await
        .expect("echo must run");
    assert!(
        out.stdout.contains("a; echo PWNED"),
        "EQUIVALENT: ';' in argv is literal data, got {:?}",
        out.stdout
    );
}

/// Direct argv: unicode arguments round-trip.
#[tokio::test]
async fn parity_shell_direct_argv_unicode() {
    let supervisor = ProcessSupervisor::default();
    let cwd = std::env::temp_dir();
    let out = supervisor
        .run_command(
            "echo",
            &["héllo-世界-🦀".to_string()],
            &cwd,
            None,
            Some(Duration::from_secs(10)),
            None,
        )
        .await
        .expect("echo must run");
    assert!(out.stdout.contains("héllo-世界"));
}

/// Shell-string via the platform contract: explicit, interpreter-resolved.
#[tokio::test]
async fn parity_shell_string_runs_through_native_interpreter() {
    let cwd = std::env::temp_dir();
    let build = platform_shell::build_shell_command("echo shell-ok", &cwd);
    assert!(
        build.is_ok(),
        "EQUIVALENT: native shell resolves on this host"
    );
    let cmd = m31a::process::supervisor::execute_shell_string("echo shell-ok", &cwd);
    let supervisor = ProcessSupervisor::default();
    let out = supervisor
        .run_supervised(cmd, Duration::from_secs(10), None)
        .await
        .expect("shell string must execute");
    assert!(
        out.stdout.contains("shell-ok"),
        "shell-string output observed: {out:?}"
    );
}

/// Shell exit status and stderr propagate through M31A results.
#[tokio::test]
async fn parity_shell_exit_status_and_stderr() {
    let supervisor = ProcessSupervisor::default();
    let cwd = std::env::temp_dir();
    #[cfg(unix)]
    let cmd = m31a::process::supervisor::execute_shell_string(
        "echo out-msg; echo err-msg >&2; exit 3",
        &cwd,
    );
    #[cfg(not(unix))]
    let cmd = m31a::process::supervisor::execute_shell_string("echo out-msg", &cwd);
    let out = supervisor
        .run_supervised(cmd, Duration::from_secs(10), None)
        .await
        .expect("shell must run");
    assert!(out.stdout.contains("out-msg"));
    #[cfg(unix)]
    {
        assert_eq!(out.exit_code, 3, "exit status propagates");
        assert!(out.stderr.contains("err-msg"), "stderr propagates");
    }
}

/// Shell working directory is honored.
#[tokio::test]
async fn parity_shell_working_directory() {
    let dir = tempfile::Builder::new()
        .prefix("m31a-p49-shell-cwd")
        .tempdir()
        .unwrap();
    let supervisor = ProcessSupervisor::default();
    let out = supervisor
        .run_command(
            "pwd",
            &[],
            dir.path(),
            None,
            Some(Duration::from_secs(10)),
            None,
        )
        .await
        .expect("pwd must run");
    let expected = dir
        .path()
        .canonicalize()
        .unwrap()
        .to_string_lossy()
        .to_string();
    let got = out.stdout.trim().to_string();
    let got_canon = std::path::Path::new(&got)
        .canonicalize()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or(got);
    assert_eq!(got_canon, expected, "cwd is honored");
}

/// Shell timeout and cancellation behave like direct execution.
#[tokio::test]
async fn parity_shell_timeout_and_cancel() {
    let supervisor = m31a::process::supervisor::ProcessSupervisor::new(
        Duration::from_millis(200),
        Duration::from_millis(300),
    );
    let cwd = std::env::temp_dir();
    #[cfg(unix)]
    let probe = "sleep 30";
    #[cfg(not(unix))]
    let probe = "echo hi";
    let cmd = m31a::process::supervisor::execute_shell_string(probe, &cwd);
    #[cfg(unix)]
    {
        let err = supervisor
            .run_supervised(cmd, Duration::from_millis(200), None)
            .await
            .expect_err("over-deadline shell must time out");
        assert!(matches!(
            err,
            m31a::process::supervisor::ProcessError::TimedOut(_)
        ));
    }
    #[cfg(not(unix))]
    {
        let _ = supervisor
            .run_supervised(cmd, Duration::from_secs(10), None)
            .await;
    }
}

/// Windows quoting helpers preserve boundaries (pure, runs everywhere).
#[test]
fn parity_shell_windows_quoting_boundaries() {
    use m31a::platform::windows::shell as wshell;
    assert_eq!(wshell::quote_cmd_arg(""), "\"\"");
    let q = wshell::quote_cmd_arg("a & b");
    assert!(q.starts_with('"') && q.ends_with('"') && q.contains("a & b"));
    assert_eq!(wshell::quote_powershell_arg("it's"), "'it''s'");
    assert!(wshell::contains_cmd_metachars("a & b"));
    assert!(!wshell::contains_cmd_metachars("plain"));
    assert!(wshell::contains_powershell_metachars("$x"));
    assert!(wshell::argv_preserves_boundaries(&["a;b".to_string()]));
}

/// Shell interpreter selection is explicit per host, never a fixed name.
#[test]
fn parity_shell_interpreter_selection_explicit() {
    let program = platform_shell::native_shell_program();
    assert!(program.is_some(), "a native interpreter resolves here");
    #[cfg(not(windows))]
    assert_eq!(program, Some("sh"));
    assert!(!platform_shell::backend_name().is_empty());
    assert!(platform_shell::is_shell_program("sh"));
    assert!(!platform_shell::is_shell_program("python3"));
}
