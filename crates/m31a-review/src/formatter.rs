//! Output formatters for code review reports (Terminal ANSI, Markdown, JSON).

use crate::model::{ReviewReport, ReviewSeverity, ReviewVerdict, RiskLevel};
use colored::Colorize;

pub struct ReviewFormatter;

impl ReviewFormatter {
    /// Renders a richly styled terminal report for interactive CLI and TUI display.
    pub fn render_terminal(report: &ReviewReport) -> String {
        let mut out = String::new();

        out.push_str("=========================================================\n");
        out.push_str("  M31A Automated Code Review Report\n");
        out.push_str("=========================================================\n\n");

        // Status Summary Header
        let verdict_str = match report.verdict {
            ReviewVerdict::Approve => "APPROVE".green().bold(),
            ReviewVerdict::Comment => "COMMENT".yellow().bold(),
            ReviewVerdict::RequestChanges => "REQUEST CHANGES".red().bold(),
        };

        let risk_str = match report.risk_level {
            RiskLevel::Low => "LOW".green().bold(),
            RiskLevel::Medium => "MEDIUM".yellow().bold(),
            RiskLevel::High => "HIGH".red().bold(),
            RiskLevel::Critical => "CRITICAL".bright_red().bold(),
        };

        out.push_str(&format!("  Verdict:           {}\n", verdict_str));
        out.push_str(&format!("  Overall Risk:      {}\n", risk_str));
        out.push_str(&format!("  Files Reviewed:    {}\n", report.files_reviewed));
        out.push_str(&format!(
            "  Diff Volume:       +{} / -{} lines\n",
            report.total_additions, report.total_deletions
        ));
        out.push_str(&format!(
            "  Total Findings:    {} (Critical: {}, High: {}, Medium: {})\n\n",
            report.findings.len(),
            report.critical_count(),
            report.high_count(),
            report.medium_count(),
        ));

        if !report.summary.is_empty() {
            out.push_str("Summary:\n");
            out.push_str(&format!("  {}\n\n", report.summary));
        }

        if report.findings.is_empty() {
            out.push_str(&format!(
                "{}\n",
                "[OK] No security vulnerabilities or critical defects identified in diff."
                    .green()
                    .bold()
            ));
            return out;
        }

        out.push_str("Detailed Findings:\n");
        out.push_str("---------------------------------------------------------\n");

        for (idx, f) in report.findings.iter().enumerate() {
            let sev_badge = match f.severity {
                ReviewSeverity::Critical => "[CRITICAL]".bright_red().bold(),
                ReviewSeverity::High => "[HIGH]".red().bold(),
                ReviewSeverity::Medium => "[MEDIUM]".yellow().bold(),
                ReviewSeverity::Low => "[LOW]".blue(),
                ReviewSeverity::Suggestion => "[SUGGESTION]".cyan(),
                ReviewSeverity::Praise => "[PRAISE]".green(),
            };

            let line_str = f.line_number.map(|l| format!(":{}", l)).unwrap_or_default();
            out.push_str(&format!(
                "{}. {} {} ({}{})\n",
                idx + 1,
                sev_badge,
                f.title.bold(),
                f.file_path.display(),
                line_str
            ));
            out.push_str(&format!("   Category:    {}\n", f.category.as_str().cyan()));
            out.push_str(&format!("   Description: {}\n", f.description));

            if let Some(snippet) = &f.code_snippet {
                out.push_str(&format!("   Code:\n     {}\n", snippet.dimmed()));
            }

            if let Some(fix) = &f.suggested_fix {
                out.push_str(&format!("   Recommendation: {}\n", fix.green()));
            }
            out.push('\n');
        }

        out
    }

    /// Renders a GitHub-compatible markdown review report.
    pub fn render_markdown(report: &ReviewReport) -> String {
        let mut out = String::new();

        out.push_str("## M31A Automated Code Review Report\n\n");
        out.push_str(&format!("- **Verdict**: {:?}\n", report.verdict));
        out.push_str(&format!("- **Risk Level**: {:?}\n", report.risk_level));
        out.push_str(&format!(
            "- **Stats**: {} files changed, +{} / -{} lines\n",
            report.files_reviewed, report.total_additions, report.total_deletions
        ));
        out.push_str(&format!(
            "- **Findings**: {} total (Critical: {}, High: {}, Medium: {})\n\n",
            report.findings.len(),
            report.critical_count(),
            report.high_count(),
            report.medium_count(),
        ));

        if !report.summary.is_empty() {
            out.push_str("### Executive Summary\n");
            out.push_str(&format!("{}\n\n", report.summary));
        }

        if report.findings.is_empty() {
            out.push_str("> [!TIP]\n> **Clean Diff**: No security vulnerabilities, regressions, or code defects detected.\n");
            return out;
        }

        out.push_str("### Findings & Recommendations\n\n");
        for (i, f) in report.findings.iter().enumerate() {
            let line_str = f
                .line_number
                .map(|l| format!(":L{}", l))
                .unwrap_or_default();
            out.push_str(&format!(
                "#### {}. [{}] {}\n",
                i + 1,
                f.severity.as_str(),
                f.title
            ));
            out.push_str(&format!(
                "- **File**: `{}`{}\n",
                f.file_path.display(),
                line_str
            ));
            out.push_str(&format!("- **Category**: {}\n", f.category.as_str()));
            out.push_str(&format!("- **Issue**: {}\n", f.description));

            if let Some(snippet) = &f.code_snippet {
                out.push_str(&format!("```\n{}\n```\n", snippet));
            }

            if let Some(fix) = &f.suggested_fix {
                out.push_str(&format!("- **Recommendation**: {}\n", fix));
            }
            out.push('\n');
        }

        out
    }

    /// Renders structured JSON for automated pipelines.
    pub fn render_json(report: &ReviewReport) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(report)
    }
}
