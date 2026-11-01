//! Pseudo-terminal (PTY) runner and process controller.

use anyhow::{Context, Result, anyhow};
use m31a_core::SandboxPolicy;
use std::ffi::{CStr, CString};
use std::io;
use std::os::fd::RawFd;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

static CMD_COUNTER: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PtySize {
    pub rows: u16,
    pub cols: u16,
}

impl Default for PtySize {
    fn default() -> Self {
        Self { rows: 24, cols: 80 }
    }
}

pub struct PtySession {
    pub master_fd: RawFd,
    pub slave_fd: Option<RawFd>,
    pub slave_path: PathBuf,
}

impl PtySession {
    pub fn open(size: PtySize) -> Result<Self> {
        let master_fd =
            unsafe { libc::posix_openpt(libc::O_RDWR | libc::O_NOCTTY | libc::O_CLOEXEC) };
        if master_fd < 0 {
            return Err(io::Error::last_os_error()).context("posix_openpt failed");
        }

        if unsafe { libc::grantpt(master_fd) } != 0 {
            let err = io::Error::last_os_error();
            unsafe { libc::close(master_fd) };
            return Err(err).context("grantpt failed");
        }

        if unsafe { libc::unlockpt(master_fd) } != 0 {
            let err = io::Error::last_os_error();
            unsafe { libc::close(master_fd) };
            return Err(err).context("unlockpt failed");
        }

        let mut pts_buf = [0i8; 256];
        if unsafe { libc::ptsname_r(master_fd, pts_buf.as_mut_ptr(), pts_buf.len()) } != 0 {
            let err = io::Error::last_os_error();
            unsafe { libc::close(master_fd) };
            return Err(err).context("ptsname_r failed");
        }

        let slave_path_cstr = unsafe { CStr::from_ptr(pts_buf.as_ptr()) };
        let slave_path_str = slave_path_cstr
            .to_str()
            .map_err(|e| anyhow!("Invalid UTF-8: {}", e))?;
        let slave_path = PathBuf::from(slave_path_str);

        let slave_path_c = CString::new(slave_path_str)?;
        let slave_fd = unsafe {
            libc::open(
                slave_path_c.as_ptr(),
                libc::O_RDWR | libc::O_NOCTTY | libc::O_CLOEXEC,
            )
        };
        if slave_fd < 0 {
            let err = io::Error::last_os_error();
            unsafe { libc::close(master_fd) };
            return Err(err).context(format!("Failed to open slave at {:?}", slave_path));
        }

        let session = Self {
            master_fd,
            slave_fd: Some(slave_fd),
            slave_path,
        };

        session.set_size(size)?;
        Ok(session)
    }

    pub fn set_size(&self, size: PtySize) -> Result<()> {
        let ws = libc::winsize {
            ws_row: size.rows,
            ws_col: size.cols,
            ws_xpixel: 0,
            ws_ypixel: 0,
        };
        let ret = unsafe { libc::ioctl(self.master_fd, libc::TIOCSWINSZ, &ws) };
        if ret != 0 {
            return Err(io::Error::last_os_error()).context("TIOCSWINSZ failed");
        }
        Ok(())
    }

    pub fn close_slave(&mut self) {
        if let Some(fd) = self.slave_fd.take() {
            unsafe { libc::close(fd) };
        }
    }
}

impl Drop for PtySession {
    fn drop(&mut self) {
        self.close_slave();
        if self.master_fd >= 0 {
            unsafe { libc::close(self.master_fd) };
            self.master_fd = -1;
        }
    }
}

#[derive(Debug)]
pub struct PtyExecutionOutput {
    pub stdout: String,
    pub exit_code: i32,
    pub timed_out: bool,
}

#[derive(Debug)]
pub struct PersistentCommandOutput {
    pub stdout: String,
    pub exit_code: i32,
    pub timed_out: bool,
    pub cwd: Option<PathBuf>,
}

/// A long-lived persistent shell process attached to a PTY master file descriptor.
/// The child shell remains active across turns, preserving environment variables,
/// background tasks, aliases, and working directory in process memory.
pub struct PersistentPtyProcess {
    master_fd: RawFd,
    child_pid: libc::pid_t,
    is_alive: bool,
}

impl PersistentPtyProcess {
    /// Spawns a long-lived interactive shell inside the PTY, optionally isolated with Bubblewrap.
    pub fn spawn(
        cwd: &Path,
        env_vars: &[(String, String)],
        sandbox_policy: Option<SandboxPolicy>,
    ) -> Result<Self> {
        let mut pty = PtySession::open(PtySize::default())?;
        let slave_fd = pty
            .slave_fd
            .take()
            .ok_or_else(|| anyhow!("Missing slave fd"))?;

        // Disable ECHO on slave PTY so written commands are not duplicated in stdout stream
        unsafe {
            let mut termios: libc::termios = std::mem::zeroed();
            if libc::tcgetattr(slave_fd, &mut termios) == 0 {
                termios.c_lflag &= !(libc::ECHO | libc::ECHOE | libc::ECHOK | libc::ECHONL);
                libc::tcsetattr(slave_fd, libc::TCSANOW, &termios);
            }
        }

        let cwd_buf = cwd.to_path_buf();
        let env_list = env_vars.to_vec();

        let use_bwrap = match sandbox_policy {
            Some(policy) if policy != SandboxPolicy::DangerFullAccess => {
                which::which("bwrap").is_ok()
            }
            _ => false,
        };

        let pid = unsafe { libc::fork() };
        if pid < 0 {
            unsafe { libc::close(slave_fd) };
            return Err(io::Error::last_os_error()).context("fork failed for persistent shell");
        }

        if pid == 0 {
            // Child process setup
            unsafe {
                libc::close(pty.master_fd);
                libc::setsid();

                #[cfg(target_os = "linux")]
                libc::ioctl(slave_fd, libc::TIOCSCTTY, 0);

                libc::dup2(slave_fd, libc::STDIN_FILENO);
                libc::dup2(slave_fd, libc::STDOUT_FILENO);
                libc::dup2(slave_fd, libc::STDERR_FILENO);
                libc::close(slave_fd);

                let cwd_str = cwd_buf.to_string_lossy().to_string();
                let cwd_c = CString::new(cwd_str.as_str()).unwrap_or_default();
                libc::chdir(cwd_c.as_ptr());

                for (k, v) in env_list {
                    let k_c = CString::new(k.as_str()).unwrap_or_default();
                    let v_c = CString::new(v.as_str()).unwrap_or_default();
                    libc::setenv(k_c.as_ptr(), v_c.as_ptr(), 1);
                }
                libc::setenv(c"TERM".as_ptr(), c"xterm-256color".as_ptr(), 1);
                libc::setenv(c"CLICOLOR_FORCE".as_ptr(), c"1".as_ptr(), 1);
                libc::setenv(c"PS1".as_ptr(), c"".as_ptr(), 1);
                libc::setenv(c"PROMPT_COMMAND".as_ptr(), c"".as_ptr(), 1);

                if use_bwrap {
                    // Exec bwrap wrapper around /bin/sh
                    let bwrap = c"bwrap";
                    let ro = c"--ro-bind";
                    let root = c"/";
                    let dev = c"--dev";
                    let dev_p = c"/dev";
                    let proc = c"--proc";
                    let proc_p = c"/proc";
                    let tmpfs = c"--tmpfs";
                    let tmp_p = c"/tmp";
                    let bind = c"--bind";
                    let unshare_pid = c"--unshare-pid";
                    let unshare_ipc = c"--unshare-ipc";
                    let unshare_uts = c"--unshare-uts";
                    let die = c"--die-with-parent";
                    let chdir = c"--chdir";
                    let d_dash = c"--";
                    let shell = c"/bin/sh";

                    let mut args = vec![
                        bwrap.as_ptr(),
                        ro.as_ptr(),
                        root.as_ptr(),
                        root.as_ptr(),
                        dev.as_ptr(),
                        dev_p.as_ptr(),
                        proc.as_ptr(),
                        proc_p.as_ptr(),
                        tmpfs.as_ptr(),
                        tmp_p.as_ptr(),
                        bind.as_ptr(),
                        cwd_c.as_ptr(),
                        cwd_c.as_ptr(),
                        unshare_pid.as_ptr(),
                        unshare_ipc.as_ptr(),
                        unshare_uts.as_ptr(),
                        die.as_ptr(),
                        chdir.as_ptr(),
                        cwd_c.as_ptr(),
                        d_dash.as_ptr(),
                        shell.as_ptr(),
                        std::ptr::null(),
                    ];

                    libc::execvp(bwrap.as_ptr(), args.as_mut_ptr());
                }

                // Fallback: direct /bin/sh
                let shell = c"/bin/sh";
                let args = [shell.as_ptr(), std::ptr::null()];
                libc::execvp(shell.as_ptr(), args.as_ptr());
                libc::_exit(127);
            }
        }

        unsafe { libc::close(slave_fd) };

        let master_fd = pty.master_fd;
        // Don't close master_fd on drop of pty session since it belongs to PersistentPtyProcess now
        pty.master_fd = -1;

        // Set non-blocking on master_fd
        unsafe {
            let flags = libc::fcntl(master_fd, libc::F_GETFL, 0);
            libc::fcntl(master_fd, libc::F_SETFL, flags | libc::O_NONBLOCK);
        }

        Ok(Self {
            master_fd,
            child_pid: pid,
            is_alive: true,
        })
    }

    pub fn is_alive(&mut self) -> bool {
        if !self.is_alive {
            return false;
        }
        let mut status: libc::c_int = 0;
        let ret = unsafe { libc::waitpid(self.child_pid, &mut status, libc::WNOHANG) };
        if ret == self.child_pid || ret < 0 {
            self.is_alive = false;
        }
        self.is_alive
    }

    /// Executes a command within the running shell and reads until the sentinel marker is observed.
    pub async fn execute(
        &mut self,
        cmd: &str,
        timeout: Duration,
    ) -> Result<PersistentCommandOutput> {
        if !self.is_alive() {
            return Err(anyhow!("Persistent shell process is dead"));
        }

        let req_id = CMD_COUNTER.fetch_add(1, Ordering::SeqCst);
        let sentinel = format!("__M31A_DONE_{}_{}__", std::process::id(), req_id);

        // Command payload: execute cmd, save exit code, emit sentinel with exit code and cwd
        let payload = format!("{cmd}\n__m31a_ec=$?\necho \"{sentinel} $__m31a_ec $(pwd)\"\n");

        // Write command into PTY master
        let bytes = payload.as_bytes();
        let mut written = 0;
        let write_start = tokio::time::Instant::now();

        while written < bytes.len() {
            if write_start.elapsed() > Duration::from_secs(5) {
                return Err(anyhow!("Timed out writing command to persistent shell PTY"));
            }
            let ret = unsafe {
                libc::write(
                    self.master_fd,
                    bytes[written..].as_ptr() as *const libc::c_void,
                    bytes.len() - written,
                )
            };
            if ret > 0 {
                written += ret as usize;
            } else if ret < 0 {
                let err = io::Error::last_os_error();
                if err.kind() != io::ErrorKind::WouldBlock {
                    return Err(err).context("Write to PTY failed");
                }
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        }

        // Read output loop until sentinel and cwd are observed
        let read_start = tokio::time::Instant::now();
        let mut output_bytes = Vec::new();
        let sentinel_tag = format!("{sentinel} ");

        loop {
            // Check overall command timeout
            if read_start.elapsed() > timeout {
                // Send Ctrl-C to interrupt command without terminating the shell
                unsafe {
                    let intr = [3u8]; // ASCII 0x03 (ETX / Ctrl-C)
                    libc::write(self.master_fd, intr.as_ptr() as *const libc::c_void, 1);
                }
                tokio::time::sleep(Duration::from_millis(50)).await;

                let text = String::from_utf8_lossy(&output_bytes)
                    .replace("\r\n", "\n")
                    .to_string();
                return Ok(PersistentCommandOutput {
                    stdout: text,
                    exit_code: 130, // Standard SIGINT exit code
                    timed_out: true,
                    cwd: None,
                });
            }

            // Read available chunk
            let mut buf = [0u8; 4096];
            let read_ret = unsafe {
                libc::read(
                    self.master_fd,
                    buf.as_mut_ptr() as *mut libc::c_void,
                    buf.len(),
                )
            };

            if read_ret > 0 {
                output_bytes.extend_from_slice(&buf[..read_ret as usize]);
                let curr_text = String::from_utf8_lossy(&output_bytes);

                // Check if sentinel is present
                if let Some(pos) = curr_text.find(&sentinel_tag) {
                    let after_sentinel = &curr_text[pos + sentinel_tag.len()..];
                    // Look for newline terminating sentinel line
                    if let Some(nl) = after_sentinel.find('\n') {
                        let line = after_sentinel[..nl].trim();
                        let (ec_str, pwd_str) = line.split_once(' ').unwrap_or((line, ""));
                        let exit_code = ec_str.parse::<i32>().unwrap_or(0);
                        let new_cwd = if !pwd_str.trim().is_empty() {
                            Some(PathBuf::from(pwd_str.trim()))
                        } else {
                            None
                        };

                        let clean_stdout = curr_text[..pos]
                            .replace("\r\n", "\n")
                            .trim_end()
                            .to_string();

                        return Ok(PersistentCommandOutput {
                            stdout: clean_stdout,
                            exit_code,
                            timed_out: false,
                            cwd: new_cwd,
                        });
                    }
                }
            } else {
                // Check if child unexpectedly exited
                let mut status: libc::c_int = 0;
                let wait_ret = unsafe { libc::waitpid(self.child_pid, &mut status, libc::WNOHANG) };
                if wait_ret == self.child_pid {
                    self.is_alive = false;
                    let mut exit_code = 0;
                    if libc::WIFEXITED(status) {
                        exit_code = libc::WEXITSTATUS(status);
                    } else if libc::WIFSIGNALED(status) {
                        exit_code = 128 + libc::WTERMSIG(status);
                    }
                    let clean_stdout = String::from_utf8_lossy(&output_bytes)
                        .replace("\r\n", "\n")
                        .trim_end()
                        .to_string();
                    return Ok(PersistentCommandOutput {
                        stdout: clean_stdout,
                        exit_code,
                        timed_out: false,
                        cwd: None,
                    });
                }
                tokio::time::sleep(Duration::from_millis(15)).await;
            }
        }
    }

    pub fn terminate(&mut self) {
        if self.is_alive && self.child_pid > 0 {
            unsafe {
                libc::kill(self.child_pid, libc::SIGTERM);
            }
            let mut status = 0;
            for _ in 0..10 {
                let ret = unsafe { libc::waitpid(self.child_pid, &mut status, libc::WNOHANG) };
                if ret == self.child_pid || ret < 0 {
                    break;
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            unsafe {
                libc::kill(self.child_pid, libc::SIGKILL);
                libc::waitpid(self.child_pid, &mut status, libc::WNOHANG);
            }
            self.is_alive = false;
        }
        if self.master_fd >= 0 {
            unsafe { libc::close(self.master_fd) };
            self.master_fd = -1;
        }
    }
}

impl Drop for PersistentPtyProcess {
    fn drop(&mut self) {
        self.terminate();
    }
}

pub struct PtyRunner;

impl PtyRunner {
    /// Executes a one-off command in a PTY with environment and timeout.
    pub async fn run_command(
        cmd: &str,
        cwd: &Path,
        env_vars: &[(String, String)],
        timeout: Duration,
    ) -> Result<PtyExecutionOutput> {
        let mut session = PersistentPtyProcess::spawn(cwd, env_vars, None)?;
        let out = session.execute(cmd, timeout).await?;
        Ok(PtyExecutionOutput {
            stdout: out.stdout,
            exit_code: out.exit_code,
            timed_out: out.timed_out,
        })
    }
}
