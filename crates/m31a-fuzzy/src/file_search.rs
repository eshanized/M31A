//! Fast workspace file scanner and candidate collector.

use crate::candidates::{Candidate, CandidateType};
use crate::matcher::fuzzy_match_indices;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

static FILE_CACHE: Mutex<Option<(PathBuf, Instant, Vec<Candidate>)>> = Mutex::new(None);

pub struct WorkspaceScanner;

impl WorkspaceScanner {
    /// Collects all non-ignored files in the workspace with caching and .gitignore support.
    pub fn collect_workspace_files_cached(root: &Path, max_entries: usize) -> Vec<Candidate> {
        let mut cache = FILE_CACHE.lock().unwrap();
        if let Some((ref cached_root, ref timestamp, ref files)) = *cache
            && cached_root == root
            && timestamp.elapsed() < Duration::from_millis(5000)
        {
            return files.clone();
        }

        let files = Self::collect_workspace_files(root, max_entries);
        *cache = Some((root.to_path_buf(), Instant::now(), files.clone()));
        files
    }

    /// Parses gitignore patterns from workspace root and parent directories.
    fn load_gitignore_rules(root: &Path) -> HashSet<String> {
        let mut rules = HashSet::new();
        // Common default ignore patterns
        for def in &[
            "target",
            "node_modules",
            "build",
            "dist",
            ".git",
            ".m31a",
            ".codex",
            "*.pyc",
            "__pycache__",
        ] {
            rules.insert(def.to_string());
        }

        let gitignore = root.join(".gitignore");
        if let Ok(content) = std::fs::read_to_string(&gitignore) {
            for line in content.lines() {
                let trimmed = line.trim();
                if !trimmed.is_empty() && !trimmed.starts_with('#') {
                    rules.insert(trimmed.trim_matches('/').to_string());
                }
            }
        }
        rules
    }

    /// Checks if a file or directory matches any gitignore rules.
    fn is_ignored(name: &str, rel_path: &str, rules: &HashSet<String>) -> bool {
        if name.starts_with('.') {
            return true;
        }

        for rule in rules {
            if let Some(suffix) = rule.strip_prefix('*') {
                if name.ends_with(suffix) {
                    return true;
                }
            } else if rule == name || rule == rel_path || rel_path.starts_with(rule) {
                return true;
            }
        }
        false
    }

    /// Collects all non-ignored files in the workspace up to max_entries.
    pub fn collect_workspace_files(root: &Path, max_entries: usize) -> Vec<Candidate> {
        let mut candidates = Vec::new();
        let mut stack = vec![root.to_path_buf()];
        let ignore_rules = Self::load_gitignore_rules(root);

        while let Some(dir) = stack.pop() {
            if candidates.len() >= max_entries {
                break;
            }

            let entries = match std::fs::read_dir(&dir) {
                Ok(e) => e,
                Err(_) => continue,
            };

            for entry in entries.flatten() {
                let path = entry.path();
                let file_name = entry.file_name();
                let name_str = file_name.to_string_lossy();

                let rel_path = path.strip_prefix(root).unwrap_or(&path);
                let rel_str = rel_path.to_string_lossy().to_string();

                if Self::is_ignored(&name_str, &rel_str, &ignore_rules) {
                    continue;
                }

                if path.is_dir() {
                    candidates.push(Candidate::new(
                        format!("{}/", rel_str),
                        CandidateType::Directory,
                        format!("{}/", rel_str),
                    ));
                    stack.push(path);
                } else if path.is_file() {
                    candidates.push(Candidate::new(
                        rel_str.clone(),
                        CandidateType::File,
                        rel_str,
                    ));
                }
            }
        }

        candidates
    }

    /// Fuzzy searches a set of candidates against a query string.
    pub fn search(query: &str, candidates: &[Candidate]) -> Vec<Candidate> {
        if query.trim().is_empty() {
            return candidates.iter().take(20).cloned().collect();
        }

        // Zero-clone candidate matching: collect indices and scores first without cloning structs
        let mut matches = Vec::new();
        for (idx, cand) in candidates.iter().enumerate() {
            if let Some((score, indices)) = fuzzy_match_indices(query, &cand.display_name) {
                matches.push((idx, score, indices));
            }
        }

        matches.sort_by_key(|&(_, score, _)| std::cmp::Reverse(score));
        matches.truncate(20);

        matches
            .into_iter()
            .map(|(idx, score, indices)| {
                let mut c = candidates[idx].clone();
                c.score = score;
                c.match_indices = indices;
                c
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_search_scoring() {
        let items = vec![
            Candidate::new("src/main.rs", CandidateType::File, "src/main.rs"),
            Candidate::new(
                "crates/m31a-core/src/lib.rs",
                CandidateType::File,
                "crates/m31a-core/src/lib.rs",
            ),
            Candidate::new("Cargo.toml", CandidateType::File, "Cargo.toml"),
        ];

        let results = WorkspaceScanner::search("main", &items);
        assert!(!results.is_empty());
        assert_eq!(results[0].display_name, "src/main.rs");
    }
}
