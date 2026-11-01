//! Low-level rollout trace events and execution telemetry envelopes.
//!
//! Serializes fine-grained turn events, tool status transitions, and compaction boundaries.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

/// Typed event payloads representing discrete turn execution stages and telemetry markers.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", content = "payload", rename_all = "snake_case")]
pub enum TraceEventPayload {
    ThreadStarted {
        thread_id: String,
        cwd: String,
        role: Option<String>,
        model: String,
    },
    TurnStarted {
        turn_id: String,
        user_message: String,
    },
    InferenceStarted {
        turn_id: String,
        model: String,
        prompt_tokens: u64,
    },
    InferenceChunk {
        turn_id: String,
        chunk_len: usize,
    },
    InferenceCompleted {
        turn_id: String,
        completion_tokens: u64,
        finish_reason: String,
    },
    ToolDispatchStarted {
        turn_id: String,
        tool_name: String,
        call_id: String,
        arguments_preview: String,
    },
    ToolDispatchCompleted {
        turn_id: String,
        tool_name: String,
        call_id: String,
        duration_micros: u64,
        is_error: bool,
    },
    CompactionStarted {
        before_tokens: u64,
        retained_turns: usize,
    },
    CompactionCompleted {
        after_tokens: u64,
        dropped_turns: usize,
    },
    AgentSpawned {
        child_thread_id: String,
        role: String,
    },
    AgentTerminated {
        child_thread_id: String,
        exit_status: String,
    },
    TurnCompleted {
        turn_id: String,
        duration_ms: u64,
    },
}

/// A single atomic event in the append-only `trace.jsonl` stream.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RawTraceEvent {
    pub seq: u64,
    pub timestamp_micros: u64,
    pub thread_id: String,
    pub event: TraceEventPayload,
}

/// Hot-path thread-safe append-only trace writer.
#[derive(Debug)]
pub struct TraceWriter {
    thread_id: String,
    trace_file_path: PathBuf,
    seq: AtomicU64,
    file_handle: Mutex<Option<File>>,
}

impl TraceWriter {
    /// Create a trace writer targeting `trace_dir/<thread_id>.trace.jsonl`.
    pub fn new(trace_dir: &Path, thread_id: &str) -> Result<Self> {
        fs::create_dir_all(trace_dir).with_context(|| {
            format!(
                "Failed to create trace directory at {}",
                trace_dir.display()
            )
        })?;

        let trace_file_path = trace_dir.join(format!("{}.trace.jsonl", thread_id));
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&trace_file_path)
            .with_context(|| {
                format!("Failed to open trace file at {}", trace_file_path.display())
            })?;

        Ok(Self {
            thread_id: thread_id.to_string(),
            trace_file_path,
            seq: AtomicU64::new(1),
            file_handle: Mutex::new(Some(file)),
        })
    }

    /// Record an event into the trace log.
    pub fn record(&self, event: TraceEventPayload) -> Result<u64> {
        let seq = self.seq.fetch_add(1, Ordering::SeqCst);
        let now_micros = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_micros() as u64)
            .unwrap_or(0);

        let raw_event = RawTraceEvent {
            seq,
            timestamp_micros: now_micros,
            thread_id: self.thread_id.clone(),
            event,
        };

        let mut serialized = serde_json::to_string(&raw_event)?;
        serialized.push('\n');

        let mut lock = self.file_handle.lock().unwrap();
        if let Some(ref mut file) = *lock {
            file.write_all(serialized.as_bytes())?;
            file.flush()?;
        }

        Ok(seq)
    }

    pub fn trace_path(&self) -> &Path {
        &self.trace_file_path
    }
}

/// Reduced semantic summary of an executed rollout trace.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RolloutSummary {
    pub thread_id: String,
    pub total_events: usize,
    pub total_turns: usize,
    pub total_prompt_tokens: u64,
    pub total_completion_tokens: u64,
    pub tools_invoked: HashMap<String, usize>,
    pub tool_errors: usize,
    pub spawned_agents: Vec<String>,
    pub compactions: usize,
    pub start_time_micros: u64,
    pub end_time_micros: u64,
    pub wall_clock_duration_ms: u64,
}

/// Replays a raw JSONL trace log and computes its semantic `RolloutSummary`.
pub fn replay_trace(trace_path: &Path) -> Result<RolloutSummary> {
    let file = File::open(trace_path).with_context(|| {
        format!(
            "Failed to open trace file for replay: {}",
            trace_path.display()
        )
    })?;

    let reader = BufReader::new(file);
    let mut summary = RolloutSummary::default();

    for line in reader.lines() {
        let line = line?;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let event: RawTraceEvent = serde_json::from_str(trimmed)
            .with_context(|| format!("Failed to deserialize trace line: {}", trimmed))?;

        if summary.start_time_micros == 0 || event.timestamp_micros < summary.start_time_micros {
            summary.start_time_micros = event.timestamp_micros;
        }
        if event.timestamp_micros > summary.end_time_micros {
            summary.end_time_micros = event.timestamp_micros;
        }

        summary.thread_id = event.thread_id;
        summary.total_events += 1;

        match event.event {
            TraceEventPayload::TurnStarted { .. } => {
                summary.total_turns += 1;
            }
            TraceEventPayload::InferenceStarted { prompt_tokens, .. } => {
                summary.total_prompt_tokens += prompt_tokens;
            }
            TraceEventPayload::InferenceCompleted {
                completion_tokens, ..
            } => {
                summary.total_completion_tokens += completion_tokens;
            }
            TraceEventPayload::ToolDispatchCompleted {
                tool_name,
                is_error,
                ..
            } => {
                *summary.tools_invoked.entry(tool_name).or_insert(0) += 1;
                if is_error {
                    summary.tool_errors += 1;
                }
            }
            TraceEventPayload::CompactionCompleted { .. } => {
                summary.compactions += 1;
            }
            TraceEventPayload::AgentSpawned {
                child_thread_id, ..
            } => {
                summary.spawned_agents.push(child_thread_id);
            }
            _ => {}
        }
    }

    if summary.end_time_micros > summary.start_time_micros {
        summary.wall_clock_duration_ms =
            (summary.end_time_micros - summary.start_time_micros) / 1000;
    }

    Ok(summary)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_trace_writer_and_replay_lifecycle() {
        let dir = tempdir().unwrap();
        let writer = TraceWriter::new(dir.path(), "thread-test-1234").unwrap();

        // 1. Thread started
        writer
            .record(TraceEventPayload::ThreadStarted {
                thread_id: "thread-test-1234".to_string(),
                cwd: "/home/user/project".to_string(),
                role: Some("carmack".to_string()),
                model: "openai/o3-mini".to_string(),
            })
            .unwrap();

        // 2. Turn started
        writer
            .record(TraceEventPayload::TurnStarted {
                turn_id: "turn-1".to_string(),
                user_message: "Implement bubblewrap sandbox".to_string(),
            })
            .unwrap();

        // 3. Inference
        writer
            .record(TraceEventPayload::InferenceStarted {
                turn_id: "turn-1".to_string(),
                model: "openai/o3-mini".to_string(),
                prompt_tokens: 1500,
            })
            .unwrap();
        writer
            .record(TraceEventPayload::InferenceCompleted {
                turn_id: "turn-1".to_string(),
                completion_tokens: 300,
                finish_reason: "tool_calls".to_string(),
            })
            .unwrap();

        // 4. Tool dispatch
        writer
            .record(TraceEventPayload::ToolDispatchStarted {
                turn_id: "turn-1".to_string(),
                tool_name: "write_file".to_string(),
                call_id: "call_abc".to_string(),
                arguments_preview: "{\"path\": \"src/sandbox.rs\"}".to_string(),
            })
            .unwrap();
        writer
            .record(TraceEventPayload::ToolDispatchCompleted {
                turn_id: "turn-1".to_string(),
                tool_name: "write_file".to_string(),
                call_id: "call_abc".to_string(),
                duration_micros: 4500,
                is_error: false,
            })
            .unwrap();

        // 5. Agent spawned
        writer
            .record(TraceEventPayload::AgentSpawned {
                child_thread_id: "child-worker-5678".to_string(),
                role: "hopper".to_string(),
            })
            .unwrap();

        // Replay and verify summary reduction
        let summary = replay_trace(writer.trace_path()).unwrap();
        assert_eq!(summary.thread_id, "thread-test-1234");
        assert_eq!(summary.total_events, 7);
        assert_eq!(summary.total_turns, 1);
        assert_eq!(summary.total_prompt_tokens, 1500);
        assert_eq!(summary.total_completion_tokens, 300);
        assert_eq!(summary.tools_invoked.get("write_file"), Some(&1));
        assert_eq!(summary.tool_errors, 0);
        assert_eq!(summary.spawned_agents.len(), 1);
        assert_eq!(summary.spawned_agents[0], "child-worker-5678");
    }
}
