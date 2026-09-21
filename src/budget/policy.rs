//! Budget Exhaustion Policy & State Transitions (BST-02, D-05).
//!
//! Differentiates soft consumable budgets from hard environmental/safety boundaries.

use serde::{Deserialize, Serialize};

use crate::budget::kind::BudgetKind;

/// Action triggered when a specific budget dimension is exhausted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BudgetExhaustionAction {
    /// Soft limit exhausted in interactive mode: pause mission for human approval.
    PauseForApproval,
    /// Soft limit exhausted in unattended mode: block mission to prevent unauthorized spend.
    Block,
    /// Hard safety/resource limit breached: fail closed unconditionally.
    FailClosed,
}

/// Policy evaluator for budget exhaustion events.
pub struct BudgetExhaustionPolicy;

impl BudgetExhaustionPolicy {
    /// Determine whether a budget dimension is a soft or hard limit.
    pub fn is_soft_limit(kind: BudgetKind) -> bool {
        matches!(
            kind,
            BudgetKind::Tokens
                | BudgetKind::CostUsd
                | BudgetKind::AgentSteps
                | BudgetKind::ModelCalls
        )
    }

    /// Determine the deterministic state transition when a budget is exhausted (D-05).
    pub fn determine_transition(kind: BudgetKind, is_interactive: bool) -> BudgetExhaustionAction {
        if Self::is_soft_limit(kind) {
            if is_interactive {
                BudgetExhaustionAction::PauseForApproval
            } else {
                BudgetExhaustionAction::Block
            }
        } else {
            // Hard safety limits (WallClock, CpuSeconds, MemoryBytes, ArtifactBytes, Retries, ConcurrentAgents)
            BudgetExhaustionAction::FailClosed
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_soft_limit_transitions() {
        assert_eq!(
            BudgetExhaustionPolicy::determine_transition(BudgetKind::Tokens, true),
            BudgetExhaustionAction::PauseForApproval
        );
        assert_eq!(
            BudgetExhaustionPolicy::determine_transition(BudgetKind::CostUsd, false),
            BudgetExhaustionAction::Block
        );
    }

    #[test]
    fn test_hard_limit_transitions() {
        assert_eq!(
            BudgetExhaustionPolicy::determine_transition(BudgetKind::WallClock, true),
            BudgetExhaustionAction::FailClosed
        );
        assert_eq!(
            BudgetExhaustionPolicy::determine_transition(BudgetKind::MemoryBytes, false),
            BudgetExhaustionAction::FailClosed
        );
        assert_eq!(
            BudgetExhaustionPolicy::determine_transition(BudgetKind::ArtifactBytes, true),
            BudgetExhaustionAction::FailClosed
        );
    }
}
