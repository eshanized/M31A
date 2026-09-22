//! Model-aware token budgeting and token estimation through TokenizerAdapter (D-14, CTX-01).
//!
//! Provides exact BPE token counting via `tiktoken-rs` when available, conservative fallback
//! estimation (~3.5 chars/token), and dynamic output/reasoning headroom calculation per ModelTier.

use crate::model::router::resolver::ModelTier;
use std::sync::Arc;
use tiktoken_rs::CoreBPE;

/// Conservative token estimation fallback (~3.5 chars/token) when exact BPE is unavailable (D-14).
///
/// Formula: `(text.len() * 10 + 34) / 35`.
/// Guaranteed properties:
/// - Empty string -> 0 tokens.
/// - Single character -> 1 token.
/// - Never underestimates typical English words / code compared to BPE.
pub fn estimate_tokens_conservative(text: &str) -> usize {
    if text.is_empty() {
        return 0;
    }
    text.len().saturating_mul(10).div_ceil(35)
}

/// Dynamic output and reasoning headroom reserved per model tier (D-14, D-16).
///
/// Fast (8B): 2048 tokens
/// Standard (70B): 4096 tokens
/// Reasoning (R1): 8192 tokens
pub fn calculate_output_headroom(tier: ModelTier) -> usize {
    match tier {
        ModelTier::Fast => 2048,
        ModelTier::Standard => 4096,
        ModelTier::Reasoning => 8192,
    }
}

/// TokenizerAdapter supporting exact BPE token counting and conservative fallback.
#[derive(Clone)]
pub struct TokenizerAdapter {
    bpe: Option<Arc<CoreBPE>>,
    model_name: Option<String>,
}

impl TokenizerAdapter {
    /// Create a new TokenizerAdapter attempting to load the standard cl100k_base BPE tokenizer.
    pub fn new() -> Self {
        let bpe = tiktoken_rs::cl100k_base().ok().map(Arc::new);
        Self {
            bpe,
            model_name: None,
        }
    }

    /// Create a TokenizerAdapter configured for a specific model name.
    pub fn for_model(model_name: &str) -> Self {
        let bpe = tiktoken_rs::get_bpe_from_model(model_name)
            .or_else(|_| tiktoken_rs::cl100k_base())
            .ok()
            .map(Arc::new);
        Self {
            bpe,
            model_name: Some(model_name.to_string()),
        }
    }

    /// Create a conservative TokenizerAdapter without BPE (heuristic fallback only).
    pub fn conservative() -> Self {
        Self {
            bpe: None,
            model_name: None,
        }
    }

    /// Returns true if exact BPE token counting is active.
    pub fn is_exact(&self) -> bool {
        self.bpe.is_some()
    }

    /// Count tokens in `text` using exact BPE if available, or conservative fallback.
    pub fn count_tokens(&self, text: &str) -> usize {
        if let Some(ref bpe) = self.bpe {
            bpe.encode_with_special_tokens(text).len()
        } else {
            estimate_tokens_conservative(text)
        }
    }

    /// Calculate output headroom for a given ModelTier.
    pub fn output_headroom(&self, tier: ModelTier) -> usize {
        calculate_output_headroom(tier)
    }

    /// Model name if configured.
    pub fn model_name(&self) -> Option<&str> {
        self.model_name.as_deref()
    }
}

impl Default for TokenizerAdapter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_estimate_tokens_conservative() {
        assert_eq!(estimate_tokens_conservative(""), 0);
        assert_eq!(estimate_tokens_conservative("a"), 1);
        assert_eq!(estimate_tokens_conservative("abc"), 1);
        assert_eq!(estimate_tokens_conservative("abcd"), 2);

        // For a ~35 character sentence, conservative estimate should be ~10 tokens
        let s = "The quick brown fox jumps over dog.";
        let estimated = estimate_tokens_conservative(s);
        assert!((9..=12).contains(&estimated), "Estimated: {}", estimated);
    }

    #[test]
    fn test_bpe_exact_counting() {
        let adapter = TokenizerAdapter::new();
        assert!(
            adapter.is_exact(),
            "cl100k_base should initialize in tiktoken-rs"
        );

        let text = "Hello world! This is a test of BPE token counting.";
        let count = adapter.count_tokens(text);
        assert!(count > 0 && count < 20, "Token count: {}", count);

        let conservative_adapter = TokenizerAdapter::conservative();
        assert!(!conservative_adapter.is_exact());
        let conservative_count = conservative_adapter.count_tokens(text);
        assert!(conservative_count > 0);
    }

    #[test]
    fn test_calculate_output_headroom() {
        assert_eq!(calculate_output_headroom(ModelTier::Fast), 2048);
        assert_eq!(calculate_output_headroom(ModelTier::Standard), 4096);
        assert_eq!(calculate_output_headroom(ModelTier::Reasoning), 8192);
    }
}
