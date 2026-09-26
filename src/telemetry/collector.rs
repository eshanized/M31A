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
