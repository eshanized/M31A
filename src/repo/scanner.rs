use crate::repo::extract::{
    ManifestExtractor, PythonExtractor, RustAstExtractor, TypeScriptExtractor,
};
use crate::repo::graph::RepositoryEdgeKind;
use crate::repo::types::{
    EntryPoint, EntryPointKind, FileClassification, RepositorySymbol, SubsystemKind,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

/// Maximum file size for full AST / regex parsing (2 MB).
/// Files exceeding this limit are indexed as files and classified, but AST extraction is skipped to avoid OOM.
pub const MAX_PARSE_SIZE_BYTES: u64 = 2 * 1024 * 1024;

/// Maximum directory traversal depth for repository scans: a read-only
/// walk must terminate even on deeply or adversarially nested trees.
pub const MAX_SCAN_DEPTH: usize = 24;

/// Maximum visited directory entries for repository scans: ensures bounded
/// resource consumption even on massive or pathological directories.
pub const MAX_SCAN_ENTRIES: usize = 100_000;

/// Detected programming language or file format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceLanguage {
    Rust,
    Python,
    TypeScript,
    JavaScript,
    Manifest,
    Other,
}

impl SourceLanguage {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Rust => "rust",
            Self::Python => "python",
            Self::TypeScript => "typescript",
            Self::JavaScript => "javascript",
            Self::Manifest => "manifest",
            Self::Other => "other",
        }
    }

    pub fn detect_from_path(path: &Path) -> Self {
        let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if file_name == "Cargo.toml"
            || file_name == "Cargo.lock"
            || file_name == "package.json"
            || file_name == "package-lock.json"
            || file_name == "pyproject.toml"
        {
            return Self::Manifest;
        }

        match path.extension().and_then(|e| e.to_str()) {
            Some("rs") => Self::Rust,
            Some("py") => Self::Python,
            Some("ts") | Some("tsx") => Self::TypeScript,
            Some("js") | Some("jsx") | Some("mjs") | Some("cjs") => Self::JavaScript,
            Some("toml") | Some("json") => Self::Manifest,
            _ => Self::Other,
        }
    }
}

/// Metadata and extracted symbols for a single scanned repository file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScannedFile {
    pub relative_path: String,
    pub language: SourceLanguage,
    pub content_hash: String,
    pub size_bytes: u64,
    pub classification: FileClassification,
    pub subsystem: Option<SubsystemKind>,
    pub symbols: Vec<RepositorySymbol>,
}

/// Discovered workspace topology, entry points, relationships, and symbol inventory.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ScanSummary {
    pub files: Vec<ScannedFile>,
    pub total_symbols: usize,
    pub scanned_file_count: usize,
    pub edges: Vec<(String, String, RepositoryEdgeKind)>,
    pub entry_points: Vec<EntryPoint>,
    pub ignored_file_count: usize,
}

/// Inspects content bytes for null byte `\0` to detect binary files within first 1024 bytes.
pub fn is_binary_content(sample: &[u8]) -> bool {
    let check_len = sample.len().min(1024);
    sample[..check_len].contains(&0)
}

/// Classifies a repository file into a structured `FileClassification`.
pub fn classify_file(
    relative_path: &str,
    is_binary: bool,
    content_sample: &[u8],
) -> FileClassification {
    if is_binary {
        return FileClassification::Binary;
    }

    let p = Path::new(relative_path);
    let file_name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
    let lower_name = file_name.to_lowercase();
    let norm_path = relative_path.replace('\\', "/");

    // 1. Build targets and package manifests
    if file_name == "Cargo.toml"
        || file_name == "Cargo.lock"
        || file_name == "package.json"
        || file_name == "package-lock.json"
        || file_name == "pnpm-lock.yaml"
        || file_name == "yarn.lock"
        || file_name == "pyproject.toml"
        || file_name == "requirements.txt"
        || file_name == "Makefile"
        || file_name == "Dockerfile"
        || file_name.starts_with("Dockerfile.")
        || file_name == "CMakeLists.txt"
        || file_name == "pom.xml"
        || file_name == "build.gradle"
    {
        return FileClassification::BuildTarget;
    }

    // 2. Tests
    if norm_path.starts_with("tests/")
        || norm_path.contains("/tests/")
        || norm_path.starts_with("test/")
        || norm_path.contains("/test/")
        || lower_name.ends_with("_test.rs")
        || lower_name.ends_with("_test.py")
        || lower_name.starts_with("test_")
        || lower_name.ends_with(".spec.ts")
        || lower_name.ends_with(".test.ts")
        || lower_name.ends_with(".spec.js")
        || lower_name.ends_with(".test.js")
    {
        return FileClassification::Test;
    }

    // 3. Documentation
    let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("");
    if ext == "md"
        || ext == "markdown"
        || ext == "rst"
        || ext == "txt"
        || lower_name == "license"
        || lower_name == "licence"
        || lower_name == "notice"
        || lower_name == "readme"
        || lower_name == "changelog"
        || lower_name == "contributing"
    {
        return FileClassification::Documentation;
    }

    // 4. Configuration
    if ext == "toml"
        || ext == "yaml"
        || ext == "yml"
        || ext == "json"
        || ext == "ini"
        || ext == "cfg"
        || ext == "conf"
        || lower_name.starts_with(".env")
        || lower_name == ".editorconfig"
        || lower_name.ends_with("rc")
    {
        return FileClassification::Configuration;
    }

    // 5. Generated code
    if norm_path.contains("generated/") || norm_path.contains("/gen/") {
        return FileClassification::Generated;
    }
    let sample_str = String::from_utf8_lossy(&content_sample[..content_sample.len().min(500)]);
    if sample_str.contains("@generated")
        || sample_str.contains("DO NOT EDIT")
        || sample_str.contains("Code generated by")
    {
        return FileClassification::Generated;
    }

    // 6. Assets
    if matches!(
        ext,
        "png"
            | "jpg"
            | "jpeg"
            | "gif"
            | "svg"
            | "ico"
            | "css"
            | "scss"
            | "sass"
            | "less"
            | "html"
            | "woff"
            | "woff2"
            | "ttf"
            | "eot"
    ) {
        return FileClassification::Asset;
    }

    // 7. Source files
    if matches!(
        ext,
        "rs" | "py"
            | "ts"
            | "tsx"
            | "js"
            | "jsx"
            | "mjs"
            | "cjs"
            | "go"
            | "c"
            | "cpp"
            | "h"
            | "hpp"
            | "java"
            | "sh"
            | "bash"
            | "zsh"
            | "sql"
    ) {
        return FileClassification::Source;
    }

    FileClassification::Source
}

/// Identifies architectural subsystem from path segments according to M31A L0-L9 architecture.
pub fn detect_subsystem(relative_path: &str) -> Option<SubsystemKind> {
    let normalized = relative_path.replace('\\', "/");
    let parts: Vec<&str> = normalized.split('/').collect();

    for &part in &parts {
        match part {
            "kernel" => return Some(SubsystemKind::Kernel),
            "security" | "policy" | "sandbox" => return Some(SubsystemKind::Security),
            "capability" | "capabilities" | "tools" | "process" => {
                return Some(SubsystemKind::Capability);
            }
            "intelligence" | "model" | "models" | "context" | "memory" | "repo" => {
                return Some(SubsystemKind::Intelligence);
            }
            "agent" | "agents" => return Some(SubsystemKind::Agent),
            "planning" | "dag" | "scheduler" => return Some(SubsystemKind::Planning),
            "execution" | "job" | "jobs" | "artifact" | "artifacts" => {
                return Some(SubsystemKind::Execution);
            }
            "verification" | "recovery" => return Some(SubsystemKind::Verification),
            "autonomy" | "mission" => return Some(SubsystemKind::Autonomy),
            "cli" | "tui" => return Some(SubsystemKind::UserInterface),
            _ => {}
        }
    }
    None
}

/// Basic `.gitignore` rule matcher supporting comments, anchors, directories, and wildcards.
#[derive(Debug, Clone, Default)]
pub struct GitIgnoreFilter {
    rules: Vec<String>,
}

impl GitIgnoreFilter {
    pub fn new() -> Self {
        Self { rules: Vec::new() }
    }

    pub fn load_from_dir(root: &Path) -> Self {
        let gitignore_path = root.join(".gitignore");
        if let Ok(content) = fs::read_to_string(gitignore_path) {
            let rules = content
                .lines()
                .map(|l| l.trim())
                .filter(|l| !l.is_empty() && !l.starts_with('#'))
                .map(|l| l.to_string())
                .collect();
            Self { rules }
        } else {
            Self { rules: Vec::new() }
        }
    }

    pub fn matches(&self, relative_path: &str, is_dir: bool) -> bool {
        let norm = relative_path.trim_start_matches('/').replace('\\', "/");
        for rule in &self.rules {
            let mut pattern = rule.as_str();
            let dir_only = pattern.ends_with('/');
            if dir_only {
                pattern = &pattern[..pattern.len() - 1];
                if !is_dir && !norm.contains('/') {
                    continue;
                }
            }

            let anchored = pattern.starts_with('/');
            if anchored {
                pattern = &pattern[1..];
            }

            if pattern.contains('*') || pattern.contains('?') {
                if glob_match(pattern, &norm) {
                    return true;
                }
            } else if anchored {
                if norm == pattern || norm.starts_with(&format!("{pattern}/")) {
                    return true;
                }
            } else if norm == pattern
                || norm.starts_with(&format!("{pattern}/"))
                || norm.ends_with(&format!("/{pattern}"))
                || norm.contains(&format!("/{pattern}/"))
            {
                return true;
            }
        }
        false
    }
}

fn glob_match(pattern: &str, text: &str) -> bool {
    let mut regex_str = String::from("^");
    for ch in pattern.chars() {
        match ch {
            '*' => regex_str.push_str(".*"),
            '?' => regex_str.push('.'),
            '.' | '(' | ')' | '+' | '|' | '^' | '$' | '@' | '%' | '{' | '}' | '[' | ']' => {
                regex_str.push('\\');
                regex_str.push(ch);
            }
            _ => regex_str.push(ch),
        }
    }
    regex_str.push('$');
    regex::Regex::new(&regex_str)
        .map(|re| re.is_match(text))
        .unwrap_or(false)
}

pub struct RepositoryScanner {
    workspace_root: PathBuf,
    excluded_dir_names: Vec<String>,
    gitignore: GitIgnoreFilter,
}

impl RepositoryScanner {
    pub fn new(workspace_root: impl Into<PathBuf>) -> Self {
        let root = workspace_root.into();
        let gitignore = GitIgnoreFilter::load_from_dir(&root);
        Self {
            workspace_root: root,
            excluded_dir_names: vec![
                "target".to_string(),
                "node_modules".to_string(),
                ".git".to_string(),
                ".gemini".to_string(),
                "dist".to_string(),
                "build".to_string(),
                ".next".to_string(),
                "venv".to_string(),
                ".venv".to_string(),
                "__pycache__".to_string(),
            ],
            gitignore,
        }
    }

    pub fn with_exclusions(mut self, exclusions: Vec<String>) -> Self {
        self.excluded_dir_names.extend(exclusions);
        self
    }

    pub fn scan(&self) -> Result<ScanSummary, std::io::Error> {
        let mut scanned_files = Vec::new();
        let mut total_symbols = 0;
        let mut all_edges = Vec::new();
        let mut entry_points = Vec::new();
        let mut ignored_file_count = 0;

        self.walk_dir(&self.workspace_root, &mut |path| {
            if let Ok(file_meta) = fs::metadata(path) {
                if !file_meta.is_file() {
                    return;
                }

                let relative_path = path
                    .strip_prefix(&self.workspace_root)
                    .unwrap_or(path)
                    .to_string_lossy()
                    .replace('\\', "/");

                // Check gitignore rules
                if self.gitignore.matches(&relative_path, false) {
                    ignored_file_count += 1;
                    return;
                }

                let language = SourceLanguage::detect_from_path(path);

                // Sample content for binary and classification checks
                let mut sample_buf = [0u8; 1024];
                let (is_binary, sample_slice) = if let Ok(mut f) = fs::File::open(path) {
                    use std::io::Read;
                    let n = f.read(&mut sample_buf).unwrap_or(0);
                    let sample = &sample_buf[..n];
                    (is_binary_content(sample), sample)
                } else {
                    (false, &sample_buf[..0])
                };

                let classification = classify_file(&relative_path, is_binary, sample_slice);
                let subsystem = detect_subsystem(&relative_path);

                // Entry point detection based on file convention
                if relative_path == "src/main.rs" || relative_path == "main.rs" {
                    entry_points.push(EntryPoint::new(
                        format!("ep:main:{relative_path}"),
                        EntryPointKind::BinaryMain,
                        &relative_path,
                        Some("main".to_string()),
                        1,
                        "Application entry point",
                    ));
                } else if relative_path == "src/lib.rs" || relative_path == "lib.rs" {
                    entry_points.push(EntryPoint::new(
                        format!("ep:lib:{relative_path}"),
                        EntryPointKind::LibraryRoot,
                        &relative_path,
                        None,
                        1,
                        "Crate library interface",
                    ));
                } else if relative_path == "build.rs" {
                    entry_points.push(EntryPoint::new(
                        format!("ep:build:{relative_path}"),
                        EntryPointKind::Script,
                        &relative_path,
                        None,
                        1,
                        "Build script",
                    ));
                } else if classification == FileClassification::Test {
                    entry_points.push(EntryPoint::new(
                        format!("ep:test:{relative_path}"),
                        EntryPointKind::TestEntry,
                        &relative_path,
                        None,
                        1,
                        "Integration test suite",
                    ));
                } else if relative_path.contains("cli") {
                    entry_points.push(EntryPoint::new(
                        format!("ep:cli:{relative_path}"),
                        EntryPointKind::CliCommand,
                        &relative_path,
                        None,
                        1,
                        "Command-line interface module",
                    ));
                }

                // If binary or file size > 2MB, skip AST parsing to avoid memory explosion
                if is_binary || file_meta.len() > MAX_PARSE_SIZE_BYTES {
                    let mut hasher = Sha256::new();
                    hasher.update(sample_slice);
                    let content_hash = format!("{:x}", hasher.finalize());

                    scanned_files.push(ScannedFile {
                        relative_path,
                        language,
                        content_hash,
                        size_bytes: file_meta.len(),
                        classification,
                        subsystem,
                        symbols: Vec::new(),
                    });
                    return;
                }

                if let Ok(content_bytes) = fs::read(path) {
                    let mut hasher = Sha256::new();
                    hasher.update(&content_bytes);
                    let content_hash = format!("{:x}", hasher.finalize());
                    let content_str = String::from_utf8_lossy(&content_bytes);

                    let (symbols, edges) = match language {
                        SourceLanguage::Rust => {
                            RustAstExtractor::extract_with_edges(&relative_path, &content_str)
                                .unwrap_or_default()
                        }
                        SourceLanguage::Python => (
                            PythonExtractor::extract(&relative_path, &content_str),
                            Vec::new(),
                        ),
                        SourceLanguage::TypeScript | SourceLanguage::JavaScript => (
                            TypeScriptExtractor::extract(&relative_path, &content_str),
                            Vec::new(),
                        ),
                        SourceLanguage::Manifest => {
                            let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                            let syms = if file_name == "Cargo.toml" {
                                ManifestExtractor::extract_cargo(&relative_path, &content_str)
                                    .unwrap_or_default()
                            } else if file_name == "package.json" {
                                ManifestExtractor::extract_package_json(
                                    &relative_path,
                                    &content_str,
                                )
                                .unwrap_or_default()
                            } else if file_name == "pyproject.toml" {
                                ManifestExtractor::extract_pyproject(&relative_path, &content_str)
                                    .unwrap_or_default()
                            } else {
                                Vec::new()
                            };
                            (syms, Vec::new())
                        }
                        SourceLanguage::Other => (Vec::new(), Vec::new()),
                    };

                    total_symbols += symbols.len();
                    all_edges.extend(edges);

                    scanned_files.push(ScannedFile {
                        relative_path,
                        language,
                        content_hash,
                        size_bytes: file_meta.len(),
                        classification,
                        subsystem,
                        symbols,
                    });
                }
            }
        })?;

        let file_count = scanned_files.len();
        Ok(ScanSummary {
            files: scanned_files,
            total_symbols,
            scanned_file_count: file_count,
            edges: all_edges,
            entry_points,
            ignored_file_count,
        })
    }

    fn walk_dir<F>(&self, dir: &Path, callback: &mut F) -> Result<(), std::io::Error>
    where
        F: FnMut(&Path),
    {
        let mut visited = std::collections::HashSet::new();
        self.walk_dir_bounded(dir, 0, &mut visited, callback)
    }

    /// Bounded directory traversal with symlink, cycle, and depth protections.
    ///
    /// - Symlinks are never followed (neither descended into nor read): a
    ///   symlink farm pointing at `/`, `..`, or itself cannot escape the
    ///   workspace, loop forever, or inflate the scan.
    /// - Traversal depth is capped at `MAX_SCAN_DEPTH`.
    /// - Canonicalized directories are tracked in `visited` so hardlink or
    ///   bind-mount cycles terminate.
    /// - Total visited entries are capped at `MAX_SCAN_ENTRIES`.
    fn walk_dir_bounded<F>(
        &self,
        dir: &Path,
        depth: usize,
        visited: &mut std::collections::HashSet<PathBuf>,
        callback: &mut F,
    ) -> Result<(), std::io::Error>
    where
        F: FnMut(&Path),
    {
        if depth > MAX_SCAN_DEPTH {
            return Ok(());
        }
        if visited.len() > MAX_SCAN_ENTRIES {
            return Err(std::io::Error::new(
                std::io::ErrorKind::QuotaExceeded,
                "repository scan entry budget exceeded",
            ));
        }
        // Never follow symlinks: lstat, not stat.
        let meta = fs::symlink_metadata(dir)?;
        if meta.file_type().is_symlink() {
            return Ok(());
        }
        if !meta.is_dir() {
            return Ok(());
        }
        // Cycle guard on the canonical directory identity.
        if let Ok(canon) = dir.canonicalize() {
            if !visited.insert(canon) {
                return Ok(());
            }
        }

        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            let file_name = entry.file_name();
            let name_str = file_name.to_string_lossy();

            if self.excluded_dir_names.iter().any(|ex| ex == &name_str) {
                continue;
            }

            // Never follow symlinks (files or dirs).
            if fs::symlink_metadata(&path)
                .map(|m| m.file_type().is_symlink())
                .unwrap_or(false)
            {
                continue;
            }

            if path.is_dir() {
                self.walk_dir_bounded(&path, depth + 1, visited, callback)?;
            } else {
                callback(&path);
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_repository_scanner_walk_and_routing() {
        let dir = tempdir().unwrap();
        let ws = dir.path();

        // Create Rust file
        let rs_path = ws.join("lib.rs");
        fs::write(&rs_path, "pub fn add(a: i32, b: i32) -> i32 { a + b }").unwrap();

        // Create Python file
        let py_path = ws.join("main.py");
        fs::write(&py_path, "def run():\n    pass").unwrap();

        // Create TypeScript file
        let ts_path = ws.join("app.ts");
        fs::write(&ts_path, "export function start() {}").unwrap();

        // Create ignored directory
        let target_dir = ws.join("target");
        fs::create_dir_all(&target_dir).unwrap();
        fs::write(target_dir.join("ignore.rs"), "pub fn ignored() {}").unwrap();

        let scanner = RepositoryScanner::new(ws);
        let summary = scanner.scan().expect("scan succeeds");

        assert_eq!(summary.scanned_file_count, 3);
        assert!(
            summary
                .files
                .iter()
                .any(|f| f.relative_path == "lib.rs" && f.language == SourceLanguage::Rust)
        );
        assert!(
            summary
                .files
                .iter()
                .any(|f| f.relative_path == "main.py" && f.language == SourceLanguage::Python)
        );
        assert!(
            summary
                .files
                .iter()
                .any(|f| f.relative_path == "app.ts" && f.language == SourceLanguage::TypeScript)
        );
        assert!(
            !summary
                .files
                .iter()
                .any(|f| f.relative_path.contains("target"))
        );

        // Verify total symbols
        assert!(summary.total_symbols >= 3);
    }

    #[test]
    fn test_gitignore_and_classification_and_subsystems() {
        let dir = tempdir().unwrap();
        let ws = dir.path();

        // Write .gitignore
        fs::write(ws.join(".gitignore"), "*.tmp\nbuild/\nsecret.key\n").unwrap();

        // Files that should be ignored
        fs::write(ws.join("test.tmp"), "temp").unwrap();
        fs::write(ws.join("secret.key"), "key").unwrap();

        // Valid files
        let src_kernel = ws.join("src").join("kernel");
        fs::create_dir_all(&src_kernel).unwrap();
        fs::write(src_kernel.join("mod.rs"), "pub fn boot() {}").unwrap();

        let tests_dir = ws.join("tests");
        fs::create_dir_all(&tests_dir).unwrap();
        fs::write(tests_dir.join("integration_test.rs"), "fn test_foo() {}").unwrap();

        fs::write(ws.join("README.md"), "# Hello").unwrap();
        fs::write(ws.join("config.toml"), "key = 1").unwrap();

        let scanner = RepositoryScanner::new(ws);
        let summary = scanner.scan().unwrap();

        assert_eq!(summary.ignored_file_count, 2);

        let kernel_file = summary
            .files
            .iter()
            .find(|f| f.relative_path.contains("kernel"))
            .expect("kernel file present");
        assert_eq!(kernel_file.classification, FileClassification::Source);
        assert_eq!(kernel_file.subsystem, Some(SubsystemKind::Kernel));

        let test_file = summary
            .files
            .iter()
            .find(|f| f.relative_path.contains("integration_test"))
            .expect("test file present");
        assert_eq!(test_file.classification, FileClassification::Test);

        let doc_file = summary
            .files
            .iter()
            .find(|f| f.relative_path == "README.md")
            .expect("readme present");
        assert_eq!(doc_file.classification, FileClassification::Documentation);

        let config_file = summary
            .files
            .iter()
            .find(|f| f.relative_path == "config.toml")
            .expect("config present");
        assert_eq!(
            config_file.classification,
            FileClassification::Configuration
        );

        // Verify entry point detection
        assert!(
            summary
                .entry_points
                .iter()
                .any(|ep| ep.kind == EntryPointKind::TestEntry)
        );
    }

    #[test]
    fn test_binary_detection_and_size_guard() {
        let dir = tempdir().unwrap();
        let ws = dir.path();

        // Binary file with null byte
        let bin_bytes = b"ELF\x00\x01\x02\x03somebinarydata";
        fs::write(ws.join("program.bin"), bin_bytes).unwrap();

        let scanner = RepositoryScanner::new(ws);
        let summary = scanner.scan().unwrap();

        let bin_file = summary
            .files
            .iter()
            .find(|f| f.relative_path == "program.bin")
            .expect("binary file indexed");
        assert_eq!(bin_file.classification, FileClassification::Binary);
        assert!(bin_file.symbols.is_empty());
    }
}
