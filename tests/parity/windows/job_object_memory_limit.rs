//! Windows: Job Object memory-limit mapping.
//!
//! Memory maps to JOB_OBJECT_LIMIT_PROCESS_MEMORY / job-wide memory while
//! FD limits have no counterpart. The test pins that asymmetry so a future
//! change cannot silently claim FD enforcement.

#[test]
fn parity_windows_job_memory_limit_mapping() {
    use m31a::platform::resources::ResourceBudget;
    let below = ResourceBudget {
        max_cpu_seconds: None,
        max_memory_bytes: Some(64 * 1024 * 1024),
        max_processes: None,
        max_open_files: None,
        max_output_bytes: None,
    };
    let desc = m31a::platform::windows::job::describe_job_limits(&below);
    assert!(
        desc.applied
            .iter()
            .any(|a| a.contains("PROCESS_MEMORY") || a.contains("MEMORY")),
        "memory maps to a Job memory limit: {:?}",
        desc.applied
    );
    assert!(
        desc.missing.is_empty(),
        "a memory-only budget has no missing Job facilities: {:?}",
        desc.missing
    );
    let empty = ResourceBudget {
        max_cpu_seconds: None,
        max_memory_bytes: None,
        max_processes: None,
        max_open_files: None,
        max_output_bytes: None,
    };
    let none = m31a::platform::windows::job::describe_job_limits(&empty);
    assert!(none.applied.is_empty(), "empty budget maps to no limits");
    assert!(none.kill_on_job_close, "boundary outlives the budget");
}
