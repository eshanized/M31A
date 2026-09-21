//! Pre-admission Budget Reservation Receipt (BST-01, D-06).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Immutable receipt proving capacity was reserved prior to task admission.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReservationReceipt {
    pub reservation_id: String,
    pub reserved_tokens: u64,
    pub reserved_cost_usd: f64,
    pub reserved_worker: bool,
    pub reserved_artifact_bytes: u64,
    pub created_at: DateTime<Utc>,
}

impl ReservationReceipt {
    /// Issue a new reservation receipt with generated UUIDv7.
    pub fn new(
        reserved_tokens: u64,
        reserved_cost_usd: f64,
        reserved_worker: bool,
        reserved_artifact_bytes: u64,
    ) -> Self {
        Self {
            reservation_id: Uuid::now_v7().to_string(),
            reserved_tokens,
            reserved_cost_usd,
            reserved_worker,
            reserved_artifact_bytes,
            created_at: Utc::now(),
        }
    }
}
