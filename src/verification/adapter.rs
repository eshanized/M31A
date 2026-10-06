//! Verification Project Adapter (CFG-01..04).
//!
//! Provides language and build system adaptation across:
//! - Rust (`Cargo.toml`, `cargo check`, `cargo test`, `cargo clippy -- -D warnings`)
//! - Python (`pyproject.toml`, `python3 -m py_compile`, `pytest`, `ruff check`)
//! - Node (`package.json`, `npm run build`, `npm test`, `npm run lint`)
//! - Go (`go.mod`, `go vet ./...`, `go test ./...`, `golangci-lint run`)
//! - Custom (explicitly specified in `[workspace.verification]` configuration)

use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::config::schema::WorkspaceVerificationConfig;

/// Strongly typed project types supported by the verification subsystem.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectType {
    Rust,
    Python,
    Node,
    Go,
    Custom,
}

impl ProjectType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Rust => "rust",
            Self::Python => "python",
            Self::Node => "node",
            Self::Go => "go",
            Self::Custom => "custom",
        }
    }

    pub fn from_str_relaxed(s: &str) -> Option<Self> {
        match s.to_lowercase().trim() {
            "rust" | "rs" => Some(Self::Rust),
            "python" | "py" => Some(Self::Python),
            "node" | "javascript" | "typescript" | "js" | "ts" => Some(Self::Node),
            "go" | "golang" => Some(Self::Go),
            "custom" => Some(Self::Custom),
            _ => None,
        }
    }
}

/// Project adapter providing build, verification, and linting commands for a workspace.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectAdapter {
    pub project_type: ProjectType,
    pub manifest_file: String,
    pub source_dir: Option<String>,
    pub compiler_command: String,
    pub test_command: String,
    pub linter_command: String,
    pub timeout_secs: u64,
}

impl Default for ProjectAdapter {
    fn default() -> Self {
        Self::rust_defaults()
    }
}

impl ProjectAdapter {
    /// Declarative adapter definitions bundled with the application.
    ///
    /// The Rust code owns validation, loading, command safety, execution,
    /// containment, and supervision; the toolchain defaults live in
    /// `assets/verification_adapters/*.toml` and are loaded through this
    /// single path (no hardcoded command strings elsewhere).
    fn load_declared(project_type: ProjectType) -> Self {
        let toml_str: &str = match project_type {
            ProjectType::Rust => include_str!("../../assets/verification_adapters/rust.toml"),
            ProjectType::Python => {
                include_str!("../../assets/verification_adapters/python.toml")
            }
            ProjectType::Node => include_str!("../../assets/verification_adapters/node.toml"),
            ProjectType::Go => include_str!("../../assets/verification_adapters/go.toml"),
            ProjectType::Custom => {
                include_str!("../../assets/verification_adapters/custom.toml")
            }
        };
        Self::parse_declared(project_type, toml_str).expect("bundled adapter must parse")
    }

    fn parse_declared(project_type: ProjectType, toml_str: &str) -> Result<Self, String> {
        #[derive(serde::Deserialize)]
        struct Declared {
            adapter: DeclaredAdapter,
        }
        #[derive(serde::Deserialize)]
        struct DeclaredAdapter {
            manifest_file: String,
            source_dir: Option<String>,
            compiler_command: String,
            test_command: String,
            linter_command: String,
            timeout_secs: u64,
        }
        let declared: Declared =
            toml::from_str(toml_str).map_err(|e| format!("adapter parse: {e}"))?;
        let a = declared.adapter;
        // Command safety: bundled declarations pass through the single
        // process safety boundary (no shell metachars, no unsafe program).
        for cmd in [&a.compiler_command, &a.test_command, &a.linter_command] {
            if !cmd.trim().is_empty() {
                let parts: Vec<&str> = cmd.split_whitespace().collect();
                if parts.is_empty() {
                    return Err("bundled adapter command is empty".to_string());
                }
                let args: Vec<String> = parts[1..].iter().map(|s| s.to_string()).collect();
                crate::process::env::check_command_safety(parts[0], &args)
                    .map_err(|e| format!("bundled adapter command rejected: {e}"))?;
            }
        }
        Ok(Self {
            project_type,
            manifest_file: a.manifest_file,
            source_dir: a.source_dir,
            compiler_command: a.compiler_command,
            test_command: a.test_command,
            linter_command: a.linter_command,
            timeout_secs: a.timeout_secs,
        })
    }

    /// Registry-driven lookup: all built-in adapters resolve through
    /// [`Self::load_declared`]; no call site hardcodes toolchain commands.
    pub fn adapter_for(project_type: ProjectType) -> Self {
        Self::load_declared(project_type)
    }

    /// Default configuration for Rust projects (declarative asset).
    pub fn rust_defaults() -> Self {
        Self::load_declared(ProjectType::Rust)
    }

    /// Default configuration for Python projects (declarative asset).
    ///
    /// Note: the compiler command is intentionally empty. A bare
    /// `python3 -m py_compile` invocation can never succeed (it requires
    /// filename arguments and exits 2 without them), so defaulting to it
    /// would fail every Python task regardless of project health. An empty
    /// command is an explicit not-applicable verdict, never assumed success:
    /// projects needing compile checks declare them via
    /// `[workspace.verification]` config or task-declared strategies.
    pub fn python_defaults() -> Self {
        Self::load_declared(ProjectType::Python)
    }

    /// Default configuration for Node.js projects (declarative asset).
    pub fn node_defaults() -> Self {
        Self::load_declared(ProjectType::Node)
    }

    /// Default configuration for Go projects (declarative asset).
    pub fn go_defaults() -> Self {
        Self::load_declared(ProjectType::Go)
    }

    /// Default configuration for Custom projects (declarative asset).
    ///
    /// Empty commands are explicit not-applicable verdicts (see the
    /// verification runners): a workspace with no detected toolchain has
    /// nothing for a tier to execute, which is absence of evidence — never
    /// failure evidence and never assumed success.
    pub fn custom_defaults() -> Self {
        Self::load_declared(ProjectType::Custom)
    }

    /// Auto-detect project type and build commands from repository markers and configuration.
    pub fn detect(
        workspace_root: &Path,
        verification_config: Option<&WorkspaceVerificationConfig>,
        project_type_override: Option<&str>,
    ) -> Self {
        let detected_type = if let Some(override_type) = project_type_override {
            ProjectType::from_str_relaxed(override_type).unwrap_or(ProjectType::Custom)
        } else if workspace_root.join("Cargo.toml").exists() {
            ProjectType::Rust
        } else if workspace_root.join("pyproject.toml").exists()
            || workspace_root.join("setup.py").exists()
            || workspace_root.join("requirements.txt").exists()
        {
            ProjectType::Python
        } else if workspace_root.join("package.json").exists() {
            ProjectType::Node
        } else if workspace_root.join("go.mod").exists() {
            ProjectType::Go
        } else {
            // No language markers: the workspace has no detected toolchain.
            // Defaulting to Rust here would impose M31A's implementation
            // language on arbitrary targets (target-neutral verification).
            // Custom carries empty manifest/commands, which the runners
            // report as explicit not-applicable verdicts.
            ProjectType::Custom
        };

        let mut adapter = match detected_type {
            ProjectType::Rust => Self::rust_defaults(),
            ProjectType::Python => {
                let mut p = Self::python_defaults();
                if !workspace_root.join("pyproject.toml").exists() {
                    if workspace_root.join("setup.py").exists() {
                        p.manifest_file = "setup.py".to_string();
                    } else if workspace_root.join("requirements.txt").exists() {
                        p.manifest_file = "requirements.txt".to_string();
                    }
                }
                p
            }
            ProjectType::Node => Self::node_defaults(),
            ProjectType::Go => Self::go_defaults(),
            ProjectType::Custom => Self::custom_defaults(),
        };

        // Apply overrides from configuration if present
        if let Some(cfg) = verification_config {
            if let Some(ref m) = cfg.tier1_manifest {
                adapter.manifest_file = m.clone();
            }
            if let Some(ref c) = cfg.tier2_compiler {
                adapter.compiler_command = c.clone();
            }
            if let Some(ref t) = cfg.tier3_tests {
                adapter.test_command = t.clone();
            }
            if let Some(ref l) = cfg.tier4_linter {
                adapter.linter_command = l.clone();
            }
            if let Some(s) = cfg.timeout_secs {
                adapter.timeout_secs = s;
            }
        }

        adapter
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_adapter_auto_detection() {
        let dir = tempdir().unwrap();
        let path = dir.path();

        // No files -> undetected toolchain (Custom). Defaulting
        // to Rust here would impose M31A's implementation language on arbitrary
        // targets; Custom carries empty manifest/commands, which runners
        // report as explicit not-applicable verdicts.
        let ad = ProjectAdapter::detect(path, None, None);
        assert_eq!(ad.project_type, ProjectType::Custom);
        assert_eq!(ad.manifest_file, "");

        // Python detection
        std::fs::write(path.join("pyproject.toml"), "[project]\nname=\"test\"").unwrap();
        let ad_py = ProjectAdapter::detect(path, None, None);
        assert_eq!(ad_py.project_type, ProjectType::Python);
        // Bare `python3 -m py_compile` can never succeed (requires filename
        // arguments), so the default is intentionally empty: explicit
        // not-applicable rather than certain failure.
        assert_eq!(ad_py.compiler_command, "");

        // Config overrides
        let custom_cfg = WorkspaceVerificationConfig {
            tier1_manifest: Some("custom.json".into()),
            tier2_compiler: Some("make check".into()),
            tier3_tests: Some("make test".into()),
            tier4_linter: Some("make lint".into()),
            timeout_secs: Some(60),
        };
        let ad_custom = ProjectAdapter::detect(path, Some(&custom_cfg), None);
        assert_eq!(ad_custom.manifest_file, "custom.json");
        assert_eq!(ad_custom.compiler_command, "make check");
        assert_eq!(ad_custom.test_command, "make test");
        assert_eq!(ad_custom.linter_command, "make lint");
        assert_eq!(ad_custom.timeout_secs, 60);
    }
}
