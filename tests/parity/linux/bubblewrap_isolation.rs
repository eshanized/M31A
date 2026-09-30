//! Linux: bubblewrap isolation evidence.
//!
//! The bubblewrap binary is a Linux implementation detail. The test proves
//! the capability answer tracks binary presence instead of assuming it.

#[test]
fn parity_linux_bubblewrap_probe_tracks_binary_presence() {
    let probe = m31a::sandbox::PlatformProbe::probe_platform_capabilities();
    let binary_present = ["bwrap", "/usr/bin/bwrap", "/bin/bwrap"]
        .iter()
        .any(|p| std::path::Path::new(p).exists())
        || std::process::Command::new("which")
            .arg("bwrap")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
    #[cfg(target_os = "linux")]
    {
        assert_eq!(
            probe.supports_fs_isolation(),
            probe.fs_isolated_workspace_rw,
            "fs-isolation support and workspace-rw readiness agree"
        );
        if !binary_present {
            assert!(
                !probe.fs_isolated_workspace_rw,
                "without bwrap there is no isolated workspace-rw claim"
            );
        }
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (probe, binary_present);
    }
}

#[test]
fn parity_linux_bubblewrap_absence_fails_closed() {
    use m31a::platform::capabilities::{CapabilityState, PlatformCapabilities};
    use m31a::platform::sandbox::{IsolationLevel, check_isolation_available};
    let no_bwrap = PlatformCapabilities {
        process_tree_control: CapabilityState::Available,
        resource_limits: CapabilityState::Available,
        filesystem_isolation: CapabilityState::Degraded,
        environment_isolation: CapabilityState::Available,
        sandboxing: CapabilityState::Unsupported,
        secure_file_permissions: CapabilityState::Available,
        native_shell: CapabilityState::Available,
        terminal_control: CapabilityState::Available,
        filesystem_watching: CapabilityState::Available,
    };
    assert!(
        !matches!(
            check_isolation_available(&no_bwrap, IsolationLevel::FullSandbox),
            Ok(m31a::platform::sandbox::SandboxReadiness::Ready)
        ),
        "without bwrap, FullSandbox is at best Degraded (explicit opt-in), never Ready"
    );
}
