//! Git repository safety auditor and hook inspection armor.
//!
//! Scans `.git/hooks`, custom `core.hooksPath`, `.git/config`, included configs,
//! and git aliases for code-execution exploit vectors before granting
//! unrestricted execution in cloned workspaces.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RiskSeverity {
    Critical,
    High,
    Medium,
    Low,
}

#[derive(Debug, Clone)]
pub struct GitRisk {
    pub severity: RiskSeverity,
    pub title: String,
    pub location: PathBuf,
    pub description: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArmorVerdict {
    Safe,
    Caution {
        high_risks: usize,
        medium_risks: usize,
    },
    Dangerous {
        critical_risks: usize,
    },
}

#[derive(Debug, Clone)]
pub struct GitArmorReport {
    pub is_safe: bool,
    pub verdict: ArmorVerdict,
    pub risks: Vec<GitRisk>,
    pub hooks_detected: usize,
    pub custom_commands_detected: usize,
}

pub struct GitArmor;

impl GitArmor {
    /// Audits a repository working tree for suspicious git hooks and configuration exploits.
    pub fn audit(repo_root: &Path) -> GitArmorReport {
        let mut risks = Vec::new();
        let mut hooks_detected = 0;
        let mut custom_commands_detected = 0;

        let git_entry = repo_root.join(".git");
        if !git_entry.exists() {
            return GitArmorReport {
                is_safe: true,
                verdict: ArmorVerdict::Safe,
                risks,
                hooks_detected: 0,
                custom_commands_detected: 0,
            };
        }

        // 1. Resolve actual git directory (supporting `.git` files in submodules and worktrees)
        let resolved_git_dir = if git_entry.is_file() {
            if let Ok(content) = fs::read_to_string(&git_entry) {
                let trimmed = content.trim();
                if let Some(target) = trimmed.strip_prefix("gitdir:") {
                    let target_path = target.trim();
                    let resolved = if Path::new(target_path).is_absolute() {
                        PathBuf::from(target_path)
                    } else {
                        repo_root.join(target_path)
                    };
                    risks.push(GitRisk {
                        severity: RiskSeverity::Medium,
                        title: "Alternate Git directory pointer (`gitdir`) detected".to_string(),
                        location: git_entry.clone(),
                        description: format!(
                            "Workspace uses external git directory: '{}'",
                            resolved.display()
                        ),
                    });
                    resolved
                } else {
                    git_entry.clone()
                }
            } else {
                git_entry.clone()
            }
        } else {
            git_entry.clone()
        };

        // 2. Audit Git configuration and detect custom hooksPath / includes
        let mut audited_configs = HashSet::new();
        let mut custom_hooks_dirs = Vec::new();

        let primary_config = resolved_git_dir.join("config");
        if primary_config.is_file() {
            Self::audit_config_file(
                repo_root,
                &primary_config,
                &mut audited_configs,
                &mut custom_hooks_dirs,
                &mut risks,
                &mut custom_commands_detected,
            );
        }

        // 3. Audit hooks directories: default `.git/hooks` + any configured `core.hooksPath`
        let mut hooks_to_check = vec![resolved_git_dir.join("hooks")];
        for custom_path in custom_hooks_dirs {
            let full_custom = if custom_path.is_absolute() {
                custom_path
            } else {
                repo_root.join(custom_path)
            };
            if !hooks_to_check.contains(&full_custom) {
                hooks_to_check.push(full_custom);
            }
        }

        for hooks_dir in hooks_to_check {
            if hooks_dir.is_dir()
                && let Ok(entries) = fs::read_dir(&hooks_dir)
            {
                for entry in entries.filter_map(|e| e.ok()) {
                    let path = entry.path();
                    let name = entry.file_name().to_string_lossy().to_string();

                    // Ignore git sample hooks
                    if name.ends_with(".sample") {
                        continue;
                    }

                    if path.is_file() {
                        hooks_detected += 1;
                        if let Ok(content) = fs::read_to_string(&path) {
                            let (has_suspicious, desc) = inspect_hook_content(&content);
                            if has_suspicious {
                                risks.push(GitRisk {
                                    severity: RiskSeverity::Critical,
                                    title: format!("Suspicious executable git hook: `{}`", name),
                                    location: path.clone(),
                                    description: desc,
                                });
                            } else {
                                risks.push(GitRisk {
                                    severity: RiskSeverity::Medium,
                                    title: format!("Active non-default git hook: `{}`", name),
                                    location: path.clone(),
                                    description:
                                        "Hook will execute automatically during git operations."
                                            .into(),
                                });
                            }
                        } else if let Ok(bytes) = fs::read(&path) {
                            // Binary hook detection (e.g. compiled ELF or Mach-O)
                            if bytes.starts_with(b"\x7fELF")
                                || bytes.starts_with(b"MZ")
                                || bytes.starts_with(&[0xfe, 0xed, 0xfa, 0xce])
                                || bytes.starts_with(&[0xcf, 0xfa, 0xed, 0xfe])
                            {
                                risks.push(GitRisk {
                                    severity: RiskSeverity::Critical,
                                    title: format!(
                                        "Binary compiled executable in git hook: `{}`",
                                        name
                                    ),
                                    location: path.clone(),
                                    description:
                                        "Binary executable payload detected in hook directory."
                                            .to_string(),
                                });
                            }
                        }
                    }
                }
            }
        }

        let has_critical = risks.iter().any(|r| r.severity == RiskSeverity::Critical);
        let high_count = risks
            .iter()
            .filter(|r| r.severity == RiskSeverity::High)
            .count();
        let medium_count = risks
            .iter()
            .filter(|r| r.severity == RiskSeverity::Medium)
            .count();

        let verdict = if has_critical {
            ArmorVerdict::Dangerous {
                critical_risks: risks
                    .iter()
                    .filter(|r| r.severity == RiskSeverity::Critical)
                    .count(),
            }
        } else if high_count > 0 || hooks_detected > 0 || medium_count > 0 {
            ArmorVerdict::Caution {
                high_risks: high_count,
                medium_risks: medium_count,
            }
        } else {
            ArmorVerdict::Safe
        };

        // Strict security rule: A repository is safe ONLY if there are no critical risks,
        // no high risks, no active unverified hooks, and no caution conditions.
        // is_safe and ArmorVerdict must never contradict each other.
        let is_safe = matches!(verdict, ArmorVerdict::Safe);

        GitArmorReport {
            is_safe,
            verdict,
            risks,
            hooks_detected,
            custom_commands_detected,
        }
    }

    fn audit_config_file(
        repo_root: &Path,
        config_file: &Path,
        audited: &mut HashSet<PathBuf>,
        custom_hooks_dirs: &mut Vec<PathBuf>,
        risks: &mut Vec<GitRisk>,
        custom_commands_detected: &mut usize,
    ) {
        if !audited.insert(config_file.to_path_buf()) {
            return; // Avoid circular includes
        }

        let content = match fs::read_to_string(config_file) {
            Ok(c) => c,
            Err(_) => return,
        };

        let dangerous_directives = [
            (
                "fsmonitor",
                "core.fsmonitor external file watcher executable",
                RiskSeverity::Critical,
            ),
            (
                "sshcommand",
                "core.sshCommand custom binary wrapper",
                RiskSeverity::Critical,
            ),
            (
                "editor",
                "core.editor custom command execution",
                RiskSeverity::High,
            ),
            (
                "pager",
                "core.pager custom execution binary",
                RiskSeverity::Medium,
            ),
            (
                "credential.helper",
                "credential.helper external binary execution",
                RiskSeverity::High,
            ),
        ];

        let mut current_section = String::new();

        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with('#') || trimmed.starts_with(';') || trimmed.is_empty() {
                continue;
            }

            if trimmed.starts_with('[') && trimmed.ends_with(']') {
                current_section = trimmed[1..trimmed.len() - 1].trim().to_lowercase();
                continue;
            }

            let lower = trimmed.to_lowercase();

            // Check dangerous directives
            for (keyword, desc, severity) in &dangerous_directives {
                if lower.contains(keyword) && lower.contains('=') {
                    *custom_commands_detected += 1;
                    risks.push(GitRisk {
                        severity: *severity,
                        title: format!("Git config command injection vector: `{}`", keyword),
                        location: config_file.to_path_buf(),
                        description: format!("{}: '{}'", desc, trimmed),
                    });
                }
            }

            // Check core.hooksPath
            if lower.starts_with("hookspath")
                && lower.contains('=')
                && let Some((_, path_str)) = trimmed.split_once('=')
            {
                let clean_path = path_str.trim().trim_matches('"').trim_matches('\'');
                *custom_commands_detected += 1;
                risks.push(GitRisk {
                    severity: RiskSeverity::Critical,
                    title: "Custom git hooks directory redirect (`core.hooksPath`)".to_string(),
                    location: config_file.to_path_buf(),
                    description: format!(
                        "Repository redirects hook execution to custom path: '{}'",
                        clean_path
                    ),
                });
                custom_hooks_dirs.push(PathBuf::from(clean_path));
            }

            // Check git aliases with shell commands: alias.<name> = !<cmd>
            if current_section.starts_with("alias")
                && trimmed.contains('=')
                && let Some((alias_name, val)) = trimmed.split_once('=')
            {
                let val_trimmed = val.trim();
                if val_trimmed.starts_with('!') {
                    *custom_commands_detected += 1;
                    risks.push(GitRisk {
                        severity: RiskSeverity::Critical,
                        title: format!(
                            "Git alias shell command injection: `{}`",
                            alias_name.trim()
                        ),
                        location: config_file.to_path_buf(),
                        description: format!(
                            "Alias executes shell command directly: '{}'",
                            val_trimmed
                        ),
                    });
                }
            }

            // Check custom diff/filter/merge drivers that invoke external commands
            let is_driver_section = current_section.starts_with("diff")
                || current_section.starts_with("filter")
                || current_section.starts_with("merge");

            if is_driver_section
                && (lower.contains("command")
                    || lower.contains("textconv")
                    || lower.contains("clean")
                    || lower.contains("smudge")
                    || lower.contains("process")
                    || lower.contains("driver"))
                && lower.contains('=')
            {
                *custom_commands_detected += 1;
                risks.push(GitRisk {
                    severity: RiskSeverity::Critical,
                    title: format!(
                        "Custom driver external command execution in section `[{}]`",
                        current_section
                    ),
                    location: config_file.to_path_buf(),
                    description: format!(
                        "Directive executes external command on git operations: '{}'",
                        trimmed
                    ),
                });
            }

            // Check config includes: `[include] path = ...` or `[includeIf "..."] path = ...`
            if (current_section.starts_with("include") || current_section.starts_with("includeif"))
                && lower.starts_with("path")
                && lower.contains('=')
                && let Some((_, inc_path)) = trimmed.split_once('=')
            {
                let clean_inc = inc_path.trim().trim_matches('"').trim_matches('\'');
                let full_inc = if Path::new(clean_inc).is_absolute() {
                    PathBuf::from(clean_inc)
                } else if let Some(parent) = config_file.parent() {
                    parent.join(clean_inc)
                } else {
                    repo_root.join(clean_inc)
                };

                risks.push(GitRisk {
                    severity: RiskSeverity::Medium,
                    title: "Git config include directive detected".to_string(),
                    location: config_file.to_path_buf(),
                    description: format!(
                        "Includes external git config file: '{}'",
                        full_inc.display()
                    ),
                });

                // Recursively audit included config
                Self::audit_config_file(
                    repo_root,
                    &full_inc,
                    audited,
                    custom_hooks_dirs,
                    risks,
                    custom_commands_detected,
                );
            }
        }
    }
}

fn inspect_hook_content(content: &str) -> (bool, String) {
    let patterns = [
        ("curl ", "Downloads external payload via curl"),
        ("wget ", "Downloads external payload via wget"),
        ("nc -", "Opens netcat reverse shell or socket"),
        ("ncat ", "Opens ncat connection"),
        ("socat ", "Opens socat socket"),
        ("/dev/tcp/", "Opens direct bash socket connection"),
        ("base64 -d", "Decodes obfuscated base64 payload"),
        ("base64 --decode", "Decodes obfuscated base64 payload"),
        ("rm -rf /", "Destructive recursive file deletion"),
        ("rm -rf ~", "Destructive home directory file deletion"),
        ("python -c", "Executes inline Python one-liner"),
        ("python3 -c", "Executes inline Python one-liner"),
        ("perl -e", "Executes inline Perl one-liner"),
        ("ruby -e", "Executes inline Ruby one-liner"),
        ("php -r", "Executes inline PHP one-liner"),
        ("powershell ", "Executes PowerShell script"),
        ("pwsh ", "Executes PowerShell core script"),
        ("eval ", "Executes dynamic eval string"),
    ];

    for (pat, desc) in &patterns {
        if content.contains(pat) {
            return (true, desc.to_string());
        }
    }

    (false, String::new())
}
