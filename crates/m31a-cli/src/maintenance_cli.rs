//! Maintenance CLI Subcommand Handlers (`m31a maintenance`).
//!
//! Exposes production CLI workflows for repository maintenance health inspection,
//! local scanning, deterministic triage, plan synthesis, verified execution,
//! approval management, and scheduling.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use anyhow::{Result, anyhow};
use clap::Subcommand;
use colored::*;
use serde_json::json;

use m31a_agent_engine::workflow::WorkflowOrchestrator;
use m31a_maintenance::{
    CaseStatus, LocalRepositorySignalSource, MaintenanceConfigFile, MaintenancePlanner,
    MaintenanceScheduler, MaintenanceStore, MaintenanceWorkflowRunner, PlanApprovalState,
    SignalIntakeManager, TriageEngine,
};
use m31a_storage::{RepoIndexStore, Storage};

#[derive(Subcommand, Debug, Clone)]
pub enum MaintenanceCommands {
    /// Overall repository maintenance health and active cases/schedules
    Status,
    /// Run immediate local repository scan
    Scan,
    /// Triage open maintenance cases
    Triage {
        #[arg(help = "Optional case ID to triage. If omitted, triages all open cases.")]
        case_id: Option<String>,
    },
    /// List maintenance cases
    Cases {
        #[arg(short = 's', long = "status", help = "Filter by case status")]
        status: Option<String>,
        #[arg(short = 'n', long = "limit", default_value = "50", help = "Max cases to list")]
        limit: usize,
    },
    /// Inspect details of a case, its signals, and plan
    Inspect {
        #[arg(help = "Case ID (e.g. case_...)")]
        case_id: String,
    },
    /// Generate or inspect a maintenance plan
    Plan {
        #[arg(help = "Case ID")]
        case_id: String,
    },
    /// Execute an approved or eligible maintenance plan
    Run {
        #[arg(help = "Case ID")]
        case_id: String,
        #[arg(long = "plan-id", help = "Specific plan ID. If omitted, uses active plan.")]
        plan_id: Option<String>,
    },
    /// List or manage scheduled maintenance jobs
    Schedule,
    /// Pause a maintenance schedule
    Pause {
        #[arg(help = "Schedule ID")]
        schedule_id: String,
    },
    /// Resume a paused maintenance schedule
    Resume {
        #[arg(help = "Schedule ID")]
        schedule_id: String,
    },
    /// Show historical executions and remediation attempts
    History {
        #[arg(short = 'c', long = "case-id", help = "Optional case ID filter")]
        case_id: Option<String>,
    },
    /// Generate a formatted repository maintenance report
    Report {
        #[arg(short = 'o', long = "output", help = "Optional file to write report")]
        output: Option<PathBuf>,
    },
    /// List configured maintenance signal sources and status
    Sources,
    /// Approve an awaiting-approval maintenance plan
    Approve {
        #[arg(help = "Plan ID (or Case ID)")]
        id: String,
    },
    /// Cancel an active maintenance run
    Cancel {
        #[arg(help = "Case ID (or Workflow ID)")]
        id: String,
        #[arg(
            short = 'r',
            long = "reason",
            default_value = "Cancelled by user via CLI",
            help = "Cancellation reason"
        )]
        reason: String,
    },
}

pub async fn run_maintenance_command(
    cmd: MaintenanceCommands,
    json_output: bool,
    cwd: &Path,
) -> Result<()> {
    let storage = Storage::open_default()?;
    let maint_store = MaintenanceStore::new(storage.connection());
    let repo_id = RepoIndexStore::derive_repo_id(cwd);
    let config = MaintenanceConfigFile::load_from_workspace(cwd).unwrap_or_default();
    let policy = config.to_policy();

    match cmd {
        MaintenanceCommands::Status => {
            let cases = maint_store.list_cases(Some(&repo_id), None, 100)?;
            let scheduler = MaintenanceScheduler::new(maint_store.clone(), policy.clone());
            let _ = scheduler.ensure_default_schedules(&repo_id);
            let schedules = maint_store.list_schedules(Some(&repo_id))?;

            let open_count = cases
                .iter()
                .filter(|c| !c.status.is_terminal())
                .count();
            let critical_count = cases
                .iter()
                .filter(|c| c.priority == m31a_maintenance::MaintenancePriority::Critical && !c.status.is_terminal())
                .count();
            let awaiting_approval = cases
                .iter()
                .filter(|c| c.status == CaseStatus::AwaitingApproval)
                .count();

            if json_output {
                let out = json!({
                    "repository_id": repo_id,
                    "workspace_root": cwd.display().to_string(),
                    "enabled": policy.enabled,
                    "autonomy_tier": policy.autonomy_tier.as_str(),
                    "open_cases": open_count,
                    "critical_cases": critical_count,
                    "awaiting_approval": awaiting_approval,
                    "total_tracked_cases": cases.len(),
                    "active_schedules": schedules.iter().filter(|s| s.is_enabled).count(),
                });
                println!("{}", serde_json::to_string_pretty(&out)?);
            } else {
                println!(
                    "{}",
                    format!("🛡️  M31A Autonomous Repository Maintenance — Status").cyan().bold()
                );
                println!("  Repository ID:    {}", repo_id.green());
                println!("  Workspace:        {}", cwd.display());
                println!("  Autonomy Tier:    {}", policy.autonomy_tier.as_str().yellow().bold());
                println!("  Open Cases:       {}", if open_count > 0 { open_count.to_string().yellow() } else { "0".green() });
                println!("  Critical Cases:   {}", if critical_count > 0 { critical_count.to_string().red().bold() } else { "0".green() });
                println!("  Awaiting Approval:{}", if awaiting_approval > 0 { awaiting_approval.to_string().bright_magenta().bold() } else { "0".green() });
                println!("  Active Schedules: {}", schedules.iter().filter(|s| s.is_enabled).count());
            }
        }

        MaintenanceCommands::Scan => {
            if !json_output {
                println!("{}", "🔍 Running local repository maintenance scanner...".yellow());
            }
            let scanner = LocalRepositorySignalSource::new(cwd);
            let signals = scanner.scan(&repo_id).await?;

            let intake = SignalIntakeManager::new(maint_store.clone());
            let mut created_count = 0;
            let mut updated_count = 0;
            let mut duplicate_count = 0;

            for sig in &signals {
                match intake.ingest_signal(sig)? {
                    m31a_maintenance::IngestionOutcome::CaseCreated { .. } => created_count += 1,
                    m31a_maintenance::IngestionOutcome::CaseUpdated { .. } => updated_count += 1,
                    m31a_maintenance::IngestionOutcome::Duplicate { .. } => duplicate_count += 1,
                }
            }

            if json_output {
                let out = json!({
                    "repository_id": repo_id,
                    "scanned_signals_count": signals.len(),
                    "cases_created": created_count,
                    "cases_updated": updated_count,
                    "signals_deduplicated": duplicate_count,
                });
                println!("{}", serde_json::to_string_pretty(&out)?);
            } else {
                println!(
                    "{}",
                    format!(
                        "Scan complete: {} signals evaluated. {} cases created, {} updated, {} duplicate.",
                        signals.len(),
                        created_count,
                        updated_count,
                        duplicate_count
                    )
                    .green()
                    .bold()
                );
            }
        }

        MaintenanceCommands::Triage { case_id } => {
            let triage_engine = TriageEngine::new(maint_store.clone(), policy.clone());
            if let Some(id) = case_id {
                let eval = triage_engine.triage_case(&id)?;
                if json_output {
                    println!("{}", serde_json::to_string_pretty(&eval)?);
                } else {
                    println!("{}", format!("Triage Evaluation for Case '{}':", id).cyan().bold());
                    println!("  Priority:    {}", eval.priority.as_str().yellow().bold());
                    println!("  Confidence:  {:.0}%", eval.confidence_score * 100.0);
                    println!("  Rationale:   {}", eval.rationale);
                }
            } else {
                let open_cases = maint_store.list_cases(Some(&repo_id), Some(CaseStatus::Open), 100)?;
                let mut evals = Vec::new();
                for c in open_cases {
                    if let Ok(eval) = triage_engine.triage_case(&c.id) {
                        evals.push(eval);
                    }
                }
                if json_output {
                    println!("{}", serde_json::to_string_pretty(&evals)?);
                } else {
                    println!("{}", format!("Triaged {} open maintenance cases.", evals.len()).green().bold());
                }
            }
        }

        MaintenanceCommands::Cases { status, limit } => {
            let filter_status = status.as_deref().and_then(CaseStatus::from_str);
            let cases = maint_store.list_cases(Some(&repo_id), filter_status, limit)?;

            if json_output {
                println!("{}", serde_json::to_string_pretty(&cases)?);
            } else {
                println!("{}", format!("Maintenance Cases for '{}' (Total: {}):", repo_id, cases.len()).cyan().bold());
                println!("{:<14} {:<10} {:<16} {:<24} {}", "ID", "PRIORITY", "STATUS", "CATEGORY", "TITLE");
                println!("{}", "-".repeat(80).dimmed());
                for c in cases {
                    let prio_color = match c.priority {
                        m31a_maintenance::MaintenancePriority::Critical => c.priority.as_str().red().bold(),
                        m31a_maintenance::MaintenancePriority::High => c.priority.as_str().yellow(),
                        _ => c.priority.as_str().normal(),
                    };
                    println!(
                        "{:<14} {:<10} {:<16} {:<24} {}",
                        c.id.green(),
                        prio_color,
                        c.status.as_str().cyan(),
                        c.category.as_str(),
                        c.title
                    );
                }
            }
        }

        MaintenanceCommands::Inspect { case_id } => {
            let case = maint_store
                .get_case(&case_id)?
                .ok_or_else(|| anyhow!("Case '{}' not found", case_id))?;
            let signals = maint_store.list_signals_for_case(&case_id)?;
            let plan = maint_store.get_plan_for_case(&case_id)?;
            let executions = maint_store.list_executions_for_case(&case_id)?;

            if json_output {
                let out = json!({
                    "case": case,
                    "signals": signals,
                    "plan": plan,
                    "executions": executions,
                });
                println!("{}", serde_json::to_string_pretty(&out)?);
            } else {
                println!("{}", format!("Case Inspection: {}", case.id).cyan().bold());
                println!("  Title:       {}", case.title);
                println!("  Category:    {}", case.category.as_str());
                println!("  Status:      {}", case.status.as_str().yellow().bold());
                println!("  Priority:    {}", case.priority.as_str());
                println!("  Problem:     {}", case.problem_statement);
                println!("  Signals:     {} linked signal(s)", signals.len());
                if let Some(p) = plan {
                    println!("  Plan:        {} ({}, hash: {})", p.id, p.selected_strategy, &p.plan_hash[..8]);
                    println!("  Requires Approval: {}", p.requires_approval);
                } else {
                    println!("  Plan:        None");
                }
                println!("  Executions:  {} attempt(s)", executions.len());
            }
        }

        MaintenanceCommands::Plan { case_id } => {
            let planner = MaintenancePlanner::new(maint_store.clone(), policy.clone());
            let baseline_sha = std::process::Command::new("git")
                .current_dir(cwd)
                .args(["rev-parse", "HEAD"])
                .output()
                .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
                .unwrap_or_else(|_| "HEAD".to_string());

            let plan = planner.generate_plan(&case_id, cwd, &baseline_sha)?;

            if json_output {
                println!("{}", serde_json::to_string_pretty(&plan)?);
            } else {
                println!("{}", format!("Maintenance Plan Generated: {}", plan.id).green().bold());
                println!("  Case ID:             {}", plan.case_id);
                println!("  Selected Strategy:   {}", plan.selected_strategy.cyan().bold());
                println!("  Baseline SHA:        {}", plan.baseline_sha);
                println!("  Plan Hash:           {}", plan.plan_hash);
                println!("  Estimated Risk:      {:?}", plan.estimated_risk);
                println!("  Required Autonomy:   {:?}", plan.required_autonomy_tier);
                println!("  Approval Required:   {}", plan.requires_approval);
            }
        }

        MaintenanceCommands::Run { case_id, plan_id } => {
            let target_plan_id = if let Some(pid) = plan_id {
                pid
            } else {
                let case = maint_store
                    .get_case(&case_id)?
                    .ok_or_else(|| anyhow!("Case '{}' not found", case_id))?;
                case.active_plan_id
                    .ok_or_else(|| anyhow!("Case '{}' has no active plan. Run `m31a maintenance plan {}` first.", case_id, case_id))?
            };

            let wf_store = storage.workflow_store();
            let orchestrator = Arc::new(WorkflowOrchestrator::new(storage.clone(), None, None, None, None));
            let runner = MaintenanceWorkflowRunner::new(maint_store.clone(), wf_store, orchestrator, policy.clone());

            let exec = runner.start_execution(&case_id, &target_plan_id, cwd).await?;

            if json_output {
                println!("{}", serde_json::to_string_pretty(&exec)?);
            } else {
                println!("{}", format!("Maintenance Execution Started!").green().bold());
                println!("  Execution ID: {}", exec.id);
                println!("  Workflow ID:  {}", exec.workflow_id.cyan().bold());
                println!("  Case ID:      {}", exec.case_id);
            }
        }

        MaintenanceCommands::Schedule => {
            let scheduler = MaintenanceScheduler::new(maint_store.clone(), policy.clone());
            let _ = scheduler.ensure_default_schedules(&repo_id);
            let schedules = maint_store.list_schedules(Some(&repo_id))?;

            if json_output {
                println!("{}", serde_json::to_string_pretty(&schedules)?);
            } else {
                println!("{}", format!("Maintenance Schedules for '{}':", repo_id).cyan().bold());
                println!("{:<24} {:<24} {:<10} {:<14}", "ID", "JOB", "ENABLED", "INTERVAL");
                println!("{}", "-".repeat(74).dimmed());
                for s in schedules {
                    println!(
                        "{:<24} {:<24} {:<10} {}s",
                        s.id,
                        s.job_kind,
                        if s.is_enabled { "true".green() } else { "false".red() },
                        s.cron_or_interval_secs
                    );
                }
            }
        }

        MaintenanceCommands::Pause { schedule_id } => {
            let scheduler = MaintenanceScheduler::new(maint_store, policy);
            scheduler.pause_schedule(&schedule_id)?;
            if json_output {
                println!("{}", json!({"status": "paused", "schedule_id": schedule_id}));
            } else {
                println!("{}", format!("Paused schedule '{}'", schedule_id).yellow());
            }
        }

        MaintenanceCommands::Resume { schedule_id } => {
            let scheduler = MaintenanceScheduler::new(maint_store, policy);
            scheduler.resume_schedule(&schedule_id)?;
            if json_output {
                println!("{}", json!({"status": "resumed", "schedule_id": schedule_id}));
            } else {
                println!("{}", format!("Resumed schedule '{}'", schedule_id).green());
            }
        }

        MaintenanceCommands::History { case_id } => {
            let cases = if let Some(cid) = case_id {
                let c = maint_store
                    .get_case(&cid)?
                    .ok_or_else(|| anyhow!("Case '{}' not found", cid))?;
                vec![c]
            } else {
                maint_store.list_cases(Some(&repo_id), None, 50)?
            };

            let mut all_attempts = Vec::new();
            for c in cases {
                for attempt in c.remediation_history {
                    all_attempts.push(json!({
                        "case_id": c.id,
                        "attempt": attempt,
                    }));
                }
            }

            if json_output {
                println!("{}", serde_json::to_string_pretty(&all_attempts)?);
            } else {
                println!("{}", format!("Remediation History ({} attempts):", all_attempts.len()).cyan().bold());
                for item in all_attempts {
                    println!("  - {}", item);
                }
            }
        }

        MaintenanceCommands::Report { output } => {
            let cases = maint_store.list_cases(Some(&repo_id), None, 100)?;
            let mut report = String::new();
            report.push_str(&format!("# Autonomous Maintenance Report: {}\n\n", repo_id));
            report.push_str(&format!("Generated at: {}\n\n", chrono::Utc::now().to_rfc3339()));
            report.push_str("## Summary\n");
            report.push_str(&format!("- **Total Cases:** {}\n", cases.len()));
            report.push_str(&format!("- **Open Cases:** {}\n", cases.iter().filter(|c| !c.status.is_terminal()).count()));
            report.push_str(&format!("- **Resolved Cases:** {}\n\n", cases.iter().filter(|c| c.status == CaseStatus::Resolved).count()));

            report.push_str("## Active Cases\n");
            for c in cases.iter().filter(|c| !c.status.is_terminal()) {
                report.push_str(&format!("- **{}** (`{}`): {} [Priority: {:?}]\n", c.id, c.category.as_str(), c.title, c.priority));
            }

            if let Some(out_path) = output {
                fs::write(&out_path, &report)?;
                if !json_output {
                    println!("{}", format!("Report written to {}", out_path.display()).green().bold());
                }
            } else if json_output {
                println!("{}", json!({"report_markdown": report}));
            } else {
                println!("{}", report);
            }
        }

        MaintenanceCommands::Sources => {
            let sources = json!([
                {
                    "provider": "local_analysis",
                    "status": "active",
                    "capabilities": ["compiler_diagnostics", "semver_drift", "doc_sync", "architecture_smells"]
                },
                {
                    "provider": "github",
                    "status": if config.sources.github_repository.is_some() { "configured" } else { "standby" },
                    "repository": config.sources.github_repository,
                    "capabilities": ["webhooks", "workflow_runs", "issues", "dependabot", "security_advisories"]
                },
                {
                    "provider": "vibe_loop",
                    "status": "integrated",
                    "capabilities": ["fast_change_detection", "healing_escalation"]
                }
            ]);

            if json_output {
                println!("{}", serde_json::to_string_pretty(&sources)?);
            } else {
                println!("{}", "Configured Maintenance Signal Sources:".cyan().bold());
                println!("  1. Local Analysis:   Active (zero-credential local scanner)");
                println!("  2. GitHub Provider:  {}", if config.sources.github_repository.is_some() { "Configured".green() } else { "Standby (configure in .m31a/maintenance.toml)".dimmed() });
                println!("  3. Vibe Loop:        Integrated (verified execution & escalation)");
            }
        }

        MaintenanceCommands::Approve { id } => {
            let mut plan_opt = maint_store.get_plan(&id)?;
            if plan_opt.is_none() {
                // Try looking up as case ID
                if let Some(case) = maint_store.get_case(&id)? {
                    if let Some(ref pid) = case.active_plan_id {
                        plan_opt = maint_store.get_plan(pid)?;
                    }
                }
            }

            let mut plan = plan_opt.ok_or_else(|| anyhow!("Plan or case '{}' not found", id))?;
            plan.approval_state = PlanApprovalState::Approved;
            maint_store.update_plan_approval(&plan.id, PlanApprovalState::Approved)?;

            // Transition case to PlanReady
            if let Some(mut case) = maint_store.get_case(&plan.case_id)? {
                let _ = case.transition_to(CaseStatus::PlanReady);
                maint_store.update_case(&case)?;
            }

            if json_output {
                println!("{}", json!({"status": "approved", "plan_id": plan.id}));
            } else {
                println!("{}", format!("Maintenance plan '{}' approved successfully!", plan.id).green().bold());
            }
        }

        MaintenanceCommands::Cancel { id, reason } => {
            if let Some(mut case) = maint_store.get_case(&id)? {
                let _ = case.transition_to(CaseStatus::Cancelled);
                maint_store.update_case(&case)?;
                if let Some(ref wf_id) = case.workflow_id {
                    let orch = WorkflowOrchestrator::new(storage.clone(), None, None, None, None);
                    let _ = orch.cancel_workflow(wf_id, &reason).await;
                }
            } else {
                // Cancel workflow directly
                let orch = WorkflowOrchestrator::new(storage.clone(), None, None, None, None);
                let _ = orch.cancel_workflow(&id, &reason).await;
            }

            if json_output {
                println!("{}", json!({"status": "cancelled", "id": id, "reason": reason}));
            } else {
                println!("{}", format!("Maintenance item '{}' cancelled: {}", id, reason).yellow().bold());
            }
        }
    }

    Ok(())
}
