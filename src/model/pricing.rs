//! canonical pricing table and cost calculator for model invocations.
//!
//! computes estimated cost when the provider does not supply authoritative cost.
//! never returns synthetic 0.00 for unknown commercial models; unknown models yield
//! (None, CostProvenance::Unknown).

use crate::model::types::CostProvenance;

/// price per million tokens for input and output.
#[derive(Debug, Clone, Copy)]
pub struct ModelPrice {
    /// price in micro-dollars per 1 million input tokens ($1.00 = 1_000_000 micro-dollars)
    pub input_per_million_micro_usd: u64,
    /// price in micro-dollars per 1 million output tokens
    pub output_per_million_micro_usd: u64,
}

impl ModelPrice {
    pub const fn new_usd(input_usd: f64, output_usd: f64) -> Self {
        Self {
            input_per_million_micro_usd: (input_usd * 1_000_000.0) as u64,
            output_per_million_micro_usd: (output_usd * 1_000_000.0) as u64,
        }
    }

    pub fn calculate_cost_usd(&self, prompt_tokens: usize, completion_tokens: usize) -> f64 {
        let input_cost = (prompt_tokens as f64 * self.input_per_million_micro_usd as f64)
            / (1_000_000.0 * 1_000_000.0);
        let output_cost = (completion_tokens as f64 * self.output_per_million_micro_usd as f64)
            / (1_000_000.0 * 1_000_000.0);
        input_cost + output_cost
    }
}

/// lookup model pricing from the canonical pricing table.
pub fn lookup_price(provider: &str, model: &str) -> Option<ModelPrice> {
    let prov = provider.to_lowercase();
    let mod_id = model.to_lowercase();

    // local providers are explicitly free
    if prov == "ollama" || prov == "local" || mod_id.contains("ollama") {
        return Some(ModelPrice {
            input_per_million_micro_usd: 0,
            output_per_million_micro_usd: 0,
        });
    }

    // nvidia nim models
    if prov.contains("nvidia") || mod_id.starts_with("meta/llama") || mod_id.starts_with("nvidia/")
    {
        if mod_id.contains("8b") {
            return Some(ModelPrice::new_usd(0.18, 0.18));
        } else if mod_id.contains("70b") {
            return Some(ModelPrice::new_usd(0.70, 0.70));
        } else if mod_id.contains("405b") {
            return Some(ModelPrice::new_usd(5.00, 5.00));
        } else if mod_id.contains("340b") || mod_id.contains("nemotron") {
            return Some(ModelPrice::new_usd(4.00, 4.00));
        }
    }

    // anthropic models
    if prov.contains("anthropic") || mod_id.contains("claude") {
        if mod_id.contains("haiku") {
            return Some(ModelPrice::new_usd(0.25, 1.25));
        } else if mod_id.contains("opus") {
            return Some(ModelPrice::new_usd(15.00, 75.00));
        } else if mod_id.contains("sonnet") {
            return Some(ModelPrice::new_usd(3.00, 15.00));
        }
    }

    // openai models
    if prov.contains("openai") || mod_id.contains("gpt") || mod_id.contains("o1") {
        if mod_id.contains(
            /* COMPATIBILITY: historical pricing estimate */ "gpt-4o-mini",
        ) {
            return Some(ModelPrice::new_usd(0.15, 0.60));
        } else if mod_id.contains("o1-preview") {
            return Some(ModelPrice::new_usd(15.00, 60.00));
        } else if mod_id.contains("o1-mini") {
            return Some(ModelPrice::new_usd(3.00, 12.00));
        } else if mod_id.contains(
            /* COMPATIBILITY: historical pricing estimate */ "gpt-4o",
        ) {
            return Some(ModelPrice::new_usd(2.50, 10.00));
        }
    }

    None
}

/// calculate invocation cost with explicit provenance.
pub fn calculate_cost(
    provider: &str,
    model: &str,
    prompt_tokens: usize,
    completion_tokens: usize,
) -> (Option<f64>, CostProvenance) {
    if let Some(price) = lookup_price(provider, model) {
        let cost = price.calculate_cost_usd(prompt_tokens, completion_tokens);
        (Some(cost), CostProvenance::Estimated)
    } else {
        (None, CostProvenance::Unknown)
    }
}

/// calculate invocation cost directly from token usage.
pub fn calculate_cost_from_usage(
    provider: &str,
    model: &str,
    usage: &crate::model::types::TokenUsage,
) -> (Option<f64>, CostProvenance) {
    calculate_cost(
        provider,
        model,
        usage.prompt_tokens,
        usage.completion_tokens,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unknown_models_return_none_and_unknown_provenance() {
        assert!(lookup_price("nvidia", "unknown-random-model-xyz").is_none());
        assert!(lookup_price("custom_corp", "internal-v1").is_none());
        assert!(lookup_price("mock", "test-model").is_none());

        let (cost, prov) = calculate_cost("nvidia", "unknown-model", 1000, 500);
        assert_eq!(cost, None);
        assert_eq!(prov, CostProvenance::Unknown);
    }

    #[test]
    fn test_known_models_and_local_models_pricing() {
        let local = lookup_price("ollama", "llama3.2").expect("local is free");
        assert_eq!(local.input_per_million_micro_usd, 0);
        assert_eq!(local.output_per_million_micro_usd, 0);

        let (cost, prov) = calculate_cost("ollama", "llama3.2", 1000, 500);
        assert_eq!(cost, Some(0.0));
        assert_eq!(prov, CostProvenance::Estimated);

        let nim_8b = lookup_price("nvidia", "meta/llama-3.1-8b-instruct").expect("8b priced");
        assert_eq!(nim_8b.input_per_million_micro_usd, 180_000);
    }
}
