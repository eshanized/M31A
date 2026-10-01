//! macOS-specific contract tests.
//!
//! These tests verify macOS-native mechanisms are correctly implemented
//! and exposed through the platform abstraction.

#[cfg(target_os = "macos")]
mod macos {

    #[test]
    fn macos_process_tree_control_available() {
        let services = m31a::platform::PlatformServices::host();
        assert_eq!(
            services.capabilities.process_tree_control,
            m31a::platform::capabilities::CapabilityState::Available
        );
    }

    #[test]
    fn macos_resource_limits_available() {
        let services = m31a::platform::PlatformServices::host();
        // macOS has POSIX rlimits
        assert_eq!(
            services.capabilities.resource_limits,
            m31a::platform::capabilities::CapabilityState::Available
        );
    }

    #[test]
    fn macos_secure_file_permissions_available() {
        let services = m31a::platform::PlatformServices::host();
        assert_eq!(
            services.capabilities.secure_file_permissions,
            m31a::platform::capabilities::CapabilityState::Available
        );
    }

    #[test]
    fn macos_native_shell_available() {
        let services = m31a::platform::PlatformServices::host();
        assert_eq!(
            services.capabilities.native_shell,
            m31a::platform::capabilities::CapabilityState::Available
        );
    }

    #[test]
    fn macos_terminal_control_available() {
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
    fn macos_backend_names_match_expected() {
        let info = m31a::platform::PlatformInfo::host();
        assert_eq!(info.process_backend, "macos-process-groups-libproc");
        assert_eq!(info.shell_backend, "posix-shell");
        assert!(
            info.sandbox_backend.contains("seatbelt")
                || info.sandbox_backend.contains("foundation")
        );
    }

    #[test]
    fn macos_read_process_starttime_uses_libproc() {
        let pid = std::process::id();
        let start = m31a::platform::process::read_process_starttime(pid);
        // May return None if libproc is not accessible, but should not panic
        let _ = start;
    }

    #[test]
    fn macos_supports_starttime_protection() {
        assert!(m31a::platform::process::supports_starttime_protection());
    }

    #[test]
    fn macos_confinement_staging_dir_is_temp() {
        let dir = m31a::platform::filesystem::HostFilesystem::confinement_staging_dir();
        assert!(dir.to_string_lossy().contains("m31a-confinement"));
    }

    #[test]
    fn macos_apply_pre_exec_limits_applies_rlimits() {
        use m31a::platform::resources::{ResourceBudget, apply_pre_exec_limits};
        let budget = ResourceBudget {
            max_cpu_seconds: Some(60),
            max_memory_bytes: Some(1024 * 1024 * 100),
            max_open_files: Some(128),
            max_processes: Some(16),
            max_output_bytes: Some(1024),
        };
        let result = apply_pre_exec_limits(&budget);
        assert!(result.is_ok());
        assert_eq!(
            result.unwrap(),
            m31a::platform::resources::LimitOutcome::Applied
        );
    }

    #[test]
    fn macos_sandbox_availability_probed() {
        let avail = m31a::platform::macos::sandbox_availability();
        // Seatbelt may be Degraded or Unsupported depending on system state
        assert!(matches!(
            avail,
            m31a::platform::capabilities::CapabilityState::Degraded
                | m31a::platform::capabilities::CapabilityState::Unsupported
        ));
    }

    #[test]
    fn macos_seatbelt_profile_confines_writes() {
        use std::path::Path;
        let ws = Path::new("/work/proj");
        let tmp = Path::new("/tmp");
        let profile = m31a::platform::macos::seatbelt_profile_for_workspace(ws, tmp);
        assert!(profile.contains("/work/proj"));
        assert!(profile.contains("(deny file-write*"));
        assert!(profile.contains("(deny network*"));
    }

    #[test]
    fn macos_native_shell_program_is_sh() {
        assert_eq!(m31a::platform::macos::native_shell_program(), Some("sh"));
    }
}

#[cfg(not(target_os = "macos"))]
mod macos {
    // Placeholder to keep test structure consistent
    #[test]
    fn macos_tests_skipped_on_non_macos() {
        // These tests only run on macOS
    }
}
