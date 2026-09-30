//! Negative: Windows network isolation is Unsupported.

#[test]
fn parity_negative_windows_net_isolation_unsupported() {
    use m31a::platform::PlatformFamily;
    use m31a::platform::capabilities::{CapabilityState, PlatformCapabilities};
    use m31a::platform::sandbox::{IsolationLevel, check_isolation_available};
    let caps = PlatformCapabilities::foundation_for(PlatformFamily::Windows);
    assert_eq!(caps.sandboxing, CapabilityState::Unsupported);
    assert!(
        check_isolation_available(&caps, IsolationLevel::NetworkIsolation).is_err(),
        "GAP: NetworkIsolation on Windows fails closed"
    );
    assert!(
        check_isolation_available(&caps, IsolationLevel::FullSandbox).is_err(),
        "GAP: FullSandbox on Windows fails closed"
    );
}
