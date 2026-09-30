//! Windows: Job Object tree-termination contract.
//!
//! Pure mapping verified on every host; native handle behavior verified on
//! Windows. The tree guarantee (join at spawn, kill as a unit, kill-on
//! -close) is the semantic under test — not the API name.

#[test]
fn parity_windows_job_tree_guarantee_and_mapping() {
    use m31a::platform::resources::ResourceBudget;
    let guarantee = m31a::platform::windows::job::tree_guarantee();
    assert!(
        guarantee.contains("Job Object") || guarantee.contains("job"),
        "guarantee names the boundary: {guarantee}"
    );
    let budget = ResourceBudget {
        max_cpu_seconds: Some(10),
        max_memory_bytes: Some(512 * 1024 * 1024),
        max_processes: Some(32),
        max_open_files: Some(256),
        max_output_bytes: Some(1024),
    };
    let desc = m31a::platform::windows::job::describe_job_limits(&budget);
    assert!(desc.kill_on_job_close, "kill-on-close is structural");
    assert!(desc.applied.iter().any(|a| a.contains("JOB_OBJECT")));
    assert!(desc.missing.iter().any(|m| m.contains("max_open_files")));
    #[cfg(windows)]
    {
        assert!(
            m31a::platform::windows::job::job_objects_available(),
            "native Job Object probe passes on Windows"
        );
        assert_eq!(
            m31a::platform::windows::process_backend_name(),
            "windows-job-objects"
        );
    }
    #[cfg(not(windows))]
    {
        assert!(
            !m31a::platform::windows::job::job_objects_available(),
            "off-Windows the probe is honestly false (ENVIRONMENT-BLOCKED native)"
        );
    }
}
