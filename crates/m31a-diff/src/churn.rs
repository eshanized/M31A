//! Minimal, Justified Changes & Diff Churn Penalty Engine (`m31a-diff::churn`).
//!
//! Enforces the principle of smallest complete solutions:
//! - Analyzes unified diffs to separate semantic functional changes from gratuitous churn.
//! - Detects whitespace-only changes, comment rewrites, and re-indentations.
//! - Computes a Churn Penalty Score (0-100) to deter unnecessary code rewrites.
//! - Recommends concrete diff minimization steps.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChurnVerdict {
    Minimal,
    Acceptable,
    ExcessiveChurn,
    GratuitousRewrite,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffChurnStats {
    pub total_lines_added: usize,
    pub total_lines_deleted: usize,
    pub whitespace_only_changes: usize,
    pub comment_only_changes: usize,
    pub semantic_lines_changed: usize,
    pub cosmetic_churn_pct: f64,
    pub churn_penalty_score: f64,
    pub verdict: ChurnVerdict,
    pub recommendations: Vec<String>,
}

pub struct DiffMinimizer;

impl DiffMinimizer {
    /// Analyzes a unified diff string and scores its churn efficiency.
    pub fn evaluate_diff(diff_text: &str) -> DiffChurnStats {
        let mut added = 0;
        let mut deleted = 0;
        let mut whitespace_changes = 0;
        let mut comment_changes = 0;

        let mut current_del: Vec<String> = Vec::new();
        let mut current_add: Vec<String> = Vec::new();

        let flush_block = |del: &mut Vec<String>,
                           add: &mut Vec<String>,
                           ws_count: &mut usize,
                           com_count: &mut usize| {
            // Check if pairs of del and add only differ by whitespace
            let min_len = del.len().min(add.len());
            for i in 0..min_len {
                let d_trim = del[i].trim();
                let a_trim = add[i].trim();
                if d_trim == a_trim && del[i] != add[i] {
                    *ws_count += 2;
                } else if (d_trim.starts_with("//") || d_trim.starts_with('#'))
                    && (a_trim.starts_with("//") || a_trim.starts_with('#'))
                {
                    *com_count += 2;
                }
            }
            del.clear();
            add.clear();
        };

        for line in diff_text.lines() {
            if line.starts_with("--- ") || line.starts_with("+++ ") || line.starts_with("@@ ") {
                flush_block(
                    &mut current_del,
                    &mut current_add,
                    &mut whitespace_changes,
                    &mut comment_changes,
                );
                continue;
            }

            if let Some(del_line) = line.strip_prefix('-') {
                deleted += 1;
                current_del.push(del_line.to_string());
            } else if let Some(add_line) = line.strip_prefix('+') {
                added += 1;
                current_add.push(add_line.to_string());
            } else {
                flush_block(
                    &mut current_del,
                    &mut current_add,
                    &mut whitespace_changes,
                    &mut comment_changes,
                );
            }
        }
        flush_block(
            &mut current_del,
            &mut current_add,
            &mut whitespace_changes,
            &mut comment_changes,
        );

        let total_changes: usize = added + deleted;
        let cosmetic_changes = whitespace_changes + comment_changes;
        let semantic_changes = total_changes.saturating_sub(cosmetic_changes);

        let cosmetic_pct = if total_changes > 0 {
            (cosmetic_changes as f64 / total_changes as f64) * 100.0
        } else {
            0.0
        };

        // Score penalty: higher if diff is large and contains high cosmetic churn
        let mut penalty: f64 = 0.0;
        if total_changes > 300 {
            penalty += 30.0;
        } else if total_changes > 150 {
            penalty += 15.0;
        }
        penalty += cosmetic_pct * 0.7;
        let churn_penalty_score = penalty.min(100.0);

        let mut recommendations = Vec::new();
        if whitespace_changes > 0 {
            recommendations.push(format!(
                "Revert {} whitespace/re-indentation line changes to preserve clean blame history.",
                whitespace_changes
            ));
        }
        if cosmetic_pct > 35.0 {
            recommendations.push("Diff contains over 35% non-semantic churn. Prefer surgical search-and-replace edits.".to_string());
        }

        let verdict = if churn_penalty_score > 60.0 {
            ChurnVerdict::GratuitousRewrite
        } else if churn_penalty_score > 30.0 {
            ChurnVerdict::ExcessiveChurn
        } else if churn_penalty_score > 10.0 {
            ChurnVerdict::Acceptable
        } else {
            ChurnVerdict::Minimal
        };

        DiffChurnStats {
            total_lines_added: added,
            total_lines_deleted: deleted,
            whitespace_only_changes: whitespace_changes,
            comment_only_changes: comment_changes,
            semantic_lines_changed: semantic_changes,
            cosmetic_churn_pct: cosmetic_pct,
            churn_penalty_score,
            verdict,
            recommendations,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_minimal_semantic_diff() {
        let diff = r#"
--- a/src/calc.rs
+++ b/src/calc.rs
@@ -10,3 +10,3 @@
-    let total = a * 2;
+    let total = a * 3;
"#;
        let stats = DiffMinimizer::evaluate_diff(diff);
        assert_eq!(stats.total_lines_added, 1);
        assert_eq!(stats.total_lines_deleted, 1);
        assert_eq!(stats.whitespace_only_changes, 0);
        assert_eq!(stats.verdict, ChurnVerdict::Minimal);
    }

    #[test]
    fn test_detect_whitespace_churn() {
        let diff = r#"
--- a/src/calc.rs
+++ b/src/calc.rs
@@ -10,3 +10,3 @@
-    let total = a * 2;
+        let total = a * 2;
"#;
        let stats = DiffMinimizer::evaluate_diff(diff);
        assert_eq!(stats.whitespace_only_changes, 2);
        assert!(stats.cosmetic_churn_pct > 90.0);
        assert!(!stats.recommendations.is_empty());
    }
}
