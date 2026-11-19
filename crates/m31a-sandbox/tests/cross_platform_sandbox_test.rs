//! Cross-Platform Sandbox Backend & Platform Abstraction Verification Suite.

use m31a_core::config::SandboxPolicy;
use m31a_sandbox::appcontainer::{
    AppContainerSandbox, WIN_CAPABILITY_INTERNET_CLIENT,
    WIN_CAPABILITY_PRIVATE_NETWORK_CLIENT_SERVER,
};
use m31a_sandbox::platform::{PlatformCapabilityReport, SandboxError, SandboxPlatform};
use m31a_sandbox::seatbelt::SeatbeltSandbox;
use std::path::Path;

#[test]
fn test_macos_seatbelt_profile_security_invariants() {
    let ws = Path::new("/Users/dev/workspace");

    // 1. Read-Only Policy: Must deny writes AND deny network
    let ro_profile = SeatbeltSandbox::generate_profile(SandboxPolicy::ReadOnly, ws).unwrap();
    assert!(ro_profile.contains("(version 1)"));
    assert!(ro_profile.contains("(deny default)"));
    assert!(ro_profile.contains("(deny file-write*)"));
    assert!(
        ro_profile.contains("(deny network*)"),
        "ReadOnly policy must deny outbound network in Seatbelt"
    );
    assert!(ro_profile.contains("allow file-read* (subpath \"/Users/dev/workspace\")"));

    // 2. Workspace-Write Policy: Allow workspace write, but deny sensitive metadata
    let rw_profile = SeatbeltSandbox::generate_profile(SandboxPolicy::WorkspaceWrite, ws).unwrap();
    assert!(rw_profile.contains("allow file-write* (subpath \"/Users/dev/workspace\")"));
    assert!(rw_profile.contains("(deny network*)"));
    assert!(!rw_profile.contains("(allow network*)"));
    // Deny masks on sensitive subdirectories
    assert!(rw_profile.contains("/Users/dev/workspace/.git"));
    assert!(rw_profile.contains("/Users/dev/workspace/.ssh"));
    assert!(rw_profile.contains("/Users/dev/workspace/.env"));
    assert!(rw_profile.contains("/Users/dev/workspace/.m31a"));
    assert!(rw_profile.contains("/Users/dev/workspace/.aws"));

    // 3. DangerFullAccess: Unrestricted access
    let full_profile =
        SeatbeltSandbox::generate_profile(SandboxPolicy::DangerFullAccess, ws).unwrap();
    assert!(full_profile.contains("allow file-write* (subpath \"/\")"));
    assert!(full_profile.contains("allow sysctl-write"));
    assert!(full_profile.contains("(allow network*)"));
}

#[test]
fn test_macos_seatbelt_path_escaping() {
    let ws = Path::new("/Users/dev/\"injected\"/path");
    let profile = SeatbeltSandbox::generate_profile(SandboxPolicy::WorkspaceWrite, ws).unwrap();
    // Injected double-quotes must be properly escaped
    assert!(profile.contains("\\\"injected\\\""));
}

#[test]
fn test_windows_appcontainer_security_plan_invariants() {
    assert!(
        !AppContainerSandbox::is_available(),
        "AppContainer must remain unavailable until restricted process creation is implemented"
    );
    let ws = Path::new("C:\\Users\\dev\\workspace");

    // 1. ReadOnly Policy: No network capability SIDs, read_only_workspace = true
    let ro_sandbox = AppContainerSandbox::new(SandboxPolicy::ReadOnly, ws).unwrap();
    let ro_plan = ro_sandbox.plan();
    assert!(ro_plan.read_only_workspace);
    assert!(!ro_plan.network_enabled);
    assert!(
        ro_plan.capability_sids.is_empty(),
        "ReadOnly AppContainer must have zero network capabilities"
    );
    assert!(
        ro_plan
            .protected_metadata_masks
            .contains(&".git".to_string())
    );
    assert!(
        ro_plan
            .protected_metadata_masks
            .contains(&".env".to_string())
    );

    // 2. WorkspaceWrite Policy: internetClient capability granted, write permitted
    let rw_sandbox = AppContainerSandbox::new(SandboxPolicy::WorkspaceWrite, ws).unwrap();
    let rw_plan = rw_sandbox.plan();
    assert!(!rw_plan.read_only_workspace);
    assert!(rw_plan.network_enabled);
    assert_eq!(
        rw_plan.capability_sids,
        vec![WIN_CAPABILITY_INTERNET_CLIENT]
    );

    // 3. DangerFullAccess: both internet and private network capabilities granted
    let full_sandbox = AppContainerSandbox::new(SandboxPolicy::DangerFullAccess, ws).unwrap();
    let full_plan = full_sandbox.plan();
    assert!(
        full_plan
            .capability_sids
            .contains(&WIN_CAPABILITY_INTERNET_CLIENT.to_string())
    );
    assert!(
        full_plan
            .capability_sids
            .contains(&WIN_CAPABILITY_PRIVATE_NETWORK_CLIENT_SERVER.to_string())
    );
}

#[test]
fn test_windows_appcontainer_icacls_permissions() {
    let ws = Path::new("C:\\projects\\app");
    let sid = "S-1-15-2-99999-88888";

    // Read-only ACL
    let ro_cmd = AppContainerSandbox::build_icacls_grant_command(sid, ws, true);
    let ro_args: Vec<_> = ro_cmd
        .get_args()
        .map(|a| a.to_string_lossy().to_string())
        .collect();
    assert!(ro_args.contains(&format!("*\\{}:(OI)(CI)(RX)", sid)));

    // Read-write ACL
    let rw_cmd = AppContainerSandbox::build_icacls_grant_command(sid, ws, false);
    let rw_args: Vec<_> = rw_cmd
        .get_args()
        .map(|a| a.to_string_lossy().to_string())
        .collect();
    assert!(rw_args.contains(&format!("*\\{}:(OI)(CI)(M)", sid)));
}

#[test]
fn test_platform_abstraction_probe_and_fail_closed_guarantees() {
    let report: PlatformCapabilityReport = SandboxPlatform::probe();
    assert!(!report.os.is_empty());
    assert!(!report.diagnostics.is_empty());

    // Policy DangerFullAccess is always permitted
    let allowed = SandboxPlatform::assert_policy_enforceable(SandboxPolicy::DangerFullAccess);
    assert!(allowed.is_ok());

    // Policy ReadOnly requires a genuine kernel boundary
    let ro_result = SandboxPlatform::assert_policy_enforceable(SandboxPolicy::ReadOnly);
    if report.has_kernel_sandbox() {
        assert!(ro_result.is_ok());
    } else {
        assert!(ro_result.is_err());
        match ro_result.unwrap_err() {
            SandboxError::IsolationUnavailable { policy, reason } => {
                assert_eq!(policy, SandboxPolicy::ReadOnly);
                assert!(reason.contains("fail-closed"));
            }
            other => panic!("Expected IsolationUnavailable, got {:?}", other),
        }
    }
}
