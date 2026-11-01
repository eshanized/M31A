//! Dedicated integration tests for `m31a-analytics` (src/dashboard.rs).
//!
//! Validates:
//! - Terminal dashboard layout generation via Ratatui text lines
//! - Overview card contents (tokens, cost, turn count, TTFT)
//! - Token distribution bars (Prompt, Completion, Reasoning R1)
//! - Multi-model consumption breakdown sections
//! - Activity sparkline trend integration and keybinding help indicators

use m31a_analytics::dashboard::AnalyticsDashboard;
use m31a_analytics::metrics::SessionTokenMetrics;

#[test]
fn test_dashboard_build_lines_empty_metrics() {
    let metrics = SessionTokenMetrics::default();
    let lines = AnalyticsDashboard::build_lines(&metrics, 80);

    assert!(!lines.is_empty());
    // Convert to flat string representation for assertion
    let rendered: String = lines
        .iter()
        .map(|l| {
            l.spans
                .iter()
                .map(|s| s.content.to_string())
                .collect::<Vec<_>>()
                .join("")
        })
        .collect::<Vec<_>>()
        .join("\n");

    assert!(rendered.contains("SESSION ANALYTICS"));
    assert!(rendered.contains("Tokens Total: 0"));
    assert!(rendered.contains("Est. Cost: $0.0000"));
    assert!(rendered.contains("Turns: 0"));
    assert!(rendered.contains("Prompt"));
    assert!(rendered.contains("Completion"));
    assert!(rendered.contains("Reasoning (R1)"));
    assert!(rendered.contains("Activity Sparkline:"));
    assert!(rendered.contains("Press [Esc] or [q] to close"));
}

#[test]
fn test_dashboard_build_lines_populated_metrics() {
    let mut metrics = SessionTokenMetrics::default();
    metrics.record_turn("deepseek-r1", 1200, 450, 300, 380, 1500);
    metrics.record_turn("nemotron-70b", 2500, 800, 0, 290, 1100);

    let lines = AnalyticsDashboard::build_lines(&metrics, 100);

    let rendered: String = lines
        .iter()
        .map(|l| {
            l.spans
                .iter()
                .map(|s| s.content.to_string())
                .collect::<Vec<_>>()
                .join("")
        })
        .collect::<Vec<_>>()
        .join("\n");

    // Total tokens: (1200+450) + (2500+800) = 1650 + 3300 = 4950
    assert!(rendered.contains("Tokens Total: 4950"));
    assert!(rendered.contains("Turns: 2"));
    assert!(rendered.contains("Last TTFT: 290ms"));

    // Verify model consumption section appears
    assert!(rendered.contains("Model Consumption:"));
    assert!(rendered.contains("deepseek-r1"));
    assert!(rendered.contains("nemotron-70b"));
}
