//! Telemetry and metrics recording service trait (CTL-01, CTL-02).

use crate::capability::error::CapabilityError;
use async_trait::async_trait;
use std::collections::HashMap;

/// Asynchronous service seam for telemetry, metrics, and structured events.
#[async_trait]
pub trait TelemetryService: Send + Sync + 'static {
    /// Emit a named telemetry event with structured key-value attributes.
    async fn record_event(
        &self,
        event_name: &str,
        attributes: HashMap<String, String>,
    ) -> Result<(), CapabilityError>;

    /// Record a numerical telemetry metric sample.
    async fn record_metric(
        &self,
        metric_name: &str,
        value: f64,
        unit: &str,
    ) -> Result<(), CapabilityError>;
}
