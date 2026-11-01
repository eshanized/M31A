//! AGENTS.md hierarchical discovery and user instruction assembly subsystem.
//!
//! Hierarchical AGENTS.md instruction discovery, parsing, and context assembly.
//! Discovers project-level instructions along the directory hierarchy:
//! 1. Finds the project root by walking upward from the current working directory
//!    until a marker (`.git`, `.hg`, `.svn`) is located.
//! 2. Collects documentation files (`AGENTS.override.md`, `AGENTS.md`, `CLAUDE.md`)
//!    from the project root down to the current working directory (inclusive).
//! 3. Enforces a strict byte budget (`project_doc_max_bytes`) to avoid context window blowup.
//! 4. Assembles entries using canonical delimiters.

use std::fs;
use std::path::{Path, PathBuf};

pub const DEFAULT_AGENTS_MD_FILENAME: &str = "AGENTS.md";
pub const LOCAL_AGENTS_MD_FILENAME: &str = "AGENTS.override.md";
pub const CLAUDE_MD_FILENAME: &str = "CLAUDE.md";

pub const DEFAULT_PROJECT_ROOT_MARKERS: &[&str] = &[".git", ".hg", ".svn"];
pub const DEFAULT_PROJECT_DOC_MAX_BYTES: usize = 128 * 1024; // 128 KB
pub const AGENTS_MD_SEPARATOR: &str = "\n\n--- project-doc ---\n\n";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentsMdEntry {
    pub source_path: PathBuf,
    pub content: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LoadedAgentsMd {
    pub entries: Vec<AgentsMdEntry>,
    pub user_instructions: Option<String>,
}

impl LoadedAgentsMd {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_user_instructions(mut self, instructions: impl Into<String>) -> Self {
        self.user_instructions = Some(instructions.into());
        self
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
            && self
                .user_instructions
                .as_deref()
                .unwrap_or("")
                .trim()
                .is_empty()
    }

    /// Render combined instructions with project docs and user instructions.
    pub fn render_instructions(&self) -> String {
        let mut parts = Vec::new();

        if let Some(user) = &self.user_instructions {
            let trimmed = user.trim();
            if !trimmed.is_empty() {
                parts.push(trimmed.to_string());
            }
        }

        for entry in &self.entries {
            let trimmed = entry.content.trim();
            if !trimmed.is_empty() {
                parts.push(trimmed.to_string());
            }
        }

        parts.join(AGENTS_MD_SEPARATOR)
    }

    /// Render instructions with an environment label prefix if multiple environments exist.
    pub fn render_environment_labeled(&self, environment_id: &str) -> String {
        let text = self.render_instructions();
        if text.is_empty() {
            return String::new();
        }
        if environment_id.is_empty() || environment_id == "default" {
            text
        } else {
            format!("[Environment: {environment_id}]\n{text}")
        }
    }
}

pub struct AgentsMdManager {
    root_markers: Vec<String>,
    fallback_filenames: Vec<String>,
    max_bytes: usize,
}

impl Default for AgentsMdManager {
    fn default() -> Self {
        Self {
            root_markers: DEFAULT_PROJECT_ROOT_MARKERS
                .iter()
                .map(|s| s.to_string())
                .collect(),
            fallback_filenames: vec![
                LOCAL_AGENTS_MD_FILENAME.to_string(),
                DEFAULT_AGENTS_MD_FILENAME.to_string(),
                CLAUDE_MD_FILENAME.to_string(),
            ],
            max_bytes: DEFAULT_PROJECT_DOC_MAX_BYTES,
        }
    }
}

impl AgentsMdManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_max_bytes(mut self, max_bytes: usize) -> Self {
        self.max_bytes = max_bytes;
        self
    }

    pub fn with_root_markers(mut self, markers: Vec<String>) -> Self {
        self.root_markers = markers;
        self
    }

    /// Finds the project root for the given directory by searching upward for markers.
    pub fn find_project_root(&self, cwd: &Path) -> PathBuf {
        let mut current = cwd;
        loop {
            for marker in &self.root_markers {
                let candidate = current.join(marker);
                if candidate.exists() {
                    return current.to_path_buf();
                }
            }
            match current.parent() {
                Some(parent) => current = parent,
                None => break,
            }
        }
        cwd.to_path_buf()
    }

    /// Discovers all doc paths from project root down to cwd.
    pub fn collect_doc_paths(&self, cwd: &Path) -> Vec<PathBuf> {
        let root = self.find_project_root(cwd);

        // Build path components from root to cwd
        let mut ancestors = Vec::new();
        let mut curr = cwd;
        while curr >= root.as_path() {
            ancestors.push(curr.to_path_buf());
            if curr == root.as_path() {
                break;
            }
            match curr.parent() {
                Some(p) => curr = p,
                None => break,
            }
        }
        // Reverse so order is root down to cwd
        ancestors.reverse();

        let mut discovered = Vec::new();
        for dir in ancestors {
            for filename in &self.fallback_filenames {
                let candidate = dir.join(filename);
                if candidate.is_file() {
                    discovered.push(candidate);
                    // Only take highest priority file in each directory
                    break;
                }
            }
        }
        discovered
    }

    /// Loads project instructions starting at cwd, respecting max byte budget.
    pub fn load(&self, cwd: &Path, user_instructions: Option<String>) -> LoadedAgentsMd {
        let paths = self.collect_doc_paths(cwd);
        let mut entries = Vec::new();
        let mut remaining_bytes = self.max_bytes;

        for path in paths {
            if remaining_bytes == 0 {
                break;
            }
            if let Ok(mut text) = fs::read_to_string(&path) {
                if text.len() > remaining_bytes {
                    // Truncate cleanly at a char boundary
                    let mut cut_idx = remaining_bytes;
                    while cut_idx > 0 && !text.is_char_boundary(cut_idx) {
                        cut_idx -= 1;
                    }
                    text.truncate(cut_idx);
                    text.push_str("\n... [truncated due to project_doc_max_bytes]");
                    remaining_bytes = 0;
                } else {
                    remaining_bytes = remaining_bytes.saturating_sub(text.len());
                }

                entries.push(AgentsMdEntry {
                    source_path: path,
                    content: text,
                });
            }
        }

        LoadedAgentsMd {
            entries,
            user_instructions,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_agents_md_discovery_hierarchy() {
        let tmp = tempdir().unwrap();
        let root = tmp.path();

        // Create a root marker (.git)
        fs::create_dir_all(root.join(".git")).unwrap();
        fs::write(root.join("AGENTS.md"), "# Root Rules\nRule 1: Always test.").unwrap();

        let child = root.join("crates").join("pkg");
        fs::create_dir_all(&child).unwrap();
        fs::write(
            child.join("AGENTS.md"),
            "# Subpackage Rules\nRule 2: Specific crate.",
        )
        .unwrap();

        let manager = AgentsMdManager::new();
        let loaded = manager.load(&child, Some("User prompt note".to_string()));

        assert_eq!(loaded.entries.len(), 2);
        assert_eq!(loaded.entries[0].source_path, root.join("AGENTS.md"));
        assert_eq!(loaded.entries[1].source_path, child.join("AGENTS.md"));

        let rendered = loaded.render_instructions();
        assert!(rendered.contains("User prompt note"));
        assert!(rendered.contains("# Root Rules"));
        assert!(rendered.contains("# Subpackage Rules"));
        assert!(rendered.contains(AGENTS_MD_SEPARATOR));
    }

    #[test]
    fn test_agents_override_priority() {
        let tmp = tempdir().unwrap();
        let root = tmp.path();
        fs::create_dir_all(root.join(".git")).unwrap();

        fs::write(root.join("AGENTS.md"), "Standard agents").unwrap();
        fs::write(root.join("AGENTS.override.md"), "Overridden agents").unwrap();

        let manager = AgentsMdManager::new();
        let loaded = manager.load(root, None);

        assert_eq!(loaded.entries.len(), 1);
        assert_eq!(
            loaded.entries[0].source_path,
            root.join("AGENTS.override.md")
        );
        assert_eq!(loaded.entries[0].content, "Overridden agents");
    }

    #[test]
    fn test_max_bytes_budget() {
        let tmp = tempdir().unwrap();
        let root = tmp.path();
        fs::create_dir_all(root.join(".git")).unwrap();

        let large_content = "x".repeat(500);
        fs::write(root.join("AGENTS.md"), large_content).unwrap();

        let manager = AgentsMdManager::new().with_max_bytes(100);
        let loaded = manager.load(root, None);

        assert_eq!(loaded.entries.len(), 1);
        assert!(loaded.entries[0].content.contains("truncated"));
        assert!(loaded.entries[0].content.len() < 200);
    }
}
