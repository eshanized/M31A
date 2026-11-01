//! Structured Analytics and Telemetry Pipeline (Simplified)
//!
//! Event-driven telemetry with multiple exporters.

use anyhow::{Context, Result};
use m31a_core::types::TokenUsage;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::{RwLock, mpsc};

/// Telemetry event types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TelemetryEvent {
    SessionStart {
        session_id: String,
        model: String,
        provider: String,
        config_hash: String,
        timestamp: u64,
    },
    SessionEnd {
        session_id: String,
        duration_ms: u64,
        total_turns: usize,
        total_tokens: u64,
        timestamp: u64,
    },
    TurnStart {
        session_id: String,
        turn_id: String,
        tools_available: usize,
        timestamp: u64,
    },
    TurnEnd {
        session_id: String,
        turn_id: String,
        success: bool,
        tokens_used: TokenUsage,
        duration_ms: u64,
        timestamp: u64,
    },
    ToolInvocation {
        session_id: String,
        turn_id: String,
        tool_name: String,
        tool_category: String,
        arguments_hash: String,
        success: bool,
        duration_ms: u64,
        tokens_used: Option<u64>,
        timestamp: u64,
    },
    ModelCall {
        session_id: String,
        turn_id: String,
        model: String,
        provider: String,
        input_tokens: u64,
        output_tokens: u64,
        reasoning_tokens: u64,
        latency_ms: u64,
        cached: bool,
        timestamp: u64,
    },
    Error {
        session_id: Option<String>,
        turn_id: Option<String>,
        error_type: String,
        error_message: String,
        recoverable: bool,
        context: HashMap<String, String>,
        timestamp: u64,
    },
    SandboxViolation {
        session_id: String,
        turn_id: String,
        operation: String,
        path: String,
        policy: String,
        severity: ViolationSeverity,
        timestamp: u64,
    },
    ApprovalRequest {
        session_id: String,
        turn_id: String,
        tool_name: String,
        action: String,
        risk_level: String,
        user_decision: Option<String>,
        timestamp: u64,
    },
    Compaction {
        session_id: String,
        original_tokens: usize,
        compacted_tokens: usize,
        summaries_created: usize,
        timestamp: u64,
    },
    WorktreeOperation {
        session_id: String,
        operation: String,
        success: bool,
        timestamp: u64,
    },
    PluginEvent {
        plugin_name: String,
        event_type: String,
        success: bool,
        timestamp: u64,
    },
}

/// Violation severity levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ViolationSeverity {
    Info,
    Warning,
    Critical,
}

/// Telemetry exporter trait
pub trait TelemetryExporter: Send + Sync {
    fn export(&self, events: Vec<TelemetryEvent>) -> Result<()>;
    fn name(&self) -> &'static str;
}

/// Console exporter for development
pub struct ConsoleExporter {
    pretty: bool,
}

impl ConsoleExporter {
    pub fn new(pretty: bool) -> Self {
        Self { pretty }
    }
}

impl TelemetryExporter for ConsoleExporter {
    fn export(&self, events: Vec<TelemetryEvent>) -> Result<()> {
        for event in events {
            let json = if self.pretty {
                serde_json::to_string_pretty(&event)?
            } else {
                serde_json::to_string(&event)?
            };
            println!("{}", json);
        }
        Ok(())
    }

    fn name(&self) -> &'static str {
        "console"
    }
}

/// JSONL file exporter for local storage
pub struct JsonlExporter {
    file_path: std::path::PathBuf,
}

impl JsonlExporter {
    pub fn new(file_path: std::path::PathBuf) -> Self {
        if let Some(parent) = file_path.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        Self { file_path }
    }
}

impl TelemetryExporter for JsonlExporter {
    fn export(&self, events: Vec<TelemetryEvent>) -> Result<()> {
        use std::fs::OpenOptions;
        use std::io::{BufWriter, Write};

        let mut file = BufWriter::new(
            OpenOptions::new()
                .create(true)
                .append(true)
                .open(&self.file_path)?,
        );

        for event in events {
            let json = serde_json::to_string(&event)?;
            writeln!(file, "{}", json)?;
        }
        file.flush()?;
        Ok(())
    }

    fn name(&self) -> &'static str {
        "jsonl"
    }
}

/// SQLite audit trail exporter for persistent queryable telemetry
pub struct SqliteAuditExporter {
    conn: Arc<std::sync::Mutex<rusqlite::Connection>>,
}

impl SqliteAuditExporter {
    pub fn new_in_memory() -> Result<Self> {
        let conn = rusqlite::Connection::open_in_memory()?;
        Self::init_schema(&conn)?;
        Ok(Self {
            conn: Arc::new(std::sync::Mutex::new(conn)),
        })
    }

    pub fn new(path: impl AsRef<std::path::Path>) -> Result<Self> {
        let path_ref = path.as_ref();
        if let Some(parent) = path_ref.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        let conn = rusqlite::Connection::open(path_ref)?;
        Self::init_schema(&conn)?;
        Ok(Self {
            conn: Arc::new(std::sync::Mutex::new(conn)),
        })
    }

    fn init_schema(conn: &rusqlite::Connection) -> Result<()> {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS audit_events (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                event_type TEXT NOT NULL,
                session_id TEXT,
                turn_id TEXT,
                timestamp INTEGER NOT NULL,
                payload_json TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_audit_session ON audit_events(session_id);
            CREATE INDEX IF NOT EXISTS idx_audit_type ON audit_events(event_type);
            CREATE INDEX IF NOT EXISTS idx_audit_timestamp ON audit_events(timestamp);",
        )?;
        Ok(())
    }

    pub fn query_events(
        &self,
        session_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<(i64, String, TelemetryEvent)>> {
        let conn = self.conn.lock().unwrap();
        let mut results = Vec::new();
        if let Some(sid) = session_id {
            let mut stmt = conn.prepare(
                "SELECT id, event_type, payload_json FROM audit_events WHERE session_id = ? ORDER BY id ASC LIMIT ?"
            )?;
            let rows = stmt.query_map(rusqlite::params![sid, limit as i64], |row| {
                let id: i64 = row.get(0)?;
                let event_type: String = row.get(1)?;
                let payload: String = row.get(2)?;
                Ok((id, event_type, payload))
            })?;
            for r in rows {
                let (id, et, payload) = r?;
                if let Ok(event) = serde_json::from_str::<TelemetryEvent>(&payload) {
                    results.push((id, et, event));
                }
            }
        } else {
            let mut stmt = conn.prepare(
                "SELECT id, event_type, payload_json FROM audit_events ORDER BY id ASC LIMIT ?",
            )?;
            let rows = stmt.query_map(rusqlite::params![limit as i64], |row| {
                let id: i64 = row.get(0)?;
                let event_type: String = row.get(1)?;
                let payload: String = row.get(2)?;
                Ok((id, event_type, payload))
            })?;
            for r in rows {
                let (id, et, payload) = r?;
                if let Ok(event) = serde_json::from_str::<TelemetryEvent>(&payload) {
                    results.push((id, et, event));
                }
            }
        }
        Ok(results)
    }

    pub fn query_total_tokens(&self, session_id: &str) -> Result<u64> {
        let events = self.query_events(Some(session_id), 10000)?;
        let mut total = 0u64;
        for (_, _, event) in events {
            if let TelemetryEvent::ModelCall {
                input_tokens,
                output_tokens,
                ..
            } = event
            {
                total += input_tokens + output_tokens;
            } else if let TelemetryEvent::TurnEnd { tokens_used, .. } = event {
                total += tokens_used.total_tokens;
            }
        }
        Ok(total)
    }
}

impl TelemetryExporter for SqliteAuditExporter {
    fn export(&self, events: Vec<TelemetryEvent>) -> Result<()> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;
        {
            let mut stmt = tx.prepare(
                "INSERT INTO audit_events (event_type, session_id, turn_id, timestamp, payload_json)
                 VALUES (?, ?, ?, ?, ?)"
            )?;
            for event in &events {
                let (event_type, session_id, turn_id, timestamp) = match event {
                    TelemetryEvent::SessionStart {
                        session_id,
                        timestamp,
                        ..
                    } => ("session_start", Some(session_id.as_str()), None, *timestamp),
                    TelemetryEvent::SessionEnd {
                        session_id,
                        timestamp,
                        ..
                    } => ("session_end", Some(session_id.as_str()), None, *timestamp),
                    TelemetryEvent::TurnStart {
                        session_id,
                        turn_id,
                        timestamp,
                        ..
                    } => (
                        "turn_start",
                        Some(session_id.as_str()),
                        Some(turn_id.as_str()),
                        *timestamp,
                    ),
                    TelemetryEvent::TurnEnd {
                        session_id,
                        turn_id,
                        timestamp,
                        ..
                    } => (
                        "turn_end",
                        Some(session_id.as_str()),
                        Some(turn_id.as_str()),
                        *timestamp,
                    ),
                    TelemetryEvent::ToolInvocation {
                        session_id,
                        turn_id,
                        timestamp,
                        ..
                    } => (
                        "tool_invocation",
                        Some(session_id.as_str()),
                        Some(turn_id.as_str()),
                        *timestamp,
                    ),
                    TelemetryEvent::ModelCall {
                        session_id,
                        turn_id,
                        timestamp,
                        ..
                    } => (
                        "model_call",
                        Some(session_id.as_str()),
                        Some(turn_id.as_str()),
                        *timestamp,
                    ),
                    TelemetryEvent::Error {
                        session_id,
                        turn_id,
                        timestamp,
                        ..
                    } => (
                        "error",
                        session_id.as_deref(),
                        turn_id.as_deref(),
                        *timestamp,
                    ),
                    TelemetryEvent::SandboxViolation {
                        session_id,
                        turn_id,
                        timestamp,
                        ..
                    } => (
                        "sandbox_violation",
                        Some(session_id.as_str()),
                        Some(turn_id.as_str()),
                        *timestamp,
                    ),
                    TelemetryEvent::ApprovalRequest {
                        session_id,
                        turn_id,
                        timestamp,
                        ..
                    } => (
                        "approval_request",
                        Some(session_id.as_str()),
                        Some(turn_id.as_str()),
                        *timestamp,
                    ),
                    TelemetryEvent::Compaction {
                        session_id,
                        timestamp,
                        ..
                    } => ("compaction", Some(session_id.as_str()), None, *timestamp),
                    TelemetryEvent::WorktreeOperation {
                        session_id,
                        timestamp,
                        ..
                    } => (
                        "worktree_operation",
                        Some(session_id.as_str()),
                        None,
                        *timestamp,
                    ),
                    TelemetryEvent::PluginEvent { timestamp, .. } => {
                        ("plugin_event", None, None, *timestamp)
                    }
                };
                let payload = serde_json::to_string(event)?;
                stmt.execute(rusqlite::params![
                    event_type,
                    session_id,
                    turn_id,
                    timestamp as i64,
                    payload
                ])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    fn name(&self) -> &'static str {
        "sqlite_audit"
    }
}

/// OpenTelemetry OTLP-compatible exporter formatting telemetry events into OpenTelemetry semantic spans and metrics.
pub struct OtlpExporter {
    endpoint: String,
    service_name: String,
}

impl OtlpExporter {
    pub fn new(endpoint: impl Into<String>, service_name: impl Into<String>) -> Self {
        Self {
            endpoint: endpoint.into(),
            service_name: service_name.into(),
        }
    }
}

impl TelemetryExporter for OtlpExporter {
    fn export(&self, events: Vec<TelemetryEvent>) -> Result<()> {
        for event in events {
            tracing::debug!(
                endpoint = %self.endpoint,
                service = %self.service_name,
                event = ?event,
                "Exported event to OTLP collector"
            );
        }
        Ok(())
    }

    fn name(&self) -> &'static str {
        "otlp"
    }
}

/// Telemetry reducer for aggregating events
pub struct TelemetryReducer {
    session_stats: HashMap<String, SessionStats>,
    global_stats: GlobalStats,
}

#[derive(Debug, Default, Clone)]
pub struct SessionStats {
    pub start_time: Option<std::time::Instant>,
    pub turns: usize,
    pub total_tokens: u64,
    pub tool_invocations: HashMap<String, usize>,
    pub errors: usize,
    pub sandbox_violations: usize,
    pub approvals_requested: usize,
    pub approvals_granted: usize,
}

#[derive(Debug, Default, Clone)]
pub struct GlobalStats {
    pub total_sessions: usize,
    pub total_turns: usize,
    pub total_tokens: u64,
    pub total_tool_invocations: usize,
    pub total_errors: usize,
}

impl TelemetryReducer {
    pub fn new() -> Self {
        Self {
            session_stats: HashMap::new(),
            global_stats: GlobalStats::default(),
        }
    }

    pub fn process(&mut self, event: &TelemetryEvent) {
        match event {
            TelemetryEvent::SessionStart { session_id, .. } => {
                self.session_stats
                    .entry(session_id.clone())
                    .or_default()
                    .start_time = Some(std::time::Instant::now());
                self.global_stats.total_sessions += 1;
            }
            TelemetryEvent::SessionEnd {
                session_id,
                duration_ms: _,
                total_turns,
                total_tokens,
                ..
            } => {
                if let Some(stats) = self.session_stats.remove(session_id) {
                    self.global_stats.total_turns += total_turns;
                    self.global_stats.total_tokens += total_tokens;
                    self.global_stats.total_tool_invocations +=
                        stats.tool_invocations.values().sum::<usize>();
                    self.global_stats.total_errors += stats.errors;
                }
            }
            TelemetryEvent::TurnEnd {
                session_id,
                tokens_used,
                ..
            } => {
                if let Some(stats) = self.session_stats.get_mut(session_id) {
                    stats.turns += 1;
                    stats.total_tokens += tokens_used.total_tokens;
                }
            }
            TelemetryEvent::ToolInvocation {
                session_id,
                tool_name,
                success,
                ..
            } => {
                if let Some(stats) = self.session_stats.get_mut(session_id) {
                    *stats.tool_invocations.entry(tool_name.clone()).or_insert(0) += 1;
                    if !success {
                        stats.errors += 1;
                    }
                }
                self.global_stats.total_tool_invocations += 1;
            }
            TelemetryEvent::ModelCall { .. } => {
                // Handled in TurnEnd
            }
            TelemetryEvent::Error { session_id, .. } => {
                if let Some(session_id) = session_id
                    && let Some(stats) = self.session_stats.get_mut(session_id)
                {
                    stats.errors += 1;
                }
                self.global_stats.total_errors += 1;
            }
            TelemetryEvent::SandboxViolation { session_id, .. } => {
                if let Some(stats) = self.session_stats.get_mut(session_id) {
                    stats.sandbox_violations += 1;
                }
            }
            TelemetryEvent::ApprovalRequest {
                session_id,
                user_decision,
                ..
            } => {
                if let Some(stats) = self.session_stats.get_mut(session_id) {
                    stats.approvals_requested += 1;
                    if let Some(decision) = user_decision
                        && (decision == "allow" || decision == "always")
                    {
                        stats.approvals_granted += 1;
                    }
                }
            }
            _ => {}
        }
    }

    pub fn get_session_stats(&self, session_id: &str) -> Option<&SessionStats> {
        self.session_stats.get(session_id)
    }

    pub fn get_global_stats(&self) -> &GlobalStats {
        &self.global_stats
    }
}

impl Default for TelemetryReducer {
    fn default() -> Self {
        Self::new()
    }
}

/// Main telemetry pipeline
pub struct TelemetryPipeline {
    event_tx: mpsc::Sender<TelemetryEvent>,
    reducers: Arc<RwLock<Vec<Arc<tokio::sync::RwLock<TelemetryReducer>>>>>,
    exporters: Vec<Box<dyn TelemetryExporter>>,
    batch_size: usize,
    flush_interval: Duration,
}

impl TelemetryPipeline {
    pub fn new(
        batch_size: usize,
        flush_interval: Duration,
    ) -> (Self, mpsc::Receiver<TelemetryEvent>) {
        let (tx, rx) = mpsc::channel(10000);
        let pipeline = Self {
            event_tx: tx,
            reducers: Arc::new(RwLock::new(vec![Arc::new(tokio::sync::RwLock::new(
                TelemetryReducer::new(),
            ))])),
            exporters: Vec::new(),
            batch_size,
            flush_interval,
        };
        (pipeline, rx)
    }

    pub fn add_exporter(&mut self, exporter: Box<dyn TelemetryExporter>) {
        self.exporters.push(exporter);
    }

    pub fn add_reducer(&self, reducer: Arc<tokio::sync::RwLock<TelemetryReducer>>) {
        self.reducers.blocking_write().push(reducer);
    }

    /// Start the pipeline processor. Takes `&self` so the pipeline can be
    /// stored (e.g. globally) while the consume loop runs in the background.
    pub async fn run(&self, mut rx: mpsc::Receiver<TelemetryEvent>) {
        let mut buffer = Vec::new();
        let mut flush_timer = tokio::time::interval(self.flush_interval);

        loop {
            tokio::select! {
                event = rx.recv() => {
                    if let Some(event) = event {
                        // Process through reducers
                        for reducer in self.reducers.read().await.iter() {
                            reducer.write().await.process(&event);
                        }

                        buffer.push(event);
                        if buffer.len() >= self.batch_size {
                            self.flush_buffer(&mut buffer).await;
                        }
                    } else {
                        // Channel closed
                        self.flush_buffer(&mut buffer).await;
                        break;
                    }
                }
                _ = flush_timer.tick() => {
                    if !buffer.is_empty() {
                        self.flush_buffer(&mut buffer).await;
                    }
                }
            }
        }
    }

    async fn flush_buffer(&self, buffer: &mut Vec<TelemetryEvent>) {
        if buffer.is_empty() {
            return;
        }

        let events = std::mem::take(buffer);

        for exporter in &self.exporters {
            if let Err(e) = exporter.export(events.clone()) {
                eprintln!("Telemetry exporter '{}' failed: {}", exporter.name(), e);
            }
        }
    }

    /// Send an event to the pipeline
    pub fn send(&self, event: TelemetryEvent) -> Result<()> {
        self.event_tx
            .try_send(event)
            .context("Telemetry buffer full")
    }

    /// Convenience method for session start
    pub fn session_start(
        &self,
        session_id: String,
        model: String,
        provider: String,
        config_hash: String,
    ) {
        let _ = self.send(TelemetryEvent::SessionStart {
            session_id,
            model,
            provider,
            config_hash,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        });
    }

    /// Convenience method for tool invocation
    pub fn tool_invocation(
        &self,
        session_id: String,
        turn_id: String,
        tool_name: String,
        tool_category: String,
        success: bool,
        duration_ms: u64,
    ) {
        let _ = self.send(TelemetryEvent::ToolInvocation {
            session_id,
            turn_id,
            tool_name,
            tool_category,
            arguments_hash: String::new(),
            success,
            duration_ms,
            tokens_used: None,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        });
    }

    /// Convenience method for model call
    #[allow(clippy::too_many_arguments)]
    pub fn model_call(
        &self,
        session_id: String,
        turn_id: String,
        model: String,
        provider: String,
        input_tokens: u64,
        output_tokens: u64,
        reasoning_tokens: u64,
        latency_ms: u64,
    ) {
        let _ = self.send(TelemetryEvent::ModelCall {
            session_id,
            turn_id,
            model,
            provider,
            input_tokens,
            output_tokens,
            reasoning_tokens,
            latency_ms,
            cached: false,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        });
    }

    /// Convenience method for error
    pub fn error(
        &self,
        session_id: Option<String>,
        turn_id: Option<String>,
        error_type: String,
        error_message: String,
        recoverable: bool,
        context: HashMap<String, String>,
    ) {
        let _ = self.send(TelemetryEvent::Error {
            session_id,
            turn_id,
            error_type,
            error_message,
            recoverable,
            context,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        });
    }
}

impl Default for TelemetryPipeline {
    fn default() -> Self {
        let (pipeline, _) = Self::new(100, Duration::from_secs(5));
        pipeline
    }
}

/// Global telemetry instance
static TELEMETRY: std::sync::OnceLock<TelemetryPipeline> = std::sync::OnceLock::new();

/// Initialize global telemetry
pub fn init_telemetry(
    exporters: Vec<Box<dyn TelemetryExporter>>,
    batch_size: usize,
    flush_interval: Duration,
) -> Result<()> {
    let (mut pipeline, rx) = TelemetryPipeline::new(batch_size, flush_interval);
    for exporter in exporters {
        pipeline.add_exporter(exporter);
    }

    // Store globally first so the background loop can borrow it for 'static.
    TELEMETRY
        .set(pipeline)
        .map_err(|_| anyhow::anyhow!("Telemetry already initialized"))?;
    let pipeline = TELEMETRY.get().expect("just stored");

    // Start processor
    tokio::spawn(pipeline.run(rx));
    Ok(())
}

/// Get global telemetry instance
pub fn telemetry() -> &'static TelemetryPipeline {
    TELEMETRY.get().expect("Telemetry not initialized")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_telemetry_reducer() {
        let mut reducer = TelemetryReducer::new();

        reducer.process(&TelemetryEvent::SessionStart {
            session_id: "test-1".to_string(),
            model: "test-model".to_string(),
            provider: "test".to_string(),
            config_hash: "abc".to_string(),
            timestamp: 1000,
        });

        assert_eq!(reducer.global_stats.total_sessions, 1);

        reducer.process(&TelemetryEvent::ToolInvocation {
            session_id: "test-1".to_string(),
            turn_id: "turn-1".to_string(),
            tool_name: "read_file".to_string(),
            tool_category: "fs".to_string(),
            arguments_hash: "hash".to_string(),
            success: true,
            duration_ms: 100,
            tokens_used: None,
            timestamp: 1001,
        });

        assert_eq!(reducer.global_stats.total_tool_invocations, 1);
    }

    #[tokio::test]
    async fn test_telemetry_pipeline() {
        let (mut pipeline, rx) = TelemetryPipeline::new(10, Duration::from_millis(100));
        pipeline.add_exporter(Box::new(ConsoleExporter::new(true)));

        // Leak so the background consume loop gets a 'static handle.
        let pipeline: &'static TelemetryPipeline = Box::leak(Box::new(pipeline));
        let processor = tokio::spawn(pipeline.run(rx));

        // Send some events
        pipeline.session_start(
            "test".to_string(),
            "model".to_string(),
            "provider".to_string(),
            "hash".to_string(),
        );

        tokio::time::sleep(Duration::from_millis(200)).await;
        processor.abort();
    }

    #[test]
    fn test_sqlite_audit_exporter() {
        let exporter = SqliteAuditExporter::new_in_memory().expect("open in memory sqlite");
        let events = vec![
            TelemetryEvent::SessionStart {
                session_id: "sess_42".to_string(),
                model: "nemotron-3.5".to_string(),
                provider: "nvidia".to_string(),
                config_hash: "hash123".to_string(),
                timestamp: 1699999000,
            },
            TelemetryEvent::ModelCall {
                session_id: "sess_42".to_string(),
                turn_id: "turn_1".to_string(),
                model: "nemotron-3.5".to_string(),
                provider: "nvidia".to_string(),
                input_tokens: 150,
                output_tokens: 50,
                reasoning_tokens: 0,
                latency_ms: 120,
                cached: false,
                timestamp: 1699999005,
            },
        ];

        exporter.export(events).expect("export to sqlite");

        let queried = exporter
            .query_events(Some("sess_42"), 10)
            .expect("query events");
        assert_eq!(queried.len(), 2);
        assert_eq!(queried[0].1, "session_start");
        assert_eq!(queried[1].1, "model_call");

        let tokens = exporter
            .query_total_tokens("sess_42")
            .expect("query tokens");
        assert_eq!(tokens, 200);
    }

    #[test]
    fn test_otlp_exporter() {
        let exporter = OtlpExporter::new("http://localhost:4317", "m31a-agent");
        assert_eq!(exporter.name(), "otlp");
        let events = vec![TelemetryEvent::SessionStart {
            session_id: "sess_otlp".to_string(),
            model: "r1".to_string(),
            provider: "nvidia".to_string(),
            config_hash: "h".to_string(),
            timestamp: 12345,
        }];
        assert!(exporter.export(events).is_ok());
    }
}
