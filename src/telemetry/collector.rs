//! Central Telemetry Collector Service (OBS-01, OBS-02, D-01, D-02).
//!
//! Coordinates secret redaction, SQLite index persistence, and high-volume
//! NDJSON stream writing for spans and metrics.

use chrono::Utc;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use crate::error::M31AError;
use crate::ids::MissionId;
use crate::persistence::sqlite::repositories::SqliteTelemetryRepository;
use crate::telemetry::context::CorrelationContext;
use crate::telemetry::redactor::SecretRedactor;
use crate::telemetry::stream::NdjsonStreamWriter;
use crate::telemetry::types::{MetricSample, SpanKind, SpanStatus, TelemetrySpan};

/// Unified telemetry collector facade.
#[derive(Clone)]
pub struct TelemetryCollector {
    redactor: Arc<SecretRedactor>,
    repository: SqliteTelemetryRepository,
    stream_writer: NdjsonStreamWriter,
    in_flight_spans: Arc<RwLock<HashMap<String, TelemetrySpan>>>,
}

impl TelemetryCollector {
    /// Create a new collector instance with dependencies.
    pub fn new(
        redactor: Arc<SecretRedactor>,
        repository: SqliteTelemetryRepository,
        stream_writer: NdjsonStreamWriter,
    ) -> Self {
        Self {
            redactor,
            repository,
            stream_writer,
            in_flight_spans: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Access the underlying redactor.
    pub fn redactor(&self) -> &Arc<SecretRedactor> {
        &self.redactor
    }

    /// Start a new telemetry span from a correlation context.
    ///
    /// Returns the active span_id.
    pub async fn start_span(
        &self,
        context: &CorrelationContext,
        name: &str,
        kind: SpanKind,
    ) -> Result<String, M31AError> {
        let now_us = Utc::now().timestamp_micros();
        let mission_id = context.mission_id.unwrap_or_default();

        let sanitized_name = self.redactor.redact_string(name);

        let mut span = TelemetrySpan::new(
            context.span_id.clone(),
            context.trace_id.clone(),
            context.parent_span_id.clone(),
            mission_id,
            sanitized_name,
            kind,
            now_us,
        );
        span.task_id = context.task_id;
        span.agent_id = context.agent_id;

        // Persist initial running span to SQLite
        self.repository.record_span(&span).await?;

        // Also stream raw event
        let stream_event = serde_json::json!({
            "event": "span_start",
            "span_id": span.span_id,
            "trace_id": span.trace_id,
            "parent_span_id": span.parent_span_id,
            "name": span.name,
            "kind": span.kind,
            "start_time_us": span.start_time_us,
        });
        let _ = self
            .stream_writer
            .append_entry(&mission_id, &stream_event)
            .await;

        let span_id = span.span_id.clone();
        {
            let mut in_flight = self.in_flight_spans.write().expect("lock in_flight");
            in_flight.insert(span_id.clone(), span);
        }

        Ok(span_id)
    }

    /// Finish an active span, applying secret redaction and recording to both SQLite and NDJSON.
    pub async fn finish_span(
        &self,
        span_id: &str,
        status: SpanStatus,
        error_message: Option<String>,
        mut attributes: serde_json::Value,
    ) -> Result<(), M31AError> {
        let now_us = Utc::now().timestamp_micros();

        let mut span = {
            let mut in_flight = self.in_flight_spans.write().expect("lock in_flight");
            in_flight.remove(span_id)
        };

        let sanitized_error = error_message.map(|err| self.redactor.redact_string(&err));
        self.redactor.redact_value(&mut attributes);

        if let Some(ref mut s) = span {
            s.finish(now_us, status, sanitized_error.clone());
            s.attributes = attributes.clone();

            self.repository.record_span(s).await?;

            let stream_event = serde_json::json!({
                "event": "span_finish",
                "span_id": s.span_id,
                "trace_id": s.trace_id,
                "status": s.status,
                "duration_us": s.duration_us,
                "error_message": s.error_message,
                "attributes": s.attributes,
            });
            let _ = self
                .stream_writer
                .append_entry(&s.mission_id, &stream_event)
                .await;
        } else {
            // Span wasn't in memory (e.g. across restart); query from SQLite or update
            let stream_event = serde_json::json!({
                "event": "span_finish_untracked",
                "span_id": span_id,
                "status": status,
                "error_message": sanitized_error,
                "attributes": attributes,
            });
            // We cannot know the mission_id with certainty without querying, but we can do a fallback
            tracing::warn!(
                span_id,
                "Attempted to finish span not found in in-flight cache"
            );
            let _ = stream_event;
        }

        Ok(())
    }

    /// Record a metric sample with automatic redaction of label contents.
    pub async fn record_metric(
        &self,
        mission_id: MissionId,
        name: &str,
        value: f64,
        unit: &str,
        mut labels: serde_json::Value,
    ) -> Result<(), M31AError> {
        let now_us = Utc::now().timestamp_micros();
        let sanitized_name = self.redactor.redact_string(name);
        self.redactor.redact_value(&mut labels);

        let sample = MetricSample {
            id: None,
            mission_id,
            timestamp_us: now_us,
            metric_name: sanitized_name,
            metric_value: value,
            metric_unit: unit.to_string(),
            labels: labels.clone(),
        };

        self.repository.record_metric_sample(&sample).await?;

        let stream_event = serde_json::json!({
            "event": "metric_sample",
            "metric_name": sample.metric_name,
            "metric_value": sample.metric_value,
            "metric_unit": sample.metric_unit,
            "timestamp_us": sample.timestamp_us,
            "labels": sample.labels,
        });
        let _ = self
            .stream_writer
            .append_entry(&mission_id, &stream_event)
            .await;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::{AgentId, TaskId};
    use crate::persistence::sqlite::schema::initialize_database;
    use tempfile::tempdir;

    async fn setup_test_collector() -> (
        TelemetryCollector,
        SqliteTelemetryRepository,
        NdjsonStreamWriter,
        tempfile::TempDir,
    ) {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("collector_test.db");
        let pool = initialize_database(&db_path).await.unwrap();
        let repository = SqliteTelemetryRepository::new(pool);
        let stream_writer = NdjsonStreamWriter::new(dir.path().join("telemetry"));
        let redactor = Arc::new(SecretRedactor::new());

        let collector =
            TelemetryCollector::new(redactor, repository.clone(), stream_writer.clone());
        (collector, repository, stream_writer, dir)
    }

    #[tokio::test]
    async fn test_collector_initialization_and_accessor() {
        let (collector, _repo, _writer, _dir) = setup_test_collector().await;

        // Test accessor
        collector.redactor().register_secret("my_collector_secret");
        assert_eq!(
            collector
                .redactor()
                .redact_string("secret: my_collector_secret"),
            "secret: [REDACTED:EXACT_SECRET]"
        );

        // Initial in-flight spans should be empty
        let in_flight = collector.in_flight_spans.read().unwrap();
        assert!(in_flight.is_empty());
    }

    #[tokio::test]
    async fn test_collector_start_and_finish_span_lifecycle() {
        let (collector, repo, writer, _dir) = setup_test_collector().await;
        collector.redactor().register_secret("secret_api_key_xyz");

        let mission_id = MissionId::new();
        let task_id = TaskId::new();
        let agent_id = AgentId::new();
        let context = CorrelationContext::new_root(mission_id)
            .with_task(task_id)
            .with_agent(agent_id);

        let span_id = collector
            .start_span(&context, "span_with_secret_api_key_xyz", SpanKind::Model)
            .await
            .unwrap();

        assert_eq!(span_id, context.span_id);
        {
            let in_flight = collector.in_flight_spans.read().unwrap();
            assert_eq!(in_flight.len(), 1);
            assert!(in_flight.contains_key(&span_id));
        }

        let err_msg = "Failed with secret_api_key_xyz".to_string();
        let attributes = serde_json::json!({
            "token": "secret_api_key_xyz",
            "normal": "value"
        });

        collector
            .finish_span(&span_id, SpanStatus::Error, Some(err_msg), attributes)
            .await
            .unwrap();

        {
            let in_flight = collector.in_flight_spans.read().unwrap();
            assert!(in_flight.is_empty());
        }

        // Verify SQLite storage
        let spans = repo.get_spans_for_mission(&mission_id).await.unwrap();
        assert_eq!(spans.len(), 1);
        let span = &spans[0];
        assert_eq!(span.span_id, span_id);
        assert_eq!(span.name, "span_with_[REDACTED:EXACT_SECRET]");
        assert_eq!(span.status, SpanStatus::Error);
        assert_eq!(
            span.error_message,
            Some("Failed with [REDACTED:EXACT_SECRET]".to_string())
        );
        assert_eq!(span.attributes["token"], "[REDACTED:SENSITIVE_KEY]");
        assert_eq!(span.attributes["normal"], "value");

        // Verify NDJSON stream output
        let entries = writer.read_entries(&mission_id).await.unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0]["event"], "span_start");
        assert_eq!(entries[1]["event"], "span_finish");
        assert_eq!(entries[1]["status"], "error");
    }

    #[tokio::test]
    async fn test_collector_finish_untracked_span() {
        let (collector, _repo, _writer, _dir) = setup_test_collector().await;

        let result = collector
            .finish_span(
                "untracked_span_999",
                SpanStatus::Ok,
                Some("no error".to_string()),
                serde_json::json!({}),
            )
            .await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_collector_record_metric_with_redaction() {
        let (collector, repo, writer, _dir) = setup_test_collector().await;
        collector.redactor().register_secret("super_secret_val");

        let mission_id = MissionId::new();
        let labels = serde_json::json!({
            "env": "prod",
            "secret": "super_secret_val"
        });

        collector
            .record_metric(mission_id, "metric_super_secret_val", 42.5, "count", labels)
            .await
            .unwrap();

        // Verify SQLite storage
        let metrics = repo.get_metrics_for_mission(&mission_id).await.unwrap();
        assert_eq!(metrics.len(), 1);
        let metric = &metrics[0];
        assert_eq!(metric.metric_name, "metric_[REDACTED:EXACT_SECRET]");
        assert_eq!(metric.metric_value, 42.5);
        assert_eq!(metric.metric_unit, "count");
        assert_eq!(metric.labels["secret"], "[REDACTED:SENSITIVE_KEY]");

        // Verify NDJSON stream output
        let entries = writer.read_entries(&mission_id).await.unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0]["event"], "metric_sample");
        assert_eq!(entries[0]["metric_name"], "metric_[REDACTED:EXACT_SECRET]");
    }
}
