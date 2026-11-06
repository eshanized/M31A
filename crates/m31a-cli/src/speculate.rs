//! Speculative Parallel Reality CLI Subsystem (`m31a speculate`).
//!
//! Spawns competing solution branches in parallel isolated workspaces, runs automated
//! compiler verification, test suites, and blast-radius gates, and promotes the winning branch.

use anyhow::Result;
use colored::Colorize;
use m31a_agent_engine::speculative::{SpeculativeArena, SpeculativeBranchConfig};
use std::path::PathBuf;
use std::time::Duration;

pub struct SpeculateCliOptions {
    pub workspace_root: PathBuf,
    pub mission: String,
    pub hypotheses: Vec<String>,
    pub timeout_seconds: u64,
    pub auto_promote: bool,
}

pub async fn run_speculate_mode(opts: SpeculateCliOptions) -> Result<()> {
    println!(
        "{} {}",
        "🔀 SPECULATIVE PARALLEL REALITY ARENA".bold().bright_cyan(),
        "(evaluating competing hypotheses concurrently)"
            .to_string()
            .dimmed()
    );
    println!(
        "  {} {}",
        "Mission:   ".bold(),
        opts.mission.bright_yellow()
    );
    println!(
        "  {} {}",
        "Workspace: ".bold(),
        opts.workspace_root.display().to_string().cyan()
    );
    println!(
        "  {} {}",
        "Candidates:".bold(),
        opts.hypotheses.len().to_string().green().bold()
    );
    println!();

    let candidates: Vec<SpeculativeBranchConfig> = if opts.hypotheses.is_empty() {
        SpeculativeArena::generate_diverse_hypotheses(&opts.mission, 3)
    } else {
        opts.hypotheses
            .iter()
            .enumerate()
            .map(|(idx, hyp)| SpeculativeBranchConfig {
                branch_id: format!("branch-{}", idx + 1),
                name: format!("Hypothesis {}", idx + 1),
                hypothesis_prompt: hyp.clone(),
                objective: Some(opts.mission.clone()),
                model_override: None,
                patch_content: None,
            })
            .collect()
    };

    for (idx, c) in candidates.iter().enumerate() {
        println!(
            "  {}. {}",
            format!("[Branch {}]", idx + 1).magenta().bold(),
            c.hypothesis_prompt
        );
    }
    println!();
    println!(
        "{}",
        "Evaluating branches in parallel isolated sandboxes...".dimmed()
    );

    let report = SpeculativeArena::compete(
        &opts.workspace_root,
        &opts.mission,
        candidates,
        Duration::from_secs(opts.timeout_seconds),
        opts.auto_promote,
    )
    .await?;

    println!(
        "\n{}",
        "═══ SPECULATIVE EVALUATION MATRIX ═══".bright_cyan().bold()
    );
    println!(
        "{:<15} {:<10} {:<8} {:<8} {:<10} {:<12} {:<8}",
        "BRANCH", "STATE", "BUILD", "TESTS", "SAFETY", "BLAST RADIUS", "SCORE"
    );
    println!(
        "{}",
        "─────────────────────────────────────────────────────────────────────────────".dimmed()
    );

    for eval in &report.evaluations {
        let state_str = match eval.state {
            m31a_agent_engine::speculative::SpeculativeCandidateState::Eligible => {
                "ELIGIBLE".green()
            }
            m31a_agent_engine::speculative::SpeculativeCandidateState::Rejected => {
                "REJECTED".yellow()
            }
            m31a_agent_engine::speculative::SpeculativeCandidateState::Failed => "FAILED".red(),
            s => s.as_str().normal(),
        };
        let build_str = if eval.build_passed {
            "PASS".green()
        } else {
            "FAIL".red()
        };
        let test_str = if eval.tests_passed {
            "PASS".green()
        } else {
            "FAIL".red()
        };
        let score_str = format!("{:.1}", eval.composite_score)
            .bright_yellow()
            .bold();

        println!(
            "{:<15} {:<10} {:<8} {:<8} {:<10} {:<12} {:<8}",
            eval.name.bold(),
            state_str,
            build_str,
            test_str,
            eval.safety_verdict.cyan(),
            format!("{:.1}/100", eval.blast_radius_score),
            score_str
        );
    }

    println!();
    if let Some(ref winner) = report.winning_branch_name {
        println!(
            "{} Winning Branch: {} (Decision: {})",
            "🏆".bright_yellow(),
            winner.bright_green().bold(),
            report.decision_rationale.dimmed()
        );
        println!("   Status: {}", report.promotion_status.cyan());
    } else {
        println!(
            "{}",
            "No candidate met the verification thresholds.".yellow()
        );
    }

    Ok(())
}
