//! `m31a-sandbox`: Multi-platform command sandboxing and process isolation engine.
//!
//! Enforces filesystem and network isolation using Linux Bubblewrap and Landlock, macOS Seatbelt,
//! Windows containers, and seccomp filters to ensure safe agent tool execution.

pub mod bwrap_builder;
pub mod daemon_isolation;
pub mod daemon_mounts;
pub mod descriptor_binary;
pub mod fd_mount;
pub mod landlock;
pub mod mxc;
pub mod network_proxy;
pub mod policy_transforms;
pub mod process_hardening;
pub mod process_manager;
pub mod process_reaper;
pub mod pty_runner;
pub mod runner;
pub mod sandbox_integrity;
pub mod sandbox_launcher;
pub mod seatbelt;
pub mod seccomp_filter;
pub mod terminal_queries;
pub mod terminal_sanitizer;
pub mod violation;
pub mod wslg;

pub use bwrap_builder::{
    BwrapMountPlan, BwrapNetworkMode, BwrapOptions, CanonicalSandboxPlan,
    LINUX_PLATFORM_DEFAULT_READ_ROOTS, PROTECTED_METADATA_NAMES, build_bwrap_arguments,
};
pub use daemon_isolation::DaemonIsolation;
pub use daemon_mounts::{SocketFilesystem, check_mounts, daemon_socket_mask_paths, mount_path};
pub use descriptor_binary::{DescriptorBinary, verify_and_pin_binary};
pub use fd_mount::verify_fd_mounts;
pub use network_proxy::{
    ConnectionAuditRecord, NetworkFirewallRule, NetworkProxyFirewall, ProxyDecision,
};
pub use policy_transforms::{
    EffectiveMountPlan, FileSystemAccessMode, FileSystemPermissions, FileSystemSandboxEntry,
    build_effective_mount_plan,
};
pub use process_hardening::{
    DANGEROUS_ENV_PREFIXES, HardeningStatus, SENSITIVE_TOKEN_VARIABLES, apply_process_hardening,
    build_hardened_environment, set_parent_death_signal,
};
pub use process_manager::{
    ProcessInfo, ProcessManager, ProcessStatus, SpawnResult, StdinResult, UNIFIED_EXEC_ENV,
};
pub use process_reaper::{ProcessReapReport, reap_zombies, terminate_process_group};
pub use pty_runner::{PtyProcessHandle, PtySession, PtySize, spawn_command_in_pty};
pub use runner::{CommandOutput, SandboxCommand, SandboxRunner};
pub use sandbox_integrity::{IntegrityFinding, SandboxIntegrityChecker};
pub use sandbox_launcher::{ContainmentStrategy, SandboxEngineCapabilities, SandboxLauncherProbe};
pub use seccomp_filter::{
    NetworkSeccompMode, SeccompAction, SeccompCmpOp, SeccompFilterPlan, SeccompRuleCondition,
    SeccompSyscallRule, determine_seccomp_mode, set_no_new_privs,
};
pub use terminal_queries::TerminalQueryResponder;
pub use terminal_sanitizer::{TerminalSanitizationPolicy, TerminalSanitizer};
pub use violation::{
    FileSystemSandboxViolation, FileSystemSandboxViolationReason, NetworkSandboxViolation,
    OUTPUT_SNIPPET_MAX_CHARS, SandboxViolationBackend, SandboxViolationEvent,
    detect_filesystem_violation,
};
pub use wslg::{WSL_INTEROP_DIR, WSLG_DISTRO_ROOT, ensure_supported_path, is_duplicate_root};
