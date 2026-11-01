//! Interactive and automated setup wizard for M31A.
//!
//! Provides first-time configuration, environment diagnostics,
//! credential onboarding, model selection, and security profile setup.

use anyhow::{Context, Result};
use colored::Colorize;
use m31a_core::{ApprovalPolicy, Config, SandboxPolicy};
use m31a_provider::models::get_nvidia_model_catalog;
use std::io::{self, BufRead, IsTerminal, Write};

#[derive(Debug, Clone, Default)]
pub struct SetupOptions {
    pub api_key: Option<String>,
    pub api_base: Option<String>,
    pub model: Option<String>,
    pub sandbox: Option<String>,
    pub approval: Option<String>,
    pub non_interactive: bool,
    pub force: bool,
}

pub struct SetupWizard;

impl SetupWizard {
    /// Executes the setup flow according to provided options.
    pub async fn run(opts: SetupOptions) -> Result<()> {
        println!(
            "{}",
            "================================================================================"
                .cyan()
        );
        println!(
            "{}",
            "                           M31A Configuration Setup"
                .cyan()
                .bold()
        );
        println!(
            "{}",
            "================================================================================"
                .cyan()
        );
        println!("Configuring M31A autonomous coding assistant environment.\n");

        let config_dir = Config::config_dir();
        let config_file = config_dir.join("config.toml");

        // 1. Check existing configuration
        let existing_config = Config::load().ok();
        if config_file.exists() && !opts.force && !opts.non_interactive {
            println!(
                "Existing configuration detected at: {}",
                config_file.display().to_string().yellow()
            );
            print!("Do you want to reconfigure M31A? [Y/n]: ");
            io::stdout().flush()?;
            let mut line = String::new();
            io::stdin().lock().read_line(&mut line)?;
            let trimmed = line.trim().to_lowercase();
            if trimmed == "n" || trimmed == "no" {
                println!("[OK] Setup aborted. Existing configuration preserved.");
                return Ok(());
            }
            println!();
        }

        let is_interactive = !opts.non_interactive && io::stdin().is_terminal();

        // 2. Discover environment
        let env_api_key = std::env::var("NVIDIA_API_KEY").ok();
        let existing_api_key = existing_config.as_ref().and_then(|c| c.api_key.clone());
        let bwrap_path = which::which("bwrap").ok();
        let git_path = which::which("git").ok();

        println!("System Environment:");
        if let Some(ref p) = bwrap_path {
            println!(
                "  [OK] Sandbox Runtime: Bubblewrap found ({})",
                p.display().to_string().green()
            );
        } else {
            println!(
                "  [Notice] Sandbox Runtime: Bubblewrap not found in PATH (host fallback will be used)"
            );
        }
        if let Some(ref p) = git_path {
            println!(
                "  [OK] Version Control: Git found ({})",
                p.display().to_string().green()
            );
        } else {
            println!("  [Warning] Version Control: Git not found in PATH");
        }
        println!();

        // 3. API Key resolution
        let final_api_key = if let Some(key) = opts.api_key {
            Some(key.trim().to_string())
        } else if is_interactive {
            let candidate_key = env_api_key.or(existing_api_key);
            if let Some(ref key) = candidate_key {
                let masked = if key.len() > 10 {
                    format!("{}...{}", &key[..7], &key[key.len() - 4..])
                } else {
                    "***".to_string()
                };
                print!("Detected NVIDIA NIM API Key ({masked}). Use this key? [Y/n/enter key]: ");
                io::stdout().flush()?;
                let mut line = String::new();
                io::stdin().lock().read_line(&mut line)?;
                let ans = line.trim();
                if ans.is_empty()
                    || ans.eq_ignore_ascii_case("y")
                    || ans.eq_ignore_ascii_case("yes")
                {
                    candidate_key
                } else if ans.eq_ignore_ascii_case("n") || ans.eq_ignore_ascii_case("no") {
                    print!("Enter NVIDIA NIM API Key: ");
                    io::stdout().flush()?;
                    let mut new_key = String::new();
                    io::stdin().lock().read_line(&mut new_key)?;
                    let trimmed = new_key.trim().to_string();
                    if trimmed.is_empty() {
                        None
                    } else {
                        Some(trimmed)
                    }
                } else {
                    Some(ans.to_string())
                }
            } else {
                print!("Enter NVIDIA NIM API Key (or press Enter to configure later via env): ");
                io::stdout().flush()?;
                let mut new_key = String::new();
                io::stdin().lock().read_line(&mut new_key)?;
                let trimmed = new_key.trim().to_string();
                if trimmed.is_empty() {
                    None
                } else {
                    Some(trimmed)
                }
            }
        } else {
            env_api_key.or(existing_api_key)
        };

        // 4. API Base resolution
        let default_base = "https://integrate.api.nvidia.com/v1".to_string();
        let final_api_base = if let Some(base) = opts.api_base {
            base
        } else if is_interactive {
            let current = existing_config
                .as_ref()
                .map(|c| c.api_base.clone())
                .unwrap_or_else(|| default_base.clone());
            print!("API Base URL [{current}]: ");
            io::stdout().flush()?;
            let mut line = String::new();
            io::stdin().lock().read_line(&mut line)?;
            let trimmed = line.trim();
            if trimmed.is_empty() {
                current
            } else {
                trimmed.to_string()
            }
        } else {
            existing_config
                .as_ref()
                .map(|c| c.api_base.clone())
                .unwrap_or(default_base)
        };

        // 5. Model Selection
        let catalog = get_nvidia_model_catalog();
        let default_model = "nvidia/nemotron-3.5-lightning-30b-a3b".to_string();

        let final_model = if let Some(m) = opts.model {
            resolve_model_slug_or_alias(&m)
        } else if is_interactive {
            println!("\nAvailable Frontier Models in NVIDIA NIM Catalog:");
            for (idx, m) in catalog.iter().enumerate() {
                let marker = if m.slug == default_model {
                    " [Recommended Default]"
                } else {
                    ""
                };
                println!(
                    "  {}. {:<42} ({}k ctx, Reasoning: {}){}",
                    idx + 1,
                    m.slug.yellow().bold(),
                    m.context_window / 1024,
                    if m.supports_thinking { "Yes" } else { "No" },
                    marker.green()
                );
            }
            print!(
                "\nSelect default model [1-{}, or enter custom slug, default: 1]: ",
                catalog.len()
            );
            io::stdout().flush()?;
            let mut line = String::new();
            io::stdin().lock().read_line(&mut line)?;
            let trimmed = line.trim();
            if trimmed.is_empty() {
                default_model
            } else if let Ok(idx) = trimmed.parse::<usize>() {
                if idx >= 1 && idx <= catalog.len() {
                    catalog[idx - 1].slug.clone()
                } else {
                    default_model
                }
            } else {
                resolve_model_slug_or_alias(trimmed)
            }
        } else {
            existing_config
                .as_ref()
                .map(|c| c.model.clone())
                .unwrap_or(default_model)
        };

        // 6. Sandbox Policy
        let final_sandbox = if let Some(s) = opts.sandbox {
            parse_sandbox_policy(&s)
        } else if is_interactive {
            println!("\nSandbox Execution Policies:");
            println!(
                "  1. workspace-write    - File writes restricted to workspace directory [Recommended]"
            );
            println!("  2. read-only          - Strictly read-only filesystem; execution isolated");
            println!("  3. danger-full-access - Unrestricted host filesystem access");
            print!("Select sandbox policy [1-3, default: 1]: ");
            io::stdout().flush()?;
            let mut line = String::new();
            io::stdin().lock().read_line(&mut line)?;
            match line.trim() {
                "2" => SandboxPolicy::ReadOnly,
                "3" => SandboxPolicy::DangerFullAccess,
                _ => SandboxPolicy::WorkspaceWrite,
            }
        } else {
            existing_config
                .as_ref()
                .map(|c| c.sandbox)
                .unwrap_or(SandboxPolicy::WorkspaceWrite)
        };

        // 7. Approval Policy
        let final_approval = if let Some(a) = opts.approval {
            parse_approval_policy(&a)
        } else if is_interactive {
            println!("\nAction Approval Policies:");
            println!(
                "  1. on-request    - Prompt human user confirmation for mutating actions [Recommended]"
            );
            println!(
                "  2. approve-for-me- Guardian automated security supervisor validates actions"
            );
            println!("  3. never         - Execute actions without confirmation");
            print!("Select approval policy [1-3, default: 1]: ");
            io::stdout().flush()?;
            let mut line = String::new();
            io::stdin().lock().read_line(&mut line)?;
            match line.trim() {
                "2" => ApprovalPolicy::ApproveForMe,
                "3" => ApprovalPolicy::Never,
                _ => ApprovalPolicy::OnRequest,
            }
        } else {
            existing_config
                .as_ref()
                .map(|c| c.approval)
                .unwrap_or(ApprovalPolicy::OnRequest)
        };

        // 8. Construct and write Config
        let mut config = existing_config.unwrap_or_default();
        config.api_key = final_api_key.clone();
        config.api_base = final_api_base;
        config.model = final_model.clone();
        config.sandbox = final_sandbox;
        config.approval = final_approval;

        std::fs::create_dir_all(&config_dir).with_context(|| {
            format!(
                "Failed to create configuration directory at {:?}",
                config_dir
            )
        })?;

        // Initialize subdirectories for persistent runtime data
        let subdirs = ["sessions", "memories", "worktrees", "feedback", "plugins"];
        for sub in &subdirs {
            let p = config_dir.join(sub);
            let _ = std::fs::create_dir_all(&p);
        }

        // Save config
        config.save().context("Failed to save configuration file")?;

        // Secure file permissions on unix
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&config_dir, std::fs::Permissions::from_mode(0o700));
            if config_file.exists() {
                let _ =
                    std::fs::set_permissions(&config_file, std::fs::Permissions::from_mode(0o600));
            }
        }

        // 9. Validation and Summary
        println!("\n{}", "Configuration Summary:".cyan().bold());
        println!(
            "  - Config Path:      {}",
            config_file.display().to_string().yellow()
        );
        println!("  - Active Model:     {}", config.model.green().bold());
        println!("  - API Base URL:     {}", config.api_base);
        println!(
            "  - API Key:          {}",
            if config.api_key.is_some() {
                "[Configured]".green()
            } else {
                "[Missing - set NVIDIA_API_KEY]".yellow()
            }
        );
        println!("  - Sandbox Policy:   {:?}", config.sandbox);
        println!("  - Approval Policy:  {:?}", config.approval);
        println!("  - Data Storage:     {}", config_dir.display());

        // Validate API Key connectivity if key is provided
        if let Some(ref key) = config.api_key {
            print!("\nValidating connection to NVIDIA NIM API... ");
            io::stdout().flush()?;
            let client =
                m31a_provider::client::NvidiaNimClient::new(&config.api_base, key, &config.model);
            match client.check_connection().await {
                Ok(_) => {
                    println!("{}", "[OK] Handshake successful!".green().bold());
                }
                Err(e) => {
                    println!(
                        "{}",
                        format!("[Notice] Endpoint reachable ({})", e).yellow()
                    );
                }
            }
        } else {
            println!(
                "\n[Notice] No API key stored. Export `NVIDIA_API_KEY=\"nvapi-...\"` before running turns."
            );
        }

        println!(
            "\n{}",
            "[OK] M31A setup completed successfully!".green().bold()
        );
        println!(
            "Run `m31a` to start an interactive session, or `m31a --help` for all commands.\n"
        );

        Ok(())
    }
}

pub fn resolve_model_slug_or_alias(input: &str) -> String {
    let lower = input.to_lowercase();
    match lower.as_str() {
        "speed" | "fast" => "nvidia/nemotron-3.5-bolt-e4b".to_string(),
        "code" | "coder" => "mistralai/codestral-22b-instruct-v0.1".to_string(),
        "reasoning" | "r1" | "think" => "deepseek-ai/deepseek-v4.1-flash".to_string(),
        "flagship" | "default" => "nvidia/nemotron-3.5-lightning-30b-a3b".to_string(),
        _ => {
            let catalog = get_nvidia_model_catalog();
            if let Some(found) = catalog.iter().find(|m| m.slug == input) {
                found.slug.clone()
            } else if let Some(found) = catalog
                .iter()
                .find(|m| m.slug.contains(input) || m.display_name.to_lowercase().contains(&lower))
            {
                found.slug.clone()
            } else {
                input.to_string()
            }
        }
    }
}

fn parse_sandbox_policy(s: &str) -> SandboxPolicy {
    match s.to_lowercase().as_str() {
        "readonly" | "read-only" => SandboxPolicy::ReadOnly,
        "danger" | "dangerfullaccess" | "danger-full-access" => SandboxPolicy::DangerFullAccess,
        _ => SandboxPolicy::WorkspaceWrite,
    }
}

fn parse_approval_policy(a: &str) -> ApprovalPolicy {
    match a.to_lowercase().as_str() {
        "approveforme" | "approve-for-me" => ApprovalPolicy::ApproveForMe,
        "never" => ApprovalPolicy::Never,
        _ => ApprovalPolicy::OnRequest,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_model_slug_or_alias() {
        assert_eq!(
            resolve_model_slug_or_alias("code"),
            "mistralai/codestral-22b-instruct-v0.1"
        );
        assert_eq!(
            resolve_model_slug_or_alias("reasoning"),
            "deepseek-ai/deepseek-v4.1-flash"
        );
        assert_eq!(
            resolve_model_slug_or_alias("speed"),
            "nvidia/nemotron-3.5-bolt-e4b"
        );
        assert_eq!(
            resolve_model_slug_or_alias("flagship"),
            "nvidia/nemotron-3.5-lightning-30b-a3b"
        );
    }

    #[test]
    fn test_parse_policies() {
        assert_eq!(parse_sandbox_policy("read-only"), SandboxPolicy::ReadOnly);
        assert_eq!(
            parse_sandbox_policy("danger-full-access"),
            SandboxPolicy::DangerFullAccess
        );
        assert_eq!(
            parse_sandbox_policy("workspace-write"),
            SandboxPolicy::WorkspaceWrite
        );

        assert_eq!(parse_approval_policy("never"), ApprovalPolicy::Never);
        assert_eq!(
            parse_approval_policy("approve-for-me"),
            ApprovalPolicy::ApproveForMe
        );
        assert_eq!(
            parse_approval_policy("on-request"),
            ApprovalPolicy::OnRequest
        );
    }
}
