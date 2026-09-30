//! Negative: Windows filesystem isolation is Unsupported.

#[test]
fn parity_negative_windows_fs_isolation_unsupported() {
    use m31a::platform::PlatformFamily;
    use m31a::platform::capabilities::{CapabilityState, PlatformCapabilities};
    use m31a::platform::sandbox::{IsolationLevel, check_isolation_available};
    let caps = PlatformCapabilities::foundation_for(PlatformFamily::Windows);
    assert_eq!(caps.filesystem_isolation, CapabilityState::Unsupported);
    for level in [
        IsolationLevel::FilesystemIsolation,
        IsolationLevel::FullSandbox,
    ] {
        assert!(
            check_isolation_available(&caps, level).is_err(),
            "GAP: {level:?} on Windows fails closed"
        );
    }
    let live = m31a::platform::windows::probe_capabilities();
    assert_eq!(
        live.filesystem_isolation,
        CapabilityState::Unsupported,
        "live probe agrees: no Windows FS isolation backend"
    );
}
