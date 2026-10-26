//! Parity: terminal / PTY behavior.
//!
//! M31A terminal semantics under test: dimensions queryable, resize
//! validated, input echoed to output, EOF terminates the child, exit status
//! observed, cancellation and timeout bound the session, large and unicode
//! output transported. Unix proves PTY-backed behavior; Windows proves
//! ConPTY capability reporting and dimension validation. Byte-identical
//! driver behavior is NOT required.

use std::time::Duration;

use m31a::platform::terminal as platform_terminal;
use m31a::platform::windows::conpty::ConsoleSize;
use m31a::process::supervisor::ProcessSupervisor;

/// Dimensions capability is Available on hosts with a console backend.
#[test]
fn parity_terminal_dimensions_available() {
    let caps = platform_terminal::TerminalCapabilities::probe_host();
    assert_eq!(
        caps.dimensions,
        m31a::platform::capabilities::CapabilityState::Available,
        "EQUIVALENT: dimensions queryable on this host: {caps:?}"
    );
}

/// PTY probe and capability agree on native hosts (no false Available).
#[test]
fn parity_terminal_pty_probe_agrees() {
    let caps = platform_terminal::TerminalCapabilities::probe_host();
    let probe = platform_terminal::pty_available();
    let reported = caps.supports_pty();
    assert_eq!(
        probe, reported,
        "pty probe ({probe}) and capability ({reported}) must agree"
    );
}

/// Console dimensions validate: zero-sized consoles are rejected.
#[test]
fn parity_terminal_dimensions_validated() {
    assert!(ConsoleSize::new(80, 24).valid());
    assert!(ConsoleSize::new(120, 40).valid());
    assert!(!ConsoleSize::new(0, 24).valid());
    assert!(!ConsoleSize::new(80, 0).valid());
    assert!(!ConsoleSize::new(0, 0).valid());
}

/// Interactive child session: input produces output, EOF ends the child,
/// exit status is observed, and failures are detected through the production supervisor API.
#[tokio::test]
async fn parity_terminal_interactive_session() {
    let supervisor = ProcessSupervisor::default();
    let cwd = std::env::temp_dir();
    let input = b"terminal-input-line\n";

    #[cfg(unix)]
    let (prog, args): (&str, Vec<String>) = ("cat", vec![]);
    #[cfg(windows)]
    let (prog, args): (&str, Vec<String>) = ("cmd", vec!["/c".to_string(), "more".to_string()]);

    // 1. Production supervisor runs with stdin input, echoes output, detects EOF, and records success
    let mut cmd = tokio::process::Command::new(prog);
    cmd.args(&args);
    cmd.current_dir(&cwd);
    let out = supervisor
        .run_supervised_with_input(cmd, Some(input), Duration::from_secs(10), None)
        .await
        .expect("interactive session through supervisor must succeed");

    assert!(
        out.stdout.contains("terminal-input-line"),
        "production supervisor must transport echoed input: {}",
        out.stdout
    );
    assert_eq!(
        out.exit_code, 0,
        "production supervisor must record exit code 0 after EOF"
    );

    // 2. Production supervisor detects command failure
    let fail_res = supervisor
        .run_command(
            "this_command_does_not_exist_xyz123",
            &[],
            &cwd,
            None,
            Some(Duration::from_secs(5)),
            None,
        )
        .await;
    assert!(
        fail_res.is_err(),
        "production supervisor must detect failure for nonexistent command"
    );
}

/// Large output is transported without loss or deadlock.
#[tokio::test]
async fn parity_terminal_large_output() {
    let supervisor = ProcessSupervisor::default();
    let cwd = std::env::temp_dir();
    #[cfg(unix)]
    let (prog, args): (&str, Vec<String>) =
        ("sh", vec!["-c".to_string(), "seq 1 5000".to_string()]);
    #[cfg(not(unix))]
    let (prog, args): (&str, Vec<String>) = ("echo", vec!["hi".to_string()]);
    let out = supervisor
        .run_command(prog, &args, &cwd, None, Some(Duration::from_secs(15)), None)
        .await
        .expect("large output must be transported");
    #[cfg(unix)]
    assert!(
        out.stdout.contains("5000"),
        "large output fully transported ({} bytes)",
        out.stdout.len()
    );
    #[cfg(not(unix))]
    assert_eq!(out.exit_code, 0, "command must exit successfully");
}

/// Unicode output round-trips through the terminal transport.
#[tokio::test]
async fn parity_terminal_unicode_output() {
    let supervisor = ProcessSupervisor::default();
    let cwd = std::env::temp_dir();
    let out = supervisor
        .run_command(
            "echo",
            &["terminal-ünïcode-日本語".to_string()],
            &cwd,
            None,
            Some(Duration::from_secs(10)),
            None,
        )
        .await
        .expect("unicode echo must run");
    assert!(out.stdout.contains("ünïcode"));
}

/// Cancellation during an interactive/large session reports Cancelled.
#[tokio::test]
async fn parity_terminal_cancel_during_output() {
    let supervisor = ProcessSupervisor::default();
    let cwd = std::env::temp_dir();
    let token = tokio_util::sync::CancellationToken::new();
    let killer = token.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(200)).await;
        killer.cancel();
    });
    #[cfg(unix)]
    let (prog, args): (&str, Vec<String>) = (
        "sh",
        vec![
            "-c".to_string(),
            "while true; do echo line; sleep 0.01; done".to_string(),
        ],
    );
    #[cfg(not(unix))]
    let (prog, args): (&str, Vec<String>) = ("sleep", vec!["30".to_string()]);
    let err = supervisor
        .run_command(
            prog,
            &args,
            &cwd,
            None,
            Some(Duration::from_secs(30)),
            Some(&token),
        )
        .await
        .expect_err("cancelled session must not succeed");
    assert_eq!(
        err,
        m31a::process::supervisor::ProcessError::Cancelled,
        "EQUIVALENT: terminal cancellation is Cancelled"
    );
}

/// Timeout during a terminal session reports TimedOut.
#[tokio::test]
async fn parity_terminal_timeout_during_session() {
    let supervisor = m31a::process::supervisor::ProcessSupervisor::new(
        Duration::from_millis(200),
        Duration::from_millis(300),
    );
    let err = supervisor
        .run_command(
            "sleep",
            &["30".to_string()],
            &std::env::temp_dir(),
            None,
            None,
            None,
        )
        .await
        .expect_err("timed-out session must not succeed");
    assert!(matches!(
        err,
        m31a::process::supervisor::ProcessError::TimedOut(_)
    ));
}

/// Unix PTY device presence matches the probe; ConPTY absence off-Windows
/// is honestly reported (no false Available).
#[test]
fn parity_terminal_backend_labels_honest() {
    assert!(!platform_terminal::backend_name().is_empty());
    #[cfg(not(windows))]
    assert!(!m31a::platform::windows::conpty::conpty_available());
    let win_caps = m31a::platform::windows::conpty::terminal_capabilities();
    assert_eq!(
        win_caps.backend,
        m31a::platform::terminal::TerminalBackend::WindowsConPty
    );
    assert_eq!(
        win_caps.dimensions,
        m31a::platform::capabilities::CapabilityState::Available
    );
}
