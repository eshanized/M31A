//! Workspace Discovery and Safe Bounded Filesystem Traversal Engine (`m31a-ast::workspace`).
//!
//! Identifies repository roots, project manifests, packages, monorepo boundaries,
//! and executes safe, cycle-protected, bounded traversal with explicit completion status.

use crate::types::{IndexStatus, LanguageId};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

/// Package dependency discovered in project manifest.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PackageDependency {
    pub name: String,
    pub version_req: Option<String>,
    pub is_dev: bool,
    pub is_internal: bool,
    pub path: Option<PathBuf>,
}

/// Package or subproject discovered within the workspace.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorkspacePackage {
    pub id: String,
    pub name: String,
    pub version: Option<String>,
    pub language: LanguageId,
    pub manifest_path: PathBuf,
    pub package_root: PathBuf,
    pub source_dirs: Vec<PathBuf>,
    pub test_dirs: Vec<PathBuf>,
    pub dependencies: Vec<PackageDependency>,
    pub is_root: bool,
}

/// Manifest file types recognized by the discovery engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ManifestKind {
    CargoToml,
    PackageJson,
    TsConfig,
    PyprojectToml,
    RequirementsTxt,
    GoMod,
    CMakeLists,
    Makefile,
    PomXml,
    BuildGradle,
    DotnetProj,
}

/// Discovered project manifest.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DiscoveredManifest {
    pub kind: ManifestKind,
    pub path: PathBuf,
    pub relative_path: PathBuf,
    pub package_name: Option<String>,
    pub version: Option<String>,
}

/// Complete inventory of discovered workspace structure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceInventory {
    pub root_path: PathBuf,
    pub repo_name: String,
    pub git_branch: Option<String>,
    pub git_head_sha: Option<String>,
    pub detected_languages: Vec<LanguageId>,
    pub manifests: Vec<DiscoveredManifest>,
    pub packages: Vec<WorkspacePackage>,
    pub is_monorepo: bool,
}

/// Traversal configuration with resource bounds.
#[derive(Debug, Clone)]
pub struct TraversalOptions {
    pub max_files: usize,
    pub max_depth: usize,
    pub max_file_size_bytes: u64,
    pub follow_symlinks: bool,
    pub extra_ignore_patterns: Vec<String>,
}

impl Default for TraversalOptions {
    fn default() -> Self {
        Self {
            max_files: 50_000,
            max_depth: 32,
            max_file_size_bytes: 2 * 1024 * 1024, // 2 MiB
            follow_symlinks: false,
            extra_ignore_patterns: Vec::new(),
        }
    }
}

/// Result of scanning the workspace files.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScannedFile {
    pub path: PathBuf,
    pub relative_path: PathBuf,
    pub language: LanguageId,
    pub size_bytes: u64,
    pub is_test: bool,
    pub package_id: Option<String>,
}

/// Diagnostic or skip report during traversal.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraversalSkip {
    pub path: PathBuf,
    pub reason: String,
}

/// Completion report for filesystem scanning.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraversalReport {
    pub status: IndexStatus,
    pub total_dirs_visited: usize,
    pub files_scanned: usize,
    pub skipped_files_count: usize,
    pub binary_files_ignored: usize,
    pub diagnostics: Vec<TraversalSkip>,
    pub files: Vec<ScannedFile>,
}

pub struct WorkspaceDiscovery;

impl WorkspaceDiscovery {
    /// Discovers workspace manifests, packages, language breakdown, and Git identity.
    pub fn discover_workspace(root: &Path) -> WorkspaceInventory {
        let repo_name = root
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("workspace")
            .to_string();

        let (git_branch, git_head_sha) = Self::read_git_metadata(root);
        let manifests = Self::find_manifests(root);

        let mut packages = Vec::new();
        let mut languages_set = HashSet::new();

        for m in &manifests {
            match m.kind {
                ManifestKind::CargoToml => {
                    languages_set.insert(LanguageId::Rust);
                    Self::parse_cargo_manifest(root, &m.path, &mut packages);
                }
                ManifestKind::PackageJson => {
                    languages_set.insert(LanguageId::TypeScript);
                    languages_set.insert(LanguageId::JavaScript);
                    Self::parse_npm_manifest(root, &m.path, &mut packages);
                }
                ManifestKind::PyprojectToml | ManifestKind::RequirementsTxt => {
                    languages_set.insert(LanguageId::Python);
                    Self::parse_python_manifest(root, &m.path, &mut packages);
                }
                ManifestKind::GoMod => {
                    languages_set.insert(LanguageId::Go);
                    Self::parse_go_manifest(root, &m.path, &mut packages);
                }
                ManifestKind::CMakeLists | ManifestKind::Makefile => {
                    languages_set.insert(LanguageId::Cpp);
                    languages_set.insert(LanguageId::C);
                }
                ManifestKind::PomXml | ManifestKind::BuildGradle => {
                    languages_set.insert(LanguageId::Java);
                }
                ManifestKind::DotnetProj => {
                    languages_set.insert(LanguageId::CSharp);
                }
                _ => {}
            }
        }

        // Deduplicate packages by package_root
        packages.sort_by(|a, b| a.package_root.cmp(&b.package_root));
        packages.dedup_by(|a, b| a.package_root == b.package_root);

        let is_monorepo = packages.len() > 1;

        let mut detected_languages: Vec<LanguageId> = languages_set.into_iter().collect();
        detected_languages.sort_by_key(|l| l.as_str());

        WorkspaceInventory {
            root_path: root.to_path_buf(),
            repo_name,
            git_branch,
            git_head_sha,
            detected_languages,
            manifests,
            packages,
            is_monorepo,
        }
    }

    /// Performs safe, bounded filesystem traversal starting at root.
    pub fn scan_files(root: &Path, options: &TraversalOptions) -> TraversalReport {
        let inventory = Self::discover_workspace(root);
        let mut scanned_files = Vec::new();
        let mut skips = Vec::new();
        let mut visited_dirs: HashSet<PathBuf> = HashSet::new();
        let mut binary_count = 0;
        let mut status = IndexStatus::Complete;

        let mut queue: Vec<(PathBuf, usize)> = vec![(root.to_path_buf(), 0)];

        while let Some((dir, depth)) = queue.pop() {
            if depth > options.max_depth {
                skips.push(TraversalSkip {
                    path: dir.clone(),
                    reason: format!("Exceeded maximum directory walk depth of {}", options.max_depth),
                });
                status = IndexStatus::Partial;
                continue;
            }

            // Symlink loop / cycle prevention via canonicalization
            let canonical_dir = match fs::canonicalize(&dir) {
                Ok(c) => c,
                Err(e) => {
                    skips.push(TraversalSkip {
                        path: dir.clone(),
                        reason: format!("Could not canonicalize directory: {}", e),
                    });
                    continue;
                }
            };

            if visited_dirs.contains(&canonical_dir) {
                skips.push(TraversalSkip {
                    path: dir.clone(),
                    reason: "Detected symlink loop or already visited directory".to_string(),
                });
                continue;
            }
            visited_dirs.insert(canonical_dir);

            let entries = match fs::read_dir(&dir) {
                Ok(e) => e,
                Err(e) => {
                    skips.push(TraversalSkip {
                        path: dir.clone(),
                        reason: format!("Permission or I/O error reading directory: {}", e),
                    });
                    status = IndexStatus::Partial;
                    continue;
                }
            };

            for entry in entries.filter_map(|e| e.ok()) {
                let path = entry.path();
                let file_name = entry.file_name();
                let name = file_name.to_string_lossy();

                // Standard and monorepo ignore filters
                if Self::is_ignored_directory_or_file(&name, &options.extra_ignore_patterns) {
                    continue;
                }

                let file_type = match entry.file_type() {
                    Ok(ft) => ft,
                    Err(_) => continue,
                };

                if file_type.is_dir() {
                    queue.push((path, depth + 1));
                } else if file_type.is_symlink() && !options.follow_symlinks {
                    skips.push(TraversalSkip {
                        path,
                        reason: "Symlink ignored by policy".to_string(),
                    });
                } else if file_type.is_file() {
                    // Check file size bounds
                    let meta = match entry.metadata() {
                        Ok(m) => m,
                        Err(_) => continue,
                    };

                    let size = meta.len();
                    if size > options.max_file_size_bytes {
                        skips.push(TraversalSkip {
                            path,
                            reason: format!(
                                "File size ({} bytes) exceeds limit ({} bytes)",
                                size, options.max_file_size_bytes
                            ),
                        });
                        continue;
                    }

                    // Check if supported language
                    let lang = LanguageId::from_path(&path);
                    if lang == LanguageId::Unknown {
                        continue;
                    }

                    // Check for binary file
                    if Self::is_binary_file(&path) {
                        binary_count += 1;
                        continue;
                    }

                    let rel_path = path.strip_prefix(root).unwrap_or(&path).to_path_buf();
                    let is_test = Self::is_test_file(&path, &rel_path);

                    // Associate with discovered package
                    let package_id = inventory
                        .packages
                        .iter()
                        .find(|p| path.starts_with(&p.package_root))
                        .map(|p| p.id.clone());

                    scanned_files.push(ScannedFile {
                        path,
                        relative_path: rel_path,
                        language: lang,
                        size_bytes: size,
                        is_test,
                        package_id,
                    });

                    if scanned_files.len() >= options.max_files {
                        status = IndexStatus::Partial;
                        skips.push(TraversalSkip {
                            path: root.to_path_buf(),
                            reason: format!(
                                "Reached maximum file scan budget of {} files",
                                options.max_files
                            ),
                        });
                        break;
                    }
                }
            }

            if scanned_files.len() >= options.max_files {
                break;
            }
        }

        TraversalReport {
            status,
            total_dirs_visited: visited_dirs.len(),
            files_scanned: scanned_files.len(),
            skipped_files_count: skips.len(),
            binary_files_ignored: binary_count,
            diagnostics: skips,
            files: scanned_files,
        }
    }

    /// Reads git HEAD commit SHA and branch name if available.
    fn read_git_metadata(root: &Path) -> (Option<String>, Option<String>) {
        let git_dir = root.join(".git");
        let head_path = git_dir.join("HEAD");
        if !head_path.exists() {
            return (None, None);
        }

        if let Ok(head_content) = fs::read_to_string(&head_path) {
            let trimmed = head_content.trim();
            if let Some(branch_ref) = trimmed.strip_prefix("ref: ") {
                let branch_name = branch_ref
                    .strip_prefix("refs/heads/")
                    .unwrap_or(branch_ref)
                    .to_string();

                let ref_path = git_dir.join(branch_ref);
                let sha = fs::read_to_string(&ref_path)
                    .ok()
                    .map(|s| s.trim().to_string());
                return (Some(branch_name), sha);
            } else if trimmed.len() >= 40 {
                // Detached HEAD
                return (None, Some(trimmed.to_string()));
            }
        }

        (None, None)
    }

    /// Finds project manifests throughout the repository tree.
    fn find_manifests(root: &Path) -> Vec<DiscoveredManifest> {
        let mut manifests = Vec::new();
        let mut dirs = vec![root.to_path_buf()];

        while let Some(dir) = dirs.pop() {
            if let Ok(entries) = fs::read_dir(&dir) {
                for entry in entries.filter_map(|e| e.ok()) {
                    let path = entry.path();
                    let name = entry.file_name().to_string_lossy().to_string();

                    if Self::is_ignored_directory_or_file(&name, &[]) {
                        continue;
                    }

                    if path.is_dir() {
                        // Don't recurse excessively deep for manifest discovery
                        if path.components().count() - root.components().count() <= 6 {
                            dirs.push(path);
                        }
                    } else if path.is_file() {
                        let kind = match name.as_str() {
                            "Cargo.toml" => Some(ManifestKind::CargoToml),
                            "package.json" => Some(ManifestKind::PackageJson),
                            "tsconfig.json" => Some(ManifestKind::TsConfig),
                            "pyproject.toml" => Some(ManifestKind::PyprojectToml),
                            "requirements.txt" => Some(ManifestKind::RequirementsTxt),
                            "go.mod" => Some(ManifestKind::GoMod),
                            "CMakeLists.txt" => Some(ManifestKind::CMakeLists),
                            "Makefile" => Some(ManifestKind::Makefile),
                            "pom.xml" => Some(ManifestKind::PomXml),
                            "build.gradle" | "build.gradle.kts" => Some(ManifestKind::BuildGradle),
                            n if n.ends_with(".csproj") || n.ends_with(".sln") => {
                                Some(ManifestKind::DotnetProj)
                            }
                            _ => None,
                        };

                        if let Some(k) = kind {
                            let rel = path.strip_prefix(root).unwrap_or(&path).to_path_buf();
                            manifests.push(DiscoveredManifest {
                                kind: k,
                                path: path.clone(),
                                relative_path: rel,
                                package_name: None,
                                version: None,
                            });
                        }
                    }
                }
            }
        }

        manifests
    }

    /// Parses Cargo.toml (root, workspaces, and member crates).
    fn parse_cargo_manifest(root: &Path, manifest_path: &Path, packages: &mut Vec<WorkspacePackage>) {
        let content = match fs::read_to_string(manifest_path) {
            Ok(c) => c,
            Err(_) => return,
        };

        let parsed: toml::Value = match toml::from_str(&content) {
            Ok(v) => v,
            Err(_) => return,
        };

        let package_root = manifest_path.parent().unwrap_or(root).to_path_buf();
        let is_root = &package_root == root;

        // Check if [package] is present
        if let Some(pkg) = parsed.get("package") {
            let name = pkg
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or_else(|| {
                    package_root
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("crate")
                })
                .to_string();
            let version = pkg.get("version").and_then(|v| v.as_str()).map(|s| s.to_string());

            let mut dependencies = Vec::new();
            if let Some(deps) = parsed.get("dependencies").and_then(|d| d.as_table()) {
                for (dep_name, dep_val) in deps {
                    let mut is_internal = false;
                    let mut path = None;
                    let mut version_req = None;

                    if let Some(table) = dep_val.as_table() {
                        if let Some(p) = table.get("path").and_then(|v| v.as_str()) {
                            is_internal = true;
                            path = Some(package_root.join(p));
                        }
                        version_req = table.get("version").and_then(|v| v.as_str()).map(|s| s.to_string());
                    } else if let Some(v) = dep_val.as_str() {
                        version_req = Some(v.to_string());
                    }

                    dependencies.push(PackageDependency {
                        name: dep_name.clone(),
                        version_req,
                        is_dev: false,
                        is_internal,
                        path,
                    });
                }
            }

            packages.push(WorkspacePackage {
                id: format!("cargo:{}", name),
                name,
                version,
                language: LanguageId::Rust,
                manifest_path: manifest_path.to_path_buf(),
                package_root: package_root.clone(),
                source_dirs: vec![package_root.join("src")],
                test_dirs: vec![package_root.join("tests"), package_root.join("src")],
                dependencies,
                is_root,
            });
        }

        // Check for workspace members: [workspace.members]
        if let Some(ws) = parsed.get("workspace")
            && let Some(members) = ws.get("members").and_then(|m| m.as_array())
        {
            for m in members {
                if let Some(pattern) = m.as_str() {
                    let member_dir = package_root.join(pattern);
                    let member_manifest = member_dir.join("Cargo.toml");
                    if member_manifest.exists() && member_manifest != manifest_path {
                        Self::parse_cargo_manifest(root, &member_manifest, packages);
                    }
                }
            }
        }
    }

    /// Parses package.json for Node.js / TypeScript.
    fn parse_npm_manifest(root: &Path, manifest_path: &Path, packages: &mut Vec<WorkspacePackage>) {
        let content = match fs::read_to_string(manifest_path) {
            Ok(c) => c,
            Err(_) => return,
        };

        let parsed: serde_json::Value = match serde_json::from_str(&content) {
            Ok(v) => v,
            Err(_) => return,
        };

        let package_root = manifest_path.parent().unwrap_or(root).to_path_buf();
        let is_root = &package_root == root;

        let name = parsed
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or_else(|| {
                package_root
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("npm_package")
            })
            .to_string();

        let version = parsed.get("version").and_then(|v| v.as_str()).map(|s| s.to_string());

        let mut dependencies = Vec::new();
        if let Some(deps) = parsed.get("dependencies").and_then(|d| d.as_object()) {
            for (dep_name, dep_ver) in deps {
                dependencies.push(PackageDependency {
                    name: dep_name.clone(),
                    version_req: dep_ver.as_str().map(|s| s.to_string()),
                    is_dev: false,
                    is_internal: dep_ver.as_str().map(|s| s.starts_with("workspace:")).unwrap_or(false),
                    path: None,
                });
            }
        }

        if let Some(dev_deps) = parsed.get("devDependencies").and_then(|d| d.as_object()) {
            for (dep_name, dep_ver) in dev_deps {
                dependencies.push(PackageDependency {
                    name: dep_name.clone(),
                    version_req: dep_ver.as_str().map(|s| s.to_string()),
                    is_dev: true,
                    is_internal: dep_ver.as_str().map(|s| s.starts_with("workspace:")).unwrap_or(false),
                    path: None,
                });
            }
        }

        packages.push(WorkspacePackage {
            id: format!("npm:{}", name),
            name,
            version,
            language: LanguageId::TypeScript,
            manifest_path: manifest_path.to_path_buf(),
            package_root: package_root.clone(),
            source_dirs: vec![package_root.join("src"), package_root.join("lib")],
            test_dirs: vec![package_root.join("tests"), package_root.join("__tests__")],
            dependencies,
            is_root,
        });
    }

    /// Parses Python pyproject.toml or requirements.txt.
    fn parse_python_manifest(root: &Path, manifest_path: &Path, packages: &mut Vec<WorkspacePackage>) {
        let package_root = manifest_path.parent().unwrap_or(root).to_path_buf();
        let is_root = &package_root == root;

        let mut name = package_root
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("python_package")
            .to_string();
        let mut version = None;

        if let Ok(content) = fs::read_to_string(manifest_path) {
            if let Ok(toml_val) = toml::from_str::<toml::Value>(&content) {
                if let Some(project) = toml_val.get("project") {
                    if let Some(n) = project.get("name").and_then(|v| v.as_str()) {
                        name = n.to_string();
                    }
                    if let Some(v) = project.get("version").and_then(|v| v.as_str()) {
                        version = Some(v.to_string());
                    }
                } else if let Some(tool) = toml_val.get("tool").and_then(|t| t.get("poetry")) {
                    if let Some(n) = tool.get("name").and_then(|v| v.as_str()) {
                        name = n.to_string();
                    }
                    if let Some(v) = tool.get("version").and_then(|v| v.as_str()) {
                        version = Some(v.to_string());
                    }
                }
            }
        }

        packages.push(WorkspacePackage {
            id: format!("py:{}", name),
            name,
            version,
            language: LanguageId::Python,
            manifest_path: manifest_path.to_path_buf(),
            package_root: package_root.clone(),
            source_dirs: vec![package_root.clone(), package_root.join("src")],
            test_dirs: vec![package_root.join("tests")],
            dependencies: Vec::new(),
            is_root,
        });
    }

    /// Parses Go go.mod.
    fn parse_go_manifest(root: &Path, manifest_path: &Path, packages: &mut Vec<WorkspacePackage>) {
        let content = match fs::read_to_string(manifest_path) {
            Ok(c) => c,
            Err(_) => return,
        };

        let mut module_name = String::new();
        for line in content.lines() {
            let trimmed = line.trim();
            if let Some(rest) = trimmed.strip_prefix("module ") {
                module_name = rest.trim().to_string();
                break;
            }
        }

        let package_root = manifest_path.parent().unwrap_or(root).to_path_buf();
        let is_root = &package_root == root;

        if module_name.is_empty() {
            module_name = package_root
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("go_module")
                .to_string();
        }

        packages.push(WorkspacePackage {
            id: format!("go:{}", module_name),
            name: module_name,
            version: None,
            language: LanguageId::Go,
            manifest_path: manifest_path.to_path_buf(),
            package_root: package_root.clone(),
            source_dirs: vec![package_root.clone()],
            test_dirs: vec![package_root.clone()],
            dependencies: Vec::new(),
            is_root,
        });
    }

    /// Determines if a file is a binary file by inspecting initial bytes for null bytes.
    pub fn is_binary_file(path: &Path) -> bool {
        let mut file = match fs::File::open(path) {
            Ok(f) => f,
            Err(_) => return false,
        };

        use std::io::Read;
        let mut buffer = [0u8; 1024];
        let bytes_read = match file.read(&mut buffer) {
            Ok(n) => n,
            Err(_) => return false,
        };

        buffer[..bytes_read].contains(&0)
    }

    /// Determines whether a file path is a test file based on conventions.
    pub fn is_test_file(path: &Path, rel_path: &Path) -> bool {
        let _path_str = path.to_string_lossy().to_ascii_lowercase();
        let rel_str = rel_path.to_string_lossy().to_ascii_lowercase();

        if rel_str.starts_with("tests/")
            || rel_str.contains("/tests/")
            || rel_str.contains("/__tests__/")
            || rel_str.contains("/test/")
        {
            return true;
        }

        let file_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();

        file_name.ends_with("_test.go")
            || file_name.ends_with("_test.rs")
            || file_name.ends_with(".test.ts")
            || file_name.ends_with(".test.tsx")
            || file_name.ends_with(".test.js")
            || file_name.ends_with(".spec.ts")
            || file_name.ends_with(".spec.js")
            || file_name.starts_with("test_")
            || file_name.ends_with("_test.py")
    }

    /// Standard directory and file ignores across language ecosystems.
    pub fn is_ignored_directory_or_file(name: &str, extra_patterns: &[String]) -> bool {
        for pat in extra_patterns {
            if name == pat || name.contains(pat) {
                return true;
            }
        }

        // Always allow .github for workflows
        if name == ".github" {
            return false;
        }

        matches!(
            name,
            ".git"
                | "target"
                | "node_modules"
                | "dist"
                | "build"
                | "out"
                | ".next"
                | ".nuxt"
                | "__pycache__"
                | ".pytest_cache"
                | ".mypy_cache"
                | ".ruff_cache"
                | ".venv"
                | "venv"
                | "env"
                | ".tox"
                | ".idea"
                | ".vscode"
                | ".DS_Store"
                | "Thumbs.db"
                | "vendor"
                | ".cargo"
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_discover_cargo_workspace_self() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .to_path_buf();

        let inv = WorkspaceDiscovery::discover_workspace(&root);
        assert!(!inv.manifests.is_empty());
        assert!(inv.detected_languages.contains(&LanguageId::Rust));
        assert!(inv.is_monorepo);
        assert!(!inv.packages.is_empty());
    }

    #[test]
    fn test_scan_files_bounds() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let options = TraversalOptions {
            max_files: 50,
            max_depth: 5,
            ..Default::default()
        };
        let report = WorkspaceDiscovery::scan_files(&root, &options);
        assert!(!report.files.is_empty());
        assert!(report.files.len() <= 50);
        assert_eq!(report.status, IndexStatus::Complete);
    }

    #[test]
    fn test_binary_detection() {
        let temp_dir = tempfile::tempdir().unwrap();
        let bin_path = temp_dir.path().join("fake_binary.bin");
        fs::write(&bin_path, b"hello\x00world").unwrap();
        assert!(WorkspaceDiscovery::is_binary_file(&bin_path));

        let text_path = temp_dir.path().join("text.rs");
        fs::write(&text_path, b"fn main() {}").unwrap();
        assert!(!WorkspaceDiscovery::is_binary_file(&text_path));
    }

    #[test]
    fn test_is_test_file() {
        assert!(WorkspaceDiscovery::is_test_file(
            Path::new("/foo/tests/integration.rs"),
            Path::new("tests/integration.rs")
        ));
        assert!(WorkspaceDiscovery::is_test_file(
            Path::new("/foo/src/server_test.go"),
            Path::new("src/server_test.go")
        ));
        assert!(WorkspaceDiscovery::is_test_file(
            Path::new("/foo/src/test_auth.py"),
            Path::new("src/test_auth.py")
        ));
        assert!(WorkspaceDiscovery::is_test_file(
            Path::new("/foo/src/App.test.tsx"),
            Path::new("src/App.test.tsx")
        ));
        assert!(!WorkspaceDiscovery::is_test_file(
            Path::new("/foo/src/main.rs"),
            Path::new("src/main.rs")
        ));
    }
}
