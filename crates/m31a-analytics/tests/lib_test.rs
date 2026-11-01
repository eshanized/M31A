//! Dedicated integration tests for `m31a-analytics` (src/lib.rs).
//!
//! Validates:
//! - Root re-exports: `render_bar`, `render_sparkline`, `AnalyticsDashboard`, `SessionTokenMetrics`
//! - Cohesive end-to-end analytics workflow: metric ingestion, chart generation, and dashboard layout

use m31a_analytics::{AnalyticsDashboard, SessionTokenMetrics, render_bar, render_sparkline};

#[test]
fn test_root_reexports() {
    let bar = render_bar("Test", 25, 50, 30);
    assert!(bar.contains("Test"));

    let spark = render_sparkline(&[1, 5, 10]);
    assert_eq!(spark.chars().count(), 3);

    let mut metrics = SessionTokenMetrics::default();
    metrics.record_turn("baseline-model", 500, 100, 0, 150, 400);
    assert_eq!(metrics.turn_count, 1);

    let lines = AnalyticsDashboard::build_lines(&metrics, 80);
    assert!(!lines.is_empty());
}

#[test]
fn test_end_to_end_analytics_pipeline() {
    let mut metrics = SessionTokenMetrics::default();

    // Ingest simulated agent turn
    metrics.record_turn(
        "nvidia/llama-3.1-nemotron-70b-instruct",
        1500,
        350,
        50,
        280,
        950,
    );

    let lines = AnalyticsDashboard::build_lines(&metrics, 90);
    let full_text: String = lines
        .iter()
        .map(|line| {
            line.spans
                .iter()
                .map(|s| s.content.as_ref())
                .collect::<Vec<_>>()
                .join("")
        })
        .collect::<Vec<_>>()
        .join("\n");

    assert!(full_text.contains("nemotron"));
    assert!(full_text.contains("Tokens Total: 1850"));
    assert!(full_text.contains("1850"));
}
