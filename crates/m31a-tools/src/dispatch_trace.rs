//! Sovereign tool dispatch trace recorder and forensic telemetry.
//!
//! Records high-resolution execution telemetry and forensic audit traces for tool invocations.
//! Captures high-resolution execution telemetry for every tool invocation:
//! - Execution timestamp, turn correlation, microsecond latency.
//! - Payload sizes (arguments byte length vs output byte length).
//! - Execution status (`Success`, `Error`, `TimedOut`, `Cancelled`).
//! - Turn-level aggregation metrics and JSONL streaming exports.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::RwLock;
use uuid::Uuid;

/// Execution status of a tool invocation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolDispatchStatus {
    Success,
    Error,
    TimedOut,
    Cancelled,
}

/// A structured telemetry trace record for a completed tool execution.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolDispatchRecord {
    pub trace_id: Uuid,
    pub session_id: String,
    pub turn_index: usize,
    pub call_id: String,
    pub tool_name: String,
    pub arguments_len_bytes: usize,
    pub output_len_bytes: usize,
    pub status: ToolDispatchStatus,
    pub timestamp: DateTime<Utc>,
    pub duration_us: u64,
    pub was_parallel: bool,
    pub error_message: Option<String>,
}

impl ToolDispatchRecord {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        session_id: impl Into<String>,
        turn_index: usize,
        call_id: impl Into<String>,
        tool_name: impl Into<String>,
        arguments_len_bytes: usize,
        output_len_bytes: usize,
        status: ToolDispatchStatus,
        duration_us: u64,
        was_parallel: bool,
        error_message: Option<String>,
    ) -> Self {
        Self {
            trace_id: Uuid::new_v4(),
            session_id: session_id.into(),
            turn_index,
            call_id: call_id.into(),
            tool_name: tool_name.into(),
            arguments_len_bytes,
            output_len_bytes,
            status,
            timestamp: Utc::now(),
            duration_us,
            was_parallel,
            error_message,
        }
    }
}

/// Turn-level aggregate telemetry summary of all tool invocations.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolTurnSummary {
    pub session_id: String,
    pub turn_index: usize,
    pub total_calls: usize,
    pub successful_calls: usize,
    pub failed_calls: usize,
    pub timed_out_calls: usize,
    pub total_duration_us: u64,
    pub total_arguments_bytes: usize,
    pub total_output_bytes: usize,
}

/// Thread-safe in-memory ring buffer for tool dispatch traces.
pub struct ToolDispatchTracer {
    capacity: usize,
    buffer: RwLock<VecDeque<ToolDispatchRecord>>,
}

impl Default for ToolDispatchTracer {
    fn default() -> Self {
        Self::new(1000)
    }
}

impl ToolDispatchTracer {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity: capacity.max(1),
            buffer: RwLock::new(VecDeque::with_capacity(capacity)),
        }
    }

    /// Record a completed tool dispatch event into the ring buffer.
    pub fn record(&self, record: ToolDispatchRecord) {
        let mut buf = self.buffer.write().unwrap();
        if buf.len() >= self.capacity {
            buf.pop_front();
        }
        buf.push_back(record);
    }

    /// Retrieve the most recent N records.
    pub fn recent_records(&self, limit: usize) -> Vec<ToolDispatchRecord> {
        let buf = self.buffer.read().unwrap();
        buf.iter().rev().take(limit).cloned().collect()
    }

    /// Filter traces belonging to a specific session.
    pub fn filter_by_session(&self, session_id: &str) -> Vec<ToolDispatchRecord> {
        let buf = self.buffer.read().unwrap();
        buf.iter()
            .filter(|r| r.session_id == session_id)
            .cloned()
            .collect()
    }

    /// Filter traces for a specific turn in a session.
    pub fn filter_by_turn(&self, session_id: &str, turn_index: usize) -> Vec<ToolDispatchRecord> {
        let buf = self.buffer.read().unwrap();
        buf.iter()
            .filter(|r| r.session_id == session_id && r.turn_index == turn_index)
            .cloned()
            .collect()
    }

    /// Aggregate turn metrics for a specific conversation turn.
    pub fn turn_summary(&self, session_id: &str, turn_index: usize) -> ToolTurnSummary {
        let records = self.filter_by_turn(session_id, turn_index);
        let mut summary = ToolTurnSummary {
            session_id: session_id.to_string(),
            turn_index,
            ..Default::default()
        };

        summary.total_calls = records.len();
        for r in &records {
            match r.status {
                ToolDispatchStatus::Success => summary.successful_calls += 1,
                ToolDispatchStatus::Error => summary.failed_calls += 1,
                ToolDispatchStatus::TimedOut => summary.timed_out_calls += 1,
                ToolDispatchStatus::Cancelled => summary.failed_calls += 1,
            }
            summary.total_duration_us += r.duration_us;
            summary.total_arguments_bytes += r.arguments_len_bytes;
            summary.total_output_bytes += r.output_len_bytes;
        }

        summary
    }

    /// Export buffered traces as newline-delimited JSON (JSONL).
    pub fn export_jsonl(&self) -> Result<String, serde_json::Error> {
        let buf = self.buffer.read().unwrap();
        let mut out = String::new();
        for r in buf.iter() {
            let line = serde_json::to_string(r)?;
            out.push_str(&line);
            out.push('\n');
        }
        Ok(out)
    }

    /// Clear all trace records.
    pub fn clear(&self) {
        let mut buf = self.buffer.write().unwrap();
        buf.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_record_and_recent() {
        let tracer = ToolDispatchTracer::new(10);
        let rec = ToolDispatchRecord::new(
            "sess_1",
            1,
            "call_123",
            "read_file",
            45,
            1200,
            ToolDispatchStatus::Success,
            2500,
            true,
            None,
        );

        tracer.record(rec.clone());
        let recent = tracer.recent_records(5);
        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0].session_id, "sess_1");
        assert_eq!(recent[0].tool_name, "read_file");
        assert_eq!(recent[0].duration_us, 2500);
    }

    #[test]
    fn test_capacity_ring_eviction() {
        let tracer = ToolDispatchTracer::new(2);
        for i in 1..=4 {
            tracer.record(ToolDispatchRecord::new(
                "sess_evict",
                i,
                format!("c{i}"),
                "grep_files",
                20,
                50,
                ToolDispatchStatus::Success,
                100,
                false,
                None,
            ));
        }

        let recent = tracer.recent_records(10);
        assert_eq!(recent.len(), 2);
        assert_eq!(recent[0].call_id, "c4");
        assert_eq!(recent[1].call_id, "c3");
    }

    #[test]
    fn test_turn_summary_aggregation() {
        let tracer = ToolDispatchTracer::new(20);

        tracer.record(ToolDispatchRecord::new(
            "sess_agg",
            3,
            "c1",
            "read_file",
            50,
            200,
            ToolDispatchStatus::Success,
            1000,
            true,
            None,
        ));
        tracer.record(ToolDispatchRecord::new(
            "sess_agg",
            3,
            "c2",
            "outline_file",
            30,
            500,
            ToolDispatchStatus::Success,
            1500,
            true,
            None,
        ));
        tracer.record(ToolDispatchRecord::new(
            "sess_agg",
            3,
            "c3",
            "web_search",
            60,
            0,
            ToolDispatchStatus::TimedOut,
            5000,
            false,
            Some("timeout".to_string()),
        ));

        let summary = tracer.turn_summary("sess_agg", 3);
        assert_eq!(summary.total_calls, 3);
        assert_eq!(summary.successful_calls, 2);
        assert_eq!(summary.timed_out_calls, 1);
        assert_eq!(summary.total_duration_us, 7500);
        assert_eq!(summary.total_arguments_bytes, 140);
        assert_eq!(summary.total_output_bytes, 700);

        let other = tracer.turn_summary("sess_agg", 4);
        assert_eq!(other.total_calls, 0);
    }

    #[test]
    fn test_export_jsonl() {
        let tracer = ToolDispatchTracer::new(5);
        tracer.record(ToolDispatchRecord::new(
            "s1",
            1,
            "c1",
            "tool_a",
            10,
            20,
            ToolDispatchStatus::Success,
            50,
            false,
            None,
        ));

        let jsonl = tracer.export_jsonl().expect("jsonl exported");
        assert!(jsonl.contains("\"session_id\":\"s1\""));
        assert!(jsonl.contains("\"tool_name\":\"tool_a\""));
        assert_eq!(jsonl.lines().count(), 1);
    }
}
