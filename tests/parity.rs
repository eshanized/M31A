//! Phase 49 behavioral parity suite.
//!
//! Contract-first tests: each test names the M31A semantic requirement under
//! test, drives the native mechanism for the executing host, observes the
//! result, and records the parity classification in the test name or
//! assertion message. Implementation-detail checks live in the
//! platform-specific subdirectories.
//!
//! Layout mirrors the Phase 49 specification section 78. The `parity/`
//! directory holds one file per capability; `parity.rs` wires them into a
//! single test binary so `cargo test --test parity` runs the whole suite.

#[path = "parity/process_tree_termination.rs"]
mod parity_process_tree_termination;

#[path = "parity/resource_limits.rs"]
mod parity_resource_limits;

#[path = "parity/filesystem_containment.rs"]
mod parity_filesystem_containment;

#[path = "parity/credential_file_protection.rs"]
mod parity_credential_file_protection;

#[path = "parity/shell_execution.rs"]
mod parity_shell_execution;

#[path = "parity/terminal_pty.rs"]
mod parity_terminal_pty;

#[path = "parity/environment_isolation.rs"]
mod parity_environment_isolation;

#[path = "parity/sandbox_fail_closed.rs"]
mod parity_sandbox_fail_closed;

#[path = "parity/capability_reporting.rs"]
mod parity_capability_reporting;

#[path = "parity/failure_semantics.rs"]
mod parity_failure_semantics;

#[path = "parity/linux/cgroups_v2_enforcement.rs"]
mod parity_linux_cgroups;

#[path = "parity/linux/bubblewrap_isolation.rs"]
mod parity_linux_bubblewrap;

#[path = "parity/linux/user_namespaces.rs"]
mod parity_linux_userns;

#[path = "parity/linux/proc_starttime_protection.rs"]
mod parity_linux_starttime;

#[path = "parity/macos/libproc_pid_identity.rs"]
mod parity_macos_libproc;

#[path = "parity/macos/seatbelt_profile_confinement.rs"]
mod parity_macos_seatbelt;

#[path = "parity/macos/rlimit_as_best_effort.rs"]
mod parity_macos_rlimit;

#[path = "parity/macos/ptmx_pty.rs"]
mod parity_macos_ptmx;

#[path = "parity/windows/job_object_tree_termination.rs"]
mod parity_windows_job_tree;

#[path = "parity/windows/job_object_memory_limit.rs"]
mod parity_windows_job_memory;

#[path = "parity/windows/acl_file_protection.rs"]
mod parity_windows_acl;

#[path = "parity/windows/path_security_reparse_points.rs"]
mod parity_windows_reparse;

#[path = "parity/windows/cmd_powershell_quoting.rs"]
mod parity_windows_quoting;

#[path = "parity/windows/conpty_dimensions_resize.rs"]
mod parity_windows_conpty;

#[path = "parity/negative/unsupported_fd_limit_windows.rs"]
mod parity_negative_fd;

#[path = "parity/negative/unsupported_filesystem_isolation_windows.rs"]
mod parity_negative_fs;

#[path = "parity/negative/unsupported_network_isolation_windows.rs"]
mod parity_negative_net;

#[path = "parity/negative/degraded_seatbelt_macos.rs"]
mod parity_negative_seatbelt;

#[path = "parity/negative/unavailable_job_objects_windows.rs"]
mod parity_negative_job;
