//! Dedicated test for `crates/m31a-review/src/engine.rs`.
//!
//! Validates `ReviewEngine::analyze`, `analyze_git` with real git operations,
//! and `analyze_with_ai_lanes` with specialized review lanes.

use m31a_review::engine::ReviewEngine;
use m31a_review::lanes::ReviewerLaneConfig;
use m31a_review::model::{ReviewCategory, ReviewVerdict, RiskLevel};
use std::fs;
use std::process::Command;
use tempfile::tempdir;

#[test]
fn test_engine_verdict_and_risk_gradation() {
    // 1. Clean diff -> Approve, Low
    let clean = "--- a/a.rs\n+++ b/a.rs\n@@ -1,1 +1,2 @@\n+let x = 1;\n";
    let report = ReviewEngine::analyze(clean);
    assert_eq!(report.verdict, ReviewVerdict::Approve);
    assert_eq!(report.risk_level, RiskLevel::Low);
    assert_eq!(report.files_reviewed, 1);
    assert_eq!(report.total_additions, 1);
    assert_eq!(report.total_deletions, 0);

    // 2. Medium finding -> Comment, Medium
    let medium = "--- a/b.rs\n+++ b/b.rs\n@@ -1,1 +1,2 @@\n+let p = root.join(\"..\").join(sub);\n";
    let report = ReviewEngine::analyze(medium);
    assert_eq!(report.verdict, ReviewVerdict::Comment);
    assert_eq!(report.risk_level, RiskLevel::Medium);

    // 3. High finding -> RequestChanges, High
    let high = "--- a/c.rs\n+++ b/c.rs\n@@ -1,1 +1,2 @@\n+let _ = Command::new(\"sh\").arg(\"-c\").arg(format!(\"rm {}\", f));\n";
    let report = ReviewEngine::analyze(high);
    assert_eq!(report.verdict, ReviewVerdict::RequestChanges);
    assert_eq!(report.risk_level, RiskLevel::High);

    // 4. Critical finding -> RequestChanges, Critical
    let crit = "--- a/d.rs\n+++ b/d.rs\n@@ -1,1 +1,2 @@\n+let key = \"ghp_123456789012345678901234567890123456\";\n";
    let report = ReviewEngine::analyze(crit);
    assert_eq!(report.verdict, ReviewVerdict::RequestChanges);
    assert_eq!(report.risk_level, RiskLevel::Critical);
}

#[test]
fn test_engine_analyze_real_git() {
    let tmp = tempdir().expect("tempdir");
    let root = tmp.path();

    let git = |args: &[&str]| {
        let status = Command::new("git")
            .args(args)
            .current_dir(root)
            .status()
            .expect("git status");
        assert!(status.success());
    };

    git(&["init"]);
    git(&["config", "user.name", "ReviewTester"]);
    git(&["config", "user.email", "review@m31a.dev"]);

    fs::write(root.join("config.rs"), "pub const PORT: u16 = 8080;\n").unwrap();
    git(&["add", "config.rs"]);
    git(&["commit", "-m", "initial commit"]);

    // Introduce a secret in an uncommitted change
    fs::write(
        root.join("config.rs"),
        "pub const PORT: u16 = 8080;\npub const TOKEN: &str = \"ghp_123456789012345678901234567890123456\";\n",
    ).unwrap();

    let report_head = ReviewEngine::analyze_git(root, None).expect("analyze_git");
    assert_eq!(report_head.verdict, ReviewVerdict::RequestChanges);
    assert_eq!(report_head.risk_level, RiskLevel::Critical);
    assert_eq!(report_head.files_reviewed, 1);
    assert_eq!(report_head.total_additions, 1);

    let report_explicit = ReviewEngine::analyze_git(root, Some("HEAD")).expect("analyze_git explicit");
    assert_eq!(report_explicit.verdict, ReviewVerdict::RequestChanges);
}

#[tokio::test]
async fn test_engine_analyze_with_ai_lanes() {
    let tmp = tempdir().expect("tempdir");
    let file_path = tmp.path().join("dispatch.rs");
    fs::write(
        &file_path,
        "pub fn execute(cmd: &str) {\n    system(cmd);\n    let a = cmd.clone();\n    let b = cmd.clone();\n    let c = cmd.clone();\n    let d = cmd.clone();\n}\n",
    ).unwrap();

    let diff = r#"--- a/dispatch.rs
+++ b/dispatch.rs
@@ -1,1 +1,7 @@
 pub fn execute(cmd: &str) {
+    system(cmd);
+    let a = cmd.clone();
+    let b = cmd.clone();
+    let c = cmd.clone();
+    let d = cmd.clone();
 }
"#;

    let lanes = vec![
        ReviewerLaneConfig {
            id: "lane-sec".into(),
            model_name: "llama-70b".into(),
            depth: "deep".into(),
            focus: ReviewCategory::Security,
        },
        ReviewerLaneConfig {
            id: "lane-perf".into(),
            model_name: "deepseek-r1".into(),
            depth: "deep".into(),
            focus: ReviewCategory::Performance,
        },
    ];

    let consensus = ReviewEngine::analyze_with_ai_lanes(tmp.path(), diff, &lanes)
        .await
        .expect("analyze with ai lanes");

    // The lanes detect dynamic code execution (system) and excessive cloning (.clone() > 3 lines)
    assert!(consensus.total_claims_received >= 2);
    assert!(consensus.lanes_evaluated.contains(&"lane-sec".to_string()));
    assert!(consensus.lanes_evaluated.contains(&"lane-perf".to_string()));
}
