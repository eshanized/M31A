//! Dedicated test for `crates/m31a-review/src/lanes.rs`.
//!
//! Validates `PrimaryConsolidator` citation requirements, multi-lane consensus,
//! primary verification against real disk files, and markdown report generation.

use m31a_review::lanes::{
    ClaimStatus, MultiLaneReviewDispatcher, PrimaryConsolidator, ReviewerClaim, ReviewerLaneConfig,
};
use m31a_review::model::{ReviewCategory, ReviewSeverity};
use std::fs;
use std::path::PathBuf;
use tempfile::tempdir;

#[test]
fn test_consolidator_citation_enforcement() {
    let tmp = tempdir().expect("tempdir");

    let claims = vec![
        // Missing file
        ReviewerClaim {
            lane_id: "lane-1".into(),
            file_path: None,
            line_number: Some(10),
            severity: ReviewSeverity::High,
            category: ReviewCategory::Security,
            claim_text: "Vague vulnerability".into(),
            suggested_fix: None,
        },
        // Empty file
        ReviewerClaim {
            lane_id: "lane-1".into(),
            file_path: Some(PathBuf::from("")),
            line_number: Some(10),
            severity: ReviewSeverity::High,
            category: ReviewCategory::Security,
            claim_text: "Empty path claim".into(),
            suggested_fix: None,
        },
        // Missing line number
        ReviewerClaim {
            lane_id: "lane-2".into(),
            file_path: Some(PathBuf::from("src/main.rs")),
            line_number: None,
            severity: ReviewSeverity::Medium,
            category: ReviewCategory::Performance,
            claim_text: "Unanchored performance claim".into(),
            suggested_fix: None,
        },
    ];

    let report = PrimaryConsolidator::consolidate(tmp.path(), &claims);
    assert_eq!(report.corroborated_findings.len(), 0);
    assert_eq!(report.discarded_claims.len(), 3);
    for d in &report.discarded_claims {
        assert_eq!(d.status, ClaimStatus::DiscardedNoCitation);
    }
}

#[test]
fn test_consolidator_refutes_nonexistent_file_and_overflow_lines() {
    let tmp = tempdir().expect("tempdir");
    let test_file = tmp.path().join("existing.rs");
    fs::write(&test_file, "line 1\nline 2\nline 3\n").expect("write existing.rs");

    let claims = vec![
        // Missing file on disk
        ReviewerClaim {
            lane_id: "lane-a".into(),
            file_path: Some(PathBuf::from("ghost.rs")),
            line_number: Some(1),
            severity: ReviewSeverity::High,
            category: ReviewCategory::Correctness,
            claim_text: "Bug in ghost file".into(),
            suggested_fix: None,
        },
        // Line number 50 out of bounds (file only has 3 lines)
        ReviewerClaim {
            lane_id: "lane-b".into(),
            file_path: Some(PathBuf::from("existing.rs")),
            line_number: Some(50),
            severity: ReviewSeverity::Critical,
            category: ReviewCategory::Security,
            claim_text: "Overflow line claim".into(),
            suggested_fix: None,
        },
    ];

    let report = PrimaryConsolidator::consolidate(tmp.path(), &claims);
    assert_eq!(report.corroborated_findings.len(), 0);
    assert_eq!(report.refuted_claims.len(), 2);
    for r in &report.refuted_claims {
        assert_eq!(r.status, ClaimStatus::Refuted);
    }
}

#[test]
fn test_consolidator_corroborates_multi_lane_agreement() {
    let tmp = tempdir().expect("tempdir");
    let file = tmp.path().join("api.rs");
    fs::write(&file, "pub fn handler() {\n    let pass = \"admin\";\n}\n").expect("write api.rs");

    let claims = vec![
        ReviewerClaim {
            lane_id: "lane-gpt".into(),
            file_path: Some(PathBuf::from("api.rs")),
            line_number: Some(2),
            severity: ReviewSeverity::Critical,
            category: ReviewCategory::Security,
            claim_text: "Hardcoded password found".into(),
            suggested_fix: Some("Use env".into()),
        },
        ReviewerClaim {
            lane_id: "lane-claude".into(),
            file_path: Some(PathBuf::from("api.rs")),
            line_number: Some(2),
            severity: ReviewSeverity::High,
            category: ReviewCategory::Security,
            claim_text: "Plaintext credential literal".into(),
            suggested_fix: Some("Store securely".into()),
        },
    ];

    let report = PrimaryConsolidator::consolidate(tmp.path(), &claims);
    assert_eq!(report.corroborated_findings.len(), 1);
    let finding = &report.corroborated_findings[0];
    assert_eq!(finding.status, ClaimStatus::Corroborated);
    assert_eq!(finding.severity, ReviewSeverity::Critical); // min severity = highest priority
    assert_eq!(finding.attesting_lanes.len(), 2);
    assert!(finding.verification_notes.contains("Corroborated across 2 lanes"));
}

#[tokio::test]
async fn test_dispatcher_markdown_output() {
    let tmp = tempdir().expect("tempdir");
    let file = tmp.path().join("calc.rs");
    fs::write(&file, "pub fn divide(a: i32, b: i32) -> i32 {\n    a / b\n}\n").expect("write calc.rs");

    let claims = vec![
        ReviewerClaim {
            lane_id: "lane-single".into(),
            file_path: Some(PathBuf::from("calc.rs")),
            line_number: Some(2),
            severity: ReviewSeverity::Medium,
            category: ReviewCategory::Correctness,
            claim_text: "Possible divide by zero panic without check".into(),
            suggested_fix: Some("Check b != 0".into()),
        },
        ReviewerClaim {
            lane_id: "lane-discarded".into(),
            file_path: None,
            line_number: None,
            severity: ReviewSeverity::Low,
            category: ReviewCategory::Maintainability,
            claim_text: "No file cited".into(),
            suggested_fix: None,
        },
    ];

    let lanes = vec![ReviewerLaneConfig {
        id: "lane-single".into(),
        model_name: "test-model".into(),
        depth: "shallow".into(),
        focus: ReviewCategory::Correctness,
    }];

    let report = MultiLaneReviewDispatcher::dispatch(tmp.path(), &lanes, &claims).await;
    assert_eq!(report.total_claims_received, 2);
    assert_eq!(report.discarded_claims.len(), 1);

    let md = PrimaryConsolidator::render_markdown(&report);
    assert!(md.contains("# Multi-AI Code Review Report"));
    assert!(md.contains("Discarded Claims (Missing File/Line Citation)"));
}
