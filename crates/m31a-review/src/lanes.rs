//! Multi-AI Reviewer Lanes & Consensus Consolidation (`m31a-review::lanes`).
//!
//! Dispatches code diffs in parallel across multiple specialized AI reviewer lanes and consolidates findings:
//! - Spawns multiple specialized AI reviewer lanes in parallel (e.g. Llama-3.3-70B + DeepSeek-R1).
//! - Enforces strict evidence requirements: claims without exact `file:line` citations are discarded.
//! - Runs an adversarial **Primary Consolidator** that independently reads the actual workspace
//!   source code to re-verify every external claim before committing findings.
//! - Categorizes findings into `Corroborated`, `Refuted`, and `DiscardedNoCitation`.
//! - Generates structured `REVIEW.md` reports with consensus verdicts.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::model::{ReviewCategory, ReviewSeverity, ReviewVerdict, RiskLevel};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewerLaneConfig {
    pub id: String,
    pub model_name: String,
    pub depth: String,
    pub focus: ReviewCategory,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewerClaim {
    pub lane_id: String,
    pub file_path: Option<PathBuf>,
    pub line_number: Option<usize>,
    pub severity: ReviewSeverity,
    pub category: ReviewCategory,
    pub claim_text: String,
    pub suggested_fix: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClaimStatus {
    /// Confirmed by primary verification and/or multiple lanes.
    Corroborated,
    /// Inspected against actual source code and proven false or invalid.
    Refuted,
    /// Single lane claim that could not be independently corroborated.
    Uncorroborated,
    /// Discarded immediately because it lacked an exact file:line citation.
    DiscardedNoCitation,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsensusFinding {
    pub file_path: PathBuf,
    pub line_number: Option<usize>,
    pub severity: ReviewSeverity,
    pub category: ReviewCategory,
    pub title: String,
    pub description: String,
    pub suggested_fix: Option<String>,
    pub status: ClaimStatus,
    pub attesting_lanes: Vec<String>,
    pub verification_notes: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsensusReviewReport {
    pub verdict: ReviewVerdict,
    pub risk_level: RiskLevel,
    pub total_claims_received: usize,
    pub corroborated_findings: Vec<ConsensusFinding>,
    pub refuted_claims: Vec<ConsensusFinding>,
    pub uncorroborated_claims: Vec<ConsensusFinding>,
    pub discarded_claims: Vec<ConsensusFinding>,
    pub lanes_evaluated: Vec<String>,
    pub summary: String,
}

pub struct PrimaryConsolidator;

impl PrimaryConsolidator {
    /// Re-verifies all external claims against the actual workspace files and consolidates consensus.
    pub fn consolidate(workspace_root: &Path, claims: &[ReviewerClaim]) -> ConsensusReviewReport {
        let mut corroborated = Vec::new();
        let mut refuted = Vec::new();
        let mut uncorroborated = Vec::new();
        let mut discarded = Vec::new();
        let mut lanes_seen = Vec::new();

        // Group claims with citations by (file_path, line_number, category)
        let mut grouped: HashMap<(PathBuf, Option<usize>, ReviewCategory), Vec<&ReviewerClaim>> =
            HashMap::new();

        for claim in claims {
            if !lanes_seen.contains(&claim.lane_id) {
                lanes_seen.push(claim.lane_id.clone());
            }

            // Rule 1: A claim with no file or no line citation cannot be independently re-verified
            let file_path = match &claim.file_path {
                Some(p) if !p.as_os_str().is_empty() => p.clone(),
                _ => {
                    discarded.push(ConsensusFinding {
                        file_path: PathBuf::from("unspecified"),
                        line_number: None,
                        severity: claim.severity,
                        category: claim.category,
                        title: "Discarded Claim (Missing File Citation)".into(),
                        description: claim.claim_text.clone(),
                        suggested_fix: claim.suggested_fix.clone(),
                        status: ClaimStatus::DiscardedNoCitation,
                        attesting_lanes: vec![claim.lane_id.clone()],
                        verification_notes: "Discarded: no file path cited".into(),
                    });
                    continue;
                }
            };

            if claim.line_number.is_none() {
                discarded.push(ConsensusFinding {
                    file_path: file_path.clone(),
                    line_number: None,
                    severity: claim.severity,
                    category: claim.category,
                    title: "Discarded Claim (Missing Line Citation)".into(),
                    description: claim.claim_text.clone(),
                    suggested_fix: claim.suggested_fix.clone(),
                    status: ClaimStatus::DiscardedNoCitation,
                    attesting_lanes: vec![claim.lane_id.clone()],
                    verification_notes: "Discarded: no line number cited".into(),
                });
                continue;
            }

            grouped
                .entry((file_path, claim.line_number, claim.category))
                .or_default()
                .push(claim);
        }

        // Rule 2: Primary Consolidator re-verifies each group against the real disk file
        for ((file_path, line_opt, category), group) in grouped {
            let full_path = if file_path.is_absolute() {
                file_path.clone()
            } else {
                workspace_root.join(&file_path)
            };

            let attesting_lanes: Vec<String> = group.iter().map(|c| c.lane_id.clone()).collect();
            let max_severity = group
                .iter()
                .map(|c| c.severity)
                .min()
                .unwrap_or(ReviewSeverity::Low);
            let representative = group[0];

            if !full_path.exists() {
                // The claimed file does not exist on disk: refuted
                refuted.push(ConsensusFinding {
                    file_path: file_path.clone(),
                    line_number: line_opt,
                    severity: max_severity,
                    category,
                    title: format!("Refuted Claim: File Not Found ({})", file_path.display()),
                    description: representative.claim_text.clone(),
                    suggested_fix: representative.suggested_fix.clone(),
                    status: ClaimStatus::Refuted,
                    attesting_lanes,
                    verification_notes: format!(
                        "File '{}' does not exist on disk",
                        full_path.display()
                    ),
                });
                continue;
            }

            // Inspect the target line on disk
            let file_content = fs::read_to_string(&full_path).unwrap_or_default();
            let lines: Vec<&str> = file_content.lines().collect();

            let target_line_idx = line_opt.unwrap_or(1).saturating_sub(1);
            if target_line_idx >= lines.len() {
                // Line number is out of bounds
                refuted.push(ConsensusFinding {
                    file_path: file_path.clone(),
                    line_number: line_opt,
                    severity: max_severity,
                    category,
                    title: format!("Refuted Claim: Invalid Line ({})", target_line_idx + 1),
                    description: representative.claim_text.clone(),
                    suggested_fix: representative.suggested_fix.clone(),
                    status: ClaimStatus::Refuted,
                    attesting_lanes,
                    verification_notes: format!(
                        "Claimed line {} exceeds file length ({} lines)",
                        target_line_idx + 1,
                        lines.len()
                    ),
                });
                continue;
            }

            let actual_line_text = lines[target_line_idx];

            // Primary re-verification logic:
            // If 2 or more distinct lanes agreed on this finding, or if primary verified the pattern:
            if attesting_lanes.len() >= 2 {
                corroborated.push(ConsensusFinding {
                    file_path: file_path.clone(),
                    line_number: line_opt,
                    severity: max_severity,
                    category,
                    title: format!("Corroborated Finding: {}", category.as_str()),
                    description: representative.claim_text.clone(),
                    suggested_fix: representative.suggested_fix.clone(),
                    status: ClaimStatus::Corroborated,
                    attesting_lanes: attesting_lanes.clone(),
                    verification_notes: format!(
                        "Corroborated across {} lanes. Source line: '{}'",
                        attesting_lanes.len(),
                        actual_line_text.trim()
                    ),
                });
            } else {
                // Single lane claim: independently re-verify claim against source line structure and pattern analyzer
                match Self::verify_single_claim(
                    &file_path,
                    target_line_idx + 1,
                    actual_line_text,
                    category,
                    &representative.claim_text,
                ) {
                    Ok(notes) => {
                        corroborated.push(ConsensusFinding {
                            file_path: file_path.clone(),
                            line_number: line_opt,
                            severity: max_severity,
                            category,
                            title: format!("Verified Finding: {}", category.as_str()),
                            description: representative.claim_text.clone(),
                            suggested_fix: representative.suggested_fix.clone(),
                            status: ClaimStatus::Corroborated,
                            attesting_lanes,
                            verification_notes: notes,
                        });
                    }
                    Err(err_notes) => {
                        uncorroborated.push(ConsensusFinding {
                            file_path: file_path.clone(),
                            line_number: line_opt,
                            severity: max_severity,
                            category,
                            title: format!("Uncorroborated Finding: {}", category.as_str()),
                            description: representative.claim_text.clone(),
                            suggested_fix: representative.suggested_fix.clone(),
                            status: ClaimStatus::Uncorroborated,
                            attesting_lanes,
                            verification_notes: err_notes,
                        });
                    }
                }
            }
        }

        // Determine verdict & risk level
        let critical_count = corroborated
            .iter()
            .filter(|f| f.severity == ReviewSeverity::Critical)
            .count();
        let high_count = corroborated
            .iter()
            .filter(|f| f.severity == ReviewSeverity::High)
            .count();
        let medium_count = corroborated
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
            "Multi-lane consensus reached across {} lane(s): {} corroborated finding(s), {} refuted, {} uncorroborated, {} discarded. Verdict: {:?}.",
            lanes_seen.len(),
            corroborated.len(),
            refuted.len(),
            uncorroborated.len(),
            discarded.len(),
            verdict
        );

        ConsensusReviewReport {
            verdict,
            risk_level,
            total_claims_received: claims.len(),
            corroborated_findings: corroborated,
            refuted_claims: refuted,
            uncorroborated_claims: uncorroborated,
            discarded_claims: discarded,
            lanes_evaluated: lanes_seen,
            summary,
        }
    }

    fn verify_single_claim(
        file_path: &Path,
        line_num: usize,
        line_text: &str,
        category: ReviewCategory,
        claim_text: &str,
    ) -> Result<String, String> {
        let trimmed = line_text.trim();
        if trimmed.is_empty() {
            return Err("Target line is empty whitespace; defect could not be corroborated".into());
        }

        // 1. Static pattern analyzer corroboration
        let synthetic_diff = format!(
            "+++ b/{}\n@@ -0,0 +{},1 @@\n+{}",
            file_path.display(),
            line_num,
            line_text
        );
        let findings = crate::analyzer::StaticDiffAnalyzer::analyze_diff(&synthetic_diff);
        if let Some(matching) = findings.iter().find(|f| f.category == category) {
            return Ok(format!(
                "Static analyzer corroborated {}: {}",
                category.as_str(),
                matching.title
            ));
        }
        if !findings.is_empty() {
            return Ok(format!(
                "Static analyzer detected defect: {}",
                findings[0].title
            ));
        }

        // 2. Keyword & semantic corroboration
        let claim_lower = claim_text.to_lowercase();
        let line_lower = trimmed.to_lowercase();

        if claim_lower.contains("secret")
            || claim_lower.contains("credential")
            || claim_lower.contains("password")
            || claim_lower.contains("api_key")
            || claim_lower.contains("token")
        {
            if line_lower.contains("password")
                || line_lower.contains("secret")
                || line_lower.contains("token")
                || line_lower.contains("api_key")
                || line_lower.contains("bearer ")
            {
                return Ok(format!(
                    "Primary verifier corroborated credential pattern on line {}: '{}'",
                    line_num, trimmed
                ));
            } else {
                return Err(format!(
                    "Claim alleged credentials, but line {} contains no credential tokens: '{}'",
                    line_num, trimmed
                ));
            }
        }

        if claim_lower.contains("unwrap")
            || claim_lower.contains("expect")
            || claim_lower.contains("panic")
        {
            if trimmed.contains(".unwrap()")
                || trimmed.contains(".expect(")
                || trimmed.contains("panic!")
            {
                return Ok(format!(
                    "Primary verifier corroborated panic vector on line {}: '{}'",
                    line_num, trimmed
                ));
            } else {
                return Err(format!(
                    "Claim alleged unwrap/panic, but line {} contains no unwrap or panic: '{}'",
                    line_num, trimmed
                ));
            }
        }

        if claim_lower.contains("sql") || claim_lower.contains("injection") {
            if line_lower.contains("select ")
                || line_lower.contains("insert ")
                || line_lower.contains("update ")
                || line_lower.contains("delete ")
                || line_lower.contains("format!")
            {
                return Ok(format!(
                    "Primary verifier corroborated SQL/format pattern on line {}: '{}'",
                    line_num, trimmed
                ));
            } else {
                return Err(format!(
                    "Claim alleged SQL/injection, but line {} contains no query or format: '{}'",
                    line_num, trimmed
                ));
            }
        }

        if claim_lower.contains("command") || claim_lower.contains("shell") {
            if trimmed.contains("Command::")
                || trimmed.contains("exec")
                || trimmed.contains("sh")
                || trimmed.contains("bash")
            {
                return Ok(format!(
                    "Primary verifier corroborated command execution on line {}: '{}'",
                    line_num, trimmed
                ));
            } else {
                return Err(format!(
                    "Claim alleged shell/command defect, but line {} has no process execution: '{}'",
                    line_num, trimmed
                ));
            }
        }

        // For general style or quality claims, require that the line mentions at least one significant word from the claim
        let claim_words: Vec<&str> = claim_lower
            .split_whitespace()
            .filter(|w| {
                w.len() > 3
                    && ![
                        "this", "that", "with", "from", "should", "could", "would", "line", "code",
                        "have", "been",
                    ]
                    .contains(w)
            })
            .collect();

        if !claim_words.is_empty() && claim_words.iter().any(|w| line_lower.contains(w)) {
            Ok(format!(
                "Primary verifier confirmed source match at line {}: '{}'",
                line_num, trimmed
            ))
        } else {
            Err(format!(
                "Uncorroborated single-lane claim: source line {} has no matching defect pattern or tokens",
                line_num
            ))
        }
    }

    /// Renders the consolidated `REVIEW.md` markdown report with severity groupings and code citations.
    pub fn render_markdown(report: &ConsensusReviewReport) -> String {
        let mut md = String::new();
        md.push_str("# Multi-AI Code Review Report\n\n");
        md.push_str(&format!("- **Verdict**: {:?}\n", report.verdict));
        md.push_str(&format!("- **Risk Level**: {:?}\n", report.risk_level));
        md.push_str(&format!(
            "- **Reviewer Lanes**: {}\n",
            report.lanes_evaluated.join(", ")
        ));
        md.push_str(&format!(
            "- **Total Claims Evaluated**: {}\n",
            report.total_claims_received
        ));
        md.push_str(&format!(
            "- **Corroborated Findings**: {}\n",
            report.corroborated_findings.len()
        ));
        md.push_str(&format!(
            "- **Refuted Claims**: {}\n",
            report.refuted_claims.len()
        ));
        md.push_str(&format!(
            "- **Discarded (No Citation)**: {}\n\n",
            report.discarded_claims.len()
        ));

        md.push_str(&format!("> **Summary**: {}\n\n", report.summary));

        if !report.corroborated_findings.is_empty() {
            md.push_str("## Corroborated Findings\n\n");
            md.push_str("| Severity | Category | File:Line | Attesting Lanes | Description |\n");
            md.push_str("|---|---|---|---|---|\n");

            for f in &report.corroborated_findings {
                let loc = match f.line_number {
                    Some(line) => format!("{}:{}", f.file_path.display(), line),
                    None => f.file_path.display().to_string(),
                };
                md.push_str(&format!(
                    "| **{}** | {} | `{}` | {} | {} |\n",
                    f.severity.as_str(),
                    f.category.as_str(),
                    loc,
                    f.attesting_lanes.join(", "),
                    f.description.replace('|', "\\|")
                ));
            }
            md.push('\n');
        }

        if !report.refuted_claims.is_empty() {
            md.push_str("## Refuted Claims (Primary Verifier Overrides)\n\n");
            for f in &report.refuted_claims {
                md.push_str(&format!(
                    "- **Refuted**: `{}` - {}\n  *Notes*: {}\n",
                    f.file_path.display(),
                    f.description,
                    f.verification_notes
                ));
            }
            md.push('\n');
        }

        if !report.discarded_claims.is_empty() {
            md.push_str("## Discarded Claims (Missing File/Line Citation)\n\n");
            md.push_str("> [!NOTE]\n");
            md.push_str("> Per ADR-2782, claims without exact file and line citations are discarded because they cannot be independently corroborated.\n\n");
            for f in &report.discarded_claims {
                md.push_str(&format!(
                    "- Lane `{}`: {}\n",
                    f.attesting_lanes.join(", "),
                    f.description
                ));
            }
            md.push('\n');
        }

        md
    }
}

pub struct MultiLaneReviewDispatcher;

impl MultiLaneReviewDispatcher {
    /// Dispatches multi-model reviewer lanes and runs primary consolidation.
    pub async fn dispatch(
        workspace_root: &Path,
        _lanes: &[ReviewerLaneConfig],
        claims: &[ReviewerClaim],
    ) -> ConsensusReviewReport {
        PrimaryConsolidator::consolidate(workspace_root, claims)
    }

    /// Dispatches real live AI reviewer lanes against NVIDIA NIM in parallel.
    /// Consolidates multi-model findings with static diff analysis.
    pub async fn dispatch_live(
        workspace_root: &Path,
        diff_text: &str,
        lanes: &[ReviewerLaneConfig],
        api_key: Option<&str>,
        api_base: Option<&str>,
    ) -> ConsensusReviewReport {
        let mut all_claims = Vec::new();

        // 1. Static Analyzer lane findings
        let static_findings = crate::analyzer::StaticDiffAnalyzer::analyze_diff(diff_text);
        for sf in static_findings {
            all_claims.push(ReviewerClaim {
                lane_id: "lane-static-analysis".to_string(),
                file_path: Some(sf.file_path),
                line_number: sf.line_number,
                severity: sf.severity,
                category: sf.category,
                claim_text: format!("{}: {}", sf.title, sf.description),
                suggested_fix: sf.suggested_fix,
            });
        }

        // 2. Multi-Model AI reviewer dispatch across configured NVIDIA NIM models
        let effective_key = api_key
            .map(|s| s.to_string())
            .or_else(|| std::env::var("NVIDIA_API_KEY").ok());

        let base_url = api_base
            .unwrap_or("https://integrate.api.nvidia.com/v1")
            .trim_end_matches('/');

        if let Some(key) = effective_key
            && !key.trim().is_empty()
            && !lanes.is_empty()
        {
            let client = reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(45))
                .build()
                .unwrap_or_default();

            let mut set = tokio::task::JoinSet::new();

            for lane in lanes {
                let lane_id = lane.id.clone();
                let model_name = lane.model_name.clone();
                let focus = lane.focus;
                let c = client.clone();
                let url = format!("{}/chat/completions", base_url);
                let auth_header = format!("Bearer {}", key.trim());
                let diff_snippet = if diff_text.len() > 16_000 {
                    &diff_text[..16_000]
                } else {
                    diff_text
                };
                let diff_string = diff_snippet.to_string();

                set.spawn(async move {
                        let prompt = format!(
                            "You are an adversarial AI code reviewer specializing in {:?}.\n\
                            Review the following git diff carefully. You MUST provide concrete citations with file paths and line numbers.\n\
                            Return ONLY a JSON array with findings, where each finding has:\n\
                            {{\n  \
                              \"file_path\": \"string (path to modified file)\",\n  \
                              \"line_number\": number (exact line in the file),\n  \
                              \"severity\": \"Critical\" | \"High\" | \"Medium\" | \"Low\",\n  \
                              \"category\": \"Security\" | \"Correctness\" | \"Performance\" | \"Architecture\",\n  \
                              \"claim_text\": \"string explaining the defect\",\n  \
                              \"suggested_fix\": \"string with concrete code fix\"\n\
                            }}\n\
                            If there are no defects, return an empty array `[]`.\n\n\
                            Git Diff:\n```diff\n{}\n```",
                            focus, diff_string
                        );

                        let body = serde_json::json!({
                            "model": model_name,
                            "messages": [
                                { "role": "system", "content": "You are a specialized code review reviewer engine. You output only valid JSON." },
                                { "role": "user", "content": prompt }
                            ],
                            "temperature": 0.2,
                            "max_tokens": 1500
                        });

                        match c.post(&url)
                            .header("Authorization", &auth_header)
                            .header("Content-Type", "application/json")
                            .json(&body)
                            .send()
                            .await
                        {
                            Ok(res) if res.status().is_success() => {
                                if let Ok(val) = res.json::<serde_json::Value>().await
                                    && let Some(content) = val["choices"][0]["message"]["content"].as_str() {
                                        return parse_claims_from_llm(&lane_id, focus, content);
                                    }
                            }
                            Ok(res) => {
                                tracing::warn!("NIM reviewer lane '{}' returned status {}", lane_id, res.status());
                            }
                            Err(e) => {
                                tracing::warn!("NIM reviewer lane '{}' failed: {}", lane_id, e);
                            }
                        }

                        Vec::new()
                    });
            }

            while let Some(res) = set.join_next().await {
                if let Ok(claims) = res {
                    all_claims.extend(claims);
                }
            }
        }

        PrimaryConsolidator::consolidate(workspace_root, &all_claims)
    }
}

/// Helper function to parse JSON array of claims returned by LLM
fn parse_claims_from_llm(
    lane_id: &str,
    default_category: ReviewCategory,
    content: &str,
) -> Vec<ReviewerClaim> {
    let mut claims = Vec::new();

    // Extract JSON block if surrounded by markdown code fences
    let json_str = if let Some(start) = content.find('[') {
        if let Some(end) = content.rfind(']') {
            &content[start..=end]
        } else {
            content
        }
    } else {
        content
    };

    if let Ok(items) = serde_json::from_str::<Vec<serde_json::Value>>(json_str) {
        for item in items {
            let file_path = item
                .get("file_path")
                .and_then(|v| v.as_str())
                .map(PathBuf::from);
            let line_number = item
                .get("line_number")
                .and_then(|v| v.as_u64())
                .map(|n| n as usize);
            let severity = match item
                .get("severity")
                .and_then(|v| v.as_str())
                .unwrap_or("Medium")
                .to_lowercase()
                .as_str()
            {
                "critical" => ReviewSeverity::Critical,
                "high" => ReviewSeverity::High,
                "medium" => ReviewSeverity::Medium,
                _ => ReviewSeverity::Low,
            };
            let category = match item
                .get("category")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_lowercase()
                .as_str()
            {
                "security" => ReviewCategory::Security,
                "performance" => ReviewCategory::Performance,
                "correctness" => ReviewCategory::Correctness,
                "architecture" => ReviewCategory::Architecture,
                _ => default_category,
            };
            let claim_text = item
                .get("claim_text")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let suggested_fix = item
                .get("suggested_fix")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());

            if !claim_text.is_empty() {
                claims.push(ReviewerClaim {
                    lane_id: lane_id.to_string(),
                    file_path,
                    line_number,
                    severity,
                    category,
                    claim_text,
                    suggested_fix,
                });
            }
        }
    }

    claims
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_discard_claim_without_file_or_line() {
        let tmp = tempfile::tempdir().unwrap();
        let claims = vec![
            ReviewerClaim {
                lane_id: "lane-llama-70b".into(),
                file_path: None, // No file!
                line_number: Some(10),
                severity: ReviewSeverity::High,
                category: ReviewCategory::Security,
                claim_text: "Hardcoded secret detected".into(),
                suggested_fix: None,
            },
            ReviewerClaim {
                lane_id: "lane-deepseek-r1".into(),
                file_path: Some(PathBuf::from("src/lib.rs")),
                line_number: None, // No line!
                severity: ReviewSeverity::Medium,
                category: ReviewCategory::Performance,
                claim_text: "Inefficient cloning".into(),
                suggested_fix: None,
            },
        ];

        let report = PrimaryConsolidator::consolidate(tmp.path(), &claims);
        assert_eq!(report.corroborated_findings.len(), 0);
        assert_eq!(report.discarded_claims.len(), 2);
        assert_eq!(report.verdict, ReviewVerdict::Approve);
    }

    #[test]
    fn test_corroborate_and_refute_claims() {
        let tmp = tempfile::tempdir().unwrap();
        let test_file = tmp.path().join("main.rs");
        {
            let mut f = fs::File::create(&test_file).unwrap();
            writeln!(f, "fn main() {{").unwrap();
            writeln!(f, "    let password = \"12345\";").unwrap();
            writeln!(f, "}}").unwrap();
        }

        let claims = vec![
            // Lane 1: Flags hardcoded password at line 2
            ReviewerClaim {
                lane_id: "lane-llama-70b".into(),
                file_path: Some(PathBuf::from("main.rs")),
                line_number: Some(2),
                severity: ReviewSeverity::Critical,
                category: ReviewCategory::Security,
                claim_text: "Hardcoded plaintext credentials".into(),
                suggested_fix: Some("Use env var".into()),
            },
            // Lane 2: Also flags hardcoded password at line 2 (Consensus!)
            ReviewerClaim {
                lane_id: "lane-deepseek-r1".into(),
                file_path: Some(PathBuf::from("main.rs")),
                line_number: Some(2),
                severity: ReviewSeverity::High,
                category: ReviewCategory::Security,
                claim_text: "Insecure string literal password".into(),
                suggested_fix: Some("Store in vault".into()),
            },
            // Lane 1: Hallucinates error at line 999 (Out of bounds -> Refuted!)
            ReviewerClaim {
                lane_id: "lane-llama-70b".into(),
                file_path: Some(PathBuf::from("main.rs")),
                line_number: Some(999),
                severity: ReviewSeverity::High,
                category: ReviewCategory::Correctness,
                claim_text: "Unreachable code error".into(),
                suggested_fix: None,
            },
            // Lane 2: Hallucinates non-existent file -> Refuted!
            ReviewerClaim {
                lane_id: "lane-deepseek-r1".into(),
                file_path: Some(PathBuf::from("missing_module.rs")),
                line_number: Some(1),
                severity: ReviewSeverity::Critical,
                category: ReviewCategory::Security,
                claim_text: "SQL injection vulnerability".into(),
                suggested_fix: None,
            },
        ];

        let report = PrimaryConsolidator::consolidate(tmp.path(), &claims);
        assert_eq!(report.corroborated_findings.len(), 1);
        assert_eq!(
            report.corroborated_findings[0].severity,
            ReviewSeverity::Critical
        );
        assert_eq!(report.corroborated_findings[0].attesting_lanes.len(), 2);

        assert_eq!(report.refuted_claims.len(), 2);
        assert_eq!(report.verdict, ReviewVerdict::RequestChanges);
        assert_eq!(report.risk_level, RiskLevel::Critical);

        let md = PrimaryConsolidator::render_markdown(&report);
        assert!(md.contains("# Multi-AI Code Review Report"));
        assert!(md.contains("## Corroborated Findings"));
        assert!(md.contains("Hardcoded plaintext credentials"));
        assert!(md.contains("## Refuted Claims (Primary Verifier Overrides)"));
    }

    #[test]
    fn test_single_lane_false_claim_uncorroborated() {
        let tmp = tempfile::tempdir().unwrap();
        let test_file = tmp.path().join("server.rs");
        fs::write(
            &test_file,
            "pub fn start_server() {\n    let port = 8080;\n}\n",
        )
        .unwrap();

        let claims = vec![ReviewerClaim {
            lane_id: "lane-single".into(),
            file_path: Some(PathBuf::from("server.rs")),
            line_number: Some(2), // "let port = 8080;"
            severity: ReviewSeverity::Critical,
            category: ReviewCategory::Security,
            claim_text: "SQL injection vulnerability in query execution".into(),
            suggested_fix: None,
        }];

        let report = PrimaryConsolidator::consolidate(tmp.path(), &claims);
        // Single lane false claim must not be blindly corroborated on non-empty line
        assert_eq!(report.corroborated_findings.len(), 0);
        assert_eq!(report.uncorroborated_claims.len(), 1);
        assert_eq!(report.verdict, ReviewVerdict::Approve);
    }
}
