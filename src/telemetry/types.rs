//! Telemetry Domain Types & Models (OBS-01, D-01, D-03).

use std::collections::HashMap;
use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::ids::{AgentId, MissionId, TaskId};

/// Functional classification of an execution span.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpanKind {
    Mission,
    Task,
    Agent,
    Model,
    Tool,
    Job,
    Verification,
    Recovery,
}

impl fmt::Display for SpanKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SpanKind::Mission => write!(f, "mission"),
            SpanKind::Task => write!(f, "task"),
            SpanKind::Agent => write!(f, "agent"),
            SpanKind::Model => write!(f, "model"),
            SpanKind::Tool => write!(f, "tool"),
            SpanKind::Job => write!(f, "job"),
            SpanKind::Verification => write!(f, "verification"),
            SpanKind::Recovery => write!(f, "recovery"),
        }
    }
}

impl FromStr for SpanKind {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "mission" => Ok(SpanKind::Mission),
            "task" => Ok(SpanKind::Task),
            "agent" => Ok(SpanKind::Agent),
            "model" => Ok(SpanKind::Model),
            "tool" => Ok(SpanKind::Tool),
            "job" => Ok(SpanKind::Job),
            "verification" => Ok(SpanKind::Verification),
            "recovery" => Ok(SpanKind::Recovery),
            other => Err(format!("Unknown span kind: {}", other)),
        }
    }
}

/// Completion status of a telemetry span.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpanStatus {
    Running,
    Ok,
    Error,
    Cancelled,
}

impl fmt::Display for SpanStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SpanStatus::Running => write!(f, "running"),
            SpanStatus::Ok => write!(f, "ok"),
            SpanStatus::Error => write!(f, "error"),
            SpanStatus::Cancelled => write!(f, "cancelled"),
        }
    }
}

impl FromStr for SpanStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "running" => Ok(SpanStatus::Running),
            "ok" => Ok(SpanStatus::Ok),
            "error" => Ok(SpanStatus::Error),
            "cancelled" => Ok(SpanStatus::Cancelled),
            other => Err(format!("Unknown span status: {}", other)),
        }
    }
}

/// Strongly typed execution trace span.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TelemetrySpan {
    pub span_id: String,
    pub trace_id: String,
    pub parent_span_id: Option<String>,
    pub mission_id: MissionId,
    pub task_id: Option<TaskId>,
    pub agent_id: Option<AgentId>,
    pub name: String,
    pub kind: SpanKind,
    pub start_time_us: i64,
    pub end_time_us: Option<i64>,
    pub duration_us: Option<i64>,
    pub status: SpanStatus,
    pub error_message: Option<String>,
    pub attributes: serde_json::Value,
}

impl TelemetrySpan {
    pub fn new(
        span_id: String,
        trace_id: String,
        parent_span_id: Option<String>,
        mission_id: MissionId,
        name: String,
        kind: SpanKind,
        start_time_us: i64,
    ) -> Self {
        Self {
            span_id,
            trace_id,
            parent_span_id,
            mission_id,
            task_id: None,
            agent_id: None,
            name,
            kind,
            start_time_us,
            end_time_us: None,
            duration_us: None,
            status: SpanStatus::Running,
            error_message: None,
            attributes: serde_json::json!({}),
        }
    }

    pub fn finish(&mut self, end_time_us: i64, status: SpanStatus, error_message: Option<String>) {
        self.end_time_us = Some(end_time_us);
        self.duration_us = Some((end_time_us - self.start_time_us).max(0));
        self.status = status;
        self.error_message = error_message;
    }
}

/// Sampled numerical metric observation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MetricSample {
    pub id: Option<i64>,
    pub mission_id: MissionId,
    pub timestamp_us: i64,
    pub metric_name: String,
    pub metric_value: f64,
    pub metric_unit: String,
    pub labels: serde_json::Value,
}

/// Consolidated mission telemetry roll-up summary.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TelemetrySummary {
    pub mission_id: MissionId,
    pub total_spans: usize,
    pub successful_spans: usize,
    pub failed_spans: usize,
    pub duration_ms: u64,
    pub model_calls: usize,
    pub tool_calls: usize,
    pub verification_runs: usize,
    pub total_tokens: u64,
    pub estimated_cost_usd: f64,
    pub spans_by_kind: HashMap<String, usize>,
    pub metrics: Vec<MetricSample>,
}
