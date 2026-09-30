//! Windows: ConPTY dimensions / resize contract.

#[test]
fn parity_windows_conpty_dimensions_resize() {
    use m31a::platform::windows::conpty::{ConsoleSize, conpty_available};
    assert!(ConsoleSize::new(80, 24).valid());
    assert!(!ConsoleSize::new(0, 24).valid());
    assert!(!ConsoleSize::new(80, 0).valid());
    let caps = m31a::platform::windows::conpty::terminal_capabilities();
    assert_eq!(
        caps.backend,
        m31a::platform::terminal::TerminalBackend::WindowsConPty
    );
    assert_eq!(
        caps.dimensions,
        m31a::platform::capabilities::CapabilityState::Available,
        "dimensions are Available even without ConPTY"
    );
    #[cfg(not(windows))]
    {
        assert!(
            !conpty_available(),
            "off-Windows ConPTY is honestly absent (native resize ENVIRONMENT-BLOCKED)"
        );
        assert_eq!(
            caps.pty,
            m31a::platform::capabilities::CapabilityState::Unsupported
        );
        assert_eq!(
            caps.raw_mode,
            m31a::platform::capabilities::CapabilityState::Degraded
        );
    }
    #[cfg(windows)]
    {
        let _ = conpty_available();
    }
}
