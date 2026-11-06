//! OS-level sleep and idle power-management inhibitor.
//!
//! Prevents the host machine from sleeping, suspending, or entering idle power-saving
//! during autonomous swarm runs, long compilation jobs, or continuous coding sessions.

use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use tracing::{debug, warn};

const DEFAULT_APP_ID: &str = "m31a";
const DEFAULT_REASON: &str = "M31A active autonomous execution session";
const BLOCKER_SLEEP_SECONDS: &str = "2147483647";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SleepInhibitorBackend {
    SystemdInhibit,
    GnomeSessionInhibit,
    None,
}

#[derive(Debug)]
enum InhibitorState {
    Inactive,
    Active {
        backend: SleepInhibitorBackend,
        child: Child,
    },
}

/// Linux and cross-platform sleep inhibitor.
pub struct SleepInhibitor {
    app_id: String,
    state: InhibitorState,
    preferred_backend: Option<SleepInhibitorBackend>,
    missing_backend_logged: bool,
}

impl Default for SleepInhibitor {
    fn default() -> Self {
        Self {
            app_id: DEFAULT_APP_ID.to_string(),
            state: InhibitorState::Inactive,
            preferred_backend: None,
            missing_backend_logged: false,
        }
    }
}

impl SleepInhibitor {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_app_id(app_id: impl Into<String>) -> Self {
        Self {
            app_id: app_id.into(),
            state: InhibitorState::Inactive,
            preferred_backend: None,
            missing_backend_logged: false,
        }
    }

    /// Returns true if an active inhibitor process is currently running.
    pub fn is_active(&mut self) -> bool {
        if let InhibitorState::Active { ref mut child, .. } = self.state {
            match child.try_wait() {
                Ok(None) => true,
                Ok(Some(_)) => {
                    self.state = InhibitorState::Inactive;
                    false
                }
                Err(_) => {
                    self.state = InhibitorState::Inactive;
                    false
                }
            }
        } else {
            false
        }
    }

    /// Return the active backend if currently holding an inhibition assertion.
    pub fn active_backend(&self) -> Option<SleepInhibitorBackend> {
        if let InhibitorState::Active { backend, .. } = self.state {
            Some(backend)
        } else {
            None
        }
    }

    /// Acquire sleep inhibition. If already active, keeps existing assertion.
    pub fn acquire(&mut self, reason: Option<&str>) {
        if self.is_active() {
            return;
        }

        let why = reason.unwrap_or(DEFAULT_REASON);
        let backends = match self.preferred_backend {
            Some(b @ SleepInhibitorBackend::SystemdInhibit) => {
                [b, SleepInhibitorBackend::GnomeSessionInhibit]
            }
            Some(b @ SleepInhibitorBackend::GnomeSessionInhibit) => {
                [b, SleepInhibitorBackend::SystemdInhibit]
            }
            _ => [
                SleepInhibitorBackend::SystemdInhibit,
                SleepInhibitorBackend::GnomeSessionInhibit,
            ],
        };

        for backend in backends {
            match self.spawn_backend(backend, why) {
                Ok(mut child) => match child.try_wait() {
                    Ok(None) => {
                        debug!(?backend, "Sleep inhibitor acquired successfully");
                        self.state = InhibitorState::Active { backend, child };
                        self.preferred_backend = Some(backend);
                        self.missing_backend_logged = false;
                        return;
                    }
                    Ok(Some(status)) => {
                        if !self.missing_backend_logged {
                            warn!(
                                ?backend,
                                ?status,
                                "Sleep inhibitor backend exited immediately"
                            );
                        }
                    }
                    Err(e) => {
                        if !self.missing_backend_logged {
                            warn!(?backend, error = %e, "Failed to query sleep inhibitor backend");
                        }
                        let _ = child.kill();
                        let _ = child.wait();
                    }
                },
                Err(err) => {
                    if err.kind() != std::io::ErrorKind::NotFound && !self.missing_backend_logged {
                        debug!(?backend, error = %err, "Sleep inhibitor backend unavailable");
                    }
                }
            }
        }

        if !self.missing_backend_logged {
            debug!(
                "No host sleep inhibitor backend available (systemd-inhibit / gnome-session-inhibit)"
            );
            self.missing_backend_logged = true;
        }
    }

    /// Release active inhibition, terminating child helper process.
    pub fn release(&mut self) {
        if let InhibitorState::Active { backend, mut child } =
            std::mem::replace(&mut self.state, InhibitorState::Inactive)
        {
            let _ = child.kill();
            let _ = child.wait();
            debug!(?backend, "Sleep inhibitor released");
        }
    }

    fn spawn_backend(&self, backend: SleepInhibitorBackend, why: &str) -> std::io::Result<Child> {
        let mut cmd = match backend {
            SleepInhibitorBackend::SystemdInhibit => {
                let mut c = Command::new("systemd-inhibit");
                c.args([
                    "--what=idle:sleep",
                    "--mode=block",
                    "--who",
                    &self.app_id,
                    "--why",
                    why,
                    "--",
                    "sleep",
                    BLOCKER_SLEEP_SECONDS,
                ]);
                c
            }
            SleepInhibitorBackend::GnomeSessionInhibit => {
                let mut c = Command::new("gnome-session-inhibit");
                c.args([
                    "--inhibit",
                    "idle:suspend",
                    "--reason",
                    why,
                    "sleep",
                    BLOCKER_SLEEP_SECONDS,
                ]);
                c
            }
            SleepInhibitorBackend::None => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    "No sleep inhibitor configured",
                ));
            }
        };

        cmd.stdin(Stdio::null());
        cmd.stdout(Stdio::null());
        cmd.stderr(Stdio::null());

        #[cfg(target_os = "linux")]
        {
            use std::os::unix::process::CommandExt;
            unsafe {
                cmd.pre_exec(|| {
                    // Instruct Linux kernel to deliver SIGTERM if parent M31A process terminates
                    libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM);
                    Ok(())
                });
            }
        }

        cmd.spawn()
    }
}

impl Drop for SleepInhibitor {
    fn drop(&mut self) {
        self.release();
    }
}

/// An RAII guard that holds sleep inhibition for the lifetime of a turn or background job.
pub struct SleepInhibitionGuard {
    inhibitor: Arc<Mutex<SleepInhibitor>>,
}

impl SleepInhibitionGuard {
    pub fn acquire(reason: &str) -> Self {
        let inhibitor = Arc::new(Mutex::new(SleepInhibitor::new()));
        inhibitor.lock().unwrap().acquire(Some(reason));
        Self { inhibitor }
    }
}

impl Drop for SleepInhibitionGuard {
    fn drop(&mut self) {
        if let Ok(mut inh) = self.inhibitor.lock() {
            inh.release();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sleep_inhibitor_lifecycle() {
        let mut inhibitor = SleepInhibitor::new();
        assert!(!inhibitor.is_active());

        // Attempting acquire gracefully succeeds or no-ops without panic
        inhibitor.acquire(Some("Test unit run"));
        // Release cleans up
        inhibitor.release();
        assert!(!inhibitor.is_active());
    }

    #[test]
    fn test_sleep_inhibition_guard_raii() {
        {
            let _guard = SleepInhibitionGuard::acquire("RAII scope test");
        }
        // Guard released on drop without leaking child processes
    }
}
