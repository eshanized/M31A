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

/// Classification of workspace initialization state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceMode {
    /// Blank slate directory with zero or only scaffold/doc files.
    Greenfield,
    /// Existing codebase with code, manifests, git history, or build files.
    Brownfield,
    /// Probe failed or directory state is unverified.
    Unknown,
}

impl std::fmt::Display for WorkspaceMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Greenfield => write!(f, "greenfield"),
            Self::Brownfield => write!(f, "brownfield"),
            Self::Unknown => write!(f, "unknown"),
        }
    }
}

/// Deterministic environment snapshot of a target workspace.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceEnvironment {
    pub workspace_root: PathBuf,
    pub facts: Vec<EnvironmentFact>,
    pub detected_mode: GenesisMode,
    pub workspace_mode: WorkspaceMode,
    pub detected_stack: Option<String>,
    pub file_count: usize,
    pub has_git: bool,
}

impl WorkspaceEnvironment {
    /// Probe the workspace deterministically.
    pub fn probe(workspace_root: &Path) -> Result<Self, GenesisError> {
        if !workspace_root.exists() {
            return Err(GenesisError::EnvironmentProbeFailed(format!(
                "workspace root does not exist: {}",
                workspace_root.display()
            )));
        }

        if !workspace_root.is_dir() {
            return Err(GenesisError::EnvironmentProbeFailed(format!(
                "workspace root is not a directory: {}",
                workspace_root.display()
            )));
        }

        // Test readability of root directory directly
        if let Err(err) = std::fs::read_dir(workspace_root) {
            return Err(GenesisError::EnvironmentProbeFailed(format!(
                "failed to read workspace root directory {}: {}",
                workspace_root.display(),
                err
            )));
        }

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

        // 3. Git repository state (including worktrees and submodules)
        let git_dir = workspace_root.join(".git");
        let has_git_dir = git_dir.exists();
        let in_worktree = if has_git_dir {
            true
        } else {
            run_cmd_in_dir(workspace_root, "git", &["rev-parse", "--show-toplevel"])
                .map(|s| {
                    let top_level = PathBuf::from(s.trim());
                    match (top_level.canonicalize(), workspace_root.canonicalize()) {
                        (Ok(top), Ok(root)) => top == root,
                        _ => top_level == workspace_root,
                    }
                })
                .unwrap_or(false)
        };
        let has_git = in_worktree;

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

            let has_git_commits =
                run_cmd_in_dir(workspace_root, "git", &["rev-parse", "HEAD"]).is_some();
            facts.push(EnvironmentFact::new(
                EnvironmentCategory::Git,
                "git.has_commits",
                has_git_commits.to_string(),
                FactSource::SystemCommand,
                Confidence::Certain,
            ));
        }

        // 4. Evidence-driven recursive filesystem scan
        let mut evidence = WorkspaceEvidence::default();
        scan_workspace_recursive(workspace_root, 0, 5, &mut evidence, &mut facts)?;

        facts.push(EnvironmentFact::new(
            EnvironmentCategory::ProjectStructure,
            "structure.file_count",
            evidence.total_files.to_string(),
            FactSource::FilesystemScan,
            Confidence::Certain,
        ));

        let is_brownfield = evidence.manifest_files > 0
            || evidence.code_files > 0
            || evidence.ci_or_build_files > 0
            || evidence.has_generated_or_vendor_dirs
            || evidence.has_nested_git
            || (evidence.source_dirs > 0 && evidence.total_files > 0)
            || (evidence.total_files > evidence.doc_files);

        let workspace_mode = if is_brownfield {
            WorkspaceMode::Brownfield
        } else {
            WorkspaceMode::Greenfield
        };

        let detected_mode = match workspace_mode {
            WorkspaceMode::Greenfield => GenesisMode::Greenfield,
            WorkspaceMode::Brownfield => GenesisMode::Brownfield,
            WorkspaceMode::Unknown => GenesisMode::AutoDetect,
        };

        let detected_stack = if !evidence.detected_stacks.is_empty() {
            Some(evidence.detected_stacks.join(", "))
        } else {
            None
        };

        Ok(Self {
            workspace_root: workspace_root.to_path_buf(),
            facts,
            detected_mode,
            workspace_mode,
            detected_stack,
            file_count: evidence.total_files,
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
        self.workspace_mode == WorkspaceMode::Greenfield
    }

    /// Whether this workspace was determined to be brownfield.
    pub fn is_brownfield(&self) -> bool {
        self.workspace_mode == WorkspaceMode::Brownfield
    }

    /// Whether this workspace mode is unverified or unknown.
    pub fn is_unknown(&self) -> bool {
        self.workspace_mode == WorkspaceMode::Unknown
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

/// Observable evidence collected from workspace filesystem inspection.
#[derive(Debug, Default)]
pub struct WorkspaceEvidence {
    pub total_files: usize,
    pub doc_files: usize,
    pub code_files: usize,
    pub manifest_files: usize,
    pub ci_or_build_files: usize,
    pub source_dirs: usize,
    pub has_generated_or_vendor_dirs: bool,
    pub has_nested_git: bool,
    pub detected_stacks: Vec<String>,
}

fn scan_workspace_recursive(
    dir: &Path,
    current_depth: usize,
    max_depth: usize,
    evidence: &mut WorkspaceEvidence,
    facts: &mut Vec<EnvironmentFact>,
) -> Result<(), GenesisError> {
    let read_dir = match std::fs::read_dir(dir) {
        Ok(rd) => rd,
        Err(err) => {
            if current_depth == 0 {
                return Err(GenesisError::EnvironmentProbeFailed(format!(
                    "failed to read workspace root directory {}: {}",
                    dir.display(),
                    err
                )));
            } else {
                facts.push(EnvironmentFact::new(
                    EnvironmentCategory::ProjectStructure,
                    format!(
                        "unreadable_dir.{}",
                        dir.file_name().unwrap_or_default().to_string_lossy()
                    ),
                    dir.to_string_lossy().to_string(),
                    FactSource::FilesystemScan,
                    Confidence::Certain,
                ));
                return Ok(());
            }
        }
    };

    for entry in read_dir.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();

        if path.is_dir() {
            if name == ".git" {
                if current_depth > 0 {
                    evidence.has_nested_git = true;
                }
                continue;
            }

            if name == ".m31a" {
                continue;
            }

            if name == "target"
                || name == "node_modules"
                || name == "vendor"
                || name == ".venv"
                || name == "venv"
                || name == "__pycache__"
                || name == ".next"
                || name == "dist"
                || name == "build"
                || name == ".cache"
                || name == ".turbo"
                || name == ".gradle"
            {
                evidence.has_generated_or_vendor_dirs = true;
                continue;
            }

            match name.as_str() {
                "src" | "lib" | "pkg" | "cmd" | "app" | "apps" | "packages" | "crates"
                | "services" | "modules" | "core" | "internal" | "components" | "pages" => {
                    evidence.source_dirs += 1;
                }
                ".github" | ".circleci" => {
                    evidence.ci_or_build_files += 1;
                    facts.push(EnvironmentFact::new(
                        EnvironmentCategory::BuildSystem,
                        "ci.provider",
                        name.clone(),
                        FactSource::FilesystemScan,
                        Confidence::Certain,
                    ));
                }
                _ => {}
            }

            if current_depth < max_depth {
                scan_workspace_recursive(&path, current_depth + 1, max_depth, evidence, facts)?;
            }
        } else if path.is_file() {
            evidence.total_files += 1;

            match name.as_str() {
                "Cargo.toml" => {
                    evidence.manifest_files += 1;
                    if !evidence
                        .detected_stacks
                        .contains(&"Rust (Cargo)".to_string())
                    {
                        evidence.detected_stacks.push("Rust (Cargo)".to_string());
                    }
                    facts.push(EnvironmentFact::new(
                        EnvironmentCategory::DependencyManifest,
                        "manifest.cargo",
                        "Cargo.toml",
                        FactSource::FilesystemScan,
                        Confidence::Certain,
                    ));
                }
                "Cargo.lock" => {
                    evidence.manifest_files += 1;
                }
                "package.json" => {
                    evidence.manifest_files += 1;
                    if !evidence
                        .detected_stacks
                        .contains(&"Node.js (npm/pnpm/yarn)".to_string())
                    {
                        evidence
                            .detected_stacks
                            .push("Node.js (npm/pnpm/yarn)".to_string());
                    }
                    facts.push(EnvironmentFact::new(
                        EnvironmentCategory::DependencyManifest,
                        "manifest.package_json",
                        "package.json",
                        FactSource::FilesystemScan,
                        Confidence::Certain,
                    ));
                }
                "package-lock.json" | "yarn.lock" | "pnpm-lock.yaml" | "bun.lockb"
                | "tsconfig.json" => {
                    evidence.manifest_files += 1;
                }
                "go.mod" => {
                    evidence.manifest_files += 1;
                    if !evidence.detected_stacks.contains(&"Go".to_string()) {
                        evidence.detected_stacks.push("Go".to_string());
                    }
                    facts.push(EnvironmentFact::new(
                        EnvironmentCategory::DependencyManifest,
                        "manifest.go_mod",
                        "go.mod",
                        FactSource::FilesystemScan,
                        Confidence::Certain,
                    ));
                }
                "go.sum" => {
                    evidence.manifest_files += 1;
                }
                "pyproject.toml" | "requirements.txt" | "setup.py" | "setup.cfg" | "Pipfile"
                | "poetry.lock" => {
                    evidence.manifest_files += 1;
                    if !evidence.detected_stacks.contains(&"Python".to_string()) {
                        evidence.detected_stacks.push("Python".to_string());
                    }
                    facts.push(EnvironmentFact::new(
                        EnvironmentCategory::DependencyManifest,
                        format!("manifest.{}", name.replace('.', "_")),
                        name.clone(),
                        FactSource::FilesystemScan,
                        Confidence::Certain,
                    ));
                }
                "pom.xml" | "build.gradle" | "build.gradle.kts" | "settings.gradle" => {
                    evidence.manifest_files += 1;
                    if !evidence.detected_stacks.contains(&"Java/JVM".to_string()) {
                        evidence.detected_stacks.push("Java/JVM".to_string());
                    }
                    facts.push(EnvironmentFact::new(
                        EnvironmentCategory::DependencyManifest,
                        format!("manifest.{}", name.replace('.', "_")),
                        name.clone(),
                        FactSource::FilesystemScan,
                        Confidence::Certain,
                    ));
                }
                "Gemfile" | "Gemfile.lock" => {
                    evidence.manifest_files += 1;
                    if !evidence.detected_stacks.contains(&"Ruby".to_string()) {
                        evidence.detected_stacks.push("Ruby".to_string());
                    }
                }
                "composer.json" | "composer.lock" => {
                    evidence.manifest_files += 1;
                    if !evidence.detected_stacks.contains(&"PHP".to_string()) {
                        evidence.detected_stacks.push("PHP".to_string());
                    }
                }
                "mix.exs" | "mix.lock" => {
                    evidence.manifest_files += 1;
                    if !evidence.detected_stacks.contains(&"Elixir".to_string()) {
                        evidence.detected_stacks.push("Elixir".to_string());
                    }
                }
                "Package.swift" => {
                    evidence.manifest_files += 1;
                    if !evidence.detected_stacks.contains(&"Swift".to_string()) {
                        evidence.detected_stacks.push("Swift".to_string());
                    }
                }
                "CMakeLists.txt" | "meson.build" => {
                    evidence.manifest_files += 1;
                    if !evidence.detected_stacks.contains(&"C/C++".to_string()) {
                        evidence.detected_stacks.push("C/C++".to_string());
                    }
                }
                "Makefile" | "makefile" | "GNUmakefile" => {
                    evidence.ci_or_build_files += 1;
                    facts.push(EnvironmentFact::new(
                        EnvironmentCategory::BuildSystem,
                        "build.makefile",
                        name.clone(),
                        FactSource::FilesystemScan,
                        Confidence::Certain,
                    ));
                }
                "Dockerfile"
                | "Containerfile"
                | "docker-compose.yml"
                | "docker-compose.yaml"
                | "compose.yaml"
                | "compose.yml" => {
                    evidence.ci_or_build_files += 1;
                    facts.push(EnvironmentFact::new(
                        EnvironmentCategory::BuildSystem,
                        "build.dockerfile",
                        name.clone(),
                        FactSource::FilesystemScan,
                        Confidence::Certain,
                    ));
                }
                ".gitlab-ci.yml" | "azure-pipelines.yml" | "Jenkinsfile" => {
                    evidence.ci_or_build_files += 1;
                    facts.push(EnvironmentFact::new(
                        EnvironmentCategory::BuildSystem,
                        "ci.config",
                        name.clone(),
                        FactSource::FilesystemScan,
                        Confidence::Certain,
                    ));
                }
                "README.md" | "README" | "README.txt" | "readme.md" | "readme.rst" => {
                    evidence.doc_files += 1;
                    facts.push(EnvironmentFact::new(
                        EnvironmentCategory::Documentation,
                        "doc.readme",
                        name.clone(),
                        FactSource::FilesystemScan,
                        Confidence::Certain,
                    ));
                }
                "LICENSE" | "LICENCE" | "LICENSE.txt" | "LICENSE.md" | ".gitignore"
                | ".gitattributes" | ".editorconfig" | ".gitkeep" => {
                    evidence.doc_files += 1;
                }
                _ => {
                    if name.ends_with(".csproj")
                        || name.ends_with(".sln")
                        || name.ends_with(".fsproj")
                    {
                        evidence.manifest_files += 1;
                        if !evidence.detected_stacks.contains(&".NET".to_string()) {
                            evidence.detected_stacks.push(".NET".to_string());
                        }
                    } else {
                        let is_code_file =
                            path.extension()
                                .and_then(|e| e.to_str())
                                .is_some_and(|ext| {
                                    matches!(
                                        ext,
                                        "rs" | "js"
                                            | "ts"
                                            | "jsx"
                                            | "tsx"
                                            | "py"
                                            | "go"
                                            | "c"
                                            | "cpp"
                                            | "cc"
                                            | "cxx"
                                            | "h"
                                            | "hpp"
                                            | "java"
                                            | "kt"
                                            | "kts"
                                            | "rb"
                                            | "php"
                                            | "cs"
                                            | "swift"
                                            | "scala"
                                            | "erl"
                                            | "ex"
                                            | "exs"
                                            | "hs"
                                            | "lua"
                                            | "sh"
                                            | "bash"
                                            | "zsh"
                                            | "sql"
                                            | "html"
                                            | "css"
                                            | "scss"
                                            | "vue"
                                            | "svelte"
                                            | "dart"
                                            | "zig"
                                    )
                                });
                        if is_code_file {
                            evidence.code_files += 1;
                        }
                    }
                }
            }
        }
    }

    Ok(())
}
