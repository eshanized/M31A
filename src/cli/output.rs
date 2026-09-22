//! NDJSON Streaming Output & Envelope Formatting (CLI-02, D-18).

use serde::{Deserialize, Serialize};
use std::io::{self, Write};

/// Terminal frame closing any streaming execution run (D-18).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TerminalFrame {
    pub mission_id: String,
    pub outcome: String,
    pub exit_code: i32,
    pub duration_ms: u64,
    pub total_tokens: u64,
    pub artifacts: Vec<String>,
}

/// Typed stream messages emitted under `--output stream-json` (CLI-02).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
pub enum CliStreamMessage {
    StatusChange {
        mission_id: String,
        old_status: String,
        new_status: String,
        timestamp_epoch_ms: u64,
    },
    TaskProgress {
        task_id: String,
        title: String,
        state: String,
        progress_pct: f32,
    },
    VerificationResult {
        task_id: String,
        tier: String,
        passed: bool,
        evidence_summary: String,
    },
    PolicyEvent {
        tool_name: String,
        decision: String,
        justification: Option<String>,
    },
    StderrLine {
        line: String,
    },
    TerminalFrame(TerminalFrame),
}

/// Writer formatting and flushing typed NDJSON frames line-by-line to a writer.
pub struct NdjsonStreamWriter<W: Write> {
    writer: W,
    sealed: bool,
}

impl<W: Write> NdjsonStreamWriter<W> {
    pub fn new(writer: W) -> Self {
        Self {
            writer,
            sealed: false,
        }
    }

    /// Emit a stream message as a single UTF-8 JSON line followed by '\n'.
    pub fn emit(&mut self, message: &CliStreamMessage) -> io::Result<()> {
        if self.sealed {
            return Err(io::Error::other(
                "Stream already sealed with terminal frame",
            ));
        }

        let json_line = serde_json::to_string(message)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;

        writeln!(self.writer, "{json_line}")?;
        self.writer.flush()?;

        if matches!(message, CliStreamMessage::TerminalFrame(_)) {
            self.sealed = true;
        }

        Ok(())
    }

    /// Whether the terminal frame has been written.
    pub fn is_sealed(&self) -> bool {
        self.sealed
    }
}
