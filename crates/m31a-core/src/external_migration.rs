//! Migration utilities for importing sessions and settings from external assistant environments.
//!
//! Translates historic sessions, skill directories, and configuration files into native M31A formats.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::config::McpServerConfig;
use crate::memories::MemoryManager;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExternalAgentSource {
    ClaudeCode,
    Cursor,
    Windsurf,
    Aider,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MigrationItemKind {
    McpServer,
    Memory,
    ProjectRules,
    LifecycleHooks,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MigrationCandidate {
    pub source: ExternalAgentSource,
    pub kind: MigrationItemKind,
    pub name: String,
    pub source_path: PathBuf,
    pub description: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct MigrationSummary {
    pub discovered_count: usize,
    pub imported_mcp_servers: Vec<String>,
    pub imported_memories: Vec<String>,
    pub imported_rules: Vec<String>,
    pub skipped_count: usize,
    pub errors: Vec<String>,
}

pub struct ExternalAgentMigrator {
    workspace_root: PathBuf,
    home_dir: PathBuf,
}

impl ExternalAgentMigrator {
    pub fn new(workspace_root: PathBuf) -> Self {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        Self {
            workspace_root,
            home_dir: PathBuf::from(home),
        }
    }

    pub fn with_home(workspace_root: PathBuf, home_dir: PathBuf) -> Self {
        Self {
            workspace_root,
            home_dir,
        }
    }

    /// Discovers all available migration candidates from external agents.
    pub fn discover_candidates(&self) -> Vec<MigrationCandidate> {
        let mut candidates = Vec::new();

        self.discover_claude_candidates(&mut candidates);
        self.discover_cursor_candidates(&mut candidates);
        self.discover_rules_candidates(&mut candidates);

        candidates
    }

    fn discover_claude_candidates(&self, candidates: &mut Vec<MigrationCandidate>) {
        // 1. Check ~/.claude/claude.json and workspace .claude/claude.json
        let paths = [
            self.home_dir.join(".claude").join("claude.json"),
            self.workspace_root.join(".claude").join("claude.json"),
        ];

        for path in &paths {
            if path.exists()
                && let Ok(content) = fs::read_to_string(path)
                && let Ok(val) = serde_json::from_str::<serde_json::Value>(&content)
                && let Some(servers) = val.get("mcpServers").and_then(|v| v.as_object())
            {
                for (name, _) in servers {
                    candidates.push(MigrationCandidate {
                        source: ExternalAgentSource::ClaudeCode,
                        kind: MigrationItemKind::McpServer,
                        name: name.clone(),
                        source_path: path.clone(),
                        description: format!("Claude Code MCP server '{name}'"),
                    });
                }
            }
        }

        // 2. Check Claude memory directories
        let mem_dirs = [
            self.home_dir.join(".claude").join("memory"),
            self.workspace_root.join(".claude").join("memory"),
        ];

        for dir in &mem_dirs {
            if dir.is_dir()
                && let Ok(entries) = fs::read_dir(dir)
            {
                for entry in entries.filter_map(Result::ok) {
                    let path = entry.path();
                    if path.extension().and_then(|e| e.to_str()) == Some("md") {
                        let name = path
                            .file_stem()
                            .and_then(|s| s.to_str())
                            .unwrap_or("memory")
                            .to_string();
                        candidates.push(MigrationCandidate {
                            source: ExternalAgentSource::ClaudeCode,
                            kind: MigrationItemKind::Memory,
                            name,
                            source_path: path,
                            description: "Claude Code learned memory note".to_string(),
                        });
                    }
                }
            }
        }
    }

    fn discover_cursor_candidates(&self, candidates: &mut Vec<MigrationCandidate>) {
        let paths = [
            self.home_dir.join(".cursor").join("mcp.json"),
            self.workspace_root.join(".cursor").join("mcp.json"),
        ];

        for path in &paths {
            if path.exists()
                && let Ok(content) = fs::read_to_string(path)
                && let Ok(val) = serde_json::from_str::<serde_json::Value>(&content)
                && let Some(servers) = val.get("mcpServers").and_then(|v| v.as_object())
            {
                for (name, _) in servers {
                    candidates.push(MigrationCandidate {
                        source: ExternalAgentSource::Cursor,
                        kind: MigrationItemKind::McpServer,
                        name: name.clone(),
                        source_path: path.clone(),
                        description: format!("Cursor MCP server '{name}'"),
                    });
                }
            }
        }
    }

    fn discover_rules_candidates(&self, candidates: &mut Vec<MigrationCandidate>) {
        let rule_files = [
            (
                self.workspace_root.join("CLAUDE.md"),
                ExternalAgentSource::ClaudeCode,
            ),
            (
                self.workspace_root.join(".cursorrules"),
                ExternalAgentSource::Cursor,
            ),
            (
                self.workspace_root.join(".windsurfrules"),
                ExternalAgentSource::Windsurf,
            ),
        ];

        for (path, source) in &rule_files {
            if path.exists() {
                let name = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("rules")
                    .to_string();
                candidates.push(MigrationCandidate {
                    source: *source,
                    kind: MigrationItemKind::ProjectRules,
                    name,
                    source_path: path.clone(),
                    description: format!("Project rules file from {:?}", source),
                });
            }
        }
    }

    /// Executes the migration of selected candidates.
    pub fn apply_migration(
        &self,
        candidates: &[MigrationCandidate],
        memory_manager: &MemoryManager,
        dry_run: bool,
    ) -> Result<MigrationSummary> {
        let mut summary = MigrationSummary {
            discovered_count: candidates.len(),
            ..Default::default()
        };

        for candidate in candidates {
            match candidate.kind {
                MigrationItemKind::McpServer => {
                    match self.extract_mcp_config(&candidate.source_path, &candidate.name) {
                        Ok(Some(cfg)) => {
                            if !dry_run {
                                // Save to M31A config or print
                                self.save_mcp_server_config(&candidate.name, &cfg)?;
                            }
                            summary.imported_mcp_servers.push(candidate.name.clone());
                        }
                        Ok(None) => summary.skipped_count += 1,
                        Err(e) => summary
                            .errors
                            .push(format!("MCP '{}': {}", candidate.name, e)),
                    }
                }
                MigrationItemKind::Memory => match fs::read_to_string(&candidate.source_path) {
                    Ok(content) => {
                        if !dry_run {
                            let _ = memory_manager.consolidate_fact(&candidate.name, &content);
                        }
                        summary.imported_memories.push(candidate.name.clone());
                    }
                    Err(e) => summary
                        .errors
                        .push(format!("Memory '{}': {}", candidate.name, e)),
                },
                MigrationItemKind::ProjectRules => {
                    match fs::read_to_string(&candidate.source_path) {
                        Ok(content) => {
                            if !dry_run {
                                self.append_to_agents_md(&candidate.name, &content)?;
                            }
                            summary.imported_rules.push(candidate.name.clone());
                        }
                        Err(e) => summary
                            .errors
                            .push(format!("Rules '{}': {}", candidate.name, e)),
                    }
                }
                MigrationItemKind::LifecycleHooks => {
                    summary.skipped_count += 1;
                }
            }
        }

        Ok(summary)
    }

    fn extract_mcp_config(
        &self,
        file_path: &Path,
        server_name: &str,
    ) -> Result<Option<McpServerConfig>> {
        let content = fs::read_to_string(file_path)?;
        let val: serde_json::Value = serde_json::from_str(&content)?;
        let Some(servers) = val.get("mcpServers").and_then(|v| v.as_object()) else {
            return Ok(None);
        };
        let Some(server_json) = servers.get(server_name) else {
            return Ok(None);
        };

        let command = server_json
            .get("command")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let args = server_json
            .get("args")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|s| s.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();

        let mut env = HashMap::new();
        if let Some(env_obj) = server_json.get("env").and_then(|v| v.as_object()) {
            for (k, v) in env_obj {
                if let Some(v_str) = v.as_str() {
                    env.insert(k.clone(), v_str.to_string());
                }
            }
        }

        Ok(Some(McpServerConfig::new(command, args, env)))
    }

    fn save_mcp_server_config(&self, name: &str, config: &McpServerConfig) -> Result<()> {
        let m31a_dir = self.home_dir.join(".config").join("m31a");
        fs::create_dir_all(&m31a_dir)?;
        let config_toml_path = m31a_dir.join("config.toml");

        let mut current_toml = if config_toml_path.exists() {
            fs::read_to_string(&config_toml_path)?
        } else {
            String::new()
        };

        // Append [mcp.server_name] block if not present
        let section_header = format!("[mcp.{name}]");
        if !current_toml.contains(&section_header) {
            current_toml.push_str(&format!(
                "\n\n# Migrated from external agent\n[mcp.{}]\ncommand = \"{}\"\nargs = {:?}\n",
                name, config.command, config.args
            ));
            fs::write(config_toml_path, current_toml)?;
        }
        Ok(())
    }

    fn append_to_agents_md(&self, source_name: &str, content: &str) -> Result<()> {
        let agents_md_path = self.workspace_root.join("AGENTS.md");
        let mut existing = if agents_md_path.exists() {
            fs::read_to_string(&agents_md_path)?
        } else {
            "# Project Agent Guidelines\n".to_string()
        };

        let marker = format!("<!-- Imported from {} -->", source_name);
        if !existing.contains(&marker) {
            existing.push_str(&format!("\n\n{}\n{}\n", marker, content.trim()));
            fs::write(agents_md_path, existing)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_discover_claude_and_cursor_candidates() {
        let temp = tempdir().unwrap();
        let ws = temp.path().join("workspace");
        let home = temp.path().join("home");
        fs::create_dir_all(&ws).unwrap();
        fs::create_dir_all(&home).unwrap();

        // 1. Setup sample Claude Code config
        let claude_dir = home.join(".claude");
        fs::create_dir_all(&claude_dir).unwrap();
        let claude_json = serde_json::json!({
            "mcpServers": {
                "github_claude": {
                    "command": "npx",
                    "args": ["-y", "@modelcontextprotocol/server-github"]
                }
            }
        });
        fs::write(claude_dir.join("claude.json"), claude_json.to_string()).unwrap();

        // 2. Setup sample Cursor config
        let cursor_dir = ws.join(".cursor");
        fs::create_dir_all(&cursor_dir).unwrap();
        let cursor_json = serde_json::json!({
            "mcpServers": {
                "postgres_cursor": {
                    "command": "docker",
                    "args": ["run", "-i", "mcp/postgres"]
                }
            }
        });
        fs::write(cursor_dir.join("mcp.json"), cursor_json.to_string()).unwrap();

        // 3. Setup sample CLAUDE.md
        fs::write(
            ws.join("CLAUDE.md"),
            "# Claude Guidelines\nAlways write tests!",
        )
        .unwrap();

        let migrator = ExternalAgentMigrator::with_home(ws.clone(), home.clone());
        let candidates = migrator.discover_candidates();

        assert_eq!(candidates.len(), 3);
        assert!(candidates.iter().any(|c| c.name == "github_claude"));
        assert!(candidates.iter().any(|c| c.name == "postgres_cursor"));
        assert!(candidates.iter().any(|c| c.name == "CLAUDE.md"));
    }

    #[test]
    fn test_apply_migration_dry_run() {
        let temp = tempdir().unwrap();
        let ws = temp.path().join("workspace");
        let home = temp.path().join("home");
        fs::create_dir_all(&ws).unwrap();
        fs::create_dir_all(&home).unwrap();

        let rules_path = ws.join(".cursorrules");
        fs::write(&rules_path, "Use functional style").unwrap();

        let candidates = vec![MigrationCandidate {
            source: ExternalAgentSource::Cursor,
            kind: MigrationItemKind::ProjectRules,
            name: ".cursorrules".to_string(),
            source_path: rules_path,
            description: "Cursor project rules".to_string(),
        }];

        let migrator = ExternalAgentMigrator::with_home(ws.clone(), home.clone());
        let mem_mgr = MemoryManager::with_dir(ws.join("memories"));

        let summary = migrator
            .apply_migration(&candidates, &mem_mgr, true)
            .unwrap();
        assert_eq!(summary.discovered_count, 1);
        assert_eq!(summary.imported_rules, vec![".cursorrules".to_string()]);
        assert!(!ws.join("AGENTS.md").exists()); // Dry run must not write
    }
}
