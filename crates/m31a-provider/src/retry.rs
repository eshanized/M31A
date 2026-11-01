//! Exponential backoff, jitter calculation, and error retry policy for outbound HTTP requests.
//!
//! Transparently retries transient network failures, HTTP 429 rate limits, and 5xx server errors.

use reqwest::StatusCode;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct RetryPolicy {
    pub max_retries: u32,
    pub base_delay_ms: u64,
    pub max_delay_ms: u64,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_retries: 6,
            base_delay_ms: 2000,
            max_delay_ms: 30000,
        }
    }
}

impl RetryPolicy {
    pub fn new(max_retries: u32, base_delay_ms: u64, max_delay_ms: u64) -> Self {
        Self {
            max_retries,
            base_delay_ms,
            max_delay_ms,
        }
    }

    /// Determines if an HTTP status code warrants a retry.
    pub fn is_retryable(&self, status: StatusCode) -> bool {
        status == StatusCode::TOO_MANY_REQUESTS
            || status == StatusCode::INTERNAL_SERVER_ERROR
            || status == StatusCode::BAD_GATEWAY
            || status == StatusCode::SERVICE_UNAVAILABLE
            || status == StatusCode::GATEWAY_TIMEOUT
    }

    /// Computes exponential backoff delay with pseudo-random jitter.
    pub fn calculate_delay(&self, attempt: u32, retry_after_header: Option<u64>) -> Duration {
        if let Some(secs) = retry_after_header {
            return Duration::from_secs(secs);
        }

        let exp = 2u64.saturating_pow(attempt);
        let calculated = self.base_delay_ms.saturating_mul(exp);
        let capped = calculated.min(self.max_delay_ms);

        // Simple deterministic jitter based on timestamp nanos to avoid stampeding
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.subsec_nanos() as u64)
            .unwrap_or(42);
        let jitter_factor = 0.75 + ((nanos % 500) as f64 / 1000.0); // 0.75 .. 1.25
        let jittered = (capped as f64 * jitter_factor) as u64;

        Duration::from_millis(jittered.max(self.base_delay_ms))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_retryable_codes() {
        let policy = RetryPolicy::default();
        assert!(policy.is_retryable(StatusCode::TOO_MANY_REQUESTS));
        assert!(policy.is_retryable(StatusCode::SERVICE_UNAVAILABLE));
        assert!(policy.is_retryable(StatusCode::INTERNAL_SERVER_ERROR));
        assert!(!policy.is_retryable(StatusCode::UNAUTHORIZED));
        assert!(!policy.is_retryable(StatusCode::BAD_REQUEST));
    }

    #[test]
    fn test_exponential_delay() {
        let policy = RetryPolicy::default();
        let delay_0 = policy.calculate_delay(0, None);
        let delay_3 = policy.calculate_delay(3, None);
        assert!(delay_3 > delay_0);

        let explicit = policy.calculate_delay(0, Some(5));
        assert_eq!(explicit, Duration::from_secs(5));
    }
}
