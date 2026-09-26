//! Explicit runtime sandbox capabilities matrix (SND-02, SND-03, D-05).
//!
//! Non-negotiable invariant: Unsupported sandbox guarantees are NEVER
//! represented as implemented guarantees. If a requested capability cannot
//! be satisfied by the provider, the execution must fail closed immediately.

use serde::{Deserialize, Serialize};

/// Verifiable platform and kernel execution isolation capabilities (SND-03, D-05).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SandboxCapabilities {
    /// Kernel read-only root mount preventing host system modifications.
    pub fs_read_only_root: bool,
    /// Isolated read-write workspace root.
    pub fs_isolated_workspace_rw: bool,
    /// Isolated private per-execution tmpfs mounted at /tmp.
    pub fs_private_tempdir: bool,
    /// Automatic masking of user credentials, SSH keys, and cloud tokens.
    pub fs_masked_credentials: bool,
    /// Complete network denial (--unshare-net).
    pub net_deny_all: bool,
    /// Explicit destination allowlisting for outbound connections.
    pub net_allowlist: bool,
    /// Isolated PID namespace hiding host processes.
    pub proc_isolated_pid_ns: bool,
    /// Process group isolation and signal propagation boundaries.
    pub proc_group_isolation: bool,
    /// Unified cgroups v2 resource metering and enforcement.
    pub proc_cgroup_v2: bool,
    /// POSIX rlimit enforcement (CPU, memory, max files, processes).
    pub proc_rlimit: bool,
    /// Linux unprivileged user namespace support.
    pub user_namespaces: bool,
}

impl SandboxCapabilities {
    /// Zero isolation capabilities (representing an uncontained environment).
    pub fn none() -> Self {
        Self {
            fs_read_only_root: false,
            fs_isolated_workspace_rw: false,
            fs_private_tempdir: false,
            fs_masked_credentials: false,
            net_deny_all: false,
            net_allowlist: false,
            proc_isolated_pid_ns: false,
            proc_group_isolation: false,
            proc_cgroup_v2: false,
            proc_rlimit: false,
            user_namespaces: false,
        }
    }

    /// Full isolation capabilities (Linux Bubblewrap with cgroups).
    pub fn all() -> Self {
        Self {
            fs_read_only_root: true,
            fs_isolated_workspace_rw: true,
            fs_private_tempdir: true,
            fs_masked_credentials: true,
            net_deny_all: true,
            net_allowlist: true,
            proc_isolated_pid_ns: true,
            proc_group_isolation: true,
            proc_cgroup_v2: true,
            proc_rlimit: true,
            user_namespaces: true,
        }
    }

    /// Whether this capability set satisfies full filesystem confinement.
    pub fn supports_fs_isolation(&self) -> bool {
        self.fs_read_only_root
            && self.fs_isolated_workspace_rw
            && self.fs_private_tempdir
            && self.fs_masked_credentials
    }

    /// Whether this capability set satisfies network confinement.
    pub fn supports_net_isolation(&self) -> bool {
        self.net_deny_all
    }

    /// Whether this capability set satisfies basic process-group isolation.
    pub fn supports_process_isolation(&self) -> bool {
        self.proc_group_isolation && self.proc_rlimit
    }
}
