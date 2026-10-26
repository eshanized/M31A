//! Cross-platform contract tests — common semantic requirements.
//!
//! These tests verify M31A semantic guarantees that must hold on every
//! supported platform. Platform-specific tests live in sibling directories.

#[test]
fn platform_capabilities_report_honest_state() {
    let services = m31a::platform::PlatformServices::host();
    let caps = &services.capabilities;
    use m31a::platform::capabilities::{Capability, CapabilityState};

    let all_caps = [
        (Capability::ProcessTreeControl, caps.process_tree_control),
        (Capability::ResourceLimits, caps.resource_limits),
        (Capability::FilesystemIsolation, caps.filesystem_isolation),
        (Capability::EnvironmentIsolation, caps.environment_isolation),
        (Capability::Sandboxing, caps.sandboxing),
        (
            Capability::SecureFilePermissions,
            caps.secure_file_permissions,
        ),
        (Capability::NativeShell, caps.native_shell),
        (Capability::TerminalControl, caps.terminal_control),
        (Capability::FileSystemWatching, caps.filesystem_watching),
    ];

    for (cap, field_state) in all_caps {
        // Looked up state must match the struct field
        assert_eq!(
            caps.state_of(cap),
            field_state,
            "state_of({cap:?}) must match struct field"
        );

        // Fail-closed requirement contract: require() succeeds iff usable()
        if field_state.usable() {
            assert!(
                caps.require(cap).is_ok(),
                "Usable capability {cap:?} must pass require()"
            );
        } else {
            let err = caps.require(cap).expect_err(&format!(
                "Non-usable capability {cap:?} must fail require()"
            ));
            assert_eq!(err.capability, cap);
            assert_eq!(err.state, field_state);
        }

        // On supported platforms (Linux, macOS, Windows), no capability should be reported as Unknown
        #[cfg(any(target_os = "linux", target_os = "macos", windows))]
        assert_ne!(
            field_state,
            CapabilityState::Unknown,
            "Capability {cap:?} must not be Unknown on supported platform"
        );
    }

    #[cfg(target_os = "linux")]
    {
        assert_eq!(caps.process_tree_control, CapabilityState::Available);
        assert_eq!(caps.environment_isolation, CapabilityState::Available);
        assert_eq!(caps.secure_file_permissions, CapabilityState::Available);
        assert_eq!(caps.native_shell, CapabilityState::Available);
        assert_eq!(caps.terminal_control, CapabilityState::Available);
        assert_eq!(caps.filesystem_watching, CapabilityState::Available);
    }
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
    let temp = tempfile::tempdir().expect("create test workspace");
    let ws_path = temp.path().join("ws");
    let sub = ws_path.join("subdir");
    std::fs::create_dir_all(&sub).expect("create subdir");

    // 1. Valid child path inside workspace
    let child = sub.join("file.txt");
    std::fs::write(&child, b"child content").expect("write child");
    assert!(
        m31a::platform::filesystem::HostFilesystem::validate_containment(&ws_path, &child).is_ok(),
        "Valid child path must be accepted"
    );

    // 2. Relative child path
    assert!(
        m31a::platform::filesystem::HostFilesystem::validate_containment(
            &ws_path,
            std::path::Path::new("subdir/file.txt")
        )
        .is_ok(),
        "Relative path within workspace must be accepted"
    );

    // 3. Parent directory escape via ..
    let outside = temp.path().join("outside");
    std::fs::create_dir_all(&outside).expect("create outside");
    let outside_file = outside.join("secret.txt");
    std::fs::write(&outside_file, b"secret").expect("write secret");
    let escape = ws_path.join("..").join("outside").join("secret.txt");
    assert!(
        m31a::platform::filesystem::HostFilesystem::validate_containment(&ws_path, &escape)
            .is_err(),
        "Parent traversal escape must be rejected"
    );

    // 4. Sibling prefix spoof
    let sibling = temp.path().join("ws_spoof");
    std::fs::create_dir_all(&sibling).expect("create sibling");
    let sibling_file = sibling.join("evil.txt");
    std::fs::write(&sibling_file, b"evil").expect("write sibling");
    assert!(
        m31a::platform::filesystem::HostFilesystem::validate_containment(&ws_path, &sibling_file)
            .is_err(),
        "Sibling prefix spoof must be rejected"
    );
}

#[test]
fn private_file_enforcement_has_typed_outcome() {
    let temp = tempfile::tempdir().expect("temporary directory required");
    let file = temp.path().join("secret.txt");
    std::fs::write(&file, b"test secret content").expect("write test file");

    let outcome = m31a::platform::permissions::ensure_private_file(&file)
        .expect("ensure_private_file call must succeed on valid file");

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            outcome,
            m31a::platform::permissions::FileSecurityOutcome::Enforced,
            "Unix platforms must enforce private file security"
        );
        let meta = std::fs::metadata(&file).expect("read metadata");
        let mode = meta.permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "Unix private file mode must be 0o600");
    }

    #[cfg(windows)]
    {
        assert_eq!(
            outcome,
            m31a::platform::permissions::FileSecurityOutcome::Enforced,
            "Windows platform must enforce private file security via ACLs"
        );
    }

    #[cfg(all(not(unix), not(windows)))]
    {
        assert_eq!(
            outcome,
            m31a::platform::permissions::FileSecurityOutcome::Unsupported
        );
    }

    // Negative case: non-existent file must fail
    let non_existent = temp.path().join("missing.txt");
    let err = m31a::platform::permissions::ensure_private_file(&non_existent);
    #[cfg(any(unix, windows))]
    assert!(
        err.is_err(),
        "Enforcing permissions on missing file must return an error"
    );
}

#[test]
fn shell_availability_is_explicit() {
    let avail = m31a::platform::shell::shell_availability();

    #[cfg(any(unix, windows))]
    {
        assert_eq!(
            avail,
            m31a::platform::shell::ShellAvailability::Available,
            "Native shell must be Available on supported platform"
        );

        let prog = m31a::platform::shell::native_shell_program();
        assert!(prog.is_some(), "Native shell program must be resolved");

        #[cfg(unix)]
        assert_eq!(prog, Some("sh"));
        #[cfg(windows)]
        assert_eq!(prog, Some("cmd.exe"));

        let cmd =
            m31a::platform::shell::build_shell_command("echo test", std::path::Path::new("."))
                .expect("build_shell_command must succeed on supported platform");
        assert_eq!(
            cmd.as_std().get_current_dir(),
            Some(std::path::Path::new("."))
        );
    }

    #[cfg(all(not(unix), not(windows)))]
    {
        assert!(matches!(
            avail,
            m31a::platform::shell::ShellAvailability::Unsupported
                | m31a::platform::shell::ShellAvailability::Unknown
        ));
    }
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
    use m31a::platform::capabilities::{CapabilityState, PlatformCapabilities};
    use m31a::platform::sandbox::{
        IsolationLevel, SandboxReadiness, SandboxShortfall, check_isolation_available,
    };

    let services = m31a::platform::PlatformServices::host();
    let caps = &services.capabilities;

    // 1. Host capabilities contract:
    // NoIsolation and WorkspaceContainment are always Ready
    assert_eq!(
        check_isolation_available(caps, IsolationLevel::NoIsolation),
        Ok(SandboxReadiness::Ready)
    );
    assert_eq!(
        check_isolation_available(caps, IsolationLevel::WorkspaceContainment),
        Ok(SandboxReadiness::Ready)
    );

    // ProcessIsolation requires process_tree_control Available or Degraded
    let proc_result = check_isolation_available(caps, IsolationLevel::ProcessIsolation);
    if caps.process_tree_control == CapabilityState::Available
        || caps.process_tree_control == CapabilityState::Degraded
    {
        assert_eq!(proc_result, Ok(SandboxReadiness::Ready));
    } else {
        assert_eq!(
            proc_result,
            Err(SandboxShortfall {
                required: IsolationLevel::ProcessIsolation
            })
        );
    }

    // FilesystemIsolation mirrors filesystem_isolation capability
    let fs_result = check_isolation_available(caps, IsolationLevel::FilesystemIsolation);
    match caps.filesystem_isolation {
        CapabilityState::Available => assert_eq!(fs_result, Ok(SandboxReadiness::Ready)),
        CapabilityState::Degraded => assert_eq!(fs_result, Ok(SandboxReadiness::Degraded)),
        _ => assert_eq!(
            fs_result,
            Err(SandboxShortfall {
                required: IsolationLevel::FilesystemIsolation
            })
        ),
    }

    // NetworkIsolation and FullSandbox require sandboxing Available
    let net_result = check_isolation_available(caps, IsolationLevel::NetworkIsolation);
    let full_result = check_isolation_available(caps, IsolationLevel::FullSandbox);
    if caps.sandboxing == CapabilityState::Available {
        assert_eq!(net_result, Ok(SandboxReadiness::Ready));
        assert_eq!(full_result, Ok(SandboxReadiness::Ready));
    } else {
        assert_eq!(
            net_result,
            Err(SandboxShortfall {
                required: IsolationLevel::NetworkIsolation
            })
        );
        assert_eq!(
            full_result,
            Err(SandboxShortfall {
                required: IsolationLevel::FullSandbox
            })
        );
    }

    // 2. Synthetic negative test: fail-closed when capabilities are unsupported
    let empty_caps = PlatformCapabilities {
        process_tree_control: CapabilityState::Unsupported,
        resource_limits: CapabilityState::Unsupported,
        filesystem_isolation: CapabilityState::Unsupported,
        environment_isolation: CapabilityState::Unsupported,
        sandboxing: CapabilityState::Unsupported,
        secure_file_permissions: CapabilityState::Unsupported,
        native_shell: CapabilityState::Unsupported,
        terminal_control: CapabilityState::Unsupported,
        filesystem_watching: CapabilityState::Unsupported,
    };
    assert_eq!(
        check_isolation_available(&empty_caps, IsolationLevel::ProcessIsolation),
        Err(SandboxShortfall {
            required: IsolationLevel::ProcessIsolation
        })
    );
    assert_eq!(
        check_isolation_available(&empty_caps, IsolationLevel::FilesystemIsolation),
        Err(SandboxShortfall {
            required: IsolationLevel::FilesystemIsolation
        })
    );
    assert_eq!(
        check_isolation_available(&empty_caps, IsolationLevel::NetworkIsolation),
        Err(SandboxShortfall {
            required: IsolationLevel::NetworkIsolation
        })
    );
    assert_eq!(
        check_isolation_available(&empty_caps, IsolationLevel::FullSandbox),
        Err(SandboxShortfall {
            required: IsolationLevel::FullSandbox
        })
    );
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
