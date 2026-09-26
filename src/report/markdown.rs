//! Human-readable executive Markdown report projector (RPT-02, D-13).

use crate::report::model::{CompletionReport, ReportCandidate};

/// Projector for rendering CompletionReport and ReportCandidate into standard Markdown (REPORT.md).
pub struct MarkdownReportProjector;

impl MarkdownReportProjector {
    /// Render a completed `CompletionReport` into Markdown with YAML frontmatter.
    pub fn render_report(report: &CompletionReport) -> String {
        let mut out = String::with_capacity(4096);

        // 1. YAML Frontmatter
        out.push_str("---\n");
        out.push_str(&format!("mission_id: \"{}\"\n", report.mission_id));
        out.push_str(&format!("status: \"{}\"\n", report.final_status.as_str()));
        out.push_str(&format!(
            "timestamp: \"{}\"\n",
            report.created_at.to_rfc3339()
        ));
        out.push_str(&format!("tasks_count: {}\n", report.tasks_executed.len()));
        out.push_str(&format!("duration_seconds: {}\n", report.duration_seconds));
        out.push_str(&format!("tokens_used: {}\n", report.total_tokens_used()));
        out.push_str(&format!("cost_usd: {:.4}\n", report.total_cost_usd()));
        out.push_str("---\n\n");

        // 2. Title & Executive Summary
        out.push_str(&format!(
            "# Mission Completion Report: {}\n\n",
            report.mission_id
        ));
        out.push_str("## Executive Summary\n\n");
        out.push_str(&format!(
            "- **Final Status:** {}\n",
            report.final_status.as_str().to_uppercase()
        ));
        out.push_str(&format!("- **Objective:** {}\n", report.objective));
        out.push_str(&format!(
            "- **Duration:** {} seconds\n",
            report.duration_seconds
        ));
        out.push_str(&format!(
            "- **Tasks Succeeded/Total:** {} / {}\n",
            report
                .tasks_executed
                .iter()
                .filter(|t| t.status.eq_ignore_ascii_case("succeeded"))
                .count(),
            report.tasks_executed.len()
        ));
        out.push_str(&format!(
            "- **Total Tokens Consumed:** {}\n",
            report.total_tokens_used()
        ));
        out.push_str(&format!(
            "- **Estimated Cost:** ${:.4} USD\n\n",
            report.total_cost_usd()
        ));

        // 3. Requirements & Verification Matrix
        out.push_str("## Requirements & Verification Matrix\n\n");
        if report.requirements_summary.is_empty() {
            out.push_str("*No specific requirements tracked.*\n\n");
        } else {
            out.push_str("| Requirement ID | Description | Status | Evidence |\n");
            out.push_str("|:---|:---|:---|:---|\n");
            for req in &report.requirements_summary {
                let evidence = req.evidence_locator.as_deref().unwrap_or("-");
                out.push_str(&format!(
                    "| `{}` | {} | {} | {} |\n",
                    req.id, req.description, req.status, evidence
                ));
            }
            out.push('\n');
        }

        // 4. Plan & Task Execution Log
        out.push_str("## Plan & Task Execution Log\n\n");
        out.push_str(&format!(
            "**Plan ID:** `{}` (Completed: {} / {}, Duration: {}s)\n\n",
            report.plan_summary.plan_id,
            report.plan_summary.completed_tasks,
            report.plan_summary.total_tasks,
            report.plan_summary.duration_seconds
        ));

        if report.tasks_executed.is_empty() {
            out.push_str("*No tasks executed.*\n\n");
        } else {
            out.push_str("| Task ID | Title | Status | Agent | Duration | Evidence |\n");
            out.push_str("|:---|:---|:---|:---|:---|:---|\n");
            for task in &report.tasks_executed {
                let agent = task
                    .agent_id
                    .map(|a| a.to_string())
                    .unwrap_or_else(|| "-".to_string());
                let evidence = task.evidence_locator.as_deref().unwrap_or("-");
                out.push_str(&format!(
                    "| `{}` | {} | {} | `{}` | {}s | {} |\n",
                    task.task_id, task.title, task.status, agent, task.duration_seconds, evidence
                ));
            }
            out.push('\n');
        }

        // 5. Agents & Models Utilized
        out.push_str("## Agents & Models Utilized\n\n");
        if report.agents_used.is_empty() {
            out.push_str("*No agent details recorded.*\n\n");
        } else {
            out.push_str("### Agents\n\n");
            out.push_str("| Agent ID | Role | Model | Steps | Tokens |\n");
            out.push_str("|:---|:---|:---|:---|:---|\n");
            for ag in &report.agents_used {
                out.push_str(&format!(
                    "| `{}` | {} | `{}` | {} | {} |\n",
                    ag.agent_id, ag.role, ag.model, ag.steps_count, ag.total_tokens
                ));
            }
            out.push('\n');
        }

        if !report.models_used.is_empty() {
            out.push_str("### Model Usage\n\n");
            out.push_str("| Provider | Model | Prompt Tokens | Completion Tokens | Total Tokens | Cost (USD) |\n");
            out.push_str("|:---|:---|:---|:---|:---|:---|\n");
            for m in &report.models_used {
                out.push_str(&format!(
                    "| {} | `{}` | {} | {} | {} | ${:.4} |\n",
                    m.provider,
                    m.model,
                    m.prompt_tokens,
                    m.completion_tokens,
                    m.total_tokens,
                    m.estimated_cost_usd
                ));
            }
            out.push('\n');
        }

        // 6. Code Changes & Git Attribution
        out.push_str("## Code Changes & Git Attribution\n\n");
        out.push_str(&format!(
            "- **Base Commit:** `{}`\n",
            report.git_state.base_commit
        ));
        out.push_str(&format!(
            "- **Final Commit:** `{}`\n",
            report.git_state.final_commit
        ));
        out.push_str(&format!("- **Branch:** `{}`\n", report.git_state.branch));
        out.push_str(&format!(
            "- **Clean Worktree:** {}\n",
            report.git_state.clean_worktree
        ));
        if !report.git_state.commit_trailers.is_empty() {
            out.push_str(&format!(
                "- **Trailers:** {}\n",
                report.git_state.commit_trailers.join(", ")
            ));
        }
        out.push('\n');

        if !report.files_changed.is_empty() {
            out.push_str("| File Path | Lines Added | Lines Removed | Diff Locator |\n");
            out.push_str("|:---|:---|:---|:---|\n");
            for f in &report.files_changed {
                let diff = f.diff_artifact_locator.as_deref().unwrap_or("-");
                out.push_str(&format!(
                    "| `{}` | +{} | -{} | {} |\n",
                    f.path, f.lines_added, f.lines_removed, diff
                ));
            }
            out.push('\n');
        }

        // 7. Verification & Review Verdicts
        out.push_str("## Verification & Review Verdicts\n\n");
        if report.verifications.is_empty() {
            out.push_str("*No verification checks recorded.*\n\n");
        } else {
            out.push_str("### Verification Checks\n\n");
            out.push_str("| Tier | Check ID | Status | Evidence |\n");
            out.push_str("|:---|:---|:---|:---|\n");
            for v in &report.verifications {
                let evidence = v.evidence_locator.as_deref().unwrap_or("-");
                out.push_str(&format!(
                    "| Tier {} | `{}` | {} | {} |\n",
                    v.tier, v.check_id, v.status, evidence
                ));
            }
            out.push('\n');
        }

        if !report.review_verdicts.is_empty() {
            out.push_str("### Review Verdicts\n\n");
            out.push_str("| Reviewer Role | Verdict | Comments | Timestamp |\n");
            out.push_str("|:---|:---|:---|:---|\n");
            for r in &report.review_verdicts {
                out.push_str(&format!(
                    "| {} | {} | {} | {} |\n",
                    r.reviewer_role,
                    r.verdict,
                    r.comments,
                    r.timestamp.to_rfc3339()
                ));
            }
            out.push('\n');
        }

        // 8. Failure & Recovery History
        out.push_str("## Failure & Recovery History\n\n");
        if report.failures_encountered.is_empty() && report.retries_and_replans.is_empty() {
            out.push_str("*Clean execution: no unhandled failures or unplanned retries.*\n\n");
        } else {
            if !report.failures_encountered.is_empty() {
                out.push_str("### Failures Encountered\n\n");
                out.push_str("| Classification | Root Cause | Recovered |\n");
                out.push_str("|:---|:---|:---|\n");
                for fail in &report.failures_encountered {
                    out.push_str(&format!(
                        "| {} | {} | {} |\n",
                        fail.classification, fail.root_cause, fail.recovered
                    ));
                }
                out.push('\n');
            }

            if !report.retries_and_replans.is_empty() {
                out.push_str("### Retries & Replans\n\n");
                out.push_str("| Strategy | Count | Justification |\n");
                out.push_str("|:---|:---|:---|\n");
                for rec in &report.retries_and_replans {
                    out.push_str(&format!(
                        "| {} | {} | {} |\n",
                        rec.strategy, rec.count, rec.justification
                    ));
                }
                out.push('\n');
            }
        }

        // 9. Policy & Governance Escalations
        out.push_str("## Policy & Governance Escalations\n\n");
        if report.policy_escalations.is_empty() {
            out.push_str("*No policy escalations recorded during this mission.*\n\n");
        } else {
            out.push_str("| Tool | Decision | Actor | Reason |\n");
            out.push_str("|:---|:---|:---|:---|\n");
            for pol in &report.policy_escalations {
                out.push_str(&format!(
                    "| `{}` | {} | {} | {} |\n",
                    pol.tool, pol.decision, pol.actor, pol.reason
                ));
            }
            out.push('\n');
        }

        // 10. Artifact Index
        out.push_str("## Artifact Index\n\n");
        if report.artifacts_produced.is_empty() {
            out.push_str("*No external artifacts produced.*\n\n");
        } else {
            out.push_str("| Artifact ID | Name | Path | Size | SHA-256 |\n");
            out.push_str("|:---|:---|:---|:---|:---|\n");
            for art in &report.artifacts_produced {
                out.push_str(&format!(
                    "| `{}` | {} | `{}` | {} bytes | `{}` |\n",
                    art.id, art.name, art.path, art.size_bytes, art.sha256
                ));
            }
            out.push('\n');
        }

        out
    }

    /// Render a `ReportCandidate` into candidate Markdown.
    pub fn render_candidate(candidate: &ReportCandidate) -> String {
        let provisional = candidate
            .provisional_status
            .unwrap_or(crate::report::model::CompletionStatus::Blocked);
        let sealed = candidate.clone().into_sealed(provisional);
        Self::render_report(&sealed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::MissionId;
    use crate::report::model::*;
    use chrono::Utc;

    #[test]
    fn test_markdown_projection_has_yaml_frontmatter_and_sections() {
        let report = CompletionReport {
            schema_version: 1,
            mission_id: MissionId::new(),
            objective: "Automate mission reporting".to_string(),
            requirements_summary: vec![RequirementReportItem {
                id: "RPT-02".to_string(),
                description: "Markdown report projection".to_string(),
                status: "Satisfied".to_string(),
                evidence_locator: Some(
                    "artifact://018f0000-0000-7000-8000-000000000001".to_string(),
                ),
            }],
            plan_summary: PlanReportSummary {
                plan_id: "plan-12-04".to_string(),
                total_tasks: 2,
                completed_tasks: 2,
                failed_tasks: 0,
                duration_seconds: 120,
            },
            tasks_executed: vec![],
            agents_used: vec![],
            models_used: vec![],
            files_changed: vec![],
            git_state: GitReportState {
                base_commit: "abc".to_string(),
                final_commit: "def".to_string(),
                branch: "master".to_string(),
                commit_trailers: vec![],
                clean_worktree: true,
            },
            verifications: vec![],
            review_verdicts: vec![],
            failures_encountered: vec![],
            retries_and_replans: vec![],
            policy_escalations: vec![],
            artifacts_produced: vec![],
            final_status: CompletionStatus::Succeeded,
            created_at: Utc::now(),
            duration_seconds: 120,
        };

        let md = MarkdownReportProjector::render_report(&report);
        assert!(md.starts_with("---\n"));
        assert!(md.contains("mission_id:"));
        assert!(md.contains("status: \"succeeded\""));
        assert!(md.contains("## Executive Summary"));
        assert!(md.contains("## Requirements & Verification Matrix"));
        assert!(md.contains("## Plan & Task Execution Log"));
        assert!(md.contains("## Code Changes & Git Attribution"));
        assert!(md.contains("## Artifact Index"));
    }
}
