//! Integration tests for code review and static analysis.
//!
//! Tests pattern detection, security vulnerability checks, and multi-model consensus merging.

use m31a_review::{
    ReviewCategory, ReviewEngine, ReviewFormatter, ReviewSeverity, ReviewVerdict, RiskLevel,
};
use std::fs;
use std::process::Command;
use tempfile::tempdir;

#[test]
fn test_review_engine_detects_sql_injection() {
    let diff = r#"diff --git a/src/db.rs b/src/db.rs
--- a/src/db.rs
+++ b/src/db.rs
@@ -10,2 +10,4 @@
 fn query_user(id: &str) {
+    let query = format!("SELECT * FROM users WHERE id = {}", id);
+    conn.execute(&query);
 }
"#;

    let report = ReviewEngine::analyze(diff);
    assert_eq!(report.verdict, ReviewVerdict::RequestChanges);
    assert!(
        report
            .findings
            .iter()
            .any(|f| f.category == ReviewCategory::Security)
    );
    assert!(
        report
            .findings
            .iter()
            .any(|f| f.title.to_lowercase().contains("sql"))
    );
}

#[test]
fn test_review_engine_detects_hardcoded_secrets() {
    let diff = r#"diff --git a/src/config.rs b/src/config.rs
--- a/src/config.rs
+++ b/src/config.rs
@@ -1,3 +1,4 @@
+const API_TOKEN: &str = "ghp_123456789012345678901234567890123456";
"#;

    let report = ReviewEngine::analyze(diff);
    assert_eq!(report.verdict, ReviewVerdict::RequestChanges);
    assert_eq!(report.risk_level, RiskLevel::Critical);
    assert!(
        report
            .findings
            .iter()
            .any(|f| f.severity == ReviewSeverity::Critical)
    );
}

#[test]
fn test_review_engine_clean_diff_approval() {
    let diff = r#"diff --git a/src/math.rs b/src/math.rs
--- a/src/math.rs
+++ b/src/math.rs
@@ -1,3 +1,4 @@
 pub fn add(a: i32, b: i32) -> i32 {
+    // Safe pure arithmetic
     a + b
 }
"#;

    let report = ReviewEngine::analyze(diff);
    assert_eq!(report.verdict, ReviewVerdict::Approve);
    assert_eq!(report.risk_level, RiskLevel::Low);
    assert!(report.findings.is_empty());
}

#[test]
fn test_review_formatters_terminal_and_markdown() {
    let diff = r#"diff --git a/src/lib.rs b/src/lib.rs
--- a/src/lib.rs
+++ b/src/lib.rs
@@ -1,1 +1,2 @@
+let key = "ghp_123456789012345678901234567890123456";
"#;

    let report = ReviewEngine::analyze(diff);

    // Terminal formatter
    let term = ReviewFormatter::render_terminal(&report);
    assert!(term.contains("M31A Automated Code Review Report"));
    assert!(term.contains("Verdict:"));

    // Markdown formatter
    let md = ReviewFormatter::render_markdown(&report);
    assert!(md.contains("M31A Automated Code Review Report"));
    assert!(md.contains("RequestChanges"));

    // JSON formatter
    let json = ReviewFormatter::render_json(&report).unwrap();
    assert!(json.contains("\"verdict\": \"RequestChanges\""));
}

#[test]
fn test_review_engine_real_git_analysis() {
    let tmp = tempdir().unwrap();
    let root = tmp.path();

    // Initialize real git repo
    let run_git = |args: &[&str]| {
        let status = Command::new("git")
            .args(args)
            .current_dir(root)
            .status()
            .expect("git execution");
        assert!(status.success());
    };

    run_git(&["init"]);
    run_git(&["config", "user.name", "Tester"]);
    run_git(&["config", "user.email", "tester@example.com"]);

    // Initial commit
    fs::write(root.join("hello.txt"), "first line\n").unwrap();
    run_git(&["add", "hello.txt"]);
    run_git(&["commit", "-m", "initial commit"]);

    // Make an unstaged modification
    fs::write(root.join("hello.txt"), "first line\nsecond line added\n").unwrap();

    let report = ReviewEngine::analyze_git(root, None).expect("analyze_git");
    assert_eq!(report.files_reviewed, 1);
    assert_eq!(report.total_additions, 1);
    assert_eq!(report.verdict, ReviewVerdict::Approve);

    // Test analyze_git with base revision
    let base_report = ReviewEngine::analyze_git(root, Some("HEAD")).expect("analyze_git base");
    assert_eq!(base_report.verdict, ReviewVerdict::Approve);
}

#[test]
fn test_review_engine_detects_shell_injection_and_path_traversal() {
    let diff = r#"diff --git a/src/handler.rs b/src/handler.rs
--- a/src/handler.rs
+++ b/src/handler.rs
@@ -1,5 +1,10 @@
 fn run_user_cmd(user_arg: &str) {
+    let _ = Command::new("sh").arg("-c").arg(format!("echo {}", user_arg));
+    let untrusted_path = root.join("..").join(user_arg);
+    let _val = parse_token().unwrap();
 }
"#;

    let report = ReviewEngine::analyze(diff);
    assert_eq!(report.verdict, ReviewVerdict::RequestChanges);
    assert!(
        report
            .findings
            .iter()
            .any(|f| f.title.to_lowercase().contains("shell command injection"))
    );
    assert!(
        report
            .findings
            .iter()
            .any(|f| f.title.to_lowercase().contains("path traversal"))
    );
    assert!(
        report
            .findings
            .iter()
            .any(|f| f.title.to_lowercase().contains("unwrap"))
    );
}
