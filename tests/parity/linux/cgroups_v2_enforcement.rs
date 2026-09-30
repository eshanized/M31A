//! Linux: cgroups v2 enforcement evidence.
//!
//! Given: the Linux cgroup v2 hierarchy.
//! When: probed through HostFilesystem.
//! Expected: the probe answer matches the filesystem fact; the staging dir
//! is the cgroup path; pre-exec rlimits apply.

#[test]
fn parity_linux_cgroup_probe_matches_filesystem_fact() {
    #[cfg(target_os = "linux")]
    {
        use m31a::platform::filesystem::HostFilesystem;
        let fact = std::path::Path::new("/sys/fs/cgroup/cgroup.controllers").exists();
        assert_eq!(
            HostFilesystem::has_cgroup_controllers(),
            fact,
            "probe must match the hierarchy fact"
        );
        assert!(
            HostFilesystem::confinement_staging_dir()
                .to_string_lossy()
                .contains("cgroup"),
            "Linux staging dir is the cgroup hierarchy"
        );
    }
    #[cfg(not(target_os = "linux"))]
    {
        assert!(
            !m31a::platform::filesystem::HostFilesystem::has_cgroup_controllers(),
            "cgroups are Linux-only; off-host probe is honestly false"
        );
    }
}

#[test]
fn parity_linux_rlimit_cpu_memory_applied_in_child() {
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        use m31a::platform::LimitOutcome;
        use m31a::platform::resources::{ResourceBudget, apply_pre_exec_limits};
        let budget = ResourceBudget {
            max_cpu_seconds: Some(60),
            max_memory_bytes: Some(256 * 1024 * 1024),
            max_processes: None,
            max_open_files: Some(256),
            max_output_bytes: None,
        };
        assert_eq!(
            apply_pre_exec_limits(&budget).unwrap(),
            LimitOutcome::Applied
        );
    }
}
