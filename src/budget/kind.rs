//! Budget dimension categories used for enforcement and policy routing (BST-01, D-12).
//!
//! This type is owned by the budget domain and consumed by both the budget enforcer
//! and the controller's BudgetTracker.

use serde::{Deserialize, Serialize};

/// Categories of resource budgets tracked across 10 dimensions (D-12, BST-01).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BudgetKind {
    WallClock,
    ConcurrentAgents,
    AgentSteps,
    ModelCalls,
    Tokens,
    CostUsd,
    CpuSeconds,
    MemoryBytes,
    ArtifactBytes,
    Retries,
}
