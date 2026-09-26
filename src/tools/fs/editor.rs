//! Robust file editing primitive supporting line-range replacement, structured diffs,
//! safe insertion/deletion, and ambiguous match rejection (P3-A, AGENTS.md Rule 5 & 6).

use serde::{Deserialize, Serialize};

/// Mode of editing operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EditMode {
    /// Replace exact or line-based substring or line range.
    Replace,
    /// Insert content after the specified line number.
    InsertAfter,
    /// Insert content before the specified line number.
    InsertBefore,
    /// Delete specified line range.
    Delete,
    /// Apply unified diff patch.
    Patch,
}

/// Request to edit a file using one of the supported editing primitives.
#[derive(Debug, Clone)]
pub enum FileEditOp<'a> {
    /// Exact or indent-tolerant substring replacement.
    Substring {
        old_content: &'a str,
        new_content: &'a str,
    },
    /// Replacement of a 1-indexed line range [start_line..=end_line].
    LineRange {
        start_line: usize,
        end_line: usize,
        new_content: &'a str,
        expected_old: Option<&'a str>,
    },
    /// Insertion before or after a 1-indexed line number.
    Insert {
        line_number: usize,
        content: &'a str,
        after: bool,
    },
    /// Deletion of a 1-indexed line range [start_line..=end_line].
    Delete {
        start_line: usize,
        end_line: usize,
        expected_old: Option<&'a str>,
    },
    /// Application of a unified diff patch.
    Patch { patch: &'a str },
}

/// Successful outcome of an applied edit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditSuccess {
    pub new_content: String,
    pub lines_modified: usize,
    pub start_line: usize,
    pub end_line: usize,
    pub description: String,
}

/// Detailed diagnostic feedback on editing failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditDiagnostic {
    /// Target was found multiple times; failing closed to prevent guessing.
    AmbiguousMatch {
        match_count: usize,
        line_numbers: Vec<usize>,
        message: String,
    },
    /// Target content could not be located in the file.
    NotFound {
        requested: String,
        total_lines: usize,
        nearest_start_line: Option<usize>,
        nearest_end_line: Option<usize>,
        nearby_context: String,
        suggestion: String,
    },
    /// Line range was outside of valid file bounds.
    OutOfBounds {
        start_line: usize,
        end_line: usize,
        total_lines: usize,
        message: String,
    },
    /// Content at specified line range did not match expected content.
    ContentMismatch {
        start_line: usize,
        end_line: usize,
        expected: String,
        actual: String,
    },
    /// Unified diff patch hunk could not be applied.
    InvalidPatch {
        hunk_index: usize,
        message: String,
        context: String,
    },
    /// Edit would cause unsafe catastrophic shrinkage of existing file definitions.
    UnsafeShrinkage {
        original_lines: usize,
        new_lines: usize,
        reduction_percent: usize,
        message: String,
    },
    /// Invalid parameters (e.g. empty inputs where required).
    InvalidInput(String),
}

impl std::fmt::Display for EditDiagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AmbiguousMatch {
                match_count,
                line_numbers,
                message,
            } => {
                write!(
                    f,
                    "Ambiguous match ({} occurrences at lines {:?}): {}. Narrow the target using line numbers (start_line, end_line) or provide more surrounding context.",
                    match_count, line_numbers, message
                )
            }
            Self::NotFound {
                requested: _,
                total_lines,
                nearest_start_line,
                nearest_end_line,
                nearby_context,
                suggestion,
            } => {
                write!(
                    f,
                    "Target content not found in file ({} total lines).",
                    total_lines
                )?;
                if let (Some(s), Some(e)) = (nearest_start_line, nearest_end_line) {
                    write!(
                        f,
                        "\nNearest candidate match found at lines {}-{}:\n{}",
                        s, e, nearby_context
                    )?;
                } else if !nearby_context.is_empty() {
                    write!(f, "\nFile context:\n{}", nearby_context)?;
                }
                write!(f, "\nSuggested action: {}", suggestion)
            }
            Self::OutOfBounds {
                start_line,
                end_line,
                total_lines,
                message,
            } => {
                write!(
                    f,
                    "Line range {}-{} out of bounds for file with {} lines: {}",
                    start_line, end_line, total_lines, message
                )
            }
            Self::ContentMismatch {
                start_line,
                end_line,
                expected,
                actual,
            } => {
                write!(
                    f,
                    "Content mismatch at lines {}-{}.\nExpected:\n{}\nActual:\n{}",
                    start_line, end_line, expected, actual
                )
            }
            Self::InvalidPatch {
                hunk_index,
                message,
                context,
            } => {
                write!(
                    f,
                    "Patch rejected at hunk {}: {}.\nHunk context:\n{}",
                    hunk_index, message, context
                )
            }
            Self::UnsafeShrinkage {
                original_lines,
                new_lines,
                reduction_percent,
                message,
            } => {
                write!(
                    f,
                    "Unsafe shrinkage rejected: file shrank from {} lines to {} lines ({}% reduction): {}",
                    original_lines, new_lines, reduction_percent, message
                )
            }
            Self::InvalidInput(msg) => write!(f, "Invalid edit input: {}", msg),
        }
    }
}

/// Robust file editor engine.
pub struct RobustFileEditor;

impl RobustFileEditor {
    /// Detect newline convention of the file (`\r\n` vs `\n`).
    pub fn detect_line_ending(content: &str) -> &'static str {
        if content.contains("\r\n") {
            "\r\n"
        } else {
            "\n"
        }
    }

    /// Apply an edit operation to the original file content.
    pub fn apply(original: &str, op: &FileEditOp) -> Result<EditSuccess, EditDiagnostic> {
        match op {
            FileEditOp::Substring {
                old_content,
                new_content,
            } => Self::apply_substring(original, old_content, new_content),
            FileEditOp::LineRange {
                start_line,
                end_line,
                new_content,
                expected_old,
            } => {
                Self::apply_line_range(original, *start_line, *end_line, new_content, *expected_old)
            }
            FileEditOp::Insert {
                line_number,
                content,
                after,
            } => Self::apply_insertion(original, *line_number, content, *after),
            FileEditOp::Delete {
                start_line,
                end_line,
                expected_old,
            } => Self::apply_deletion(original, *start_line, *end_line, *expected_old),
            FileEditOp::Patch { patch } => Self::apply_unified_diff(original, patch),
        }
    }

    /// Normalize common JSON-escaped quotes and newlines if they do not match raw content.
    pub fn normalize_escapes(s: &str) -> String {
        s.replace(r#"\""#, "\"")
            .replace(r#"\'"#, "'")
            .replace(r#"\n"#, "\n")
            .replace(r#"\t"#, "\t")
    }

    /// Exact or indent-tolerant substring replacement with duplicate-match rejection.
    fn apply_substring(
        original: &str,
        old_content: &str,
        new_content: &str,
    ) -> Result<EditSuccess, EditDiagnostic> {
        if old_content.is_empty() {
            return Err(EditDiagnostic::InvalidInput(
                "old_content cannot be empty for substring replacement".to_string(),
            ));
        }

        let le = Self::detect_line_ending(original);
        let has_trailing_newline = original.ends_with('\n');

        let unescaped = Self::normalize_escapes(old_content);
        let target_old = if !original.contains(old_content) && original.contains(&unescaped) {
            &unescaped
        } else {
            old_content
        };

        // 1. Direct exact byte match
        let exact_matches: Vec<_> = original.match_indices(target_old).collect();
        if exact_matches.len() > 1 {
            let line_numbers: Vec<usize> = exact_matches
                .iter()
                .map(|(offset, _)| original[..*offset].lines().count() + 1)
                .collect();
            return Err(EditDiagnostic::AmbiguousMatch {
                match_count: exact_matches.len(),
                line_numbers,
                message: format!(
                    "Exact substring matched {} times ({} occurrences). Cannot safely guess target.",
                    exact_matches.len(),
                    exact_matches.len()
                ),
            });
        }
        if exact_matches.len() == 1 {
            let offset = exact_matches[0].0;
            let start_line = original[..offset].lines().count() + 1;
            let old_line_count = target_old.lines().count().max(1);
            let end_line = start_line + old_line_count - 1;

            let mut result = original.replacen(target_old, new_content, 1);
            if has_trailing_newline && !result.ends_with('\n') {
                result.push_str(le);
            }

            Self::check_shrinkage(original, &result)?;

            let new_line_count = new_content.lines().count().max(1);
            return Ok(EditSuccess {
                new_content: result,
                lines_modified: new_line_count,
                start_line,
                end_line,
                description: format!(
                    "Exact substring replaced at lines {}-{}",
                    start_line, end_line
                ),
            });
        }

        // 2. Line ending normalized match (LF vs CRLF)
        let orig_lf = original.replace("\r\n", "\n");
        let old_lf = old_content.replace("\r\n", "\n");
        let new_lf = new_content.replace("\r\n", "\n");

        let lf_matches: Vec<_> = orig_lf.match_indices(&old_lf).collect();
        if lf_matches.len() > 1 {
            let line_numbers: Vec<usize> = lf_matches
                .iter()
                .map(|(offset, _)| orig_lf[..*offset].lines().count() + 1)
                .collect();
            return Err(EditDiagnostic::AmbiguousMatch {
                match_count: lf_matches.len(),
                line_numbers,
                message: format!(
                    "Newline-normalized substring matched {} times.",
                    lf_matches.len()
                ),
            });
        }
        if lf_matches.len() == 1 {
            let offset = lf_matches[0].0;
            let start_line = orig_lf[..offset].lines().count() + 1;
            let old_line_count = old_lf.lines().count().max(1);
            let end_line = start_line + old_line_count - 1;

            let replaced_lf = orig_lf.replacen(&old_lf, &new_lf, 1);
            let mut result = if le == "\r\n" {
                replaced_lf.replace('\n', "\r\n")
            } else {
                replaced_lf
            };
            if has_trailing_newline && !result.ends_with('\n') {
                result.push_str(le);
            }

            Self::check_shrinkage(original, &result)?;

            let new_line_count = new_lf.lines().count().max(1);
            return Ok(EditSuccess {
                new_content: result,
                lines_modified: new_line_count,
                start_line,
                end_line,
                description: format!(
                    "Normalized substring replaced at lines {}-{}",
                    start_line, end_line
                ),
            });
        }

        // 3. Line-based trimmed / indentation-tolerant matching
        let orig_lines: Vec<&str> = original.lines().collect();
        let old_lines: Vec<&str> = old_content.lines().collect();
        let new_lines: Vec<&str> = new_content.lines().collect();

        if !old_lines.is_empty() {
            let trimmed_matches = Self::find_line_trimmed_matches(&orig_lines, &old_lines);
            if trimmed_matches.len() > 1 {
                let line_numbers: Vec<usize> = trimmed_matches.iter().map(|idx| idx + 1).collect();
                return Err(EditDiagnostic::AmbiguousMatch {
                    match_count: trimmed_matches.len(),
                    line_numbers,
                    message: "Indentation-tolerant match found multiple occurrences.".to_string(),
                });
            }
            if trimmed_matches.len() == 1 {
                let match_idx = trimmed_matches[0];
                let start_line = match_idx + 1;
                let end_line = match_idx + old_lines.len();

                // Compute indentation offset of the target line
                let target_indent = Self::leading_indent(orig_lines[match_idx]);
                let old_indent = Self::leading_indent(old_lines[0]);

                let adjusted_new_lines: Vec<String> = new_lines
                    .iter()
                    .map(|nl| {
                        if target_indent != old_indent && !nl.trim().is_empty() {
                            let inner_indent = Self::leading_indent(nl);
                            if let Some(remainder) = inner_indent.strip_prefix(old_indent) {
                                format!("{}{}{}", target_indent, remainder, nl.trim_start())
                            } else {
                                format!("{}{}", target_indent, nl.trim_start())
                            }
                        } else {
                            (*nl).to_string()
                        }
                    })
                    .collect();

                let mut out_lines: Vec<String> = Vec::new();
                for line in &orig_lines[..match_idx] {
                    out_lines.push((*line).to_string());
                }
                for line in &adjusted_new_lines {
                    out_lines.push(line.clone());
                }
                for line in &orig_lines[match_idx + old_lines.len()..] {
                    out_lines.push((*line).to_string());
                }

                let mut result = out_lines.join(le);
                if has_trailing_newline {
                    result.push_str(le);
                }

                Self::check_shrinkage(original, &result)?;

                return Ok(EditSuccess {
                    new_content: result,
                    lines_modified: adjusted_new_lines.len(),
                    start_line,
                    end_line,
                    description: format!(
                        "Indentation-tolerant replacement applied at lines {}-{}",
                        start_line, end_line
                    ),
                });
            }
        }

        // 4. Mismatch diagnostic generation
        let nearest = Self::find_nearest_slice(&orig_lines, &old_lines);
        let (n_start, n_end, nearby_str) = if let Some((s_idx, e_idx)) = nearest {
            let s_line = s_idx + 1;
            let e_line = e_idx + 1;
            let context_start = s_idx.saturating_sub(2);
            let context_end = (e_idx + 3).min(orig_lines.len());
            let snippet = orig_lines[context_start..context_end]
                .iter()
                .enumerate()
                .map(|(i, l)| format!("{:4} | {}", context_start + i + 1, l))
                .collect::<Vec<_>>()
                .join("\n");
            (Some(s_line), Some(e_line), snippet)
        } else {
            let total = orig_lines.len();
            let preview_count = total.min(10);
            let snippet = orig_lines[..preview_count]
                .iter()
                .enumerate()
                .map(|(i, l)| format!("{:4} | {}", i + 1, l))
                .collect::<Vec<_>>()
                .join("\n");
            (None, None, snippet)
        };

        Err(EditDiagnostic::NotFound {
            requested: old_content.to_string(),
            total_lines: orig_lines.len(),
            nearest_start_line: n_start,
            nearest_end_line: n_end,
            nearby_context: nearby_str,
            suggestion: if let (Some(s), Some(e)) = (n_start, n_end) {
                format!(
                    "Inspect lines {}-{} using read_file, or specify start_line: {}, end_line: {} with edit_file.",
                    s, e, s, e
                )
            } else {
                "Call read_file to inspect current contents before editing.".to_string()
            },
        })
    }

    /// Replace a 1-indexed line range [start_line..=end_line] with new_content.
    pub fn apply_line_range(
        original: &str,
        start_line: usize,
        end_line: usize,
        new_content: &str,
        expected_old: Option<&str>,
    ) -> Result<EditSuccess, EditDiagnostic> {
        let orig_lines: Vec<&str> = original.lines().collect();
        let total_lines = orig_lines.len();
        let le = Self::detect_line_ending(original);
        let has_trailing_newline = original.ends_with('\n');

        if start_line == 0 || end_line == 0 {
            return Err(EditDiagnostic::InvalidInput(
                "Line numbers must be 1-indexed (start_line and end_line >= 1)".to_string(),
            ));
        }

        if start_line > end_line {
            return Err(EditDiagnostic::OutOfBounds {
                start_line,
                end_line,
                total_lines,
                message: format!(
                    "start_line ({}) cannot be greater than end_line ({})",
                    start_line, end_line
                ),
            });
        }

        // Allow appending if start_line == total_lines + 1
        if start_line > total_lines + 1 {
            return Err(EditDiagnostic::OutOfBounds {
                start_line,
                end_line,
                total_lines,
                message: format!(
                    "start_line ({}) exceeds file length ({} lines)",
                    start_line, total_lines
                ),
            });
        }

        let clamped_end = end_line.min(total_lines);

        // If expected_old is provided, verify matching lines
        if let Some(expected) = expected_old {
            let exp_trim = expected.trim();
            if !exp_trim.is_empty() && start_line <= total_lines {
                let actual_slice = orig_lines[start_line - 1..clamped_end].join("\n");
                let actual_trim = actual_slice.trim();
                let unescaped_exp = Self::normalize_escapes(exp_trim);
                if actual_trim != exp_trim
                    && actual_trim != unescaped_exp
                    && actual_trim.replace("\r\n", "\n") != exp_trim.replace("\r\n", "\n")
                {
                    // Check if expected content (or unescaped version) is a substring within the targeted line range
                    let matched_sub = if actual_slice.contains(exp_trim) {
                        Some(exp_trim)
                    } else if actual_slice.contains(&unescaped_exp) {
                        Some(unescaped_exp.as_str())
                    } else {
                        None
                    };

                    if let Some(sub) = matched_sub {
                        let replaced_slice = actual_slice.replacen(sub, new_content, 1);
                        let mut out: Vec<String> = orig_lines[..start_line - 1]
                            .iter()
                            .map(|l| (*l).to_string())
                            .collect();
                        for rl in replaced_slice.lines() {
                            out.push(rl.to_string());
                        }
                        for l in &orig_lines[clamped_end..] {
                            out.push((*l).to_string());
                        }
                        let mut result = out.join(le);
                        if has_trailing_newline && !result.ends_with('\n') {
                            result.push_str(le);
                        }
                        return Ok(EditSuccess {
                            new_content: result,
                            lines_modified: new_content.lines().count().max(1),
                            start_line,
                            end_line: clamped_end,
                            description: format!(
                                "Line-range substring replacement at lines {}-{}",
                                start_line, clamped_end
                            ),
                        });
                    }

                    // Fall back to robust substring replacement if expected content exists elsewhere in the file
                    if let Ok(sub_res) = Self::apply_substring(original, expected, new_content) {
                        return Ok(sub_res);
                    }
                    if let Ok(sub_res) =
                        Self::apply_substring(original, &unescaped_exp, new_content)
                    {
                        return Ok(sub_res);
                    }
                    return Err(EditDiagnostic::ContentMismatch {
                        start_line,
                        end_line: clamped_end,
                        expected: expected.to_string(),
                        actual: actual_slice,
                    });
                }
            }
        }

        let new_lines: Vec<&str> = if new_content.is_empty() {
            Vec::new()
        } else {
            new_content.lines().collect()
        };

        let mut out_lines: Vec<String> = Vec::new();
        // Lines before start_line
        let prefix_count = (start_line - 1).min(total_lines);
        for l in &orig_lines[..prefix_count] {
            out_lines.push((*l).to_string());
        }
        // Replacement lines
        for l in &new_lines {
            out_lines.push((*l).to_string());
        }
        // Lines after clamped_end
        if clamped_end < total_lines {
            for l in &orig_lines[clamped_end..] {
                out_lines.push((*l).to_string());
            }
        }

        let mut result = out_lines.join(le);
        if has_trailing_newline || (!out_lines.is_empty() && original.is_empty()) {
            result.push_str(le);
        }

        Self::check_shrinkage(original, &result)?;

        Ok(EditSuccess {
            new_content: result,
            lines_modified: new_lines.len(),
            start_line,
            end_line: clamped_end,
            description: format!("Replaced lines {}-{}", start_line, clamped_end),
        })
    }

    /// Safe insertion before or after a line number.
    pub fn apply_insertion(
        original: &str,
        line_number: usize,
        content: &str,
        after: bool,
    ) -> Result<EditSuccess, EditDiagnostic> {
        let orig_lines: Vec<&str> = original.lines().collect();
        let total_lines = orig_lines.len();
        let le = Self::detect_line_ending(original);
        let has_trailing_newline = original.ends_with('\n');

        let insert_idx = if after {
            line_number.min(total_lines)
        } else {
            line_number.saturating_sub(1).min(total_lines)
        };

        let new_lines: Vec<&str> = if content.is_empty() {
            Vec::new()
        } else {
            content.lines().collect()
        };

        let mut out_lines = Vec::with_capacity(total_lines + new_lines.len());
        for l in &orig_lines[..insert_idx] {
            out_lines.push((*l).to_string());
        }
        for l in &new_lines {
            out_lines.push((*l).to_string());
        }
        if insert_idx < total_lines {
            for l in &orig_lines[insert_idx..] {
                out_lines.push((*l).to_string());
            }
        }

        let mut result = out_lines.join(le);
        if has_trailing_newline || (!out_lines.is_empty() && original.is_empty()) {
            result.push_str(le);
        }

        Self::check_shrinkage(original, &result)?;

        let start_line = insert_idx + 1;
        let end_line = start_line + new_lines.len().saturating_sub(1);
        Ok(EditSuccess {
            new_content: result,
            lines_modified: new_lines.len(),
            start_line,
            end_line,
            description: format!("Inserted {} lines at line {}", new_lines.len(), start_line),
        })
    }

    /// Safe deletion of a line range.
    pub fn apply_deletion(
        original: &str,
        start_line: usize,
        end_line: usize,
        expected_old: Option<&str>,
    ) -> Result<EditSuccess, EditDiagnostic> {
        Self::apply_line_range(original, start_line, end_line, "", expected_old)
    }

    /// Unified diff patch application with context tolerance.
    pub fn apply_unified_diff(original: &str, patch: &str) -> Result<EditSuccess, EditDiagnostic> {
        let orig_lines: Vec<&str> = original.lines().collect();
        let patch_lines: Vec<&str> = patch.lines().collect();
        let le = Self::detect_line_ending(original);
        let has_trailing_newline = original.ends_with('\n');

        let mut result_lines: Vec<String> = Vec::new();
        let mut orig_idx = 0;
        let mut in_hunk = false;
        let mut hunk_count = 0;

        let mut hunk_old: Vec<String> = Vec::new();
        let mut hunk_new: Vec<String> = Vec::new();

        let flush = |orig_idx: &mut usize,
                     hunk_old: &mut Vec<String>,
                     hunk_new: &mut Vec<String>,
                     result_lines: &mut Vec<String>,
                     hunk_idx: usize|
         -> Result<(), EditDiagnostic> {
            if hunk_old.is_empty() && hunk_new.is_empty() {
                return Ok(());
            }

            let old_count = hunk_old.len();
            let mut match_idx = None;

            // Search for matching lines from current orig_idx
            for i in *orig_idx..=orig_lines.len().saturating_sub(old_count) {
                let slice = &orig_lines[i..i + old_count];
                let exact = slice.iter().zip(hunk_old.iter()).all(|(a, b)| *a == b);
                if exact {
                    match_idx = Some(i);
                    break;
                }
                // Indentation / whitespace tolerant match
                let trimmed = slice
                    .iter()
                    .zip(hunk_old.iter())
                    .all(|(a, b)| a.trim() == b.trim());
                if trimmed {
                    match_idx = Some(i);
                    break;
                }
            }

            let found = match_idx.ok_or_else(|| EditDiagnostic::InvalidPatch {
                hunk_index: hunk_idx,
                message: "Hunk context lines could not be located in target file".to_string(),
                context: hunk_old.join("\n"),
            })?;

            // Append unchanged lines preceding hunk
            for l in &orig_lines[*orig_idx..found] {
                result_lines.push((*l).to_string());
            }

            // Append new hunk lines
            for l in hunk_new.drain(..) {
                result_lines.push(l);
            }

            *orig_idx = found + old_count;
            hunk_old.clear();
            Ok(())
        };

        for line in patch_lines {
            if line.starts_with("---") || line.starts_with("+++") {
                continue;
            }
            if line.starts_with("@@") {
                if in_hunk {
                    flush(
                        &mut orig_idx,
                        &mut hunk_old,
                        &mut hunk_new,
                        &mut result_lines,
                        hunk_count,
                    )?;
                }
                in_hunk = true;
                hunk_count += 1;
                continue;
            }

            if in_hunk {
                if let Some(stripped) = line.strip_prefix('+') {
                    hunk_new.push(stripped.to_string());
                } else if let Some(stripped) = line.strip_prefix('-') {
                    hunk_old.push(stripped.to_string());
                } else if let Some(stripped) = line.strip_prefix(' ') {
                    hunk_old.push(stripped.to_string());
                    hunk_new.push(stripped.to_string());
                } else if line.is_empty() {
                    hunk_old.push(String::new());
                    hunk_new.push(String::new());
                }
            }
        }

        if in_hunk {
            flush(
                &mut orig_idx,
                &mut hunk_old,
                &mut hunk_new,
                &mut result_lines,
                hunk_count,
            )?;
        }

        if hunk_count == 0 {
            return Err(EditDiagnostic::InvalidInput(
                "Unified diff patch contained no valid hunks (missing '@@' lines)".to_string(),
            ));
        }

        // Append remainder of original file
        for l in &orig_lines[orig_idx..] {
            result_lines.push((*l).to_string());
        }

        let mut result = result_lines.join(le);
        if has_trailing_newline {
            result.push_str(le);
        }

        Self::check_shrinkage(original, &result)?;

        Ok(EditSuccess {
            new_content: result,
            lines_modified: result_lines.len(),
            start_line: 1,
            end_line: result_lines.len(),
            description: format!("Applied unified diff with {} hunk(s)", hunk_count),
        })
    }

    /// Check for catastrophic file shrinkage (>80% reduction on files >= 30 lines).
    fn check_shrinkage(original: &str, new_content: &str) -> Result<(), EditDiagnostic> {
        let orig_lines = original.lines().count();
        let new_lines = new_content.lines().count();

        if orig_lines >= 30 && new_lines < 10 && orig_lines > new_lines {
            let reduction = ((orig_lines - new_lines) * 100) / orig_lines;
            if reduction >= 80 {
                return Err(EditDiagnostic::UnsafeShrinkage {
                    original_lines: orig_lines,
                    new_lines,
                    reduction_percent: reduction,
                    message: "Catastrophic file shrinkage rejected. Local edits must not erase entire structs/modules.".to_string(),
                });
            }
        }
        Ok(())
    }

    fn leading_indent(line: &str) -> &str {
        let end = line
            .find(|c: char| !c.is_whitespace())
            .unwrap_or(line.len());
        &line[..end]
    }

    fn find_line_trimmed_matches(orig_lines: &[&str], target_lines: &[&str]) -> Vec<usize> {
        if target_lines.is_empty() || orig_lines.len() < target_lines.len() {
            return Vec::new();
        }
        let t_len = target_lines.len();
        let mut matches = Vec::new();

        for i in 0..=orig_lines.len() - t_len {
            let slice = &orig_lines[i..i + t_len];
            let all_match = slice
                .iter()
                .zip(target_lines.iter())
                .all(|(a, b)| a.trim() == b.trim());
            if all_match {
                matches.push(i);
            }
        }
        matches
    }

    fn find_nearest_slice(orig_lines: &[&str], target_lines: &[&str]) -> Option<(usize, usize)> {
        if orig_lines.is_empty() || target_lines.is_empty() {
            return None;
        }

        let first_target_trimmed = target_lines[0].trim();
        if first_target_trimmed.is_empty() {
            return None;
        }

        let mut best_score = 0;
        let mut best_range = None;

        for (i, line) in orig_lines.iter().enumerate() {
            let l_trim = line.trim();
            if l_trim == first_target_trimmed {
                let end = (i + target_lines.len() - 1).min(orig_lines.len() - 1);
                return Some((i, end));
            }

            // Partial similarity score
            let score = Self::line_similarity(l_trim, first_target_trimmed);
            if score > best_score && score >= 50 {
                best_score = score;
                let end = (i + target_lines.len() - 1).min(orig_lines.len() - 1);
                best_range = Some((i, end));
            }
        }

        best_range
    }

    fn line_similarity(a: &str, b: &str) -> usize {
        if a.is_empty() || b.is_empty() {
            return 0;
        }
        let common = a.chars().filter(|c| b.contains(*c)).count();
        (common * 100) / a.len().max(b.len())
    }
}
