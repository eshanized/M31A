//! Negative: macOS Seatbelt is Degraded, never Available.
//!
//! Optional FS isolation may proceed Degraded; required FullSandbox must
//! still fail closed. Degraded is not Available.

#[test]
fn parity_negative_seatbelt_degraded_not_available() {
    use m31a::platform::capabilities::{CapabilityState, PlatformCapabilities};
    use m31a::platform::sandbox::{IsolationLevel, SandboxReadiness, check_isolation_available};
    let caps = m31a::platform::macos::probe_capabilities();
    assert_ne!(
        caps.filesystem_isolation,
        CapabilityState::Available,
        "Seatbelt never reports Available"
    );
    let degraded_caps = PlatformCapabilities {
        filesystem_isolation: CapabilityState::Degraded,
        sandboxing: CapabilityState::Unsupported,
        ..PlatformCapabilities::probe_host()
    };
    match check_isolation_available(&degraded_caps, IsolationLevel::FilesystemIsolation) {
        Ok(SandboxReadiness::Degraded) | Err(_) => {}
        Ok(SandboxReadiness::Ready) => {
            panic!("Degraded Seatbelt must not report Ready for FS isolation")
        }
        Ok(SandboxReadiness::Unsupported) => {
            unreachable!("readiness has no Unsupported variant")
        }
    }
    assert!(
        !matches!(
            check_isolation_available(&degraded_caps, IsolationLevel::FullSandbox),
            Ok(SandboxReadiness::Ready)
        ),
        "Degraded Seatbelt cannot satisfy FullSandbox as Ready (Degraded-or-shortfall only)"
    );
}
