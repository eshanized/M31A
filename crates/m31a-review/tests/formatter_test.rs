//! Dedicated test for `crates/m31a-review/src/formatter.rs`.
//!
//! Validates `ReviewFormatter` terminal ANSI styling, GitHub Markdown output,
//! and structured JSON serialization.

use m31a_review::formatter::ReviewFormatter;
use m31a_review::model::{
    ReviewCategory, ReviewFinding, ReviewReport, ReviewSeverity, ReviewVerdict, RiskLevel,
};
use std::path::PathBuf;

#[test]
fn test_formatter_clean_report() {
    let report = ReviewReport {
        verdict: ReviewVerdict::Approve,
        risk_level: RiskLevel::Low,
        files_reviewed: 3,
        total_additions: 45,
        total_deletions: 12,
        findings: Vec::new(),
        summary: "Clean diff with no defects.".into(),
    };

    let term = ReviewFormatter::render_terminal(&report);
    assert!(term.contains("APPROVE"));
    assert!(term.contains("Files Reviewed:    3"));
    assert!(term.contains("[OK] No security vulnerabilities"));

    let md = ReviewFormatter::render_markdown(&report);
    assert!(md.contains("**Verdict**: Approve"));
    assert!(md.contains("Clean Diff"));

    let json = ReviewFormatter::render_json(&report).expect("json");
    assert!(json.contains("\"files_reviewed\": 3"));
}

#[test]
fn test_formatter_all_severities_and_snippets() {
    let severities = vec![
        (ReviewSeverity::Critical, "[CRITICAL]"),
        (ReviewSeverity::High, "[HIGH]"),
        (ReviewSeverity::Medium, "[MEDIUM]"),
        (ReviewSeverity::Low, "[LOW]"),
        (ReviewSeverity::Suggestion, "[SUGGESTION]"),
        (ReviewSeverity::Praise, "[PRAISE]"),
    ];

    let mut findings = Vec::new();
    for (sev, name) in severities {
        findings.push(ReviewFinding {
            file_path: PathBuf::from("src/test.rs"),
            line_number: Some(15),
            severity: sev,
            category: ReviewCategory::Security,
            title: format!("Title for {}", name),
            description: format!("Description for {}", name),
            code_snippet: Some("let dummy = true;".into()),
            suggested_fix: Some("let dummy = false;".into()),
        });
    }

    let report = ReviewReport {
        verdict: ReviewVerdict::RequestChanges,
        risk_level: RiskLevel::Critical,
        files_reviewed: 1,
        total_additions: 10,
        total_deletions: 5,
        findings,
        summary: "Comprehensive multi-severity review.".into(),
    };

    let term = ReviewFormatter::render_terminal(&report);
    assert!(term.contains("CRITICAL"));
    assert!(term.contains("HIGH"));
    assert!(term.contains("MEDIUM"));
    assert!(term.contains("SUGGESTION"));
    assert!(term.contains("Recommendation:"));
    assert!(term.contains("let dummy = true;"));

    let md = ReviewFormatter::render_markdown(&report);
    assert!(md.contains("#### 1. [CRITICAL] Title for [CRITICAL]"));
    assert!(md.contains("`src/test.rs`:L15"));
    assert!(md.contains("```\nlet dummy = true;\n```"));
    assert!(md.contains("- **Recommendation**: let dummy = false;"));

    let json = ReviewFormatter::render_json(&report).expect("render json");
    let deser: ReviewReport = serde_json::from_str(&json).expect("deser json");
    assert_eq!(deser.findings.len(), 6);
}
