//! Repository Intelligence CLI subcommands (`m31a repo ...`).

use anyhow::Result;
use clap::Subcommand;
use colored::*;
use m31a_core::SandboxPolicy;
use m31a_tools::ToolRegistry;
use serde_json::json;
use std::path::Path;

#[derive(Subcommand, Debug, Clone)]
pub enum RepoCommands {
    #[command(about = "Inspect repository indexing status, freshness, and parser diagnostics")]
    Status,

    #[command(about = "Index or re-index the repository")]
    Index {
        #[arg(
            short = 'f',
            long = "force",
            help = "Force full re-indexing from scratch"
        )]
        force: bool,
    },

    #[command(
        about = "Show repository overview including manifests, packages, languages, and symbol counts"
    )]
    Overview,

    #[command(about = "Search for symbols by name, qualified name, or kind")]
    Symbols {
        #[arg(help = "Symbol name or query pattern")]
        query: String,
        #[arg(
            short = 'k',
            long = "kind",
            help = "Filter by symbol kind (fn, struct, class, trait, enum, method)"
        )]
        kind: Option<String>,
        #[arg(
            short = 'l',
            long = "limit",
            default_value = "30",
            help = "Maximum results"
        )]
        limit: usize,
    },

    #[command(about = "Inspect direct and transitive dependency graphs")]
    Graph {
        #[arg(
            short = 't',
            long = "tier",
            default_value = "package",
            help = "Graph tier: package, module, or symbol"
        )]
        tier: String,
        #[arg(help = "Target package, module, or symbol name")]
        target: Option<String>,
        #[arg(
            short = 'd',
            long = "depth",
            default_value = "5",
            help = "Maximum traversal depth"
        )]
        depth: usize,
    },

    #[command(about = "Analyze change impact and blast radius for proposed file or symbol edits")]
    Impact {
        #[arg(required = true, help = "File paths to modify or analyze")]
        files: Vec<String>,
        #[arg(
            short = 's',
            long = "symbol",
            help = "Specific symbol names being edited"
        )]
        symbols: Vec<String>,
    },

    #[command(about = "Discover related tests exercising target symbol or file")]
    Tests {
        #[arg(short = 's', long = "symbol", help = "Target symbol")]
        symbol: Option<String>,
        #[arg(short = 'f', long = "file", help = "Target source file")]
        file: Option<String>,
    },

    #[command(about = "Generate architecture, modularity, and circular dependency report")]
    Architecture,

    #[command(about = "Retrieve task-aware, token-budgeted repository context packet")]
    Context {
        #[arg(help = "Task description, error message, or prompt")]
        task: String,
        #[arg(
            short = 'b',
            long = "budget",
            default_value = "3000",
            help = "Token budget"
        )]
        budget: usize,
    },
}

pub async fn run_repo_command(cmd: RepoCommands, json_output: bool, cwd: &Path) -> Result<()> {
    let registry = ToolRegistry::new();

    match cmd {
        RepoCommands::Status => {
            let res = registry
                .execute(
                    "repository_index_status",
                    "{}",
                    cwd,
                    SandboxPolicy::ReadOnly,
                )
                .await?;
            if json_output {
                println!("{}", res);
            } else {
                let val: serde_json::Value = serde_json::from_str(&res)?;
                println!("{}", "Repository Index Status".cyan().bold());
                println!(
                    "  Status:           {}",
                    val["status"].as_str().unwrap_or("unknown").green()
                );
                println!(
                    "  Root Path:        {}",
                    val["root_path"].as_str().unwrap_or("")
                );
                println!("  Files Indexed:    {}", val["files_indexed"]);
                println!("  Total Symbols:    {}", val["total_symbols"]);
                println!("  Relationships:    {}", val["total_relationships"]);
                println!("  Test Files:       {}", val["test_files_count"]);
                println!("  Diagnostics:      {}", val["diagnostics_count"]);
            }
        }
        RepoCommands::Index { force } => {
            let start = std::time::Instant::now();
            if !json_output {
                println!(
                    "{}",
                    format!(
                        "Indexing repository at '{}' (force: {})...",
                        cwd.display(),
                        force
                    )
                    .cyan()
                    .bold()
                );
            }
            let index = registry.get_or_build_repo_index(cwd, force).await;
            let elapsed = start.elapsed();
            if json_output {
                let out = json!({
                    "status": index.status(),
                    "files_indexed": index.files_indexed(),
                    "total_symbols": index.total_symbols(),
                    "relationships": index.all_relationships().len(),
                    "elapsed_ms": elapsed.as_millis(),
                });
                println!("{}", serde_json::to_string_pretty(&out)?);
            } else {
                println!(
                    "{} Index complete in {:.2?} ({} files, {} symbols, {} relationships)",
                    "[OK]".green().bold(),
                    elapsed,
                    index.files_indexed(),
                    index.total_symbols(),
                    index.all_relationships().len()
                );
            }
        }
        RepoCommands::Overview => {
            let res = registry
                .execute("repository_overview", "{}", cwd, SandboxPolicy::ReadOnly)
                .await?;
            if json_output {
                println!("{}", res);
            } else {
                let val: serde_json::Value = serde_json::from_str(&res)?;
                println!("{}", "Repository Architecture Overview".cyan().bold());
                println!(
                    "  Root:          {}",
                    val["root_path"].as_str().unwrap_or("")
                );
                println!("  Status:        {}", val["status"].as_str().unwrap_or(""));
                println!("  Files:         {}", val["files_indexed"]);
                println!("  Symbols:       {}", val["total_symbols"]);
                println!("  Relationships: {}", val["total_relationships"]);

                if let Some(pkgs) = val["packages"].as_array() {
                    if !pkgs.is_empty() {
                        println!(
                            "\n{}",
                            "Discovered Packages / Workspace Members:".yellow().bold()
                        );
                        for p in pkgs {
                            let name = p["name"].as_str().unwrap_or("");
                            let ver = p["version"].as_str().unwrap_or("0.0.0");
                            let lang = p["language"].as_str().unwrap_or("");
                            let path = p["manifest_path"].as_str().unwrap_or("");
                            println!(
                                "  - {:<24} {:<10} {:<10} ({})",
                                name.green(),
                                ver.dimmed(),
                                lang.cyan(),
                                path
                            );
                        }
                    }
                }

                if let Some(langs) = val["languages"].as_object() {
                    if !langs.is_empty() {
                        println!("\n{}", "Language Breakdown (Symbol Count):".yellow().bold());
                        for (lang, count) in langs {
                            println!("  - {:<16}: {}", lang.cyan(), count);
                        }
                    }
                }
            }
        }
        RepoCommands::Symbols { query, kind, limit } => {
            let args = json!({
                "query": query,
                "kind": kind,
                "limit": limit,
            });
            let res = registry
                .execute(
                    "symbol_search",
                    &args.to_string(),
                    cwd,
                    SandboxPolicy::ReadOnly,
                )
                .await?;
            if json_output {
                println!("{}", res);
            } else {
                let val: serde_json::Value = serde_json::from_str(&res)?;
                let total = val["total_found"].as_u64().unwrap_or(0);
                println!(
                    "{}",
                    format!("Symbol Search: '{}' (found {})", query, total)
                        .cyan()
                        .bold()
                );
                if let Some(symbols) = val["symbols"].as_array() {
                    for s in symbols {
                        let name = s["name"].as_str().unwrap_or("");
                        let kind = s["kind"].as_str().unwrap_or("");
                        let file = s["file_path"].as_str().unwrap_or("");
                        let lines = s["line_range"].as_array();
                        let line_str = lines
                            .map(|l| format!("{}-{}", l[0], l[1]))
                            .unwrap_or_default();
                        let sig = s["signature"].as_str().unwrap_or("");
                        println!(
                            "  - [{:<8}] {:<24} {}:{}",
                            kind.yellow(),
                            name.green().bold(),
                            file,
                            line_str
                        );
                        if !sig.is_empty() {
                            println!("      {}", sig.dimmed());
                        }
                    }
                }
            }
        }
        RepoCommands::Graph {
            tier,
            target,
            depth,
        } => {
            let args = json!({
                "tier": tier,
                "target": target,
                "max_depth": depth,
            });
            let res = registry
                .execute(
                    "dependency_graph",
                    &args.to_string(),
                    cwd,
                    SandboxPolicy::ReadOnly,
                )
                .await?;
            if json_output {
                println!("{}", res);
            } else {
                let val: serde_json::Value = serde_json::from_str(&res)?;
                println!("{}", format!("Dependency Graph ({})", tier).cyan().bold());
                match tier.as_str() {
                    "package" => {
                        if let Some(pkgs) = val["packages"].as_array() {
                            for p in pkgs {
                                let name = p["name"].as_str().unwrap_or("");
                                println!("  - Package: {}", name.green().bold());
                            }
                        }
                        if let Some(cycles) = val["cycles"].as_array() {
                            if cycles.is_empty() {
                                println!("  Cycles: {}", "None detected (Clean DAG)".green());
                            } else {
                                println!(
                                    "  Cycles: {}",
                                    format!(
                                        "{} circular dependency cycle(s) detected!",
                                        cycles.len()
                                    )
                                    .red()
                                    .bold()
                                );
                            }
                        }
                    }
                    "module" => {
                        println!("  Total Modules: {}", val["total_modules"]);
                        println!("  Cycles:        {}", val["cycles_detected"]);
                    }
                    _ => {
                        println!("  Target:        {}", val["target"]);
                        println!("  Callers Count: {}", val["callers_count"]);
                        println!("  Callees Count: {}", val["callees_count"]);
                    }
                }
            }
        }
        RepoCommands::Impact { files, symbols } => {
            let args = json!({
                "files": files,
                "symbols": symbols,
            });
            let res = registry
                .execute(
                    "analyze_change_impact",
                    &args.to_string(),
                    cwd,
                    SandboxPolicy::ReadOnly,
                )
                .await?;
            if json_output {
                println!("{}", res);
            } else {
                let val: serde_json::Value = serde_json::from_str(&res)?;
                let risk = val["risk_level"].as_str().unwrap_or("unknown");
                let risk_colored = match risk {
                    "critical" => risk.red().bold(),
                    "high" => risk.red(),
                    "medium" => risk.yellow(),
                    _ => risk.green(),
                };
                println!(
                    "{}",
                    "Semantic Blast Radius & Impact Analysis".cyan().bold()
                );
                println!("  Risk Level:        {}", risk_colored);
                println!(
                    "  Blast Score:       {:.2}",
                    val["blast_radius_score"].as_f64().unwrap_or(0.0)
                );
                println!("  Affected Files:    {}", val["total_affected_files"]);
                println!(
                    "  Summary:           {}",
                    val["summary"].as_str().unwrap_or("")
                );

                if let Some(tests) = val["recommended_test_targets"].as_array() {
                    if !tests.is_empty() {
                        println!("\n{}", "Recommended Verification Tests:".yellow().bold());
                        for t in tests {
                            println!("  - {}", t.as_str().unwrap_or("").green());
                        }
                    }
                }
            }
        }
        RepoCommands::Tests { symbol, file } => {
            let args = json!({
                "symbol": symbol,
                "file": file,
            });
            let res = registry
                .execute(
                    "related_tests",
                    &args.to_string(),
                    cwd,
                    SandboxPolicy::ReadOnly,
                )
                .await?;
            if json_output {
                println!("{}", res);
            } else {
                let val: serde_json::Value = serde_json::from_str(&res)?;
                if let Some(tests) = val.as_array() {
                    println!(
                        "{}",
                        format!("Related Tests (found {})", tests.len())
                            .cyan()
                            .bold()
                    );
                    for t in tests {
                        let name = t["test_name"].as_str().unwrap_or("");
                        let file = t["test_file"].as_str().unwrap_or("");
                        let line = t["line"].as_u64().unwrap_or(1);
                        let conf = t["confidence"].as_f64().unwrap_or(0.0);
                        let exp = t["explanation"].as_str().unwrap_or("");
                        println!(
                            "  - {:<28} {}:{} (confidence: {:.0}%)",
                            name.green().bold(),
                            file,
                            line,
                            conf * 100.0
                        );
                        println!("      {}", exp.dimmed());
                    }
                }
            }
        }
        RepoCommands::Architecture => {
            let res = registry
                .execute("architecture_report", "{}", cwd, SandboxPolicy::ReadOnly)
                .await?;
            if json_output {
                println!("{}", res);
            } else {
                let val: serde_json::Value = serde_json::from_str(&res)?;
                println!(
                    "{}",
                    "Repository Architecture & Modularity Report".cyan().bold()
                );
                println!("  Total Packages:       {}", val["total_packages"]);
                println!("  Total Modules:        {}", val["total_modules"]);
                println!("  Module Cycles:        {}", val["module_cycles_count"]);

                if let Some(high_i) = val["high_instability_modules"].as_array() {
                    if !high_i.is_empty() {
                        println!(
                            "\n{}",
                            "High Instability Modules (I > 0.8, high outgoing dependencies):"
                                .yellow()
                        );
                        for m in high_i {
                            println!("  - {}", m.as_str().unwrap_or(""));
                        }
                    }
                }

                if let Some(high_ca) = val["high_afferent_coupling_modules"].as_array() {
                    if !high_ca.is_empty() {
                        println!(
                            "\n{}",
                            "High Afferent Coupling Components (Core dependencies):".yellow()
                        );
                        for m in high_ca {
                            println!("  - {}", m.as_str().unwrap_or(""));
                        }
                    }
                }
            }
        }
        RepoCommands::Context { task, budget } => {
            let args = json!({
                "task": task,
                "token_budget": budget,
            });
            let res = registry
                .execute(
                    "retrieve_repository_context",
                    &args.to_string(),
                    cwd,
                    SandboxPolicy::ReadOnly,
                )
                .await?;
            println!("{}", res);
        }
    }

    Ok(())
}
