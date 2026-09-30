//! macOS: RLIMIT_AS best-effort semantics.
//!
//! macOS honors RLIMIT_AS with allocator-dependent behavior. The mapping
//! names it best-effort explicitly; output bounds stay runtime-enforced.

#[test]
fn parity_macos_rlimit_as_best_effort_named() {
    use m31a::platform::resources::ResourceBudget;
    let budget = ResourceBudget {
        max_cpu_seconds: Some(5),
        max_memory_bytes: Some(128 * 1024 * 1024),
        max_processes: None,
        max_open_files: Some(128),
        max_output_bytes: Some(4096),
    };
    let (applied, missing) = m31a::platform::macos::macos_enforceable_limits(&budget);
    assert!(applied.iter().any(|a| a.contains("RLIMIT_CPU")));
    assert!(
        applied
            .iter()
            .any(|a| a.contains("RLIMIT_AS") && a.contains("best-effort")),
        "SEMANTICALLY-DIFFERENT: macOS memory is best-effort: {applied:?}"
    );
    assert!(applied.iter().any(|a| a.contains("RLIMIT_NOFILE")));
    assert!(missing.iter().any(|m| m.contains("max_output_bytes")));
    let verdict = m31a::platform::macos::validate_macos_budget(&budget).unwrap();
    assert!(
        matches!(
            verdict,
            m31a::platform::resources::LimitOutcome::PartiallyApplied { .. }
        ),
        "output-bytes exclusion yields PartiallyApplied, got {verdict:?}"
    );
}
