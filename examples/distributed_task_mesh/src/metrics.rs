//! Runtime metrics and telemetry counters for Distributed Task Mesh.
//!
//! Tracks task execution throughput, queue latencies, and cluster error rates using atomic counters.

use serde::Serialize;
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Debug, Default)]
pub struct TaskMeshMetrics {
    pub tasks_submitted: AtomicU64,
    pub tasks_completed: AtomicU64,
    pub tasks_failed: AtomicU64,
    pub total_execution_time_ms: AtomicU64,
}

#[derive(Debug, Serialize)]
pub struct MetricsSnapshot {
    pub tasks_submitted: u64,
    pub tasks_completed: u64,
    pub tasks_failed: u64,
    pub avg_execution_time_ms: f64,
    pub success_rate_percent: f64,
}

impl TaskMeshMetrics {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record_submitted(&self) {
        self.tasks_submitted.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_completed(&self, duration_ms: u64) {
        self.tasks_completed.fetch_add(1, Ordering::Relaxed);
        self.total_execution_time_ms
            .fetch_add(duration_ms, Ordering::Relaxed);
    }

    pub fn record_failed(&self) {
        self.tasks_failed.fetch_add(1, Ordering::Relaxed);
    }

    pub fn snapshot(&self) -> MetricsSnapshot {
        let submitted = self.tasks_submitted.load(Ordering::Relaxed);
        let completed = self.tasks_completed.load(Ordering::Relaxed);
        let failed = self.tasks_failed.load(Ordering::Relaxed);
        let total_time = self.total_execution_time_ms.load(Ordering::Relaxed);

        let avg_time = if completed > 0 {
            total_time as f64 / completed as f64
        } else {
            0.0
        };

        let total_finished = completed + failed;
        let success_rate = if total_finished > 0 {
            (completed as f64 / total_finished as f64) * 100.0
        } else {
            100.0
        };

        MetricsSnapshot {
            tasks_submitted: submitted,
            tasks_completed: completed,
            tasks_failed: failed,
            avg_execution_time_ms: avg_time,
            success_rate_percent: success_rate,
        }
    }
}
