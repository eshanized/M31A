//! Capability health state machine and transition tracker (CTL-03, D-02).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};

/// The 4-state health state machine for capability instances (CTL-03, D-02).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityHealthState {
    /// Fully operational and responding to probes.
    Healthy,
    /// Experiencing non-fatal errors or elevated failure rate.
    Degraded,
    /// Circuit open; operations fail closed immediately without invoking provider.
    Unavailable,
    /// Cooldown expired; admitting a single probe to test recovery.
    HalfOpen,
}

impl CapabilityHealthState {
    /// Returns true if the capability can admit an execution request.
    pub fn is_admissible(&self) -> bool {
        matches!(self, Self::Healthy | Self::Degraded | Self::HalfOpen)
    }

    /// Returns true if the capability is healthy.
    pub fn is_healthy(&self) -> bool {
        matches!(self, Self::Healthy)
    }
}

/// Audit record for a capability health state transition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HealthTransitionRecord {
    pub from: CapabilityHealthState,
    pub to: CapabilityHealthState,
    pub reason: String,
    pub timestamp: DateTime<Utc>,
    pub consecutive_failures: u32,
}

/// Configuration for health state transitions.
#[derive(Debug, Clone)]
pub struct HealthConfig {
    /// Consecutive infrastructure failures to reach Degraded (default 1).
    pub degraded_threshold: u32,
    /// Consecutive infrastructure failures to trip to Unavailable (default 3).
    pub unavailable_threshold: u32,
    /// Duration to stay in Unavailable before transitioning to HalfOpen (default 500ms for fast tests/restarts).
    pub cooldown_duration: Duration,
}

impl Default for HealthConfig {
    fn default() -> Self {
        Self {
            degraded_threshold: 1,
            unavailable_threshold: 3,
            cooldown_duration: Duration::from_millis(500),
        }
    }
}

/// State machine tracker for a single capability instance's operational health.
#[derive(Debug)]
pub struct CapabilityHealthTracker {
    state: CapabilityHealthState,
    consecutive_failures: u32,
    config: HealthConfig,
    last_state_change: Instant,
    history: Vec<HealthTransitionRecord>,
}

impl CapabilityHealthTracker {
    /// Create a new health tracker in the Healthy state.
    pub fn new(config: HealthConfig) -> Self {
        Self::with_initial_state(config, CapabilityHealthState::Healthy)
    }

    /// Create a new health tracker with a specific initial state.
    pub fn with_initial_state(config: HealthConfig, state: CapabilityHealthState) -> Self {
        Self {
            state,
            consecutive_failures: 0,
            config,
            last_state_change: Instant::now(),
            history: Vec::new(),
        }
    }

    /// Get current health state, checking if Unavailable cooldown has expired to enter HalfOpen.
    pub fn current_state(&mut self) -> CapabilityHealthState {
        if self.state == CapabilityHealthState::Unavailable
            && self.last_state_change.elapsed() >= self.config.cooldown_duration
        {
            self.transition_to(
                CapabilityHealthState::HalfOpen,
                "cooldown expired, entering half-open probe state".to_string(),
            );
        }
        self.state
    }

    /// Peek at current health state without mutating.
    pub fn peek_state(&self) -> CapabilityHealthState {
        if self.state == CapabilityHealthState::Unavailable
            && self.last_state_change.elapsed() >= self.config.cooldown_duration
        {
            CapabilityHealthState::HalfOpen
        } else {
            self.state
        }
    }

    /// Record a successful execution or probe.
    pub fn record_success(&mut self) {
        // Ensure cooldown expiration is applied first
        let _ = self.current_state();

        match self.state {
            CapabilityHealthState::HalfOpen => {
                self.consecutive_failures = 0;
                self.transition_to(
                    CapabilityHealthState::Healthy,
                    "half-open probe succeeded, capability recovered".to_string(),
                );
            }
            CapabilityHealthState::Degraded => {
                self.consecutive_failures = 0;
                self.transition_to(
                    CapabilityHealthState::Healthy,
                    "successful execution restored health".to_string(),
                );
            }
            CapabilityHealthState::Healthy => {
                self.consecutive_failures = 0;
            }
            CapabilityHealthState::Unavailable => {
                // If an explicit success occurred while unavailable, restore to healthy
                self.consecutive_failures = 0;
                self.transition_to(
                    CapabilityHealthState::Healthy,
                    "explicit probe succeeded".to_string(),
                );
            }
        }
    }

    /// Record an execution failure.
    ///
    /// Per D-02: semantic task errors (e.g. file not found, bad syntax, test failure)
    /// must NEVER degrade capability health. Only infrastructure faults trip transitions.
    pub fn record_failure(&mut self, is_infrastructure_fault: bool, reason: impl Into<String>) {
        let reason = reason.into();
        if !is_infrastructure_fault {
            // Semantic task errors are valid execution outcomes and do not affect health
            return;
        }

        // Apply any pending cooldown transition
        let _ = self.current_state();

        self.consecutive_failures = self.consecutive_failures.saturating_add(1);

        match self.state {
            CapabilityHealthState::HalfOpen => {
                // Probe failed in HalfOpen; return to Unavailable and reset cooldown
                self.transition_to(
                    CapabilityHealthState::Unavailable,
                    format!("half-open recovery probe failed: {reason}"),
                );
            }
            CapabilityHealthState::Healthy => {
                if self.consecutive_failures >= self.config.unavailable_threshold {
                    self.transition_to(
                        CapabilityHealthState::Unavailable,
                        format!("circuit tripped to unavailable: {reason}"),
                    );
                } else if self.consecutive_failures >= self.config.degraded_threshold {
                    self.transition_to(
                        CapabilityHealthState::Degraded,
                        format!("elevated failure rate: {reason}"),
                    );
                }
            }
            CapabilityHealthState::Degraded => {
                if self.consecutive_failures >= self.config.unavailable_threshold {
                    self.transition_to(
                        CapabilityHealthState::Unavailable,
                        format!("circuit tripped to unavailable: {reason}"),
                    );
                }
            }
            CapabilityHealthState::Unavailable => {
                // Already unavailable; reset cooldown timer on repeated faults
                self.last_state_change = Instant::now();
            }
        }
    }

    /// Transition directly to a target state with an audit reason.
    fn transition_to(&mut self, new_state: CapabilityHealthState, reason: String) {
        let record = HealthTransitionRecord {
            from: self.state,
            to: new_state,
            reason,
            timestamp: Utc::now(),
            consecutive_failures: self.consecutive_failures,
        };
        self.state = new_state;
        self.last_state_change = Instant::now();
        self.history.push(record);
    }

    /// Returns the transition history records.
    pub fn history(&self) -> &[HealthTransitionRecord] {
        &self.history
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_health_state_transitions() {
        let config = HealthConfig {
            degraded_threshold: 1,
            unavailable_threshold: 3,
            cooldown_duration: Duration::from_millis(50),
        };
        let mut tracker = CapabilityHealthTracker::new(config);

        assert_eq!(tracker.current_state(), CapabilityHealthState::Healthy);

        // 1. Semantic failure does not degrade health
        tracker.record_failure(false, "file not found");
        assert_eq!(tracker.current_state(), CapabilityHealthState::Healthy);

        // 2. Infrastructure failure degrades health
        tracker.record_failure(true, "connection refused");
        assert_eq!(tracker.current_state(), CapabilityHealthState::Degraded);

        // 3. Second infrastructure failure remains Degraded
        tracker.record_failure(true, "connection timeout");
        assert_eq!(tracker.current_state(), CapabilityHealthState::Degraded);

        // 4. Third infrastructure failure trips to Unavailable
        tracker.record_failure(true, "process crash");
        assert_eq!(tracker.current_state(), CapabilityHealthState::Unavailable);
        assert!(!tracker.current_state().is_admissible());

        // 5. Sleep past cooldown duration to reach HalfOpen
        std::thread::sleep(Duration::from_millis(60));
        assert_eq!(tracker.current_state(), CapabilityHealthState::HalfOpen);
        assert!(tracker.current_state().is_admissible());

        // 6. HalfOpen recovery probe success restores to Healthy
        tracker.record_success();
        assert_eq!(tracker.current_state(), CapabilityHealthState::Healthy);
    }

    #[test]
    fn test_half_open_failure_returns_to_unavailable() {
        let config = HealthConfig {
            degraded_threshold: 1,
            unavailable_threshold: 2,
            cooldown_duration: Duration::from_millis(50),
        };
        let mut tracker = CapabilityHealthTracker::new(config);

        tracker.record_failure(true, "fault 1");
        tracker.record_failure(true, "fault 2");
        assert_eq!(tracker.current_state(), CapabilityHealthState::Unavailable);

        std::thread::sleep(Duration::from_millis(60));
        assert_eq!(tracker.current_state(), CapabilityHealthState::HalfOpen);

        // Probe fails in HalfOpen
        tracker.record_failure(true, "probe failed");
        assert_eq!(tracker.current_state(), CapabilityHealthState::Unavailable);
    }
}
