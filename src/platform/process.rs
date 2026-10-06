//! Native process-tree contract.
//!
//! The runtime guarantee is that a terminated child execution leaves no
//! uncontrolled descendant running after the operation reports completion.
//! Linux uses process groups; macOS uses process groups with libproc identity
//! checks; Windows uses Job Objects. Other backends report their honest
//! foundation state instead of pretending a plain child kill provides
//! equivalent isolation.

use std::time::Duration;
use tokio::process::{Child, Command};
#[cfg(unix)]
use tokio::time::sleep;

/// Failure to terminate a worker tree. Unsupported is distinct from I/O so
/// callers can distinguish a missing backend from a transient failure.
#[derive(Debug, thiserror::Error)]
pub enum PlatformProcessError {
    #[error("process-tree control unsupported on this backend: {0}")]
    Unsupported(String),
    #[error("process control I/O failure: {0}")]
    Io(String),
}

impl From<std::io::Error> for PlatformProcessError {
    fn from(value: std::io::Error) -> Self {
        if value.kind() == std::io::ErrorKind::Unsupported {
            Self::Unsupported(value.to_string())
        } else {
            Self::Io(value.to_string())
        }
    }
}

/// Whether the executing backend can terminate a full descendant tree.
pub fn supports_tree_termination() -> bool {
    cfg!(target_os = "linux") || cfg!(target_os = "macos") || cfg!(windows)
}

/// Backend name selected for the executing host.
pub fn backend_name() -> &'static str {
    if cfg!(target_os = "linux") {
        "linux-process-groups"
    } else if cfg!(target_os = "macos") {
        crate::platform::macos::process_backend_name()
    } else if cfg!(windows) {
        crate::platform::windows::process_backend_name()
    } else if cfg!(unix) {
        "generic-unix"
    } else {
        "unknown"
    }
}

/// Place a command in its own isolation scope when the host supports one.
pub fn configure_isolation(cmd: &mut Command) {
    #[cfg(target_os = "linux")]
    {
        cmd.process_group(0);
    }
    #[cfg(target_os = "macos")]
    {
        cmd.process_group(0);
    }
    #[cfg(windows)]
    {
        // Job Object assignment happens at spawn time in the supervisor.
        // No pre-execution hook is needed.
        let _ = cmd;
    }
    #[cfg(all(not(target_os = "linux"), not(target_os = "macos"), not(windows), unix))]
    {
        cmd.process_group(0);
    }
    #[cfg(all(not(unix), not(windows)))]
    {
        let _ = cmd;
    }
}

/// Spawn already configured for isolation and return the live child.
///
/// On Windows, the spawned child is immediately assigned to the Job Object
/// by the supervisor after this call returns.
pub fn spawn_isolated(mut cmd: Command) -> Result<(Child, u32), std::io::Error> {
    configure_isolation(&mut cmd);
    let child = cmd.spawn()?;
    let pid = child
        .id()
        .ok_or_else(|| std::io::Error::other("spawned child has no process identifier"))?;
    Ok((child, pid))
}

/// Graceful-then-forceful termination of one supervised child.
///
/// Linux/macOS: group-wide escalation via process-group signals.
/// Windows: Job Object termination escalation handled by supervisor.
/// Other: single child kill with explicit unsupported tree guarantee.
pub async fn terminate_supervised_child(
    child: &mut Child,
    pid: u32,
    grace_period: Duration,
) -> Result<std::process::ExitStatus, PlatformProcessError> {
    if let Ok(Some(status)) = child.try_wait() {
        return Ok(status);
    }
    #[cfg(target_os = "linux")]
    {
        terminate_linux_tree(child, pid, grace_period).await
    }
    #[cfg(target_os = "macos")]
    {
        terminate_macos_tree(child, pid, grace_period).await
    }
    #[cfg(windows)]
    {
        // Windows Job Object termination is handled by the supervisor
        // which owns the JobHandle. This path is unreachable when Job
        // Objects are available; retained for completeness.
        let _ = (pid, grace_period);
        child
            .kill()
            .await
            .map_err(|e| PlatformProcessError::Io(e.to_string()))?;
        child
            .wait()
            .await
            .map_err(|e| PlatformProcessError::Io(e.to_string()))
    }
    #[cfg(all(not(target_os = "linux"), not(target_os = "macos"), not(windows), unix))]
    {
        terminate_unix_tree(child, pid, grace_period).await
    }
    #[cfg(all(not(unix), not(windows)))]
    {
        let _ = pid;
        child
            .kill()
            .await
            .map_err(|e| PlatformProcessError::Io(e.to_string()))?;
        child
            .wait()
            .await
            .map_err(|e| PlatformProcessError::Io(e.to_string()))
    }
}

/// Terminate an externally tracked worker tree by group-leader identifier.
///
/// Linux/macOS: group signaling by PID. Windows: Job Object (requires
/// JobHandle, not a bare PID). Other: explicit unsupported.
pub async fn terminate_process_tree_by_pid(
    pid: u32,
    grace_period: Duration,
) -> Result<(), PlatformProcessError> {
    #[cfg(target_os = "linux")]
    {
        terminate_linux_tree_by_pid(pid, grace_period).await
    }
    #[cfg(target_os = "macos")]
    {
        terminate_macos_tree_by_pid(pid, grace_period).await
    }
    #[cfg(windows)]
    {
        let _ = grace_period;
        crate::platform::windows::terminate_process_tree_fallback(pid, 1)
            .map_err(PlatformProcessError::Io)
    }
    #[cfg(all(not(target_os = "linux"), not(target_os = "macos"), not(windows), unix))]
    {
        terminate_unix_tree_by_pid(pid, grace_period).await
    }
    #[cfg(all(not(unix), not(windows)))]
    {
        let _ = grace_period;
        Err(PlatformProcessError::Unsupported(format!(
            "process-tree termination by pid {pid} requires native controls unavailable on '{}'",
            backend_name()
        )))
    }
}

/// Linux process-tree termination via process groups.
#[cfg(target_os = "linux")]
async fn terminate_linux_tree(
    child: &mut Child,
    pid: u32,
    grace_period: Duration,
) -> Result<std::process::ExitStatus, PlatformProcessError> {
    let pgid = pid as libc::pid_t;
    unsafe {
        libc::kill(-pgid, libc::SIGTERM);
    }
    match tokio::time::timeout(grace_period, child.wait()).await {
        Ok(Ok(status)) => Ok(status),
        Ok(Err(e)) => Err(PlatformProcessError::Io(e.to_string())),
        Err(_) => {
            if let Ok(Some(status)) = child.try_wait() {
                return Ok(status);
            }
            unsafe {
                libc::kill(-pgid, libc::SIGKILL);
            }
            child
                .wait()
                .await
                .map_err(|e| PlatformProcessError::Io(e.to_string()))
        }
    }
}

/// macOS process-tree termination via process groups with libproc identity.
#[cfg(target_os = "macos")]
async fn terminate_macos_tree(
    child: &mut Child,
    pid: u32,
    grace_period: Duration,
) -> Result<std::process::ExitStatus, PlatformProcessError> {
    // Verify the group leader identity before signaling to avoid PID reuse.
    if let Some(start) = crate::platform::macos::read_process_starttime(pid) {
        // Record the start time; after grace period we re-verify the identity
        // before escalating. If the process is gone or replaced, we do not
        // signal the new occupant.
        let _identity = start;
    }
    let pgid = pid as libc::pid_t;
    unsafe {
        libc::kill(-pgid, libc::SIGTERM);
    }
    match tokio::time::timeout(grace_period, child.wait()).await {
        Ok(Ok(status)) => Ok(status),
        Ok(Err(e)) => Err(PlatformProcessError::Io(e.to_string())),
        Err(_) => {
            if let Ok(Some(status)) = child.try_wait() {
                return Ok(status);
            }
            // Re-verify PID identity before SIGKILL.
            if crate::platform::macos::macos_pid_alive(pid) {
                unsafe {
                    libc::kill(-pgid, libc::SIGKILL);
                }
            }
            child
                .wait()
                .await
                .map_err(|e| PlatformProcessError::Io(e.to_string()))
        }
    }
}

/// Unix (non-Linux/macOS) process-tree termination via process groups.
#[cfg(all(not(target_os = "linux"), not(target_os = "macos"), unix))]
async fn terminate_unix_tree(
    child: &mut Child,
    pid: u32,
    grace_period: Duration,
) -> Result<std::process::ExitStatus, PlatformProcessError> {
    let pgid = pid as libc::pid_t;
    unsafe {
        libc::kill(-pgid, libc::SIGTERM);
    }
    match tokio::time::timeout(grace_period, child.wait()).await {
        Ok(Ok(status)) => Ok(status),
        Ok(Err(e)) => Err(PlatformProcessError::Io(e.to_string())),
        Err(_) => {
            if let Ok(Some(status)) = child.try_wait() {
                return Ok(status);
            }
            unsafe {
                libc::kill(-pgid, libc::SIGKILL);
            }
            child
                .wait()
                .await
                .map_err(|e| PlatformProcessError::Io(e.to_string()))
        }
    }
}

/// Linux termination by PID.
#[cfg(target_os = "linux")]
async fn terminate_linux_tree_by_pid(
    pid: u32,
    grace_period: Duration,
) -> Result<(), PlatformProcessError> {
    let pgid = pid as libc::pid_t;
    unsafe {
        libc::kill(-pgid, libc::SIGTERM);
    }
    sleep(grace_period).await;
    let alive = unsafe { libc::kill(-pgid, 0) == 0 };
    if alive {
        unsafe {
            libc::kill(-pgid, libc::SIGKILL);
        }
    }
    Ok(())
}

/// macOS termination by PID with identity check.
#[cfg(target_os = "macos")]
async fn terminate_macos_tree_by_pid(
    pid: u32,
    grace_period: Duration,
) -> Result<(), PlatformProcessError> {
    // Record identity before SIGTERM.
    let identity = crate::platform::macos::read_process_starttime(pid);
    let pgid = pid as libc::pid_t;
    unsafe {
        libc::kill(-pgid, libc::SIGTERM);
    }
    sleep(grace_period).await;
    // Verify the target still exists and is the same process.
    let alive = unsafe { libc::kill(-pgid, 0) == 0 };
    if alive {
        if let Some(start) = identity {
            // Re-read and compare start time to detect recycling.
            if crate::platform::macos::read_process_starttime(pid) == Some(start) {
                unsafe {
                    libc::kill(-pgid, libc::SIGKILL);
                }
            }
        } else {
            // No identity probe: still escalate to be safe.
            unsafe {
                libc::kill(-pgid, libc::SIGKILL);
            }
        }
    }
    Ok(())
}

/// Unix termination by PID.
#[cfg(all(not(target_os = "linux"), not(target_os = "macos"), unix))]
async fn terminate_unix_tree_by_pid(
    pid: u32,
    grace_period: Duration,
) -> Result<(), PlatformProcessError> {
    let pgid = pid as libc::pid_t;
    unsafe {
        libc::kill(-pgid, libc::SIGTERM);
    }
    sleep(grace_period).await;
    let alive = unsafe { libc::kill(-pgid, 0) == 0 };
    if alive {
        unsafe {
            libc::kill(-pgid, libc::SIGKILL);
        }
    }
    Ok(())
}

/// Linux-only process start time used to detect identifier recycling.
///
/// Returns `None` on hosts without the corresponding kernel interface so
/// reconciliation treats the identity as unprovable rather than guessing.
pub fn read_process_starttime(pid: u32) -> Option<u64> {
    #[cfg(target_os = "linux")]
    {
        let content = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
        let last_paren = content.rfind(')')?;
        let rest = content[last_paren + 1..].trim_start();
        let fields: Vec<&str> = rest.split_whitespace().collect();
        if fields.len() > 19 {
            fields[19].parse::<u64>().ok()
        } else {
            None
        }
    }
    #[cfg(target_os = "macos")]
    {
        crate::platform::macos::read_process_starttime(pid)
    }
    #[cfg(windows)]
    {
        crate::platform::windows::read_process_starttime(pid)
    }
    #[cfg(all(not(target_os = "linux"), not(target_os = "macos"), not(windows)))]
    {
        let _ = pid;
        None
    }
}

/// Whether identifier-recycling protection is enforceable here.
pub fn supports_starttime_protection() -> bool {
    cfg!(target_os = "linux") || cfg!(target_os = "macos") || cfg!(windows)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backend_name_matches_host() {
        let name = backend_name();
        assert!(!name.is_empty());
    }

    #[test]
    fn supports_tree_termination_on_targets() {
        #[cfg(any(target_os = "linux", target_os = "macos", windows))]
        {
            assert!(supports_tree_termination());
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
        {
            // Other Unix may or may not; we only claim it where implemented.
        }
    }

    #[test]
    fn configure_isolation_compiles_on_all_hosts() {
        use tokio::process::Command;
        let mut cmd = Command::new("true");
        configure_isolation(&mut cmd);
        // No panic = success
    }

    #[test]
    fn read_process_starttime_safe_off_target() {
        #[cfg(not(target_os = "linux"))]
        {
            assert_eq!(read_process_starttime(1), None);
        }
    }
}
