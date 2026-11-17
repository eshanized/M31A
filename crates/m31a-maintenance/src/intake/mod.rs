//! Signal Intake, Provider Adapters, and Normalization (`m31a-maintenance::intake`).

pub mod github;
pub mod local;
pub mod manager;

pub use github::GitHubSignalAdapter;
pub use local::LocalRepositorySignalSource;
pub use manager::{CaseManager, IngestionOutcome, SignalIntakeManager};

use std::collections::HashMap;
use anyhow::Result;
use async_trait::async_trait;

use crate::domain::{MaintenanceSignal, SignalProvider};

/// Provider-independent abstraction for maintenance signal sources.
#[async_trait]
pub trait SignalSource: Send + Sync {
    /// Identifier of the provider.
    fn provider(&self) -> SignalProvider;

    /// Validates webhook authenticity using constant-time signature comparison.
    fn validate_webhook(
        &self,
        headers: &HashMap<String, String>,
        body: &[u8],
        secret: &str,
    ) -> Result<bool>;

    /// Ingests and normalizes a webhook payload into canonical signals.
    fn parse_webhook(
        &self,
        repo_id: &str,
        event_name: &str,
        body: &[u8],
    ) -> Result<Vec<MaintenanceSignal>>;

    /// Pulls incremental signals since an optional watermark / cursor.
    async fn poll_signals(
        &self,
        repo_id: &str,
        watermark: Option<&str>,
    ) -> Result<(Vec<MaintenanceSignal>, Option<String>)>;
}
