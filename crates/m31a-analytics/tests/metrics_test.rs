//! Dedicated integration tests for `m31a-analytics` (src/metrics.rs).
//!
//! Validates:
//! - NVIDIA NIM pricing calculator per model architecture tier
//! - Multi-turn token accounting (prompt, completion, reasoning, total)
//! - Duration, TTFT, and turn history accumulation
//! - Per-model consumption breakdown
//! - Serde JSON serialization fidelity

use m31a_analytics::metrics::{SessionTokenMetrics, nim_model_pricing};

#[test]
fn test_nim_model_pricing_tiers() {
    let (in_nem, out_nem) = nim_model_pricing("nvidia/llama-3.1-nemotron-70b-instruct");
    assert_eq!(in_nem, 0.0000007);
    assert_eq!(out_nem, 0.0000020);

    let (in_r1, out_r1) = nim_model_pricing("deepseek-ai/deepseek-r1");
    assert_eq!(in_r1, 0.00000055);
    assert_eq!(out_r1, 0.00000219);

    let (in_mistral, out_mistral) = nim_model_pricing("mistralai/codestral-22b-v0.1");
    assert_eq!(in_mistral, 0.00000030);
    assert_eq!(out_mistral, 0.00000090);

    let (in_405b, out_405b) = nim_model_pricing("meta/llama-3.1-405b-instruct");
    assert_eq!(in_405b, 0.00000200);
    assert_eq!(out_405b, 0.00000600);

    let (in_def, out_def) = nim_model_pricing("unknown/custom-model");
    assert_eq!(in_def, 0.00000050);
    assert_eq!(out_def, 0.00000150);
}

#[test]
fn test_session_token_metrics_multi_turn_accumulation() {
    let mut metrics = SessionTokenMetrics::default();
    assert_eq!(metrics.turn_count, 0);
    assert_eq!(metrics.total_tokens, 0);

    // Turn 1: DeepSeek R1 reasoning turn
    metrics.record_turn("deepseek-r1", 1000, 400, 250, 420, 1100);
    assert_eq!(metrics.turn_count, 1);
    assert_eq!(metrics.prompt_tokens, 1000);
    assert_eq!(metrics.completion_tokens, 400);
    assert_eq!(metrics.reasoning_tokens, 250);
    assert_eq!(metrics.total_tokens, 1400);
    assert_eq!(metrics.ttft_ms, 420);
    assert_eq!(metrics.total_duration_ms, 1100);
    assert_eq!(metrics.turn_token_history, vec![1400]);
    assert_eq!(metrics.model_breakdown.get("deepseek-r1"), Some(&1400));
    assert!(metrics.estimated_cost_usd > 0.0);

    let cost_turn_1 = metrics.estimated_cost_usd;

    // Turn 2: Nemotron turn
    metrics.record_turn("nemotron-70b", 2000, 600, 0, 310, 850);
    assert_eq!(metrics.turn_count, 2);
    assert_eq!(metrics.prompt_tokens, 3000);
    assert_eq!(metrics.completion_tokens, 1000);
    assert_eq!(metrics.total_tokens, 4000);
    assert_eq!(metrics.ttft_ms, 310);
    assert_eq!(metrics.total_duration_ms, 1950);
    assert_eq!(metrics.turn_token_history, vec![1400, 2600]);
    assert_eq!(metrics.model_breakdown.get("nemotron-70b"), Some(&2600));
    assert!(metrics.estimated_cost_usd > cost_turn_1);
}

#[test]
fn test_session_token_metrics_serde_roundtrip() {
    let mut metrics = SessionTokenMetrics::default();
    metrics.record_turn("meta/llama-3.1-70b-instruct", 500, 200, 50, 200, 600);

    let json = serde_json::to_string(&metrics).expect("serialize metrics");
    let deserialized: SessionTokenMetrics =
        serde_json::from_str(&json).expect("deserialize metrics");

    assert_eq!(deserialized.turn_count, 1);
    assert_eq!(deserialized.prompt_tokens, 500);
    assert_eq!(deserialized.completion_tokens, 200);
    assert_eq!(deserialized.total_tokens, 700);
    assert_eq!(deserialized.ttft_ms, 200);
    assert_eq!(deserialized.turn_token_history, vec![700]);
}
