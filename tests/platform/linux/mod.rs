//! Linux-specific contract tests.
//!
//! These tests verify Linux-native mechanisms are correctly implemented
//! and exposed through the platform abstraction.

#[cfg(target_os = "linux")]
mod linux {
    #[test]
    fn linux_process_tree_control_available() {
        let services = m31a::platform::PlatformServices::host();
        assert_eq!(
            services.capabilities.process_tree_control,
            m31a::platform::capabilities::CapabilityState::Available
        );
    }

    #[test]
    fn linux_resource_limits_available() {
        let services = m31a::platform::PlatformServices::host();
        // Linux has rlimits and cgroups
        assert_eq!(
            services.capabilities.resource_limits,
            m31a::platform::capabilities::CapabilityState::Available
        );
    }

    #[test]
    fn linux_secure_file_permissions_available() {
        let services = m31a::platform::PlatformServices::host();
        assert_eq!(
            services.capabilities.secure_file_permissions,
            m31a::platform::capabilities::CapabilityState::Available
        );
    }

    #[test]
    fn linux_native_shell_available() {
        let services = m31a::platform::PlatformServices::host();
        assert_eq!(
            services.capabilities.native_shell,
            m31a::platform::capabilities::CapabilityState::Available
        );
    }

    #[test]
    fn linux_terminal_control_available() {
        let services = m31a::platform::PlatformServices::host();
        assert_eq!(
            services.capabilities.terminal_control,
            m31a::platform::capabilities::CapabilityState::Available
        );
        assert_eq!(
            services.terminal.pty,
            m31a::platform::capabilities::CapabilityState::Available
        );
    }

    #[test]
    fn linux_backend_names_match_expected() {
        let info = m31a::platform::PlatformInfo::host();
        assert_eq!(info.process_backend, "linux-process-groups");
        assert_eq!(info.shell_backend, "posix-shell");
        assert!(
            info.sandbox_backend.contains("bubblewrap")
                || info.sandbox_backend.contains("fallback")
        );
    }

    #[test]
    fn linux_read_process_starttime_works() {
        let pid = std::process::id();
        let start = m31a::platform::process::read_process_starttime(pid)
            .expect("Linux current process must have readable start time from /proc/self/stat");
        assert!(start > 0, "process start time ticks must be positive");

        let start2 = m31a::platform::process::read_process_starttime(pid)
            .expect("subsequent read must also succeed");
        assert_eq!(
            start, start2,
            "process start time must be immutable across reads"
        );

        // Non-existent PID must return None without panicking
        let fake_pid = 4_194_304;
        assert_eq!(
            m31a::platform::process::read_process_starttime(fake_pid),
            None,
            "non-existent process must return None"
        );
    }

    #[test]
    fn linux_supports_starttime_protection() {
        assert!(m31a::platform::process::supports_starttime_protection());
    }

    #[test]
    fn linux_cgroup_controllers_detected() {
        let expected = std::path::Path::new("/sys/fs/cgroup/cgroup.controllers").exists();
        let has_cgroups = m31a::platform::filesystem::HostFilesystem::has_cgroup_controllers();
        assert_eq!(
            has_cgroups, expected,
            "has_cgroup_controllers must match actual presence of /sys/fs/cgroup/cgroup.controllers"
        );
    }

    #[test]
    fn linux_user_namespaces_probe() {
        let gate = std::path::Path::new("/proc/sys/kernel/unprivileged_userns_clone");
        let expected = if std::fs::read_to_string(gate).is_ok_and(|c| c.trim() == "0") {
            false
        } else {
            std::path::Path::new("/proc/self/ns/user").exists()
        };
        let has_userns = m31a::platform::filesystem::HostFilesystem::has_user_namespaces();
        assert_eq!(
            has_userns, expected,
            "has_user_namespaces probe must match kernel interface state"
        );
    }

    #[test]
    fn linux_confinement_staging_dir_is_cgroup() {
        let dir = m31a::platform::filesystem::HostFilesystem::confinement_staging_dir();
        assert!(dir.to_string_lossy().contains("cgroup"));
    }

    #[test]
    fn linux_apply_pre_exec_limits_applies_rlimits() {
        use m31a::platform::resources::{ResourceBudget, apply_pre_exec_limits};
        use std::os::unix::process::CommandExt;
        let budget = ResourceBudget {
            max_cpu_seconds: Some(60),
            max_memory_bytes: None,
            max_open_files: Some(128),
            max_processes: None,
            max_output_bytes: Some(1024),
        };
        let mut cmd = std::process::Command::new("sh");
        cmd.arg("-c").arg("true");
        unsafe {
            let b = budget.clone();
            cmd.pre_exec(move || match apply_pre_exec_limits(&b) {
                Ok(_) => Ok(()),
                Err(e) => Err(e),
            });
        }
        let mut child = cmd.spawn().expect("spawn child with limits");
        let status = child.wait().expect("wait on child");
        assert!(
            status.success(),
            "child runs successfully with applied limits"
        );
    }
}

#[cfg(not(target_os = "linux"))]
mod linux {
    // Placeholder to keep test structure consistent
    #[test]
    fn linux_tests_skipped_on_non_linux() {
        // These tests only run on Linux
    }
}
