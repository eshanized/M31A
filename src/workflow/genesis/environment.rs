//! Deterministic environment probing and workspace fact extraction.

use super::errors::GenesisError;
use super::intake::GenesisMode;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Category of an environment fact.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EnvironmentCategory {
    Git,
    Toolchain,
    Platform,
    ProjectStructure,
    DependencyManifest,
    BuildSystem,
    Documentation,
}

impl std::fmt::Display for EnvironmentCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Git => write!(f, "git"),
            Self::Toolchain => write!(f, "toolchain"),
            Self::Platform => write!(f, "platform"),
            Self::ProjectStructure => write!(f, "project_structure"),
            Self::DependencyManifest => write!(f, "dependency_manifest"),
            Self::BuildSystem => write!(f, "build_system"),
            Self::Documentation => write!(f, "documentation"),
        }
    }
}

/// Source that provided the fact.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FactSource {
    SystemCommand,
    FilesystemScan,
    HostEnvironment,
}

/// Epistemic confidence in a discovered fact.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Confidence {
    Unknown,
    Inferred,
    Certain,
}

/// A deterministic or inferred fact about the workspace environment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnvironmentFact {
    pub category: EnvironmentCategory,
    pub key: String,
    pub value: String,
    pub source: FactSource,
    pub confidence: Confidence,
}

impl EnvironmentFact {
    pub fn new(
        category: EnvironmentCategory,
        key: impl Into<String>,
        value: impl Into<String>,
        source: FactSource,
        confidence: Confidence,
    ) -> Self {
        Self {
            category,
            key: key.into(),
            value: value.into(),
            source,
            confidence,
        }
    }
}

/// Deterministic environment snapshot of a target workspace.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceEnvironment {
    pub workspace_root: PathBuf,
    pub facts: Vec<EnvironmentFact>,
    pub detected_mode: GenesisMode,
    pub detected_stack: Option<String>,
    pub file_count: usize,
    pub has_git: bool,
}

impl WorkspaceEnvironment {
    /// Probe the workspace deterministically.
    pub fn probe(workspace_root: &Path) -> Result<Self, GenesisError> {
        let mut facts = Vec::new();

        // 1. Host platform facts
        facts.push(EnvironmentFact::new(
            EnvironmentCategory::Platform,
            "platform.os",
            std::env::consts::OS,
            FactSource::HostEnvironment,
            Confidence::Certain,
        ));
        facts.push(EnvironmentFact::new(
            EnvironmentCategory::Platform,
            "platform.arch",
            std::env::consts::ARCH,
            FactSource::HostEnvironment,
            Confidence::Certain,
        ));

        // 2. Toolchain versions
        probe_toolchain(&mut facts, "rustc", &["--version"]);
        probe_toolchain(&mut facts, "cargo", &["--version"]);
        probe_toolchain(&mut facts, "git", &["--version"]);
        probe_toolchain(&mut facts, "node", &["--version"]);
        probe_toolchain(&mut facts, "python3", &["--version"]);
        probe_toolchain(&mut facts, "go", &["version"]);

        // 3. Git repository state
        let git_dir = workspace_root.join(".git");
        let has_git = git_dir.exists();
        facts.push(EnvironmentFact::new(
            EnvironmentCategory::Git,
            "git.present",
            has_git.to_string(),
            FactSource::FilesystemScan,
            Confidence::Certain,
        ));

        if has_git {
            if let Some(branch) =
                run_cmd_in_dir(workspace_root, "git", &["branch", "--show-current"])
                    .filter(|b| !b.trim().is_empty())
            {
                facts.push(EnvironmentFact::new(
                    EnvironmentCategory::Git,
                    "git.branch",
                    branch.trim(),
                    FactSource::SystemCommand,
                    Confidence::Certain,
                ));
            }

            if let Some(status) = run_cmd_in_dir(workspace_root, "git", &["status", "--porcelain"])
            {
                let is_dirty = !status.trim().is_empty();
                facts.push(EnvironmentFact::new(
                    EnvironmentCategory::Git,
                    "git.dirty",
                    is_dirty.to_string(),
                    FactSource::SystemCommand,
                    Confidence::Certain,
                ));
            }
        }

        // 4. Filesystem scan
        let mut file_count = 0;
        let mut has_source_code = false;
        let mut detected_stacks = Vec::new();

        if workspace_root.exists() {
            scan_workspace(
                workspace_root,
                &mut file_count,
                &mut has_source_code,
                &mut detected_stacks,
                &mut facts,
            )?;
        }

        facts.push(EnvironmentFact::new(
            EnvironmentCategory::ProjectStructure,
            "structure.file_count",
            file_count.to_string(),
            FactSource::FilesystemScan,
            Confidence::Certain,
        ));

        // 5. Deduce mode (Greenfield vs Brownfield)
        let detected_mode = if !workspace_root.exists() || (file_count <= 2 && !has_source_code) {
            GenesisMode::Greenfield
        } else {
            GenesisMode::Brownfield
        };

        let detected_stack = if !detected_stacks.is_empty() {
            Some(detected_stacks.join(", "))
        } else {
            None
        };

        Ok(Self {
            workspace_root: workspace_root.to_path_buf(),
            facts,
            detected_mode,
            detected_stack,
            file_count,
            has_git,
        })
    }

    /// Retrieve value of a specific fact by key.
    pub fn get_fact(&self, key: &str) -> Option<&str> {
        self.facts
            .iter()
            .find(|f| f.key == key)
            .map(|f| f.value.as_str())
    }

    /// Check if a fact exists and equals a specific string.
    pub fn fact_equals(&self, key: &str, expected: &str) -> bool {
        self.get_fact(key).map(|v| v == expected).unwrap_or(false)
    }

    /// Check if a fact exists.
    pub fn has_fact(&self, key: &str) -> bool {
        self.facts.iter().any(|f| f.key == key)
    }

    /// Filter facts by category.
    pub fn facts_in_category(&self, category: EnvironmentCategory) -> Vec<&EnvironmentFact> {
        self.facts
            .iter()
            .filter(|f| f.category == category)
            .collect()
    }

    /// Whether this workspace was determined to be greenfield.
    pub fn is_greenfield(&self) -> bool {
        self.detected_mode == GenesisMode::Greenfield
    }
}

fn probe_toolchain(facts: &mut Vec<EnvironmentFact>, tool: &str, args: &[&str]) {
    let output = Command::new(tool).args(args).output().ok();
    if let Some(out) = output.filter(|o| o.status.success())
        && let Ok(stdout) = String::from_utf8(out.stdout)
    {
        let first_line = stdout.lines().next().unwrap_or("").trim();
        if !first_line.is_empty() {
            facts.push(EnvironmentFact::new(
                EnvironmentCategory::Toolchain,
                format!("toolchain.{}", tool),
                first_line,
                FactSource::SystemCommand,
                Confidence::Certain,
            ));
        }
    }
}

fn run_cmd_in_dir(dir: &Path, tool: &str, args: &[&str]) -> Option<String> {
    let output = Command::new(tool)
        .current_dir(dir)
        .args(args)
        .output()
        .ok()?;
    if output.status.success() {
        String::from_utf8(output.stdout).ok()
    } else {
        None
    }
}

fn scan_workspace(
    root: &Path,
    file_count: &mut usize,
    has_source_code: &mut bool,
    detected_stacks: &mut Vec<String>,
    facts: &mut Vec<EnvironmentFact>,
) -> Result<(), GenesisError> {
    let read_dir = match std::fs::read_dir(root) {
        Ok(rd) => rd,
        Err(_) => return Ok(()),
    };

    for entry in read_dir.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();

        // Skip .git and .m31a directories in direct scan
        if name == ".git" || name == ".m31a" || name == "target" || name == "node_modules" {
            continue;
        }

        if path.is_file() {
            *file_count += 1;

            match name.as_str() {
                "Cargo.toml" => {
                    *has_source_code = true;
                    if !detected_stacks.contains(&"Rust (Cargo)".to_string()) {
                        detected_stacks.push("Rust (Cargo)".to_string());
                    }
                    facts.push(EnvironmentFact::new(
                        EnvironmentCategory::DependencyManifest,
                        "manifest.cargo",
                        "Cargo.toml",
                        FactSource::FilesystemScan,
                        Confidence::Certain,
                    ));
                }
                "package.json" => {
                    *has_source_code = true;
                    if !detected_stacks.contains(&"Node.js (npm/pnpm/yarn)".to_string()) {
                        detected_stacks.push("Node.js (npm/pnpm/yarn)".to_string());
                    }
                    facts.push(EnvironmentFact::new(
                        EnvironmentCategory::DependencyManifest,
                        "manifest.package_json",
                        "package.json",
                        FactSource::FilesystemScan,
                        Confidence::Certain,
                    ));
                }
                "go.mod" => {
                    *has_source_code = true;
                    if !detected_stacks.contains(&"Go".to_string()) {
                        detected_stacks.push("Go".to_string());
                    }
                    facts.push(EnvironmentFact::new(
                        EnvironmentCategory::DependencyManifest,
                        "manifest.go_mod",
                        "go.mod",
                        FactSource::FilesystemScan,
                        Confidence::Certain,
                    ));
                }
                "pyproject.toml" | "requirements.txt" | "setup.py" => {
                    *has_source_code = true;
                    if !detected_stacks.contains(&"Python".to_string()) {
                        detected_stacks.push("Python".to_string());
                    }
                    facts.push(EnvironmentFact::new(
                        EnvironmentCategory::DependencyManifest,
                        format!("manifest.{}", name.replace('.', "_")),
                        name.clone(),
                        FactSource::FilesystemScan,
                        Confidence::Certain,
                    ));
                }
                "Makefile" => {
                    facts.push(EnvironmentFact::new(
                        EnvironmentCategory::BuildSystem,
                        "build.makefile",
                        "Makefile",
                        FactSource::FilesystemScan,
                        Confidence::Certain,
                    ));
                }
                "Dockerfile" | "Containerfile" => {
                    facts.push(EnvironmentFact::new(
                        EnvironmentCategory::BuildSystem,
                        "build.dockerfile",
                        name.clone(),
                        FactSource::FilesystemScan,
                        Confidence::Certain,
                    ));
                }
                "README.md" => {
                    facts.push(EnvironmentFact::new(
                        EnvironmentCategory::Documentation,
                        "doc.readme",
                        "README.md",
                        FactSource::FilesystemScan,
                        Confidence::Certain,
                    ));
                }
                _ => {
                    let is_code_file =
                        path.extension()
                            .and_then(|e| e.to_str())
                            .is_some_and(|ext| {
                                matches!(
                                    ext,
                                    "rs" | "js" | "ts" | "py" | "go" | "c" | "cpp" | "java" | "rb"
                                )
                            });
                    if is_code_file {
                        *has_source_code = true;
                    }
                }
            }
        } else if path.is_dir() {
            // Check subdirectories
            if name == "src" || name == "lib" || name == "pkg" || name == "cmd" {
                *has_source_code = true;
            }
            if name == ".github" {
                facts.push(EnvironmentFact::new(
                    EnvironmentCategory::BuildSystem,
                    "ci.github_actions",
                    ".github",
                    FactSource::FilesystemScan,
                    Confidence::Certain,
                ));
            }
        }
    }

    Ok(())
}
