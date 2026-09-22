//! Diff self-review and anti-fake verification engine.
//!
//! Enforces:
//! - AGENTS.md Rule 5: Never fake success (rejects todo!(), unimplemented!(), empty stubs, mock returns).
//! - AGENTS.md Rule 6: Completion requires evidence.
//! - Section 21: Final diff corresponds strictly to assigned task (no accidental edits).
//! - Section 23: Test assertions must not be weakened or deleted to mask failures.

use crate::kernel::change::{ChangeSurface, DiffReviewReport, DiffReviewViolation};

/// Independent inspector analyzing change diffs for quality, authenticity, and scope.
pub struct DiffReviewer;

impl DiffReviewer {
    /// Review the unified diff and modified files against task change surface and quality rules.
    pub fn review(
        diff_text: &str,
        change_surface: &ChangeSurface,
        files_modified: &[String],
    ) -> DiffReviewReport {
        let mut violations = Vec::new();
        let mut lines_added = 0;
        let mut lines_removed = 0;

        // 1. Verify that all modified files reside within authorized ChangeSurface
        for file in files_modified {
            if !change_surface.contains_path(file) {
                violations.push(DiffReviewViolation::AccidentalEdit {
                    path: file.clone(),
                    reason: format!(
                        "File '{}' was modified but not declared in the task ChangeSurface",
                        file
                    ),
                });
            }
        }

        // 2. Parse diff hunks and inspect line additions / modifications
        let mut current_file = String::new();
        let mut current_line = 0usize;

        for line in diff_text.lines() {
            if line.starts_with("+++ b/") {
                current_file = line.trim_start_matches("+++ b/").to_string();
                current_line = 1;
                continue;
            } else if line.starts_with("--- ") {
                continue;
            } else if line.starts_with("@@ ") {
                // Parse @@ -start,len +start,len @@
                if let Some(plus_idx) = line.find('+') {
                    let after_plus = &line[plus_idx + 1..];
                    let comma_or_space = after_plus.find([',', ' ']).unwrap_or(after_plus.len());
                    if let Ok(num) = after_plus[..comma_or_space].parse::<usize>() {
                        current_line = num;
                    }
                }
                continue;
            }

            if line.starts_with('+') && !line.starts_with("+++") {
                lines_added += 1;
                let added_content = &line[1..];
                let trimmed = added_content.trim();

                // Fake implementation checks
                if trimmed.contains("todo!(") || trimmed.contains("todo!") {
                    violations.push(DiffReviewViolation::FakeImplementation {
                        path: current_file.clone(),
                        line: current_line,
                        snippet: trimmed.to_string(),
                        pattern: "todo!()".to_string(),
                    });
                } else if trimmed.contains("unimplemented!(") || trimmed.contains("unimplemented!")
                {
                    violations.push(DiffReviewViolation::FakeImplementation {
                        path: current_file.clone(),
                        line: current_line,
                        snippet: trimmed.to_string(),
                        pattern: "unimplemented!()".to_string(),
                    });
                } else if trimmed.contains("panic!(\"todo")
                    || trimmed.contains("panic!(\"implement")
                {
                    violations.push(DiffReviewViolation::FakeImplementation {
                        path: current_file.clone(),
                        line: current_line,
                        snippet: trimmed.to_string(),
                        pattern: "panic!(\"todo\")".to_string(),
                    });
                }

                // Leftover debug code checks
                if (trimmed.starts_with("dbg!(")
                    || trimmed.contains(" dbg!(")
                    || trimmed.contains("println!(\"DEBUG")
                    || trimmed.contains("eprintln!(\"DEBUG"))
                    && !current_file.contains("test")
                {
                    violations.push(DiffReviewViolation::LeftoverDebug {
                        path: current_file.clone(),
                        line: current_line,
                        snippet: trimmed.to_string(),
                    });
                }

                // Unresolved TODO / FIXME checks in production code
                if (trimmed.starts_with("// TODO") || trimmed.starts_with("// FIXME"))
                    && !current_file.contains("test")
                {
                    violations.push(DiffReviewViolation::UnresolvedTodo {
                        path: current_file.clone(),
                        line: current_line,
                        snippet: trimmed.to_string(),
                    });
                }

                // Weaker test assertion checks
                if current_file.contains("test")
                    && (trimmed.starts_with("// assert")
                        || trimmed.starts_with("//assert")
                        || trimmed.contains("// assert_eq!")
                        || trimmed == "assert!(true);"
                        || trimmed == "assert!(true)")
                {
                    violations.push(DiffReviewViolation::WeakerTestAssertion {
                        path: current_file.clone(),
                        line: current_line,
                        snippet: trimmed.to_string(),
                    });
                }

                current_line += 1;
            } else if line.starts_with('-') && !line.starts_with("---") {
                lines_removed += 1;
            } else {
                current_line += 1;
            }
        }

        DiffReviewReport::with_violations(
            violations,
            files_modified.len(),
            lines_added,
            lines_removed,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_diff_review_clean_passes() {
        let diff = r#"
--- a/src/service.rs
+++ b/src/service.rs
@@ -10,3 +10,4 @@
 pub fn compute() -> u32 {
+    let factor = 2;
     40 + factor
 }
"#;
        let surface = ChangeSurface::new(vec!["src/service.rs".to_string()]);
        let report = DiffReviewer::review(diff, &surface, &["src/service.rs".to_string()]);

        assert!(report.passed);
        assert!(report.violations.is_empty());
        assert_eq!(report.lines_added, 1);
    }

    #[test]
    fn test_diff_review_detects_todo_macro() {
        let diff = r#"
--- a/src/service.rs
+++ b/src/service.rs
@@ -10,3 +10,4 @@
 pub fn compute() -> u32 {
+    todo!()
 }
"#;
        let surface = ChangeSurface::new(vec!["src/service.rs".to_string()]);
        let report = DiffReviewer::review(diff, &surface, &["src/service.rs".to_string()]);

        assert!(!report.passed);
        assert_eq!(report.violations.len(), 1);
        assert!(matches!(
            report.violations[0],
            DiffReviewViolation::FakeImplementation { .. }
        ));
    }

    #[test]
    fn test_diff_review_detects_commented_out_assertion() {
        let diff = r#"
--- a/tests/service_test.rs
+++ b/tests/service_test.rs
@@ -20,3 +20,4 @@
 #[test]
 fn test_compute() {
+    // assert_eq!(res, 42);
 }
"#;
        let surface = ChangeSurface::new(vec!["tests/service_test.rs".to_string()]);
        let report = DiffReviewer::review(diff, &surface, &["tests/service_test.rs".to_string()]);

        assert!(!report.passed);
        assert!(matches!(
            report.violations[0],
            DiffReviewViolation::WeakerTestAssertion { .. }
        ));
    }

    #[test]
    fn test_diff_review_detects_scope_creep() {
        let diff = "";
        let surface = ChangeSurface::new(vec!["src/auth.rs".to_string()]);
        let report = DiffReviewer::review(
            diff,
            &surface,
            &["src/auth.rs".to_string(), "src/unrelated.rs".to_string()],
        );

        assert!(!report.passed);
        assert!(matches!(
            report.violations[0],
            DiffReviewViolation::AccidentalEdit { .. }
        ));
    }
}
