//! eBPF Execution Auditing Integration Tests.

use m31a_ebpf::*;

#[test]
fn test_capability_detection_and_degraded_status() {
    let probe_status = BpfProbe::probe();
    // Verify probe produced a concrete, non-empty status description
    assert!(!probe_status.description().is_empty());
}

#[test]
fn test_process_provenance_and_child_tracking() {
    let auditor = ExecutionAuditor::new(500);

    // Root execution registered
    auditor
        .correlator()
        .register_execution(4001, "turn-999", "sandbox-bwrap-1", "bash build.sh");

    // Root executes child
    auditor.ingest_event(AuditEventKind::ProcessExec {
        pid: 4001,
        ppid: 1,
        uid: 1000,
        binary_path: "/bin/bash".to_string(),
        args: vec!["bash".to_string(), "build.sh".to_string()],
    });

    // Forked compiler child process 4002
    auditor.ingest_event(AuditEventKind::ProcessExec {
        pid: 4002,
        ppid: 4001,
        uid: 1000,
        binary_path: "/usr/bin/rustc".to_string(),
        args: vec![
            "rustc".to_string(),
            "main.rs".to_string(),
            "--token=super-secret".to_string(),
        ],
    });

    // Query events by turn_id
    let records = auditor.query_events(&AuditQueryFilter {
        turn_id: Some("turn-999".to_string()),
        ..Default::default()
    });

    assert_eq!(records.len(), 2);
    assert_eq!(records[0].pid(), Some(4001));
    assert_eq!(records[1].pid(), Some(4002));
    assert_eq!(records[1].turn_id.as_deref(), Some("turn-999"));

    // Verify secret redaction
    if let AuditEventKind::ProcessExec { ref args, .. } = records[1].kind {
        assert_eq!(args[2], "--token=[REDACTED_SECRET]");
    } else {
        panic!("Expected ProcessExec");
    }
}

#[test]
fn test_bounded_ring_buffer_backpressure_and_overflow_visibility() {
    // Tiny buffer of 5 events
    let auditor = ExecutionAuditor::new(5);

    for i in 0..20 {
        auditor.ingest_event(AuditEventKind::FileOpen {
            pid: 7000,
            path: format!("/workspace/file_{}.txt", i),
            flags: 0,
            read_only: true,
        });
    }

    let summary = auditor.summary();
    assert_eq!(summary.total_events_emitted, 20);
    assert_eq!(summary.total_events_lost, 15);
    assert_eq!(summary.drop_rate_percentage, 75.0);

    // Buffer should remain bounded at capacity
    assert_eq!(summary.queued_events_count, 5);

    // Overflow visibility: an overflow event must be present in the ring buffer
    let records = auditor.query_events(&m31a_ebpf::AuditQueryFilter::default());
    assert!(
        records
            .iter()
            .any(|r| matches!(r.kind, AuditEventKind::RingBufferOverflow { .. }))
    );
}

#[test]
fn test_audit_performance_benchmark() {
    let report = AuditBenchmarker::run_benchmark(1000, 2000);
    assert_eq!(report.total_iterations, 1000);
    assert!(report.throughput_events_per_sec > 100.0);
    assert!(report.mean_latency_nanos_per_event > 0.0);
}
