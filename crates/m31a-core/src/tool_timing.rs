//! High-resolution tool execution timer and latency profiler.
//!
//! Measures start, finish, and wall-clock execution durations for agent tool calls.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Work kind category for critical path timing analysis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkKind {
    /// LLM inference generation.
    Inference,
    /// Automated tool execution (file read, exec, patch, etc.).
    ToolExecution,
    /// Automated safety / guardian review.
    GuardianReview,
    /// Waiting for human user input or approval.
    HumanWait,
    /// MCP tool RPC roundtrip.
    McpCall,
}

/// Recorded work interval.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkInterval {
    pub name: String,
    pub kind: WorkKind,
    pub duration_micros: u64,
    pub is_error: bool,
}

/// RAII Guard that automatically measures execution duration.
pub struct ToolTimingGuard {
    name: String,
    kind: WorkKind,
    started_at: Instant,
    is_error: bool,
    recorder: Option<Arc<DirectToolTimings>>,
}

impl ToolTimingGuard {
    pub fn mark_error(&mut self) {
        self.is_error = true;
    }

    pub fn finish(mut self) -> CompletedWork {
        let ended_at = Instant::now();
        let duration = ended_at.saturating_duration_since(self.started_at);
        let work = CompletedWork {
            name: self.name.clone(),
            kind: self.kind,
            started_at: self.started_at,
            ended_at,
            duration,
            is_error: self.is_error,
        };

        if let Some(ref recorder) = self.recorder {
            recorder.record_completed(&work);
        }

        // Prevent drop from re-recording
        self.recorder = None;
        work
    }
}

impl Drop for ToolTimingGuard {
    fn drop(&mut self) {
        if let Some(ref recorder) = self.recorder {
            let ended_at = Instant::now();
            let duration = ended_at.saturating_duration_since(self.started_at);
            let work = CompletedWork {
                name: self.name.clone(),
                kind: self.kind,
                started_at: self.started_at,
                ended_at,
                duration,
                is_error: self.is_error,
            };
            recorder.record_completed(&work);
        }
    }
}

/// Result of a completed work interval with high-resolution timestamps.
#[derive(Debug, Clone)]
pub struct CompletedWork {
    pub name: String,
    pub kind: WorkKind,
    pub started_at: Instant,
    pub ended_at: Instant,
    pub duration: Duration,
    pub is_error: bool,
}

/// Thread-safe accumulator for tool invocations and critical path computation.
#[derive(Debug, Default)]
pub struct DirectToolTimings {
    records: Mutex<Vec<CompletedWork>>,
}

impl DirectToolTimings {
    pub fn new() -> Self {
        Self {
            records: Mutex::new(Vec::new()),
        }
    }

    /// Begin timing a tool invocation. Drop or `.finish()` the guard to finalize.
    pub fn start(self: &Arc<Self>, name: &str, kind: WorkKind) -> ToolTimingGuard {
        ToolTimingGuard {
            name: name.to_string(),
            kind,
            started_at: Instant::now(),
            is_error: false,
            recorder: Some(Arc::clone(self)),
        }
    }

    fn record_completed(&self, work: &CompletedWork) {
        let mut list = self.records.lock().unwrap();
        list.push(work.clone());
    }

    /// Get all completed work intervals in chronological order.
    pub fn completed_work(&self) -> Vec<CompletedWork> {
        self.records.lock().unwrap().clone()
    }

    /// Calculate total wall-clock duration and critical path metrics.
    pub fn analyze_critical_path(&self) -> CriticalPathReport {
        let records = self.records.lock().unwrap().clone();
        if records.is_empty() {
            return CriticalPathReport::default();
        }

        let earliest = records.iter().map(|r| r.started_at).min().unwrap();
        let latest = records.iter().map(|r| r.ended_at).max().unwrap();
        let total_wall_clock = latest.saturating_duration_since(earliest);

        let mut sum_duration = Duration::ZERO;
        let mut kind_breakdown: HashMap<WorkKind, Duration> = HashMap::new();
        let mut slowest_work: Option<CompletedWork> = None;

        for r in &records {
            sum_duration += r.duration;
            *kind_breakdown.entry(r.kind).or_default() += r.duration;

            if slowest_work
                .as_ref()
                .is_none_or(|s| s.duration < r.duration)
            {
                slowest_work = Some(r.clone());
            }
        }

        // Concurrency ratio = sum of active work durations / total wall clock
        let concurrency_ratio = if !total_wall_clock.is_zero() {
            sum_duration.as_secs_f64() / total_wall_clock.as_secs_f64()
        } else {
            1.0
        };

        CriticalPathReport {
            total_items: records.len(),
            wall_clock_duration_ms: total_wall_clock.as_millis() as u64,
            sum_duration_ms: sum_duration.as_millis() as u64,
            concurrency_ratio,
            slowest_operation: slowest_work.map(|w| (w.name, w.duration.as_millis() as u64)),
            tool_micros: kind_breakdown
                .get(&WorkKind::ToolExecution)
                .map(|d| d.as_micros() as u64)
                .unwrap_or(0),
            inference_micros: kind_breakdown
                .get(&WorkKind::Inference)
                .map(|d| d.as_micros() as u64)
                .unwrap_or(0),
            human_wait_micros: kind_breakdown
                .get(&WorkKind::HumanWait)
                .map(|d| d.as_micros() as u64)
                .unwrap_or(0),
            mcp_micros: kind_breakdown
                .get(&WorkKind::McpCall)
                .map(|d| d.as_micros() as u64)
                .unwrap_or(0),
        }
    }
}

/// Summary report of execution timing and critical path analysis.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CriticalPathReport {
    pub total_items: usize,
    pub wall_clock_duration_ms: u64,
    pub sum_duration_ms: u64,
    pub concurrency_ratio: f64,
    pub slowest_operation: Option<(String, u64)>,
    pub tool_micros: u64,
    pub inference_micros: u64,
    pub human_wait_micros: u64,
    pub mcp_micros: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread::sleep;

    #[test]
    fn test_timing_guard_recording() {
        let timings = Arc::new(DirectToolTimings::new());

        {
            let _guard = timings.start("exec_command", WorkKind::ToolExecution);
            sleep(Duration::from_millis(5));
        }

        {
            let mut guard = timings.start("request_user_input", WorkKind::HumanWait);
            guard.mark_error();
            sleep(Duration::from_millis(5));
            let completed = guard.finish();
            assert!(completed.is_error);
        }

        let completed = timings.completed_work();
        assert_eq!(completed.len(), 2);
        assert_eq!(completed[0].name, "exec_command");
        assert_eq!(completed[0].kind, WorkKind::ToolExecution);
        assert!(!completed[0].is_error);
        assert_eq!(completed[1].name, "request_user_input");
        assert!(completed[1].is_error);

        let report = timings.analyze_critical_path();
        assert_eq!(report.total_items, 2);
        assert!(report.wall_clock_duration_ms >= 10);
        assert!(report.sum_duration_ms >= 10);
        assert!(report.slowest_operation.is_some());
    }
}
