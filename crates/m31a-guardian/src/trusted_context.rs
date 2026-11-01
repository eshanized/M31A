//! Trusted tool and skill verification for Guardian Reviewer.
//!
//! Verifies whether tools or skills originate from trusted connectors, internal plugins,
//! or verified directories. Reduces friction and unnecessary approval blocks for benign operations.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrustedToolSource {
    SystemBuiltin,
    PluginServiceOrchestrator,
    HomeOwnedPlugin(PathBuf),
    UserVerifiedMcp(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrustedTool {
    pub server_name: String,
    pub connector_id: Option<String>,
    pub source: TrustedToolSource,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrustedSkill {
    pub name: String,
    pub path: PathBuf,
    pub content_hash: String,
    pub trusted_by_user: bool,
}

#[derive(Debug, Default)]
pub struct TrustedRegistry {
    trusted_servers: HashSet<String>,
    trusted_skills: Vec<TrustedSkill>,
    builtin_tools: HashSet<String>,
}

impl TrustedRegistry {
    pub fn new() -> Self {
        let mut builtin_tools = HashSet::new();
        // Safe read-only and inspection tools that do not require external authorization
        for tool in &[
            "get_goal",
            "file_search",
            "list_dir",
            "read_file",
            "grep_search",
            "current_time",
            "list_skills",
            "read_skill",
            "history_list_windows",
            "history_list_items",
            "history_read_item",
            "history_search",
            "list_connectors",
            "thread_lineage",
        ] {
            builtin_tools.insert(tool.to_string());
        }

        Self {
            trusted_servers: HashSet::new(),
            trusted_skills: Vec::new(),
            builtin_tools,
        }
    }

    pub fn register_trusted_server(&mut self, server_name: impl Into<String>) {
        self.trusted_servers.insert(server_name.into());
    }

    pub fn register_trusted_skill(&mut self, skill: TrustedSkill) {
        self.trusted_skills.push(skill);
    }

    /// Verifies if a tool call by name and server is intrinsically trusted.
    pub fn is_tool_trusted(&self, tool_name: &str, server_name: Option<&str>) -> bool {
        if self.builtin_tools.contains(tool_name) {
            return true;
        }
        if let Some(server) = server_name
            && self.trusted_servers.contains(server)
        {
            return true;
        }
        false
    }

    /// Verifies if a skill invocation is trusted based on path and hash.
    pub fn is_skill_trusted(&self, skill_name: &str, path: &Path, content_hash: &str) -> bool {
        self.trusted_skills.iter().any(|s| {
            s.name == skill_name
                && s.path == path
                && s.content_hash == content_hash
                && s.trusted_by_user
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builtin_tool_trusted() {
        let reg = TrustedRegistry::new();
        assert!(reg.is_tool_trusted("read_file", None));
        assert!(reg.is_tool_trusted("grep_search", None));
        assert!(reg.is_tool_trusted("get_goal", None));
        assert!(!reg.is_tool_trusted("execute_shell", None));
    }

    #[test]
    fn test_trusted_server_tool() {
        let mut reg = TrustedRegistry::new();
        reg.register_trusted_server("internal_db");
        assert!(reg.is_tool_trusted("query", Some("internal_db")));
        assert!(!reg.is_tool_trusted("query", Some("untrusted_external")));
    }

    #[test]
    fn test_trusted_skill_verification() {
        let mut reg = TrustedRegistry::new();
        let path = PathBuf::from("/home/user/.m31a/skills/git-audit");
        reg.register_trusted_skill(TrustedSkill {
            name: "git-audit".to_string(),
            path: path.clone(),
            content_hash: "sha256:abc123".to_string(),
            trusted_by_user: true,
        });

        assert!(reg.is_skill_trusted("git-audit", &path, "sha256:abc123"));
        assert!(!reg.is_skill_trusted("git-audit", &path, "sha256:tampered"));
    }
}
