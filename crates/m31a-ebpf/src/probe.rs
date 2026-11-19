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
    /// Platform is unsupported or no operational kernel event source is available
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
                    "eBPF kernel tracing prerequisites available (Kernel: {}, BTF: {})",
                    kernel_version, btf_available
                )
            }
            Self::DisabledByKernelSysctl { sysctl_value } => {
                format!(
                    "Unprivileged eBPF loading disabled by kernel sysctl (unprivileged_bpf_disabled={}); kernel auditing is unavailable.",
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
            if val != 0 && !Self::has_required_capability() {
                return BpfCapabilityStatus::DisabledByKernelSysctl { sysctl_value: val };
            }
        }

        // 2. Check BTF availability
        let btf_available = Path::new(Self::BTF_VMLINUX_PATH).exists();

        // Check the effective capability set, not UID. Root without CAP_BPF/CAP_SYS_ADMIN
        // must not be reported as privileged, while non-root processes with either
        // effective capability may proceed to the remaining checks.
        if !Self::has_required_capability() {
            return BpfCapabilityStatus::PrivilegeRequired {
                reason: "eBPF tracepoint/kprobe attachment requires effective CAP_BPF or CAP_SYS_ADMIN.".to_string(),
            };
        }

        if !btf_available {
            return BpfCapabilityStatus::BtfUnavailable;
        }

        // Kernel prerequisites alone do not mean that auditing is operational.
        // This crate has no loaded BPF object or kernel ring-buffer reader, so
        // never claim kernel observation is supported.
        BpfCapabilityStatus::UnsupportedPlatform {
            platform: "Linux".to_string(),
            details: format!(
                "kernel prerequisites are present ({}; BTF available), but this build has no eBPF program loader or kernel event reader; no kernel events are collected",
                kernel_version
            ),
        }
    }

    #[cfg(target_os = "linux")]
    fn has_required_capability() -> bool {
        let status = match std::fs::read_to_string("/proc/self/status") {
            Ok(status) => status,
            Err(_) => return false,
        };
        let cap_eff = match status
            .lines()
            .find_map(|line| line.strip_prefix("CapEff:"))
            .map(str::trim)
        {
            Some(value) => value,
            None => return false,
        };
        let effective = match u64::from_str_radix(cap_eff, 16) {
            Ok(value) => value,
            Err(_) => return false,
        };

        const CAP_SYS_ADMIN: u32 = 21;
        const CAP_BPF: u32 = 39;
        (effective & (1u64 << CAP_SYS_ADMIN) != 0)
            || (effective & (1u64 << CAP_BPF) != 0)
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
        // No kernel event producer is shipped yet, so the probe must never
        // claim that kernel syscall auditing is operational.
        #[cfg(target_os = "linux")]
        assert!(
            !status.is_supported(),
            "eBPF must remain unavailable without an attached kernel event source: {:?}",
            status
        );
    }
}
