//! High-leverage Vibe Coding tools: Autonomous Visual TDD and Codebase Safety Gate.

use anyhow::Result;
use m31a_verify::{
    AutonomousVerifier, CodebaseSafetyGate, UiActionStep, UiAssertion, VisualTddScenario,
};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tracing::info;

/// Arguments for the `verify_visual_state` tool.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifyVisualStateArgs {
    pub scenario_name: String,
    pub url: String,
    #[serde(default)]
    pub viewport: Option<(u32, u32)>,
    #[serde(default)]
    pub actions: Vec<UiActionStep>,
    pub assertions: Vec<UiAssertion>,
}

/// Executes spec-driven Visual TDD against a web frontend.
pub async fn execute_verify_visual_state(args: &VerifyVisualStateArgs) -> Result<String> {
    info!(name = %args.scenario_name, url = %args.url, "Tool execute_verify_visual_state called");

    let scenario = VisualTddScenario {
        name: args.scenario_name.clone(),
        description: None,
        initial_url: args.url.clone(),
        viewport: args.viewport,
        actions: args.actions.clone(),
        assertions: args.assertions.clone(),
    };

    let report = AutonomousVerifier::verify_visual_state(&scenario, None, None).await?;

    let mut out = format!(
        "### Visual TDD Verification: {}\n\n- **Target URL**: {}\n- **Verdict**: {}\n- **Assertions**: {}/{} passed\n",
        report.scenario_name,
        report.initial_url,
        if report.passed { "PASSED" } else { "FAILED" },
        report.passed_assertions,
        report.total_assertions
    );

    if !report.assertion_outcomes.is_empty() {
        out.push_str("\n#### Assertion Outcomes:\n");
        for o in &report.assertion_outcomes {
            let status = if o.passed { "[PASS]" } else { "[FAIL]" };
            out.push_str(&format!("- {} {}\n", status, o.description));
            if !o.passed {
                out.push_str(&format!(
                    "  - Expected: {}\n  - Actual: {}\n",
                    o.expected, o.actual
                ));
                if let Some(ref d) = o.error_details {
                    out.push_str(&format!("  - Details: {}\n", d));
                }
            }
        }
    }

    if !report.console_errors.is_empty() {
        out.push_str("\n#### Browser Console Errors:\n");
        for err in &report.console_errors {
            out.push_str(&format!("- `{}`\n", err));
        }
    }

    if !report.network_errors.is_empty() {
        out.push_str("\n#### Network Failures:\n");
        for net in &report.network_errors {
            out.push_str(&format!("- `{}`\n", net));
        }
    }

    if !report.captured_screenshots.is_empty() {
        out.push_str("\n#### Screenshots Captured:\n");
        for shot in &report.captured_screenshots {
            out.push_str(&format!("- `{}`\n", shot.display()));
        }
    }

    if let Some(ref guidance) = report.self_healing_guidance {
        out.push_str(&format!(
            "\n#### Recommended Self-Healing Action:\n{}\n",
            guidance
        ));
    }

    Ok(out)
}

/// Arguments for the `audit_codebase_safety` tool.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditCodebaseSafetyArgs {
    #[serde(default)]
    pub modified_files: Vec<String>,
    #[serde(default)]
    pub candidate_symbols: Vec<String>,
}

/// Executes blast-radius impact and data-flow taint analysis on the current workspace.
pub async fn execute_audit_codebase_safety(
    cwd: &Path,
    args: &AuditCodebaseSafetyArgs,
) -> Result<String> {
    info!(cwd = ?cwd, "Tool execute_audit_codebase_safety called");

    let files: Vec<PathBuf> = args.modified_files.iter().map(PathBuf::from).collect();
    let report = CodebaseSafetyGate::audit_workspace_changes(cwd, &files, &args.candidate_symbols)?;

    let mut out = format!(
        "### Codebase Safety & Blast-Radius Audit\n\n- **Verdict**: {:?}\n- **Blast-Radius Score**: {:.1}/100 ({:?})\n- **Affected Downstream Files**: {}\n- **Taints Found**: {}\n- **Unconsumed Mutations**: {}\n\n",
        report.verdict,
        report.blast_radius.blast_radius_score,
        report.blast_radius.risk_level,
        report.blast_radius.total_affected_files,
        report.total_taints_found,
        report.unconsumed_mutations_count
    );

    if !report.blast_radius.impacts.is_empty() {
        out.push_str("#### Downstream Symbol Impact:\n");
        for impact in &report.blast_radius.impacts {
            out.push_str(&format!(
                "- Symbol `{}` in `{}`: {} direct caller(s), {} transitive dependent(s)\n",
                impact.target_symbol,
                impact.target_file.display(),
                impact.direct_callers_count,
                impact.transitive_dependents.len()
            ));
        }
    }

    if !report.recommended_test_commands.is_empty() {
        out.push_str("\n#### Recommended Verification Tests:\n");
        for cmd in &report.recommended_test_commands {
            out.push_str(&format!("- `{}`\n", cmd));
        }
    }

    Ok(out)
}
