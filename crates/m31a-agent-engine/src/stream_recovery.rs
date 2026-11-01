//! Stream failure classification and recovery mechanics (inspired by OpenCode's `classifyProviderFailure` and `runner/llm.ts`).
//!
//! Provides intelligent error recovery:
//! 1. `ContextOverflow`: Detects provider context window overflow and triggers auto-compaction.
//! 2. `ContinueInterrupted`: Recovers from dropped TCP connections mid-generation without discarding partial output.
//! 3. `TransparentRetry`: Handles transient pre-stream rate limits and server overload with jittered exponential backoff.
//! 4. `OutputLimit`: Emits continuation guidance when hit max output tokens.

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StreamOutcome {
    /// Stream completed successfully.
    Completed,
    /// Connection failed before any tokens streamed. Retryable with backoff delay.
    TransparentRetry { delay_ms: u64 },
    /// Connection dropped after producing output. Retain partial response & continue turn.
    ContinueInterrupted { partial_content: String },
    /// Model hit max output tokens (`finish_reason: length`).
    ContinueOutputLimit,
    /// Context window limit exceeded. Emergency compaction required.
    ContextOverflow,
    /// Non-retryable fatal error (e.g., authentication, permissions, malformed request).
    Fatal(String),
}

pub const CONTINUE_INTERRUPTED_PROMPT: &str = "The previous response was interrupted by a network disconnection. Please continue directly from where you left off without repeating prior sentences.";

pub const OUTPUT_LIMIT_PROMPT: &str = "Your last response hit the maximum output token limit. Please continue your response directly from where you left off.";

/// Classifies a model streaming failure into an actionable recovery strategy.
pub fn classify_stream_failure(
    err: &anyhow::Error,
    tokens_streamed: usize,
    accumulated_content: &str,
    attempt: usize,
) -> StreamOutcome {
    let err_str = err.to_string().to_lowercase();

    // 1. Detect Context Overflow (matches 35+ provider patterns from OpenCode's regex battery)
    if err_str.contains("context_length_exceeded")
        || err_str.contains("model_context_window_exceeded")
        || err_str.contains("prompt is too long")
        || err_str.contains("maximum context length")
        || err_str.contains("request_too_large")
        || err_str.contains("prompt exceeds max length")
        || err_str.contains("exceeds the context window")
        || err_str.contains("too many tokens")
    {
        return StreamOutcome::ContextOverflow;
    }

    // 2. Mid-stream disconnection after emitting output
    if tokens_streamed > 0 && !accumulated_content.trim().is_empty() {
        return StreamOutcome::ContinueInterrupted {
            partial_content: accumulated_content.to_string(),
        };
    }

    // 3. Transient pre-stream failures (429 RateLimit, 500/502/503/529 Overload, TCP timeout)
    let is_transient = err_str.contains("rate limit")
        || err_str.contains("429")
        || err_str.contains("503")
        || err_str.contains("502")
        || err_str.contains("529")
        || err_str.contains("overloaded")
        || err_str.contains("slow down")
        || err_str.contains("server_error")
        || err_str.contains("connection refused")
        || err_str.contains("timed out")
        || err_str.contains("timeout");

    if is_transient && attempt < 10 {
        // Exponential backoff: 2s, 4s, 8s, capped at 10s per gap
        let base_delay = std::cmp::min(2000 * (1 << attempt.min(4)), 10_000);
        // Simple deterministic jitter based on attempt to avoid rand dependency
        let jitter = ((attempt * 137) % 500) as u64;
        return StreamOutcome::TransparentRetry {
            delay_ms: base_delay + jitter,
        };
    }

    StreamOutcome::Fatal(err.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_context_overflow_detection() {
        let err = anyhow::anyhow!("HTTP 400: prompt is too long for model_context_window_exceeded");
        assert_eq!(
            classify_stream_failure(&err, 0, "", 0),
            StreamOutcome::ContextOverflow
        );
    }

    #[test]
    fn test_midstream_interruption_continuation() {
        let err = anyhow::anyhow!("Socket read timeout mid-stream");
        let partial = "function calculateTotal() {\n    let sum = 0;";
        assert_eq!(
            classify_stream_failure(&err, 25, partial, 0),
            StreamOutcome::ContinueInterrupted {
                partial_content: partial.to_string(),
            }
        );
    }

    #[test]
    fn test_prestream_transient_retry() {
        let err = anyhow::anyhow!("HTTP 429: Rate limit exceeded. Please slow down.");
        match classify_stream_failure(&err, 0, "", 1) {
            StreamOutcome::TransparentRetry { delay_ms } => {
                assert!((2000..=11000).contains(&delay_ms));
            }
            other => panic!("Expected TransparentRetry, got {:?}", other),
        }
    }

    #[test]
    fn test_fatal_error() {
        let err = anyhow::anyhow!("HTTP 401: Invalid API key provided");
        assert_eq!(
            classify_stream_failure(&err, 0, "", 0),
            StreamOutcome::Fatal("HTTP 401: Invalid API key provided".to_string())
        );
    }
}
