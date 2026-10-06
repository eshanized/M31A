//! Platform-aware process confinement manager (D-07, SEC-03).
//!
//! Provides defense-in-depth process execution bounds:
//! - Tier 1: Linux control-group enforcement (`memory.max`, `cpu.max`)
//! - Tier 2: Portable resource budgets enforced via the platform boundary
//! - Tier 3: Isolation-scope leadership and watchdog termination
//!
//! Tier selection probes through the platform layer so higher layers reason
//! about budgets rather than kernel interfaces.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::process::Command;

use crate::ids::JobId;
use crate::state::budget::ResourceBudget;

/// Active tier of process confinement detected on the host.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ConfinementTier {
    Tier1CgroupsV2,
    Tier2PosixRlimits,
    TierWindowsJobObjects,
    Tier3WatchdogOnly,
}

/// Manages platform-aware sandboxing and process isolation.
#[derive(Debug, Clone)]
pub struct ConfinementManager {
    tier: ConfinementTier,
    cgroup_base: PathBuf,
}

impl Default for ConfinementManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ConfinementManager {
    /// Probe host environment and detect the highest available confinement tier.
    pub fn new() -> Self {
        let (tier, cgroup_base) = Self::probe_host();
        Self { tier, cgroup_base }
    }

    /// Explicit constructor for testing specific tiers.
    pub fn with_tier(tier: ConfinementTier) -> Self {
        Self {
            tier,
            cgroup_base: crate::platform::filesystem::HostFilesystem::confinement_staging_dir(),
        }
    }

    pub fn tier(&self) -> ConfinementTier {
        self.tier
    }

    fn probe_host() -> (ConfinementTier, PathBuf) {
        if cfg!(target_os = "linux")
            && crate::platform::filesystem::HostFilesystem::has_cgroup_controllers()
        {
            let base = crate::platform::filesystem::HostFilesystem::confinement_staging_dir();
            if std::fs::create_dir_all(&base).is_ok() {
                return (ConfinementTier::Tier1CgroupsV2, base);
            }
        }

        if cfg!(windows) && crate::platform::windows::job::job_objects_available() {
            return (
                ConfinementTier::TierWindowsJobObjects,
                crate::platform::filesystem::HostFilesystem::temp_root().join("m31a-confinement"),
            );
        }

        if cfg!(unix) {
            return (
                ConfinementTier::Tier2PosixRlimits,
                crate::platform::filesystem::HostFilesystem::temp_root().join("m31a-confinement"),
            );
        }

        (
            ConfinementTier::Tier3WatchdogOnly,
            crate::platform::filesystem::HostFilesystem::temp_root().join("m31a-confinement"),
        )
    }

    /// Configure a command with isolation scope and resource bounds.
    pub fn apply_confinement(
        &self,
        cmd: &mut Command,
        limits: &ResourceBudget,
        job_id: &JobId,
    ) -> ConfinementHandle {
        // Tier 3: Always request the platform isolation scope when available.
        crate::platform::process::configure_isolation(cmd);

        // Tier 2: Install the platform resource hook. Enforcement and
        // validation live beneath the contract.
        let budget = crate::platform::resources::ResourceBudget {
            max_cpu_seconds: limits.max_cpu_seconds,
            max_memory_bytes: limits.max_memory_bytes,
            max_processes: None,
            max_open_files: Some(1024),
            max_output_bytes: None,
        };
        crate::platform::resources::install_pre_exec_hook(cmd, budget);

        // Tier 1: Prepare control-group directory if supported
        let mut job_cgroup_dir = None;
        if self.tier == ConfinementTier::Tier1CgroupsV2 {
            let cgroup_dir = self.cgroup_base.join(job_id.to_string());
            if std::fs::create_dir_all(&cgroup_dir).is_ok() {
                if let Some(mem_bytes) = limits.max_memory_bytes {
                    let _ = std::fs::write(cgroup_dir.join("memory.max"), mem_bytes.to_string());
                }
                if let Some(cpu_sec) = limits.max_cpu_seconds {
                    // CPU quota: e.g. 100000 us period, quota scaled by cpu_sec
                    let quota_us = cpu_sec * 100_000;
                    let _ =
                        std::fs::write(cgroup_dir.join("cpu.max"), format!("{} 100000", quota_us));
                }
                job_cgroup_dir = Some(cgroup_dir);
            }
        }

        ConfinementHandle {
            tier: self.tier,
            job_cgroup_dir,
        }
    }
}

/// Handle representing an active confinement scope for a spawned process.
pub struct ConfinementHandle {
    tier: ConfinementTier,
    job_cgroup_dir: Option<PathBuf>,
}

impl ConfinementHandle {
    pub fn tier(&self) -> ConfinementTier {
        self.tier
    }

    /// Attach spawned child process PID to control-group accounting if applicable.
    pub fn attach_pid(&self, pid: u32) {
        if let Some(ref dir) = self.job_cgroup_dir {
            let procs_file = dir.join("cgroup.procs");
            let _ = std::fs::write(procs_file, pid.to_string());
        }
    }

    /// Clean up confinement resources.
    pub fn cleanup(&mut self) {
        if let Some(dir) = self.job_cgroup_dir.take() {
            let _ = std::fs::remove_dir_all(dir);
        }
    }
}

impl Drop for ConfinementHandle {
    fn drop(&mut self) {
        self.cleanup();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_confinement_tier_detection() {
        let manager = ConfinementManager::new();
        // Manager should probe without crashing
        let tier = manager.tier();
        assert!(matches!(
            tier,
            ConfinementTier::Tier1CgroupsV2
                | ConfinementTier::Tier2PosixRlimits
                | ConfinementTier::TierWindowsJobObjects
                | ConfinementTier::Tier3WatchdogOnly
        ));
    }

    #[test]
    fn test_apply_confinement_configures_command() {
        let manager = ConfinementManager::new();
        let mut cmd = Command::new("true");
        let limits = ResourceBudget {
            max_memory_bytes: Some(1024 * 1024 * 512), // 512 MB
            max_cpu_seconds: Some(10),
            ..Default::default()
        };
        let job_id = JobId::new();

        let handle = manager.apply_confinement(&mut cmd, &limits, &job_id);
        assert_eq!(handle.tier(), manager.tier());
    }
}
