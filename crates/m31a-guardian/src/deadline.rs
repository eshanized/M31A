//! Review deadline tracking and timeout management.
//! Enforces timeout deadlines and remaining duration bounds for asynchronous security evaluations.

use std::time::{Duration, Instant};

/// Default overall review budget when timeout is omitted.
pub const DEFAULT_REVIEW_TIMEOUT: Duration = Duration::from_secs(90);

/// Tracks the deadline for an active guardian review turn.
#[derive(Clone, Copy, Debug)]
pub struct ReviewDeadline {
    deadline: Instant,
}

impl ReviewDeadline {
    pub fn new(timeout: Option<Duration>) -> Self {
        let dur = timeout.unwrap_or(DEFAULT_REVIEW_TIMEOUT);
        Self {
            deadline: Instant::now() + dur,
        }
    }

    /// Checks if the review deadline has expired.
    pub fn is_expired(&self) -> bool {
        Instant::now() >= self.deadline
    }

    /// Remaining time before deadline expiration.
    pub fn remaining(&self) -> Duration {
        self.deadline.saturating_duration_since(Instant::now())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deadline_expiration() {
        let dl = ReviewDeadline::new(Some(Duration::from_millis(10)));
        assert!(!dl.is_expired());
        std::thread::sleep(Duration::from_millis(15));
        assert!(dl.is_expired());
        assert_eq!(dl.remaining(), Duration::ZERO);
    }
}
