//! Dedicated test for `crates/m31a-review/src/model.rs`.
//!
//! Validates review domain models, severity ordering, category string mappings,
//! risk levels, verdicts, and report aggregators.

use m31a_review::model::{
    ReviewCategory, ReviewFinding, ReviewReport, ReviewSeverity, ReviewVerdict, RiskLevel,
};
use std::collections::HashSet;
use std::path::PathBuf;

#[test]
fn test_severity_as_str_and_ordering() {
    assert_eq!(ReviewSeverity::Critical.as_str(), "CRITICAL");
    assert_eq!(ReviewSeverity::High.as_str(), "HIGH");
    assert_eq!(ReviewSeverity::Medium.as_str(), "MEDIUM");
    assert_eq!(ReviewSeverity::Low.as_str(), "LOW");
    assert_eq!(ReviewSeverity::Suggestion.as_str(), "SUGGESTION");
    assert_eq!(ReviewSeverity::Praise.as_str(), "PRAISE");

    // Rust derive Ord: earlier enum variant is considered "less than" later
    assert!(ReviewSeverity::Critical < ReviewSeverity::High);
    assert!(ReviewSeverity::High < ReviewSeverity::Medium);
    assert!(ReviewSeverity::Medium < ReviewSeverity::Low);
    assert!(ReviewSeverity::Low < ReviewSeverity::Suggestion);
    assert!(ReviewSeverity::Suggestion < ReviewSeverity::Praise);

    let mut severities = vec![
        ReviewSeverity::Praise,
        ReviewSeverity::Critical,
        ReviewSeverity::Medium,
        ReviewSeverity::High,
    ];
    severities.sort();
    assert_eq!(
        severities,
        vec![
            ReviewSeverity::Critical,
            ReviewSeverity::High,
            ReviewSeverity::Medium,
            ReviewSeverity::Praise,
        ]
    );
}

#[test]
fn test_category_as_str_and_hashing() {
    assert_eq!(ReviewCategory::Security.as_str(), "Security");
    assert_eq!(ReviewCategory::Correctness.as_str(), "Correctness");
    assert_eq!(ReviewCategory::Performance.as_str(), "Performance");
    assert_eq!(ReviewCategory::Architecture.as_str(), "Architecture");
    assert_eq!(ReviewCategory::Maintainability.as_str(), "Maintainability");

    let mut set = HashSet::new();
    set.insert(ReviewCategory::Security);
    set.insert(ReviewCategory::Correctness);
    set.insert(ReviewCategory::Security); // duplicate

    assert_eq!(set.len(), 2);
    assert!(set.contains(&ReviewCategory::Security));
    assert!(set.contains(&ReviewCategory::Correctness));
}

#[test]
fn test_report_counters_and_serialization() {
    let findings = vec![
        ReviewFinding {
            file_path: PathBuf::from("a.rs"),
            line_number: Some(1),
            severity: ReviewSeverity::Critical,
            category: ReviewCategory::Security,
            title: "Crit 1".into(),
            description: "".into(),
            code_snippet: None,
            suggested_fix: None,
        },
        ReviewFinding {
            file_path: PathBuf::from("b.rs"),
            line_number: Some(2),
            severity: ReviewSeverity::Critical,
            category: ReviewCategory::Security,
            title: "Crit 2".into(),
            description: "".into(),
            code_snippet: None,
            suggested_fix: None,
        },
        ReviewFinding {
            file_path: PathBuf::from("c.rs"),
            line_number: Some(3),
            severity: ReviewSeverity::High,
            category: ReviewCategory::Correctness,
            title: "High 1".into(),
            description: "".into(),
            code_snippet: None,
            suggested_fix: None,
        },
        ReviewFinding {
            file_path: PathBuf::from("d.rs"),
            line_number: Some(4),
            severity: ReviewSeverity::Medium,
            category: ReviewCategory::Performance,
            title: "Med 1".into(),
            description: "".into(),
            code_snippet: None,
            suggested_fix: None,
        },
        ReviewFinding {
            file_path: PathBuf::from("e.rs"),
            line_number: Some(5),
            severity: ReviewSeverity::Low,
            category: ReviewCategory::Maintainability,
            title: "Low 1".into(),
            description: "".into(),
            code_snippet: None,
            suggested_fix: None,
        },
    ];

    let report = ReviewReport {
        verdict: ReviewVerdict::RequestChanges,
        risk_level: RiskLevel::Critical,
        files_reviewed: 5,
        total_additions: 100,
        total_deletions: 20,
        findings,
        summary: "Multi-severity review".into(),
    };

    assert_eq!(report.critical_count(), 2);
    assert_eq!(report.high_count(), 1);
    assert_eq!(report.medium_count(), 1);

    let json = serde_json::to_string(&report).expect("serialize report");
    let deser: ReviewReport = serde_json::from_str(&json).expect("deserialize report");
    assert_eq!(deser.critical_count(), 2);
    assert_eq!(deser.verdict, ReviewVerdict::RequestChanges);
    assert_eq!(deser.risk_level, RiskLevel::Critical);
}
