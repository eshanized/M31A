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

use std::time::Duration;
use tokio::process::{Child, Command};

use crate::platform::process as platform_process;

/// Process tree controller managing isolation scopes and escalation (D-14).
#[derive(Debug, Clone)]
pub struct ProcessTreeController {
    pid: u32,
}

impl ProcessTreeController {
    /// Create a controller tracking an existing isolation-scope leader PID.
    pub fn new(pid: u32) -> Self {
        Self { pid }
    }

    /// Process ID of the group leader.
    pub fn pid(&self) -> u32 {
        self.pid
    }

    /// Prepares a `Command` with isolation scope from the platform backend.
    pub fn configure_command(cmd: &mut Command) {
        platform_process::configure_isolation(cmd);
    }

    /// Spawns a command configured with isolation scope and returns the child handle and its controller.
    pub fn spawn_isolated(mut cmd: Command) -> Result<(Child, Self), std::io::Error> {
        Self::configure_command(&mut cmd);
        let child = cmd.spawn()?;
        let pid = child
            .id()
            .ok_or_else(|| std::io::Error::other("Failed to get child PID"))?;
        Ok((child, Self::new(pid)))
    }

    /// Executes the two-phase escalation termination protocol (Law 8, D-14).
    ///
    /// 1. Checks `child.try_wait()` to avoid signaling if already terminated.
    /// 2. Step 1: Sends a graceful termination request to the entire isolation scope.
    /// 3. Waits up to `grace_period`.
    /// 4. Step 2: If still running, escalates to forceful termination of the scope.
    /// 5. Awaits final status and ensures descendant processes are reaped.
    pub async fn terminate_supervised(
        &self,
        child: &mut Child,
        grace_period: Duration,
    ) -> Result<std::process::ExitStatus, std::io::Error> {
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
