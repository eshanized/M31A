//! Predefined agent roles, system prompt overlays, and role capability profiles.
//!
//! Configures specialized subagent roles (e.g. Architect, Coder, Reviewer, Debugger, Researcher).

use crate::SandboxPolicy;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AgentRole {
    pub name: String,
    pub description: String,
    pub model_override: Option<String>,
    pub sandbox_policy: SandboxPolicy,
    pub temperature: Option<f32>,
    pub system_prompt_overlay: Option<String>,
    pub nickname_candidates: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct AgentRoleRegistry {
    roles: HashMap<String, AgentRole>,
}

impl Default for AgentRole {
    fn default() -> Self {
        Self {
            name: "worker".to_string(),
            description: "General software development and code modification worker.".to_string(),
            model_override: None,
            sandbox_policy: SandboxPolicy::WorkspaceWrite,
            temperature: Some(0.2),
            system_prompt_overlay: None,
            nickname_candidates: vec!["Nova".to_string(), "Atlas".to_string(), "Vega".to_string()],
        }
    }
}

impl AgentRoleRegistry {
    pub fn new() -> Self {
        let mut reg = Self {
            roles: HashMap::new(),
        };
        reg.register_builtins();
        reg
    }

    fn register_builtins(&mut self) {
        self.roles.insert(
            "architect".to_string(),
            AgentRole {
                name: "architect".to_string(),
                description: "High-level software architect responsible for design plans, modular decomposition, and system blueprints.".to_string(),
                model_override: Some("deepseek-ai/deepseek-r1".to_string()),
                sandbox_policy: SandboxPolicy::ReadOnly,
                temperature: Some(0.6),
                system_prompt_overlay: Some("You are acting as System Architect. Focus exclusively on architecture, dependency graphs, interface boundaries, and step-by-step implementation roadmaps without mutating code directly.".to_string()),
                nickname_candidates: vec!["Turing".to_string(), "Ada".to_string(), "Knuth".to_string()],
            },
        );

        self.roles.insert(
            "coder".to_string(),
            AgentRole {
                name: "coder".to_string(),
                description: "Deep production implementation specialist executing clean, robust, and tested code changes.".to_string(),
                model_override: Some("nvidia/nemotron-3.5-lightning-30b-a3b".to_string()),
                sandbox_policy: SandboxPolicy::WorkspaceWrite,
                temperature: Some(0.1),
                system_prompt_overlay: Some("You are acting as Production Software Engineer. Write clean, complete, and idiomatic production code with zero placeholders and rigorous error handling.".to_string()),
                nickname_candidates: vec!["Torvalds".to_string(), "Ritchie".to_string(), "Carmack".to_string()],
            },
        );

        self.roles.insert(
            "reviewer".to_string(),
            AgentRole {
                name: "reviewer".to_string(),
                description: "Adversarial code reviewer verifying correctness, security constraints, edge cases, and performance regressions.".to_string(),
                model_override: Some("nvidia/nemotron-3.5-lightning-30b-a3b".to_string()),
                sandbox_policy: SandboxPolicy::ReadOnly,
                temperature: Some(0.1),
                system_prompt_overlay: Some("You are acting as Adversarial Security & Quality Reviewer. Audit diffs meticulously for CVEs, race conditions, memory leaks, and breaking API changes.".to_string()),
                nickname_candidates: vec!["Hopper".to_string(), "Hamilton".to_string(), "Dijkstra".to_string()],
            },
        );

        self.roles.insert(
            "explorer".to_string(),
            AgentRole {
                name: "explorer".to_string(),
                description: "Rapid repository and dependency investigator reading files, symbols, and documentation.".to_string(),
                model_override: Some("meta/llama-3.1-8b-instruct".to_string()),
                sandbox_policy: SandboxPolicy::ReadOnly,
                temperature: Some(0.2),
                system_prompt_overlay: Some("You are acting as Fast Codebase Explorer. Navigate repository hierarchies, locate symbol definitions, and extract targeted evidence rapidly.".to_string()),
                nickname_candidates: vec!["Magellan".to_string(), "Cook".to_string(), "Voyager".to_string()],
            },
        );
    }

    /// Loads custom user and workspace agent roles from disk (`.m31a/roles/*.toml`).
    pub fn load_custom_roles(&mut self, cwd: &Path) {
        let mut search_dirs = Vec::new();
        search_dirs.push(cwd.join(".m31a").join("roles"));

        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        search_dirs.push(
            PathBuf::from(home)
                .join(".config")
                .join("m31a")
                .join("roles"),
        );

        for dir in search_dirs {
            if dir.is_dir()
                && let Ok(entries) = fs::read_dir(dir)
            {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.extension().and_then(|s| s.to_str()) == Some("toml")
                        && let Ok(content) = fs::read_to_string(&path)
                        && let Ok(role) = toml::from_str::<AgentRole>(&content)
                    {
                        self.roles.insert(role.name.to_ascii_lowercase(), role);
                    }
                }
            }
        }
    }

    pub fn get_role(&self, name: &str) -> Option<&AgentRole> {
        self.roles.get(&name.to_ascii_lowercase())
    }

    pub fn get(&self, name: &str) -> Option<&AgentRole> {
        self.get_role(name)
    }

    pub fn list_roles(&self) -> Vec<&AgentRole> {
        let mut list: Vec<&AgentRole> = self.roles.values().collect();
        list.sort_by_key(|r| &r.name);
        list
    }

    /// Formats an actionable prompt block explaining available agent roles for subagent delegation.
    pub fn render_roles_prompt(&self) -> String {
        let mut out = String::from("## Available Specialized Subagent Roles\n\n");
        out.push_str("When spawning background subagents via `spawn_agent`, assign the most appropriate role:\n");
        for role in self.list_roles() {
            let model_str = role.model_override.as_deref().unwrap_or("inherit");
            out.push_str(&format!(
                "- **{}** (model: `{model_str}`, sandbox: `{:?}`): {}\n",
                role.name, role.sandbox_policy, role.description
            ));
        }
        out.push('\n');
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builtin_agent_roles() {
        let registry = AgentRoleRegistry::new();
        let architect = registry.get_role("architect").unwrap();
        assert_eq!(architect.sandbox_policy, SandboxPolicy::ReadOnly);
        assert!(architect.description.contains("architect"));

        let coder = registry.get_role("coder").unwrap();
        assert_eq!(coder.sandbox_policy, SandboxPolicy::WorkspaceWrite);

        let reviewer = registry.get_role("reviewer").unwrap();
        assert_eq!(reviewer.sandbox_policy, SandboxPolicy::ReadOnly);

        let explorer = registry.get_role("explorer").unwrap();
        assert!(
            explorer
                .nickname_candidates
                .contains(&"Voyager".to_string())
        );
    }

    #[test]
    fn test_render_roles_prompt() {
        let registry = AgentRoleRegistry::new();
        let prompt = registry.render_roles_prompt();
        assert!(prompt.contains("Available Specialized Subagent Roles"));
        assert!(prompt.contains("architect"));
        assert!(prompt.contains("coder"));
        assert!(prompt.contains("reviewer"));
    }
}
