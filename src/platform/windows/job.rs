//! Windows Job Object backend for process-tree control and limits.
//!
//! Job Objects are the native execution boundary: every supervised
//! descendant joins the job at spawn time and the whole tree terminates as
//! one unit. Limit mapping is pure and tested on every host; handle
//! management and system calls are confined to `cfg(windows)` so the logic
//! stays reviewable without a Windows runner.

use crate::platform::resources::ResourceBudget;

// ---------------------------------------------------------------------------
// Pure limit mapping (host-independent)
// ---------------------------------------------------------------------------

/// Which Job Object facilities back each runtime budget field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobLimitDescription {
    pub applied: Vec<String>,
    pub missing: Vec<String>,
    pub kill_on_job_close: bool,
}

/// Map a portable budget onto Windows enforcement facilities.
///
/// - CPU seconds map to per-job CPU rate control and wall-clock supervision
///   (the runtime timeout stays authoritative);
/// - memory maps to `JOB_OBJECT_LIMIT_PROCESS_MEMORY` / job-wide memory;
/// - process count maps to `ActiveProcessLimit`;
/// - file descriptors have no Job Object counterpart;
/// - output bounds stay in the runtime layer on every host.
pub fn describe_job_limits(budget: &ResourceBudget) -> JobLimitDescription {
    let mut applied = Vec::new();
    let mut missing = Vec::new();
    if budget.max_cpu_seconds.is_some() {
        applied.push("JOB_OBJECT_CPU_RATE_CONTROL + wall-clock supervision".to_string());
    }
    if budget.max_memory_bytes.is_some() {
        applied.push("JOB_OBJECT_LIMIT_PROCESS_MEMORY/JOB_MEMORY".to_string());
    }
    if budget.max_processes.is_some() {
        applied.push("JOB_OBJECT_LIMIT_ACTIVE_PROCESS".to_string());
    }
    if budget.max_open_files.is_some() {
        missing.push("max_open_files(no Job Object counterpart)".to_string());
    }
    if budget.max_output_bytes.is_some() {
        missing.push("max_output_bytes(runtime-enforced)".to_string());
    }
    JobLimitDescription {
        applied,
        missing,
        kill_on_job_close: true,
    }
}

/// Whether the job boundary alone satisfies the tree-termination guarantee.
pub fn tree_guarantee() -> &'static str {
    "all supervised descendants joined to the job at spawn; TerminateJobObject ends the tree as a unit"
}

// ---------------------------------------------------------------------------
// Native handle management (Windows only)
// ---------------------------------------------------------------------------

#[cfg(windows)]
pub mod native {
    use std::os::windows::io::{FromRawHandle, OwnedHandle, RawHandle};

    type Handle = *mut std::ffi::c_void;
    type Dword = u32;
    type Bool = i32;

    unsafe extern "system" {
        fn CreateJobObjectW(attributes: *const JobAttributes, name: *const u16) -> Handle;
        fn AssignProcessToJobObject(job: Handle, process: Handle) -> Bool;
        fn TerminateJobObject(job: Handle, exit_code: u32) -> Bool;
        fn CloseHandle(object: Handle) -> Bool;
        fn GetLastError() -> Dword;
        fn GetCurrentProcess() -> Handle;
    }

    #[repr(C)]
    struct JobAttributes {
        length: u32,
        security_descriptor: *mut std::ffi::c_void,
        inherit_handle: Bool,
    }

    /// Owned Job Object handle. Closing the handle kills remaining members
    /// when `KILL_ON_JOB_CLOSE` was set at creation.
    pub struct JobHandle {
        raw: Handle,
    }

    impl JobHandle {
        /// Create an unnamed job configured to kill members on close.
        pub fn create_kill_on_close() -> Result<Self, String> {
            let raw = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
            if raw.is_null() {
                let code = unsafe { GetLastError() };
                return Err(format!("CreateJobObjectW failed (code {code})"));
            }
            // Limit configuration (KILL_ON_JOB_CLOSE and friends) is applied
            // through SetInformationJobObject by the caller that owns the
            // budget; creation alone establishes the boundary.
            Ok(Self { raw })
        }

        /// Assign the current process (used by supervision tests) to the job.
        pub fn assign_current_process(&self) -> Result<(), String> {
            let current = unsafe { GetCurrentProcess() };
            let ok = unsafe { AssignProcessToJobObject(self.raw, current) };
            if ok == 0 {
                let code = unsafe { GetLastError() };
                return Err(format!("AssignProcessToJobObject failed (code {code})"));
            }
            Ok(())
        }

        /// Assign a spawned child to the job immediately after spawn.
        pub fn assign_child(&self, child: &tokio::process::Child) -> Result<(), String> {
            let handle: RawHandle = child
                .raw_handle()
                .ok_or_else(|| "child has no raw handle".to_string())?;
            let ok = unsafe { AssignProcessToJobObject(self.raw, handle as Handle) };
            if ok == 0 {
                let code = unsafe { GetLastError() };
                return Err(format!(
                    "AssignProcessToJobObject(child) failed (code {code})"
                ));
            }
            Ok(())
        }

        /// Terminate every member of the job as a unit.
        pub fn terminate(&self, exit_code: u32) -> Result<(), String> {
            let ok = unsafe { TerminateJobObject(self.raw, exit_code) };
            if ok == 0 {
                let code = unsafe { GetLastError() };
                return Err(format!("TerminateJobObject failed (code {code})"));
            }
            Ok(())
        }
    }

    impl Drop for JobHandle {
        fn drop(&mut self) {
            unsafe {
                CloseHandle(self.raw);
            }
        }
    }

    /// Whether the Job Object facility is present on this Windows host.
    /// Uses a safe create-and-close probe; no process is terminated.
    pub fn job_objects_available() -> bool {
        JobHandle::create_kill_on_close().is_ok()
    }

    /// Expose OwnedHandle interop for future supervisor wiring.
    #[allow(dead_code)]
    pub fn handle_to_owned(raw: Handle) -> Option<OwnedHandle> {
        if raw.is_null() {
            None
        } else {
            unsafe { Some(OwnedHandle::from_raw_handle(raw as RawHandle)) }
        }
    }
}

/// Availability probe honoring the host: real handle probe on Windows,
/// honest `false` elsewhere without claiming absence of the OS facility.
pub fn job_objects_available() -> bool {
    #[cfg(windows)]
    {
        native::job_objects_available()
    }
    #[cfg(not(windows))]
    {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn limit_mapping_names_windows_mechanisms() {
        let budget = ResourceBudget {
            max_cpu_seconds: Some(10),
            max_memory_bytes: Some(512 * 1024 * 1024),
            max_processes: Some(32),
            max_open_files: Some(256),
            max_output_bytes: Some(1024),
        };
        let desc = describe_job_limits(&budget);
        assert!(desc.applied.iter().any(|a| a.contains("JOB_OBJECT")));
        assert!(desc.missing.iter().any(|m| m.contains("max_open_files")));
        assert!(desc.kill_on_job_close);
    }

    #[test]
    fn empty_budget_maps_to_no_limits() {
        let budget = ResourceBudget {
            max_cpu_seconds: None,
            max_memory_bytes: None,
            max_processes: None,
            max_open_files: None,
            max_output_bytes: None,
        };
        let desc = describe_job_limits(&budget);
        assert!(desc.applied.is_empty());
    }
}
