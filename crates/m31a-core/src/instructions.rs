//! Hierarchical user instructions subsystem with source attribution and subagent inheritance.
//!
//! Hierarchical user instruction loader and session isolation boundary rules.
//! Loads and attributes instructions across multiple discovery tiers:
//! 1. Subdirectory overrides (`AGENTS.override.md`)
//! 2. Workspace root documentation (`AGENTS.md`, `CLAUDE.md`, `.m31a/instructions.md`)
//! 3. User global instructions (`~/.config/m31a/instructions.md`, `~/.m31a/instructions.md`)
//! 4. Dynamic thread-level instructions and subagent inheritance controls.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

pub const DEFAULT_MAX_INSTRUCTIONS_BYTES: usize = 64 * 1024; // 64 KB total budget
pub const GLOBAL_INSTRUCTIONS_FILENAMES: &[&str] = &["instructions.md", "USER_INSTRUCTIONS.md"];
pub const WORKSPACE_INSTRUCTIONS_FILENAMES: &[&str] = &[
    "AGENTS.override.md",
    "AGENTS.md",
    "CLAUDE.md",
    ".m31a/instructions.md",
];

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "path")]
pub enum InstructionSource {
    SubdirectoryOverride(PathBuf),
    WorkspaceRoot(PathBuf),
    GlobalUser(PathBuf),
    ThreadSpecific { thread_id: String },
    CommandLineFlag,
}

impl InstructionSource {
    pub fn display_label(&self) -> String {
        match self {
            Self::SubdirectoryOverride(p) => format!(
                "Subdirectory Override ({})",
                p.file_name().unwrap_or_default().to_string_lossy()
            ),
            Self::WorkspaceRoot(p) => format!(
                "Workspace Documentation ({})",
                p.file_name().unwrap_or_default().to_string_lossy()
            ),
            Self::GlobalUser(p) => format!("User Global Instructions ({})", p.display()),
            Self::ThreadSpecific { thread_id } => format!("Thread Custom ({})", thread_id),
            Self::CommandLineFlag => "CLI Argument (--instructions)".to_string(),
        }
    }

    pub fn file_path(&self) -> Option<&Path> {
        match self {
            Self::SubdirectoryOverride(p) | Self::WorkspaceRoot(p) | Self::GlobalUser(p) => Some(p),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstructionEntry {
    pub source: InstructionSource,
    pub text: String,
    pub byte_size: usize,
    pub estimated_tokens: usize,
}

impl InstructionEntry {
    pub fn new(source: InstructionSource, text: impl Into<String>) -> Self {
        let text = text.into();
        let byte_size = text.len();
        let estimated_tokens = byte_size / 4;
        Self {
            source,
            text,
            byte_size,
            estimated_tokens,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct HierarchicalInstructions {
    pub entries: Vec<InstructionEntry>,
    pub warnings: Vec<String>,
    pub share_with_subagents: bool,
}

impl HierarchicalInstructions {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            warnings: Vec::new(),
            share_with_subagents: true,
        }
    }

    /// Discovers hierarchical instructions starting from current working directory,
    /// searching up to project root, and then checking user global directories.
    pub fn discover(cwd: &Path, global_override: Option<&Path>) -> Self {
        let mut inst = Self::new();
        let mut consumed_bytes = 0usize;
        let mut seen_paths = std::collections::HashSet::new();

        // 1. Check local directory for AGENTS.override.md
        let local_override = cwd.join("AGENTS.override.md");
        if local_override.is_file()
            && let Ok(content) = fs::read_to_string(&local_override)
        {
            let trimmed = content.trim().to_string();
            if !trimmed.is_empty() {
                consumed_bytes += trimmed.len();
                seen_paths.insert(local_override.clone());
                inst.entries.push(InstructionEntry::new(
                    InstructionSource::SubdirectoryOverride(local_override),
                    trimmed,
                ));
            }
        }

        // 2. Discover workspace root and load project-level files
        let root = find_workspace_root(cwd).unwrap_or_else(|| cwd.to_path_buf());
        for candidate_name in &["AGENTS.md", "CLAUDE.md", ".m31a/instructions.md"] {
            if consumed_bytes >= DEFAULT_MAX_INSTRUCTIONS_BYTES {
                inst.warnings
                    .push("Instruction size limit reached; skipping further sources.".to_string());
                break;
            }
            let candidate_path = root.join(candidate_name);
            if candidate_path.is_file()
                && !seen_paths.contains(&candidate_path)
                && let Ok(content) = fs::read_to_string(&candidate_path)
            {
                let trimmed = content.trim().to_string();
                if !trimmed.is_empty() {
                    let remaining = DEFAULT_MAX_INSTRUCTIONS_BYTES.saturating_sub(consumed_bytes);
                    let final_text = if trimmed.len() > remaining {
                        inst.warnings.push(format!(
                            "Truncated {} to fit within byte budget.",
                            candidate_path.display()
                        ));
                        trimmed[..remaining].to_string()
                    } else {
                        trimmed
                    };
                    consumed_bytes += final_text.len();
                    seen_paths.insert(candidate_path.clone());
                    inst.entries.push(InstructionEntry::new(
                        InstructionSource::WorkspaceRoot(candidate_path),
                        final_text,
                    ));
                }
            }
        }

        // 3. User global instructions
        if consumed_bytes < DEFAULT_MAX_INSTRUCTIONS_BYTES {
            let global_paths = if let Some(p) = global_override {
                vec![p.to_path_buf()]
            } else {
                let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
                vec![
                    PathBuf::from(&home)
                        .join(".config")
                        .join("m31a")
                        .join("instructions.md"),
                    PathBuf::from(&home).join(".m31a").join("instructions.md"),
                ]
            };

            for gp in global_paths {
                if gp.is_file()
                    && !seen_paths.contains(&gp)
                    && let Ok(content) = fs::read_to_string(&gp)
                {
                    let trimmed = content.trim().to_string();
                    if !trimmed.is_empty() {
                        let remaining =
                            DEFAULT_MAX_INSTRUCTIONS_BYTES.saturating_sub(consumed_bytes);
                        let final_text = if trimmed.len() > remaining {
                            inst.warnings.push(format!(
                                "Truncated global instructions {} to fit within budget.",
                                gp.display()
                            ));
                            trimmed[..remaining].to_string()
                        } else {
                            trimmed
                        };
                        let _ = consumed_bytes;
                        seen_paths.insert(gp.clone());
                        inst.entries.push(InstructionEntry::new(
                            InstructionSource::GlobalUser(gp),
                            final_text,
                        ));
                        break; // First existing global file wins
                    }
                }
            }
        }

        inst
    }

    /// Renders all active instruction sources into clearly attributed markdown.
    pub fn render_attributed_markdown(&self) -> String {
        if self.entries.is_empty() {
            return String::new();
        }

        let mut out = String::from("## User & Project Instructions\n\n");
        for entry in &self.entries {
            let header = match &entry.source {
                InstructionSource::SubdirectoryOverride(p) => {
                    format!("### Local Directory Override (`{}`):\n", p.display())
                }
                InstructionSource::WorkspaceRoot(p) => {
                    format!("### Project Instructions (`{}`):\n", p.display())
                }
                InstructionSource::GlobalUser(p) => {
                    format!("### User Global Preferences (`{}`):\n", p.display())
                }
                InstructionSource::ThreadSpecific { thread_id } => {
                    format!("### Session Custom Directives (Thread `{}`):\n", thread_id)
                }
                InstructionSource::CommandLineFlag => "### CLI Flag Instructions:\n".to_string(),
            };

            out.push_str(&header);
            out.push_str(&entry.text);
            out.push_str("\n\n");
        }

        out
    }

    /// Generates structured JSON metadata of all active instruction sources.
    pub fn sources_summary(&self) -> Vec<serde_json::Value> {
        self.entries
            .iter()
            .map(|e| {
                serde_json::json!({
                    "label": e.source.display_label(),
                    "source": e.source,
                    "byte_size": e.byte_size,
                    "estimated_tokens": e.estimated_tokens,
                    "path": e.source.file_path().map(|p| p.to_string_lossy()),
                })
            })
            .collect()
    }
}

fn find_workspace_root(start: &Path) -> Option<PathBuf> {
    let mut current = start.to_path_buf();
    loop {
        for marker in &[".git", ".hg", ".svn", "Cargo.toml", "package.json"] {
            if current.join(marker).exists() {
                return Some(current);
            }
        }
        if !current.pop() {
            break;
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_hierarchical_instructions_discovery_and_attribution() {
        let dir = tempdir().unwrap();
        let ws_root = dir.path();

        // Create workspace AGENTS.md
        let agents_md = ws_root.join("AGENTS.md");
        fs::write(&agents_md, "Enforce strict error handling in Rust.").unwrap();

        // Create custom global file
        let global_file = ws_root.join("global_prefs.md");
        fs::write(&global_file, "Always prefer functional iterators.").unwrap();

        let instructions = HierarchicalInstructions::discover(ws_root, Some(&global_file));
        assert_eq!(instructions.entries.len(), 2);

        let rendered = instructions.render_attributed_markdown();
        assert!(rendered.contains("Project Instructions"));
        assert!(rendered.contains("Enforce strict error handling in Rust."));
        assert!(rendered.contains("User Global Preferences"));
        assert!(rendered.contains("Always prefer functional iterators."));

        let summary = instructions.sources_summary();
        assert_eq!(summary.len(), 2);
        assert_eq!(summary[0]["byte_size"], 38);
    }
}
