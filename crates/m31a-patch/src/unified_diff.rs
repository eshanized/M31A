//! Unified diff and search/replace patch parser.

use anyhow::{Result, anyhow};
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PatchHunk {
    Unified {
        old_start: usize,
        old_count: usize,
        new_start: usize,
        new_count: usize,
        context_header: Option<String>,
        lines: Vec<PatchLine>,
    },
    SearchReplace {
        search_lines: Vec<String>,
        replace_lines: Vec<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PatchLine {
    Context(String),
    Add(String),
    Delete(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileAction {
    Modify {
        path: PathBuf,
        hunks: Vec<PatchHunk>,
    },
    Create {
        path: PathBuf,
        content: String,
    },
    Delete {
        path: PathBuf,
    },
    Rename {
        old_path: PathBuf,
        new_path: PathBuf,
        hunks: Vec<PatchHunk>,
    },
}

#[derive(Debug, Clone, Default)]
pub struct ParsedPatch {
    pub actions: Vec<FileAction>,
}

pub struct UnifiedDiffParser;

impl UnifiedDiffParser {
    /// Parses any patch text into structured file actions.
    /// Supports the V4A format (`*** Begin Patch`), unified diffs (git diff),
    /// and search/replace blocks.
    pub fn parse(text: &str) -> Result<ParsedPatch> {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return Ok(ParsedPatch::default());
        }

        // 0. V4A structured patch format (*** Begin Patch delimiter) — checked first
        // since its `*** Begin Patch` marker is unambiguous (report §7.4).
        if text.contains("*** Begin Patch") {
            return crate::v4a::parse_v4a_patch(text);
        }

        // 1. Try search/replace blocks first if markers exist
        if text.contains("<<<<<<< SEARCH")
            && text.contains("=======")
            && text.contains(">>>>>>> REPLACE")
        {
            return Self::parse_search_replace(text);
        }

        // 2. Standard unified diff parser
        Self::parse_unified_diff(text)
    }

    fn parse_search_replace(text: &str) -> Result<ParsedPatch> {
        let mut actions = Vec::new();
        let mut current_file: Option<PathBuf> = None;
        let mut hunks = Vec::new();

        let mut lines = text.lines().peekable();
        while let Some(line) = lines.next() {
            let line_trimmed = line.trim();

            // Look for file path declarations: e.g. "File: path/to/file.rs" or "# path/to/file.rs" or "diff --git a/path b/path"
            if line_trimmed.starts_with("File:") || line_trimmed.starts_with("FILE:") {
                if let Some(path) = current_file.take()
                    && !hunks.is_empty()
                {
                    actions.push(FileAction::Modify {
                        path,
                        hunks: std::mem::take(&mut hunks),
                    });
                }
                let raw_path = line_trimmed.split_once(':').unwrap().1.trim();
                current_file = Some(PathBuf::from(raw_path.trim_matches(['`', '"', '\''])));
                continue;
            } else if line_trimmed.starts_with("### ") || line_trimmed.starts_with("## ") {
                let candidate = line_trimmed
                    .trim_start_matches('#')
                    .trim()
                    .trim_matches(['`', '"', '\'']);
                if candidate.contains('.') || candidate.contains('/') {
                    if let Some(path) = current_file.take()
                        && !hunks.is_empty()
                    {
                        actions.push(FileAction::Modify {
                            path,
                            hunks: std::mem::take(&mut hunks),
                        });
                    }
                    current_file = Some(PathBuf::from(candidate));
                    continue;
                }
            }

            if line_trimmed == "<<<<<<< SEARCH" {
                let mut search_lines = Vec::new();
                let mut replace_lines = Vec::new();
                let mut in_replace = false;

                for inner in lines.by_ref() {
                    let inner_trimmed = inner.trim();
                    if inner_trimmed == "=======" {
                        in_replace = true;
                        continue;
                    }
                    if inner_trimmed == ">>>>>>> REPLACE" {
                        break;
                    }

                    if in_replace {
                        replace_lines.push(inner.to_string());
                    } else {
                        search_lines.push(inner.to_string());
                    }
                }

                hunks.push(PatchHunk::SearchReplace {
                    search_lines,
                    replace_lines,
                });
            }
        }

        if let Some(path) = current_file
            && !hunks.is_empty()
        {
            actions.push(FileAction::Modify { path, hunks });
        }

        Ok(ParsedPatch { actions })
    }

    fn parse_unified_diff(text: &str) -> Result<ParsedPatch> {
        let mut actions = Vec::new();
        let mut current_file: Option<PathBuf> = None;
        let mut is_new_file = false;
        let mut is_deleted_file = false;
        let mut current_hunks = Vec::new();

        let lines: Vec<&str> = text.lines().collect();
        let mut i = 0;

        while i < lines.len() {
            let line = lines[i];

            if line.starts_with("diff --git ") {
                if let Some(path) = current_file.take() {
                    if is_new_file {
                        let content = Self::hunks_to_content(&current_hunks);
                        actions.push(FileAction::Create { path, content });
                    } else if is_deleted_file {
                        actions.push(FileAction::Delete { path });
                    } else if !current_hunks.is_empty() {
                        actions.push(FileAction::Modify {
                            path,
                            hunks: std::mem::take(&mut current_hunks),
                        });
                    }
                }
                is_new_file = false;
                is_deleted_file = false;

                // Extract path from b/path
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 4 {
                    let b_path = parts[3].strip_prefix("b/").unwrap_or(parts[3]);
                    current_file = Some(PathBuf::from(b_path));
                }
                i += 1;
                continue;
            }

            if line.starts_with("new file mode") {
                is_new_file = true;
                i += 1;
                continue;
            }

            if line.starts_with("deleted file mode") {
                is_deleted_file = true;
                i += 1;
                continue;
            }

            if line.starts_with("--- ") {
                let path_str = line.strip_prefix("--- ").unwrap().trim();
                if path_str == "/dev/null" {
                    is_new_file = true;
                } else if current_file.is_none() {
                    let cleaned = path_str.strip_prefix("a/").unwrap_or(path_str);
                    current_file = Some(PathBuf::from(cleaned));
                }
                i += 1;
                continue;
            }

            if line.starts_with("+++ ") {
                let path_str = line.strip_prefix("+++ ").unwrap().trim();
                if path_str == "/dev/null" {
                    is_deleted_file = true;
                } else {
                    let cleaned = path_str.strip_prefix("b/").unwrap_or(path_str);
                    current_file = Some(PathBuf::from(cleaned));
                }
                i += 1;
                continue;
            }

            if line.starts_with("@@ ") {
                let (hunk, next_i) = Self::parse_hunk(&lines, i)?;
                current_hunks.push(hunk);
                i = next_i;
                continue;
            }

            i += 1;
        }

        if let Some(path) = current_file {
            if is_new_file {
                let content = Self::hunks_to_content(&current_hunks);
                actions.push(FileAction::Create { path, content });
            } else if is_deleted_file {
                actions.push(FileAction::Delete { path });
            } else if !current_hunks.is_empty() {
                actions.push(FileAction::Modify {
                    path,
                    hunks: current_hunks,
                });
            }
        }

        Ok(ParsedPatch { actions })
    }

    fn parse_hunk(lines: &[&str], start_idx: usize) -> Result<(PatchHunk, usize)> {
        let header = lines[start_idx];
        // Parse @@ -old_start,old_count +new_start,new_count @@
        let parts: Vec<&str> = header.split("@@").collect();
        if parts.len() < 3 {
            return Err(anyhow!("Malformed hunk header: {}", header));
        }

        let range_str = parts[1].trim();
        let context_header = if !parts[2].trim().is_empty() {
            Some(parts[2].trim().to_string())
        } else {
            None
        };

        let range_parts: Vec<&str> = range_str.split_whitespace().collect();
        if range_parts.len() < 2 {
            return Err(anyhow!("Invalid hunk range: {}", range_str));
        }

        let (old_start, old_count) =
            Self::parse_range(range_parts[0].strip_prefix('-').unwrap_or(range_parts[0]))?;
        let (new_start, new_count) =
            Self::parse_range(range_parts[1].strip_prefix('+').unwrap_or(range_parts[1]))?;

        let mut hunk_lines = Vec::new();
        let mut i = start_idx + 1;

        while i < lines.len() {
            let line = lines[i];
            if line.starts_with("@@ ") || line.starts_with("diff --git ") {
                break;
            }

            if let Some(stripped) = line.strip_prefix('+') {
                hunk_lines.push(PatchLine::Add(stripped.to_string()));
            } else if let Some(stripped) = line.strip_prefix('-') {
                hunk_lines.push(PatchLine::Delete(stripped.to_string()));
            } else if let Some(stripped) = line.strip_prefix(' ') {
                hunk_lines.push(PatchLine::Context(stripped.to_string()));
            } else if line.is_empty() {
                hunk_lines.push(PatchLine::Context(String::new()));
            } else {
                // End of hunk or unexpected line
                break;
            }
            i += 1;
        }

        Ok((
            PatchHunk::Unified {
                old_start,
                old_count,
                new_start,
                new_count,
                context_header,
                lines: hunk_lines,
            },
            i,
        ))
    }

    fn parse_range(s: &str) -> Result<(usize, usize)> {
        if let Some((start, count)) = s.split_once(',') {
            Ok((start.parse::<usize>()?, count.parse::<usize>()?))
        } else {
            Ok((s.parse::<usize>()?, 1))
        }
    }

    fn hunks_to_content(hunks: &[PatchHunk]) -> String {
        let mut out = Vec::new();
        for hunk in hunks {
            match hunk {
                PatchHunk::Unified { lines, .. } => {
                    for l in lines {
                        if let PatchLine::Add(content) = l {
                            out.push(content.clone());
                        }
                    }
                }
                PatchHunk::SearchReplace { replace_lines, .. } => {
                    out.extend(replace_lines.clone());
                }
            }
        }
        out.join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_unified_diff_hunk() {
        let diff = r#"diff --git a/src/lib.rs b/src/lib.rs
--- a/src/lib.rs
+++ b/src/lib.rs
@@ -1,3 +1,3 @@
 fn hello() {
-    println!("old");
+    println!("new");
 }
"#;
        let parsed = UnifiedDiffParser::parse(diff).unwrap();
        assert_eq!(parsed.actions.len(), 1);
        if let FileAction::Modify { path, hunks } = &parsed.actions[0] {
            assert_eq!(path, &PathBuf::from("src/lib.rs"));
            assert_eq!(hunks.len(), 1);
        } else {
            panic!("Expected Modify action");
        }
    }

    #[test]
    fn test_parse_search_replace_block() {
        let patch = r#"File: src/main.rs
<<<<<<< SEARCH
let a = 1;
=======
let a = 2;
let b = 3;
>>>>>>> REPLACE
"#;
        let parsed = UnifiedDiffParser::parse(patch).unwrap();
        assert_eq!(parsed.actions.len(), 1);
        if let FileAction::Modify { path, hunks } = &parsed.actions[0] {
            assert_eq!(path, &PathBuf::from("src/main.rs"));
            assert_eq!(hunks.len(), 1);
        } else {
            panic!("Expected Modify action");
        }
    }
}
