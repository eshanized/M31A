//! Integration tests for the M31A Analytics engine.
//!
//! Validates token consumption tracking, cost calculations, sparkline rendering, and dashboard layout.

use m31a_analytics::{AnalyticsDashboard, SessionTokenMetrics, render_bar, render_sparkline};

#[test]
fn test_session_token_metrics_multi_turn_aggregation() {
    let mut m = SessionTokenMetrics::default();

    // Turn 1: Nemotron 30b
    m.record_turn(
        "nvidia/nemotron-3.5-lightning-30b-a3b",
        500,
        200,
        50,
        300,
        1500,
    );
    assert_eq!(m.prompt_tokens, 500);
    assert_eq!(m.completion_tokens, 200);
    assert_eq!(m.reasoning_tokens, 50);
    assert_eq!(m.total_tokens, 700);
    assert_eq!(m.turn_count, 1);
    assert_eq!(m.ttft_ms, 300);
    assert_eq!(m.total_duration_ms, 1500);

    // Turn 2: Omni 30b
    m.record_turn(
        "nvidia/nemotron-3-nano-omni-30b-a3b-reasoning",
        800,
        400,
        150,
        400,
        2000,
    );
    assert_eq!(m.prompt_tokens, 1300);
    assert_eq!(m.completion_tokens, 600);
    assert_eq!(m.reasoning_tokens, 200);
    assert_eq!(m.total_tokens, 1900);
    assert_eq!(m.turn_count, 2);
    assert_eq!(m.total_duration_ms, 3500);

    // Verify model breakdown
    assert_eq!(
        m.model_breakdown
            .get("nvidia/nemotron-3.5-lightning-30b-a3b"),
        Some(&700)
    );
    assert_eq!(
        m.model_breakdown
            .get("nvidia/nemotron-3-nano-omni-30b-a3b-reasoning"),
        Some(&1200)
    );

    // Cost estimate should be > 0
    assert!(m.estimated_cost_usd > 0.0);
}

#[test]
fn test_render_bar_boundary_conditions() {
    // 0 max_val
    let b_zero = render_bar("Zero", 0, 0, 40);
    assert!(b_zero.contains("Zero"));
    assert!(b_zero.contains('0'));

    // 100% full
    let b_full = render_bar("Full", 100, 100, 40);
    assert!(b_full.contains("Full"));
    assert!(b_full.contains('█'));
    assert!(b_full.contains("100"));

    // Half full
    let b_half = render_bar("Half", 50, 100, 40);
    assert!(b_half.contains('█'));
    assert!(b_half.contains('░'));
}

#[test]
fn test_render_sparkline_varied_series() {
    // Empty
    assert_eq!(render_sparkline(&[]), "");

    // All zeroes
    let s_zeros = render_sparkline(&[0, 0, 0]);
    assert_eq!(s_zeros.chars().count(), 3);

    // Ascending
    let s_asc = render_sparkline(&[10, 20, 30, 40, 50]);
    assert_eq!(s_asc.chars().count(), 5);
}

#[test]
fn test_analytics_dashboard_build_lines() {
    let mut m = SessionTokenMetrics::default();
    m.record_turn(
        "nvidia/nemotron-3.5-lightning-30b-a3b",
        1000,
        500,
        100,
        250,
        1200,
    );

    let lines = AnalyticsDashboard::build_lines(&m, 80);
    assert!(!lines.is_empty());

    let full_text: String = lines
        .iter()
        .flat_map(|l| l.spans.iter().map(|s| s.content.as_ref()))
        .collect();

    assert!(full_text.contains("SESSION ANALYTICS"));
    assert!(full_text.contains("Tokens Total:"));
    assert!(full_text.contains("1500"));
    assert!(full_text.contains("Prompt"));
    assert!(full_text.contains("Completion"));
    assert!(full_text.contains("nvidia/nemotron-3.5-lightning-30b-a3b"));
}
