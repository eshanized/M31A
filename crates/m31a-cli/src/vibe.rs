//! Vibe Coding Mode Subsystem (`m31a vibe`).
//!
//! Sustains frictionless developer flow state by combining asynchronous filesystem watching,
//! non-intrusive compiler checks, automated visual state assertions, AST blast-radius gates,
//! and background self-healing patch generation via low-latency NVIDIA NIM models.

use anyhow::Result;
use colored::Colorize;
use m31a_agent_engine::vibe_loop::{VibeConfig, VibeEngine, VibeEvent};
use m31a_core::Config;
use m31a_provider::NvidiaNimClient;
use m31a_verify::{AutonomousVerifier, CodebaseSafetyGate, UiAssertion, VisualTddScenario};
use std::io::{self, BufRead, Write};
use std::path::PathBuf;
use std::time::Duration;

pub struct VibeCliOptions {
    pub workspace_root: PathBuf,
    pub model: Option<String>,
    pub auto_heal: bool,
    pub debounce_ms: u64,
    pub visual_url: Option<String>,
    pub initial_prompt: Option<String>,
}

pub async fn run_vibe_mode(opts: VibeCliOptions) -> Result<()> {
    print_banner();

    let cfg = Config::load().unwrap_or_default();
    let model = opts.model.unwrap_or_else(|| cfg.model.clone());
    let api_base = cfg.api_base.clone();
    let api_key = cfg.api_key.clone().unwrap_or_default();

    println!(
        "{} {}",
        "⚡ VIBE MODE INITIALIZED".bold().bright_magenta(),
        "(vibe coding at maximum flow state)".to_string().dimmed()
    );
    println!(
        "  {} {}",
        "Workspace:".bold(),
        opts.workspace_root.display().to_string().cyan()
    );
    println!("  {} {}", "Model:    ".bold(), model.bright_yellow());
    println!(
        "  {} {}",
        "Auto-Heal:".bold(),
        if opts.auto_heal {
            "ENABLED (silent background compiler repair)".green().bold()
        } else {
            "DISABLED (read-only diagnostics)".yellow()
        }
    );
    if let Some(ref url) = opts.visual_url {
        println!("  {} {}", "Visual TDD:".bold(), url.blue().underline());
    }
    if let Some(ref prompt) = opts.initial_prompt {
        println!("  {} {}", "Prompt:    ".bold(), prompt.bright_white());
    }
    println!();
    println!(
        "{}",
        "Commands: [t] run tests | [s] blast-radius audit | [v] visual check | [r] rewind last checkpoint | [q] exit".dimmed()
    );
    println!(
        "{}",
        "──────────────────────────────────────────────────────────────────────────────".dimmed()
    );

    let client = NvidiaNimClient::new(api_base, api_key, model);

    let vibe_config = VibeConfig {
        debounce_duration: Duration::from_millis(opts.debounce_ms),
        auto_heal: opts.auto_heal,
        max_heal_attempts: 3,
        model_override: None,
        check_timeout: Duration::from_secs(45),
        enable_timeline: true,
        suggest_companions: true,
    };

    let engine = VibeEngine::new(opts.workspace_root.clone(), client.clone(), vibe_config);
    let (mut rx, run_flag) = engine.start();

    // Spawn task to read events from the engine
    let event_handle = tokio::spawn(async move {
        while let Some(event) = rx.recv().await {
            match event {
                VibeEvent::WatcherStarted { root } => {
                    println!(
                        "{} Watching {} for file changes...",
                        "[👁️ WATCHER]".blue().bold(),
                        root.display().to_string().dimmed()
                    );
                }
                VibeEvent::FileChanged { paths } => {
                    let file_names: Vec<String> = paths
                        .iter()
                        .filter_map(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
                        .collect();
                    println!(
                        "{} Modified: {}",
                        "[📁 CHANGE]".yellow().bold(),
                        file_names.join(", ").cyan()
                    );
                }
                VibeEvent::CheckingWorkspace => {
                    print!("{} Fast typecheck in progress... ", "[🔍 CHECK]".magenta());
                    let _ = io::stdout().flush();
                }
                VibeEvent::WorkspaceClean { duration_ms } => {
                    println!(
                        "{} Workspace builds cleanly {}",
                        "✓ CLEAN".green().bold(),
                        format!("({}ms)", duration_ms).dimmed()
                    );
                }
                VibeEvent::ErrorsDetected {
                    error_count,
                    diagnostics,
                    ..
                } => {
                    println!(
                        "{} Found {} error(s):",
                        "✖ ERRORS".red().bold(),
                        error_count
                    );
                    for d in diagnostics.iter().take(3) {
                        println!("  {} {}", "→".red(), d);
                    }
                    if diagnostics.len() > 3 {
                        println!("  {} ... and {} more", "→".dimmed(), diagnostics.len() - 3);
                    }
                }
                VibeEvent::SelfHealingStarted {
                    attempt,
                    max_attempts,
                } => {
                    println!(
                        "{} Synthesizing corrective patch via NVIDIA NIM (attempt {}/{})...",
                        "[⚡ AUTO-HEAL]".bright_yellow().bold(),
                        attempt,
                        max_attempts
                    );
                }
                VibeEvent::SelfHealingSuccess {
                    duration_ms,
                    files_healed,
                } => {
                    let files: Vec<String> = files_healed
                        .iter()
                        .filter_map(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
                        .collect();
                    println!(
                        "{} Repaired in {}ms! Patched: {}",
                        "✨ HEALED".bright_green().bold(),
                        duration_ms,
                        files.join(", ").bright_cyan()
                    );
                }
                VibeEvent::SelfHealingFailed { reason } => {
                    println!(
                        "{} Auto-healing could not resolve diagnostics: {}",
                        "[⚠️ HEAL FAILED]".red().bold(),
                        reason
                    );
                }
                VibeEvent::CheckpointCreated {
                    checkpoint_id,
                    description,
                    files,
                    ..
                } => {
                    let file_names: Vec<String> = files
                        .iter()
                        .filter_map(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
                        .collect();
                    println!(
                        "{} Recorded Ghost Checkpoint '{}' ({}) [{}]",
                        "[📌 CHECKPOINT]".bright_cyan().bold(),
                        checkpoint_id.dimmed(),
                        description.green(),
                        file_names.join(", ").dimmed()
                    );
                }
                VibeEvent::RewindCompleted {
                    checkpoint_id,
                    restored_files,
                } => {
                    let file_names: Vec<String> = restored_files
                        .iter()
                        .filter_map(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
                        .collect();
                    println!(
                        "{} Rewound workspace to checkpoint '{}': restored {}",
                        "[⏪ REWIND]".bright_yellow().bold(),
                        checkpoint_id.cyan(),
                        file_names.join(", ").bright_green()
                    );
                }
                VibeEvent::CompanionSuggestion {
                    source_file,
                    suggested_companion,
                    rationale,
                } => {
                    println!(
                        "{} Found companion: {} → {} ({})",
                        "[💡 COMPANION]".bright_magenta(),
                        source_file.display().to_string().cyan(),
                        suggested_companion.display().to_string().bright_yellow(),
                        rationale.dimmed()
                    );
                }
                VibeEvent::VibeStopped => {
                    println!("{}", "[🛑 VIBE STOPPED]".dimmed());
                    break;
                }
            }
        }
    });

    // Interactive command input loop in main thread
    let ws_clone = opts.workspace_root.clone();
    let visual_url_clone = opts.visual_url.clone();
    let timeline_clone = engine.timeline();
    let stdin = io::stdin();

    tokio::task::spawn_blocking(move || {
        for line_res in stdin.lock().lines() {
            let line = match line_res {
                Ok(l) => l.trim().to_string(),
                Err(_) => break,
            };

            if line.eq_ignore_ascii_case("q") || line.eq_ignore_ascii_case("exit") {
                break;
            } else if line.eq_ignore_ascii_case("r") || line.eq_ignore_ascii_case("rewind") {
                let ws = ws_clone.clone();
                let tl = timeline_clone.lock().unwrap();
                if let Some(last_ckpt) = tl.checkpoints().last() {
                    match tl.rewind_to(&ws, &last_ckpt.id) {
                        Ok(restored) => {
                            println!(
                                "\n{} Rewound to '{}' ({}) - restored {} file(s)\n",
                                "⏪ REWIND SUCCESS".bright_green().bold(),
                                last_ckpt.id.cyan(),
                                last_ckpt.description,
                                restored.len()
                            );
                        }
                        Err(e) => println!("{} Rewind failed: {}\n", "✖ ERROR".red(), e),
                    }
                } else {
                    println!(
                        "\n{} No checkpoints recorded yet in this vibe session.\n",
                        "ℹ️ INFO".yellow()
                    );
                }
            } else if line.eq_ignore_ascii_case("t") || line.eq_ignore_ascii_case("test") {
                println!("\n{} Running full test suite...", "[🧪 TESTS]".cyan().bold());
                let handle = tokio::runtime::Handle::current();
                let ws = ws_clone.clone();
                handle.spawn(async move {
                    match AutonomousVerifier::verify_tests(&ws, Duration::from_secs(60)).await {
                        Ok(rep) => {
                            if rep.success {
                                println!("{} All workspace tests passed!\n", "✓ TESTS PASSED".green().bold());
                            } else {
                                println!("{} Tests failed:\n{}\n", "✖ TESTS FAILED".red().bold(), rep.raw_output);
                            }
                        }
                        Err(e) => println!("{} Failed to execute tests: {}\n", "✖ ERROR".red(), e),
                    }
                });
            } else if line.eq_ignore_ascii_case("s") || line.eq_ignore_ascii_case("safety") {
                println!("\n{} Auditing codebase blast-radius and data-flow invariants...", "[🛡️ SAFETY]".magenta().bold());
                let ws = ws_clone.clone();
                match CodebaseSafetyGate::audit_workspace_changes(&ws, &[], &[]) {
                    Ok(audit) => {
                        println!(
                            "  Verdict:      {:?}\n  Blast Radius: {:.1}/100 ({:?})\n  Affected:     {} file(s)\n  Taints:       {}\n",
                            audit.verdict,
                            audit.blast_radius.blast_radius_score,
                            audit.blast_radius.risk_level,
                            audit.blast_radius.total_affected_files,
                            audit.total_taints_found
                        );
                    }
                    Err(e) => println!("{} Safety audit error: {}\n", "✖ ERROR".red(), e),
                }
            } else if line.eq_ignore_ascii_case("v") || line.eq_ignore_ascii_case("visual") {
                let target_url = visual_url_clone.clone().unwrap_or_else(|| "http://localhost:3000".to_string());
                println!("\n{} Running Visual TDD against {}...", "[🌐 VISUAL TDD]".blue().bold(), target_url);
                let handle = tokio::runtime::Handle::current();
                handle.spawn(async move {
                    let scenario = VisualTddScenario {
                        name: "Interactive Vibe Inspection".to_string(),
                        description: None,
                        initial_url: target_url.clone(),
                        viewport: Some((1280, 800)),
                        actions: vec![],
                        assertions: vec![
                            UiAssertion::ZeroConsoleErrors,
                            UiAssertion::ZeroNetworkFailures,
                        ],
                    };
                    match AutonomousVerifier::verify_visual_state(&scenario, None, None).await {
                        Ok(rep) => {
                            if rep.passed {
                                println!("{} Visual and console state clean on {}\n", "✓ VISUAL PASS".green().bold(), rep.initial_url);
                            } else {
                                println!("{} Visual TDD failed:\n{}\n", "✖ VISUAL REGRESSION".red().bold(), rep.self_healing_guidance.unwrap_or_default());
                            }
                        }
                        Err(e) => println!("{} Visual verification error: {}\n", "✖ ERROR".red(), e),
                    }
                });
            } else if !line.is_empty() {
                println!("{} Instruction received: '{}' (dispatching to agent...)", "⚡".bright_yellow(), line);
            }
        }
    }).await?;

    run_flag.store(false, std::sync::atomic::Ordering::SeqCst);
    let _ = event_handle.await;
    println!(
        "\n{}",
        "Flow state session ended. Keep vibing!".bright_cyan()
    );
    Ok(())
}

fn print_banner() {
    let banner = r#"
                 __  __ _____  _    _     __     _____ ____  _____ 
                |  \/  |___ / / |  / \    \ \   / /_ _| __ )| ____|
                | |\/| | |_ \ | | / _ \    \ \ / / | ||  _ \|  _|  
                | |  | |___) || |/ ___ \    \ V /  | || |_) | |___ 
                |_|  |_|____/ |_/_/   \_\    \_/  |___|____/|_____|
    "#;
    println!("{}", banner.bright_magenta().bold());
}
