//! Telemetry Inspection CLI Formatter (OBS-03, D-04).
//!
//! Provides human-readable console projections and machine-readable JSON/NDJSON exports
//! of mission telemetry spans and metric observations.

use std::fmt::Write as _;

use crate::error::M31AError;
use crate::ids::MissionId;
use crate::persistence::sqlite::repositories::SqliteTelemetryRepository;
use crate::telemetry::stream::NdjsonStreamWriter;

/// Output report produced from inspecting telemetry data for a mission.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TelemetryInspectionReport {
    pub text: String,
    pub data: serde_json::Value,
    pub exit_code: i32,
}

impl TelemetryInspectionReport {
    pub fn success(text: impl Into<String>, data: serde_json::Value) -> Self {
        Self {
            text: text.into(),
            data,
            exit_code: 0,
        }
    }

    pub fn with_exit_code(mut self, code: i32) -> Self {
        self.exit_code = code;
        self
    }
}

/// Run inspection over stored telemetry data for a mission.
pub async fn inspect_telemetry(
    repo: &SqliteTelemetryRepository,
    stream_writer: &NdjsonStreamWriter,
    mission_id: &MissionId,
    show_summary: bool,
    show_spans: bool,
    show_metrics: bool,
    export: Option<&str>,
) -> Result<TelemetryInspectionReport, M31AError> {
    let summary = repo.get_summary(mission_id).await?;
    let spans = repo.get_spans_for_mission(mission_id).await?;
    let metrics = repo.get_metrics_for_mission(mission_id).await?;

    // Handle machine-readable export requests
    if let Some(format) = export {
        match format.to_lowercase().as_str() {
            "ndjson" => {
                let stream_entries = stream_writer.read_entries(mission_id).await.map_err(|e| {
                    M31AError::persistence(format!("Failed to read NDJSON stream: {}", e))
                })?;

                let mut out = String::new();
                for entry in &stream_entries {
                    if let Ok(line) = serde_json::to_string(entry) {
                        out.push_str(&line);
                        out.push('\n');
                    }
                }
                return Ok(TelemetryInspectionReport::success(
                    out,
                    serde_json::json!(stream_entries),
                ));
            }
            _ => {
                let data = serde_json::json!({
                    "summary": summary,
                    "spans": spans,
                    "metrics": metrics,
                });
                let text = serde_json::to_string_pretty(&data).unwrap_or_else(|_| "{}".to_string());
                return Ok(TelemetryInspectionReport::success(text, data));
            }
        }
    }

    // Otherwise produce clean human-readable console tables
    let mut out = String::new();
    let _ = writeln!(&mut out, "=== Mission Telemetry: {} ===", mission_id);

    // If no specific flag is requested, default to summary
    let default_view = !show_summary && !show_spans && !show_metrics;

    if show_summary || default_view {
        let _ = writeln!(&mut out, "\n--- Summary ---");
        let _ = writeln!(&mut out, "Duration:           {} ms", summary.duration_ms);
        let _ = writeln!(&mut out, "Total Spans:        {}", summary.total_spans);
        let _ = writeln!(&mut out, "Successful Spans:   {}", summary.successful_spans);
        let _ = writeln!(&mut out, "Failed Spans:       {}", summary.failed_spans);
        let _ = writeln!(&mut out, "Model Invocations:  {}", summary.model_calls);
        let _ = writeln!(&mut out, "Tool Executions:    {}", summary.tool_calls);
        let _ = writeln!(
            &mut out,
            "Verification Runs:  {}",
            summary.verification_runs
        );
        let _ = writeln!(&mut out, "Total Tokens:       {}", summary.total_tokens);
        let _ = writeln!(
            &mut out,
            "Estimated Cost:     ${:.4}",
            summary.estimated_cost_usd
        );

        if !summary.spans_by_kind.is_empty() {
            let _ = writeln!(&mut out, "\nSpans by Kind:");
            for (kind, count) in &summary.spans_by_kind {
                let _ = writeln!(&mut out, "  - {}: {}", kind, count);
            }
        }
    }

    if show_spans {
        let _ = writeln!(&mut out, "\n--- Spans ({}) ---", spans.len());
        for span in &spans {
            let dur_str = span
                .duration_us
                .map(|d| format!("{} µs", d))
                .unwrap_or_else(|| "in-flight".to_string());
            let _ = writeln!(
                &mut out,
                "[{}] {:<12} {:<24} (status: {}, dur: {})",
                span.span_id, span.kind, span.name, span.status, dur_str
            );
            if let Some(ref err) = span.error_message {
                let _ = writeln!(&mut out, "      Error: {}", err);
            }
        }
    }

    if show_metrics {
        let _ = writeln!(&mut out, "\n--- Metrics ({}) ---", metrics.len());
        for metric in &metrics {
            let _ = writeln!(
                &mut out,
                "{:<24}: {:.2} {} (labels: {})",
                metric.metric_name, metric.metric_value, metric.metric_unit, metric.labels
            );
        }
    }

    let payload = serde_json::json!({
        "summary": summary,
        "span_count": spans.len(),
        "metric_count": metrics.len(),
    });

    Ok(TelemetryInspectionReport::success(out, payload))
}
