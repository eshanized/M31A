//! Windows native backend aggregator.
//!
//! Re-exports the focused Windows mechanisms (process containment via Job
//! Objects, path security, shell backends, credential-file ACLs, ConPTY
//! terminals) and provides the genuine capability probe used on Windows
//! hosts. Pure reasoning compiles everywhere; system bindings stay behind
//! `cfg(windows)`.

pub mod acl;
pub mod conpty;
pub mod job;
pub mod path;
pub mod shell;

pub use acl::{AclOutcome, ensure_private_file_windows, intended_acl_description};
pub use conpty::{ConsoleSize, conpty_available, terminal_capabilities};
pub use job::{JobLimitDescription, describe_job_limits, job_objects_available, tree_guarantee};
pub use path::{
    drive_letter, is_absolute_windows_path, is_reserved_name, is_unc_path, is_within_workspace,
    lexical_normalize_windows, normalize_separators, paths_identical_windows,
    strip_extended_prefix, validate_containment,
};
pub use shell::{
    ShellBackend, argv_preserves_boundaries, build_cmd_command, build_powershell_command,
    contains_cmd_metachars, contains_powershell_metachars, is_windows_shell_program,
    preferred_backend, quote_cmd_arg, quote_powershell_arg,
};

use crate::platform::capabilities::{CapabilityState, PlatformCapabilities};

/// Genuine Windows capability probe.
///
/// Reports `Available` only for mechanisms observed on the executing host:
/// Job Objects via a safe create-and-close probe, ConPTY via entry-point
/// resolution, shells via program lookup, ACLs via the security API.
/// Anything unobserved stays `Degraded` or `Unsupported` so execution fails
/// closed rather than assuming containment.
pub fn probe_capabilities() -> PlatformCapabilities {
    let job = job_objects_available();
    let conpty = conpty_available();
    let shell_present = windows_shell_present();
    PlatformCapabilities {
        process_tree_control: if job {
            CapabilityState::Available
        } else {
            CapabilityState::Degraded
        },
        resource_limits: if job {
            CapabilityState::Available
        } else {
            CapabilityState::Unsupported
        },
        filesystem_isolation: CapabilityState::Unsupported,
        environment_isolation: CapabilityState::Available,
        sandboxing: CapabilityState::Unsupported,
        secure_file_permissions: if acl::supports_private_files() {
            CapabilityState::Available
        } else {
            // Off-host answer for contract tests: the mechanism exists as a
            // native implementation but this host cannot enforce it.
            CapabilityState::Unsupported
        },
        native_shell: if shell_present {
            CapabilityState::Available
        } else {
            CapabilityState::Unsupported
        },
        terminal_control: if conpty {
            CapabilityState::Available
        } else {
            CapabilityState::Degraded
        },
        filesystem_watching: CapabilityState::Available,
    }
}

/// Whether a native shell interpreter resolves on the executing host.
pub fn windows_shell_present() -> bool {
    #[cfg(windows)]
    {
        windows_shell_present_native()
    }
    #[cfg(not(windows))]
    {
        false
    }
}

#[cfg(windows)]
fn windows_shell_present_native() -> bool {
    // PATH lookup for cmd.exe without executing anything.
    if let Ok(path_var) = std::env::var("PATH") {
        for dir in std::env::split_paths(&path_var) {
            if dir.join("cmd.exe").is_file() {
                return true;
            }
        }
    }
    std::path::Path::new("C:\\Windows\\System32\\cmd.exe").is_file()
}

/// Process backend label for diagnostics.
pub fn process_backend_name() -> &'static str {
    "windows-job-objects"
}

/// Shell backend label for diagnostics.
pub fn shell_backend_name() -> &'static str {
    "windows-cmd-powershell"
}

/// Sandbox backend label for diagnostics.
pub fn sandbox_backend_name() -> &'static str {
    "job-object-containment"
}
