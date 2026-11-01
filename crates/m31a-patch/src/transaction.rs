//! Two-phase atomic commit transaction engine for patches.

use anyhow::{Context, Result, anyhow};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::fuzzy_matcher::FuzzyMatcher;
use crate::syntax_validator::SyntaxValidator;
use crate::unified_diff::{FileAction, ParsedPatch, PatchHunk, PatchLine};

#[derive(Debug, Clone, Default)]
pub struct TransactionStats {
    pub files_created: usize,
    pub files_modified: usize,
    pub files_deleted: usize,
    pub hunks_applied: usize,
    pub average_drift: f32,
}

#[derive(Debug)]
pub struct PatchTransaction {
    cwd: PathBuf,
    // Path -> (OriginalContent, NewContent, IsCreate, IsDelete)
    pub pending_writes: HashMap<PathBuf, FilePlan>,
    pub stats: TransactionStats,
}

#[derive(Debug, Clone)]
pub enum FilePlan {
    Create { content: String },
    Modify { original: String, updated: String },
    Delete { original: String },
}

impl PatchTransaction {
    pub fn new(cwd: impl Into<PathBuf>) -> Self {
        Self {
            cwd: cwd.into(),
            pending_writes: HashMap::new(),
            stats: TransactionStats::default(),
        }
    }

    /// Phase 1: Validates and plans all file changes in memory without touching disk.
    pub fn prepare(&mut self, patch: &ParsedPatch) -> Result<()> {
        let mut total_drift: isize = 0;
        let mut hunk_count: usize = 0;

        for action in &patch.actions {
            match action {
                FileAction::Create { path, content } => {
                    let full_path = self.resolve_path(path)?;
                    if full_path.exists() {
                        return Err(anyhow!(
                            "Cannot create file that already exists: {} (use Update File instead)",
                            path.display()
                        ));
                    }
                    self.reject_duplicate_path(&full_path, path)?;
                    self.pending_writes.insert(
                        full_path,
                        FilePlan::Create {
                            content: content.clone(),
                        },
                    );
                    self.stats.files_created += 1;
                }
                FileAction::Delete { path } => {
                    let full_path = self.resolve_path(path)?;
                    if !full_path.exists() {
                        return Err(anyhow!(
                            "Cannot delete nonexistent file: {}",
                            path.display()
                        ));
                    }
                    self.reject_duplicate_path(&full_path, path)?;
                    let original = fs::read_to_string(&full_path).with_context(|| {
                        format!("Failed to read file before delete: {}", full_path.display())
                    })?;
                    self.pending_writes
                        .insert(full_path, FilePlan::Delete { original });
                    self.stats.files_deleted += 1;
                }
                FileAction::Modify { path, hunks } => {
                    let full_path = self.resolve_path(path)?;
                    if !full_path.exists() {
                        return Err(anyhow!(
                            "Cannot modify nonexistent file: {}",
                            path.display()
                        ));
                    }
                    self.reject_duplicate_path(&full_path, path)?;

                    let original = fs::read_to_string(&full_path).with_context(|| {
                        format!("Failed to read file for patch: {}", full_path.display())
                    })?;

                    let (updated, hunks_done, drift_sum) = Self::apply_hunks_to_content(
                        &original,
                        hunks,
                        path.to_str().unwrap_or(""),
                    )?;

                    // Syntax validation
                    let ext = path.extension().and_then(|e| e.to_str());
                    SyntaxValidator::validate_structural_balance(&updated, ext).with_context(
                        || {
                            format!(
                                "Patch introduces structural delimiter syntax error into {}",
                                path.display()
                            )
                        },
                    )?;

                    self.pending_writes
                        .insert(full_path, FilePlan::Modify { original, updated });
                    self.stats.files_modified += 1;
                    hunk_count += hunks_done;
                    total_drift += drift_sum;
                }
                FileAction::Rename {
                    old_path,
                    new_path,
                    hunks,
                } => {
                    let old_full = self.resolve_path(old_path)?;
                    let new_full = self.resolve_path(new_path)?;

                    if !old_full.exists() {
                        return Err(anyhow!(
                            "Cannot rename nonexistent file: {}",
                            old_path.display()
                        ));
                    }
                    if new_full.exists() {
                        return Err(anyhow!(
                            "Cannot rename to a file that already exists: {}",
                            new_path.display()
                        ));
                    }
                    self.reject_duplicate_path(&old_full, old_path)?;
                    self.reject_duplicate_path(&new_full, new_path)?;

                    let original = fs::read_to_string(&old_full)?;
                    let (updated, hunks_done, drift_sum) = Self::apply_hunks_to_content(
                        &original,
                        hunks,
                        old_path.to_str().unwrap_or(""),
                    )?;

                    let ext = new_path.extension().and_then(|e| e.to_str());
                    SyntaxValidator::validate_structural_balance(&updated, ext)?;

                    self.pending_writes.insert(
                        old_full,
                        FilePlan::Delete {
                            original: original.clone(),
                        },
                    );
                    self.pending_writes
                        .insert(new_full, FilePlan::Create { content: updated });
                    self.stats.files_modified += 1;
                    hunk_count += hunks_done;
                    total_drift += drift_sum;
                }
            }
        }

        self.stats.hunks_applied = hunk_count;
        if hunk_count > 0 {
            self.stats.average_drift = total_drift.abs() as f32 / hunk_count as f32;
        }

        Ok(())
    }

    /// Phase 2: Commits the planned writes to disk atomically.
    /// If an I/O error occurs, rolls back previously committed files.
    pub fn commit(self) -> Result<TransactionStats> {
        let mut committed: Vec<(PathBuf, FilePlan)> = Vec::new();

        for (path, plan) in &self.pending_writes {
            let res = match plan {
                FilePlan::Create { content } => {
                    if let Some(parent) = path.parent() {
                        fs::create_dir_all(parent)?;
                    }
                    fs::write(path, content)
                }
                FilePlan::Modify { updated, .. } => fs::write(path, updated),
                FilePlan::Delete { .. } => {
                    if path.exists() {
                        fs::remove_file(path)
                    } else {
                        Ok(())
                    }
                }
            };

            if let Err(e) = res {
                // Rollback previously committed changes
                tracing::error!(
                    "I/O error during patch commit: {}. Rolling back transaction...",
                    e
                );
                for (rb_path, rb_plan) in committed.iter().rev() {
                    match rb_plan {
                        FilePlan::Create { .. } => {
                            let _ = fs::remove_file(rb_path);
                        }
                        FilePlan::Modify { original, .. } => {
                            let _ = fs::write(rb_path, original);
                        }
                        FilePlan::Delete { original } => {
                            let _ = fs::write(rb_path, original);
                        }
                    }
                }
                return Err(e)
                    .context("Failed committing atomic patch transaction; rollback succeeded");
            }

            committed.push((path.clone(), plan.clone()));
        }

        Ok(self.stats)
    }

    fn apply_hunks_to_content(
        content: &str,
        hunks: &[PatchHunk],
        path_label: &str,
    ) -> Result<(String, usize, isize)> {
        // §7.4 line-ending preservation: the file's first-seen ending style
        // applies to inserted lines too, and the trailing-ending state is
        // preserved (LF / CRLF / CR are all kept intact across a patch).
        let line_ending = detect_line_ending(content);
        let mut target_lines: Vec<String> = split_lines_any(content);
        let mut total_drift: isize = 0;
        let mut hunks_applied = 0;
        let mut current_search_idx = 0;

        for (h_idx, hunk) in hunks.iter().enumerate() {
            match hunk {
                PatchHunk::SearchReplace {
                    search_lines,
                    replace_lines,
                } => {
                    let hint = current_search_idx;
                    let is_eof = h_idx + 1 == hunks.len();
                    let match_loc =
                        FuzzyMatcher::find_match(&target_lines, search_lines, Some(hint), 100)
                            .or_else(|| {
                                FuzzyMatcher::seek_sequence(
                                    &target_lines,
                                    search_lines,
                                    current_search_idx,
                                    is_eof,
                                )
                                .map(|idx| {
                                    crate::fuzzy_matcher::MatchLocation {
                                        start_line: idx,
                                        matched_line_count: search_lines
                                            .len()
                                            .min(target_lines.len().saturating_sub(idx)),
                                        leading_indent_override: None,
                                        drift: (idx as isize) - (hint as isize),
                                    }
                                })
                            })
                            .ok_or_else(|| {
                                anyhow!(
                                    "Failed to match search block in {}\nBlock: {:?}",
                                    path_label,
                                    search_lines.first().unwrap_or(&String::new())
                                )
                            })?;

                    let start = match_loc.start_line;
                    let count = match_loc.matched_line_count;

                    let mut adjusted_replace = replace_lines.clone();
                    if let Some(indent) = match_loc.leading_indent_override {
                        adjusted_replace = Self::reindent_lines(&adjusted_replace, &indent);
                    }

                    target_lines.splice(start..start + count, adjusted_replace.clone());
                    total_drift += (adjusted_replace.len() as isize) - (count as isize);
                    current_search_idx = start + adjusted_replace.len();
                    hunks_applied += 1;
                }
                PatchHunk::Unified {
                    old_start, lines, ..
                } => {
                    let mut search_lines = Vec::new();
                    let mut replace_lines = Vec::new();

                    for l in lines {
                        match l {
                            PatchLine::Context(c) => {
                                search_lines.push(c.clone());
                                replace_lines.push(c.clone());
                            }
                            PatchLine::Delete(d) => {
                                search_lines.push(d.clone());
                            }
                            PatchLine::Add(a) => {
                                replace_lines.push(a.clone());
                            }
                        }
                    }

                    let raw_hint = old_start.saturating_sub(1);
                    let drifted_hint = ((raw_hint as isize) + total_drift).max(0) as usize;
                    let hint = drifted_hint.max(current_search_idx);
                    let is_eof = h_idx + 1 == hunks.len();

                    let match_loc =
                        FuzzyMatcher::find_match(&target_lines, &search_lines, Some(hint), 100)
                            .or_else(|| {
                                FuzzyMatcher::seek_sequence(
                                    &target_lines,
                                    &search_lines,
                                    current_search_idx,
                                    is_eof,
                                )
                                .map(|idx| {
                                    crate::fuzzy_matcher::MatchLocation {
                                        start_line: idx,
                                        matched_line_count: search_lines
                                            .len()
                                            .min(target_lines.len().saturating_sub(idx)),
                                        leading_indent_override: None,
                                        drift: (idx as isize) - (raw_hint as isize),
                                    }
                                })
                            })
                            .ok_or_else(|| {
                                anyhow!(
                                    "Failed to locate hunk #{} in {}\nContext: {:?}",
                                    h_idx + 1,
                                    path_label,
                                    search_lines.first().unwrap_or(&String::new())
                                )
                            })?;

                    let start = match_loc.start_line;
                    let count = match_loc.matched_line_count;

                    target_lines.splice(start..start + count, replace_lines.clone());
                    total_drift += (replace_lines.len() as isize) - (count as isize);
                    current_search_idx = start + replace_lines.len();
                    hunks_applied += 1;
                }
            }
        }

        let mut out = target_lines.join(line_ending);
        // Preserve the original trailing-ending state (including CRLF/CR).
        if content.ends_with('\n') || content.ends_with('\r') {
            out.push_str(line_ending);
        }

        Ok((out, hunks_applied, total_drift))
    }

    fn reindent_lines(lines: &[String], target_indent: &str) -> Vec<String> {
        let mut out = Vec::new();
        let min_current_indent = lines
            .iter()
            .filter(|l| !l.trim().is_empty())
            .map(|l| l.chars().take_while(|c| c.is_whitespace()).count())
            .min()
            .unwrap_or(0);

        for line in lines {
            if line.trim().is_empty() {
                out.push(String::new());
            } else {
                let stripped = if line.len() >= min_current_indent {
                    &line[min_current_indent..]
                } else {
                    line.trim_start()
                };
                out.push(format!("{}{}", target_indent, stripped));
            }
        }
        out
    }

    fn resolve_path(&self, p: &Path) -> Result<PathBuf> {
        for component in p.components() {
            if matches!(component, std::path::Component::ParentDir) {
                return Err(anyhow!(
                    "Directory traversal detected in patch path: '{}'",
                    p.display()
                ));
            }
        }

        let canonical_cwd = self.cwd.canonicalize().unwrap_or_else(|_| self.cwd.clone());

        let target = if p.is_absolute() {
            p.to_path_buf()
        } else {
            self.cwd.join(p)
        };

        // If target or an ancestor exists, check canonical path
        if let Ok(canon_target) = target.canonicalize() {
            if !canon_target.starts_with(&canonical_cwd) {
                return Err(anyhow!(
                    "Patch path '{}' escapes workspace root '{}' (resolved to '{}')",
                    p.display(),
                    self.cwd.display(),
                    canon_target.display()
                ));
            }
            Ok(canon_target)
        } else {
            // Find closest existing parent to check for symlink escapes
            let mut ancestor = target.parent();
            while let Some(anc) = ancestor {
                if let Ok(canon_anc) = anc.canonicalize() {
                    if !canon_anc.starts_with(&canonical_cwd) {
                        return Err(anyhow!(
                            "Symlink escape detected in patch path '{}': ancestor '{}' resolves outside workspace root to '{}'",
                            p.display(),
                            anc.display(),
                            canon_anc.display()
                        ));
                    }
                    break;
                }
                ancestor = anc.parent();
            }

            // Normal relative path within cwd
            if !target.starts_with(&self.cwd) && !target.starts_with(&canonical_cwd) {
                return Err(anyhow!(
                    "Patch path '{}' escapes workspace root '{}'",
                    p.display(),
                    self.cwd.display()
                ));
            }

            Ok(target)
        }
    }

    /// Upstream parity (report §7.4): duplicate paths within one patch are
    /// rejected instead of silently overwriting earlier plans.
    fn reject_duplicate_path(&self, full_path: &Path, patch_label: &Path) -> Result<()> {
        if self.pending_writes.contains_key(full_path) {
            return Err(anyhow!(
                "Duplicate path in patch transaction: {} (a file may only appear once per patch)",
                patch_label.display()
            ));
        }
        Ok(())
    }
}

/// Detects the first-seen line-ending style in the original content (report
/// §7.4): the first `\n` or `\r` decides — `\r\n` → CRLF, lone `\n` → LF,
/// lone `\r` → CR. Content without any line endings defaults to LF.
fn detect_line_ending(content: &str) -> &'static str {
    let bytes = content.as_bytes();
    for i in 0..bytes.len() {
        match bytes[i] {
            b'\n' => return "\n",
            b'\r' => {
                return if bytes.get(i + 1) == Some(&b'\n') {
                    "\r\n"
                } else {
                    "\r"
                };
            }
            _ => {}
        }
    }
    "\n"
}

/// Splits content into lines on ANY of LF / CRLF / CR, dropping the endings
/// (matching the patch-side line normalization). A trailing final newline does
/// not produce an extra empty line.
fn split_lines_any(content: &str) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current = String::new();
    let mut chars = content.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\r' => {
                if chars.peek() == Some(&'\n') {
                    chars.next();
                }
                lines.push(std::mem::take(&mut current));
            }
            '\n' => lines.push(std::mem::take(&mut current)),
            _ => current.push(c),
        }
    }
    if !current.is_empty() {
        lines.push(current);
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::unified_diff::UnifiedDiffParser;
    use tempfile::tempdir;

    #[test]
    fn test_atomic_transaction_success() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("code.rs");
        fs::write(&file_path, "fn hello() {\n    let a = 1;\n}\n").unwrap();

        let patch_text = format!(
            "File: {}\n<<<<<<<\n SEARCH\n    let a = 1;\n=======\n    let a = 2;\n    let b = 3;\n>>>>>>> REPLACE\n",
            file_path.display()
        ).replace("<<<<<<<\n SEARCH", "<<<<<<< SEARCH");

        let parsed = crate::unified_diff::UnifiedDiffParser::parse(&patch_text).unwrap();
        let mut tx = PatchTransaction::new(dir.path());
        tx.prepare(&parsed).unwrap();
        let stats = tx.commit().unwrap();

        assert_eq!(stats.files_modified, 1);
        let updated = fs::read_to_string(&file_path).unwrap();
        assert!(updated.contains("let a = 2;"));
        assert!(updated.contains("let b = 3;"));
    }

    #[test]
    fn test_crlf_line_endings_preserved() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("crlf.txt");
        fs::write(&file_path, "line1\r\nline2\r\nline3\r\n").unwrap();

        let patch_text = "\
--- a/crlf.txt
+++ b/crlf.txt
@@ -1,3 +1,4 @@
 line1
+inserted
 line2
 line3
";
        let parsed = UnifiedDiffParser::parse(patch_text).unwrap();
        let mut tx = PatchTransaction::new(dir.path());
        tx.prepare(&parsed).unwrap();
        let stats = tx.commit().unwrap();
        assert_eq!(stats.files_modified, 1);

        let updated = fs::read_to_string(&file_path).unwrap();
        assert_eq!(updated, "line1\r\ninserted\r\nline2\r\nline3\r\n");
    }

    #[test]
    fn test_trailing_newline_state_preserved() {
        let dir = tempdir().unwrap();
        // File WITHOUT a trailing newline.
        let file_path = dir.path().join("tailless.txt");
        fs::write(&file_path, "alpha\nbeta").unwrap();

        let patch_text = "\
--- a/tailless.txt
+++ b/tailless.txt
@@ -1,2 +1,2 @@
 alpha
-beta
+gamma
";
        let parsed = UnifiedDiffParser::parse(patch_text).unwrap();
        let mut tx = PatchTransaction::new(dir.path());
        tx.prepare(&parsed).unwrap();
        tx.commit().unwrap();

        let updated = fs::read_to_string(&file_path).unwrap();
        assert_eq!(updated, "alpha\ngamma");
    }

    #[test]
    fn test_cr_only_line_endings_preserved() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("cr.txt");
        fs::write(&file_path, "one\rtwo\rthree\r").unwrap();

        let patch_text = "\
--- a/cr.txt
+++ b/cr.txt
@@ -1,3 +1,4 @@
 one
+mid
 two
 three
";
        let parsed = UnifiedDiffParser::parse(patch_text).unwrap();
        let mut tx = PatchTransaction::new(dir.path());
        tx.prepare(&parsed).unwrap();
        tx.commit().unwrap();

        let updated = fs::read_to_string(&file_path).unwrap();
        assert_eq!(updated, "one\rmid\rtwo\rthree\r");
    }

    #[test]
    fn test_duplicate_path_rejected() {
        let dir = tempdir().unwrap();
        let a = dir.path().join("a.txt");
        fs::write(&a, "hello\n").unwrap();

        // Two Modify actions on the SAME path in one patch.
        let mk_action = |hunk: &str| FileAction::Modify {
            path: PathBuf::from("a.txt"),
            hunks: vec![PatchHunk::SearchReplace {
                search_lines: vec![hunk.to_string()],
                replace_lines: vec!["x".to_string()],
            }],
        };
        let parsed = ParsedPatch {
            actions: vec![mk_action("hello"), mk_action("hello")],
        };

        let mut tx = PatchTransaction::new(dir.path());
        let err = tx.prepare(&parsed).unwrap_err();
        assert!(err.to_string().contains("Duplicate path"), "got: {err}");
    }

    #[test]
    fn test_create_existing_file_rejected() {
        let dir = tempdir().unwrap();
        let a = dir.path().join("exists.txt");
        fs::write(&a, "original\n").unwrap();

        let parsed = ParsedPatch {
            actions: vec![FileAction::Create {
                path: PathBuf::from("exists.txt"),
                content: "overwritten\n".to_string(),
            }],
        };

        let mut tx = PatchTransaction::new(dir.path());
        let err = tx.prepare(&parsed).unwrap_err();
        assert!(err.to_string().contains("already exists"), "got: {err}");
        // The original file is untouched.
        assert_eq!(fs::read_to_string(&a).unwrap(), "original\n");
    }

    #[test]
    fn test_patch_path_traversal_rejected() {
        let dir = tempdir().unwrap();
        let parsed = ParsedPatch {
            actions: vec![FileAction::Create {
                path: PathBuf::from("../../escaped.txt"),
                content: "malicious\n".to_string(),
            }],
        };

        let mut tx = PatchTransaction::new(dir.path());
        let err = tx.prepare(&parsed).unwrap_err();
        assert!(
            err.to_string().contains("Directory traversal detected")
                || err.to_string().contains("escapes workspace root"),
            "Expected traversal error, got: {err}"
        );
    }

    #[test]
    fn test_patch_absolute_escape_rejected() {
        let dir = tempdir().unwrap();
        let outside = tempdir().unwrap();
        let outside_file = outside.path().join("evil.txt");

        let parsed = ParsedPatch {
            actions: vec![FileAction::Create {
                path: outside_file,
                content: "malicious\n".to_string(),
            }],
        };

        let mut tx = PatchTransaction::new(dir.path());
        let err = tx.prepare(&parsed).unwrap_err();
        assert!(
            err.to_string().contains("outside workspace root")
                || err.to_string().contains("escapes workspace root"),
            "Expected escape error, got: {err}"
        );
    }
}
