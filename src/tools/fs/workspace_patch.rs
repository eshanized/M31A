//! Multi-file workspace diff and patch application engine (Issue 14).
//!
//! Parses and safely applies multi-file unified git diffs, supporting file creation,
//! file deletion, renames, and modifications with dry-run atomicity and whitespace tolerance.

use crate::capability::traits::fs::FileSystemService;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Single diff hunk inside a unified diff.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PatchHunk {
    pub old_start: usize,
    pub old_count: usize,
    pub new_start: usize,
    pub new_count: usize,
    pub lines: Vec<String>,
}

/// Action to be performed on a file as part of a multi-file patch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FilePatchAction {
    Modify {
        path: String,
        hunks: Vec<PatchHunk>,
    },
    Create {
        path: String,
        content: String,
    },
    Delete {
        path: String,
    },
    Rename {
        old_path: String,
        new_path: String,
        hunks: Vec<PatchHunk>,
    },
}

/// Detailed diagnostic when patch parsing or application fails.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
pub enum PatchDiagnostic {
    #[error("Parse error: {0}")]
    ParseError(String),
    #[error("File not found for patching: '{file_path}'")]
    FileNotFound { file_path: String },
    #[error("Cannot create file '{file_path}': file already exists")]
    FileAlreadyExists { file_path: String },
    #[error("Hunk {hunk_index} failed to apply to '{file_path}': {reason}\nContext:\n{context}")]
    HunkApplicationFailed {
        file_path: String,
        hunk_index: usize,
        reason: String,
        context: String,
    },
    #[error("IO/Filesystem error: {0}")]
    FsError(String),
}

/// Structured outcome of a workspace patch application.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspacePatchReport {
    pub files_modified: Vec<String>,
    pub files_created: Vec<String>,
    pub files_deleted: Vec<String>,
    pub files_renamed: Vec<(String, String)>,
    pub total_hunks_applied: usize,
    pub success: bool,
}

/// Multi-file patch engine.
pub struct WorkspacePatchEngine;

impl WorkspacePatchEngine {
    /// Parse a unified diff (single or multi-file) into concrete file actions.
    pub fn parse(diff: &str) -> Result<Vec<FilePatchAction>, PatchDiagnostic> {
        let mut actions = Vec::new();
        let lines: Vec<&str> = diff.lines().collect();
        let mut i = 0;

        while i < lines.len() {
            let line = lines[i];

            if line.starts_with("diff --git ") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() < 4 {
                    i += 1;
                    continue;
                }
                let raw_old = parts[2].trim_start_matches("a/");
                let raw_new = parts[3].trim_start_matches("b/");

                i += 1;
                let mut is_new = false;
                let mut is_deleted = false;
                let mut is_rename = false;
                let mut rename_from = None;
                let mut rename_to = None;

                while i < lines.len()
                    && !lines[i].starts_with("diff --git ")
                    && !lines[i].starts_with("--- ")
                {
                    let h_line = lines[i];
                    if h_line.starts_with("new file mode") {
                        is_new = true;
                    } else if h_line.starts_with("deleted file mode") {
                        is_deleted = true;
                    } else if h_line.starts_with("rename from ") {
                        is_rename = true;
                        rename_from =
                            Some(h_line.trim_start_matches("rename from ").trim().to_string());
                    } else if h_line.starts_with("rename to ") {
                        is_rename = true;
                        rename_to =
                            Some(h_line.trim_start_matches("rename to ").trim().to_string());
                    }
                    i += 1;
                }

                // Check for --- and +++ lines
                let mut old_file_header = None;
                let mut new_file_header = None;

                if i < lines.len() && lines[i].starts_with("--- ") {
                    old_file_header = Some(clean_diff_path(lines[i].trim_start_matches("--- ")));
                    i += 1;
                }
                if i < lines.len() && lines[i].starts_with("+++ ") {
                    new_file_header = Some(clean_diff_path(lines[i].trim_start_matches("+++ ")));
                    i += 1;
                }

                let effective_old = rename_from
                    .unwrap_or_else(|| old_file_header.unwrap_or_else(|| raw_old.to_string()));
                let effective_new = rename_to
                    .unwrap_or_else(|| new_file_header.unwrap_or_else(|| raw_new.to_string()));

                if effective_old == "/dev/null" {
                    is_new = true;
                }
                if effective_new == "/dev/null" {
                    is_deleted = true;
                }

                // Parse hunks
                let mut hunks = Vec::new();
                while i < lines.len() && !lines[i].starts_with("diff --git ") {
                    if lines[i].starts_with("@@ ") {
                        let hunk = Self::parse_hunk(&lines, &mut i)?;
                        hunks.push(hunk);
                    } else {
                        i += 1;
                    }
                }

                if is_deleted {
                    actions.push(FilePatchAction::Delete {
                        path: effective_old,
                    });
                } else if is_new {
                    let mut content = String::new();
                    for hunk in &hunks {
                        for hline in &hunk.lines {
                            if let Some(stripped) = hline.strip_prefix('+') {
                                content.push_str(stripped);
                                content.push('\n');
                            }
                        }
                    }
                    actions.push(FilePatchAction::Create {
                        path: effective_new,
                        content,
                    });
                } else if is_rename {
                    actions.push(FilePatchAction::Rename {
                        old_path: effective_old,
                        new_path: effective_new,
                        hunks,
                    });
                } else {
                    actions.push(FilePatchAction::Modify {
                        path: effective_new,
                        hunks,
                    });
                }
            } else if line.starts_with("--- ")
                && i + 1 < lines.len()
                && lines[i + 1].starts_with("+++ ")
            {
                // Header-only diff without "diff --git"
                let old_p = clean_diff_path(line.trim_start_matches("--- "));
                let new_p = clean_diff_path(lines[i + 1].trim_start_matches("+++ "));
                i += 2;

                let is_new = old_p == "/dev/null";
                let is_deleted = new_p == "/dev/null";

                let mut hunks = Vec::new();
                while i < lines.len()
                    && !lines[i].starts_with("--- ")
                    && !lines[i].starts_with("diff --git ")
                {
                    if lines[i].starts_with("@@ ") {
                        let hunk = Self::parse_hunk(&lines, &mut i)?;
                        hunks.push(hunk);
                    } else {
                        i += 1;
                    }
                }

                if is_deleted {
                    actions.push(FilePatchAction::Delete { path: old_p });
                } else if is_new {
                    let mut content = String::new();
                    for hunk in &hunks {
                        for hline in &hunk.lines {
                            if let Some(stripped) = hline.strip_prefix('+') {
                                content.push_str(stripped);
                                content.push('\n');
                            }
                        }
                    }
                    actions.push(FilePatchAction::Create {
                        path: new_p,
                        content,
                    });
                } else {
                    actions.push(FilePatchAction::Modify { path: new_p, hunks });
                }
            } else {
                i += 1;
            }
        }

        Ok(actions)
    }

    fn parse_hunk(lines: &[&str], i: &mut usize) -> Result<PatchHunk, PatchDiagnostic> {
        let header = lines[*i];
        let parts: Vec<&str> = header.split("@@").collect();
        if parts.len() < 3 {
            return Err(PatchDiagnostic::ParseError(format!(
                "Invalid hunk header: {}",
                header
            )));
        }

        let ranges = parts[1].trim();
        let range_parts: Vec<&str> = ranges.split_whitespace().collect();
        let (old_start, old_count) = if !range_parts.is_empty() {
            parse_range(range_parts[0].trim_start_matches('-'))
        } else {
            (1, 0)
        };
        let (new_start, new_count) = if range_parts.len() > 1 {
            parse_range(range_parts[1].trim_start_matches('+'))
        } else {
            (1, 0)
        };

        *i += 1;
        let mut hunk_lines = Vec::new();

        while *i < lines.len() {
            let l = lines[*i];
            if l.starts_with("@@ ") || l.starts_with("diff --git ") || l.starts_with("--- ") {
                break;
            }
            if l.starts_with('+') || l.starts_with('-') || l.starts_with(' ') || l.is_empty() {
                hunk_lines.push(l.to_string());
                *i += 1;
            } else {
                break;
            }
        }

        Ok(PatchHunk {
            old_start,
            old_count,
            new_start,
            new_count,
            lines: hunk_lines,
        })
    }

    /// Dry-run test applicability of all actions across the filesystem.
    /// Ensures atomicity: if any action fails, no mutation takes place.
    pub async fn test_applicable(
        fs: &dyn FileSystemService,
        actions: &[FilePatchAction],
    ) -> Result<Vec<(String, Option<String>)>, PatchDiagnostic> {
        let mut simulated_outputs = Vec::new();

        for action in actions {
            match action {
                FilePatchAction::Create { path, content } => {
                    let p = Path::new(path);
                    if fs.file_metadata(p).await.is_ok() {
                        return Err(PatchDiagnostic::FileAlreadyExists {
                            file_path: path.clone(),
                        });
                    }
                    simulated_outputs.push((path.clone(), Some(content.clone())));
                }
                FilePatchAction::Delete { path } => {
                    let p = Path::new(path);
                    if fs.file_metadata(p).await.is_err() {
                        return Err(PatchDiagnostic::FileNotFound {
                            file_path: path.clone(),
                        });
                    }
                    simulated_outputs.push((path.clone(), None));
                }
                FilePatchAction::Modify { path, hunks } => {
                    let p = Path::new(path);
                    let raw = fs.read_file(p, None, None).await.map_err(|_| {
                        PatchDiagnostic::FileNotFound {
                            file_path: path.clone(),
                        }
                    })?;
                    let original = String::from_utf8(raw)
                        .map_err(|e| PatchDiagnostic::ParseError(format!("non-UTF8 file: {e}")))?;
                    let patched = Self::apply_hunks(&original, path, hunks)?;
                    simulated_outputs.push((path.clone(), Some(patched)));
                }
                FilePatchAction::Rename {
                    old_path,
                    new_path: _,
                    hunks,
                } => {
                    let p = Path::new(old_path);
                    let raw = fs.read_file(p, None, None).await.map_err(|_| {
                        PatchDiagnostic::FileNotFound {
                            file_path: old_path.clone(),
                        }
                    })?;
                    let original = String::from_utf8(raw)
                        .map_err(|e| PatchDiagnostic::ParseError(format!("non-UTF8 file: {e}")))?;
                    let patched = Self::apply_hunks(&original, old_path, hunks)?;
                    simulated_outputs.push((old_path.clone(), Some(patched)));
                }
            }
        }

        Ok(simulated_outputs)
    }

    /// Apply all actions atomically: tests first, then applies.
    pub async fn apply_workspace_patch(
        fs: &dyn FileSystemService,
        patch: &str,
    ) -> Result<WorkspacePatchReport, PatchDiagnostic> {
        let actions = Self::parse(patch)?;
        if actions.is_empty() {
            return Ok(WorkspacePatchReport {
                files_modified: Vec::new(),
                files_created: Vec::new(),
                files_deleted: Vec::new(),
                files_renamed: Vec::new(),
                total_hunks_applied: 0,
                success: true,
            });
        }

        // 1. Dry run
        let simulated = Self::test_applicable(fs, &actions).await?;

        // 2. Commit changes
        let mut report = WorkspacePatchReport {
            files_modified: Vec::new(),
            files_created: Vec::new(),
            files_deleted: Vec::new(),
            files_renamed: Vec::new(),
            total_hunks_applied: 0,
            success: true,
        };

        for (action, (_, simulated_content)) in actions.iter().zip(simulated) {
            match action {
                FilePatchAction::Create { path, .. } => {
                    if let Some(content) = simulated_content {
                        fs.write_file(Path::new(path), content.as_bytes())
                            .await
                            .map_err(|e| PatchDiagnostic::FsError(e.to_string()))?;
                        report.files_created.push(path.clone());
                    }
                }
                FilePatchAction::Delete { path } => {
                    fs.delete_file(Path::new(path))
                        .await
                        .map_err(|e| PatchDiagnostic::FsError(e.to_string()))?;
                    report.files_deleted.push(path.clone());
                }
                FilePatchAction::Modify { path, hunks } => {
                    if let Some(content) = simulated_content {
                        fs.write_file(Path::new(path), content.as_bytes())
                            .await
                            .map_err(|e| PatchDiagnostic::FsError(e.to_string()))?;
                        report.files_modified.push(path.clone());
                        report.total_hunks_applied += hunks.len();
                    }
                }
                FilePatchAction::Rename {
                    old_path,
                    new_path,
                    hunks,
                } => {
                    if let Some(content) = simulated_content {
                        fs.rename_file(Path::new(old_path), Path::new(new_path))
                            .await
                            .map_err(|e| PatchDiagnostic::FsError(e.to_string()))?;
                        fs.write_file(Path::new(new_path), content.as_bytes())
                            .await
                            .map_err(|e| PatchDiagnostic::FsError(e.to_string()))?;
                        report
                            .files_renamed
                            .push((old_path.clone(), new_path.clone()));
                        report.total_hunks_applied += hunks.len();
                    }
                }
            }
        }

        Ok(report)
    }

    fn apply_hunks(
        original: &str,
        file_path: &str,
        hunks: &[PatchHunk],
    ) -> Result<String, PatchDiagnostic> {
        let orig_lines: Vec<&str> = original.lines().collect();
        let mut result_lines: Vec<String> = Vec::new();
        let mut orig_idx = 0;

        for (hunk_idx, hunk) in hunks.iter().enumerate() {
            let mut context_lines = Vec::new();
            let mut new_lines = Vec::new();

            for line in &hunk.lines {
                if let Some(stripped) = line.strip_prefix(' ') {
                    context_lines.push(stripped);
                    new_lines.push(stripped.to_string());
                } else if let Some(stripped) = line.strip_prefix('-') {
                    context_lines.push(stripped);
                } else if let Some(stripped) = line.strip_prefix('+') {
                    new_lines.push(stripped.to_string());
                }
            }

            let match_idx =
                find_hunk_position(&orig_lines, &context_lines, orig_idx, hunk.old_start);
            let found = match match_idx {
                Some(idx) => idx,
                None => {
                    return Err(PatchDiagnostic::HunkApplicationFailed {
                        file_path: file_path.to_string(),
                        hunk_index: hunk_idx + 1,
                        reason: "Context lines not found in file".to_string(),
                        context: context_lines.join("\n"),
                    });
                }
            };

            for l in &orig_lines[orig_idx..found] {
                result_lines.push((*l).to_string());
            }

            for nl in new_lines {
                result_lines.push(nl);
            }

            orig_idx = found + context_lines.len();
        }

        if orig_idx < orig_lines.len() {
            for l in &orig_lines[orig_idx..] {
                result_lines.push((*l).to_string());
            }
        }

        let mut out = result_lines.join("\n");
        if original.ends_with('\n') {
            out.push('\n');
        }
        Ok(out)
    }
}

fn clean_diff_path(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.starts_with("a/") || trimmed.starts_with("b/") {
        trimmed[2..].to_string()
    } else {
        trimmed.to_string()
    }
}

fn parse_range(range_str: &str) -> (usize, usize) {
    let parts: Vec<&str> = range_str.split(',').collect();
    let start = parts[0].parse::<usize>().unwrap_or(1);
    let count = if parts.len() > 1 {
        parts[1].parse::<usize>().unwrap_or(1)
    } else {
        1
    };
    (start, count)
}

fn find_hunk_position(
    haystack: &[&str],
    needle: &[&str],
    search_start: usize,
    expected_start: usize,
) -> Option<usize> {
    if needle.is_empty() {
        return Some(search_start);
    }

    // Try exact position first (1-indexed converted to 0-indexed)
    if expected_start > 0
        && expected_start > search_start
        && expected_start - 1 + needle.len() <= haystack.len()
    {
        let pos = expected_start - 1;
        if haystack[pos..pos + needle.len()] == *needle {
            return Some(pos);
        }
    }

    // Search from search_start forward
    for i in search_start..=haystack.len().saturating_sub(needle.len()) {
        if haystack[i..i + needle.len()] == *needle {
            return Some(i);
        }
    }

    // Search with whitespace trimming (fuzz factor)
    for i in search_start..=haystack.len().saturating_sub(needle.len()) {
        let matches = haystack[i..i + needle.len()]
            .iter()
            .zip(needle.iter())
            .all(|(a, b)| a.trim() == b.trim());
        if matches {
            return Some(i);
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_multi_file_patch() {
        let diff = r#"diff --git a/src/foo.rs b/src/foo.rs
--- a/src/foo.rs
+++ b/src/foo.rs
@@ -1,3 +1,3 @@
 fn main() {
-    println!("hello");
+    println!("world");
 }
diff --git a/src/bar.rs b/src/bar.rs
new file mode 100644
--- /dev/null
+++ b/src/bar.rs
@@ -0,0 +1,2 @@
+pub fn bar() {}
"#;
        let actions = WorkspacePatchEngine::parse(diff).unwrap();
        assert_eq!(actions.len(), 2);
        assert!(
            matches!(&actions[0], FilePatchAction::Modify { path, .. } if path == "src/foo.rs")
        );
        assert!(
            matches!(&actions[1], FilePatchAction::Create { path, .. } if path == "src/bar.rs")
        );
    }
}
