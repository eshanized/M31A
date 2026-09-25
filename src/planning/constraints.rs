//! Deterministic static repository constraint discovery engine (PLN-01, D-07).
//!
//! Performs read-only inspection of repository manifests and directory layout
//! without executing shell commands or relying on heavyweight indexing.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

/// Strongly typed repository constraint discovered from filesystem manifests (D-07).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RepositoryConstraint {
    RustCargo {
        manifest_path: String,
        edition: Option<String>,
        has_workspace: bool,
    },
    NodePackage {
        manifest_path: String,
        manager: String,
        has_package_json: bool,
    },
    PythonProject {
        manifest_path: String,
        tool: String,
    },
    GoModule {
        manifest_path: String,
        module_path: Option<String>,
    },
    BuildSystem {
        system: String,
        file_path: String,
    },
    DirectoryLayout {
        layout_type: String,
        relative_path: String,
    },
}

impl RepositoryConstraint {
    pub fn description(&self) -> String {
        match self {
            Self::RustCargo {
                manifest_path,
                edition,
                has_workspace,
            } => format!(
                "Rust/Cargo manifest at {} (edition: {:?}, workspace: {})",
                manifest_path, edition, has_workspace
            ),
            Self::NodePackage {
                manifest_path,
                manager,
                ..
            } => {
                format!("Node.js manifest at {} using {}", manifest_path, manager)
            }
            Self::PythonProject {
                manifest_path,
                tool,
            } => {
                format!("Python manifest at {} using {}", manifest_path, tool)
            }
            Self::GoModule {
                manifest_path,
                module_path,
            } => {
                format!("Go module at {} (module: {:?})", manifest_path, module_path)
            }
            Self::BuildSystem { system, file_path } => {
                format!("Build system {} configuration at {}", system, file_path)
            }
            Self::DirectoryLayout {
                layout_type,
                relative_path,
            } => {
                format!(
                    "Standard directory layout '{}' at {}",
                    layout_type, relative_path
                )
            }
        }
    }
}

/// Static intake engine discovering repository constraints from disk (D-07).
pub struct ConstraintDiscoveryEngine;

impl ConstraintDiscoveryEngine {
    /// Discovers constraints within `repo_root` using read-only filesystem inspection.
    ///
    /// Implements ASVS Level 1 path-containment mitigation ensuring symlinks cannot
    /// escape `repo_root`.
    pub fn discover(repo_root: &Path) -> Vec<RepositoryConstraint> {
        let canonical_root = match repo_root.canonicalize() {
            Ok(root) => root,
            Err(_) => return Vec::new(),
        };

        let mut constraints = Vec::new();

        // 1. Rust / Cargo
        let cargo_path = canonical_root.join("Cargo.toml");
        if Self::is_safe_file(&canonical_root, &cargo_path)
            && let Ok(content) = fs::read_to_string(&cargo_path)
        {
            let has_workspace = content.contains("[workspace]");
            let edition = content
                .lines()
                .find(|l| l.trim().starts_with("edition"))
                .and_then(|l| l.split('=').nth(1))
                .map(|e| e.trim().trim_matches('"').to_string());

            constraints.push(RepositoryConstraint::RustCargo {
                manifest_path: "Cargo.toml".to_string(),
                edition,
                has_workspace,
            });
        }

        // 2. Node.js (package.json, lockfiles)
        let package_json_path = canonical_root.join("package.json");
        if Self::is_safe_file(&canonical_root, &package_json_path) {
            let manager =
                if Self::is_safe_file(&canonical_root, &canonical_root.join("pnpm-lock.yaml")) {
                    "pnpm".to_string()
                } else if Self::is_safe_file(&canonical_root, &canonical_root.join("yarn.lock")) {
                    "yarn".to_string()
                } else if Self::is_safe_file(
                    &canonical_root,
                    &canonical_root.join("package-lock.json"),
                ) {
                    "npm".to_string()
                } else {
                    "npm_or_compatible".to_string()
                };

            constraints.push(RepositoryConstraint::NodePackage {
                manifest_path: "package.json".to_string(),
                manager,
                has_package_json: true,
            });
        }

        // 3. Python (pyproject.toml, requirements.txt)
        let pyproject_path = canonical_root.join("pyproject.toml");
        if Self::is_safe_file(&canonical_root, &pyproject_path) {
            constraints.push(RepositoryConstraint::PythonProject {
                manifest_path: "pyproject.toml".to_string(),
                tool: "pyproject".to_string(),
            });
        } else {
            let req_path = canonical_root.join("requirements.txt");
            if Self::is_safe_file(&canonical_root, &req_path) {
                constraints.push(RepositoryConstraint::PythonProject {
                    manifest_path: "requirements.txt".to_string(),
                    tool: "pip".to_string(),
                });
            }
        }

        // 4. Go (go.mod)
        let go_mod_path = canonical_root.join("go.mod");
        if Self::is_safe_file(&canonical_root, &go_mod_path) {
            let module_path = fs::read_to_string(&go_mod_path).ok().and_then(|c| {
                c.lines()
                    .find(|l| l.trim().starts_with("module"))
                    .and_then(|l| l.split_whitespace().nth(1))
                    .map(|s| s.to_string())
            });

            constraints.push(RepositoryConstraint::GoModule {
                manifest_path: "go.mod".to_string(),
                module_path,
            });
        }

        // 5. Build Systems (Makefile, CMakeLists.txt, Justfile)
        if Self::is_safe_file(&canonical_root, &canonical_root.join("Makefile")) {
            constraints.push(RepositoryConstraint::BuildSystem {
                system: "make".to_string(),
                file_path: "Makefile".to_string(),
            });
        }
        if Self::is_safe_file(&canonical_root, &canonical_root.join("CMakeLists.txt")) {
            constraints.push(RepositoryConstraint::BuildSystem {
                system: "cmake".to_string(),
                file_path: "CMakeLists.txt".to_string(),
            });
        }
        if Self::is_safe_file(&canonical_root, &canonical_root.join("Justfile"))
            || Self::is_safe_file(&canonical_root, &canonical_root.join("justfile"))
        {
            constraints.push(RepositoryConstraint::BuildSystem {
                system: "just".to_string(),
                file_path: "Justfile".to_string(),
            });
        }

        // 6. Directory Layouts (src, tests, docs)
        for (dir_name, layout_type) in [
            ("src", "source_tree"),
            ("tests", "integration_tests"),
            ("docs", "documentation"),
        ] {
            let dir_path = canonical_root.join(dir_name);
            if Self::is_safe_dir(&canonical_root, &dir_path) {
                constraints.push(RepositoryConstraint::DirectoryLayout {
                    layout_type: layout_type.to_string(),
                    relative_path: dir_name.to_string(),
                });
            }
        }

        constraints
    }

    /// Validates that target path exists, is a regular file, and resides strictly inside canonical_root.
    fn is_safe_file(canonical_root: &Path, path: &Path) -> bool {
        if !path.is_file() {
            return false;
        }
        match path.canonicalize() {
            Ok(canonical) => canonical.starts_with(canonical_root),
            Err(_) => false,
        }
    }

    /// Validates that target path exists, is a directory, and resides strictly inside canonical_root.
    fn is_safe_dir(canonical_root: &Path, path: &Path) -> bool {
        if !path.is_dir() {
            return false;
        }
        match path.canonicalize() {
            Ok(canonical) => canonical.starts_with(canonical_root),
            Err(_) => false,
        }
    }
}

/// Repository planning evidence combining static manifest constraints with deep repository intelligence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepositoryPlanEvidence {
    pub constraints: Vec<RepositoryConstraint>,
    pub entry_points: Vec<crate::repo::types::EntryPoint>,
    pub subsystems: Vec<crate::repo::types::SubsystemKind>,
    pub known_test_files: Vec<String>,
}

impl RepositoryPlanEvidence {
    pub fn from_graph(
        constraints: Vec<RepositoryConstraint>,
        graph: &crate::repo::graph::RepositoryGraph,
    ) -> Self {
        let entry_points = graph.entry_points().to_vec();
        let mut subsystems = std::collections::BTreeSet::new();
        let mut known_test_files = Vec::new();

        for file in graph.get_all_files() {
            subsystems.insert(graph.get_file_subsystem(&file));
            if graph.get_file_classification(&file) == crate::repo::types::FileClassification::Test
            {
                known_test_files.push(file);
            }
        }

        Self {
            constraints,
            entry_points,
            subsystems: subsystems.into_iter().collect(),
            known_test_files,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_discover_mock_repository_constraints() {
        let dir = tempdir().expect("create temp dir");
        let root = dir.path();

        // Create Cargo.toml
        let cargo_content = r#"
[package]
name = "demo-pkg"
version = "0.1.0"
edition = "2021"

[workspace]
members = ["core"]
"#;
        fs::write(root.join("Cargo.toml"), cargo_content).unwrap();

        // Create package.json and pnpm-lock.yaml
        fs::write(root.join("package.json"), "{\"name\": \"demo-ui\"}").unwrap();
        fs::write(root.join("pnpm-lock.yaml"), "lockfileVersion: '6.0'").unwrap();

        // Create src and tests directories
        fs::create_dir(root.join("src")).unwrap();
        fs::create_dir(root.join("tests")).unwrap();

        let constraints = ConstraintDiscoveryEngine::discover(root);

        assert!(constraints.iter().any(|c| matches!(
            c,
            RepositoryConstraint::RustCargo { has_workspace: true, edition: Some(ed), .. } if ed == "2021"
        )));

        assert!(constraints.iter().any(|c| matches!(
            c,
            RepositoryConstraint::NodePackage { manager, .. } if manager == "pnpm"
        )));

        assert!(constraints.iter().any(|c| matches!(
            c,
            RepositoryConstraint::DirectoryLayout { relative_path, .. } if relative_path == "src"
        )));

        assert!(constraints.iter().any(|c| matches!(
            c,
            RepositoryConstraint::DirectoryLayout { relative_path, .. } if relative_path == "tests"
        )));
    }

    #[test]
    fn test_empty_or_nonexistent_repo_root() {
        let non_existent = Path::new("/path/does/not/exist/for/sure/12345");
        let res = ConstraintDiscoveryEngine::discover(non_existent);
        assert!(res.is_empty());
    }

    #[test]
    fn test_repository_plan_evidence() {
        let mut graph = crate::repo::graph::RepositoryGraph::new(1);
        graph.add_file("src/main.rs", "rust");
        graph.add_file("tests/it.rs", "rust");
        graph.set_file_classification("tests/it.rs", crate::repo::types::FileClassification::Test);
        graph.set_file_subsystem("src/main.rs", crate::repo::types::SubsystemKind::Kernel);

        let constraints = vec![RepositoryConstraint::DirectoryLayout {
            layout_type: "source_tree".to_string(),
            relative_path: "src".to_string(),
        }];

        let evidence = RepositoryPlanEvidence::from_graph(constraints, &graph);
        assert_eq!(evidence.constraints.len(), 1);
        assert!(
            evidence
                .subsystems
                .contains(&crate::repo::types::SubsystemKind::Kernel)
        );
        assert!(
            evidence
                .known_test_files
                .contains(&"tests/it.rs".to_string())
        );
    }
}
