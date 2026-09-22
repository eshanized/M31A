use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::controller::budget_tracker::BudgetKind;
use crate::controller::loop_detector::LoopSignature;

/// Exhaustive reasons why the Autonomy Controller halts execution (D-09).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ControllerHaltReason {
    FatalError { failure_class: String },
    UnrecoverableState { reason: String },
    Cancelled,
    Paused,
    BudgetExhausted { kind: BudgetKind },
    LoopDetected { signature: LoopSignature },
    MaxCyclesExceeded { limit: u64 },
    Blocked,
    MissionCompleted,
}

impl ControllerHaltReason {
    /// Rank order corresponding to fixed safety precedence (D-10):
    /// FatalError / UnrecoverableState (0) > Cancelled (1) > Paused (2) > BudgetExhausted (3) >
    /// LoopDetected (4) > MaxCyclesExceeded (5) > Blocked (6) > MissionCompleted (7)
    pub fn precedence_rank(&self) -> u8 {
        match self {
            Self::FatalError { .. } | Self::UnrecoverableState { .. } => 0,
            Self::Cancelled => 1,
            Self::Paused => 2,
            Self::BudgetExhausted { .. } => 3,
            Self::LoopDetected { .. } => 4,
            Self::MaxCyclesExceeded { .. } => 5,
            Self::Blocked => 6,
            Self::MissionCompleted => 7,
        }
    }

    /// Evaluates multiple active halt conditions and returns the primary halt reason
    /// while preserving all conditions for audit logs (D-10, Edge 5).
    pub fn resolve_primary(conditions: &[ControllerHaltReason]) -> Option<ControllerHaltReason> {
        conditions
            .iter()
            .min_by_key(|c| c.precedence_rank())
            .cloned()
    }
}

/// Audit structure capturing the resolved primary halt reason alongside all contributing conditions (D-10).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HaltEvaluation {
    pub primary: ControllerHaltReason,
    pub contributing_conditions: Vec<ControllerHaltReason>,
    pub evaluated_at: DateTime<Utc>,
}

impl HaltEvaluation {
    pub fn new(conditions: Vec<ControllerHaltReason>) -> Option<Self> {
        let primary = ControllerHaltReason::resolve_primary(&conditions)?;
        Some(Self {
            primary,
            contributing_conditions: conditions,
            evaluated_at: Utc::now(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::controller::progress::LoopStage;

    #[test]
    fn test_precedence_ranking() {
        let dummy_sig = LoopSignature {
            task_id: None,
            failure_class: "error".into(),
            recovery_strategy: "retry".into(),
            stage: LoopStage::Observe,
            progress_fingerprint: 0,
        };

        assert_eq!(
            ControllerHaltReason::FatalError {
                failure_class: "crash".into()
            }
            .precedence_rank(),
            0
        );
        assert_eq!(ControllerHaltReason::Cancelled.precedence_rank(), 1);
        assert_eq!(ControllerHaltReason::Paused.precedence_rank(), 2);
        assert_eq!(
            ControllerHaltReason::BudgetExhausted {
                kind: BudgetKind::Tokens
            }
            .precedence_rank(),
            3
        );
        assert_eq!(
            ControllerHaltReason::LoopDetected {
                signature: dummy_sig
            }
            .precedence_rank(),
            4
        );
        assert_eq!(
            ControllerHaltReason::MaxCyclesExceeded { limit: 10 }.precedence_rank(),
            5
        );
        assert_eq!(ControllerHaltReason::Blocked.precedence_rank(), 6);
        assert_eq!(ControllerHaltReason::MissionCompleted.precedence_rank(), 7);
    }

    #[test]
    fn test_resolve_primary_safety_invariants() {
        let dummy_sig = LoopSignature {
            task_id: None,
            failure_class: "error".into(),
            recovery_strategy: "retry".into(),
            stage: LoopStage::Observe,
            progress_fingerprint: 0,
        };

        // Simultaneous [MissionCompleted, Cancelled, FatalError] resolves to FatalError (D-10, Edge 5)
        let conditions = vec![
            ControllerHaltReason::MissionCompleted,
            ControllerHaltReason::Cancelled,
            ControllerHaltReason::FatalError {
                failure_class: "SecurityViolation".into(),
            },
        ];
        assert_eq!(
            ControllerHaltReason::resolve_primary(&conditions),
            Some(ControllerHaltReason::FatalError {
                failure_class: "SecurityViolation".into()
            })
        );

        // Simultaneous [MissionCompleted, BudgetExhausted] resolves to BudgetExhausted
        let conditions2 = vec![
            ControllerHaltReason::MissionCompleted,
            ControllerHaltReason::BudgetExhausted {
                kind: BudgetKind::Tokens,
            },
        ];
        assert_eq!(
            ControllerHaltReason::resolve_primary(&conditions2),
            Some(ControllerHaltReason::BudgetExhausted {
                kind: BudgetKind::Tokens
            })
        );

        // Simultaneous [Blocked, LoopDetected] resolves to LoopDetected
        let conditions3 = vec![
            ControllerHaltReason::Blocked,
            ControllerHaltReason::LoopDetected {
                signature: dummy_sig.clone(),
            },
        ];
        assert_eq!(
            ControllerHaltReason::resolve_primary(&conditions3),
            Some(ControllerHaltReason::LoopDetected {
                signature: dummy_sig
            })
        );
    }

    #[test]
    fn test_halt_evaluation_audit() {
        let conditions = vec![
            ControllerHaltReason::MissionCompleted,
            ControllerHaltReason::Cancelled,
        ];
        let eval = HaltEvaluation::new(conditions.clone()).unwrap();
        assert_eq!(eval.primary, ControllerHaltReason::Cancelled);
        assert_eq!(eval.contributing_conditions, conditions);
    }

    #[test]
    fn test_precedence_rank_ordering_strictly_monotonic() {
        // Arrange: all 8 halt reasons in descending order of safety importance
        let dummy_sig = LoopSignature {
            task_id: None,
            failure_class: "err".into(),
            recovery_strategy: "retry".into(),
            stage: LoopStage::Observe,
            progress_fingerprint: 0,
        };

        let ordered = [
            ControllerHaltReason::FatalError {
                failure_class: "fatal".into(),
            },
            ControllerHaltReason::Cancelled,
            ControllerHaltReason::Paused,
            ControllerHaltReason::BudgetExhausted {
                kind: BudgetKind::Tokens,
            },
            ControllerHaltReason::LoopDetected {
                signature: dummy_sig,
            },
            ControllerHaltReason::MaxCyclesExceeded { limit: 100 },
            ControllerHaltReason::Blocked,
            ControllerHaltReason::MissionCompleted,
        ];

        // Act & Assert: verify ranks are 0..=7 strictly ascending
        for (i, reason) in ordered.iter().enumerate() {
            assert_eq!(reason.precedence_rank(), i as u8);
        }

        // Verify empty slice resolves to None
        assert_eq!(ControllerHaltReason::resolve_primary(&[]), None);
        assert!(HaltEvaluation::new(vec![]).is_none());
    }
}
