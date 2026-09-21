//! Durable Budget Grants & Extensions (BST-02, D-05).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::ids::MissionId;
use crate::state::budget::ResourceBudget;

/// Explicit, durable authorization grant extending or setting a mission's budget.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BudgetGrant {
    pub grant_id: String,
    pub mission_id: MissionId,
    pub actor: String,
    pub reason: String,
    pub limits: ResourceBudget,
    pub granted_at: DateTime<Utc>,
}

impl BudgetGrant {
    pub fn new(
        mission_id: MissionId,
        actor: impl Into<String>,
        reason: impl Into<String>,
        limits: ResourceBudget,
    ) -> Self {
        Self {
            grant_id: Uuid::now_v7().to_string(),
            mission_id,
            actor: actor.into(),
            reason: reason.into(),
            limits,
            granted_at: Utc::now(),
        }
    }
}
