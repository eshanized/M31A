//! 100ms Delta Batcher for high-speed token streams (inspired by OpenCode's `publish-llm-event.ts`).
//!
//! Frontier models (e.g., Nemotron, Cerebras, Groq) generate at 100–250+ tokens/second.
//! Emitting individual token deltas directly onto event channels causes UI thread saturation,
//! terminal flicker, and excessive database locks.
//!
//! DeltaBatcher accumulates content and reasoning chunks and flushes them in batches,
//! while supporting immediate synchronous flushes on block boundaries.

use std::time::{Duration, Instant};

#[derive(Debug)]
pub struct DeltaBatcher {
    pending_content: String,
    pending_reasoning: String,
    last_flush: Instant,
    batch_interval: Duration,
}

impl DeltaBatcher {
    pub fn new(interval_ms: u64) -> Self {
        Self {
            pending_content: String::new(),
            pending_reasoning: String::new(),
            last_flush: Instant::now(),
            batch_interval: Duration::from_millis(interval_ms),
        }
    }

    pub fn push_content(&mut self, chunk: &str) {
        self.pending_content.push_str(chunk);
    }

    pub fn push_reasoning(&mut self, chunk: &str) {
        self.pending_reasoning.push_str(chunk);
    }

    /// Checks if the batch interval has elapsed since the last flush.
    pub fn is_ready(&self) -> bool {
        self.last_flush.elapsed() >= self.batch_interval
            && (!self.pending_content.is_empty() || !self.pending_reasoning.is_empty())
    }

    /// Extracts and clears accumulated content if any.
    pub fn take_content(&mut self) -> Option<String> {
        if self.pending_content.is_empty() {
            None
        } else {
            self.last_flush = Instant::now();
            Some(std::mem::take(&mut self.pending_content))
        }
    }

    /// Extracts and clears accumulated reasoning if any.
    pub fn take_reasoning(&mut self) -> Option<String> {
        if self.pending_reasoning.is_empty() {
            None
        } else {
            self.last_flush = Instant::now();
            Some(std::mem::take(&mut self.pending_reasoning))
        }
    }

    /// Flushes all pending text and reasoning unconditionally (e.g. at stream completion or block boundaries).
    pub fn flush_all(&mut self) -> (Option<String>, Option<String>) {
        self.last_flush = Instant::now();
        let content = if self.pending_content.is_empty() {
            None
        } else {
            Some(std::mem::take(&mut self.pending_content))
        };
        let reasoning = if self.pending_reasoning.is_empty() {
            None
        } else {
            Some(std::mem::take(&mut self.pending_reasoning))
        };
        (content, reasoning)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_delta_batcher_accumulation() {
        let mut batcher = DeltaBatcher::new(100);
        batcher.push_content("Hello ");
        batcher.push_content("world");
        batcher.push_reasoning("Thinking step 1");

        let (content, reasoning) = batcher.flush_all();
        assert_eq!(content.as_deref(), Some("Hello world"));
        assert_eq!(reasoning.as_deref(), Some("Thinking step 1"));

        // After flush, buffers should be empty
        let (content2, reasoning2) = batcher.flush_all();
        assert_eq!(content2, None);
        assert_eq!(reasoning2, None);
    }
}
