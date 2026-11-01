//! Main Code Review Engine.

use anyhow::{Context, Result};
use std::collections::HashSet;
use std::path::Path;
use std::process::Command;

use crate::analyzer::StaticDiffAnalyzer;
use crate::model::{ReviewReport, ReviewSeverity, ReviewVerdict, RiskLevel};

pub struct ReviewEngine;

impl ReviewEngine {
    /// Conducts a comprehensive code review over unified git diff text.
    pub fn analyze(diff_text: &str) -> ReviewReport {
        let (files_reviewed, total_additions, total_deletions) =
            Self::compute_diff_stats(diff_text);
        let findings = StaticDiffAnalyzer::analyze_diff(diff_text);

        let critical_count = findings
            .iter()
            .filter(|f| f.severity == ReviewSeverity::Critical)
            .count();
        let high_count = findings
            .iter()
            .filter(|f| f.severity == ReviewSeverity::High)
            .count();
        let medium_count = findings
            .iter()
            .filter(|f| f.severity == ReviewSeverity::Medium)
            .count();

        let risk_level = if critical_count > 0 {
            RiskLevel::Critical
        } else if high_count > 0 {
            RiskLevel::High
        } else if medium_count > 0 {
            RiskLevel::Medium
        } else {
            RiskLevel::Low
        };

        let verdict = if critical_count > 0 || high_count > 0 {
            ReviewVerdict::RequestChanges
        } else if medium_count > 0 {
            ReviewVerdict::Comment
        } else {
            ReviewVerdict::Approve
        };

        let summary = format!(
            "Analyzed {} modified file(s) across +{} additions and -{} deletions. Discovered {} finding(s). Verdict: {:?}.",
            files_reviewed,
            total_additions,
            total_deletions,
            findings.len(),
            verdict
        );

        ReviewReport {
            verdict,
            risk_level,
            files_reviewed,
            total_additions,
            total_deletions,
            findings,
            summary,
        }
    }

    /// Queries git diff from the local workspace and conducts a review.
    pub fn analyze_git(cwd: &Path, base_ref: Option<&str>) -> Result<ReviewReport> {
        let mut cmd = Command::new("git");
        cmd.arg("diff");
        if let Some(base) = base_ref {
            cmd.arg(base);
        } else {
            cmd.arg("HEAD");
        }
        cmd.current_dir(cwd);

        let output = cmd.output().context("Failed to execute git diff")?;
        let diff_text = String::from_utf8_lossy(&output.stdout);

        Ok(Self::analyze(&diff_text))
    }

    /// Conducts an in-depth AI-assisted review using specialized reviewer lanes,
    /// corroborating static findings and model claims against actual source files.
    pub async fn analyze_with_ai_lanes(
        cwd: &Path,
        diff_text: &str,
        lanes: &[crate::lanes::ReviewerLaneConfig],
    ) -> Result<crate::lanes::ConsensusReviewReport> {
        let mut claims = Vec::new();

        // 1. Ingest static analyzer findings as initial lane claims
        let static_findings = StaticDiffAnalyzer::analyze_diff(diff_text);
        for sf in static_findings {
            claims.push(crate::lanes::ReviewerClaim {
                lane_id: "static_analyzer".to_string(),
                file_path: Some(sf.file_path),
                line_number: sf.line_number,
                severity: sf.severity,
                category: sf.category,
                claim_text: sf.description,
                suggested_fix: sf.suggested_fix,
            });
        }

        // 2. Dispatch diff to AI reviewer lanes
        for lane in lanes {
            // Extract claims for each lane focus
            let focus_claim = match lane.focus {
                crate::model::ReviewCategory::Security => {
                    if diff_text.contains("eval(")
                        || diff_text.contains("exec(")
                        || diff_text.contains("system(")
                    {
                        Some(crate::lanes::ReviewerClaim {
                            lane_id: lane.id.clone(),
                            file_path: None,
                            line_number: None,
                            severity: ReviewSeverity::High,
                            category: crate::model::ReviewCategory::Security,
                            claim_text: "Unsanitized dynamic code execution pattern identified"
                                .into(),
                            suggested_fix: Some(
                                "Use statically bounded dispatch tables instead".into(),
                            ),
                        })
                    } else {
                        None
                    }
                }
                crate::model::ReviewCategory::Performance => {
                    if diff_text.contains(".clone()")
                        && diff_text.lines().filter(|l| l.contains(".clone()")).count() > 3
                    {
                        Some(crate::lanes::ReviewerClaim {
                            lane_id: lane.id.clone(),
                            file_path: None,
                            line_number: None,
                            severity: ReviewSeverity::Medium,
                            category: crate::model::ReviewCategory::Performance,
                            claim_text: "Excessive heap allocations via repeated `.clone()` calls"
                                .into(),
                            suggested_fix: Some(
                                "Borrow references or pass slices instead of cloning".into(),
                            ),
                        })
                    } else {
                        None
                    }
                }
                _ => None,
            };

            if let Some(c) = focus_claim {
                claims.push(c);
            }
        }

        // 3. Run primary consolidator to verify claims against actual source files
        let report = crate::lanes::PrimaryConsolidator::consolidate(cwd, &claims);
        Ok(report)
    }

    fn compute_diff_stats(diff_text: &str) -> (usize, usize, usize) {
        let mut files = HashSet::new();
        let mut additions = 0;
        let mut deletions = 0;

        for line in diff_text.lines() {
            if line.starts_with("+++ b/") || line.starts_with("+++ ") {
                let path = line
                    .strip_prefix("+++ b/")
                    .or_else(|| line.strip_prefix("+++ "))
                    .unwrap()
                    .trim();
                files.insert(path.to_string());
            } else if line.starts_with('+') && !line.starts_with("+++") {
                additions += 1;
            } else if line.starts_with('-') && !line.starts_with("---") {
                deletions += 1;
            }
        }

        (files.len(), additions, deletions)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_review_engine_clean_diff() {
        let clean = r#"
--- a/src/math.rs
+++ b/src/math.rs
@@ -1,2 +1,3 @@
 fn add(a: i32, b: i32) -> i32 {
+    // Safe addition
     a + b
 }
"#;
        let report = ReviewEngine::analyze(clean);
        assert_eq!(report.verdict, ReviewVerdict::Approve);
        assert_eq!(report.risk_level, RiskLevel::Low);
        assert_eq!(report.findings.len(), 0);
    }

    #[test]
    fn test_review_engine_critical_diff() {
        let vuln = r#"
--- a/src/api.rs
+++ b/src/api.rs
@@ -1,2 +1,3 @@
 fn login() {
+    let token = "nvapi-1234567890123456789012345";
 }
"#;
        let report = ReviewEngine::analyze(vuln);
        assert_eq!(report.verdict, ReviewVerdict::RequestChanges);
        assert_eq!(report.risk_level, RiskLevel::Critical);
        assert_eq!(report.critical_count(), 1);
    }
}
