//! Post-mutation repository observation and fresh context compiler.
//!
//! Mandatory invariant: After modifying code, DO NOT continue reasoning from stale
//! pre-edit context.
//! Re-queries:
//! - Fresh file content & SHA-256 hashes
//! - Affected symbols and definitions
//! - Callers, callees, and impacted dependencies
//! - Targeted tests discovered via BoundedQueryEngine
//! - Fresh unified git diff of modifications

use sha2::{Digest, Sha256};
use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};

use crate::repo::query::{BoundedQueryEngine, QueryBounds};

/// Fresh evidence bundle captured immediately after mutations are applied.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct FreshMutationEvidence {
    pub modified_files: Vec<String>,
    pub affected_symbols: Vec<String>,
    pub recommended_tests: Vec<String>,
    pub unified_diff: String,
    pub fresh_file_hashes: HashMap<String, String>,
}

/// Observes repository state after mutations to ground subsequent reasoning in reality.
pub struct PostMutationObserver;

impl PostMutationObserver {
    /// Capture fresh observation over the modified files.
    pub async fn observe(
        workspace_root: &Path,
        files_modified: &[String],
        query_engine: Option<&BoundedQueryEngine>,
    ) -> Result<FreshMutationEvidence, std::io::Error> {
        Self::observe_with_snapshots(workspace_root, files_modified, query_engine, None).await
    }

    /// Capture fresh observation over the modified files, with optional pre-mutation snapshots.
    pub async fn observe_with_snapshots(
        workspace_root: &Path,
        files_modified: &[String],
        query_engine: Option<&BoundedQueryEngine>,
        snapshots: Option<&HashMap<PathBuf, Option<Vec<u8>>>>,
    ) -> Result<FreshMutationEvidence, std::io::Error> {
        let mut fresh_file_hashes = HashMap::new();
        let mut affected_symbols_set = BTreeSet::new();
        let mut recommended_tests_set = BTreeSet::new();

        // 1. Re-read modified files and re-compute hashes.
        // Because `files_modified` is model-influenced, resolve every path
        // through canonical containment before touching disk. A rejected path
        // fails the observation closed — evidence must never be gathered
        // from outside the workspace.
        let guard = crate::capability::providers::LocalFileSystemProvider::new(workspace_root)
            .map_err(|e| {
                std::io::Error::new(
                    std::io::ErrorKind::PermissionDenied,
                    format!("workspace containment base invalid: {e}"),
                )
            })?;
        for file_str in files_modified {
            let full_path = guard.resolve_and_verify(Path::new(file_str)).map_err(|e| {
                std::io::Error::new(
                    std::io::ErrorKind::PermissionDenied,
                    format!("modified file '{file_str}' failed containment: {e}"),
                )
            })?;
            if full_path.exists()
                && let Ok(bytes) = tokio::fs::read(&full_path).await
            {
                let mut hasher = Sha256::new();
                hasher.update(&bytes);
                let hash = format!("{:x}", hasher.finalize());
                fresh_file_hashes.insert(file_str.clone(), hash);
            }
        }

        // 2. Query repository intelligence if available
        if let Some(qe) = query_engine {
            // Change impact
            let impact = qe.calculate_change_impact(files_modified, QueryBounds::default());
            for sym in impact.data.directly_affected_symbols {
                affected_symbols_set.insert(sym.name);
            }
            for sym in impact.data.downstream_affected_symbols {
                affected_symbols_set.insert(sym.name);
            }
            for test in impact.data.recommended_tests {
                recommended_tests_set.insert(test);
            }

            // Direct test discovery per file
            for file_str in files_modified {
                let test_res = qe.find_tests_for_file(file_str, QueryBounds::default());
                for test_file in test_res.data {
                    recommended_tests_set.insert(test_file);
                }
            }
        }

        // 3. Generate unified git diff for the modified files
        let unified_diff = Self::generate_diff(workspace_root, files_modified, snapshots).await;

        Ok(FreshMutationEvidence {
            modified_files: files_modified.to_vec(),
            affected_symbols: affected_symbols_set.into_iter().collect(),
            recommended_tests: recommended_tests_set.into_iter().collect(),
            unified_diff,
            fresh_file_hashes,
        })
    }

    async fn generate_diff(
        workspace_root: &Path,
        files: &[String],
        snapshots: Option<&HashMap<PathBuf, Option<Vec<u8>>>>,
    ) -> String {
        if files.is_empty() {
            return String::new();
        }

        let mut cmd = crate::git::scoped_git_command();
        cmd.kill_on_drop(true);
        cmd.arg("diff");
        cmd.arg("--");
        for f in files {
            if f.is_empty() || f.starts_with('-') {
                continue;
            }
            cmd.arg(f);
        }
        cmd.current_dir(workspace_root);

        if let Ok(out) = tokio::time::timeout(crate::git::GIT_COMMAND_TIMEOUT, cmd.output())
            .await
            .map_err(|_| std::io::Error::new(std::io::ErrorKind::TimedOut, "git diff timed out"))
            .and_then(|r| r)
            && out.status.success()
        {
            let diff_str = String::from_utf8_lossy(&out.stdout).to_string();
            if !diff_str.trim().is_empty() {
                return diff_str;
            }
        }

        // Fallback: if git diff was empty or failed, synthesize unified diff from snapshots vs disk
        let mut synthesized = String::new();
        let fallback_guard =
            crate::capability::providers::LocalFileSystemProvider::new(workspace_root).ok();
        for file in files {
            // Enforce path containment first; skip (never read) rejected paths.
            let full_path = match fallback_guard.as_ref() {
                Some(g) => match g.resolve_and_verify(Path::new(file)) {
                    Ok(p) => p,
                    Err(_) => continue,
                },
                None => continue,
            };
            // Snapshot map is keyed by pre-mutation join paths; fall back to
            // the legacy join key when the canonical form differs (e.g.
            // symlinked root components) so diff quality is preserved.
            let legacy_path = workspace_root.join(file);
            let old_bytes_opt = snapshots
                .and_then(|s| s.get(&full_path).cloned().flatten())
                .or_else(|| snapshots.and_then(|s| s.get(&legacy_path).cloned().flatten()));
            let new_bytes_opt = tokio::fs::read(&full_path).await.ok();

            let file_diff = Self::generate_file_unified_diff(
                file,
                old_bytes_opt.as_deref(),
                new_bytes_opt.as_deref(),
            );
            synthesized.push_str(&file_diff);
        }

        if synthesized.trim().is_empty() {
            format!("Modified files on disk: {:?}", files)
        } else {
            synthesized
        }
    }

    /// Pure Rust unified diff generator for two snapshots.
    pub fn generate_file_unified_diff(
        file: &str,
        old_bytes: Option<&[u8]>,
        new_bytes: Option<&[u8]>,
    ) -> String {
        let old_str = old_bytes.map(String::from_utf8_lossy);
        let new_str = new_bytes.map(String::from_utf8_lossy);

        match (old_str, new_str) {
            (None, None) => String::new(),
            (None, Some(new)) => {
                let lines: Vec<&str> = new.lines().collect();
                let mut diff = format!(
                    "--- /dev/null\n+++ b/{file}\n@@ -0,0 +1,{} @@\n",
                    lines.len()
                );
                for line in lines {
                    diff.push('+');
                    diff.push_str(line);
                    diff.push('\n');
                }
                diff
            }
            (Some(old), None) => {
                let lines: Vec<&str> = old.lines().collect();
                let mut diff = format!(
                    "--- a/{file}\n+++ /dev/null\n@@ -1,{} +0,0 @@\n",
                    lines.len()
                );
                for line in lines {
                    diff.push('-');
                    diff.push_str(line);
                    diff.push('\n');
                }
                diff
            }
            (Some(old), Some(new)) => {
                if old == new {
                    return String::new();
                }
                let old_lines: Vec<&str> = old.lines().collect();
                let new_lines: Vec<&str> = new.lines().collect();

                let mut prefix_len = 0;
                while prefix_len < old_lines.len()
                    && prefix_len < new_lines.len()
                    && old_lines[prefix_len] == new_lines[prefix_len]
                {
                    prefix_len += 1;
                }

                let mut suffix_len = 0;
                while suffix_len < (old_lines.len() - prefix_len)
                    && suffix_len < (new_lines.len() - prefix_len)
                    && old_lines[old_lines.len() - 1 - suffix_len]
                        == new_lines[new_lines.len() - 1 - suffix_len]
                {
                    suffix_len += 1;
                }

                let old_mid = &old_lines[prefix_len..old_lines.len() - suffix_len];
                let new_mid = &new_lines[prefix_len..new_lines.len() - suffix_len];

                let mut diff = format!(
                    "--- a/{file}\n+++ b/{file}\n@@ -{},{} +{},{} @@\n",
                    prefix_len + 1,
                    old_lines.len(),
                    prefix_len + 1,
                    new_lines.len()
                );

                for line in old_mid {
                    diff.push('-');
                    diff.push_str(line);
                    diff.push('\n');
                }
                for line in new_mid {
                    diff.push('+');
                    diff.push_str(line);
                    diff.push('\n');
                }
                diff
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_post_mutation_observe_captures_hashes() {
        let dir = tempdir().unwrap();
        let ws = dir.path();
        let file_path = ws.join("src/lib.rs");
        tokio::fs::create_dir_all(file_path.parent().unwrap())
            .await
            .unwrap();
        tokio::fs::write(&file_path, b"pub fn fresh() {}\n")
            .await
            .unwrap();

        let evidence = PostMutationObserver::observe(ws, &["src/lib.rs".to_string()], None)
            .await
            .unwrap();

        assert_eq!(evidence.modified_files, vec!["src/lib.rs"]);
        assert!(evidence.fresh_file_hashes.contains_key("src/lib.rs"));
        assert!(!evidence.fresh_file_hashes["src/lib.rs"].is_empty());
    }
}
