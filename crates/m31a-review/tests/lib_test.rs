//! Dedicated test for `crates/m31a-review/src/lib.rs`.
//!
//! Validates library re-exports, type system integration, and end-to-end
//! code review execution across models, analyzers, formatters, and lanes.

use m31a_review::{
    ReviewCategory, ReviewEngine, ReviewFinding, ReviewFormatter, ReviewReport, ReviewSeverity,
    ReviewVerdict, ReviewerLaneConfig, RiskLevel, StaticDiffAnalyzer,
};
use std::path::PathBuf;

#[test]
fn test_lib_exports_and_roundtrip_types() {
    let finding = ReviewFinding {
        file_path: PathBuf::from("crates/core/src/lib.rs"),
        line_number: Some(42),
        severity: ReviewSeverity::High,
        category: ReviewCategory::Security,
        title: "Test Finding".into(),
        description: "Test description".into(),
        code_snippet: Some("let x = 1;".into()),
        suggested_fix: Some("let x = 2;".into()),
    };

    let report = ReviewReport {
        verdict: ReviewVerdict::RequestChanges,
        risk_level: RiskLevel::High,
        files_reviewed: 1,
        total_additions: 10,
        total_deletions: 2,
        findings: vec![finding.clone()],
        summary: "Review completed".into(),
    };

    assert_eq!(report.high_count(), 1);
    assert_eq!(report.critical_count(), 0);
    assert_eq!(report.medium_count(), 0);

    let json_report = ReviewFormatter::render_json(&report).expect("serialize report");
    let deser_report: ReviewReport =
        serde_json::from_str(&json_report).expect("deserialize report");
    assert_eq!(deser_report.verdict, ReviewVerdict::RequestChanges);
    assert_eq!(deser_report.findings.len(), 1);
    assert_eq!(deser_report.findings[0].title, "Test Finding");
}

#[tokio::test]
async fn test_lib_pipeline_end_to_end() {
    let tmp = tempfile::tempdir().expect("create temp dir");
    let test_file = tmp.path().join("service.rs");
    std::fs::write(
        &test_file,
        "pub fn run() {\n    let token = \"nvapi-1234567890123456789012345\";\n}\n",
    )
    .expect("write test file");

    let diff =
        "--- a/service.rs\n+++ b/service.rs\n@@ -1,1 +1,3 @@\n pub fn run() {\n+    let token = \"nvapi-1234567890123456789012345\";\n }\n".to_string();

    // 1. Static Analyzer
    let static_findings = StaticDiffAnalyzer::analyze_diff(&diff);
    assert_eq!(static_findings.len(), 1);
    assert_eq!(static_findings[0].severity, ReviewSeverity::Critical);

    // 2. Review Engine
    let report = ReviewEngine::analyze(&diff);
    assert_eq!(report.verdict, ReviewVerdict::RequestChanges);
    assert_eq!(report.risk_level, RiskLevel::Critical);

    // 3. Review Formatter
    let term = ReviewFormatter::render_terminal(&report);
    assert!(term.contains("CRITICAL"));
    let md = ReviewFormatter::render_markdown(&report);
    assert!(md.contains("CRITICAL"));

    // 4. AI Lanes Dispatcher & Primary Consolidator
    let lanes = vec![ReviewerLaneConfig {
        id: "lane-sec".into(),
        model_name: "llama-3".into(),
        depth: "deep".into(),
        focus: ReviewCategory::Security,
    }];
    let consensus = ReviewEngine::analyze_with_ai_lanes(tmp.path(), &diff, &lanes)
        .await
        .expect("analyze with ai lanes");

    assert_eq!(consensus.verdict, ReviewVerdict::RequestChanges);
    assert_eq!(consensus.risk_level, RiskLevel::Critical);
    assert!(!consensus.corroborated_findings.is_empty());
}
