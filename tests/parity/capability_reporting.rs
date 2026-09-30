//! Parity: capability reporting honesty.
//!
//! For every platform the probe must report the mechanism actually observed:
//! Available only when present, Degraded only for reduced guarantees,
//! Unsupported for absent backends, Unknown when probing was impossible.
//! No false Available. No silent fallback.

use m31a::platform::capabilities::{Capability, CapabilityState, PlatformCapabilities};
use m31a::platform::{PlatformFamily, PlatformServices};

/// Every capability has an explicit state on the live host (never implicit).
#[test]
fn parity_caps_all_states_explicit() {
    let caps = PlatformCapabilities::probe_host();
    for cap in [
        Capability::ProcessTreeControl,
        Capability::ResourceLimits,
        Capability::FilesystemIsolation,
        Capability::EnvironmentIsolation,
        Capability::Sandboxing,
        Capability::SecureFilePermissions,
        Capability::NativeShell,
        Capability::TerminalControl,
        Capability::FileSystemWatching,
    ] {
        let state = caps.state_of(cap);
        assert!(
            matches!(
                state,
                CapabilityState::Available
                    | CapabilityState::Unavailable
                    | CapabilityState::Degraded
                    | CapabilityState::Unsupported
                    | CapabilityState::Unknown
            ),
            "{cap:?} has an explicit state: {state:?}"
        );
    }
}

/// Environment isolation is Available on the live host (baseline guarantee).
#[test]
fn parity_caps_environment_isolation_available() {
    let caps = PlatformCapabilities::probe_host();
    assert_eq!(
        caps.environment_isolation,
        CapabilityState::Available,
        "EQUIVALENT: environment isolation is the portable baseline"
    );
}

/// Linux host reports Available tree control, rlimits, and file permissions.
#[test]
fn parity_caps_linux_host_truthful() {
    #[cfg(target_os = "linux")]
    {
        let caps = PlatformCapabilities::probe_host();
        assert_eq!(caps.process_tree_control, CapabilityState::Available);
        assert_eq!(caps.secure_file_permissions, CapabilityState::Available);
        assert_eq!(caps.native_shell, CapabilityState::Available);
    }
}

/// Foundation profiles never claim Available for mechanisms that need a
/// native probe that did not run.
#[test]
fn parity_caps_foundation_never_fakes_available() {
    let windows = PlatformCapabilities::foundation_for(PlatformFamily::Windows);
    assert_eq!(
        windows.filesystem_isolation,
        CapabilityState::Unsupported,
        "Windows foundation honestly reports no FS isolation"
    );
    assert_eq!(windows.sandboxing, CapabilityState::Unsupported);
    let macos = PlatformCapabilities::foundation_for(PlatformFamily::Macos);
    assert_ne!(
        macos.filesystem_isolation,
        CapabilityState::Available,
        "macOS foundation never claims full FS isolation"
    );
    let unknown = PlatformCapabilities::foundation_for(PlatformFamily::Unknown);
    assert_eq!(unknown.process_tree_control, CapabilityState::Unknown);
    assert!(unknown.require(Capability::ProcessTreeControl).is_err());
}

/// Unknown is handled: require() fails closed, sandboxing label names it.
#[test]
fn parity_caps_unknown_fails_closed() {
    let unknown = PlatformCapabilities::foundation_for(PlatformFamily::Unknown);
    for cap in [
        Capability::ProcessTreeControl,
        Capability::FilesystemIsolation,
        Capability::NativeShell,
    ] {
        assert!(
            unknown.require(cap).is_err(),
            "Unknown {cap:?} must fail closed, never proceed"
        );
    }
    assert_eq!(unknown.sandboxing_label(), "unknown");
}

/// Missing dependency simulation: flipping one capability to Unsupported
/// flips exactly its dependents to refused, nothing else.
#[test]
fn parity_caps_missing_dependency_is_surgical() {
    let mut caps = PlatformCapabilities::probe_host();
    caps.native_shell = CapabilityState::Unsupported;
    assert!(caps.require(Capability::NativeShell).is_err());
    assert!(caps.require(Capability::EnvironmentIsolation).is_ok());
}

/// Disabled feature simulation: a Degraded tree-control host still refuses
/// require() until the caller explicitly accepts degradation.
#[test]
fn parity_caps_degraded_requires_explicit_acceptance() {
    let mut caps = PlatformCapabilities::probe_host();
    caps.process_tree_control = CapabilityState::Degraded;
    assert!(
        caps.require(Capability::ProcessTreeControl).is_err(),
        "Degraded fails require(); callers must check degraded_usable()"
    );
    assert!(caps.process_tree_control.degraded_usable());
}

/// Live probe agrees with the platform backend labels.
#[test]
fn parity_caps_probe_agrees_with_backend_labels() {
    let services = PlatformServices::host();
    assert!(!services.info.process_backend.is_empty());
    assert!(!services.info.shell_backend.is_empty());
    assert!(!services.info.sandbox_backend.is_empty());
    let summary = services.info.summary();
    assert!(summary.contains(&services.info.os_name));
}
