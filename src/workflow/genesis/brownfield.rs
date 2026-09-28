//! Brownfield codebase scanning, topology mapping, and BROWNFIELD.md projection.

use super::errors::GenesisError;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Structural topology of an existing brownfield codebase.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodebaseTopology {
    pub root: PathBuf,
    pub primary_language: String,
    pub detected_frameworks: Vec<String>,
    pub entry_points: Vec<String>,
    pub module_tree: Vec<String>,
    pub test_frameworks: Vec<String>,
    pub ci_cd: Vec<String>,
    pub package_manifests: Vec<String>,
}

/// Comprehensive brownfield map detailing architecture, conventions, and delta scope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BrownfieldMap {
    pub topology: CodebaseTopology,
    pub existing_conventions: Vec<String>,
    pub subsystem_boundaries: Vec<String>,
    pub delta_scope: Option<String>,
}

impl BrownfieldMap {
    /// Scan a workspace directory and construct a BrownfieldMap.
    pub fn scan(workspace_root: &Path) -> Result<Self, GenesisError> {
        let mut primary_language = "Unknown".to_string();
        let frameworks = Vec::new();
        let mut entry_points = Vec::new();
        let mut module_tree = Vec::new();
        let mut test_frameworks = Vec::new();
        let mut ci_cd = Vec::new();
        let mut package_manifests = Vec::new();
        let mut conventions = Vec::new();
        let mut subsystems = Vec::new();

        if workspace_root.exists() {
            // Check Rust
            if workspace_root.join("Cargo.toml").exists() {
                primary_language = "Rust".to_string();
                package_manifests.push("Cargo.toml".to_string());
                test_frameworks.push("cargo test".to_string());
                conventions.push("Rust 2021/2024 edition idioms".to_string());
                conventions.push("clippy & rustfmt gating".to_string());

                if workspace_root.join("src/main.rs").exists() {
                    entry_points.push("src/main.rs".to_string());
                }
                if workspace_root.join("src/lib.rs").exists() {
                    entry_points.push("src/lib.rs".to_string());
                }
            }

            // Check Node/TS
            if workspace_root.join("package.json").exists() {
                if primary_language == "Unknown" {
                    primary_language = "TypeScript / JavaScript".to_string();
                }
                package_manifests.push("package.json".to_string());
                test_frameworks.push("npm test / jest / vitest".to_string());
            }

            // Check Python
            if workspace_root.join("pyproject.toml").exists()
                || workspace_root.join("requirements.txt").exists()
            {
                if primary_language == "Unknown" {
                    primary_language = "Python".to_string();
                }
                package_manifests.push("pyproject.toml / requirements.txt".to_string());
                test_frameworks.push("pytest".to_string());
            }

            // Check CI
            if workspace_root.join(".github/workflows").exists() {
                ci_cd.push("GitHub Actions".to_string());
            }
            if workspace_root.join(".gitlab-ci.yml").exists() {
                ci_cd.push("GitLab CI".to_string());
            }

            // Scan top-level source modules
            let src_dir = workspace_root.join("src");
            if let Ok(rd) = std::fs::read_dir(&src_dir) {
                for entry in rd.flatten() {
                    let fname = entry.file_name().to_string_lossy().to_string();
                    if entry.path().is_dir() {
                        module_tree.push(format!("src/{}/", fname));
                        subsystems.push(format!("src/{}", fname));
                    } else if fname.ends_with(".rs") && fname != "main.rs" && fname != "lib.rs" {
                        module_tree.push(format!("src/{}", fname));
                        subsystems.push(format!("src/{}", fname));
                    }
                }
            }
        }

        let topology = CodebaseTopology {
            root: workspace_root.to_path_buf(),
            primary_language,
            detected_frameworks: frameworks,
            entry_points,
            module_tree,
            test_frameworks,
            ci_cd,
            package_manifests,
        };

        Ok(Self {
            topology,
            existing_conventions: conventions,
            subsystem_boundaries: subsystems,
            delta_scope: None,
        })
    }

    /// Render into canonical markdown for `.planning/BROWNFIELD.md`.
    pub fn to_markdown(&self) -> String {
        let mut out = String::new();
        out.push_str("# Brownfield Codebase Map\n\n");

        out.push_str("## Primary Language & Ecosystem\n");
        out.push_str(&format!(
            "- **Language**: {}\n",
            self.topology.primary_language
        ));
        if !self.topology.package_manifests.is_empty() {
            out.push_str(&format!(
                "- **Manifests**: {}\n",
                self.topology.package_manifests.join(", ")
            ));
        }
        if !self.topology.entry_points.is_empty() {
            out.push_str(&format!(
                "- **Entry Points**: {}\n",
                self.topology.entry_points.join(", ")
            ));
        }
        out.push('\n');

        out.push_str("## Subsystem Boundaries & Modules\n");
        if self.topology.module_tree.is_empty() {
            out.push_str("- Root package flat layout\n");
        } else {
            for m in &self.topology.module_tree {
                out.push_str(&format!("- `{}`\n", m));
            }
        }
        out.push('\n');

        out.push_str("## Existing Conventions & Testing\n");
        for c in &self.existing_conventions {
            out.push_str(&format!("- {}\n", c));
        }
        for t in &self.topology.test_frameworks {
            out.push_str(&format!("- Test Runner: {}\n", t));
        }
        if !self.topology.ci_cd.is_empty() {
            out.push_str(&format!("- CI/CD: {}\n", self.topology.ci_cd.join(", ")));
        }
        out.push('\n');

        if let Some(ref delta) = self.delta_scope {
            out.push_str("## Delta Scope & Extension Targets\n");
            out.push_str(delta.trim());
            out.push_str("\n\n");
        }

        out
    }

    /// Persist to `<workspace_root>/<projection_dir>/BROWNFIELD.md`.
    pub fn save_to_dir(
        &self,
        workspace_root: &Path,
        projection_dir: &str,
    ) -> Result<PathBuf, GenesisError> {
        let dir = workspace_root.join(projection_dir);
        std::fs::create_dir_all(&dir)?;
        let file_path = dir.join("BROWNFIELD.md");
        std::fs::write(&file_path, self.to_markdown())?;
        Ok(file_path)
    }
}
