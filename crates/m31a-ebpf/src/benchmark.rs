//! Performance Benchmarking Suite for Kernel Execution Auditing.
//!
//! Provides reproducible benchmarks measuring audit event ingestion throughput,
//! dispatch latency, and backpressure loss rates under high contention.

use super::auditor::ExecutionAuditor;
use super::events::AuditEventKind;
use serde::{Deserialize, Serialize};
use std::time::Instant;

/// Benchmark performance metrics report for execution auditing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditBenchmarkReport {
    pub total_iterations: usize,
    pub elapsed_millis: f64,
    pub throughput_events_per_sec: f64,
    pub mean_latency_nanos_per_event: f64,
    pub events_lost: u64,
    pub drop_rate_percentage: f64,
}

pub struct AuditBenchmarker;

impl AuditBenchmarker {
    /// Executes a benchmark run simulating high-frequency process and filesystem events.
    pub fn run_benchmark(iterations: usize, ring_capacity: usize) -> AuditBenchmarkReport {
        let auditor = ExecutionAuditor::new(ring_capacity);
        auditor
            .correlator()
            .register_execution(5000, "bench-turn", "bench-sandbox", "benchmark");

        let start = Instant::now();

        for i in 0..iterations {
            let child_pid = 5001 + (i % 50) as u32;
            if i % 4 == 0 {
                auditor.ingest_event(AuditEventKind::ProcessExec {
                    pid: child_pid,
                    ppid: 5000,
                    uid: 1000,
                    binary_path: "/bin/worker".to_string(),
                    args: vec!["--job".to_string(), i.to_string()],
                });
            } else if i % 4 == 1 {
                auditor.ingest_event(AuditEventKind::FileOpen {
                    pid: child_pid,
                    path: format!("/workspace/file_{}.rs", i),
                    flags: 0,
                    read_only: true,
                });
            } else if i % 4 == 2 {
                auditor.ingest_event(AuditEventKind::NetworkConnect {
                    pid: child_pid,
                    destination_ip: "127.0.0.1".to_string(),
                    port: 8080,
                    protocol: "tcp".to_string(),
                });
            } else {
                auditor.ingest_event(AuditEventKind::ProcessExit {
                    pid: child_pid,
                    ppid: 5000,
                    exit_code: 0,
                    duration_ms: 12,
                });
            }
        }

        let elapsed = start.elapsed();
        let elapsed_secs = elapsed.as_secs_f64();
        let elapsed_millis = elapsed_secs * 1000.0;

        let throughput = if elapsed_secs > 0.0 {
            iterations as f64 / elapsed_secs
        } else {
            0.0
        };

        let mean_latency = if iterations > 0 {
            (elapsed.as_nanos() as f64) / (iterations as f64)
        } else {
            0.0
        };

        let summary = auditor.summary();

        AuditBenchmarkReport {
            total_iterations: iterations,
            elapsed_millis,
            throughput_events_per_sec: throughput,
            mean_latency_nanos_per_event: mean_latency,
            events_lost: summary.total_events_lost,
            drop_rate_percentage: summary.drop_rate_percentage,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audit_benchmark_execution() {
        let report = AuditBenchmarker::run_benchmark(5000, 10000);
        assert_eq!(report.total_iterations, 5000);
        assert!(report.throughput_events_per_sec > 1000.0);
        assert!(report.elapsed_millis > 0.0);
        assert_eq!(report.events_lost, 0); // Fits within 10000 capacity
    }
}
