//! Dedicated test for `crates/m31a-diff/src/churn.rs`.
//!
//! Validates diff churn scoring, whitespace/comment detection, penalty thresholds,
//! and recommendation generation.

use m31a_diff::churn::{ChurnVerdict, DiffMinimizer};

#[test]
fn test_churn_empty_diff() {
    let stats = DiffMinimizer::evaluate_diff("");
    assert_eq!(stats.total_lines_added, 0);
    assert_eq!(stats.total_lines_deleted, 0);
    assert_eq!(stats.whitespace_only_changes, 0);
    assert_eq!(stats.comment_only_changes, 0);
    assert_eq!(stats.semantic_lines_changed, 0);
    assert_eq!(stats.cosmetic_churn_pct, 0.0);
    assert_eq!(stats.churn_penalty_score, 0.0);
    assert_eq!(stats.verdict, ChurnVerdict::Minimal);
    assert!(stats.recommendations.is_empty());
}

#[test]
fn test_churn_pure_whitespace_indentation() {
    let diff = r#"--- a/src/main.rs
+++ b/src/main.rs
@@ -10,3 +10,3 @@
-    let a = 10;
+        let a = 10;
-    let b = 20;
+        let b = 20;
"#;
    let stats = DiffMinimizer::evaluate_diff(diff);
    assert_eq!(stats.total_lines_added, 2);
    assert_eq!(stats.total_lines_deleted, 2);
    assert_eq!(stats.whitespace_only_changes, 4);
    assert_eq!(stats.comment_only_changes, 0);
    assert_eq!(stats.semantic_lines_changed, 0);
    assert_eq!(stats.cosmetic_churn_pct, 100.0);
    assert!(stats.churn_penalty_score >= 70.0);
    assert_eq!(stats.verdict, ChurnVerdict::GratuitousRewrite);

    assert!(stats.recommendations.iter().any(|r| r.contains("Revert 4 whitespace")));
    assert!(stats.recommendations.iter().any(|r| r.contains("over 35% non-semantic churn")));
}

#[test]
fn test_churn_pure_comment_rewriting() {
    let diff = r#"--- a/src/calc.rs
+++ b/src/calc.rs
@@ -1,2 +1,2 @@
-// old explanation
+// new shiny explanation
-# python comment
+# modified python comment
"#;
    let stats = DiffMinimizer::evaluate_diff(diff);
    assert_eq!(stats.total_lines_added, 2);
    assert_eq!(stats.total_lines_deleted, 2);
    assert_eq!(stats.comment_only_changes, 4);
    assert_eq!(stats.whitespace_only_changes, 0);
    assert_eq!(stats.semantic_lines_changed, 0);
    assert_eq!(stats.cosmetic_churn_pct, 100.0);
    assert_eq!(stats.verdict, ChurnVerdict::GratuitousRewrite);
}

#[test]
fn test_churn_mixed_semantic_and_cosmetic() {
    let diff = r#"--- a/src/calc.rs
+++ b/src/calc.rs
@@ -1,6 +1,6 @@
-    let mut sum = 0;
+        let mut sum = 0;
-    for i in 0..10 {
-        sum += i;
-    }
+    for i in 0..20 {
+        sum += i * 2;
+    }
"#;
    let stats = DiffMinimizer::evaluate_diff(diff);
    assert_eq!(stats.total_lines_deleted, 4);
    assert_eq!(stats.total_lines_added, 4);
    assert_eq!(stats.whitespace_only_changes, 2);
    assert_eq!(stats.semantic_lines_changed, 6);
}

#[test]
fn test_churn_penalty_thresholds() {
    // Generate a massive diff > 300 lines with zero cosmetic changes
    let mut large_diff = String::from("--- a/huge.rs\n+++ b/huge.rs\n@@ -1,400 +1,400 @@\n");
    for i in 0..160 {
        large_diff.push_str(&format!("-func_old_{}();\n+func_new_{}();\n", i, i));
    }
    // Total changes = 320 lines (> 300 triggers penalty += 30)
    let stats = DiffMinimizer::evaluate_diff(&large_diff);
    assert_eq!(stats.total_lines_added, 160);
    assert_eq!(stats.total_lines_deleted, 160);
    assert_eq!(stats.whitespace_only_changes, 0);
    assert_eq!(stats.comment_only_changes, 0);
    assert_eq!(stats.cosmetic_churn_pct, 0.0);
    assert_eq!(stats.churn_penalty_score, 30.0);
    assert_eq!(stats.verdict, ChurnVerdict::Acceptable);

    // Medium diff > 150 lines
    let mut med_diff = String::from("--- a/med.rs\n+++ b/med.rs\n@@ -1,200 +1,200 @@\n");
    for i in 0..80 {
        med_diff.push_str(&format!("-call_a_{}();\n+call_b_{}();\n", i, i));
    }
    // Total changes = 160 lines (> 150 triggers penalty += 15)
    let med_stats = DiffMinimizer::evaluate_diff(&med_diff);
    assert_eq!(med_stats.churn_penalty_score, 15.0);
    assert_eq!(med_stats.verdict, ChurnVerdict::Acceptable);
}
