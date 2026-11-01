//! Session token metrics, latency tracking, and cost accounting.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SessionTokenMetrics {
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub reasoning_tokens: u64,
    pub total_tokens: u64,
    pub estimated_cost_usd: f64,
    pub ttft_ms: u64,
    pub total_duration_ms: u64,
    pub turn_count: usize,
    pub model_breakdown: HashMap<String, u64>,
    #[serde(default)]
    pub turn_token_history: Vec<u64>,
}

/// Computes per-token input and output prices in USD for NVIDIA NIM models.
pub fn nim_model_pricing(model: &str) -> (f64, f64) {
    let m = model.to_lowercase();
    if m.contains("nemotron") {
        (0.0000007, 0.0000020) // $0.70 / 1M input, $2.00 / 1M output
    } else if m.contains("deepseek") || m.contains("r1") {
        (0.00000055, 0.00000219) // $0.55 / 1M input, $2.19 / 1M output
    } else if m.contains("codestral") || m.contains("mistral") {
        (0.00000030, 0.00000090) // $0.30 / 1M input, $0.90 / 1M output
    } else if m.contains("405b") {
        (0.00000200, 0.00000600) // $2.00 / 1M input, $6.00 / 1M output
    } else if m.contains("70b") || m.contains("llama") {
        (0.00000080, 0.00000240) // $0.80 / 1M input, $2.40 / 1M output
    } else {
        (0.00000050, 0.00000150) // Baseline NIM tier
    }
}

impl SessionTokenMetrics {
    pub fn record_turn(
        &mut self,
        model: &str,
        prompt: u64,
        completion: u64,
        reasoning: u64,
        ttft_ms: u64,
        duration_ms: u64,
    ) {
        self.prompt_tokens += prompt;
        self.completion_tokens += completion;
        self.reasoning_tokens += reasoning;
        let turn_total = prompt + completion;
        self.total_tokens += turn_total;
        self.turn_count += 1;
        self.ttft_ms = ttft_ms;
        self.total_duration_ms += duration_ms;
        self.turn_token_history.push(turn_total);

        *self.model_breakdown.entry(model.to_string()).or_insert(0) += turn_total;

        // Accurate model-specific NVIDIA NIM pricing
        let (input_rate, output_rate) = nim_model_pricing(model);
        let cost = (prompt as f64 * input_rate) + (completion as f64 * output_rate);
        self.estimated_cost_usd += cost;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_record_turn_accounting() {
        let mut m = SessionTokenMetrics::default();
        m.record_turn("deepseek-ai/deepseek-r1", 1000, 500, 300, 450, 1200);
        assert_eq!(m.total_tokens, 1500);
        assert_eq!(m.reasoning_tokens, 300);
        assert!(m.estimated_cost_usd > 0.0);
    }
}
