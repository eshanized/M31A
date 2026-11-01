//! Surgical text edit tool for precise in-place string replacements.
//!
//! Provides the `edit` tool: replaces exact occurrences of `old_string` with `new_string`.
//! Enforces strict uniqueness verification to prevent ambiguous accidental mutations.

use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditArgs {
    /// Relative or absolute path to the target text file.
    pub path: String,
    /// Exact text sequence in the file to replace.
    pub old_string: String,
    /// New replacement text sequence.
    pub new_string: String,
    /// If true, replace every occurrence; otherwise requires exactly one unique match.
    #[serde(default)]
    pub replace_all: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditResult {
    pub path: String,
    pub occurrences_replaced: usize,
    pub message: String,
}

/// Executes a surgical in-place edit on the file at `path`.
pub fn execute_edit(cwd: &Path, args: &EditArgs) -> Result<EditResult> {
    if args.old_string == args.new_string {
        return Err(anyhow!(
            "Invalid edit operation: 'old_string' and 'new_string' are identical."
        ));
    }

    if args.old_string.is_empty() {
        return Err(anyhow!(
            "Invalid edit operation: 'old_string' cannot be empty."
        ));
    }

    let target_path = if Path::new(&args.path).is_absolute() {
        PathBuf::from(&args.path)
    } else {
        cwd.join(&args.path)
    };

    if !target_path.exists() {
        return Err(anyhow!("Target file does not exist: {}", args.path));
    }

    if target_path.is_dir() {
        return Err(anyhow!(
            "Target path is a directory, not a file: {}",
            args.path
        ));
    }

    let content = fs::read_to_string(&target_path)
        .with_context(|| format!("Failed to read target file '{}'", target_path.display()))?;

    let is_crlf = content.contains("\r\n");
    let content_lf = content.replace("\r\n", "\n");
    let old_lf = args.old_string.replace("\r\n", "\n");
    let new_lf = args.new_string.replace("\r\n", "\n");

    let occurrences = content_lf.matches(&old_lf).count();
    let replace_all = args.replace_all.unwrap_or(false);

    let (new_content_lf, replaced_count, note) = if occurrences > 0 {
        if occurrences > 1 && !replace_all {
            return Err(anyhow!(
                "Found {} occurrences of 'old_string' in '{}'. By default, 'old_string' must match exactly once to prevent accidental multi-site edits. Please provide more surrounding context lines to uniquely identify the block, or explicitly set 'replace_all: true' if replacing all instances is intended.",
                occurrences,
                args.path
            ));
        }
        if replace_all {
            let replaced = content_lf.replace(&old_lf, &new_lf);
            (replaced, occurrences, String::new())
        } else {
            let replaced = content_lf.replacen(&old_lf, &new_lf, 1);
            (replaced, 1, String::new())
        }
    } else {
        // Fallback: Line-by-line whitespace and typographic normalization
        let content_lines: Vec<&str> = content_lf.lines().collect();
        let old_lines: Vec<&str> = old_lf.lines().collect();

        if !old_lines.is_empty() && old_lines.len() <= content_lines.len() {
            let mut matches = Vec::new();
            for idx in 0..=(content_lines.len() - old_lines.len()) {
                let matches_at_idx = content_lines[idx..idx + old_lines.len()]
                    .iter()
                    .zip(old_lines.iter())
                    .all(|(a, b)| normalize_line(a) == normalize_line(b));
                if matches_at_idx {
                    matches.push(idx);
                }
            }

            if matches.len() == 1 {
                let match_idx = matches[0];
                let mut updated_lines: Vec<String> = content_lines[..match_idx]
                    .iter()
                    .map(|s| s.to_string())
                    .collect();

                let orig_indent = content_lines[match_idx]
                    .chars()
                    .take_while(|c| c.is_whitespace())
                    .collect::<String>();
                let new_lines_vec: Vec<&str> = new_lf.lines().collect();
                for (nl_idx, nl) in new_lines_vec.iter().enumerate() {
                    if nl_idx == 0
                        && nl.chars().take_while(|c| c.is_whitespace()).count() == 0
                        && !orig_indent.is_empty()
                    {
                        updated_lines.push(format!("{}{}", orig_indent, nl));
                    } else {
                        updated_lines.push(nl.to_string());
                    }
                }
                updated_lines.extend(
                    content_lines[match_idx + old_lines.len()..]
                        .iter()
                        .map(|s| s.to_string()),
                );

                let mut res = updated_lines.join("\n");
                if content_lf.ends_with('\n') {
                    res.push('\n');
                }
                let note = format!(
                    " (matched via line whitespace/typographic normalization at lines {}-{})",
                    match_idx + 1,
                    match_idx + old_lines.len()
                );
                (res, 1, note)
            } else if matches.len() > 1 && !replace_all {
                return Err(anyhow!(
                    "Found {} fuzzy matches of 'old_string' in '{}' with normalized whitespace. Please provide more surrounding lines to uniquely locate the target.",
                    matches.len(),
                    args.path
                ));
            } else {
                // Intelligent diagnostics: find nearest match for first line
                let norm_first = normalize_line(old_lines[0]);
                let mut nearest_line = None;
                for (l_idx, l) in content_lines.iter().enumerate() {
                    if normalize_line(l) == norm_first {
                        nearest_line = Some(l_idx + 1);
                        break;
                    }
                }

                if let Some(line_num) = nearest_line {
                    let preview_start = line_num.saturating_sub(2).max(1);
                    let preview_end = (line_num + old_lines.len() + 1).min(content_lines.len());
                    let mut preview = String::new();
                    for p_line in preview_start..=preview_end {
                        preview.push_str(&format!(
                            "  {:4} | {}\n",
                            p_line,
                            content_lines[p_line - 1]
                        ));
                    }
                    return Err(anyhow!(
                        "Found 0 exact occurrences of 'old_string' in '{}'. Candidate match for starting line found at line {}:\n{}\nPlease check line contents and whitespace before re-trying.",
                        args.path,
                        line_num,
                        preview.trim_end()
                    ));
                }

                return Err(anyhow!(
                    "Found 0 occurrences of 'old_string' in '{}'. Please inspect the file with 'read_file' to confirm the exact content and context lines before editing.",
                    args.path
                ));
            }
        } else {
            return Err(anyhow!(
                "Found 0 occurrences of 'old_string' in '{}'. Please inspect the file with 'read_file' to confirm the exact content and context lines before editing.",
                args.path
            ));
        }
    };

    let new_content = if is_crlf {
        new_content_lf.replace("\n", "\r\n")
    } else {
        new_content_lf
    };

    // Pre-save AST syntax validation gate (prevents corrupting codebase)
    m31a_ast::SyntaxValidator::validate_file(&target_path, &new_content)
        .map_err(|e| anyhow!("Edit aborted by AST syntax gate: {e}"))?;

    fs::write(&target_path, new_content).with_context(|| {
        format!(
            "Failed to write updated content to '{}'",
            target_path.display()
        )
    })?;

    Ok(EditResult {
        path: args.path.clone(),
        occurrences_replaced: replaced_count,
        message: format!(
            "Successfully updated '{}': replaced {} occurrence(s){}.",
            args.path, replaced_count, note
        ),
    })
}

fn normalize_line(s: &str) -> String {
    s.trim()
        .chars()
        .map(|c| match c {
            '\u{2010}' | '\u{2011}' | '\u{2012}' | '\u{2013}' | '\u{2014}' | '\u{2015}'
            | '\u{2212}' => '-',
            '\u{2018}' | '\u{2019}' | '\u{201A}' | '\u{201B}' => '\'',
            '\u{201C}' | '\u{201D}' | '\u{201E}' | '\u{201F}' => '"',
            '\u{00A0}' | '\u{2002}' | '\u{2003}' | '\u{2004}' | '\u{2005}' | '\u{2006}'
            | '\u{2007}' | '\u{2008}' | '\u{2009}' | '\u{200A}' | '\u{202F}' | '\u{205F}'
            | '\u{3000}' => ' ',
            other => other,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_execute_edit_single_occurrence() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("sample.rs");
        fs::write(&file_path, "fn hello() {\n    println!(\"world\");\n}\n").unwrap();

        let args = EditArgs {
            path: "sample.rs".to_string(),
            old_string: "println!(\"world\");".to_string(),
            new_string: "println!(\"m31a\");".to_string(),
            replace_all: None,
        };

        let result = execute_edit(dir.path(), &args).unwrap();
        assert_eq!(result.occurrences_replaced, 1);

        let updated = fs::read_to_string(&file_path).unwrap();
        assert!(updated.contains("println!(\"m31a\");"));
        assert!(!updated.contains("world"));
    }

    #[test]
    fn test_execute_edit_multiple_requires_replace_all() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("repeat.txt");
        fs::write(&file_path, "foo bar foo baz foo").unwrap();

        let args = EditArgs {
            path: "repeat.txt".to_string(),
            old_string: "foo".to_string(),
            new_string: "qux".to_string(),
            replace_all: Some(false),
        };

        let err = execute_edit(dir.path(), &args).unwrap_err();
        assert!(err.to_string().contains("Found 3 occurrences"));

        // Now with replace_all = true
        let args_all = EditArgs {
            path: "repeat.txt".to_string(),
            old_string: "foo".to_string(),
            new_string: "qux".to_string(),
            replace_all: Some(true),
        };

        let result = execute_edit(dir.path(), &args_all).unwrap();
        assert_eq!(result.occurrences_replaced, 3);
        let updated = fs::read_to_string(&file_path).unwrap();
        assert_eq!(updated, "qux bar qux baz qux");
    }

    #[test]
    fn test_execute_edit_identical_strings() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("dummy.txt");
        fs::write(&file_path, "same").unwrap();

        let args = EditArgs {
            path: "dummy.txt".to_string(),
            old_string: "same".to_string(),
            new_string: "same".to_string(),
            replace_all: None,
        };

        let err = execute_edit(dir.path(), &args).unwrap_err();
        assert!(err.to_string().contains("identical"));
    }

    #[test]
    fn test_execute_edit_rejects_corrupted_syntax() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("broken.rs");
        fs::write(&file_path, "fn hello() {\n    println!(\"world\");\n}\n").unwrap();

        let args = EditArgs {
            path: "broken.rs".to_string(),
            old_string: "println!(\"world\");\n}".to_string(),
            new_string: "println!(\"world\");".to_string(), // missing closing brace!
            replace_all: None,
        };

        let result = execute_edit(dir.path(), &args);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("AST syntax gate"));
    }

    #[test]
    fn test_execute_edit_fuzzy_whitespace_and_crlf() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("crlf_sample.rs");
        // File with 4 spaces indent and CRLF
        fs::write(
            &file_path,
            "fn hello() {\r\n    let x = 42;\r\n    println!(\"val: {}\", x);\r\n}\r\n",
        )
        .unwrap();

        // Model sends 2 spaces indent and LF with curly quotes
        let args = EditArgs {
            path: "crlf_sample.rs".to_string(),
            old_string: "  let x = 42;\n  println!(“val: {}”, x);".to_string(),
            new_string: "    let x = 100;\n    println!(\"val: {}\", x);".to_string(),
            replace_all: None,
        };

        let result = execute_edit(dir.path(), &args).expect("Failed fuzzy edit");
        assert_eq!(result.occurrences_replaced, 1);
        assert!(result.message.contains("matched via line whitespace"));

        let updated = fs::read_to_string(&file_path).unwrap();
        assert!(updated.contains("100"));
        assert!(updated.contains("\r\n")); // CRLF preserved
    }
}
