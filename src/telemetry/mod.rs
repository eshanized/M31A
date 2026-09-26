//! Local Observability, Telemetry & Distributed Correlation (OBS-01, OBS-02, OBS-03).

pub mod collector;
pub mod context;
pub mod inspect;
pub mod redactor;
pub mod stream;
pub mod types;

pub use collector::TelemetryCollector;
pub use context::CorrelationContext;
pub use inspect::{TelemetryInspectionReport, inspect_telemetry};
pub use redactor::SecretRedactor;
pub use stream::NdjsonStreamWriter;
pub use types::{MetricSample, SpanKind, SpanStatus, TelemetrySpan, TelemetrySummary};
