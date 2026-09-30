//! macOS: PTY device contract.

#[test]
fn parity_macos_ptmx_contract() {
    let present =
        std::path::Path::new("/dev/ptmx").exists() || std::path::Path::new("/dev/ptc").exists();
    assert_eq!(
        m31a::platform::macos::pty_available(),
        present,
        "PTY probe matches the device fact"
    );
    assert_eq!(
        m31a::platform::macos::native_shell_program(),
        Some("sh"),
        "macOS shell is POSIX sh"
    );
    assert_eq!(
        m31a::platform::macos::terminal_backend_name(),
        "macos-posix-pty"
    );
}
