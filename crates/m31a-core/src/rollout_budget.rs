//! Rollout budget, sliding-window token rate limiter, and spend throttling.
//!
//! Enforces minute and hour token limits to prevent runaway loops, controls API
//! spend velocity, and computes precise backoff wait durations when quota is constrained.

use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Rate limit decision outcome.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RateLimitDecision {
    Allowed { remaining_tokens_in_window: u64 },
    RateLimited { wait_millis: u64, limit_tokens: u64 },
}

/// Token spend and rate limiting policy configuration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RolloutBudgetPolicy {
    pub max_tokens_per_minute: u64,
    pub max_tokens_per_hour: u64,
    pub max_budget_usd: Option<f64>,
}

impl Default for RolloutBudgetPolicy {
    fn default() -> Self {
        Self {
            max_tokens_per_minute: 100_000,
            max_tokens_per_hour: 2_000_000,
            max_budget_usd: None,
        }
    }
}

/// A recorded token consumption event with monotonic timestamp.
#[derive(Debug, Clone)]
struct TokenUsageEntry {
    instant: Instant,
    tokens: u64,
}

/// Thread-safe sliding window token rate limiter.
#[derive(Debug)]
pub struct SlidingWindowRateLimiter {
    policy: RolloutBudgetPolicy,
    history: Mutex<VecDeque<TokenUsageEntry>>,
}

impl SlidingWindowRateLimiter {
    pub fn new(policy: RolloutBudgetPolicy) -> Self {
        Self {
            policy,
            history: Mutex::new(VecDeque::new()),
        }
    }

    /// Check if consuming `tokens` right now is permitted under the minute and hour limits.
    /// If allowed, records the usage and returns remaining budget in the minute window.
    /// If not allowed, returns the required wait duration in milliseconds without consuming.
    pub fn check_and_consume(&self, tokens: u64) -> RateLimitDecision {
        let mut history = self.history.lock().unwrap();
        let now = Instant::now();

        // Evict entries older than 1 hour (3600 seconds)
        let one_hour_ago = now.checked_sub(Duration::from_secs(3600)).unwrap_or(now);
        while let Some(front) = history.front() {
            if front.instant < one_hour_ago {
                history.pop_front();
            } else {
                break;
            }
        }

        // Calculate usage in last 60 seconds and last 3600 seconds
        let one_minute_ago = now.checked_sub(Duration::from_secs(60)).unwrap_or(now);
        let mut tokens_last_minute: u64 = 0;
        let mut tokens_last_hour: u64 = 0;

        for entry in history.iter() {
            tokens_last_hour = tokens_last_hour.saturating_add(entry.tokens);
            if entry.instant >= one_minute_ago {
                tokens_last_minute = tokens_last_minute.saturating_add(entry.tokens);
            }
        }

        // Check minute limit
        if tokens_last_minute.saturating_add(tokens) > self.policy.max_tokens_per_minute {
            // Find earliest entry in the 60s window to calculate wait
            let wait_ms =
                if let Some(oldest_in_min) = history.iter().find(|e| e.instant >= one_minute_ago) {
                    let elapsed = now.duration_since(oldest_in_min.instant);
                    Duration::from_secs(60).saturating_sub(elapsed).as_millis() as u64
                } else {
                    1000
                };
            return RateLimitDecision::RateLimited {
                wait_millis: wait_ms.max(100),
                limit_tokens: self.policy.max_tokens_per_minute,
            };
        }

        // Check hour limit
        if tokens_last_hour.saturating_add(tokens) > self.policy.max_tokens_per_hour {
            let wait_ms = if let Some(oldest_in_hr) = history.front() {
                let elapsed = now.duration_since(oldest_in_hr.instant);
                Duration::from_secs(3600)
                    .saturating_sub(elapsed)
                    .as_millis() as u64
            } else {
                60_000
            };
            return RateLimitDecision::RateLimited {
                wait_millis: wait_ms.max(1000),
                limit_tokens: self.policy.max_tokens_per_hour,
            };
        }

        // Allowed - record usage
        history.push_back(TokenUsageEntry {
            instant: now,
            tokens,
        });

        let remaining = self
            .policy
            .max_tokens_per_minute
            .saturating_sub(tokens_last_minute.saturating_add(tokens));

        RateLimitDecision::Allowed {
            remaining_tokens_in_window: remaining,
        }
    }

    /// Reset limiter history.
    pub fn reset(&self) {
        self.history.lock().unwrap().clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sliding_window_rate_limiter_allow_and_limit() {
        let policy = RolloutBudgetPolicy {
            max_tokens_per_minute: 1_000,
            max_tokens_per_hour: 10_000,
            max_budget_usd: None,
        };
        let limiter = SlidingWindowRateLimiter::new(policy);

        // First consume 600 tokens -> allowed (400 remaining)
        match limiter.check_and_consume(600) {
            RateLimitDecision::Allowed {
                remaining_tokens_in_window,
            } => {
                assert_eq!(remaining_tokens_in_window, 400);
            }
            other => panic!("Expected Allowed, got {:?}", other),
        }

        // Next consume 300 tokens -> allowed (100 remaining)
        match limiter.check_and_consume(300) {
            RateLimitDecision::Allowed {
                remaining_tokens_in_window,
            } => {
                assert_eq!(remaining_tokens_in_window, 100);
            }
            other => panic!("Expected Allowed, got {:?}", other),
        }

        // Next attempt 200 tokens -> exceeds 1,000 limit -> RateLimited
        match limiter.check_and_consume(200) {
            RateLimitDecision::RateLimited {
                wait_millis,
                limit_tokens,
            } => {
                assert!(wait_millis > 0);
                assert_eq!(limit_tokens, 1_000);
            }
            other => panic!("Expected RateLimited, got {:?}", other),
        }
    }
}
