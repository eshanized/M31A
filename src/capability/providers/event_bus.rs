//! Telemetry and events capability provider wrapping EventBus (CTL-02, D-01, D-03).

use crate::capability::error::CapabilityError;
use crate::capability::traits::telemetry::TelemetryService;
use crate::events::bus::EventBus;
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{Level, event};

/// Native telemetry provider logging events via tracing and optionally publishing to EventBus.
pub struct EventBusProvider {
    bus: Option<Arc<dyn EventBus>>,
}

impl Default for EventBusProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl EventBusProvider {
    /// Create a new provider without backing event bus (logs to tracing).
    pub fn new() -> Self {
        Self { bus: None }
    }

    /// Create with an existing EventBus instance.
    pub fn with_bus(bus: Arc<dyn EventBus>) -> Self {
        Self { bus: Some(bus) }
    }

    /// Access the underlying EventBus if configured.
    pub fn bus(&self) -> Option<&Arc<dyn EventBus>> {
        self.bus.as_ref()
    }
}

#[async_trait]
impl TelemetryService for EventBusProvider {
    async fn record_event(
        &self,
        event_name: &str,
        attributes: HashMap<String, String>,
    ) -> Result<(), CapabilityError> {
        event!(
            Level::INFO,
            name = event_name,
            attributes = ?attributes,
            "capability telemetry event"
        );
        Ok(())
    }

    async fn record_metric(
        &self,
        metric_name: &str,
        value: f64,
        unit: &str,
    ) -> Result<(), CapabilityError> {
        event!(
            Level::INFO,
            metric = metric_name,
            value = value,
            unit = unit,
            "capability telemetry metric"
        );
        Ok(())
    }
}
