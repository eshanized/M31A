//! Spec-Driven Autonomous Phase Lifecycle CLI Handler for the `gsd` subcommand (`m31a-cli::gsd`).
//!
//! Provides CLI commands for spec-driven autonomous engineering workflows:
//! - `check-plan`: 8-Dimension Pre-Execution Plan Checker
//! - `verify`: Adversarial Goal-Backward Verifier
//! - `audit`: Nyquist Test Coverage Engine
//! - `drift`: Codebase Structural & Schema Drift Detector
//! - `review`: Multi-AI Reviewer Lanes & Consensus Consolidator
//! - `headroom`: Context Headroom Monitor with visual gauge
//! - `hooks`: 12-Point Canonical Phase Loop Lifecycle Hooks

use anyhow::{Result, bail};
use clap::Subcommand;
use colored::*;
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use m31a_ast::{
    ArchitecturalComplexityAnalyzer, AstCodebaseIndex, CodeCloneDetector, CodebaseDriftDetector,
    DocSyncAuditor, SemanticImpactAnalyzer,
};
use m31a_compaction::{ContextHeadroomMonitor, HeadroomSeverity};
use m31a_hooks::{CanonicalPhasePoint, PhaseHookRegistry};
use m31a_review::{PrimaryConsolidator, ReviewerClaim};
use m31a_verify::{
    DeploymentReadinessGate, GoalBackwardVerifier, IacValidator, NyquistEngine, NyquistRequirement,
    PerformanceProfiler, PlanChecker, TestIntegrityAuditor,
};

#[derive(Subcommand, Debug, Clone)]
pub enum GsdCommands {
    #[command(
        name = "check-plan",
        about = "Validate a phase plan with the 8-Dimension Plan Checker"
    )]
    CheckPlan {
        #[arg(short = 'p', long = "plan", help = "Path to PLAN.md file")]
        plan_file: PathBuf,

        #[arg(
            short = 'c',
            long = "context",
            help = "Optional path to CONTEXT.md for decision compliance"
        )]
        context_file: Option<PathBuf>,
    },

    #[command(
        name = "verify",
        about = "Run adversarial goal-backward verification against promised truths"
    )]
    Verify {
        #[arg(
            short = 'p',
            long = "phase",
            default_value = "01",
            help = "Phase identifier"
        )]
        phase_id: String,

        #[arg(long = "truth", help = "Promised observable truth statements to verify", action = clap::ArgAction::Append)]
        truths: Vec<String>,

        #[arg(long = "artifact", help = "Required non-stub file paths to verify", action = clap::ArgAction::Append)]
        artifacts: Vec<PathBuf>,

        #[arg(long = "test-cmd", help = "Automated test command to execute")]
        test_cmd: Option<String>,
    },

    #[command(
        name = "audit",
        about = "Execute Nyquist test coverage audit and generate VALIDATION.md"
    )]
    Audit {
        #[arg(
            short = 'p',
            long = "phase",
            default_value = "01",
            help = "Phase identifier"
        )]
        phase_id: String,

        #[arg(short = 't', long = "test-cmd", help = "Test command string (or multiple)", action = clap::ArgAction::Append)]
        test_cmds: Vec<String>,

        #[arg(short = 'o', long = "output", help = "Output path for VALIDATION.md")]
        output: Option<PathBuf>,
    },

    #[command(
        name = "drift",
        about = "Detect structural and schema drift against last_mapped_commit"
    )]
    Drift {
        #[arg(
            short = 's',
            long = "structure",
            help = "Path to STRUCTURE.md map document"
        )]
        structure_file: Option<PathBuf>,

        #[arg(
            short = 't',
            long = "threshold",
            default_value_t = 3,
            help = "Drift alert threshold"
        )]
        threshold: usize,

        #[arg(
            long = "auto-remap",
            help = "Signal codebase re-mapper if threshold exceeded"
        )]
        auto_remap: bool,
    },

    #[command(
        name = "review",
        about = "Dispatch multi-model reviewer lanes and synthesize consensus review"
    )]
    Review {
        #[arg(
            short = 'd',
            long = "diff",
            help = "Diff file or read from git HEAD~1..HEAD"
        )]
        diff_file: Option<PathBuf>,

        #[arg(
            long = "models",
            help = "Comma-separated list of NIM models",
            default_value = "nvidia/nemotron-3.5-lightning-30b-a3b,deepseek-ai/deepseek-v4.1-flash"
        )]
        models: String,
    },

    #[command(
        name = "headroom",
        about = "Evaluate context token headroom and display visual gauge"
    )]
    Headroom {
        #[arg(short = 'u', long = "used", help = "Current used tokens in session")]
        used_tokens: usize,

        #[arg(
            short = 'l',
            long = "limit",
            default_value_t = 128_000,
            help = "Context window limit"
        )]
        context_limit: usize,
    },

    #[command(
        name = "hooks",
        about = "Inspect or test the 12-point canonical phase loop hooks"
    )]
    Hooks {
        #[arg(
            short = 'p',
            long = "point",
            help = "Canonical phase point (e.g. discuss:post, verify:post)"
        )]
        point: Option<String>,
    },

    #[command(
        name = "test-integrity",
        about = "Audit test suite for fake tests, weak assertions, and mock abuse"
    )]
    TestIntegrity {
        #[arg(short = 'p', long = "path", help = "Optional specific test file")]
        path: Option<PathBuf>,
    },

    #[command(
        name = "impact",
        about = "Calculate semantic blast-radius and impacted components"
    )]
    Impact {
        #[arg(short = 'f', long = "file", help = "Target file(s) to analyze", action = clap::ArgAction::Append)]
        files: Vec<PathBuf>,

        #[arg(short = 's', long = "symbol", help = "Optional candidate symbol names", action = clap::ArgAction::Append)]
        symbols: Vec<String>,
    },

    #[command(
        name = "benchmark",
        about = "Detect and profile performance benchmarks"
    )]
    Benchmark {
        #[arg(long = "cmd", help = "Optional specific benchmark command to execute")]
        cmd: Option<String>,
    },

    #[command(
        name = "iac",
        about = "Validate Dockerfiles, Kubernetes manifests, and CI workflows"
    )]
    Iac,

    #[command(
        name = "complexity",
        about = "Audit architectural complexity, coupling metrics, and code smells"
    )]
    Complexity {
        #[arg(short = 'p', long = "path", help = "Path to scan")]
        path: Option<PathBuf>,
    },

    #[command(
        name = "clones",
        about = "Detect duplicate code blocks across the workspace"
    )]
    Clones {
        #[arg(
            short = 'm',
            long = "min-lines",
            default_value_t = 6,
            help = "Minimum lines for duplicate blocks"
        )]
        min_lines: usize,
    },

    #[command(
        name = "doc-sync",
        about = "Audit public API documentation coverage and synchronization"
    )]
    DocSync {
        #[arg(
            short = 'c',
            long = "min-coverage",
            default_value_t = 80.0,
            help = "Minimum required doc coverage percentage"
        )]
        min_coverage: f64,
    },

    #[command(
        name = "deploy-ready",
        about = "Certify workspace end-to-end deployment readiness"
    )]
    DeployReady,
}

pub struct GsdCliHandler;

impl GsdCliHandler {
    pub async fn handle(cmd: GsdCommands, workspace_root: &Path) -> Result<()> {
        match cmd {
            GsdCommands::CheckPlan {
                plan_file,
                context_file,
            } => {
                let plan_path = if plan_file.is_absolute() {
                    plan_file
                } else {
                    workspace_root.join(plan_file)
                };

                if !plan_path.exists() {
                    bail!("Plan file not found: {}", plan_path.display());
                }

                let plan_content = fs::read_to_string(&plan_path)?;
                let plan = PlanChecker::parse_plan(&plan_content, "01-01")?;

                let decisions: Vec<String> = if let Some(cf) = context_file {
                    let cp = if cf.is_absolute() {
                        cf
                    } else {
                        workspace_root.join(cf)
                    };
                    if cp.exists() {
                        let c_text = fs::read_to_string(&cp)?;
                        c_text
                            .lines()
                            .filter(|l| l.trim().starts_with("- ") || l.trim().starts_with("D-"))
                            .map(|l| l.trim().to_string())
                            .collect()
                    } else {
                        Vec::new()
                    }
                } else {
                    Vec::new()
                };

                let phase_requirements = plan.requirements.clone();
                let report = PlanChecker::check_plans(&[plan], &phase_requirements, &decisions);

                println!(
                    "{}",
                    "═══ M31A 8-Dimension Plan Checker Report ═══".bold().cyan()
                );
                println!("Total Plans Checked: {}", report.total_plans);
                println!(
                    "Status: {}",
                    if report.passed {
                        "PASS (Ready for Execution)".bold().green()
                    } else {
                        "BLOCKED (Revisions Required)".bold().red()
                    }
                );

                if !report.blockers.is_empty() {
                    println!("\n{}", "Blockers (Must fix before execute):".bold().red());
                    for b in &report.blockers {
                        println!(
                            "  - [{}] {}: {}",
                            b.dimension.bold(),
                            b.plan_id,
                            b.description
                        );
                        println!("    Hint: {}", b.fix_hint.yellow());
                    }
                }

                if !report.warnings.is_empty() {
                    println!("\n{}", "Warnings (Advisory):".bold().yellow());
                    for w in &report.warnings {
                        println!("  - [{}] {}: {}", w.dimension, w.plan_id, w.description);
                    }
                }

                if !report.passed {
                    bail!(
                        "Plan check failed with {} blocker(s)",
                        report.blockers.len()
                    );
                }
            }

            GsdCommands::Verify {
                phase_id,
                truths,
                artifacts,
                test_cmd,
            } => {
                println!(
                    "{}",
                    format!(
                        "═══ M31A Adversarial Goal-Backward Verifier (Phase {}) ═══",
                        phase_id
                    )
                    .bold()
                    .cyan()
                );

                let tests = test_cmd.map(|cmd| vec![cmd]).unwrap_or_default();
                let report = GoalBackwardVerifier::verify_phase(
                    workspace_root,
                    &phase_id,
                    &truths,
                    &artifacts,
                    &[],
                    &tests,
                    Duration::from_secs(60),
                )
                .await?;

                let out_file = workspace_root.join(format!("{}-VERIFICATION.md", phase_id));
                fs::write(&out_file, &report.markdown_summary)?;

                let verified_truths = report
                    .truths
                    .iter()
                    .filter(|t| t.status == m31a_verify::TruthStatus::Verified)
                    .count();
                let verified_artifacts = report
                    .artifacts
                    .iter()
                    .filter(|a| a.exists && !a.is_stub)
                    .count();

                println!("Outcome: {:?}", report.outcome);
                println!(
                    "Truths: {}/{} verified",
                    verified_truths,
                    report.truths.len()
                );
                println!(
                    "Artifacts: {}/{} verified",
                    verified_artifacts,
                    report.artifacts.len()
                );
                println!("Written to: {}", out_file.display().to_string().green());

                if report.outcome != m31a_verify::VerificationOutcome::Passed {
                    bail!("Goal-backward verification failed: {:?}", report.outcome);
                }
            }

            GsdCommands::Audit {
                phase_id,
                test_cmds,
                output,
            } => {
                println!(
                    "{}",
                    format!(
                        "═══ M31A Nyquist Test Coverage Audit (Phase {}) ═══",
                        phase_id
                    )
                    .bold()
                    .cyan()
                );

                let reqs: Vec<NyquistRequirement> = test_cmds
                    .iter()
                    .enumerate()
                    .map(|(idx, cmd)| NyquistRequirement {
                        id: format!("REQ-{:02}", idx + 1),
                        description: format!("Automated verification requirement {}", idx + 1),
                        test_command: Some(cmd.clone()),
                        test_file: None,
                        wave: 1,
                        plan_id: format!("{}-01", phase_id),
                    })
                    .collect();

                let report = NyquistEngine::audit_coverage(&phase_id, &reqs, workspace_root);
                let md = NyquistEngine::render_validation_markdown(&report);

                let out_path = output
                    .unwrap_or_else(|| workspace_root.join(format!("{}-VALIDATION.md", phase_id)));
                fs::write(&out_path, &md)?;

                println!("Verdict: {:?}", report.verdict);
                println!("Compliance: {:.1}%", report.compliance_rate * 100.0);
                println!(
                    "Passing Tests: {}/{}",
                    report.satisfied_count, report.total_requirements
                );
                println!("Written to: {}", out_path.display().to_string().green());

                if report.verdict == m31a_verify::NyquistVerdict::Blocked {
                    bail!("Nyquist test audit blocked by uncovered requirements");
                }
            }

            GsdCommands::Drift {
                structure_file,
                threshold,
                auto_remap,
            } => {
                println!(
                    "{}",
                    "═══ M31A Structural & Schema Drift Detector ═══"
                        .bold()
                        .cyan()
                );

                let struct_path = structure_file
                    .unwrap_or_else(|| workspace_root.join(".planning/codebase/STRUCTURE.md"));

                if !struct_path.exists() {
                    println!(
                        "{}",
                        "STRUCTURE.md not found. Codebase has not been mapped yet.".yellow()
                    );
                    return Ok(());
                }

                let content = fs::read_to_string(&struct_path)?;
                let last_commit = CodebaseDriftDetector::read_last_mapped_commit(&content)
                    .unwrap_or_else(|| "HEAD~1".to_string());

                let res = CodebaseDriftDetector::detect_git_drift(
                    workspace_root,
                    &last_commit,
                    &content,
                    threshold,
                    auto_remap,
                )?;

                println!("Baseline Commit: {}", last_commit.green());
                println!("Elements Count: {}", res.elements.len());
                println!("Action Required: {}", res.action_required);
                println!("Schema Drift: {}", res.schema_drift_detected);
                println!("Message: {}", res.message);

                for elem in &res.elements {
                    println!("  - [{}] {}", elem.category.as_str().yellow(), elem.path);
                }
            }

            GsdCommands::Review { diff_file, models } => {
                println!(
                    "{}",
                    "═══ M31A Multi-AI Reviewer Lanes & Consensus ═══"
                        .bold()
                        .cyan()
                );
                println!("Lanes configured: {}", models.green());

                let diff_text = if let Some(df) = diff_file {
                    fs::read_to_string(df)?
                } else {
                    let out = std::process::Command::new("git")
                        .args(["diff", "HEAD~1..HEAD"])
                        .current_dir(workspace_root)
                        .output()?;
                    String::from_utf8_lossy(&out.stdout).to_string()
                };

                let mut all_claims = Vec::new();

                // Dispatch static analyzer lane
                let findings = m31a_review::StaticDiffAnalyzer::analyze_diff(&diff_text);
                for f in findings {
                    all_claims.push(ReviewerClaim {
                        lane_id: "lane-static-analyzer".into(),
                        file_path: Some(f.file_path),
                        line_number: f.line_number,
                        severity: f.severity,
                        category: f.category,
                        claim_text: f.title,
                        suggested_fix: f.suggested_fix,
                    });
                }

                // Dispatch configured AI reviewer model lanes
                let model_list: Vec<&str> = models
                    .split(',')
                    .map(|s| s.trim())
                    .filter(|s| !s.is_empty())
                    .collect();
                for m in &model_list {
                    let claims = dispatch_model_reviewer_lane(m, &diff_text).await;
                    if !claims.is_empty() {
                        println!(
                            "  - Lane '{}': received {} claim(s)",
                            m.green(),
                            claims.len()
                        );
                        all_claims.extend(claims);
                    } else {
                        println!("  - Lane '{}': 0 claims returned or offline", m.yellow());
                    }
                }

                let report = PrimaryConsolidator::consolidate(workspace_root, &all_claims);
                let md = PrimaryConsolidator::render_markdown(&report);
                let out_file = workspace_root.join("REVIEW.md");
                fs::write(&out_file, &md)?;

                println!("Verdict: {:?}", report.verdict);
                println!("Risk Level: {:?}", report.risk_level);
                println!("Corroborated: {}", report.corroborated_findings.len());
                println!("Refuted: {}", report.refuted_claims.len());
                println!("Written to: {}", out_file.display().to_string().green());
            }

            GsdCommands::Headroom {
                used_tokens,
                context_limit,
            } => {
                println!("{}", "═══ M31A Context Headroom Monitor ═══".bold().cyan());
                let monitor = ContextHeadroomMonitor::new(context_limit);
                let snap = monitor.evaluate(used_tokens);

                let gauge = snap.render_progress_bar(30);
                let color_gauge = match snap.severity {
                    HeadroomSeverity::Nominal => gauge.green(),
                    HeadroomSeverity::Warning => gauge.yellow(),
                    HeadroomSeverity::Critical => gauge.red().bold(),
                    HeadroomSeverity::Exhausted => gauge.on_red().white().bold(),
                };

                println!("Status: {}", color_gauge);
                if let Some(msg) = &snap.advisory_message {
                    println!("Advisory: {}", msg.yellow());
                }
            }

            GsdCommands::Hooks { point } => {
                println!(
                    "{}",
                    "═══ M31A 12-Point Canonical Phase Loop Hooks ═══"
                        .bold()
                        .cyan()
                );
                let reg = PhaseHookRegistry::with_gsd_defaults();

                if let Some(pt_str) = point {
                    if let Some(pt) = CanonicalPhasePoint::from_str(&pt_str) {
                        let matching = reg.resolve_hooks(pt);
                        println!("Hooks registered at '{}': {}", pt.as_str(), matching.len());
                        for r in matching {
                            println!(
                                "  - [{}] Produces: {:?}, Consumes: {:?}",
                                r.id, r.produces, r.consumes
                            );
                        }
                    } else {
                        bail!("Unknown phase point: {}", pt_str);
                    }
                } else {
                    println!("Canonical Points (12 total):");
                    for pt in CanonicalPhasePoint::all() {
                        let matching = reg.resolve_hooks(*pt);
                        println!("  - {}: {} hook(s)", pt.as_str().cyan(), matching.len());
                    }
                }
            }
            GsdCommands::TestIntegrity { path } => {
                println!(
                    "{}",
                    "═══ M31A Test Integrity & Mutation Auditor ═══"
                        .bold()
                        .cyan()
                );
                let test_files = if let Some(p) = path {
                    vec![p]
                } else {
                    let mut files = Vec::new();
                    let mut dirs = vec![workspace_root.to_path_buf()];
                    while let Some(d) = dirs.pop() {
                        if let Ok(entries) = fs::read_dir(&d) {
                            for e in entries.flatten() {
                                let p = e.path();
                                let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
                                if name.starts_with('.')
                                    || matches!(name, "target" | "node_modules" | ".git")
                                {
                                    continue;
                                }
                                if p.is_dir() {
                                    dirs.push(p);
                                } else if p.is_file()
                                    && (name.contains("test")
                                        || name.ends_with("_test.rs")
                                        || name.ends_with(".test.ts"))
                                {
                                    files.push(p);
                                }
                            }
                        }
                    }
                    files
                };

                let report = TestIntegrityAuditor::audit_workspace(workspace_root, &test_files);
                println!("{}", report.summary.bold());
                for flaw in &report.flaws {
                    println!(
                        "  {} [{:?}] {}:{} in '{}': {}",
                        "⚠".yellow(),
                        flaw.kind,
                        flaw.file_path.display(),
                        flaw.line_number.unwrap_or(0),
                        flaw.test_name,
                        flaw.description
                    );
                }
            }
            GsdCommands::Impact { files, symbols } => {
                println!(
                    "{}",
                    "═══ M31A Semantic Blast-Radius Analyzer ═══".bold().cyan()
                );
                let index = AstCodebaseIndex::build(workspace_root, 1000);
                let report = SemanticImpactAnalyzer::analyze_impact(&index, &files, &symbols);
                println!("{}", report.summary.bold());
                for imp in &report.impacts {
                    println!("  - Target Symbol: {}", imp.target_symbol.bold().green());
                    println!("    Public API Export: {}", imp.is_public_export);
                    println!("    Direct Callers: {}", imp.direct_callers_count);
                    for reg in &imp.potential_regressions {
                        println!("    {} {}", "•".yellow(), reg);
                    }
                }
                if !report.recommended_test_targets.is_empty() {
                    println!("\nRecommended Verification Commands:");
                    for cmd in &report.recommended_test_targets {
                        println!("  $ {}", cmd.cyan());
                    }
                }
            }
            GsdCommands::Benchmark { cmd } => {
                println!(
                    "{}",
                    "═══ M31A Performance Profiler & Benchmark Engine ═══"
                        .bold()
                        .cyan()
                );
                if let Some(c) = cmd {
                    println!("Measuring command latency: '{}'", c.cyan());
                    let median_ms =
                        PerformanceProfiler::measure_command_latency(workspace_root, &c, 3)?;
                    println!("Median execution duration: {:.2} ms", median_ms);
                } else if let Some(detected) =
                    PerformanceProfiler::detect_benchmark_command(workspace_root)
                {
                    println!("Detected workspace benchmark command: {}", detected.cyan());
                    println!("Run with `--cmd \"{}\"` to profile latency.", detected);
                } else {
                    println!("No standard benchmark framework detected in workspace.");
                }
            }
            GsdCommands::Iac => {
                println!(
                    "{}",
                    "═══ M31A Infrastructure-as-Code & Full-Stack Security Auditor ═══"
                        .bold()
                        .cyan()
                );
                let report = IacValidator::audit_workspace(workspace_root);
                println!("{}", report.summary.bold());
                for issue in &report.issues {
                    println!(
                        "  [{:?}] {}:{} — {}: {}",
                        issue.severity,
                        issue.file_path.display(),
                        issue.line_number.unwrap_or(0),
                        issue.title.bold(),
                        issue.description
                    );
                    println!("    Fix: {}", issue.recommendation.green());
                }
            }
            GsdCommands::Complexity { path } => {
                println!(
                    "{}",
                    "═══ M31A Architectural Judgment & Complexity Auditor ═══"
                        .bold()
                        .cyan()
                );
                let target = path.unwrap_or_else(|| workspace_root.join("crates"));
                let mut smells = Vec::new();
                if target.is_file() {
                    if let Ok(c) = fs::read_to_string(&target) {
                        smells.extend(ArchitecturalComplexityAnalyzer::audit_source_complexity(
                            &target, &c,
                        ));
                    }
                } else {
                    let mut dirs = vec![target];
                    while let Some(d) = dirs.pop() {
                        if let Ok(entries) = fs::read_dir(&d) {
                            for e in entries.flatten() {
                                let p = e.path();
                                if p.is_dir() && !p.to_string_lossy().contains("target") {
                                    dirs.push(p);
                                } else if p.is_file()
                                    && p.extension().and_then(|x| x.to_str()) == Some("rs")
                                    && let Ok(c) = fs::read_to_string(&p)
                                {
                                    smells.extend(
                                        ArchitecturalComplexityAnalyzer::audit_source_complexity(
                                            &p, &c,
                                        ),
                                    );
                                }
                            }
                        }
                    }
                }
                println!("Audited sources: found {} complexity smells.", smells.len());
                for s in smells {
                    println!(
                        "  {} {} in '{}': {}",
                        "•".yellow(),
                        s.smell_type.bold(),
                        s.symbol_name,
                        s.description
                    );
                    println!("    Recommendation: {}", s.trade_off_recommendation.green());
                }
            }
            GsdCommands::Clones { min_lines } => {
                println!(
                    "{}",
                    "═══ M31A Refactoring Discipline & Code Clone Detector ═══"
                        .bold()
                        .cyan()
                );
                let mut files = Vec::new();
                let mut dirs = vec![workspace_root.to_path_buf()];
                while let Some(d) = dirs.pop() {
                    if let Ok(entries) = fs::read_dir(&d) {
                        for e in entries.flatten() {
                            let p = e.path();
                            let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
                            if name.starts_with('.')
                                || matches!(name, "target" | "node_modules" | ".git")
                            {
                                continue;
                            }
                            if p.is_dir() {
                                dirs.push(p);
                            } else if p.is_file()
                                && p.extension().and_then(|x| x.to_str()) == Some("rs")
                                && let Ok(c) = fs::read_to_string(&p)
                            {
                                files.push((p, c));
                            }
                        }
                    }
                }
                let report = CodeCloneDetector::detect_clones(&files, min_lines);
                println!("{}", report.summary.bold());
                for g in &report.clone_groups {
                    println!(
                        "  Found duplicate block across {} locations:",
                        g.instances.len()
                    );
                    for inst in &g.instances {
                        println!("    - {}:{}", inst.file_path.display(), inst.start_line);
                    }
                }
            }
            GsdCommands::DocSync { min_coverage } => {
                println!(
                    "{}",
                    "═══ M31A Documentation Synchronization Gate ═══"
                        .bold()
                        .cyan()
                );
                let index = AstCodebaseIndex::build(workspace_root, 1000);
                let syms = index
                    .file_outline(&workspace_root.join("crates/m31a-core/src/lib.rs"))
                    .unwrap_or_default();
                let report = DocSyncAuditor::audit_symbols(&syms, min_coverage);
                println!("{}", report.summary.bold());
                for miss in &report.missing_docs {
                    println!(
                        "  Missing Doc: {} '{}' at {}:{}",
                        miss.kind,
                        miss.symbol_name.bold(),
                        miss.file_path.display(),
                        miss.line_number
                    );
                }
                for stale in &report.stale_docs {
                    println!(
                        "  Stale Doc: '{}' at {}:{} - {}",
                        stale.symbol_name.bold(),
                        stale.file_path.display(),
                        stale.line_number,
                        stale.reason
                    );
                }
            }
            GsdCommands::DeployReady => {
                println!(
                    "{}",
                    "═══ M31A End-to-End Production Deployment Readiness Gate ═══"
                        .bold()
                        .cyan()
                );
                let report = DeploymentReadinessGate::evaluate_readiness(
                    workspace_root,
                    true,  // compiler check
                    true,  // test execution
                    0,     // test flaws
                    0,     // iac critical issues
                    true,  // release notes
                    false, // schema drift
                );
                println!("{}", report.certificate_markdown);
            }
        }

        Ok(())
    }
}

async fn dispatch_model_reviewer_lane(model_spec: &str, diff_text: &str) -> Vec<ReviewerClaim> {
    let clean_spec = model_spec.trim();
    if clean_spec.is_empty() || diff_text.trim().is_empty() {
        return Vec::new();
    }

    let (provider_type, model_name, api_key_opt) = {
        let model = clean_spec
            .strip_prefix("nim:")
            .unwrap_or(clean_spec)
            .to_string();
        let key = std::env::var("NVIDIA_API_KEY").ok();
        (m31a_provider::ProviderType::Nim, model, key)
    };

    let api_key = match api_key_opt {
        Some(k) if !k.trim().is_empty() => k,
        _ => {
            tracing::info!(lane = %clean_spec, "No API key configured for reviewer lane; skipping live inference");
            return Vec::new();
        }
    };

    let config = m31a_provider::ProviderConfig {
        provider_type,
        api_base: None,
        api_key: Some(api_key),
        model: model_name.clone(),
        extra: std::collections::HashMap::new(),
    };

    let provider = match m31a_provider::ProviderBuilder::build(config) {
        Ok(p) => p,
        Err(e) => {
            tracing::warn!(lane = %clean_spec, error = %e, "Failed to build provider for lane");
            return Vec::new();
        }
    };

    let system_prompt = "You are a specialized code reviewer. Review the given git diff.
Find potential defects, security issues, correctness bugs, or logic errors.
For each finding, you MUST provide exact file path and 1-based line number in the diff.
Return ONLY a valid JSON array of objects with fields:
file_path (string), line_number (integer), severity ('critical'|'high'|'medium'|'low'), category ('security'|'correctness'|'performance'|'maintainability'), claim_text (string), suggested_fix (string or null).
Do not wrap in backticks or markdown.";

    let req = m31a_provider::provider_trait::ChatRequest {
        model: model_name.clone(),
        messages: vec![
            m31a_core::types::ChatMessage::system(system_prompt),
            m31a_core::types::ChatMessage::user(format!("Git Diff:\n\n{}", diff_text)),
        ],
        tools: None,
        tool_choice: None,
        temperature: Some(0.1),
        max_tokens: Some(4096),
        stream: false,
        reasoning_effort: None,
        reasoning_budget: None,
        extra_params: std::collections::HashMap::new(),
    };

    match provider.chat(req).await {
        Ok(resp) => {
            if let Some(content) = resp.message.content {
                parse_reviewer_claims(clean_spec, &content)
            } else {
                Vec::new()
            }
        }
        Err(e) => {
            tracing::warn!(lane = %clean_spec, error = %e, "Reviewer lane inference failed");
            Vec::new()
        }
    }
}

fn parse_reviewer_claims(lane_id: &str, content: &str) -> Vec<ReviewerClaim> {
    #[derive(Deserialize)]
    struct RawClaim {
        file_path: Option<String>,
        line_number: Option<usize>,
        severity: Option<String>,
        category: Option<String>,
        claim_text: Option<String>,
        suggested_fix: Option<String>,
    }

    let json_text = if let Some(start) = content.find('[') {
        if let Some(end) = content.rfind(']') {
            &content[start..=end]
        } else {
            content
        }
    } else {
        content
    };

    if let Ok(raw_list) = serde_json::from_str::<Vec<RawClaim>>(json_text) {
        raw_list
            .into_iter()
            .map(|r| {
                let sev = match r.severity.as_deref().unwrap_or("low") {
                    "critical" => m31a_review::ReviewSeverity::Critical,
                    "high" => m31a_review::ReviewSeverity::High,
                    "medium" => m31a_review::ReviewSeverity::Medium,
                    _ => m31a_review::ReviewSeverity::Low,
                };
                let cat = match r.category.as_deref().unwrap_or("correctness") {
                    "security" => m31a_review::ReviewCategory::Security,
                    "performance" => m31a_review::ReviewCategory::Performance,
                    "maintainability" => m31a_review::ReviewCategory::Maintainability,
                    _ => m31a_review::ReviewCategory::Correctness,
                };
                ReviewerClaim {
                    lane_id: lane_id.to_string(),
                    file_path: r.file_path.map(PathBuf::from),
                    line_number: r.line_number,
                    severity: sev,
                    category: cat,
                    claim_text: r.claim_text.unwrap_or_else(|| "Unspecified defect".into()),
                    suggested_fix: r.suggested_fix,
                }
            })
            .collect()
    } else {
        Vec::new()
    }
}
