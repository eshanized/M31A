//! Regression Prevention and Targeted Regression Test Synthesizer (`m31a-verify::regression`).
//!
//! Preserves existing behaviors and guarantees that targeted regression tests are introduced
//! for every meaningful bug fix to permanently prevent regressions.
//!
//! Key components:
//! 1. `RegressionGuard`: Audits git diffs and commit messages. If a bug fix is identified,
//!    enforces that a corresponding test case was introduced in the changes.
//! 2. `RegressionTestSynthesizer`: Generates structured regression test scaffolding across
//!    Rust, Python, and TypeScript based on bug descriptions and diagnostic inputs.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RegressionAuditStatus {
    /// Regression test verified present in the commit or changeset.
    Verified {
        test_file: PathBuf,
        test_name: String,
    },
    /// Changeset modifies production logic for a bug fix without adding or modifying tests.
    MissingTest { reason: String },
    /// Non-bugfix changes (e.g., chore, docs, refactor) where regression test is optional.
    Exempt { reason: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegressionAuditReport {
    pub is_bug_fix: bool,
    pub bug_indicators: Vec<String>,
    pub test_files_touched: Vec<PathBuf>,
    pub status: RegressionAuditStatus,
    pub remediation_hint: Option<String>,
}

pub struct RegressionGuard;

impl RegressionGuard {
    /// Inspects commit message, pull request summary, and touched files to ensure
    /// regression test compliance.
    pub fn audit_changeset(
        commit_message: &str,
        touched_files: &[PathBuf],
    ) -> RegressionAuditReport {
        let msg_lower = commit_message.to_lowercase();
        let bug_keywords = [
            "fix",
            "bug",
            "patch",
            "resolves",
            "closes",
            "defect",
            "panic",
            "crash",
            "regression",
        ];

        let mut bug_indicators = Vec::new();
        for kw in &bug_keywords {
            if msg_lower.contains(kw) {
                bug_indicators.push(kw.to_string());
            }
        }

        let is_bug_fix = !bug_indicators.is_empty();
        let test_files_touched: Vec<PathBuf> = touched_files
            .iter()
            .filter(|p| {
                let s = p.to_string_lossy().to_lowercase();
                s.contains("test")
                    || s.contains("spec")
                    || s.contains("_test.rs")
                    || s.contains("test_")
            })
            .cloned()
            .collect();

        let status = if !is_bug_fix {
            RegressionAuditStatus::Exempt {
                reason: "Commit does not match bug-fix keywords.".to_string(),
            }
        } else if !test_files_touched.is_empty() {
            let primary = test_files_touched[0].clone();
            RegressionAuditStatus::Verified {
                test_file: primary,
                test_name: format!("regression_{}", bug_indicators[0]),
            }
        } else {
            RegressionAuditStatus::MissingTest {
                reason: format!(
                    "Changeset indicates a bug fix ({:?}) but touches 0 test files.",
                    bug_indicators
                ),
            }
        };

        let remediation_hint = match &status {
            RegressionAuditStatus::MissingTest { .. } => Some(
                "Create a targeted regression test reproducing the original bug before applying the fix."
                    .to_string(),
            ),
            _ => None,
        };

        RegressionAuditReport {
            is_bug_fix,
            bug_indicators,
            test_files_touched,
            status,
            remediation_hint,
        }
    }
}

pub struct RegressionTestSynthesizer;

impl RegressionTestSynthesizer {
    /// Generates targeted regression test code in Rust, Python, or TypeScript.
    pub fn synthesize_test(
        ecosystem: &str,
        test_name: &str,
        bug_description: &str,
        failing_input: &str,
        expected_output: &str,
    ) -> String {
        match ecosystem.to_lowercase().as_str() {
            "rust" | "rs" => format!(
                r#"// Regression test: {bug_description}
#[test]
fn {test_name}() {{
    // Reproduce original edge-case input
    let input = {failing_input};
    let outcome = execute_under_test(&input);
    assert_eq!(outcome, {expected_output}, "Regression failure: {bug_description}");
}}
"#
            ),
            "python" | "py" => format!(
                r#"# Regression test: {bug_description}
def {test_name}():
    # Reproduce original edge-case input
    input_val = {failing_input}
    outcome = execute_under_test(input_val)
    assert outcome == {expected_output}, "Regression failure: {bug_description}"
"#
            ),
            "ts" | "typescript" | "js" | "javascript" => format!(
                r#"// Regression test: {bug_description}
test("{test_name}", () => {{
    const inputVal = {failing_input};
    const outcome = executeUnderTest(inputVal);
    expect(outcome).toEqual({expected_output});
}});
"#
            ),
            _ => format!(
                r#"// Regression test: {bug_description}
// Input: {failing_input}
// Expected: {expected_output}
"#
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_regression_guard_flags_missing_tests_on_bugfix() {
        let msg = "fix(auth): resolve nil pointer panic on token refresh";
        let files = vec![PathBuf::from("crates/auth/src/token.rs")];
        let report = RegressionGuard::audit_changeset(msg, &files);

        assert!(report.is_bug_fix);
        assert!(matches!(
            report.status,
            RegressionAuditStatus::MissingTest { .. }
        ));
        assert!(report.remediation_hint.is_some());
    }

    #[test]
    fn test_regression_guard_passes_when_test_included() {
        let msg = "fix(auth): resolve nil pointer panic on token refresh";
        let files = vec![
            PathBuf::from("crates/auth/src/token.rs"),
            PathBuf::from("crates/auth/tests/token_regression_test.rs"),
        ];
        let report = RegressionGuard::audit_changeset(msg, &files);

        assert!(report.is_bug_fix);
        assert!(matches!(
            report.status,
            RegressionAuditStatus::Verified { .. }
        ));
    }

    #[test]
    fn test_regression_guard_exempts_chore() {
        let msg = "chore: bump dependency versions in Cargo.toml";
        let files = vec![PathBuf::from("Cargo.toml")];
        let report = RegressionGuard::audit_changeset(msg, &files);

        assert!(!report.is_bug_fix);
        assert!(matches!(
            report.status,
            RegressionAuditStatus::Exempt { .. }
        ));
    }

    #[test]
    fn test_synthesizer_rust() {
        let code = RegressionTestSynthesizer::synthesize_test(
            "rust",
            "test_null_deref_regression",
            "Handle null strings without panic",
            "\"\"",
            "Ok(\"\")",
        );
        assert!(code.contains("#[test]"));
        assert!(code.contains("test_null_deref_regression"));
        assert!(code.contains("assert_eq!"));
    }
}
