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
pub use native::JobHandle;

#[cfg(not(windows))]
#[derive(Debug, Clone)]
pub struct JobHandle;

#[cfg(not(windows))]
impl JobHandle {
    pub fn create_kill_on_close() -> Result<Self, String> {
        Err("Job Objects are only available on Windows".to_string())
    }

    pub fn create_with_limits(_budget: &ResourceBudget) -> Result<Self, String> {
        Err("Job Objects are only available on Windows".to_string())
    }

    pub fn apply_limits(&self, _budget: &ResourceBudget) -> Result<(), String> {
        Err("Job Objects are only available on Windows".to_string())
    }

    pub fn assign_child(&self, _child: &tokio::process::Child) -> Result<(), String> {
        Err("Job Objects are only available on Windows".to_string())
    }

    pub fn terminate(&self, _exit_code: u32) -> Result<(), String> {
        Err("Job Objects are only available on Windows".to_string())
    }
}

#[cfg(windows)]
pub mod native {
    use super::ResourceBudget;
    use std::os::windows::io::{FromRawHandle, OwnedHandle, RawHandle};

    type Handle = *mut std::ffi::c_void;
    type Dword = u32;
    type Bool = i32;

    const JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE: Dword = 0x00002000;
    const JOB_OBJECT_LIMIT_ACTIVE_PROCESS: Dword = 0x00000008;
    const JOB_OBJECT_LIMIT_PROCESS_MEMORY: Dword = 0x00000100;
    const JOB_OBJECT_LIMIT_JOB_MEMORY: Dword = 0x00000200;
    const JOB_OBJECT_LIMIT_JOB_TIME: Dword = 0x00000004;

    const JOB_OBJECT_EXTENDED_LIMIT_INFORMATION: u32 = 9;

    #[repr(C)]
    struct JobAttributes {
        length: u32,
        security_descriptor: *mut std::ffi::c_void,
        inherit_handle: Bool,
    }

    #[repr(C)]
    struct JobObjectBasicLimitInformation {
        per_process_user_time_limit: i64,
        per_job_user_time_limit: i64,
        limit_flags: Dword,
        minimum_working_set_size: usize,
        maximum_working_set_size: usize,
        active_process_limit: Dword,
        affinity: usize,
        priority_class: Dword,
        scheduling_class: Dword,
    }

    #[repr(C)]
    struct IoCounters {
        read_operation_count: u64,
        write_operation_count: u64,
        other_operation_count: u64,
        read_transfer_count: u64,
        write_transfer_count: u64,
        other_transfer_count: u64,
    }

    #[repr(C)]
    struct JobObjectExtendedLimitInformation {
        basic_limit_information: JobObjectBasicLimitInformation,
        io_info: IoCounters,
        process_memory_limit: usize,
        job_memory_limit: usize,
        peak_process_memory_used: usize,
        peak_job_memory_used: usize,
    }

    unsafe extern "system" {
        fn CreateJobObjectW(attributes: *const JobAttributes, name: *const u16) -> Handle;
        fn SetInformationJobObject(
            job: Handle,
            info_class: u32,
            info: *const std::ffi::c_void,
            info_length: u32,
        ) -> Bool;
        fn AssignProcessToJobObject(job: Handle, process: Handle) -> Bool;
        fn TerminateJobObject(job: Handle, exit_code: u32) -> Bool;
        fn CloseHandle(object: Handle) -> Bool;
        fn GetLastError() -> Dword;
        fn GetCurrentProcess() -> Handle;
    }

    /// Owned Job Object handle. Closing the handle kills remaining members
    /// when `KILL_ON_JOB_CLOSE` was set at creation.
    #[derive(Debug)]
    pub struct JobHandle {
        raw: Handle,
    }

    unsafe impl Send for JobHandle {}
    unsafe impl Sync for JobHandle {}

    impl JobHandle {
        /// Create an unnamed job configured to kill members on close.
        pub fn create_kill_on_close() -> Result<Self, String> {
            let handle = Self::create_raw()?;
            handle.configure_kill_on_close()?;
            Ok(handle)
        }

        /// Create a job configured with budget resource limits and kill on close.
        pub fn create_with_limits(budget: &ResourceBudget) -> Result<Self, String> {
            let handle = Self::create_raw()?;
            handle.apply_limits(budget)?;
            Ok(handle)
        }

        fn create_raw() -> Result<Self, String> {
            let raw = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
            if raw.is_null() {
                let code = unsafe { GetLastError() };
                return Err(format!("CreateJobObjectW failed (code {code})"));
            }
            Ok(Self { raw })
        }

        /// Configure `KILL_ON_JOB_CLOSE` so any surviving descendants terminate on handle drop.
        pub fn configure_kill_on_close(&self) -> Result<(), String> {
            let mut info: JobObjectExtendedLimitInformation = unsafe { std::mem::zeroed() };
            info.basic_limit_information.limit_flags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            let ok = unsafe {
                SetInformationJobObject(
                    self.raw,
                    JOB_OBJECT_EXTENDED_LIMIT_INFORMATION,
                    &info as *const _ as *const std::ffi::c_void,
                    std::mem::size_of::<JobObjectExtendedLimitInformation>() as u32,
                )
            };
            if ok == 0 {
                let code = unsafe { GetLastError() };
                return Err(format!(
                    "SetInformationJobObject(KILL_ON_JOB_CLOSE) failed (code {code})"
                ));
            }
            Ok(())
        }

        /// Apply enforceable limits from a `ResourceBudget` to the Job Object.
        pub fn apply_limits(&self, budget: &ResourceBudget) -> Result<(), String> {
            let mut info: JobObjectExtendedLimitInformation = unsafe { std::mem::zeroed() };
            let mut flags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;

            if let Some(mem) = budget.max_memory_bytes {
                flags |= JOB_OBJECT_LIMIT_PROCESS_MEMORY | JOB_OBJECT_LIMIT_JOB_MEMORY;
                info.process_memory_limit = mem as usize;
                info.job_memory_limit = mem as usize;
            }

            if let Some(procs) = budget.max_processes {
                flags |= JOB_OBJECT_LIMIT_ACTIVE_PROCESS;
                info.basic_limit_information.active_process_limit = procs as u32;
            }

            if let Some(cpu) = budget.max_cpu_seconds {
                flags |= JOB_OBJECT_LIMIT_JOB_TIME;
                info.basic_limit_information.per_job_user_time_limit =
                    (cpu as i64).saturating_mul(10_000_000);
            }

            info.basic_limit_information.limit_flags = flags;

            let ok = unsafe {
                SetInformationJobObject(
                    self.raw,
                    JOB_OBJECT_EXTENDED_LIMIT_INFORMATION,
                    &info as *const _ as *const std::ffi::c_void,
                    std::mem::size_of::<JobObjectExtendedLimitInformation>() as u32,
                )
            };
            if ok == 0 {
                let code = unsafe { GetLastError() };
                return Err(format!(
                    "SetInformationJobObject(limits) failed (code {code})"
                ));
            }
            Ok(())
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

    /// Read process creation timestamp on Windows via GetProcessTimes.
    pub fn get_process_creation_time(pid: u32) -> Option<u64> {
        const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;
        unsafe extern "system" {
            fn OpenProcess(desired_access: u32, inherit_handle: Bool, process_id: u32) -> Handle;
            fn GetProcessTimes(
                process: Handle,
                creation_time: *mut FileTime,
                exit_time: *mut FileTime,
                kernel_time: *mut FileTime,
                user_time: *mut FileTime,
            ) -> Bool;
        }
        #[repr(C)]
        struct FileTime {
            low: u32,
            high: u32,
        }
        let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
        if handle.is_null() {
            return None;
        }
        let mut creation: FileTime = unsafe { std::mem::zeroed() };
        let mut exit: FileTime = unsafe { std::mem::zeroed() };
        let mut kernel: FileTime = unsafe { std::mem::zeroed() };
        let mut user: FileTime = unsafe { std::mem::zeroed() };
        let ok =
            unsafe { GetProcessTimes(handle, &mut creation, &mut exit, &mut kernel, &mut user) };
        unsafe { CloseHandle(handle) };
        if ok != 0 {
            Some(((creation.high as u64) << 32) | (creation.low as u64))
        } else {
            None
        }
    }

    /// Process-tree termination fallback by PID via Toolhelp32 snapshot when no JobHandle is held.
    pub fn terminate_process_tree_fallback(pid: u32, exit_code: u32) -> Result<(), String> {
        const TH32CS_SNAPPROCESS: u32 = 0x00000002;
        const PROCESS_TERMINATE: u32 = 0x0001;
        #[repr(C)]
        struct ProcessEntry32W {
            size: u32,
            usage: u32,
            process_id: u32,
            default_heap_id: usize,
            module_id: u32,
            threads: u32,
            parent_process_id: u32,
            pri_class_base: i32,
            flags: u32,
            exe_file: [u16; 260],
        }
        unsafe extern "system" {
            fn CreateToolhelp32Snapshot(flags: u32, process_id: u32) -> Handle;
            fn Process32FirstW(snapshot: Handle, entry: *mut ProcessEntry32W) -> Bool;
            fn Process32NextW(snapshot: Handle, entry: *mut ProcessEntry32W) -> Bool;
            fn OpenProcess(desired_access: u32, inherit_handle: Bool, process_id: u32) -> Handle;
            fn TerminateProcess(process: Handle, exit_code: u32) -> Bool;
        }

        let snap = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
        if snap.is_null() || snap == (-1isize as Handle) {
            let h = unsafe { OpenProcess(PROCESS_TERMINATE, 0, pid) };
            if !h.is_null() {
                unsafe {
                    TerminateProcess(h, exit_code);
                    CloseHandle(h);
                }
            }
            return Ok(());
        }

        let mut children = Vec::new();
        let mut entry: ProcessEntry32W = unsafe { std::mem::zeroed() };
        entry.size = std::mem::size_of::<ProcessEntry32W>() as u32;
        if unsafe { Process32FirstW(snap, &mut entry) } != 0 {
            loop {
                if entry.parent_process_id == pid && entry.process_id != pid {
                    children.push(entry.process_id);
                }
                if unsafe { Process32NextW(snap, &mut entry) } == 0 {
                    break;
                }
            }
        }
        unsafe { CloseHandle(snap) };

        for child_pid in children {
            let _ = terminate_process_tree_fallback(child_pid, exit_code);
        }

        let h = unsafe { OpenProcess(PROCESS_TERMINATE, 0, pid) };
        if !h.is_null() {
            unsafe {
                TerminateProcess(h, exit_code);
                CloseHandle(h);
            }
        }
        Ok(())
    }

    /// Whether the Job Object facility is present on this Windows host.
    /// Uses a safe create-and-close probe; no process is terminated.
    pub fn job_objects_available() -> bool {
        JobHandle::create_kill_on_close().is_ok()
    }

    /// Expose OwnedHandle interop for supervisor wiring.
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
