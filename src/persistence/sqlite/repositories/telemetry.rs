//! SQLite repository for Telemetry Spans and Metric Samples (OBS-01, D-01).

use std::collections::HashMap;
use std::str::FromStr;

use sqlx::{Row, SqlitePool};

use crate::error::M31AError;
use crate::ids::{AgentId, MissionId, TaskId};
use crate::telemetry::types::{
    MetricSample, SpanKind, SpanStatus, TelemetrySpan, TelemetrySummary,
};

/// SQLite concrete repository for telemetry spans and metrics.
#[derive(Debug, Clone)]
pub struct SqliteTelemetryRepository {
    pool: SqlitePool,
}

impl SqliteTelemetryRepository {
    /// Create a new telemetry repository with the given pool.
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Record or update a telemetry span.
    pub async fn record_span(&self, span: &TelemetrySpan) -> Result<(), M31AError> {
        let task_id_str = span.task_id.map(|id| id.to_string());
        let agent_id_str = span.agent_id.map(|id| id.to_string());
        let kind_str = span.kind.to_string();
        let status_str = span.status.to_string();
        let attributes_str =
            serde_json::to_string(&span.attributes).unwrap_or_else(|_| "{}".to_string());

        sqlx::query(
            r#"
            INSERT INTO telemetry_spans (
                span_id, trace_id, parent_span_id, mission_id, task_id, agent_id,
                name, kind, start_time_us, end_time_us, duration_us, status,
                error_message, attributes_json
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(span_id) DO UPDATE SET
                end_time_us = excluded.end_time_us,
                duration_us = excluded.duration_us,
                status = excluded.status,
                error_message = excluded.error_message,
                attributes_json = excluded.attributes_json
            "#,
        )
        .bind(&span.span_id)
        .bind(&span.trace_id)
        .bind(&span.parent_span_id)
        .bind(span.mission_id.to_string())
        .bind(task_id_str)
        .bind(agent_id_str)
        .bind(&span.name)
        .bind(kind_str)
        .bind(span.start_time_us)
        .bind(span.end_time_us)
        .bind(span.duration_us)
        .bind(status_str)
        .bind(&span.error_message)
        .bind(attributes_str)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Record a single metric observation.
    pub async fn record_metric_sample(&self, sample: &MetricSample) -> Result<(), M31AError> {
        let labels_str = serde_json::to_string(&sample.labels).unwrap_or_else(|_| "{}".to_string());

        sqlx::query(
            r#"
            INSERT INTO metric_samples (
                mission_id, timestamp_us, metric_name, metric_value, metric_unit, labels_json
            ) VALUES (?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(sample.mission_id.to_string())
        .bind(sample.timestamp_us)
        .bind(&sample.metric_name)
        .bind(sample.metric_value)
        .bind(&sample.metric_unit)
        .bind(labels_str)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Retrieve all recorded spans for a mission ordered by start time.
    pub async fn get_spans_for_mission(
        &self,
        mission_id: &MissionId,
    ) -> Result<Vec<TelemetrySpan>, M31AError> {
        let rows = sqlx::query(
            r#"
            SELECT span_id, trace_id, parent_span_id, mission_id, task_id, agent_id,
                   name, kind, start_time_us, end_time_us, duration_us, status,
                   error_message, attributes_json
            FROM telemetry_spans
            WHERE mission_id = ?
            ORDER BY start_time_us ASC
            "#,
        )
        .bind(mission_id.to_string())
        .fetch_all(&self.pool)
        .await?;

        let mut spans = Vec::with_capacity(rows.len());
        for row in rows {
            let span_id: String = row.get("span_id");
            let trace_id: String = row.get("trace_id");
            let parent_span_id: Option<String> = row.get("parent_span_id");
            let m_id_str: String = row.get("mission_id");
            let m_id: MissionId = m_id_str
                .parse()
                .map_err(|e| M31AError::persistence(format!("invalid mission_id: {}", e)))?;
            let task_id_str: Option<String> = row.get("task_id");
            let task_id = task_id_str.and_then(|s| s.parse::<TaskId>().ok());
            let agent_id_str: Option<String> = row.get("agent_id");
            let agent_id = agent_id_str.and_then(|s| s.parse::<AgentId>().ok());
            let name: String = row.get("name");
            let kind_str: String = row.get("kind");
            let kind = SpanKind::from_str(&kind_str).map_err(|e| {
                M31AError::persistence(format!("corrupt span kind '{kind_str}': {e}"))
            })?;
            let start_time_us: i64 = row.get("start_time_us");
            let end_time_us: Option<i64> = row.get("end_time_us");
            let duration_us: Option<i64> = row.get("duration_us");
            let status_str: String = row.get("status");
            let status = SpanStatus::from_str(&status_str).map_err(|e| {
                M31AError::persistence(format!("corrupt span status '{status_str}': {e}"))
            })?;
            let error_message: Option<String> = row.get("error_message");
            let attr_str: String = row.get("attributes_json");
            let attributes = serde_json::from_str(&attr_str)
                .map_err(|e| M31AError::persistence(format!("corrupt span attributes: {e}")))?;

            spans.push(TelemetrySpan {
                span_id,
                trace_id,
                parent_span_id,
                mission_id: m_id,
                task_id,
                agent_id,
                name,
                kind,
                start_time_us,
                end_time_us,
                duration_us,
                status,
                error_message,
                attributes,
            });
        }

        Ok(spans)
    }

    /// Retrieve all recorded metrics for a mission ordered by timestamp.
    pub async fn get_metrics_for_mission(
        &self,
        mission_id: &MissionId,
    ) -> Result<Vec<MetricSample>, M31AError> {
        let rows = sqlx::query(
            r#"
            SELECT id, mission_id, timestamp_us, metric_name, metric_value, metric_unit, labels_json
            FROM metric_samples
            WHERE mission_id = ?
            ORDER BY timestamp_us ASC
            "#,
        )
        .bind(mission_id.to_string())
        .fetch_all(&self.pool)
        .await?;

        let mut samples = Vec::with_capacity(rows.len());
        for row in rows {
            let id: i64 = row.get("id");
            let m_id_str: String = row.get("mission_id");
            let m_id: MissionId = m_id_str
                .parse()
                .map_err(|e| M31AError::persistence(format!("invalid mission_id: {}", e)))?;
            let timestamp_us: i64 = row.get("timestamp_us");
            let metric_name: String = row.get("metric_name");
            let metric_value: f64 = row.get("metric_value");
            let metric_unit: String = row.get("metric_unit");
            let labels_str: String = row.get("labels_json");
            let labels = serde_json::from_str(&labels_str)
                .map_err(|e| M31AError::persistence(format!("corrupt metric labels: {e}")))?;

            samples.push(MetricSample {
                id: Some(id),
                mission_id: m_id,
                timestamp_us,
                metric_name,
                metric_value,
                metric_unit,
                labels,
            });
        }

        Ok(samples)
    }

    /// Compute consolidated telemetry summary for a mission.
    pub async fn get_summary(&self, mission_id: &MissionId) -> Result<TelemetrySummary, M31AError> {
        let spans = self.get_spans_for_mission(mission_id).await?;
        let metrics = self.get_metrics_for_mission(mission_id).await?;

        let total_spans = spans.len();
        let mut successful_spans = 0;
        let mut failed_spans = 0;
        let mut model_calls = 0;
        let mut tool_calls = 0;
        let mut verification_runs = 0;
        let mut spans_by_kind: HashMap<String, usize> = HashMap::new();

        let mut min_start = i64::MAX;
        let mut max_end = 0i64;

        for span in &spans {
            min_start = min_start.min(span.start_time_us);
            let end = span.end_time_us.unwrap_or(span.start_time_us);
            max_end = max_end.max(end);

            match span.status {
                SpanStatus::Ok => successful_spans += 1,
                SpanStatus::Error => failed_spans += 1,
                _ => {}
            }

            *spans_by_kind.entry(span.kind.to_string()).or_insert(0) += 1;

            match span.kind {
                SpanKind::Model => model_calls += 1,
                SpanKind::Tool => tool_calls += 1,
                SpanKind::Verification => verification_runs += 1,
                _ => {}
            }
        }

        let duration_ms = if total_spans > 0 && max_end >= min_start {
            ((max_end - min_start) / 1000).max(0) as u64
        } else {
            0
        };

        // Aggregate token usage and estimated cost from metrics
        let mut total_tokens = 0u64;
        let mut estimated_cost_usd = 0.0;

        for m in &metrics {
            if m.metric_name == "tokens_total" || m.metric_name == "tokens" {
                total_tokens += m.metric_value as u64;
            } else if m.metric_name == "cost_usd" || m.metric_name == "estimated_cost" {
                estimated_cost_usd += m.metric_value;
            }
        }

        Ok(TelemetrySummary {
            mission_id: *mission_id,
            total_spans,
            successful_spans,
            failed_spans,
            duration_ms,
            model_calls,
            tool_calls,
            verification_runs,
            total_tokens,
            estimated_cost_usd,
            spans_by_kind,
            metrics,
        })
    }
}
