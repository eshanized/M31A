//! Negative: Windows FD limit is Unsupported.
//!
//! Required negative parity test. Windows Job Objects have no file-
//! descriptor counterpart. Requesting FD enforcement for Windows must yield
//! Missing/Unsupported and fail closed — never a silent downgrade.

#[test]
fn parity_negative_windows_fd_limit_unsupported() {
    use m31a::platform::PlatformFamily;
    use m31a::platform::capabilities::{CapabilityState, PlatformCapabilities};
    use m31a::platform::resources::ResourceBudget;
    let budget = ResourceBudget {
        max_open_files: Some(256),
        ..ResourceBudget::default()
    };
    let desc = m31a::platform::windows::job::describe_job_limits(&budget);
    assert!(
        desc.missing.iter().any(|m| m.contains("max_open_files")),
        "GAP pinned: FD limit has no Job Object mapping"
    );
    let foundation = PlatformCapabilities::foundation_for(PlatformFamily::Windows);
    assert!(
        matches!(
            foundation.resource_limits,
            CapabilityState::Unsupported | CapabilityState::Degraded
        ),
        "Windows foundation never claims full resource limits: {:?}",
        foundation.resource_limits
    );
    assert!(
        !desc
            .applied
            .iter()
            .any(|a| a.contains("NOFILE") || a.contains("FD")),
        "no fabricated FD mechanism in the applied set: {:?}",
        desc.applied
    );
}
