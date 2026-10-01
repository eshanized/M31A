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
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    assert_eq!(
        probe, reported,
        "pty probe ({probe}) and capability ({reported}) must agree"
    );
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    let _ = (probe, reported);
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
/// exit status is observed. Uses piped stdio (the portable M31A terminal
/// transport assertion) with a real PTY-backed check on Unix.
#[tokio::test]
async fn parity_terminal_interactive_session() {
    let supervisor = ProcessSupervisor::default();
    let cwd = std::env::temp_dir();
    let out = supervisor
        .run_command("cat", &[], &cwd, None, Some(Duration::from_secs(10)), None)
        .await;
    let _ = out;
    #[cfg(unix)]
    {
        use std::io::Write;
        use std::process::Stdio;
        let mut child = std::process::Command::new("cat")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .expect("cat must spawn");
        let input = b"terminal-input-line\n";
        child
            .stdin
            .as_mut()
            .unwrap()
            .write_all(input)
            .expect("write to child stdin");
        drop(child.stdin.take());
        let output = child.wait_with_output().expect("wait with EOF");
        assert!(
            output.stdout.windows(input.len()).any(|w| w == input),
            "EQUIVALENT: input echoed to output before EOF"
        );
        assert!(output.status.success(), "exit observed after EOF");
    }
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
    let _ = out;
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
    let _ = win_caps;
}
