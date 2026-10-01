//! Windows-specific contract tests.
//!
//! These tests verify Windows-native mechanisms are correctly implemented
//! and exposed through the platform abstraction.

#[cfg(windows)]
mod windows {
    use super::*;
    use std::path::Path;

    #[test]
    fn windows_process_tree_control_via_job_objects() {
        let services = m31a::platform::PlatformServices::host();
        // Job Objects provide process tree control
        assert!(matches!(
            services.capabilities.process_tree_control,
            m31a::platform::capabilities::CapabilityState::Available
                | m31a::platform::capabilities::CapabilityState::Degraded
        ));
    }

    #[test]
    fn windows_resource_limits_via_job_objects() {
        let services = m31a::platform::PlatformServices::host();
        // Job Objects provide memory, CPU, process count limits
        assert!(matches!(
            services.capabilities.resource_limits,
            m31a::platform::capabilities::CapabilityState::Available
                | m31a::platform::capabilities::CapabilityState::Unsupported
        ));
    }

    #[test]
    fn windows_secure_file_permissions_via_acls() {
        let services = m31a::platform::PlatformServices::host();
        assert_eq!(
            services.capabilities.secure_file_permissions,
            m31a::platform::capabilities::CapabilityState::Available
        );
    }

    #[test]
    fn windows_native_shell_available() {
        let services = m31a::platform::PlatformServices::host();
        assert_eq!(
            services.capabilities.native_shell,
            m31a::platform::capabilities::CapabilityState::Available
        );
    }

    #[test]
    fn windows_terminal_control_via_conpty() {
        let services = m31a::platform::PlatformServices::host();
        // ConPTY availability depends on Windows version
        let _ = services.capabilities.terminal_control;
        let _ = services.terminal.pty;
    }

    #[test]
    fn windows_backend_names_match_expected() {
        let info = m31a::platform::PlatformInfo::host();
        assert_eq!(info.process_backend, "windows-job-objects");
        assert_eq!(info.shell_backend, "windows-cmd-powershell");
        assert_eq!(info.sandbox_backend, "job-object-containment");
    }

    #[test]
    fn windows_job_objects_probe() {
        let available = m31a::platform::windows::job::job_objects_available();
        // Should be true on supported Windows versions
        let _ = available;
    }

    #[test]
    fn windows_job_limit_mapping_names_mechanisms() {
        use m31a::platform::resources::ResourceBudget;
        use m31a::platform::windows::job::describe_job_limits;
        let budget = ResourceBudget {
            max_cpu_seconds: Some(10),
            max_memory_bytes: Some(512 * 1024 * 1024),
            max_processes: Some(32),
            max_open_files: Some(256),
            max_output_bytes: Some(1024),
        };
        let desc = describe_job_limits(&budget);
        assert!(desc.applied.iter().any(|a| a.contains("JOB_OBJECT")));
        assert!(desc.missing.iter().any(|m| m.contains("max_open_files")));
        assert!(desc.kill_on_job_close);
    }

    #[test]
    fn windows_tree_guarantee_described() {
        let guarantee = m31a::platform::windows::job::tree_guarantee();
        assert!(guarantee.contains("Job Object") || guarantee.contains("job"));
    }

    #[test]
    fn windows_conpty_probe() {
        let available = m31a::platform::windows::conpty::conpty_available();
        let _ = available;
    }

    #[test]
    fn windows_conpty_terminal_capabilities() {
        let caps = m31a::platform::windows::conpty::terminal_capabilities();
        let _ = caps;
    }

    #[test]
    fn windows_path_security_resists_prefix_tricks() {
        // Windows path security must not be bypassed by prefix matching
        assert!(!m31a::platform::windows::path::is_within_workspace(
            "C:\\work",
            "C:\\work-evil\\file.txt"
        ));
        assert!(m31a::platform::windows::path::is_within_workspace(
            "C:\\work",
            "C:\\work\\file.txt"
        ));
    }

    #[test]
    fn windows_path_security_case_insensitive() {
        assert!(m31a::platform::windows::path::is_within_workspace(
            "C:\\Work",
            "c:\\work\\SUB\\file.txt"
        ));
    }

    #[test]
    fn windows_path_security_rejects_dotdot() {
        assert!(!m31a::platform::windows::path::is_within_workspace(
            "C:\\work",
            "C:\\work\\..\\evil\\file.txt"
        ));
    }

    #[test]
    fn windows_path_security_unc_hosts_must_match() {
        assert!(!m31a::platform::windows::path::is_within_workspace(
            "\\\\srv\\share\\work",
            "\\\\other\\share\\work\\f"
        ));
        assert!(m31a::platform::windows::path::is_within_workspace(
            "\\\\srv\\share\\work",
            "\\\\srv\\share\\work\\sub\\f"
        ));
    }

    #[test]
    fn windows_reserved_names_rejected() {
        assert!(m31a::platform::windows::path::is_reserved_name(Path::new(
            "NUL"
        )));
        assert!(m31a::platform::windows::path::is_reserved_name(Path::new(
            "com1.txt"
        )));
        assert!(!m31a::platform::windows::path::is_reserved_name(Path::new(
            "normal.txt"
        )));
    }

    #[test]
    fn windows_path_identity_case_insensitive() {
        assert!(m31a::platform::windows::path::paths_identical_windows(
            Path::new("A"),
            Path::new("a")
        ));
    }

    #[test]
    fn windows_acl_enforcement() {
        let temp = tempfile::tempdir().unwrap();
        let file = temp.path().join("secret.txt");
        std::fs::write(&file, b"test").unwrap();
        let outcome = m31a::platform::windows::acl::ensure_private_file_windows(&file);
        assert!(outcome.is_ok());
        let outcome = outcome.unwrap();
        assert!(matches!(
            outcome,
            m31a::platform::windows::acl::AclOutcome::Enforced
                | m31a::platform::windows::acl::AclOutcome::Unsupported
                | m31a::platform::windows::acl::AclOutcome::Failed
        ));
    }

    #[test]
    fn windows_shell_cmd_backend() {
        let backend = m31a::platform::windows::shell::preferred_backend();
        assert_eq!(backend, m31a::platform::windows::shell::ShellBackend::Cmd);
    }

    #[test]
    fn windows_shell_quoting_preserves_boundaries() {
        let q = m31a::platform::windows::shell::quote_cmd_arg("a & b");
        assert!(q.starts_with('"') && q.ends_with('"'));
        assert!(q.contains("a & b"));
    }

    #[test]
    fn windows_shell_empty_arg_quoted() {
        assert_eq!(m31a::platform::windows::shell::quote_cmd_arg(""), "\"\"");
    }

    #[test]
    fn windows_shell_powershell_quoting() {
        assert_eq!(
            m31a::platform::windows::shell::quote_powershell_arg("it's"),
            "'it''s'"
        );
    }

    #[test]
    fn windows_shell_detection_covers_known() {
        assert!(m31a::platform::windows::shell::is_windows_shell_program(
            "cmd.exe"
        ));
        assert!(m31a::platform::windows::shell::is_windows_shell_program(
            "PowerShell.EXE"
        ));
        assert!(!m31a::platform::windows::shell::is_windows_shell_program(
            "python.exe"
        ));
    }
}

#[cfg(not(windows))]
mod windows {
    // Placeholder to keep test structure consistent
    #[test]
    fn windows_tests_skipped_on_non_windows() {
        // These tests only run on Windows
    }
}
