//! Linux eBPF Kernel Capability and Privilege Probing.
//!
//! Evaluates host kernel support, BTF availability, sysctl policies,
//! and process capability bounds before attempting eBPF program attachment.

use serde::{Deserialize, Serialize};
use std::path::Path;

/// Status of host eBPF capabilities and execution auditing prerequisites.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BpfCapabilityStatus {
    /// Kernel supports eBPF tracing, BTF is available, and process has adequate permissions
    Supported {
        kernel_version: String,
        btf_available: bool,
    },
    /// Unprivileged eBPF is disabled by kernel sysctl (`/proc/sys/kernel/unprivileged_bpf_disabled`)
    DisabledByKernelSysctl { sysctl_value: i32 },
    /// Process lacks `CAP_BPF` or `CAP_SYS_ADMIN` required to load tracing programs
    PrivilegeRequired { reason: String },
    /// Kernel BTF debugging information (`/sys/kernel/btf/vmlinux`) is not present
    BtfUnavailable,
    /// Operating system is not Linux or kernel is older than 4.18
    UnsupportedPlatform { platform: String, details: String },
}

impl BpfCapabilityStatus {
    pub fn is_supported(&self) -> bool {
        matches!(self, Self::Supported { .. })
    }

    pub fn description(&self) -> String {
        match self {
            Self::Supported {
                kernel_version,
                btf_available,
            } => {
                format!(
                    "eBPF tracing fully supported (Kernel: {}, BTF: {})",
                    kernel_version, btf_available
                )
            }
            Self::DisabledByKernelSysctl { sysctl_value } => {
                format!(
                    "Unprivileged eBPF disabled by kernel sysctl (unprivileged_bpf_disabled={}). Auditing operating in unprivileged-degraded mode.",
                    sysctl_value
                )
            }
            Self::PrivilegeRequired { reason } => {
                format!("eBPF auditing requires elevated capability: {reason}")
            }
            Self::BtfUnavailable => {
                "eBPF BTF type metadata (/sys/kernel/btf/vmlinux) unavailable on this kernel."
                    .to_string()
            }
            Self::UnsupportedPlatform { platform, details } => {
                format!(
                    "eBPF execution auditing unavailable on {}: {}",
                    platform, details
                )
            }
        }
    }
}

pub struct BpfProbe;

impl BpfProbe {
    pub const SYSCTL_UNPRIVILEGED_BPF: &'static str = "/proc/sys/kernel/unprivileged_bpf_disabled";
    pub const BTF_VMLINUX_PATH: &'static str = "/sys/kernel/btf/vmlinux";

    /// Performs capability probing against the running host environment.
    pub fn probe() -> BpfCapabilityStatus {
        #[cfg(not(target_os = "linux"))]
        {
            BpfCapabilityStatus::UnsupportedPlatform {
                platform: std::env::consts::OS.to_string(),
                details: "eBPF kernel-level auditing is only supported on Linux kernels"
                    .to_string(),
            }
        }

        #[cfg(target_os = "linux")]
        {
            Self::probe_linux()
        }
    }

    #[cfg(target_os = "linux")]
    fn probe_linux() -> BpfCapabilityStatus {
        let kernel_version = Self::read_kernel_release().unwrap_or_else(|| "unknown".to_string());

        // 1. Check sysctl unprivileged_bpf_disabled
        if let Ok(val_str) = std::fs::read_to_string(Self::SYSCTL_UNPRIVILEGED_BPF) {
            let val: i32 = val_str.trim().parse().unwrap_or(0);
            if val != 0 {
                // If unprivileged BPF is disabled and process is not root (UID != 0)
                let uid = unsafe { libc::getuid() };
                if uid != 0 {
                    return BpfCapabilityStatus::DisabledByKernelSysctl { sysctl_value: val };
                }
            }
        }

        // 2. Check BTF availability
        let btf_available = Path::new(Self::BTF_VMLINUX_PATH).exists();

        // 3. Check privileges
        let uid = unsafe { libc::getuid() };
        let euid = unsafe { libc::geteuid() };
        if uid != 0 && euid != 0 {
            // Unprivileged user: eBPF tracing programs (kprobes/tracepoints) require CAP_BPF / CAP_SYS_ADMIN
            return BpfCapabilityStatus::PrivilegeRequired {
                reason: "eBPF tracepoint/kprobe attachment requires CAP_BPF or CAP_SYS_ADMIN. M31A runs unprivileged by design.".to_string(),
            };
        }

        if !btf_available {
            return BpfCapabilityStatus::BtfUnavailable;
        }

        BpfCapabilityStatus::Supported {
            kernel_version,
            btf_available,
        }
    }

    #[cfg(target_os = "linux")]
    fn read_kernel_release() -> Option<String> {
        let mut uts = std::mem::MaybeUninit::<libc::utsname>::uninit();
        if unsafe { libc::uname(uts.as_mut_ptr()) } == 0 {
            let uts = unsafe { uts.assume_init() };
            let release_cstr = unsafe { std::ffi::CStr::from_ptr(uts.release.as_ptr()) };
            Some(release_cstr.to_string_lossy().to_string())
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bpf_probe_runs_without_panic() {
        let status = BpfProbe::probe();
        let desc = status.description();
        assert!(!desc.is_empty());
        // On our Linux environment with unprivileged_bpf_disabled = 2 and UID 1000,
        // it should detect DisabledByKernelSysctl or PrivilegeRequired.
        #[cfg(target_os = "linux")]
        {
            let uid = unsafe { libc::getuid() };
            if uid != 0 {
                assert!(
                    matches!(
                        status,
                        BpfCapabilityStatus::DisabledByKernelSysctl { .. }
                            | BpfCapabilityStatus::PrivilegeRequired { .. }
                    ),
                    "Expected DisabledByKernelSysctl or PrivilegeRequired for unprivileged test run, got {:?}",
                    status
                );
            }
        }
    }
}
