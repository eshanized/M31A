//! macOS: Seatbelt profile confinement (pure rendering + availability).
//!
//! The profile text is pure and verified on every host: writes confined to
//! workspace + temp, default-deny writes, network denied. Availability is
//! Degraded at best, never Available — Seatbelt deprecation is not hidden.

use std::path::Path;

#[test]
fn parity_macos_seatbelt_profile_confines_writes() {
    let ws = Path::new("/work/proj");
    let tmp = Path::new("/tmp");
    let profile = m31a::platform::macos::seatbelt_profile_for_workspace(ws, tmp);
    assert!(profile.contains("/work/proj"), "workspace is named");
    assert!(profile.contains("(deny file-write*"), "default-deny writes");
    assert!(profile.contains("(deny network*"), "network denied");
    assert!(
        profile.contains("(allow file-write* (literal \"/work/proj\")"),
        "workspace writes allowed: {profile}"
    );
}

#[test]
fn parity_macos_seatbelt_never_claims_full_sandbox() {
    let caps = m31a::platform::macos::probe_capabilities();
    assert_ne!(
        caps.filesystem_isolation,
        m31a::platform::capabilities::CapabilityState::Available,
        "SEMANTICALLY-DIFFERENT: Seatbelt is Degraded at best, never Available"
    );
    let avail = m31a::platform::macos::sandbox_availability();
    assert!(
        matches!(
            avail,
            m31a::platform::capabilities::CapabilityState::Degraded
                | m31a::platform::capabilities::CapabilityState::Unsupported
        ),
        "availability is Degraded-or-Unsupported, got {avail:?}"
    );
}

#[test]
fn parity_macos_sandbox_exec_command_fails_closed_without_binary() {
    let ws = std::env::temp_dir().join("m31a-p49-seatbelt");
    let cmd = m31a::platform::macos::sandbox_exec_command(&ws, "echo", &["hi".to_string()]);
    assert_eq!(
        cmd.is_some(),
        m31a::platform::macos::find_sandbox_exec().is_some(),
        "no sandbox-exec binary means None (fail closed), never a fake command"
    );
}
