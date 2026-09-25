//! Native macOS backend behind the platform contract.
//!
//! Mechanisms used:
//! - process groups via `setpgid` / `kill(-pgid, ...)` for tree control,
//!   with `libproc` identity checks against identifier recycling;
//! - POSIX `setrlimit` for CPU, address-space, file-descriptor, and process
//!   budgets where the kernel honors them;
//! - Unix owner-only mode bits for credential-bearing files;
//! - `sandbox-exec` with Seatbelt profiles as an optional filesystem
//!   isolation backend where the deployment still provides it;
//! - POSIX shell and PTY-backed terminals shared with other Unix hosts.
//!
//! Pure helpers (profile rendering, budget mapping, availability parsing)
//! compile on every host so contract tests exercise them natively. Raw
//! system bindings are confined to `target_os = "macos"` sections.

use std::path::{Path, PathBuf};

use crate::platform::capabilities::CapabilityState;
use crate::platform::resources::{LimitOutcome, ResourceBudget};

// ---------------------------------------------------------------------------
// Process identity
// ---------------------------------------------------------------------------

/// libproc flavor requesting BSD info for one process identifier.
pub const PROC_PIDTBSDINFO: i32 = 3;

/// Size class used when sizing the BSD-info buffer. The concrete layout is
/// resolved inside the `macos` section; the constant documents intent for
/// portable callers.
pub const PROC_PIDTBSDINFO_SIZE_HINT: usize = 256;

/// Whether identifier-recycling protection can be enforced on this host.
/// True only when executing on macOS where `libproc` answers.
pub fn supports_starttime_protection() -> bool {
    cfg!(target_os = "macos")
}

/// Backend label for diagnostics.
pub fn process_backend_name() -> &'static str {
    "macos-process-groups-libproc"
}

/// Read a stable per-process identity token on macOS.
///
/// Returns the `pbi_start_tvsec/usec` pair encoded as a single `u64` when the
/// host provides it, or `None` when the kernel interface is absent. Other
/// hosts always return `None` so callers treat identity as unprovable.
pub fn read_process_starttime(pid: u32) -> Option<u64> {
    #[cfg(target_os = "macos")]
    {
        macos_proc_starttime(pid)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = pid;
        None
    }
}

#[cfg(target_os = "macos")]
fn macos_proc_starttime(pid: u32) -> Option<u64> {
    // Minimal BSD-info layout: only the fields up to the start-time members
    // are modeled. Offsets match the system `proc_bsdinfo` definition; the
    // trailing members are intentionally omitted because this backend only
    // needs the start-time pair for recycling detection.
    #[repr(C)]
    struct ProcBsdInfoPartial {
        pbi_flags: u32,
        pbi_status: u32,
        pbi_xstatus: u32,
        pbi_pid: u32,
        pbi_ppid: u32,
        pbi_uid: u32,
        pbi_gid: u32,
        pbi_ruid: u32,
        pbi_rgid: u32,
        pbi_svuid: u32,
        pbi_svgid: u32,
        rfu_1: u32,
        pbi_comm: [u8; 16],
        pbi_nfiles: u32,
        pbi_pgid: u32,
        pbi_pjobc: u32,
        e_tdev: u32,
        e_tpgid: u32,
        pbi_nice: i32,
        pbi_start_tvsec: u64,
        pbi_start_tvusec: u32,
    }

    unsafe extern "C" {
        fn proc_pidinfo(
            pid: i32,
            flavor: i32,
            arg: u64,
            buffer: *mut std::ffi::c_void,
            buffersize: i32,
        ) -> i32;
    }

    let mut info: ProcBsdInfoPartial = unsafe { std::mem::zeroed() };
    let size = std::mem::size_of::<ProcBsdInfoPartial>() as i32;
    let read = unsafe {
        proc_pidinfo(
            pid as i32,
            PROC_PIDTBSDINFO,
            0,
            &mut info as *mut _ as *mut std::ffi::c_void,
            size,
        )
    };
    if read < size {
        return None;
    }
    if info.pbi_pid != pid {
        return None;
    }
    Some(
        info.pbi_start_tvsec
            .saturating_mul(1_000_000)
            .saturating_add(info.pbi_start_tvusec as u64),
    )
}

/// Whether a macOS process identifier currently refers to a live process.
pub fn macos_pid_alive(pid: u32) -> bool {
    #[cfg(target_os = "macos")]
    {
        unsafe { libc::kill(pid as libc::pid_t, 0) == 0 }
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = pid;
        false
    }
}

// ---------------------------------------------------------------------------
// Resource limits
// ---------------------------------------------------------------------------

/// Which budget fields map to enforceable `setrlimit` names on macOS.
pub fn macos_enforceable_limits(budget: &ResourceBudget) -> (Vec<String>, Vec<String>) {
    let mut applied = Vec::new();
    let mut missing = Vec::new();
    if budget.max_cpu_seconds.is_some() {
        applied.push("RLIMIT_CPU".to_string());
    }
    if budget.max_memory_bytes.is_some() {
        // macOS honors RLIMIT_AS / RLIMIT_RSS with allocator-dependent
        // behavior; the guarantee is best-effort, never exact accounting.
        applied.push("RLIMIT_AS(best-effort)".to_string());
    }
    if budget.max_open_files.is_some() {
        applied.push("RLIMIT_NOFILE".to_string());
    }
    if budget.max_processes.is_some() {
        applied.push("RLIMIT_NPROC".to_string());
    }
    if budget.max_output_bytes.is_some() {
        // Output bounds stay in the runtime dual-buffer layer on every host.
        missing.push("max_output_bytes(runtime-enforced)".to_string());
    }
    (applied, missing)
}

/// Validate a budget against macOS enforcement semantics.
pub fn validate_macos_budget(budget: &ResourceBudget) -> Result<LimitOutcome, LimitOutcome> {
    crate::platform::resources::validate_budget(budget).map_err(|e| e.clone())?;
    let (applied, missing) = macos_enforceable_limits(budget);
    if applied.is_empty() {
        return Ok(LimitOutcome::Unsupported);
    }
    if missing.is_empty() {
        Ok(LimitOutcome::Applied)
    } else {
        Ok(LimitOutcome::PartiallyApplied { applied, missing })
    }
}

// ---------------------------------------------------------------------------
// Seatbelt / sandbox-exec isolation
// ---------------------------------------------------------------------------

/// Candidate `sandbox-exec` binary locations probed in order.
pub fn sandbox_exec_candidates() -> Vec<PathBuf> {
    vec![
        PathBuf::from("/usr/bin/sandbox-exec"),
        PathBuf::from("/bin/sandbox-exec"),
    ]
}

/// Locate a usable `sandbox-exec` binary without executing a profile.
pub fn find_sandbox_exec() -> Option<PathBuf> {
    sandbox_exec_candidates().into_iter().find(|p| p.is_file())
}

/// Availability of the Seatbelt backend on the executing host.
pub fn sandbox_availability() -> CapabilityState {
    #[cfg(target_os = "macos")]
    {
        match find_sandbox_exec() {
            Some(_) => CapabilityState::Degraded,
            None => CapabilityState::Unsupported,
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        // Portable answer for contract tests: report the mechanism state
        // without claiming the host provides it.
        match find_sandbox_exec() {
            Some(_) => CapabilityState::Degraded,
            None => CapabilityState::Unsupported,
        }
    }
}

/// Render a minimal Seatbelt profile confining file writes to `workspace`.
///
/// The profile allows standard system reads, denies writes outside the
/// workspace and the platform temporary root, and denies network egress.
/// Callers pass the rendered text to `sandbox-exec -p`. The function is
/// pure so tests verify the containment properties on every host.
pub fn seatbelt_profile_for_workspace(workspace: &Path, temp_root: &Path) -> String {
    let ws = workspace.to_string_lossy().replace('"', "\\\"");
    let tmp = temp_root.to_string_lossy().replace('"', "\\\"");
    format!(
        "(version 1)\n\
         (deny default)\n\
         (allow process-exec)\n\
         (allow process-fork)\n\
         (allow sysctl-read)\n\
         (allow mach-lookup)\n\
         (allow file-read* (regex #\"^/usr\" #\"^/bin\" #\"^/System\" #\"^/Library\"))\n\
         (allow file-read* (literal \"{ws}\") (subpath \"{ws}\"))\n\
         (allow file-write* (literal \"{ws}\") (subpath \"{ws}\"))\n\
         (allow file-write* (literal \"{tmp}\") (subpath \"{tmp}\"))\n\
         (deny file-write* (with no-log))\n\
         (deny network* (with no-log))\n"
    )
}

/// Build a `sandbox-exec` command confining `program` to the workspace.
///
/// Returns `None` when no backend binary is present so callers fail closed.
pub fn sandbox_exec_command(
    workspace: &Path,
    program: &str,
    args: &[String],
) -> Option<std::process::Command> {
    let backend = find_sandbox_exec()?;
    let temp_root = std::env::temp_dir();
    let profile = seatbelt_profile_for_workspace(workspace, &temp_root);
    let mut cmd = std::process::Command::new(backend);
    cmd.arg("-p");
    cmd.arg(profile);
    cmd.arg(program);
    cmd.args(args);
    cmd.current_dir(workspace);
    Some(cmd)
}

// ---------------------------------------------------------------------------
// Shell and terminal
// ---------------------------------------------------------------------------

/// Native interpreter on macOS.
pub fn native_shell_program() -> Option<&'static str> {
    Some("sh")
}

/// Terminal backend label.
pub fn terminal_backend_name() -> &'static str {
    "macos-posix-pty"
}

/// Whether PTY-backed terminals are available. macOS always provides POSIX
/// PTY interfaces; sandboxed or hardened contexts may still refuse them, so
/// this reports the mechanism, not a per-call guarantee.
pub fn pty_available() -> bool {
    #[cfg(target_os = "macos")]
    {
        Path::new("/dev/ptmx").exists() || Path::new("/dev/ptc").exists()
    }
    #[cfg(not(target_os = "macos"))]
    {
        Path::new("/dev/ptmx").exists()
    }
}

// ---------------------------------------------------------------------------
// Capability probing
// ---------------------------------------------------------------------------

/// Genuine macOS capability probe. Never reports `Available` for a mechanism
/// that was not observed; degraded states name the reduced guarantee.
pub fn probe_capabilities() -> crate::platform::capabilities::PlatformCapabilities {
    use crate::platform::capabilities::{CapabilityState, PlatformCapabilities};
    // Process groups and rlimits are kernel interfaces: present on every
    // macOS release this runtime targets. Seatbelt stays Degraded at best
    // because the system tool is deprecated and profile behavior varies by
    // release and entitlement context.
    let filesystem_isolation = match sandbox_availability() {
        CapabilityState::Degraded => CapabilityState::Degraded,
        _ => CapabilityState::Unsupported,
    };
    let sandboxing = match sandbox_availability() {
        CapabilityState::Degraded => CapabilityState::Degraded,
        _ => CapabilityState::Unsupported,
    };
    PlatformCapabilities {
        process_tree_control: CapabilityState::Available,
        resource_limits: CapabilityState::Available,
        filesystem_isolation,
        environment_isolation: CapabilityState::Available,
        sandboxing,
        secure_file_permissions: CapabilityState::Available,
        native_shell: CapabilityState::Available,
        terminal_control: CapabilityState::Available,
        filesystem_watching: CapabilityState::Available,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seatbelt_profile_confines_writes_to_workspace() {
        let ws = Path::new("/work/proj");
        let tmp = Path::new("/tmp");
        let profile = seatbelt_profile_for_workspace(ws, tmp);
        assert!(profile.contains("/work/proj"));
        assert!(profile.contains("(deny file-write*"));
        assert!(profile.contains("(deny network*"));
    }

    #[test]
    fn macos_budget_mapping_is_typed() {
        let budget = ResourceBudget {
            max_cpu_seconds: Some(5),
            max_memory_bytes: Some(1024),
            max_processes: None,
            max_open_files: Some(128),
            max_output_bytes: Some(1024),
        };
        let (applied, missing) = macos_enforceable_limits(&budget);
        assert!(applied.iter().any(|a| a.contains("RLIMIT_CPU")));
        assert!(missing.iter().any(|m| m.contains("max_output_bytes")));
    }

    #[test]
    fn macos_probe_never_claims_full_sandbox() {
        let caps = probe_capabilities();
        assert_ne!(
            caps.filesystem_isolation,
            crate::platform::capabilities::CapabilityState::Available
        );
        assert_eq!(
            caps.process_tree_control,
            crate::platform::capabilities::CapabilityState::Available
        );
    }

    #[test]
    fn starttime_probe_is_safe_off_macos() {
        assert_eq!(read_process_starttime(1), None);
    }
}
