//! Supervised process-tree isolation and two-phase cancellation (TL-02, Law 8, per D-14).
//!
//! Enforces:
//! - Group-scoped isolation so descendant child processes cannot escape.
//! - Two-phase escalation (graceful request, grace period, forceful fallback).
//! - Pre-signal `try_wait()` to prevent identifier-recycling hazards.
//! - Complete descendant reaping to eliminate orphan/zombie processes.
//!
//! The native mechanism lives in the platform boundary. This controller
//! states the runtime guarantee and delegates enforcement to it.

use std::sync::Arc;
use std::time::Duration;
use tokio::process::{Child, Command};

use crate::platform::process as platform_process;
use crate::platform::resources::ResourceBudget;
use crate::platform::windows::job::JobHandle;

/// Process tree controller managing isolation scopes, native Job Objects, and escalation (D-14).
#[derive(Debug, Clone)]
pub struct ProcessTreeController {
    pid: u32,
    job: Option<Arc<JobHandle>>,
}

impl ProcessTreeController {
    /// Create a controller tracking an existing isolation-scope leader PID without a native job object.
    pub fn new(pid: u32) -> Self {
        Self { pid, job: None }
    }

    /// Create a controller tracking an existing isolation-scope leader PID with an optional native job object.
    pub fn with_job(pid: u32, job: Option<Arc<JobHandle>>) -> Self {
        Self { pid, job }
    }

    /// Process ID of the group leader.
    pub fn pid(&self) -> u32 {
        self.pid
    }

    /// Access the native Job Object handle if one is tracked.
    pub fn job_handle(&self) -> Option<&Arc<JobHandle>> {
        self.job.as_ref()
    }

    /// Prepares a `Command` with isolation scope from the platform backend.
    pub fn configure_command(cmd: &mut Command) {
        platform_process::configure_isolation(cmd);
    }

    /// Spawns a command configured with isolation scope and returns the child handle and its controller.
    pub fn spawn_isolated(cmd: Command) -> Result<(Child, Self), std::io::Error> {
        Self::spawn_isolated_with_budget(cmd, None)
    }

    /// Spawns a command configured with isolation scope and resource budget enforcement.
    /// On Windows, this creates a native Job Object, applies budget limits, and assigns the spawned child.
    pub fn spawn_isolated_with_budget(
        mut cmd: Command,
        budget: Option<&ResourceBudget>,
    ) -> Result<(Child, Self), std::io::Error> {
        Self::configure_command(&mut cmd);
        #[cfg(windows)]
        {
            let job = if let Some(b) = budget {
                JobHandle::create_with_limits(b).ok()
            } else {
                JobHandle::create_kill_on_close().ok()
            };
            let child = cmd.spawn()?;
            let pid = child
                .id()
                .ok_or_else(|| std::io::Error::other("Failed to get child PID"))?;
            let job_arc = job.map(|j| {
                let _ = j.assign_child(&child);
                Arc::new(j)
            });
            Ok((child, Self { pid, job: job_arc }))
        }
        #[cfg(not(windows))]
        {
            let _ = budget;
            let child = cmd.spawn()?;
            let pid = child
                .id()
                .ok_or_else(|| std::io::Error::other("Failed to get child PID"))?;
            Ok((child, Self { pid, job: None }))
        }
    }

    /// Executes the two-phase escalation termination protocol (Law 8, D-14).
    ///
    /// 1. Checks `child.try_wait()` to avoid signaling if already terminated.
    /// 2. If a native Job Object handle is held (Windows), terminates the entire job object tree directly.
    /// 3. Otherwise delegates to platform escalation.
    pub async fn terminate_supervised(
        &self,
        child: &mut Child,
        grace_period: Duration,
    ) -> Result<std::process::ExitStatus, std::io::Error> {
        if let Ok(Some(status)) = child.try_wait() {
            return Ok(status);
        }
        #[cfg(windows)]
        {
            if let Some(ref job) = self.job {
                let _ = job.terminate(1);
                return child.wait().await;
            }
        }
        platform_process::terminate_supervised_child(child, self.pid, grace_period)
            .await
            .map_err(|e| match e {
                crate::platform::process::PlatformProcessError::Unsupported(msg) => {
                    std::io::Error::new(std::io::ErrorKind::Unsupported, msg)
                }
                crate::platform::process::PlatformProcessError::Io(msg) => {
                    std::io::Error::other(msg)
                }
            })
    }

    /// Terminate an external or background process scope by PID when child handle is not held.
    pub async fn terminate_by_pid(pid: u32, grace_period: Duration) -> Result<(), std::io::Error> {
        platform_process::terminate_process_tree_by_pid(pid, grace_period)
            .await
            .map_err(|e| match e {
                crate::platform::process::PlatformProcessError::Unsupported(msg) => {
                    std::io::Error::new(std::io::ErrorKind::Unsupported, msg)
                }
                crate::platform::process::PlatformProcessError::Io(msg) => {
                    std::io::Error::other(msg)
                }
            })
    }

    /// Terminate an external or background process scope by PID via two-phase escalation (D-15).
    pub async fn kill_process_group(
        pid: u32,
        grace_period: Duration,
    ) -> Result<(), std::io::Error> {
        Self::terminate_by_pid(pid, grace_period).await
    }
}
