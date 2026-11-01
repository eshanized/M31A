//! File content grep and filename search tool implementations.
//!
//! Provides fast ripgrep-style regex search and fuzzy filename discovery across the workspace.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::{DirEntry, WalkDir};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SearchEntryType {
    File,
    Directory,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SearchResult {
    pub relative_path: PathBuf,
    pub entry_type: SearchEntryType,
    pub depth: usize,
    pub score: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub indices: Option<Vec<u32>>,
}

pub type FileMatch = SearchResult;
pub type MatchType = SearchEntryType;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct FileSearchSnapshot {
    pub query: String,
    pub matches: Vec<SearchResult>,
    pub total_match_count: usize,
    pub scanned_file_count: usize,
    pub walk_complete: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RelatedFileMatch {
    pub path: String,
    pub score: u32,
    pub reasons: Vec<String>,
}

pub struct FileSearchEngine {
    ignored_dir_names: HashSet<&'static str>,
}

impl Default for FileSearchEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl FileSearchEngine {
    pub fn new() -> Self {
        let mut ignored_dir_names = HashSet::new();
        ignored_dir_names.insert(".git");
        ignored_dir_names.insert("target");
        ignored_dir_names.insert("node_modules");
        ignored_dir_names.insert("dist");
        ignored_dir_names.insert("build");
        ignored_dir_names.insert(".cache");
        ignored_dir_names.insert(".next");
        ignored_dir_names.insert("venv");
        ignored_dir_names.insert(".venv");
        ignored_dir_names.insert("__pycache__");

        Self { ignored_dir_names }
    }

    /// Recursively searches for filenames matching pattern or substring, respecting ignore rules.
    pub fn search(
        &self,
        root: &Path,
        pattern: &str,
        max_depth: usize,
        max_results: usize,
    ) -> Vec<SearchResult> {
        let pattern_lower = pattern.to_ascii_lowercase();
        let custom_ignores = self.load_ignore_patterns(root);

        let mut results = Vec::new();

        let walker = WalkDir::new(root)
            .max_depth(max_depth)
            .into_iter()
            .filter_entry(|entry| !self.should_skip_entry(entry, &custom_ignores));

        for entry in walker.filter_map(|e| e.ok()) {
            if entry.path() == root {
                continue;
            }

            let file_name = entry.file_name().to_string_lossy();
            let file_name_lower = file_name.to_ascii_lowercase();

            let matched = if pattern.contains('*') || pattern.contains('?') {
                glob_match(&pattern_lower, &file_name_lower)
            } else {
                file_name_lower.contains(&pattern_lower)
            };

            if matched && let Ok(rel) = entry.path().strip_prefix(root) {
                let entry_type = if entry.file_type().is_dir() {
                    SearchEntryType::Directory
                } else {
                    SearchEntryType::File
                };

                results.push(SearchResult {
                    relative_path: rel.to_path_buf(),
                    entry_type,
                    depth: entry.depth(),
                    score: 100,
                    indices: None,
                });

                if results.len() >= max_results {
                    break;
                }
            }
        }

        results.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
        results
    }

    /// High-performance fuzzy search with score ranking and character match indices.
    pub fn fuzzy_search(
        &self,
        root: &Path,
        query: &str,
        max_depth: usize,
        max_results: usize,
    ) -> FileSearchSnapshot {
        let custom_ignores = self.load_ignore_patterns(root);
        let mut matches = Vec::new();
        let mut scanned = 0;

        let walker = WalkDir::new(root)
            .max_depth(max_depth)
            .into_iter()
            .filter_entry(|entry| !self.should_skip_entry(entry, &custom_ignores));

        for entry in walker.filter_map(|e| e.ok()) {
            if entry.path() == root {
                continue;
            }
            scanned += 1;

            if let Ok(rel) = entry.path().strip_prefix(root) {
                let rel_str = rel.to_string_lossy();
                if let Some((score, indices)) = fuzzy_match_path(query, &rel_str, true) {
                    let entry_type = if entry.file_type().is_dir() {
                        SearchEntryType::Directory
                    } else {
                        SearchEntryType::File
                    };

                    matches.push(SearchResult {
                        relative_path: rel.to_path_buf(),
                        entry_type,
                        depth: entry.depth(),
                        score,
                        indices: Some(indices),
                    });
                }
            }
        }

        let total_match_count = matches.len();
        // Sort descending by score, then ascending by depth
        matches.sort_by(|a, b| b.score.cmp(&a.score).then_with(|| a.depth.cmp(&b.depth)));
        matches.truncate(max_results);

        FileSearchSnapshot {
            query: query.to_string(),
            matches,
            total_match_count,
            scanned_file_count: scanned,
            walk_complete: true,
        }
    }

    /// Discovers related files based on shared directory, filename stem, and extension family.
    pub fn find_related_files(
        &self,
        root: &Path,
        target_file: &str,
        limit: usize,
    ) -> Vec<RelatedFileMatch> {
        let target_path = Path::new(target_file);
        let target_stem = target_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        let target_parent = target_path
            .parent()
            .map(|p| p.to_string_lossy().replace('\\', "/"))
            .unwrap_or_default();

        let all_files = self.search(root, "*", 10, 1000);
        let mut scored = Vec::new();

        for file in all_files {
            if file.entry_type != SearchEntryType::File {
                continue;
            }
            let rel_str = file.relative_path.to_string_lossy().replace('\\', "/");
            if rel_str == target_file || rel_str.ends_with(target_file) {
                continue;
            }

            let cand_stem = file
                .relative_path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_ascii_lowercase();
            let cand_parent = file
                .relative_path
                .parent()
                .map(|p| p.to_string_lossy().replace('\\', "/"))
                .unwrap_or_default();

            let mut score = 0;
            let mut reasons = Vec::new();

            // 1. Same directory
            if !target_parent.is_empty() && cand_parent == target_parent {
                score += 1;
                reasons.push("same directory".to_string());
            }

            // 2. Same filename stem or prefix/suffix variant
            if !target_stem.is_empty() {
                if cand_stem == target_stem {
                    score += 3;
                    reasons.push("same filename stem".to_string());
                } else if cand_stem.starts_with(&target_stem) || target_stem.starts_with(&cand_stem)
                {
                    score += 2;
                    reasons.push("prefix/suffix stem match".to_string());
                }
            }

            if score > 0 {
                scored.push(RelatedFileMatch {
                    path: rel_str,
                    score,
                    reasons,
                });
            }
        }

        scored.sort_by(|a, b| b.score.cmp(&a.score).then_with(|| a.path.cmp(&b.path)));
        scored.truncate(limit);
        scored
    }

    fn should_skip_entry(&self, entry: &DirEntry, custom_ignores: &[String]) -> bool {
        let name = entry.file_name().to_string_lossy();

        if entry.file_type().is_dir() && self.ignored_dir_names.contains(name.as_ref()) {
            return true;
        }

        for rule in custom_ignores {
            if name == rule.as_str() || name.ends_with(rule.trim_start_matches('*')) {
                return true;
            }
        }

        false
    }

    fn load_ignore_patterns(&self, root: &Path) -> Vec<String> {
        let mut patterns = Vec::new();

        for filename in &[".gitignore", ".m31aignore", ".ignore"] {
            let path = root.join(filename);
            if let Ok(content) = fs::read_to_string(&path) {
                for line in content.lines() {
                    let trimmed = line.trim();
                    if !trimmed.is_empty() && !trimmed.starts_with('#') {
                        patterns.push(trimmed.trim_end_matches('/').to_string());
                    }
                }
            }
        }

        patterns
    }
}

/// Computes fuzzy matching score and indices between query and target path.
pub fn fuzzy_match_path(
    query: &str,
    path_str: &str,
    compute_indices: bool,
) -> Option<(u32, Vec<u32>)> {
    if query.is_empty() {
        return Some((0, Vec::new()));
    }

    let q_chars: Vec<char> = query.chars().collect();
    let p_chars: Vec<char> = path_str.chars().collect();

    let mut q_idx = 0;
    let mut indices = Vec::new();
    let mut score: u32 = 0;
    let mut last_match_idx: Option<usize> = None;

    let filename_start = path_str.rfind('/').map(|i| i + 1).unwrap_or(0);

    for (p_idx, &p_ch) in p_chars.iter().enumerate() {
        if q_idx < q_chars.len() {
            let q_ch = q_chars[q_idx];
            let is_exact = p_ch == q_ch;
            let is_case_insensitive = p_ch.eq_ignore_ascii_case(&q_ch);

            if is_case_insensitive {
                let mut match_score = 10;
                if is_exact {
                    match_score += 5;
                }

                // Consecutive match bonus
                if let Some(prev) = last_match_idx
                    && prev + 1 == p_idx
                {
                    match_score += 15;
                }

                // Word boundary bonus (after /, _, -, .)
                if p_idx == 0
                    || p_chars[p_idx - 1] == '/'
                    || p_chars[p_idx - 1] == '_'
                    || p_chars[p_idx - 1] == '-'
                    || p_chars[p_idx - 1] == '.'
                {
                    match_score += 30;
                }

                // CamelCase boundary bonus
                if p_idx > 0 && p_chars[p_idx - 1].is_lowercase() && p_ch.is_uppercase() {
                    match_score += 20;
                }

                // Filename match bonus over directory path
                if p_idx >= filename_start {
                    match_score += 25;
                }

                score += match_score;
                if compute_indices {
                    indices.push(p_idx as u32);
                }
                last_match_idx = Some(p_idx);
                q_idx += 1;
            }
        }
    }

    if q_idx == q_chars.len() {
        Some((score, indices))
    } else {
        None
    }
}

/// Simple glob matcher supporting `*` (any characters) and `?` (single char).
pub fn glob_match(pattern: &str, text: &str) -> bool {
    let p_chars: Vec<char> = pattern.chars().collect();
    let t_chars: Vec<char> = text.chars().collect();

    let mut p = 0;
    let mut t = 0;
    let mut star_p = None;
    let mut match_t = 0;

    while t < t_chars.len() {
        if p < p_chars.len() && (p_chars[p] == '?' || p_chars[p] == t_chars[t]) {
            p += 1;
            t += 1;
        } else if p < p_chars.len() && p_chars[p] == '*' {
            star_p = Some(p);
            p += 1;
            match_t = t;
        } else if let Some(sp) = star_p {
            p = sp + 1;
            match_t += 1;
            t = match_t;
        } else {
            return false;
        }
    }

    while p < p_chars.len() && p_chars[p] == '*' {
        p += 1;
    }

    p == p_chars.len()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_glob_match() {
        assert!(glob_match("*.rs", "main.rs"));
        assert!(glob_match("test_*.py", "test_algo.py"));
        assert!(glob_match("data?.json", "data1.json"));
        assert!(!glob_match("*.rs", "main.py"));
    }

    #[test]
    fn test_fuzzy_match_path() {
        let query = "msyn";
        let target = "src/markdown_syntax.rs";
        let res = fuzzy_match_path(query, target, true);
        assert!(res.is_some());
        let (score, indices) = res.unwrap();
        assert!(score > 50);
        assert_eq!(indices.len(), 4);
    }

    #[test]
    fn test_search_engine_ignores_git_and_target() {
        let dir = tempdir().unwrap();
        let root = dir.path();

        // Create target/hidden.rs
        let target_dir = root.join("target");
        fs::create_dir_all(&target_dir).unwrap();
        fs::write(target_dir.join("build.rs"), "fn main() {}").unwrap();

        // Create src/main.rs
        let src_dir = root.join("src");
        fs::create_dir_all(&src_dir).unwrap();
        fs::write(src_dir.join("main.rs"), "fn main() {}").unwrap();

        let engine = FileSearchEngine::new();
        let results = engine.search(root, "*.rs", 5, 20);

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].relative_path, PathBuf::from("src/main.rs"));
    }

    #[test]
    fn test_fuzzy_search_ranking() {
        let dir = tempdir().unwrap();
        let root = dir.path();

        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(root.join("src/main.rs"), "fn main() {}").unwrap();
        fs::write(root.join("src/lib.rs"), "pub mod foo;").unwrap();
        fs::write(root.join("src/model_manager.rs"), "").unwrap();

        let engine = FileSearchEngine::new();
        let snapshot = engine.fuzzy_search(root, "modman", 5, 10);
        assert!(!snapshot.matches.is_empty());
        assert_eq!(
            snapshot.matches[0].relative_path,
            PathBuf::from("src/model_manager.rs")
        );
    }

    #[test]
    fn test_find_related_files_heuristic() {
        let dir = tempdir().unwrap();
        let root = dir.path();

        fs::create_dir_all(root.join("src/auth")).unwrap();
        fs::write(root.join("src/auth/jwt.rs"), "").unwrap();
        fs::write(root.join("src/auth/jwt_test.rs"), "").unwrap();
        fs::write(root.join("src/auth/session.rs"), "").unwrap();
        fs::write(root.join("src/unrelated.rs"), "").unwrap();

        let engine = FileSearchEngine::new();
        let related = engine.find_related_files(root, "src/auth/jwt.rs", 10);

        assert!(!related.is_empty());
        // jwt_test.rs should score highest (same directory + prefix/suffix stem match)
        assert_eq!(related[0].path, "src/auth/jwt_test.rs");
        assert!(related[0].score >= 3);
        // session.rs is same directory
        assert!(related.iter().any(|r| r.path == "src/auth/session.rs"));
        // unrelated is not in the same directory or stem
        assert!(!related.iter().any(|r| r.path == "src/unrelated.rs"));
    }
}
