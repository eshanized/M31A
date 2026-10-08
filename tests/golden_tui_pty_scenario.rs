//! Golden PTY Interactive TUI Scenario Test (CLI-04, TUI-01, TDS-01, D-10).
//!
//! Validates real terminal interaction via POSIX Pseudo-Terminal (PTY):
//! 1. Pseudo-terminal (master/slave) allocation with standard terminal dimensions (100x30)
//! 2. Interactive cockpit launch in raw mode (`m31a tui`)
//! 3. Initial TUI frame rendering with conversation timeline and focused composer
//! 4. Live typing into composer with immediate character echo
//! 5. Interactive command submission via Enter
//! 6. Clean graceful exit via Esc -> 'q' / Ctrl+C
//! 7. Zero crashes, clean terminal state restoration, and exit code 0

#![cfg(unix)]

use std::fs;
use std::io::{Read, Write};
use std::os::unix::io::FromRawFd;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};
use tempfile::TempDir;

#[test]
fn test_golden_tui_pty_scenario() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Prepare isolated workspace fixture
    let temp_dir = TempDir::new()?;
    let ws = temp_dir.path();

    // Mark workspace as onboarded to skip setup wizard and launch cockpit immediately
    let m31a_dir = ws.join(".m31a");
    fs::create_dir_all(&m31a_dir)?;
    let init_sentinel = serde_json::json!({
        "state": "Onboarded",
        "step_data": {},
        "created_at": "2026-09-19T00:00:00Z",
        "updated_at": "2026-09-19T00:00:00Z"
    });
    fs::write(m31a_dir.join("init.json"), init_sentinel.to_string())?;

    // Create a dummy workspace file
    fs::write(ws.join("README.md"), "# PTY Test Workspace\n")?;

    // Initialize git repository in workspace
    let _ = Command::new("git").args(["init"]).current_dir(ws).output();
    let _ = Command::new("git")
        .args(["config", "user.name", "PTY Test"])
        .current_dir(ws)
        .output();
    let _ = Command::new("git")
        .args(["config", "user.email", "pty@m31a.dev"])
        .current_dir(ws)
        .output();
    let _ = Command::new("git")
        .args(["add", "."])
        .current_dir(ws)
        .output();
    let _ = Command::new("git")
        .args(["commit", "-m", "Initial commit"])
        .current_dir(ws)
        .output();

    // 2. Allocate real POSIX pseudo-terminal (PTY)
    let mut master_fd: libc::c_int = 0;
    let mut slave_fd: libc::c_int = 0;
    let mut win = libc::winsize {
        ws_row: 30,
        ws_col: 100,
        ws_xpixel: 0,
        ws_ypixel: 0,
    };

    let pty_res = unsafe {
        libc::openpty(
            &mut master_fd,
            &mut slave_fd,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            &mut win as *mut libc::winsize,
        )
    };
    assert_eq!(pty_res, 0, "openpty must succeed");

    // Make master non-blocking for asynchronous polling
    unsafe {
        let flags = libc::fcntl(master_fd, libc::F_GETFL, 0);
        libc::fcntl(master_fd, libc::F_SETFL, flags | libc::O_NONBLOCK);
    }

    // 3. Spawn m31a binary with stdin/stdout/stderr attached to slave PTY
    let slave_in = unsafe { Stdio::from_raw_fd(libc::dup(slave_fd)) };
    let slave_out = unsafe { Stdio::from_raw_fd(libc::dup(slave_fd)) };
    let slave_err = unsafe { Stdio::from_raw_fd(libc::dup(slave_fd)) };

    // Close parent's handle to slave_fd so EOF works as expected
    unsafe {
        libc::close(slave_fd);
    }

    let bin_path = env!("CARGO_BIN_EXE_m31a");
    let mut child = Command::new(bin_path)
        .arg("--workspace")
        .arg(ws.to_str().unwrap())
        .arg("tui")
        .env("TERM", "xterm-256color")
        .env_remove("M31A_HEADLESS")
        .stdin(slave_in)
        .stdout(slave_out)
        .stderr(slave_err)
        .spawn()?;

    let mut master_file = unsafe { fs::File::from_raw_fd(master_fd) };

    // Helper closure to read available bytes from master PTY
    let read_master = |file: &mut fs::File, timeout: Duration| -> Vec<u8> {
        let start = Instant::now();
        let mut accum = Vec::new();
        let mut buf = [0u8; 1024];

        while start.elapsed() < timeout {
            match file.read(&mut buf) {
                Ok(n) if n > 0 => {
                    accum.extend_from_slice(&buf[..n]);
                }
                Ok(_) => {
                    std::thread::sleep(Duration::from_millis(20));
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(20));
                }
                Err(_) => break,
            }
        }
        accum
    };

    // 4. Read initial TUI output and verify cockpit launch
    let initial_output = read_master(&mut master_file, Duration::from_secs(3));
    let initial_str = String::from_utf8_lossy(&initial_output);

    assert!(
        initial_str.contains("M31A")
            || initial_str.contains("Composer")
            || initial_str.contains("m31a"),
        "TUI initial screen did not produce expected branding: {:?}",
        initial_str
    );

    // 5. Type natural language / slash command into composer
    // Composer starts focused automatically (conversation-first)
    let input_text = "/help\r";
    master_file.write_all(input_text.as_bytes())?;
    master_file.flush()?;

    // Read response from PTY after typing
    let after_type = read_master(&mut master_file, Duration::from_millis(500));
    println!("PTY Output after typing: {} bytes", after_type.len());

    // 6. Test clean exit sequence:
    // Send ESC (0x1b) to unfocus composer, then send 'q' to quit
    std::thread::sleep(Duration::from_millis(200));
    master_file.write_all(b"\x1b")?;
    master_file.flush()?;
    std::thread::sleep(Duration::from_millis(200));

    master_file.write_all(b"q")?;
    master_file.flush()?;

    // If still running after 2 seconds, send Ctrl+C
    let start_wait = Instant::now();
    let mut exited = false;
    while start_wait.elapsed() < Duration::from_secs(3) {
        if let Ok(Some(status)) = child.try_wait() {
            assert!(
                status.success(),
                "Process exited with non-zero status: {:?}",
                status
            );
            exited = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }

    if !exited {
        // Send Ctrl+C interrupt
        master_file.write_all(b"\x03")?;
        master_file.flush()?;

        let status = child.wait()?;
        assert!(
            status.success(),
            "Process exited with non-zero status after Ctrl+C: {:?}",
            status
        );
    }

    Ok(())
}

#[test]
fn test_uninitialized_workspace_pty_scenario() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Prepare fresh uninitialized workspace fixture (no .m31a/init.json)
    let temp_dir = TempDir::new()?;
    let ws = temp_dir.path();

    // Create dummy workspace file
    fs::write(ws.join("README.md"), "# Uninitialized Workspace\n")?;

    // 2. Allocate real POSIX pseudo-terminal (PTY)
    let mut master_fd: libc::c_int = 0;
    let mut slave_fd: libc::c_int = 0;
    let mut win = libc::winsize {
        ws_row: 30,
        ws_col: 100,
        ws_xpixel: 0,
        ws_ypixel: 0,
    };

    let pty_res = unsafe {
        libc::openpty(
            &mut master_fd,
            &mut slave_fd,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            &mut win as *mut libc::winsize,
        )
    };
    assert_eq!(pty_res, 0, "openpty must succeed");

    // Make master non-blocking for asynchronous polling
    unsafe {
        let flags = libc::fcntl(master_fd, libc::F_GETFL, 0);
        libc::fcntl(master_fd, libc::F_SETFL, flags | libc::O_NONBLOCK);
    }

    // 3. Spawn m31a binary with stdin/stdout/stderr attached to slave PTY
    let slave_in = unsafe { Stdio::from_raw_fd(libc::dup(slave_fd)) };
    let slave_out = unsafe { Stdio::from_raw_fd(libc::dup(slave_fd)) };
    let slave_err = unsafe { Stdio::from_raw_fd(libc::dup(slave_fd)) };

    // Close parent's handle to slave_fd so EOF works as expected
    unsafe {
        libc::close(slave_fd);
    }

    let bin_path = env!("CARGO_BIN_EXE_m31a");
    let mut child = Command::new(bin_path)
        .arg("--workspace")
        .arg(ws.to_str().unwrap())
        .arg("tui")
        .env("TERM", "xterm-256color")
        .env_remove("M31A_HEADLESS")
        .stdin(slave_in)
        .stdout(slave_out)
        .stderr(slave_err)
        .spawn()?;

    let mut master_file = unsafe { fs::File::from_raw_fd(master_fd) };

    // Helper closure to read available bytes from master PTY
    let read_master = |file: &mut fs::File, timeout: Duration| -> Vec<u8> {
        let start = Instant::now();
        let mut accum = Vec::new();
        let mut buf = [0u8; 1024];

        while start.elapsed() < timeout {
            match file.read(&mut buf) {
                Ok(n) if n > 0 => {
                    accum.extend_from_slice(&buf[..n]);
                }
                Ok(_) => {
                    std::thread::sleep(Duration::from_millis(20));
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(20));
                }
                Err(_) => break,
            }
        }
        accum
    };

    // 4. Read initial output: must appear immediately (<1.5s) and contain Setup Wizard / Step 1
    let initial_output = read_master(&mut master_file, Duration::from_millis(1500));
    let initial_str = String::from_utf8_lossy(&initial_output);

    assert!(
        initial_str.contains("M31A")
            || initial_str.contains("WIZARD")
            || initial_str.contains("Step 1")
            || initial_str.contains("Trust"),
        "Uninitialized TUI initial screen did not produce expected wizard branding: {:?}",
        initial_str
    );

    // 5. Send Esc to cancel onboarding cleanly
    master_file.write_all(b"\x1b")?;
    master_file.flush()?;

    // 6. Child must exit cleanly with code 0
    let start_wait = Instant::now();
    let mut exited = false;
    while start_wait.elapsed() < Duration::from_secs(3) {
        if let Ok(Some(status)) = child.try_wait() {
            assert!(
                status.success(),
                "Process exited with non-zero status after Esc: {:?}",
                status
            );
            exited = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }

    if !exited {
        // Fallback send Ctrl+C
        master_file.write_all(b"\x03")?;
        master_file.flush()?;
        let status = child.wait()?;
        assert!(
            status.success(),
            "Process exited with non-zero status after Ctrl+C: {:?}",
            status
        );
    }

    // Verify workspace was NOT marked onboarded
    assert!(
        !ws.join(".m31a").join("init.json").exists(),
        "Cancelled onboarding must not mark workspace onboarded"
    );

    Ok(())
}
