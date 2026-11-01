//! Asynchronous POSIX Process Group Reaper and Zombie Cleaner.
//!
//! Provides deterministic process group cleanup (`SIGTERM` -> bounded grace period ->
//! `SIGKILL` escalation) and non-blocking zombie reaping (`waitpid(WNOHANG)`) for
//! sandbox command execution, preventing orphaned background worker processes or
//! compiler tasks from leaking host resources.

use std::time::{Duration, Instant};

/// Summary report returned after reaping a process group.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessReapReport {
    pub pgid: i32,
    pub terminated_with_sigterm: bool,
    pub escalated_to_sigkill: bool,
    pub reaped_pids: Vec<i32>,
    pub duration_ms: u64,
}

/// Terminate an entire POSIX process group with bounded grace period.
///
/// 1. Sends `SIGTERM` to the process group (`-pgid`).
/// 2. Polls `waitpid` with `WNOHANG` up to `grace_period_ms`.
/// 3. If any processes remain alive after the grace period, escalates to `SIGKILL` (`-pgid`).
/// 4. Reaps and collects all terminated child PIDs.
pub fn terminate_process_group(
    pgid: i32,
    grace_period_ms: u64,
) -> std::io::Result<ProcessReapReport> {
    if pgid <= 1 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("Invalid pgid for group termination: {}", pgid),
        ));
    }

    let start = Instant::now();
    let mut reaped_pids = Vec::new();
    let mut escalated_to_sigkill = false;

    // Check if any process exists in group
    let is_alive = unsafe { libc::kill(-pgid, 0) == 0 };
    if !is_alive {
        // Already dead, drain any pending zombies
        reaped_pids.extend(reap_zombies(Some(pgid)));
        return Ok(ProcessReapReport {
            pgid,
            terminated_with_sigterm: false,
            escalated_to_sigkill: false,
            reaped_pids,
            duration_ms: start.elapsed().as_millis() as u64,
        });
    }

    // Step 1: Send SIGTERM to -pgid
    unsafe {
        libc::kill(-pgid, libc::SIGTERM);
    }
    let terminated_with_sigterm = true;

    // Step 2: Poll for exit within grace period
    let grace = Duration::from_millis(grace_period_ms);
    let poll_interval = Duration::from_millis(15);

    while start.elapsed() < grace {
        let reaped = reap_zombies(Some(pgid));
        reaped_pids.extend(reaped);

        // Check if group is completely gone
        if unsafe { libc::kill(-pgid, 0) != 0 } {
            break;
        }

        std::thread::sleep(poll_interval);
    }

    // Step 3: If still alive, escalate to SIGKILL
    if unsafe { libc::kill(-pgid, 0) == 0 } {
        unsafe {
            libc::kill(-pgid, libc::SIGKILL);
        }
        escalated_to_sigkill = true;

        // Give kernel a brief slice to process SIGKILL
        std::thread::sleep(Duration::from_millis(10));
        let final_reaped = reap_zombies(Some(pgid));
        reaped_pids.extend(final_reaped);
    }

    reaped_pids.sort_unstable();
    reaped_pids.dedup();

    Ok(ProcessReapReport {
        pgid,
        terminated_with_sigterm,
        escalated_to_sigkill,
        reaped_pids,
        duration_ms: start.elapsed().as_millis() as u64,
    })
}

/// Reaps any exited child processes without blocking.
///
/// If `pgid` is specified, reaps specifically within that process group (`-pgid`),
/// otherwise reaps across all child processes (`-1`).
pub fn reap_zombies(pgid: Option<i32>) -> Vec<i32> {
    let target = match pgid {
        Some(p) => -p,
        None => -1,
    };

    let mut reaped = Vec::new();
    loop {
        let mut status = 0;
        let pid = unsafe { libc::waitpid(target, &mut status, libc::WNOHANG) };
        if pid > 0 {
            reaped.push(pid);
        } else {
            break;
        }
    }
    reaped
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::process::CommandExt;
    use std::process::Command;

    #[test]
    fn test_terminate_sleeping_process_group() {
        // Spawn a child process in a new process group (pgid = child pid)
        let mut child = Command::new("sleep")
            .arg("10")
            .process_group(0) // setpgid(0, 0)
            .spawn()
            .expect("Failed to spawn sleep child");

        let pid = child.id() as i32;
        let pgid = pid;

        // Verify group is running
        assert_eq!(unsafe { libc::kill(-pgid, 0) }, 0);

        // Terminate group
        let report = terminate_process_group(pgid, 200).expect("Reap failed");
        assert_eq!(report.pgid, pgid);
        let _ = child.wait();
        assert!(report.terminated_with_sigterm);

        // Verify process is now dead
        assert_ne!(unsafe { libc::kill(-pgid, 0) }, 0);
    }

    #[test]
    fn test_rejects_invalid_pgids() {
        assert!(terminate_process_group(0, 100).is_err());
        assert!(terminate_process_group(1, 100).is_err());
        assert!(terminate_process_group(-1, 100).is_err());
    }
}
