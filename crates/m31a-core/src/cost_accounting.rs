//! Model token cost and turn usage accounting engine.
//!
//! Provides granular cost calculations across sovereign LLM providers and models,
//! tracks per-turn and per-thread expenditure, breaks down token consumption by role
//! and model, and enforces budget caps (soft warnings and hard stops).

use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Pricing rates for a model per million tokens in USD.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelPricing {
    pub model_id: String,
    pub input_cost_per_million: f64,
    pub output_cost_per_million: f64,
    pub cache_read_cost_per_million: Option<f64>,
    pub cache_write_cost_per_million: Option<f64>,
}

impl ModelPricing {
    pub fn new(
        model_id: impl Into<String>,
        input_per_m: f64,
        output_per_m: f64,
        cache_read_per_m: Option<f64>,
        cache_write_per_m: Option<f64>,
    ) -> Self {
        Self {
            model_id: model_id.into(),
            input_cost_per_million: input_per_m,
            output_cost_per_million: output_per_m,
            cache_read_cost_per_million: cache_read_per_m,
            cache_write_cost_per_million: cache_write_per_m,
        }
    }

    /// Lookup pricing for a known model or return sensible sovereign default pricing.
    pub fn for_model(model: &str) -> Self {
        let m = model.to_lowercase();
        if m.contains("llama-3.3-70b") || m.contains("llama-3.1-70b") {
            Self::new(model, 0.70, 0.80, Some(0.18), None)
        } else if m.contains("llama-3.1-405b") || m.contains("llama-3.3-405b") {
            Self::new(model, 3.00, 3.00, Some(0.80), None)
        } else if m.contains("deepseek-r1") {
            Self::new(model, 0.55, 2.19, Some(0.14), None)
        } else if m.contains("deepseek-v3")
            || m.contains("deepseek-chat")
            || m.contains("deepseek-coder")
        {
            Self::new(model, 0.14, 0.28, Some(0.014), None)
        } else if m.contains("qwen-2.5-coder") || m.contains("qwen2.5-coder") {
            Self::new(model, 0.20, 0.20, Some(0.05), None)
        } else if m.contains("claude-3-5-sonnet") || m.contains("claude-3.5-sonnet") {
            Self::new(model, 3.00, 15.00, Some(0.30), Some(3.75))
        } else if m.contains("gpt-4o-mini") {
            Self::new(model, 0.15, 0.60, Some(0.075), None)
        } else if m.contains("gpt-4o") {
            Self::new(model, 2.50, 10.00, Some(1.25), None)
        } else if m.contains("mistral-large") {
            Self::new(model, 2.00, 6.00, Some(0.50), None)
        } else {
            // Default sovereign fallback ($0.50/M in, $1.00/M out, $0.10/M cached)
            Self::new(model, 0.50, 1.00, Some(0.10), None)
        }
    }

    /// Calculate costs in USD for a single invocation.
    pub fn compute_costs(
        &self,
        prompt_tokens: u64,
        completion_tokens: u64,
        cached_tokens: u64,
    ) -> (f64, f64, f64, f64) {
        let uncached_prompt = prompt_tokens.saturating_sub(cached_tokens);
        let input_cost = (uncached_prompt as f64 / 1_000_000.0) * self.input_cost_per_million;

        let cached_rate = self
            .cache_read_cost_per_million
            .unwrap_or(self.input_cost_per_million * 0.25);
        let cached_cost = (cached_tokens as f64 / 1_000_000.0) * cached_rate;

        let output_cost = (completion_tokens as f64 / 1_000_000.0) * self.output_cost_per_million;
        let total_cost = input_cost + cached_cost + output_cost;

        (input_cost, cached_cost, output_cost, total_cost)
    }
}

/// Cost and token breakdown for a single assistant turn.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TurnCostRecord {
    pub turn_index: u32,
    pub model: String,
    pub role: Option<String>,
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub cached_tokens: u64,
    pub total_tokens: u64,
    pub input_cost_usd: f64,
    pub cached_cost_usd: f64,
    pub output_cost_usd: f64,
    pub total_cost_usd: f64,
    pub duration_ms: u64,
    pub timestamp_millis: i64,
}

impl TurnCostRecord {
    pub fn new(
        turn_index: u32,
        model: impl Into<String>,
        role: Option<String>,
        prompt_tokens: u64,
        completion_tokens: u64,
        cached_tokens: u64,
        duration_ms: u64,
    ) -> Self {
        let model_str = model.into();
        let pricing = ModelPricing::for_model(&model_str);
        let (input_cost, cached_cost, output_cost, total_cost) =
            pricing.compute_costs(prompt_tokens, completion_tokens, cached_tokens);

        Self {
            turn_index,
            model: model_str,
            role,
            prompt_tokens,
            completion_tokens,
            cached_tokens,
            total_tokens: prompt_tokens + completion_tokens,
            input_cost_usd: input_cost,
            cached_cost_usd: cached_cost,
            output_cost_usd: output_cost,
            total_cost_usd: total_cost,
            duration_ms,
            timestamp_millis: Utc::now().timestamp_millis(),
        }
    }

    pub fn format_cost_summary(&self) -> String {
        format!(
            "Turn #{}: {} tokens ({} in, {} out, {} cached) | ${:.5} USD ({:.1}s)",
            self.turn_index,
            self.total_tokens,
            self.prompt_tokens,
            self.completion_tokens,
            self.cached_tokens,
            self.total_cost_usd,
            self.duration_ms as f64 / 1000.0
        )
    }
}

/// Aggregated usage statistics grouped by model or agent role.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ModelUsageBreakdown {
    pub name: String,
    pub turn_count: u32,
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub cached_tokens: u64,
    pub total_tokens: u64,
    pub total_cost_usd: f64,
}

/// Budget enforcement status.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum BudgetEnforcementAction {
    Allow,
    Warn {
        current_cost_usd: f64,
        threshold_usd: f64,
    },
    HaltLimitExceeded {
        current_cost_usd: f64,
        limit_usd: f64,
    },
}

/// Budget configuration thresholds for a session or task.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CostBudgetPolicy {
    pub warning_threshold_usd: Option<f64>,
    pub hard_limit_usd: Option<f64>,
}

impl CostBudgetPolicy {
    pub fn new(warning_usd: Option<f64>, hard_limit_usd: Option<f64>) -> Self {
        Self {
            warning_threshold_usd: warning_usd,
            hard_limit_usd,
        }
    }

    pub fn evaluate(&self, current_cost_usd: f64) -> BudgetEnforcementAction {
        if let Some(limit) = self.hard_limit_usd
            && current_cost_usd >= limit
        {
            return BudgetEnforcementAction::HaltLimitExceeded {
                current_cost_usd,
                limit_usd: limit,
            };
        }

        if let Some(threshold) = self.warning_threshold_usd
            && current_cost_usd >= threshold
        {
            return BudgetEnforcementAction::Warn {
                current_cost_usd,
                threshold_usd: threshold,
            };
        }

        BudgetEnforcementAction::Allow
    }
}

/// Thread-level expenditure tracker accumulating turn costs and usage distributions.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ThreadCostTracker {
    pub thread_id: String,
    pub turns: Vec<TurnCostRecord>,
    pub budget_policy: CostBudgetPolicy,
}

impl ThreadCostTracker {
    pub fn new(thread_id: impl Into<String>, budget_policy: CostBudgetPolicy) -> Self {
        Self {
            thread_id: thread_id.into(),
            turns: Vec::new(),
            budget_policy,
        }
    }

    pub fn record_turn(&mut self, record: TurnCostRecord) -> BudgetEnforcementAction {
        self.turns.push(record);
        let total = self.total_cost_usd();
        self.budget_policy.evaluate(total)
    }

    pub fn total_cost_usd(&self) -> f64 {
        self.turns.iter().map(|t| t.total_cost_usd).sum()
    }

    pub fn total_tokens(&self) -> u64 {
        self.turns.iter().map(|t| t.total_tokens).sum()
    }

    pub fn total_duration_ms(&self) -> u64 {
        self.turns.iter().map(|t| t.duration_ms).sum()
    }

    pub fn breakdown_by_model(&self) -> BTreeMap<String, ModelUsageBreakdown> {
        let mut map = BTreeMap::new();
        for t in &self.turns {
            let entry = map
                .entry(t.model.clone())
                .or_insert_with(|| ModelUsageBreakdown {
                    name: t.model.clone(),
                    ..Default::default()
                });
            entry.turn_count += 1;
            entry.prompt_tokens += t.prompt_tokens;
            entry.completion_tokens += t.completion_tokens;
            entry.cached_tokens += t.cached_tokens;
            entry.total_tokens += t.total_tokens;
            entry.total_cost_usd += t.total_cost_usd;
        }
        map
    }

    pub fn breakdown_by_role(&self) -> BTreeMap<String, ModelUsageBreakdown> {
        let mut map = BTreeMap::new();
        for t in &self.turns {
            let role_name = t.role.clone().unwrap_or_else(|| "default".to_string());
            let entry = map
                .entry(role_name.clone())
                .or_insert_with(|| ModelUsageBreakdown {
                    name: role_name,
                    ..Default::default()
                });
            entry.turn_count += 1;
            entry.prompt_tokens += t.prompt_tokens;
            entry.completion_tokens += t.completion_tokens;
            entry.cached_tokens += t.cached_tokens;
            entry.total_tokens += t.total_tokens;
            entry.total_cost_usd += t.total_cost_usd;
        }
        map
    }

    pub fn format_ledger(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!(
            "Cost Ledger for Thread [{}]\nTotal Expenditure: ${:.5} USD across {} turn(s) ({} total tokens)\n",
            self.thread_id,
            self.total_cost_usd(),
            self.turns.len(),
            self.total_tokens()
        ));
        out.push_str(
            "--------------------------------------------------------------------------------\n",
        );
        for t in &self.turns {
            out.push_str(&format!("  {}\n", t.format_cost_summary()));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pricing_for_model_llama_and_deepseek() {
        let llama = ModelPricing::for_model("meta/llama-3.3-70b-instruct");
        assert_eq!(llama.input_cost_per_million, 0.70);
        assert_eq!(llama.output_cost_per_million, 0.80);
        assert_eq!(llama.cache_read_cost_per_million, Some(0.18));

        let deepseek_r1 = ModelPricing::for_model("deepseek/deepseek-r1");
        assert_eq!(deepseek_r1.input_cost_per_million, 0.55);
        assert_eq!(deepseek_r1.output_cost_per_million, 2.19);
    }

    #[test]
    fn test_turn_cost_record_computation() {
        // 10,000 prompt tokens (2,000 cached), 500 completion tokens on llama-3.3-70b
        let turn = TurnCostRecord::new(
            1,
            "meta/llama-3.3-70b-instruct",
            Some("coordinator".to_string()),
            10_000,
            500,
            2_000,
            1200,
        );

        assert_eq!(turn.turn_index, 1);
        assert_eq!(turn.total_tokens, 10_500);

        // uncached input = 8,000 * 0.70 / 1M = 0.0056
        // cached input = 2,000 * 0.18 / 1M = 0.00036
        // output = 500 * 0.80 / 1M = 0.00040
        // total = 0.0056 + 0.00036 + 0.00040 = 0.00636
        let diff = (turn.total_cost_usd - 0.00636).abs();
        assert!(
            diff < 1e-6,
            "Cost should match formula: {}",
            turn.total_cost_usd
        );

        let summary = turn.format_cost_summary();
        assert!(summary.contains("Turn #1: 10500 tokens"));
        assert!(summary.contains("$0.00636 USD"));
    }

    #[test]
    fn test_budget_policy_enforcement() {
        let policy = CostBudgetPolicy::new(Some(0.01), Some(0.02));

        assert_eq!(policy.evaluate(0.005), BudgetEnforcementAction::Allow);

        match policy.evaluate(0.015) {
            BudgetEnforcementAction::Warn {
                current_cost_usd,
                threshold_usd,
            } => {
                assert_eq!(current_cost_usd, 0.015);
                assert_eq!(threshold_usd, 0.01);
            }
            other => panic!("Expected Warn, got {:?}", other),
        }

        match policy.evaluate(0.025) {
            BudgetEnforcementAction::HaltLimitExceeded {
                current_cost_usd,
                limit_usd,
            } => {
                assert_eq!(current_cost_usd, 0.025);
                assert_eq!(limit_usd, 0.02);
            }
            other => panic!("Expected HaltLimitExceeded, got {:?}", other),
        }
    }

    #[test]
    fn test_thread_cost_tracker_accumulation_and_breakdowns() {
        let mut tracker = ThreadCostTracker::new("thread-xyz-123", CostBudgetPolicy::default());

        tracker.record_turn(TurnCostRecord::new(
            1,
            "meta/llama-3.3-70b-instruct",
            Some("engineer".to_string()),
            5_000,
            1_000,
            0,
            800,
        ));

        tracker.record_turn(TurnCostRecord::new(
            2,
            "deepseek/deepseek-r1",
            Some("architect".to_string()),
            10_000,
            2_000,
            1_000,
            2500,
        ));

        assert_eq!(tracker.turns.len(), 2);
        assert_eq!(tracker.total_tokens(), 6_000 + 12_000);
        assert!(tracker.total_cost_usd() > 0.0);

        let model_breakdown = tracker.breakdown_by_model();
        assert_eq!(model_breakdown.len(), 2);
        assert!(model_breakdown.contains_key("meta/llama-3.3-70b-instruct"));
        assert!(model_breakdown.contains_key("deepseek/deepseek-r1"));

        let role_breakdown = tracker.breakdown_by_role();
        assert_eq!(role_breakdown.len(), 2);
        assert!(role_breakdown.contains_key("engineer"));
        assert!(role_breakdown.contains_key("architect"));

        let ledger = tracker.format_ledger();
        assert!(ledger.contains("Cost Ledger for Thread [thread-xyz-123]"));
        assert!(ledger.contains("Turn #1:"));
        assert!(ledger.contains("Turn #2:"));
    }
}
