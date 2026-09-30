//! macOS: libproc PID identity (pure contract + native check).
//!
//! Off-macOS the pure contract is exercised: identity is unprovable (None),
//! liveness is honestly false. On macOS the native libproc read returns a
//! stable token for the live process.

#[test]
fn parity_macos_libproc_identity_contract() {
    #[cfg(target_os = "macos")]
    {
        let pid = std::process::id();
        let a = m31a::platform::macos::read_process_starttime(pid);
        assert!(a.is_some(), "libproc answers for the live process");
        assert_eq!(a, m31a::platform::macos::read_process_starttime(pid));
        assert!(m31a::platform::macos::macos_pid_alive(pid));
        assert!(m31a::platform::macos::supports_starttime_protection());
    }
    #[cfg(not(target_os = "macos"))]
    {
        assert_eq!(m31a::platform::macos::read_process_starttime(1), None);
        assert!(!m31a::platform::macos::macos_pid_alive(1));
        assert!(!m31a::platform::macos::supports_starttime_protection());
        assert_eq!(
            m31a::platform::macos::process_backend_name(),
            "macos-process-groups-libproc"
        );
    }
}
