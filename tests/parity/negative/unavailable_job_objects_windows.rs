//! Negative: unavailable Job Objects degrade tree control honestly.
//!
//! Without Job Objects, Windows tree control is Degraded (single-child
//! kill only) and resource limits are Unsupported. The degraded path is
//! explicit, never a silent full-tree claim.

#[test]
fn parity_negative_job_objects_unavailable_degrades_honestly() {
    use m31a::platform::PlatformFamily;
    use m31a::platform::capabilities::{CapabilityState, PlatformCapabilities};
    #[cfg(not(windows))]
    assert!(
        !m31a::platform::windows::job::job_objects_available(),
        "off-Windows the Job probe is false"
    );
    let foundation = PlatformCapabilities::foundation_for(PlatformFamily::Windows);
    assert_eq!(
        foundation.process_tree_control,
        CapabilityState::Degraded,
        "without Jobs, tree control is Degraded (single-child kill)"
    );
    assert!(
        foundation
            .require(m31a::platform::capabilities::Capability::ProcessTreeControl)
            .is_err(),
        "Degraded tree control fails require() until explicitly accepted"
    );
    assert!(
        foundation
            .require(m31a::platform::capabilities::Capability::ResourceLimits)
            .is_err(),
        "resource limits without Jobs fail closed"
    );
}
