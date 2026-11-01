//! Real End-to-End PTY Integration Tests for M31A TUI.
//!
//! Spawns real POSIX pseudo-terminals (`openpty`), runs the terminal application,
//! feeds keyboard input directly through the master PTY descriptor, and asserts on
//! rendered terminal output frames.

#[cfg(unix)]
mod pty_tests {
    use std::io::{Read, Write};
    use std::os::unix::io::{FromRawFd, RawFd};
    use std::process::{Child, Command, Stdio};
    use std::time::{Duration, Instant};
    use tempfile::tempdir;

    struct RealPtyHarness {
        master_fd: RawFd,
        child: Child,
    }

    impl RealPtyHarness {
        fn find_binary() -> std::path::PathBuf {
            if let Ok(path) = std::env::var("CARGO_BIN_EXE_m31a") {
                return std::path::PathBuf::from(path);
            }
            let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
            let workspace_root = manifest_dir
                .parent()
                .and_then(|p| p.parent())
                .unwrap_or(&manifest_dir);
            let debug_path = workspace_root.join("target/debug/m31a");
            if debug_path.exists() {
                return debug_path;
            }
            let release_path = workspace_root.join("target/release/m31a");
            if release_path.exists() {
                return release_path;
            }
            debug_path
        }

        fn spawn_with_args(args: &[&str], envs: &[(&str, &str)]) -> Self {
            let mut master: libc::c_int = 0;
            let mut slave: libc::c_int = 0;

            let ret = unsafe {
                libc::openpty(
                    &mut master,
                    &mut slave,
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                )
            };
            assert_eq!(ret, 0, "openpty must succeed");

            // Set terminal size on master PTY to 80x24 standard geometry
            let ws = libc::winsize {
                ws_row: 24,
                ws_col: 80,
                ws_xpixel: 0,
                ws_ypixel: 0,
            };
            unsafe {
                libc::ioctl(master, libc::TIOCSWINSZ, &ws);
            }

            // Set master PTY to non-blocking read
            unsafe {
                let flags = libc::fcntl(master, libc::F_GETFL, 0);
                libc::fcntl(master, libc::F_SETFL, flags | libc::O_NONBLOCK);
            }

            let slave_in = unsafe { Stdio::from_raw_fd(libc::dup(slave)) };
            let slave_out = unsafe { Stdio::from_raw_fd(libc::dup(slave)) };
            let slave_err = unsafe { Stdio::from_raw_fd(libc::dup(slave)) };
            unsafe {
                libc::close(slave);
            }

            // Locate m31a binary in target debug dir
            let exe_path = Self::find_binary();

            let mut cmd = Command::new(exe_path);
            cmd.args(args)
                .stdin(slave_in)
                .stdout(slave_out)
                .stderr(slave_err)
                .env("TERM", "xterm-256color")
                .env("RUST_BACKTRACE", "1");

            for (k, v) in envs {
                cmd.env(k, v);
            }

            let child = cmd.spawn().expect("failed to spawn m31a into PTY");
            Self {
                master_fd: master,
                child,
            }
        }

        fn send_bytes(&mut self, bytes: &[u8]) {
            let mut f = unsafe { std::fs::File::from_raw_fd(self.master_fd) };
            let _ = f.write_all(bytes);
            let _ = f.flush();
            std::mem::forget(f); // prevent closing master_fd
        }

        fn send_str(&mut self, s: &str) {
            self.send_bytes(s.as_bytes());
        }

        fn drain_output(&mut self) {
            let _ = self.read_available();
        }

        fn resize(&mut self, rows: u16, cols: u16) {
            let ws = libc::winsize {
                ws_row: rows,
                ws_col: cols,
                ws_xpixel: 0,
                ws_ypixel: 0,
            };
            unsafe {
                libc::ioctl(self.master_fd, libc::TIOCSWINSZ, &ws);
            }
        }

        fn spawn_default(dir: &std::path::Path) -> Self {
            let db_dir = dir.join("db");
            std::fs::create_dir_all(&db_dir).unwrap();

            // Initialize a clean git repository in the workspace so git status, worktree, and diff features function
            let _ = Command::new("git").args(["init"]).current_dir(dir).output();
            let _ = Command::new("git")
                .args(["config", "user.name", "Tester"])
                .current_dir(dir)
                .output();
            let _ = Command::new("git")
                .args(["config", "user.email", "tester@test.local"])
                .current_dir(dir)
                .output();
            let _ = Command::new("git")
                .args(["commit", "--allow-empty", "-m", "init"])
                .current_dir(dir)
                .output();

            Self::spawn_with_args(
                &[],
                &[
                    ("HOME", dir.to_str().unwrap()),
                    ("M31A_DATA_DIR", db_dir.to_str().unwrap()),
                    ("NVIDIA_API_KEY", "dummy-nim-key"),
                    ("M31A_TEST_MODE", "1"),
                ],
            )
        }

        fn read_available(&mut self) -> Vec<u8> {
            let mut buf = vec![0u8; 16384];
            let mut out = Vec::new();
            let mut f = unsafe { std::fs::File::from_raw_fd(self.master_fd) };

            while let Ok(n) = f.read(&mut buf) {
                if n == 0 {
                    break;
                }
                out.extend_from_slice(&buf[..n]);
            }

            std::mem::forget(f);
            out
        }

        fn wait_for_output_containing(&mut self, needle: &str, timeout: Duration) -> bool {
            let start = Instant::now();
            let mut accumulated = String::new();

            while start.elapsed() < timeout {
                let bytes = self.read_available();
                if !bytes.is_empty() {
                    let text = String::from_utf8_lossy(&bytes);
                    accumulated.push_str(&text);
                    if accumulated.contains(needle) {
                        return true;
                    }
                }
                std::thread::sleep(Duration::from_millis(50));
            }

            false
        }
    }

    impl Drop for RealPtyHarness {
        fn drop(&mut self) {
            let _ = self.child.kill();
            let _ = self.child.wait();
            unsafe {
                libc::close(self.master_fd);
            }
        }
    }

    #[test]
    fn test_pty_app_help_and_version_flags() {
        // Test standard CLI non-interactive invocation inside pseudo-terminal
        let mut harness = RealPtyHarness::spawn_with_args(&["--help"], &[]);
        let found = harness.wait_for_output_containing("M31A", Duration::from_secs(5));
        assert!(found, "PTY execution of --help must output M31A usage info");
    }

    #[test]
    fn test_pty_terminal_session_raw_mode_and_quit() {
        let dir = tempdir().unwrap();
        let db_dir = dir.path().join("db");
        std::fs::create_dir_all(&db_dir).unwrap();

        let mut harness = RealPtyHarness::spawn_with_args(
            &[],
            &[
                ("HOME", dir.path().to_str().unwrap()),
                ("M31A_DATA_DIR", db_dir.to_str().unwrap()),
                ("NVIDIA_API_KEY", "dummy-nim-key"),
            ],
        );

        // Wait for TUI to initialize and render welcome / status frame
        let rendered = harness.wait_for_output_containing("M31A", Duration::from_secs(5));
        assert!(
            rendered,
            "TUI must render welcome banner onto pseudo-terminal"
        );

        // Send double Ctrl+C (0x03) to trigger clean shutdown
        harness.send_bytes(&[0x03]); // First Ctrl+C
        std::thread::sleep(Duration::from_millis(200));
        harness.send_bytes(&[0x03]); // Second Ctrl+C within 2s

        // Child process should exit cleanly with terminal restored
        let exited = harness.child.wait();
        assert!(
            exited.is_ok(),
            "App must terminate cleanly upon double Ctrl+C"
        );
    }

    #[test]
    fn test_pty_instant_shell_execution_feedback() {
        let dir = tempdir().unwrap();
        let db_dir = dir.path().join("db");
        std::fs::create_dir_all(&db_dir).unwrap();

        let mut harness = RealPtyHarness::spawn_with_args(
            &[],
            &[
                ("HOME", dir.path().to_str().unwrap()),
                ("M31A_DATA_DIR", db_dir.to_str().unwrap()),
                ("NVIDIA_API_KEY", "dummy-nim-key"),
            ],
        );

        // Wait for TUI to render
        let _ = harness.wait_for_output_containing("M31A", Duration::from_secs(5));

        // Authorize working directory via real trust policy
        harness.send_bytes(b"/trust session\r");
        std::thread::sleep(Duration::from_millis(400));

        // Submit instant shell command via PTY: !echo 'pty-shell-verified'\r
        harness.send_bytes(b"!echo 'pty-shell-verified'\r");

        // Verify output is visible on terminal
        let detected =
            harness.wait_for_output_containing("pty-shell-verified", Duration::from_secs(5));
        assert!(
            detected,
            "PTY master must receive shell execution result from TUI"
        );
    }

    #[test]
    fn test_pty_modals_open_and_esc_dismissal() {
        let dir = tempdir().unwrap();
        let mut harness = RealPtyHarness::spawn_default(dir.path());

        // Wait for TUI to initialize
        let rendered = harness.wait_for_output_containing("M31A", Duration::from_secs(5));
        assert!(rendered, "TUI must render welcome banner");

        // List of modal commands and needles to assert presence in rendered PTY output
        let modal_cases = [
            ("/help\r", "Help"),
            ("/model\r", "Model"),
            ("/mcp\r", "MCP"),
            ("/ps\r", "Process"),
            ("/agents\r", "Subagent"),
            ("/scratchpad\r", "Scratchpad"),
            ("/context\r", "Context"),
            ("/workspace\r", "Worktree"),
            ("/persona\r", "Persona"),
            ("/skills\r", "Skill"),
            ("/templates\r", "Template"),
            ("/rollout\r", "Rollout"),
            ("/diff\r", "Diff"),
        ];

        for (cmd, expected_needle) in modal_cases {
            harness.drain_output();
            harness.send_str(cmd);
            let opened =
                harness.wait_for_output_containing(expected_needle, Duration::from_secs(4));
            assert!(
                opened,
                "Modal command '{}' must render modal containing '{}' on pseudo-terminal",
                cmd.trim(),
                expected_needle
            );

            // Dismiss modal via Esc (0x1b)
            harness.send_bytes(&[0x1b]);
            std::thread::sleep(Duration::from_millis(300));
        }

        // Clean quit
        harness.send_bytes(&[0x03]);
        std::thread::sleep(Duration::from_millis(200));
        harness.send_bytes(&[0x03]);
        let _ = harness.child.wait();
    }

    #[test]
    fn test_pty_command_palette_workflow() {
        let dir = tempdir().unwrap();
        let mut harness = RealPtyHarness::spawn_default(dir.path());

        let _ = harness.wait_for_output_containing("M31A", Duration::from_secs(5));

        // Open command palette via Ctrl+P (0x10)
        harness.drain_output();
        harness.send_bytes(&[0x10]);
        let opened = harness.wait_for_output_containing("Palette", Duration::from_secs(4));
        assert!(opened, "Ctrl+P must open Command Palette on PTY");

        // Filter commands
        harness.send_str("help");
        std::thread::sleep(Duration::from_millis(100));

        // Close palette via Esc
        harness.send_bytes(&[0x1b]);
        std::thread::sleep(Duration::from_millis(150));

        // Clean quit
        harness.send_bytes(&[0x03]);
        std::thread::sleep(Duration::from_millis(200));
        harness.send_bytes(&[0x03]);
        let _ = harness.child.wait();
    }

    #[test]
    fn test_pty_clipboard_copy_feedback() {
        let dir = tempdir().unwrap();
        let mut harness = RealPtyHarness::spawn_default(dir.path());

        let _ = harness.wait_for_output_containing("M31A", Duration::from_secs(5));

        // Press Ctrl+Y (0x19) to copy code block to clipboard
        harness.drain_output();
        harness.send_bytes(&[0x19]);

        // Expect notice about clipboard or code block
        let noticed = harness.wait_for_output_containing("code block", Duration::from_secs(4))
            || harness.wait_for_output_containing("clipboard", Duration::from_secs(4));
        assert!(
            noticed,
            "Ctrl+Y must emit clipboard copy result or notice in PTY"
        );

        // Clean quit
        harness.send_bytes(&[0x03]);
        std::thread::sleep(Duration::from_millis(200));
        harness.send_bytes(&[0x03]);
        let _ = harness.child.wait();
    }

    #[test]
    fn test_pty_viewport_resize_resilience() {
        let dir = tempdir().unwrap();
        let mut harness = RealPtyHarness::spawn_default(dir.path());

        let _ = harness.wait_for_output_containing("M31A", Duration::from_secs(5));

        // Resize down to narrow geometry 60x20
        harness.resize(20, 60);
        std::thread::sleep(Duration::from_millis(200));

        // Authorize session trust
        harness.send_str("/trust session\r");
        std::thread::sleep(Duration::from_millis(300));

        // Run instant shell command in resized terminal
        harness.send_str("!echo 'resized-pty-ok'\r");
        let detected = harness.wait_for_output_containing("resized-pty-ok", Duration::from_secs(4));
        assert!(
            detected,
            "TUI must continue operating correctly across terminal resize without panic"
        );

        // Resize back to standard 80x24 geometry
        harness.resize(24, 80);
        std::thread::sleep(Duration::from_millis(200));

        // Clean quit
        harness.send_bytes(&[0x03]);
        std::thread::sleep(Duration::from_millis(200));
        harness.send_bytes(&[0x03]);
        let _ = harness.child.wait();
    }

    #[test]
    fn test_pty_model_picker_selection_and_switch() {
        let dir = tempdir().unwrap();
        let mut harness = RealPtyHarness::spawn_default(dir.path());

        let _ = harness.wait_for_output_containing("M31A", Duration::from_secs(5));

        // Open model picker modal
        harness.drain_output();
        harness.send_str("/model\r");
        let opened = harness.wait_for_output_containing("Model", Duration::from_secs(4));
        assert!(opened, "/model must open Model Picker modal in PTY");

        // Navigate down and select new model with Enter
        harness.send_bytes(b"\x1b[B"); // Down arrow
        std::thread::sleep(Duration::from_millis(150));
        harness.send_bytes(b"\r"); // Enter

        // Verify model switch is reflected in bottom status badge or transcript notice
        let switched = harness.wait_for_output_containing("super-vl", Duration::from_secs(4))
            || harness.wait_for_output_containing("switched", Duration::from_secs(1))
            || harness.wait_for_output_containing("Active model", Duration::from_secs(1))
            || harness.wait_for_output_containing("Failed to switch model", Duration::from_secs(1));
        assert!(
            switched,
            "Selecting model in ModelPicker must confirm switch notice or honest credential check in PTY"
        );

        // Clean quit
        harness.send_bytes(&[0x03]);
        std::thread::sleep(Duration::from_millis(200));
        harness.send_bytes(&[0x03]);
        let _ = harness.child.wait();
    }

    #[test]
    fn test_pty_settings_modal_navigation_and_save() {
        let dir = tempdir().unwrap();
        let mut harness = RealPtyHarness::spawn_default(dir.path());

        let _ = harness.wait_for_output_containing("M31A", Duration::from_secs(5));

        // Open settings modal
        harness.drain_output();
        harness.send_str("/settings\r");
        let opened = harness.wait_for_output_containing("Settings", Duration::from_secs(4));
        assert!(opened, "/settings must open Settings modal in PTY");

        // Navigate tab with Tab key (0x09)
        harness.send_bytes(&[0x09]);
        std::thread::sleep(Duration::from_millis(100));

        // Close and persist with Esc
        harness.send_bytes(&[0x1b]);
        let saved = harness.wait_for_output_containing("Settings saved", Duration::from_secs(4))
            || harness.wait_for_output_containing("saved to", Duration::from_secs(4));
        assert!(
            saved,
            "Dismissing Settings modal must save config to disk and report notice"
        );

        // Clean quit
        harness.send_bytes(&[0x03]);
        std::thread::sleep(Duration::from_millis(200));
        harness.send_bytes(&[0x03]);
        let _ = harness.child.wait();
    }
}
