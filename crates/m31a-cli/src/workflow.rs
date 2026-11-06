//! CLI workflow orchestration commands (`m31a workflow`).
//!
//! Provides CLI execution, inspection, resume, cancellation, and artifact management
//! for autonomous engineering workflows.

use anyhow::{Result, anyhow};
use clap::Subcommand;
use colored::Colorize;
use m31a_agent_engine::workflow::{WorkflowExecutionConfig, WorkflowOrchestrator};
use m31a_storage::Storage;
use std::path::PathBuf;

#[derive(Subcommand, Debug, Clone)]
pub enum WorkflowCommands {
    #[command(about = "Execute an autonomous workflow for an engineering objective")]
    Run {
        #[arg(help = "The engineering objective to plan, execute, and verify")]
        objective: String,

        #[arg(long = "json", help = "Output workflow progress and summary as JSON")]
        json: bool,

        #[arg(
            long = "no-speculate",
            help = "Disable speculative candidate execution"
        )]
        no_speculate: bool,
    },

    #[command(about = "Display current status of a workflow run")]
    Status {
        #[arg(help = "Workflow identifier")]
        workflow_id: String,

        #[arg(long = "json", help = "Output status as JSON")]
        json: bool,
    },

    #[command(about = "Inspect full details, task graph, and execution evidence for a workflow")]
    Inspect {
        #[arg(help = "Workflow identifier")]
        workflow_id: String,

        #[arg(long = "json", help = "Output inspection data as JSON")]
        json: bool,
    },

    #[command(about = "Resume an interrupted or paused workflow run")]
    Resume {
        #[arg(help = "Workflow identifier")]
        workflow_id: String,

        #[arg(long = "json", help = "Output result as JSON")]
        json: bool,
    },

    #[command(about = "Cancel an active workflow run")]
    Cancel {
        #[arg(help = "Workflow identifier")]
        workflow_id: String,

        #[arg(
            long = "reason",
            default_value = "User cancelled via CLI",
            help = "Cancellation rationale"
        )]
        reason: String,

        #[arg(long = "json", help = "Output result as JSON")]
        json: bool,
    },

    #[command(about = "List append-only audit event history for a workflow")]
    Events {
        #[arg(help = "Workflow identifier")]
        workflow_id: String,

        #[arg(long = "json", help = "Output events as JSON")]
        json: bool,
    },

    #[command(about = "List immutable content-addressed artifacts for a workflow")]
    Artifacts {
        #[arg(help = "Workflow identifier")]
        workflow_id: String,

        #[arg(long = "json", help = "Output artifacts as JSON")]
        json: bool,
    },
}

pub async fn run_workflow_mode(
    cwd: PathBuf,
    storage: Storage,
    cmd: WorkflowCommands,
) -> Result<()> {
    let orchestrator = WorkflowOrchestrator::new(storage, None, None, None, None);

    match cmd {
        WorkflowCommands::Run {
            objective,
            json,
            no_speculate,
        } => {
            if !json {
                println!(
                    "{} {}",
                    "⚡ AUTONOMOUS WORKFLOW ORCHESTRATOR".bold().bright_cyan(),
                    "(initiating planning and wave execution)".dimmed()
                );
                println!("  {} {}", "Objective: ".bold(), objective.bright_yellow());
                println!(
                    "  {} {}",
                    "Workspace: ".bold(),
                    cwd.display().to_string().cyan()
                );
                println!();
            }

            let mut cfg = WorkflowExecutionConfig::default();
            if no_speculate {
                cfg.allow_speculative_stages = false;
            }

            let run =
                orchestrator.create_workflow(&cwd, &objective, "autonomous", None, Some(cfg))?;
            if !json {
                println!(
                    "  {} {}",
                    "Workflow ID:".bold(),
                    run.id.bright_green().bold()
                );
                println!("{}", "  Generating execution plan and task DAG...".dimmed());
            }

            let plan = orchestrator.generate_plan(&run.id).await?;
            if !json {
                println!(
                    "  {} {} tasks across {} waves",
                    "Plan Revision:".bold(),
                    plan.tasks.len().to_string().cyan(),
                    plan.partition_into_waves()?.len().to_string().yellow()
                );
                println!("{}", "  Dispatching execution waves...".dimmed());
                println!();
            }

            let completed_run = orchestrator.execute_workflow(&run.id).await?;

            if json {
                println!("{}", serde_json::to_string_pretty(&completed_run)?);
            } else {
                let status_colored =
                    if completed_run.state == m31a_core::workflow::WorkflowState::Completed {
                        "COMPLETED".bright_green().bold()
                    } else {
                        format!("{:?}", completed_run.state).bright_red().bold()
                    };

                println!(
                    "{} Execution Finished: {}",
                    "🏆".bright_yellow(),
                    status_colored
                );
                if let Some(ref err) = completed_run.cancellation_reason {
                    println!("   {} {}", "Detail:".bold(), err.red());
                }
            }
        }

        WorkflowCommands::Status { workflow_id, json } => {
            let run = orchestrator
                .store()
                .get_workflow(&workflow_id)?
                .ok_or_else(|| anyhow!("Workflow '{workflow_id}' not found"))?;

            if json {
                println!("{}", serde_json::to_string_pretty(&run)?);
            } else {
                println!("{} {}", "Workflow:".bold(), run.id.bright_cyan().bold());
                println!(
                    "  {} {}",
                    "State:    ".bold(),
                    run.state.as_str().yellow().bold()
                );
                println!("  {} {}", "Objective:".bold(), run.objective);
                println!("  {} {}", "Workspace:".bold(), run.workspace_root.display());
                println!("  {} {}", "Revision: ".bold(), run.plan_revision);
            }
        }

        WorkflowCommands::Inspect { workflow_id, json } => {
            let run = orchestrator
                .store()
                .get_workflow(&workflow_id)?
                .ok_or_else(|| anyhow!("Workflow '{workflow_id}' not found"))?;
            let plan = orchestrator.store().get_workflow_plan(&workflow_id, None)?;
            let tasks = orchestrator.store().list_tasks(&workflow_id)?;
            let artifacts = orchestrator.store().list_artifacts(&workflow_id)?;

            if json {
                let val = serde_json::json!({
                    "workflow": run,
                    "plan": plan,
                    "tasks": tasks,
                    "artifacts": artifacts,
                });
                println!("{}", serde_json::to_string_pretty(&val)?);
            } else {
                println!(
                    "{} {}",
                    "═══ WORKFLOW INSPECTOR:".bold().bright_cyan(),
                    run.id.bright_yellow().bold()
                );
                println!(
                    "  {} {}",
                    "State:    ".bold(),
                    run.state.as_str().green().bold()
                );
                println!("  {} {}", "Objective:".bold(), run.objective);
                println!();
                println!("{}", "─── TASK DAG ───".dimmed());
                println!(
                    "{:<25} {:<15} {:<6} {:<12} {:<20}",
                    "TASK ID", "ROLE", "WAVE", "STATUS", "NAME"
                );
                for t in &tasks {
                    println!(
                        "{:<25} {:<15} {:<6} {:<12} {:<20}",
                        t.id,
                        t.assigned_role.cyan(),
                        t.wave_index,
                        t.status.as_str().yellow(),
                        t.name.bold()
                    );
                }
                println!();
                println!("{}", "─── ARTIFACTS ───".dimmed());
                for a in &artifacts {
                    println!(
                        "  • {} ({}) - {}",
                        a.name.bold(),
                        a.artifact_type.dimmed(),
                        a.file_path.display()
                    );
                }
            }
        }

        WorkflowCommands::Resume { workflow_id, json } => {
            if !json {
                println!(
                    "{} Reconciling and resuming workflow '{}'...",
                    "🔄".bright_cyan(),
                    workflow_id.bold()
                );
            }
            let run = orchestrator.resume_workflow(&workflow_id).await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&run)?);
            } else {
                println!(
                    "{} Workflow resume concluded with state: {}",
                    "✓".bright_green(),
                    run.state.as_str().bold()
                );
            }
        }

        WorkflowCommands::Cancel {
            workflow_id,
            reason,
            json,
        } => {
            orchestrator.cancel_workflow(&workflow_id, &reason)?;
            if json {
                println!(
                    "{}",
                    serde_json::json!({
                        "workflow_id": workflow_id,
                        "cancelled": true,
                        "reason": reason
                    })
                );
            } else {
                println!(
                    "{} Workflow '{}' cancelled. Reason: {}",
                    "🛑".bright_red(),
                    workflow_id.bold(),
                    reason.dimmed()
                );
            }
        }

        WorkflowCommands::Events { workflow_id, json } => {
            let events = orchestrator.store().list_events(&workflow_id, None)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&events)?);
            } else {
                println!(
                    "{} {}",
                    "═══ AUDIT EVENT HISTORY:".bold().bright_cyan(),
                    workflow_id.bright_yellow().bold()
                );
                for e in events {
                    println!(
                        "  [{}] {:<25} (task: {:?})",
                        chrono::DateTime::from_timestamp_millis(e.timestamp_epoch_ms)
                            .map(|d| d.format("%H:%M:%S").to_string())
                            .unwrap_or_default()
                            .dimmed(),
                        e.event_type.as_str().cyan().bold(),
                        e.task_id.as_deref().unwrap_or("-")
                    );
                }
            }
        }

        WorkflowCommands::Artifacts { workflow_id, json } => {
            let artifacts = orchestrator.store().list_artifacts(&workflow_id)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&artifacts)?);
            } else {
                println!(
                    "{} {}",
                    "═══ WORKFLOW ARTIFACTS:".bold().bright_cyan(),
                    workflow_id.bright_yellow().bold()
                );
                for a in artifacts {
                    println!(
                        "  • {:<25} [{}] ({} bytes) -> {}",
                        a.name.bold(),
                        a.artifact_type.cyan(),
                        a.size_bytes,
                        a.file_path.display()
                    );
                }
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_cli_workflow_commands_roundtrip() {
        let dir = tempdir().unwrap();
        let ws_root = dir.path().to_path_buf();
        let storage = Storage::open_in_memory().unwrap();

        // 1. Create a workflow via orchestrator directly first
        let orchestrator = WorkflowOrchestrator::new(storage.clone(), None, None, None, None);
        let run = orchestrator
            .create_workflow(
                &ws_root,
                "Build scalable caching layer",
                "autonomous",
                None,
                None,
            )
            .unwrap();

        let wf_id = run.id.clone();

        // 2. Status command
        let res_status = run_workflow_mode(
            ws_root.clone(),
            storage.clone(),
            WorkflowCommands::Status {
                workflow_id: wf_id.clone(),
                json: true,
            },
        )
        .await;
        assert!(res_status.is_ok());

        // 3. Inspect command
        let res_inspect = run_workflow_mode(
            ws_root.clone(),
            storage.clone(),
            WorkflowCommands::Inspect {
                workflow_id: wf_id.clone(),
                json: true,
            },
        )
        .await;
        assert!(res_inspect.is_ok());

        // 4. Events command
        let res_events = run_workflow_mode(
            ws_root.clone(),
            storage.clone(),
            WorkflowCommands::Events {
                workflow_id: wf_id.clone(),
                json: true,
            },
        )
        .await;
        assert!(res_events.is_ok());

        // 5. Artifacts command
        let res_artifacts = run_workflow_mode(
            ws_root.clone(),
            storage.clone(),
            WorkflowCommands::Artifacts {
                workflow_id: wf_id.clone(),
                json: true,
            },
        )
        .await;
        assert!(res_artifacts.is_ok());

        // 6. Cancel command
        let res_cancel = run_workflow_mode(
            ws_root.clone(),
            storage.clone(),
            WorkflowCommands::Cancel {
                workflow_id: wf_id.clone(),
                reason: "CLI test termination".to_string(),
                json: true,
            },
        )
        .await;
        assert!(res_cancel.is_ok());

        let cancelled_wf = orchestrator.store().get_workflow(&wf_id).unwrap().unwrap();
        assert_eq!(
            cancelled_wf.state,
            m31a_core::workflow::WorkflowState::Cancelled
        );
    }
}
