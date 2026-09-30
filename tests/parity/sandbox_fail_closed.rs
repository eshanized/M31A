//! Parity: sandbox fail-closed behavior.
//!
//! For each platform: requesting required filesystem isolation, required
//! network isolation, or a full sandbox without the mechanism must produce
//! a typed shortfall and block execution. No silent fallback, ever.

use m31a::platform::capabilities::{Capability, CapabilityState, PlatformCapabilities};
use m31a::platform::sandbox::{
    IsolationLevel, SandboxReadiness, check_isolation_available, describe_isolation,
};
use m31a::platform::{PlatformFamily, PlatformServices};

fn unsupported_all() -> PlatformCapabilities {
    PlatformCapabilities {
        process_tree_control: CapabilityState::Available,
        resource_limits: CapabilityState::Available,
        filesystem_isolation: CapabilityState::Unsupported,
        environment_isolation: CapabilityState::Available,
        sandboxing: CapabilityState::Unsupported,
        secure_file_permissions: CapabilityState::Available,
        native_shell: CapabilityState::Available,
        terminal_control: CapabilityState::Available,
        filesystem_watching: CapabilityState::Available,
    }
}

/// Required filesystem isolation without a backend fails closed.
#[test]
fn parity_sandbox_required_fs_isolation_fails_closed() {
    let caps = unsupported_all();
    let err = check_isolation_available(&caps, IsolationLevel::FilesystemIsolation)
        .expect_err("missing FS isolation must block");
    assert_eq!(err.required, IsolationLevel::FilesystemIsolation);
}

/// Required network isolation without a backend fails closed.
#[test]
fn parity_sandbox_required_net_isolation_fails_closed() {
    let caps = unsupported_all();
    let err = check_isolation_available(&caps, IsolationLevel::NetworkIsolation)
        .expect_err("missing network isolation must block");
    assert_eq!(err.required, IsolationLevel::NetworkIsolation);
}

/// Full sandbox without a backend fails closed.
#[test]
fn parity_sandbox_required_full_sandbox_fails_closed() {
    let caps = unsupported_all();
    assert!(
        check_isolation_available(&caps, IsolationLevel::FullSandbox).is_err(),
        "EQUIVALENT: FullSandbox without a backend is a typed shortfall"
    );
}

/// Unsupported capabilities fail the require() gate on the live host.
#[test]
fn parity_sandbox_host_unsupported_fails_require() {
    let services = PlatformServices::host();
    for cap in [
        Capability::FilesystemIsolation,
        Capability::Sandboxing,
        Capability::ProcessTreeControl,
        Capability::ResourceLimits,
    ] {
        if services.capabilities.state_of(cap) == CapabilityState::Unsupported {
            assert!(
                services.capabilities.require(cap).is_err(),
                "Unsupported {cap:?} must fail require()"
            );
        }
    }
}

/// Windows foundation profile: FS/network isolation Unsupported, full
/// sandbox refused. (Pure profile: identical on every host.)
#[test]
fn parity_sandbox_windows_foundation_fails_closed() {
    let caps = PlatformCapabilities::foundation_for(PlatformFamily::Windows);
    assert_eq!(
        caps.filesystem_isolation,
        CapabilityState::Unsupported,
        "GAP: Windows has no filesystem isolation backend"
    );
    assert_eq!(caps.sandboxing, CapabilityState::Unsupported);
    assert!(
        check_isolation_available(&caps, IsolationLevel::FullSandbox).is_err(),
        "GAP: FullSandbox on Windows fails closed, never silently passes"
    );
}

/// Degraded (macOS Seatbelt) is usable for optional FS isolation but is NOT
/// silently equivalent to Available for required full sandboxing.
#[test]
fn parity_sandbox_degraded_is_not_available() {
    let caps = PlatformCapabilities {
        filesystem_isolation: CapabilityState::Degraded,
        sandboxing: CapabilityState::Unsupported,
        ..unsupported_all()
    };
    let degraded = check_isolation_available(&caps, IsolationLevel::FilesystemIsolation);
    assert!(
        matches!(degraded, Ok(SandboxReadiness::Degraded) | Err(_)),
        "Degraded FS isolation is at most Degraded, never silently Ready-equivalent"
    );
    assert!(
        !matches!(
            check_isolation_available(&caps, IsolationLevel::FullSandbox),
            Ok(SandboxReadiness::Ready)
        ),
        "SEMANTICALLY-DIFFERENT: Degraded Seatbelt cannot satisfy FullSandbox as Ready"
    );
    assert!(
        !CapabilityState::Degraded.usable(),
        "Degraded.usable() is false: callers must opt in explicitly"
    );
    assert!(CapabilityState::Degraded.degraded_usable());
}

/// No-isolation and workspace containment always describe; process isolation
/// follows tree control.
#[test]
fn parity_sandbox_describe_isolation_minimums() {
    let caps = unsupported_all();
    let levels = describe_isolation(&caps);
    assert!(levels.contains(&IsolationLevel::NoIsolation));
    assert!(levels.contains(&IsolationLevel::WorkspaceContainment));
    assert!(levels.contains(&IsolationLevel::ProcessIsolation));
    assert!(!levels.contains(&IsolationLevel::FullSandbox));
}

/// Live host: FullSandbox readiness is Ready or a typed shortfall — never a
/// silent pass without the backend.
#[test]
fn parity_sandbox_live_host_full_sandbox_typed() {
    let services = PlatformServices::host();
    let result = check_isolation_available(&services.capabilities, IsolationLevel::FullSandbox);
    match result {
        Ok(SandboxReadiness::Ready) => {
            assert_eq!(
                services.capabilities.sandboxing,
                CapabilityState::Available,
                "Ready requires Available sandboxing"
            );
        }
        Ok(SandboxReadiness::Degraded) | Err(_) => {}
        Ok(SandboxReadiness::Unsupported) => {
            unreachable!("Unsupported is an error variant, not readiness")
        }
    }
}
