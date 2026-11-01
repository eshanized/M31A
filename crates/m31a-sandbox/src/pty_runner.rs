//! Unix Pseudo-Terminal (PTY) runner and process controller.
//!
//! Allocates a master/slave pseudo-terminal pair, initializes terminal window dimensions
//! (`winsize`), and spawns child commands with dedicated controlling terminal sessions,
//! enabling interactive CLI commands (curses, vim, sudo prompts, pagers) to execute without
//! "stdin/stdout is not a tty" errors.

use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::ffi::{CStr, CString};
use std::io;
use std::os::fd::RawFd;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// Terminal window dimensions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PtySize {
    pub rows: u16,
    pub cols: u16,
    pub xpixel: u16,
    pub ypixel: u16,
}

impl Default for PtySize {
    fn default() -> Self {
        Self {
            rows: 24,
            cols: 80,
            xpixel: 0,
            ypixel: 0,
        }
    }
}

impl PtySize {
    pub fn new(rows: u16, cols: u16) -> Self {
        Self {
            rows,
            cols,
            xpixel: 0,
            ypixel: 0,
        }
    }

    fn to_libc_winsize(self) -> libc::winsize {
        libc::winsize {
            ws_row: self.rows,
            ws_col: self.cols,
            ws_xpixel: self.xpixel,
            ws_ypixel: self.ypixel,
        }
    }
}

/// Allocated master/slave pseudo-terminal pair.
#[derive(Debug)]
pub struct PtySession {
    pub master_fd: RawFd,
    pub slave_fd: Option<RawFd>,
    pub slave_path: PathBuf,
}

impl PtySession {
    /// Allocate a new PTY pair using standard POSIX APIs (`posix_openpt`, `grantpt`, `unlockpt`, `ptsname_r`).
    pub fn open(size: PtySize) -> Result<Self> {
        let master_fd =
            unsafe { libc::posix_openpt(libc::O_RDWR | libc::O_NOCTTY | libc::O_CLOEXEC) };
        if master_fd < 0 {
            return Err(io::Error::last_os_error())
                .context("Failed to open master pseudo-terminal (posix_openpt)");
        }

        if unsafe { libc::grantpt(master_fd) } != 0 {
            let err = io::Error::last_os_error();
            unsafe { libc::close(master_fd) };
            return Err(err).context("Failed to grantpt on master pseudo-terminal");
        }

        if unsafe { libc::unlockpt(master_fd) } != 0 {
            let err = io::Error::last_os_error();
            unsafe { libc::close(master_fd) };
            return Err(err).context("Failed to unlockpt on master pseudo-terminal");
        }

        let mut pts_buf = [0i8; 256];
        if unsafe { libc::ptsname_r(master_fd, pts_buf.as_mut_ptr(), pts_buf.len()) } != 0 {
            let err = io::Error::last_os_error();
            unsafe { libc::close(master_fd) };
            return Err(err).context("Failed to query ptsname_r on master pseudo-terminal");
        }

        let slave_path_cstr = unsafe { CStr::from_ptr(pts_buf.as_ptr()) };
        let slave_path_str = slave_path_cstr
            .to_str()
            .map_err(|e| anyhow!("Invalid UTF-8 in slave pty path: {}", e))?;
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
            return Err(err).context(format!(
                "Failed to open slave pseudo-terminal at {:?}",
                slave_path
            ));
        }

        let session = Self {
            master_fd,
            slave_fd: Some(slave_fd),
            slave_path,
        };

        session.set_size(size)?;
        Ok(session)
    }

    /// Resize the pseudo-terminal window via `TIOCSWINSZ` ioctl.
    pub fn set_size(&self, size: PtySize) -> Result<()> {
        let ws = size.to_libc_winsize();
        let ret = unsafe { libc::ioctl(self.master_fd, libc::TIOCSWINSZ, &ws) };
        if ret != 0 {
            return Err(io::Error::last_os_error())
                .context("Failed to set PTY window size (TIOCSWINSZ)");
        }
        Ok(())
    }

    /// Close slave descriptor in parent process after child has inherited it.
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

/// Handle to a child process executing inside an allocated pseudo-terminal.
pub struct PtyProcessHandle {
    pub pid: libc::pid_t,
    master_fd: RawFd,
    has_exited: Arc<AtomicBool>,
}

impl PtyProcessHandle {
    pub fn master_raw_fd(&self) -> RawFd {
        self.master_fd
    }

    /// Write input bytes (e.g. keyboard keystrokes, answers to interactive prompts) to the PTY.
    pub fn write_all(&self, data: &[u8]) -> Result<()> {
        let mut total_written = 0;
        while total_written < data.len() {
            let ret = unsafe {
                libc::write(
                    self.master_fd,
                    data[total_written..].as_ptr() as *const libc::c_void,
                    data.len() - total_written,
                )
            };
            if ret < 0 {
                let err = io::Error::last_os_error();
                if err.kind() == io::ErrorKind::Interrupted {
                    continue;
                }
                return Err(err).context("Failed writing to PTY master descriptor");
            }
            total_written += ret as usize;
        }
        Ok(())
    }

    /// Non-blocking read of available output from the PTY.
    pub fn read_nonblocking(&self, buf: &mut [u8]) -> Result<usize> {
        // Set master nonblocking temporarily or check poll/select
        let mut pollfd = libc::pollfd {
            fd: self.master_fd,
            events: libc::POLLIN,
            revents: 0,
        };
        let poll_ret = unsafe { libc::poll(&mut pollfd, 1, 0) };
        if poll_ret <= 0 {
            return Ok(0);
        }

        let ret = unsafe {
            libc::read(
                self.master_fd,
                buf.as_mut_ptr() as *mut libc::c_void,
                buf.len(),
            )
        };
        if ret < 0 {
            let err = io::Error::last_os_error();
            if err.kind() == io::ErrorKind::WouldBlock
                || err.kind() == io::ErrorKind::Interrupted
                || err.raw_os_error() == Some(libc::EIO)
            {
                return Ok(0);
            }
            return Err(err).context("Failed reading from PTY master descriptor");
        }
        Ok(ret as usize)
    }

    /// Drain all currently buffered terminal output.
    pub fn drain_available_output(&self) -> Result<Vec<u8>> {
        let mut output = Vec::new();
        let mut chunk = [0u8; 4096];
        loop {
            let read_bytes = self.read_nonblocking(&mut chunk)?;
            if read_bytes == 0 {
                break;
            }
            output.extend_from_slice(&chunk[..read_bytes]);
        }
        Ok(output)
    }

    /// Check if child process has exited without blocking. Returns Some(exit_code) if terminated.
    pub fn try_wait(&self) -> Result<Option<i32>> {
        if self.has_exited.load(Ordering::SeqCst) {
            return Ok(Some(0));
        }

        let mut status: libc::c_int = 0;
        let ret = unsafe { libc::waitpid(self.pid, &mut status, libc::WNOHANG) };
        if ret == 0 {
            Ok(None)
        } else if ret == self.pid {
            self.has_exited.store(true, Ordering::SeqCst);
            if libc::WIFEXITED(status) {
                Ok(Some(libc::WEXITSTATUS(status)))
            } else if libc::WIFSIGNALED(status) {
                Ok(Some(128 + libc::WTERMSIG(status)))
            } else {
                Ok(Some(status))
            }
        } else {
            let err = io::Error::last_os_error();
            if err.raw_os_error() == Some(libc::ECHILD) {
                self.has_exited.store(true, Ordering::SeqCst);
                return Ok(Some(0));
            }
            Err(err).context("Error during try_wait waitpid")
        }
    }

    /// Block waiting for child process to terminate.
    pub fn wait(&self) -> Result<i32> {
        let mut status: libc::c_int = 0;
        let ret = unsafe { libc::waitpid(self.pid, &mut status, 0) };
        if ret == self.pid {
            self.has_exited.store(true, Ordering::SeqCst);
            if libc::WIFEXITED(status) {
                Ok(libc::WEXITSTATUS(status))
            } else if libc::WIFSIGNALED(status) {
                Ok(128 + libc::WTERMSIG(status))
            } else {
                Ok(status)
            }
        } else {
            let err = io::Error::last_os_error();
            if err.raw_os_error() == Some(libc::ECHILD) {
                self.has_exited.store(true, Ordering::SeqCst);
                return Ok(0);
            }
            Err(err).context("Error during waitpid")
        }
    }

    /// Send a POSIX signal (e.g. SIGTERM, SIGKILL, SIGINT) to the child process.
    pub fn send_signal(&self, signal: i32) -> Result<()> {
        let ret = unsafe { libc::kill(self.pid, signal) };
        if ret != 0 {
            let err = io::Error::last_os_error();
            if err.raw_os_error() != Some(libc::ESRCH) {
                return Err(err).context("Failed sending signal to PTY child process");
            }
        }
        Ok(())
    }

    /// Resize terminal window.
    pub fn resize(&self, size: PtySize) -> Result<()> {
        let ws = size.to_libc_winsize();
        let ret = unsafe { libc::ioctl(self.master_fd, libc::TIOCSWINSZ, &ws) };
        if ret != 0 {
            return Err(io::Error::last_os_error()).context("Failed to resize PTY window");
        }
        Ok(())
    }
}

impl Drop for PtyProcessHandle {
    fn drop(&mut self) {
        if !self.has_exited.load(Ordering::SeqCst) {
            let _ = self.send_signal(libc::SIGTERM);
            // Brief pause then force kill if still running
            std::thread::sleep(std::time::Duration::from_millis(50));
            if let Ok(None) = self.try_wait() {
                let _ = self.send_signal(libc::SIGKILL);
                let _ = self.wait();
            }
        }
        if self.master_fd >= 0 {
            unsafe { libc::close(self.master_fd) };
            self.master_fd = -1;
        }
    }
}

/// Spawns a command in an allocated Unix pseudo-terminal (PTY).
pub fn spawn_command_in_pty(
    program: &str,
    args: &[&str],
    envs: &HashMap<String, String>,
    cwd: Option<&Path>,
    size: PtySize,
) -> Result<PtyProcessHandle> {
    let mut session = PtySession::open(size)?;
    let slave_fd = session
        .slave_fd
        .ok_or_else(|| anyhow!("Slave descriptor not available"))?;

    let prog_c = CString::new(program)?;
    let mut argv_c = Vec::new();
    argv_c.push(prog_c.clone());
    for arg in args {
        argv_c.push(CString::new(*arg)?);
    }
    let mut argv_ptrs: Vec<*const libc::c_char> = argv_c.iter().map(|s| s.as_ptr()).collect();
    argv_ptrs.push(std::ptr::null());

    let cwd_c = match cwd {
        Some(p) => Some(CString::new(p.to_string_lossy().as_ref())?),
        None => None,
    };

    let mut env_c = Vec::new();
    for (k, v) in envs {
        env_c.push(CString::new(format!("{}={}", k, v))?);
    }
    // Set TERM if not already present
    if !envs.contains_key("TERM") {
        env_c.push(CString::new("TERM=xterm-256color")?);
    }
    let mut env_ptrs: Vec<*const libc::c_char> = env_c.iter().map(|s| s.as_ptr()).collect();
    env_ptrs.push(std::ptr::null());

    let pid = unsafe { libc::fork() };
    if pid < 0 {
        return Err(io::Error::last_os_error()).context("Failed to fork PTY child process");
    }

    if pid == 0 {
        // --- CHILD PROCESS ---
        unsafe {
            // Establish new session and process group
            if libc::setsid() < 0 {
                libc::_exit(101);
            }

            // Set slave as controlling terminal
            if libc::ioctl(slave_fd, libc::TIOCSCTTY, 0) < 0 {
                // TIOCSCTTY failure is non-fatal on some kernels, proceed
            }

            // Duplicate slave descriptor to standard I/O
            libc::dup2(slave_fd, libc::STDIN_FILENO);
            libc::dup2(slave_fd, libc::STDOUT_FILENO);
            libc::dup2(slave_fd, libc::STDERR_FILENO);

            // Close descriptors above STDERR_FILENO
            let max_fd = libc::sysconf(libc::_SC_OPEN_MAX).max(1024) as i32;
            for fd in 3..max_fd {
                libc::close(fd);
            }

            // Change directory if requested
            if let Some(ref dir) = cwd_c
                && libc::chdir(dir.as_ptr()) != 0
            {
                libc::_exit(102);
            }

            // Execute target binary
            libc::execvpe(prog_c.as_ptr(), argv_ptrs.as_ptr(), env_ptrs.as_ptr());

            // If exec fails:
            libc::_exit(127);
        }
    }

    // --- PARENT PROCESS ---
    session.close_slave();

    // Take master fd from session so PtySession::drop doesn't close it
    let master_fd = session.master_fd;
    session.master_fd = -1;

    Ok(PtyProcessHandle {
        pid,
        master_fd,
        has_exited: Arc::new(AtomicBool::new(false)),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pty_session_open_and_resize() {
        let size = PtySize::new(30, 100);
        let session = PtySession::open(size).expect("Failed to open PTY session");
        assert!(session.master_fd >= 0);
        assert!(session.slave_fd.is_some());
        assert!(session.slave_path.exists());

        session
            .set_size(PtySize::new(40, 120))
            .expect("Failed to resize PTY");
    }

    #[test]
    fn test_spawn_command_in_pty_echo() {
        let envs = HashMap::new();
        let handle = spawn_command_in_pty(
            "echo",
            &["m31a-pty-verification-test"],
            &envs,
            None,
            PtySize::default(),
        )
        .expect("Failed to spawn echo in PTY");

        let status = handle.wait().expect("Failed to wait on echo child");
        assert_eq!(status, 0);

        let output = handle
            .drain_available_output()
            .expect("Failed to drain output");
        let output_str = String::from_utf8_lossy(&output);
        assert!(
            output_str.contains("m31a-pty-verification-test"),
            "Expected echo output in PTY, got: {}",
            output_str
        );
    }

    #[test]
    fn test_spawn_command_in_pty_stdin_write() {
        let envs = HashMap::new();
        let handle = spawn_command_in_pty("cat", &[], &envs, None, PtySize::default())
            .expect("Failed to spawn cat in PTY");

        handle
            .write_all(b"sovereign coder\n")
            .expect("Failed to write to cat PTY");
        std::thread::sleep(std::time::Duration::from_millis(50));

        let output = handle
            .drain_available_output()
            .expect("Failed to drain output");
        let output_str = String::from_utf8_lossy(&output);
        assert!(
            output_str.contains("sovereign coder"),
            "Expected echoed cat output in PTY, got: {}",
            output_str
        );

        // Send EOF (Ctrl+D = 0x04)
        handle.write_all(&[4]).expect("Failed to send EOF");
        let status = handle.wait().expect("Failed to wait on cat child");
        assert_eq!(status, 0);
    }
}
