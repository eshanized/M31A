//! V4A structured multi-file patch format parser (`*** Begin Patch` … `*** End Patch`).
//!
//! This is the structured multi-file patch format used for reliable atomic edits,
//! carried within `apply_patch <<'EOF'` heredocs and direct patch tool calls. Grammar:
//!
//! ```text
//! *** Begin Patch
//! *** Add File: path
//! +new line
//! *** Update File: path
//! @@ optional anchor
//! -removed
//!  context
//! +added
//! *** Delete File: path
//! *** End Patch
//! ```
//!
//! Parsed into M31A's existing `ParsedPatch` model so the transaction engine,
//! fuzzy matcher, and syntax validator apply unchanged.

use anyhow::{Result, anyhow};
use std::path::PathBuf;

use crate::unified_diff::{FileAction, ParsedPatch, PatchHunk, PatchLine};

const BEGIN_MARKER: &str = "*** Begin Patch";
const END_MARKER: &str = "*** End Patch";

/// Parses a V4A patch into structured file actions.
/// Returns an error when the shape is invalid; callers fall back to other
/// formats on `Err`.
pub fn parse_v4a_patch(text: &str) -> Result<ParsedPatch> {
    let mut lines = text.lines().map(|l| l.trim_end());
    let begin = lines
        .by_ref()
        .find(|l| l.trim() == BEGIN_MARKER)
        .ok_or_else(|| anyhow!("No `*** Begin Patch` marker found"))?;

    let _ = begin;
    let mut actions = Vec::new();
    let mut current: Option<FileAction> = None;
    let mut seen_end = false;

    for line in lines {
        if line.trim() == END_MARKER {
            seen_end = true;
            break;
        }

        let directive = parse_directive(line);
        match directive {
            Some(V4aDirective::AddFile(path)) => {
                flush(&mut actions, current.take())?;
                current = Some(FileAction::Create {
                    path,
                    content: String::new(),
                });
            }
            Some(V4aDirective::UpdateFile(path)) => {
                flush(&mut actions, current.take())?;
                current = Some(FileAction::Modify {
                    path,
                    hunks: Vec::new(),
                });
            }
            Some(V4aDirective::DeleteFile(path)) => {
                flush(&mut actions, current.take())?;
                actions.push(FileAction::Delete { path });
            }
            Some(V4aDirective::MoveTo(target)) => match current.take() {
                Some(FileAction::Modify { path, hunks }) => {
                    current = Some(FileAction::Rename {
                        old_path: path,
                        new_path: target,
                        hunks,
                    });
                }
                Some(other) => {
                    return Err(anyhow!(
                        "`*** Move to:` can only follow `*** Update File:`, found: {:?}",
                        other
                    ));
                }
                None => {
                    return Err(anyhow!(
                        "`*** Move to:` without preceding `*** Update File:`"
                    ));
                }
            },
            Some(V4aDirective::EndOfFile) => {
                flush(&mut actions, current.take())?;
            }
            Some(V4aDirective::Other) => {
                return Err(anyhow!("Unknown patch directive: `{line}`"));
            }
            None => {
                // Content line: append to the current section's body.
                append_content_line(&mut current, line)?;
            }
        }
    }

    if !seen_end {
        return Err(anyhow!("Missing `*** End Patch` marker"));
    }
    flush(&mut actions, current.take())?;

    if actions.is_empty() {
        return Err(anyhow!("Empty patch: no file actions found"));
    }
    Ok(ParsedPatch { actions })
}

/// Finalizes an in-progress Add/Update section into a completed action.
fn flush(actions: &mut Vec<FileAction>, current: Option<FileAction>) -> Result<()> {
    match current {
        None => Ok(()),
        Some(FileAction::Create { path, content }) => {
            actions.push(FileAction::Create { path, content });
            Ok(())
        }
        Some(FileAction::Modify { path, mut hunks }) => {
            if hunks.is_empty() {
                return Err(anyhow!(
                    "`*** Update File: {}` has no hunks",
                    path.display()
                ));
            }
            // Drop hunks left empty after blank-line trimming.
            hunks.retain(|h| match h {
                PatchHunk::Unified { lines, .. } => !lines.is_empty(),
                PatchHunk::SearchReplace { .. } => true,
            });
            if hunks.is_empty() {
                return Err(anyhow!(
                    "`*** Update File: {}` has no hunks",
                    path.display()
                ));
            }
            actions.push(FileAction::Modify { path, hunks });
            Ok(())
        }
        Some(FileAction::Rename {
            old_path,
            new_path,
            mut hunks,
        }) => {
            hunks.retain(|h| match h {
                PatchHunk::Unified { lines, .. } => !lines.is_empty(),
                PatchHunk::SearchReplace { .. } => true,
            });
            actions.push(FileAction::Rename {
                old_path,
                new_path,
                hunks,
            });
            Ok(())
        }
        Some(other) => Err(anyhow!("Unexpected action state: {:?}", other)),
    }
}

/// Appends one payload line (`+`, `-`, bare context, or `@@` anchor) to the
/// in-progress Add/Update section.
fn append_content_line(current: &mut Option<FileAction>, line: &str) -> Result<()> {
    let Some(action) = current.as_mut() else {
        return Err(anyhow!(
            "Patch content outside of a `*** Add File:` / `*** Update File:` section: `{line}`"
        ));
    };

    match action {
        FileAction::Create { content, .. } => {
            let payload = line.strip_prefix('+').unwrap_or(line);
            content.push_str(payload);
            content.push('\n');
            Ok(())
        }
        FileAction::Modify { hunks, .. } | FileAction::Rename { hunks, .. } => {
            let trimmed = line.trim();
            if trimmed.starts_with("@@") {
                // Trim blank context lines off the tail of the previous hunk —
                // blank separators between hunks are formatting, not content.
                if let Some(PatchHunk::Unified { lines: prev, .. }) = hunks.last_mut() {
                    while matches!(prev.last(), Some(PatchLine::Context(c)) if c.trim().is_empty())
                    {
                        prev.pop();
                    }
                }
                // Start a new hunk; parse the optional `@@ -a,b +c,d @@ ctx` header.
                let old_start = parse_hunk_header_hint(trimmed);
                let ctx_text = hunk_context_header(trimmed);
                let mut hunk = PatchHunk::Unified {
                    old_start,
                    old_count: 0,
                    new_start: 0,
                    new_count: 0,
                    context_header: ctx_text.clone(),
                    lines: Vec::new(),
                };
                // The anchor text participates in matching as a leading context
                // line (upstream treats `@@ <ctx>` as a locate hint).
                if let Some(ctx) = ctx_text
                    && let PatchHunk::Unified { lines: hl, .. } = &mut hunk
                {
                    hl.push(PatchLine::Context(ctx));
                }
                hunks.push(hunk);
                return Ok(());
            }

            // Tolerate blank formatting lines before the first `@@` anchor.
            if trimmed.is_empty() && hunks.is_empty() {
                return Ok(());
            }

            let Some(PatchHunk::Unified {
                lines: hunk_lines, ..
            }) = hunks.last_mut()
            else {
                return Err(anyhow!(
                    "Update File content before the first `@@` hunk anchor: `{line}`"
                ));
            };

            if let Some(added) = line.strip_prefix('+') {
                hunk_lines.push(PatchLine::Add(added.to_string()));
            } else if let Some(removed) = line.strip_prefix('-') {
                hunk_lines.push(PatchLine::Delete(removed.to_string()));
            } else {
                hunk_lines.push(PatchLine::Context(line.to_string()));
            }
            Ok(())
        }
        _ => Err(anyhow!("Cannot append content to this action type")),
    }
}

/// A `*** <verb>` directive line, if the line is one.
fn parse_directive(line: &str) -> Option<V4aDirective> {
    let trimmed = line.trim_start();
    let rest = trimmed.strip_prefix("*** ")?;
    let (verb, arg) = match rest.split_once(':') {
        Some((v, a)) => (v.trim(), a.trim()),
        None => (rest.trim(), ""),
    };

    let unquote = |p: &str| PathBuf::from(p.trim_matches(['`', '"', '\'']));
    match verb {
        "Add File" => Some(V4aDirective::AddFile(unquote(arg))),
        "Update File" => Some(V4aDirective::UpdateFile(unquote(arg))),
        "Delete File" => Some(V4aDirective::DeleteFile(unquote(arg))),
        "Move to" => Some(V4aDirective::MoveTo(unquote(arg))),
        "End of File" => Some(V4aDirective::EndOfFile),
        _ => Some(V4aDirective::Other),
    }
}

enum V4aDirective {
    AddFile(PathBuf),
    UpdateFile(PathBuf),
    DeleteFile(PathBuf),
    MoveTo(PathBuf),
    EndOfFile,
    Other,
}

/// Extracts the 0-based hint from an `@@ -a,b +c,d @@ ctx` header (a = 1-based
/// old start). Bare `@@` headers hint at the file start.
fn parse_hunk_header_hint(header: &str) -> usize {
    let Some(inner) = header.strip_prefix("@@").map(str::trim_start) else {
        return 0;
    };
    let Some(inner) = inner.strip_prefix('-') else {
        return 0;
    };
    let digits: String = inner.chars().take_while(|c| c.is_ascii_digit()).collect();
    digits
        .parse::<usize>()
        .map(|n| n.saturating_sub(1))
        .unwrap_or(0)
}

/// Trailing context text of a hunk header, if any. Supports:
///   `@@ -a,b +c,d @@ ctx`  (text after the closing marker)
///   `@@ ctx @@`            (text between two markers)
///   `@@ ctx`               (unclosed marker)
/// Numeric-only headers yield None.
fn hunk_context_header(header: &str) -> Option<String> {
    let parts: Vec<&str> = header.split("@@").collect();
    if parts.len() < 2 {
        return None;
    }
    // Text after the closing `@@`.
    if let Some(after) = parts.get(2) {
        let c = after.trim();
        if !c.is_empty() {
            return Some(c.to_string());
        }
    }
    // Text between the markers — skip numeric-only middles (` -a,b +c,d `).
    let mid = parts[1].trim();
    if !mid.is_empty() && !mid.starts_with('-') {
        return Some(mid.to_string());
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_add_update_delete() {
        let text = "\
*** Begin Patch
*** Add File: src/new.rs
+fn main() {
+}
*** Update File: src/old.rs
@@ fn old() {
-    let a = 1;
+    let a = 2;
@@
-fn stale() {}
+fn fresh() {}
*** Delete File: src/gone.rs
*** End Patch";

        let parsed = parse_v4a_patch(text).unwrap();
        assert_eq!(parsed.actions.len(), 3);

        match &parsed.actions[0] {
            FileAction::Create { path, content } => {
                assert_eq!(path, &PathBuf::from("src/new.rs"));
                assert_eq!(content, "fn main() {\n}\n");
            }
            _ => panic!("expected Create"),
        }

        match &parsed.actions[1] {
            FileAction::Modify { path, hunks } => {
                assert_eq!(path, &PathBuf::from("src/old.rs"));
                assert_eq!(hunks.len(), 2);
                match &hunks[0] {
                    PatchHunk::Unified {
                        old_start, lines, ..
                    } => {
                        assert_eq!(*old_start, 0);
                        assert_eq!(lines.len(), 3);
                        assert_eq!(lines[0], PatchLine::Context("fn old() {".to_string()));
                        assert_eq!(lines[1], PatchLine::Delete("    let a = 1;".to_string()));
                        assert_eq!(lines[2], PatchLine::Add("    let a = 2;".to_string()));
                    }
                    _ => panic!("expected Unified hunk"),
                }
            }
            _ => panic!("expected Modify"),
        }

        match &parsed.actions[2] {
            FileAction::Delete { path } => assert_eq!(path, &PathBuf::from("src/gone.rs")),
            _ => panic!("expected Delete"),
        }
    }

    #[test]
    fn test_parse_numeric_hunk_header_hint() {
        let text = "\
*** Begin Patch
*** Update File: f.rs
@@ -10,2 +10,3 @@ context words
-ok
+good
*** End Patch";
        let parsed = parse_v4a_patch(text).unwrap();
        match &parsed.actions[0] {
            FileAction::Modify { hunks, .. } => match &hunks[0] {
                PatchHunk::Unified {
                    old_start,
                    context_header,
                    ..
                } => {
                    assert_eq!(*old_start, 9); // 1-based 10 → 0-based hint 9
                    assert_eq!(context_header.as_deref(), Some("context words"));
                }
                _ => panic!("expected Unified"),
            },
            _ => panic!("expected Modify"),
        }
    }

    #[test]
    fn test_rejects_missing_end_marker() {
        let text = "*** Begin Patch\n*** Add File: a.rs\n+x";
        assert!(parse_v4a_patch(text).is_err());
    }

    #[test]
    fn test_rejects_unknown_directive() {
        let text = "*** Begin Patch\n*** Append File: a.rs\n+x\n*** End Patch";
        assert!(parse_v4a_patch(text).is_err());
    }

    #[test]
    fn test_rejects_empty_update() {
        let text = "*** Begin Patch\n*** Update File: a.rs\n*** End Patch";
        assert!(parse_v4a_patch(text).is_err());
    }

    #[test]
    fn test_rejects_content_outside_section() {
        let text = "*** Begin Patch\n+stray\n*** End Patch";
        assert!(parse_v4a_patch(text).is_err());
    }

    #[test]
    fn test_blank_separators_between_hunks_are_not_context() {
        let text = "\
*** Begin Patch
*** Update File: f.rs
@@ a
-x
+y

@@ b
-z
+w
*** End Patch";
        let parsed = parse_v4a_patch(text).unwrap();
        match &parsed.actions[0] {
            FileAction::Modify { hunks, .. } => {
                assert_eq!(hunks.len(), 2);
                let anchors = ["a", "b"];
                for (i, h) in hunks.iter().enumerate() {
                    match h {
                        PatchHunk::Unified { lines, .. } => {
                            // Blank separators must not join a hunk, but the
                            // `@@ <ctx>` anchor DOES join as a context line.
                            assert_eq!(lines.len(), 3, "unexpected hunk lines: {:?}", lines);
                            assert_eq!(lines[0], PatchLine::Context(anchors[i].to_string()));
                        }
                        _ => panic!("expected Unified"),
                    }
                }
            }
            _ => panic!("expected Modify"),
        }
    }

    #[test]
    fn test_parse_v4a_move_to_rename() {
        let text = "\
*** Begin Patch
*** Update File: src/old.rs
*** Move to: src/new.rs
@@
-old content
+new content
*** End Patch";
        let parsed = parse_v4a_patch(text).unwrap();
        assert_eq!(parsed.actions.len(), 1);
        match &parsed.actions[0] {
            FileAction::Rename {
                old_path,
                new_path,
                hunks,
            } => {
                assert_eq!(old_path, &PathBuf::from("src/old.rs"));
                assert_eq!(new_path, &PathBuf::from("src/new.rs"));
                assert_eq!(hunks.len(), 1);
                match &hunks[0] {
                    PatchHunk::Unified { lines, .. } => {
                        assert_eq!(lines.len(), 2);
                        assert_eq!(lines[0], PatchLine::Delete("old content".to_string()));
                        assert_eq!(lines[1], PatchLine::Add("new content".to_string()));
                    }
                    _ => panic!("expected Unified hunk"),
                }
            }
            _ => panic!("expected Rename"),
        }
    }
}
