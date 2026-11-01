//! Policy denial circuit breaker tracking consecutive and sliding-window denials.
//! Implements a failure circuit breaker that halts execution if repeated security policy denials occur.

use std::collections::{HashMap, VecDeque};

pub const MAX_CONSECUTIVE_CYBER_GUARDIAN_DENIALS_PER_TURN: u32 = 1;
pub const MAX_CONSECUTIVE_GUARDIAN_DENIALS_PER_TURN: u32 = 3;
pub const MAX_RECENT_CYBER_AUTO_REVIEW_DENIALS_PER_TURN: u32 = 1;
pub const MAX_RECENT_AUTO_REVIEW_DENIALS_PER_TURN: u32 = 10;
pub const AUTO_REVIEW_DENIAL_WINDOW_SIZE: usize = 50;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GuardianRejectionCircuitBreakerPolicy {
    Standard,
    CyberModel,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GuardianRejectionCircuitBreakerAction {
    Continue,
    InterruptTurn {
        consecutive_denials: u32,
        recent_denials: u32,
    },
}

#[derive(Debug, Default)]
struct TurnCircuitState {
    consecutive_denials: u32,
    recent_denials: VecDeque<bool>,
    interrupt_triggered: bool,
}

/// Tracks denials across active turns, tripping if thresholds are exceeded.
#[derive(Debug, Default)]
pub struct GuardianRejectionCircuitBreaker {
    turns: HashMap<String, TurnCircuitState>,
}

impl GuardianRejectionCircuitBreaker {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear_turn(&mut self, turn_id: &str) {
        self.turns.remove(turn_id);
    }

    /// Records a review denial on a specific turn.
    pub fn record_denial(
        &mut self,
        turn_id: &str,
        policy: GuardianRejectionCircuitBreakerPolicy,
    ) -> GuardianRejectionCircuitBreakerAction {
        let turn = self.turns.entry(turn_id.to_string()).or_default();
        turn.consecutive_denials = turn.consecutive_denials.saturating_add(1);

        Self::record_recent_review(turn, true);
        let recent_denials = turn.recent_denials.iter().filter(|d| **d).count() as u32;

        let (max_consecutive, max_recent) = match policy {
            GuardianRejectionCircuitBreakerPolicy::Standard => (
                MAX_CONSECUTIVE_GUARDIAN_DENIALS_PER_TURN,
                MAX_RECENT_AUTO_REVIEW_DENIALS_PER_TURN,
            ),
            GuardianRejectionCircuitBreakerPolicy::CyberModel => (
                MAX_CONSECUTIVE_CYBER_GUARDIAN_DENIALS_PER_TURN,
                MAX_RECENT_CYBER_AUTO_REVIEW_DENIALS_PER_TURN,
            ),
        };

        if !turn.interrupt_triggered
            && (turn.consecutive_denials >= max_consecutive || recent_denials >= max_recent)
        {
            turn.interrupt_triggered = true;
            GuardianRejectionCircuitBreakerAction::InterruptTurn {
                consecutive_denials: turn.consecutive_denials,
                recent_denials,
            }
        } else {
            GuardianRejectionCircuitBreakerAction::Continue
        }
    }

    /// Records an approval on a specific turn (resets consecutive denials).
    pub fn record_approval(&mut self, turn_id: &str) {
        if let Some(turn) = self.turns.get_mut(turn_id) {
            turn.consecutive_denials = 0;
            Self::record_recent_review(turn, false);
        }
    }

    fn record_recent_review(turn: &mut TurnCircuitState, denied: bool) {
        if turn.recent_denials.len() >= AUTO_REVIEW_DENIAL_WINDOW_SIZE {
            turn.recent_denials.pop_front();
        }
        turn.recent_denials.push_back(denied);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_consecutive_denials_trip_standard() {
        let mut cb = GuardianRejectionCircuitBreaker::new();
        let turn = "turn_1";

        assert_eq!(
            cb.record_denial(turn, GuardianRejectionCircuitBreakerPolicy::Standard),
            GuardianRejectionCircuitBreakerAction::Continue
        );
        assert_eq!(
            cb.record_denial(turn, GuardianRejectionCircuitBreakerPolicy::Standard),
            GuardianRejectionCircuitBreakerAction::Continue
        );

        // 3rd denial trips
        let action = cb.record_denial(turn, GuardianRejectionCircuitBreakerPolicy::Standard);
        assert_eq!(
            action,
            GuardianRejectionCircuitBreakerAction::InterruptTurn {
                consecutive_denials: 3,
                recent_denials: 3,
            }
        );
    }

    #[test]
    fn test_approval_resets_consecutive_denials() {
        let mut cb = GuardianRejectionCircuitBreaker::new();
        let turn = "turn_2";

        cb.record_denial(turn, GuardianRejectionCircuitBreakerPolicy::Standard);
        cb.record_denial(turn, GuardianRejectionCircuitBreakerPolicy::Standard);
        cb.record_approval(turn);

        // Next denial is consecutive count 1, does not trip
        assert_eq!(
            cb.record_denial(turn, GuardianRejectionCircuitBreakerPolicy::Standard),
            GuardianRejectionCircuitBreakerAction::Continue
        );
    }

    #[test]
    fn test_cyber_model_trips_immediately() {
        let mut cb = GuardianRejectionCircuitBreaker::new();
        let turn = "turn_cyber";

        let action = cb.record_denial(turn, GuardianRejectionCircuitBreakerPolicy::CyberModel);
        assert_eq!(
            action,
            GuardianRejectionCircuitBreakerAction::InterruptTurn {
                consecutive_denials: 1,
                recent_denials: 1,
            }
        );
    }
}
