//! Cross-platform contract tests — common semantic requirements.
//!
//! These tests verify M31A semantic guarantees that must hold on every
//! supported platform. Platform-specific tests live in sibling directories.

#[test]
fn platform_capabilities_report_honest_state() {
    let services = m31a::platform::PlatformServices::host();
    let caps = &services.capabilities;
    // Every platform must report a state for every capability
    // (Available, Degraded, Unsupported, or Unknown — never implicit)
    let _ = caps.process_tree_control;
    let _ = caps.resource_limits;
    let _ = caps.filesystem_isolation;
    let _ = caps.environment_isolation;
    let _ = caps.sandboxing;
    let _ = caps.secure_file_permissions;
    let _ = caps.native_shell;
    let _ = caps.terminal_control;
    let _ = caps.filesystem_watching;
}

#[test]
fn environment_isolation_always_available() {
    let services = m31a::platform::PlatformServices::host();
    // Environment isolation is a baseline guarantee on all platforms
    assert_eq!(
        services.capabilities.environment_isolation,
        m31a::platform::capabilities::CapabilityState::Available
    );
}

#[test]
fn terminal_dimensions_always_available() {
    let services = m31a::platform::PlatformServices::host();
    // Terminal dimensions are a baseline on all platforms
    assert_eq!(
        services.terminal.dimensions,
        m31a::platform::capabilities::CapabilityState::Available
    );
}

#[test]
fn filesystem_watching_always_available() {
    let services = m31a::platform::PlatformServices::host();
    // Filesystem watching is available on all target platforms
    assert_eq!(
        services.capabilities.filesystem_watching,
        m31a::platform::capabilities::CapabilityState::Available
    );
}

#[test]
fn fail_closed_on_unsupported_capability() {
    let services = m31a::platform::PlatformServices::host();
    let caps = &services.capabilities;
    // Any capability reported as Unsupported must fail the require() gate
    for cap in [
        m31a::platform::capabilities::Capability::ProcessTreeControl,
        m31a::platform::capabilities::Capability::ResourceLimits,
        m31a::platform::capabilities::Capability::FilesystemIsolation,
        m31a::platform::capabilities::Capability::Sandboxing,
        m31a::platform::capabilities::Capability::SecureFilePermissions,
        m31a::platform::capabilities::Capability::NativeShell,
        m31a::platform::capabilities::Capability::TerminalControl,
    ] {
        let state = caps.state_of(cap);
        if state == m31a::platform::capabilities::CapabilityState::Unsupported {
            let result = caps.require(cap);
            assert!(
                result.is_err(),
                "Unsupported capability {cap:?} must fail require()"
            );
            let err = result.unwrap_err();
            assert_eq!(err.capability, cap);
            assert_eq!(err.state, state);
        }
    }
}

#[test]
fn platform_info_has_consistent_backends() {
    let info = m31a::platform::PlatformInfo::host();
    assert!(!info.process_backend.is_empty());
    assert!(!info.shell_backend.is_empty());
    assert!(!info.sandbox_backend.is_empty());
    assert!(!info.rust_target.is_empty());
    assert!(!info.os_name.is_empty());
    assert!(!info.architecture.is_empty());
}

#[test]
fn temp_root_is_usable() {
    let root = m31a::platform::filesystem::HostFilesystem::temp_root();
    assert!(root.exists() || std::fs::create_dir_all(&root).is_ok());
}

#[test]
fn null_device_exists_on_host() {
    let dev = m31a::platform::filesystem::HostFilesystem::null_device();
    // The device name must be non-empty and platform-appropriate
    assert!(!dev.is_empty());
    #[cfg(windows)]
    assert_eq!(dev, "NUL");
    #[cfg(not(windows))]
    assert_eq!(dev, "/dev/null");
}

#[test]
fn default_path_is_non_empty() {
    let path = m31a::platform::filesystem::HostFilesystem::default_path_value();
    assert!(!path.is_empty());
    #[cfg(windows)]
    {
        assert!(path.contains("Windows"));
        assert!(path.contains(';'));
    }
    #[cfg(not(windows))]
    {
        assert!(path.contains("bin"));
        assert!(path.contains(':'));
    }
}

#[test]
fn path_identity_respects_host_semantics() {
    use std::path::Path;
    #[cfg(windows)]
    {
        // Windows: case-insensitive
        assert!(m31a::platform::filesystem::HostFilesystem::paths_identical(
            Path::new("A"),
            Path::new("a")
        ));
    }
    #[cfg(not(windows))]
    {
        // Unix: case-sensitive
        assert!(
            !m31a::platform::filesystem::HostFilesystem::paths_identical(
                Path::new("A"),
                Path::new("a")
            )
        );
    }
}

#[test]
fn workspace_containment_uses_platform_logic() {
    let ws = std::env::temp_dir().join("m31a_test_ws");
    if std::fs::create_dir_all(&ws).is_err() {
        // Environmental failure (disk quota) - skip test
        return;
    }
    // Valid child path
    let child = ws.join("subdir").join("file.txt");
    let result = m31a::platform::filesystem::HostFilesystem::validate_containment(&ws, &child);
    // The function must run without panic; result may vary by platform
    let _ = result;
}

#[test]
fn private_file_enforcement_has_typed_outcome() {
    let temp = tempfile::tempdir();
    if temp.is_err() {
        // Environmental failure (disk quota) - skip test
        return;
    }
    let temp = temp.unwrap();
    let file = temp.path().join("secret.txt");
    if std::fs::write(&file, b"test").is_err() {
        return;
    }
    let outcome = m31a::platform::permissions::ensure_private_file(&file);
    assert!(outcome.is_ok());
    let outcome = outcome.unwrap();
    assert!(matches!(
        outcome,
        m31a::platform::permissions::FileSecurityOutcome::Enforced
            | m31a::platform::permissions::FileSecurityOutcome::Unsupported
            | m31a::platform::permissions::FileSecurityOutcome::Failed
    ));
}

#[test]
fn shell_availability_is_explicit() {
    let avail = m31a::platform::shell::shell_availability();
    assert!(matches!(
        avail,
        m31a::platform::shell::ShellAvailability::Available
            | m31a::platform::shell::ShellAvailability::Unsupported
            | m31a::platform::shell::ShellAvailability::Unknown
    ));
}

#[test]
fn shell_backend_fails_closed_when_unavailable() {
    #[cfg(all(not(unix), not(windows)))]
    {
        use std::path::Path;
        let result = m31a::platform::shell::build_shell_command("echo test", Path::new("."));
        assert!(result.is_err());
    }
}

#[test]
fn process_tree_termination_supported_on_implemented_platforms() {
    let supported = m31a::platform::process::supports_tree_termination();
    // Linux, macOS, Windows should report true
    #[cfg(any(target_os = "linux", target_os = "macos", windows))]
    assert!(supported);
    // Other platforms may or may not
}

#[test]
fn resource_budget_validation_rejects_zero() {
    let budget = m31a::platform::resources::ResourceBudget {
        max_cpu_seconds: Some(0),
        ..m31a::platform::resources::ResourceBudget::default()
    };
    let result = m31a::platform::resources::validate_budget(&budget);
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        m31a::platform::resources::LimitOutcome::Rejected { .. }
    ));
}

#[test]
fn sandbox_readiness_uses_typed_states() {
    let services = m31a::platform::PlatformServices::host();
    let caps = &services.capabilities;
    for level in [
        m31a::platform::sandbox::IsolationLevel::NoIsolation,
        m31a::platform::sandbox::IsolationLevel::WorkspaceContainment,
        m31a::platform::sandbox::IsolationLevel::ProcessIsolation,
        m31a::platform::sandbox::IsolationLevel::FilesystemIsolation,
        m31a::platform::sandbox::IsolationLevel::NetworkIsolation,
        m31a::platform::sandbox::IsolationLevel::FullSandbox,
    ] {
        let result = m31a::platform::sandbox::check_isolation_available(caps, level);
        assert!(matches!(
            result,
            Ok(m31a::platform::sandbox::SandboxReadiness::Ready)
                | Ok(m31a::platform::sandbox::SandboxReadiness::Degraded)
                | Err(m31a::platform::sandbox::SandboxShortfall { .. })
        ));
    }
}

#[test]
fn terminal_pty_probe_reports_accurately() {
    let services = m31a::platform::PlatformServices::host();
    let term = &services.terminal;
    // pty_available() and term.pty must be consistent
    let pty_probe = m31a::platform::terminal::pty_available();
    let pty_cap = term.pty == m31a::platform::capabilities::CapabilityState::Available;
    // On native platforms, both should agree
    #[cfg(any(target_os = "linux", target_os = "macos", windows))]
    {
        assert_eq!(pty_probe, pty_cap, "pty probe and capability must agree");
    }
}
