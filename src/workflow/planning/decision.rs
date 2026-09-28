//! Architectural decision statuses and governance rules.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Explicit status of an architectural decision or ADR (Section 17, 19).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Default,
)]
#[serde(rename_all = "snake_case")]
pub enum DecisionStatus {
    #[default]
    Proposed,
    Accepted,
    Rejected,
    Superseded,
    NeedsOperatorDecision,
}

impl DecisionStatus {
    /// True if and only if the decision is officially accepted and forms an authoritative constraint.
    pub fn is_authoritative(&self) -> bool {
        matches!(self, Self::Accepted)
    }

    /// Check whether a transition from this status to another status is permissible.
    pub fn can_transition_to(&self, new_status: DecisionStatus) -> bool {
        match (self, new_status) {
            (Self::Proposed, Self::Accepted) => true,
            (Self::Proposed, Self::Rejected) => true,
            (Self::Proposed, Self::NeedsOperatorDecision) => true,
            (Self::NeedsOperatorDecision, Self::Accepted) => true,
            (Self::NeedsOperatorDecision, Self::Rejected) => true,
            (Self::Accepted, Self::Superseded) => true,
            // Re-evaluating or proposing a superseded decision creates a new version instead
            _ => false,
        }
    }
}

impl fmt::Display for DecisionStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Proposed => write!(f, "proposed"),
            Self::Accepted => write!(f, "accepted"),
            Self::Rejected => write!(f, "rejected"),
            Self::Superseded => write!(f, "superseded"),
            Self::NeedsOperatorDecision => write!(f, "needs_operator_decision"),
        }
    }
}

impl std::str::FromStr for DecisionStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_lowercase().replace(['-', ' '], "_").as_str() {
            "proposed" => Ok(Self::Proposed),
            "accepted" => Ok(Self::Accepted),
            "rejected" => Ok(Self::Rejected),
            "superseded" => Ok(Self::Superseded),
            "needs_operator_decision" | "needs_review" | "needs_operator" => {
                Ok(Self::NeedsOperatorDecision)
            }
            other => Err(format!("Unknown decision status: {}", other)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decision_status_transitions() {
        assert!(DecisionStatus::Proposed.can_transition_to(DecisionStatus::Accepted));
        assert!(DecisionStatus::Proposed.can_transition_to(DecisionStatus::Rejected));
        assert!(DecisionStatus::Proposed.can_transition_to(DecisionStatus::NeedsOperatorDecision));
        assert!(DecisionStatus::Accepted.can_transition_to(DecisionStatus::Superseded));
        assert!(!DecisionStatus::Accepted.can_transition_to(DecisionStatus::Proposed));
        assert!(!DecisionStatus::Rejected.can_transition_to(DecisionStatus::Accepted));
    }

    #[test]
    fn test_authoritative_rule() {
        assert!(DecisionStatus::Accepted.is_authoritative());
        assert!(!DecisionStatus::Proposed.is_authoritative());
        assert!(!DecisionStatus::NeedsOperatorDecision.is_authoritative());
    }
}
