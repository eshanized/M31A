//! Developer-oriented @mention parsing, path resolution, and context injection (PRD §01, CLI-01).
//!
//! Supports:
//! - `@file`
//! - `@directory`
//! - `@path:line` (e.g. `@src/parser.rs:120`)
//! - `@path:start-end` (e.g. `@src/parser.rs:10-50`)
//!
//! Enforces:
//! - Workspace path confinement (rejection of `..`, `/etc/passwd`, symlink escapes)
//! - Protection of internal `.m31a` storage paths
//! - Bounded file/directory reads for context enrichment
//! - Typed `MentionReference` and `ParsedUserMessage` structures

use regex::Regex;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::sync::LazyLock;

/// Maximum bytes read from a single referenced file to prevent context budget blowup.
pub const MAX_MENTION_FILE_BYTES: usize = 32 * 1024; // 32 KB

/// Maximum entries listed for a referenced directory.
pub const MAX_MENTION_DIR_ENTRIES: usize = 50;

static MENTION_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    // Matches @path or @path:line or @path:start-end
    // Allows alphanumerics, underscores, hyphens, dots, and slashes in path
    Regex::new(r"@([A-Za-z0-9_./\\-]+)(?::([0-9]+)(?:-([0-9]+))?)?").expect("valid regex")
});

/// Line range within a referenced file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct LineRange {
    pub start: usize,
    pub end: usize,
}

impl LineRange {
    pub fn single(line: usize) -> Self {
        Self {
            start: line,
            end: line,
        }
    }

    pub fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }
}

/// Nature of the referenced entity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MentionKind {
    File,
    Directory,
    FileSnippet { line_range: LineRange },
    Unknown,
}

/// Resolution status of a mention against the local workspace filesystem.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResolutionStatus {
    Resolved,
    NotFound,
    AccessDenied(String),
    Malformed(String),
}

/// Strongly typed record of a parsed and resolved developer @mention reference.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MentionReference {
    pub original_text: String,
    pub raw_path: String,
    pub normalized_path: PathBuf,
    pub workspace_relative_path: PathBuf,
    pub kind: MentionKind,
    pub line_range: Option<LineRange>,
    pub status: ResolutionStatus,
}

/// Segment of user text (either verbatim text or a resolved mention).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MessageSegment {
    Text(String),
    Mention(MentionReference),
}

/// Fully parsed user message with extracted mentions and decomposed segments.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParsedUserMessage {
    pub raw_text: String,
    pub segments: Vec<MessageSegment>,
    pub mentions: Vec<MentionReference>,
    /// Request correlation id stamped by the submitting surface (TUI/CLI).
    ///
    /// `None` for parser-level construction; the interactive TUI stamps a
    /// fresh id per submission so the submission is traceable through the
    /// bridge, coordinator/agent invocation, emitted events, and the
    /// conversation item. Never a durable id; never persisted as authority.
    #[serde(default)]
    pub request_id: Option<String>,
}

impl ParsedUserMessage {
    /// Returns true if the message explicitly references any file or directory.
    pub fn has_mentions(&self) -> bool {
        !self.mentions.is_empty()
    }

    /// Returns all successfully resolved mentions.
    pub fn resolved_mentions(&self) -> Vec<&MentionReference> {
        self.mentions
            .iter()
            .filter(|m| m.status == ResolutionStatus::Resolved)
            .collect()
    }

    /// Produce a clean version of the text without @ symbols if requested.
    pub fn normalized_prompt(&self) -> String {
        let mut out = String::new();
        for seg in &self.segments {
            match seg {
                MessageSegment::Text(t) => out.push_str(t),
                MessageSegment::Mention(m) => {
                    out.push_str(&m.workspace_relative_path.to_string_lossy());
                }
            }
        }
        out
    }
}

/// Parser and resolver for developer @mentions against a specific workspace root.
pub struct MentionParser;

impl MentionParser {
    /// Parse and resolve all mentions in the raw user text against the workspace root.
    pub fn parse(raw_text: &str, workspace_root: &Path) -> ParsedUserMessage {
        let mut segments = Vec::new();
        let mut mentions = Vec::new();
        let mut last_end = 0;

        for cap in MENTION_REGEX.captures_iter(raw_text) {
            let full_match = cap.get(0).expect("full match");
            let start = full_match.start();
            let end = full_match.end();

            // Append preceding text segment if any
            if start > last_end {
                segments.push(MessageSegment::Text(raw_text[last_end..start].to_string()));
            }

            let original_text = full_match.as_str().to_string();
            let mut raw_path = cap.get(1).map(|m| m.as_str()).unwrap_or("").to_string();

            let line_range = match (cap.get(2), cap.get(3)) {
                (Some(s), Some(e)) => {
                    let start_num = s.as_str().parse::<usize>().unwrap_or(1).max(1);
                    let end_num = e.as_str().parse::<usize>().unwrap_or(start_num);
                    Some((start_num, end_num, true))
                }
                (Some(s), None) => {
                    let line_num = s.as_str().parse::<usize>().unwrap_or(1).max(1);
                    Some((line_num, line_num, false))
                }
                _ => {
                    while raw_path.ends_with('.')
                        || raw_path.ends_with(',')
                        || raw_path.ends_with(';')
                        || raw_path.ends_with('?')
                        || raw_path.ends_with('!')
                    {
                        if raw_path == "." || raw_path == ".." {
                            break;
                        }
                        raw_path.pop();
                    }
                    None
                }
            };

            let mention =
                Self::resolve_mention(&original_text, &raw_path, line_range, workspace_root);
            segments.push(MessageSegment::Mention(mention.clone()));
            mentions.push(mention);

            last_end = end;
        }

        if last_end < raw_text.len() {
            segments.push(MessageSegment::Text(raw_text[last_end..].to_string()));
        }

        ParsedUserMessage {
            raw_text: raw_text.to_string(),
            segments,
            mentions,
            request_id: None,
        }
    }

    /// Parse both explicit @mentions and implicit workspace-relative file paths (GAP-04).
    pub fn parse_implicit_or_explicit(raw_text: &str, workspace_root: &Path) -> ParsedUserMessage {
        let mut parsed = Self::parse(raw_text, workspace_root);
        if !parsed.mentions.is_empty() {
            return parsed;
        }

        // Implicit mention discovery: scan words for workspace-relative paths
        let mut seen = std::collections::HashSet::new();
        for word in raw_text.split_whitespace() {
            let clean = word.trim_matches(|c: char| {
                c == '@'
                    || c == '"'
                    || c == '\''
                    || c == '`'
                    || c == '('
                    || c == ')'
                    || c == '['
                    || c == ']'
                    || c == '{'
                    || c == '}'
                    || c == '<'
                    || c == '>'
                    || c == ','
                    || c == ';'
                    || c == '?'
                    || c == '!'
                    || c == ':'
            });

            if clean.is_empty() || clean.contains("..") || clean.starts_with(".m31a") {
                continue;
            }

            let is_path_like = clean.contains('/')
                || clean.contains('\\')
                || clean.ends_with(".rs")
                || clean.ends_with(".toml")
                || clean.ends_with(".json")
                || clean.ends_with(".md")
                || clean.ends_with(".txt")
                || clean.ends_with(".yaml")
                || clean.ends_with(".yml");

            if is_path_like && seen.insert(clean.to_string()) {
                let candidate = workspace_root.join(clean);
                if candidate.exists() {
                    let mention = Self::resolve_mention(clean, clean, None, workspace_root);
                    if mention.status == ResolutionStatus::Resolved {
                        parsed.mentions.push(mention);
                    }
                }
            }
        }

        parsed
    }

    /// Resolves a single mention candidate against the workspace.
    fn resolve_mention(
        original_text: &str,
        raw_path: &str,
        range_spec: Option<(usize, usize, bool)>,
        workspace_root: &Path,
    ) -> MentionReference {
        // 1. Security validation: Check for path traversal attempts (..)
        let path_obj = Path::new(raw_path);
        for comp in path_obj.components() {
            if comp == Component::ParentDir {
                return MentionReference {
                    original_text: original_text.to_string(),
                    raw_path: raw_path.to_string(),
                    normalized_path: PathBuf::from(raw_path),
                    workspace_relative_path: PathBuf::from(raw_path),
                    kind: MentionKind::Unknown,
                    line_range: None,
                    status: ResolutionStatus::AccessDenied(
                        "Path traversal ('..') is prohibited in @mentions".to_string(),
                    ),
                };
            }
        }

        // 2. Security validation: Protect internal .m31a metadata directory
        let clean_path_str = raw_path.trim_start_matches('/').trim_start_matches("./");
        if clean_path_str.starts_with(".m31a")
            || clean_path_str.contains("/.m31a/")
            || clean_path_str.ends_with("/.m31a")
        {
            return MentionReference {
                original_text: original_text.to_string(),
                raw_path: raw_path.to_string(),
                normalized_path: PathBuf::from(raw_path),
                workspace_relative_path: PathBuf::from(clean_path_str),
                kind: MentionKind::Unknown,
                line_range: None,
                status: ResolutionStatus::AccessDenied(
                    "Access to internal '.m31a' directory is prohibited".to_string(),
                ),
            };
        }

        // 3. Resolve absolute vs relative paths
        let (abs_path, rel_path) = if path_obj.is_absolute() {
            // Absolute path must reside within workspace_root
            if let Ok(rel) = path_obj.strip_prefix(workspace_root) {
                (path_obj.to_path_buf(), rel.to_path_buf())
            } else {
                return MentionReference {
                    original_text: original_text.to_string(),
                    raw_path: raw_path.to_string(),
                    normalized_path: PathBuf::from(raw_path),
                    workspace_relative_path: PathBuf::from(raw_path),
                    kind: MentionKind::Unknown,
                    line_range: None,
                    status: ResolutionStatus::AccessDenied(
                        "Absolute path escapes workspace root".to_string(),
                    ),
                };
            }
        } else {
            let rel = PathBuf::from(clean_path_str);
            let abs = workspace_root.join(&rel);
            (abs, rel)
        };

        // 4. Validate line range if provided
        let parsed_range = if let Some((start, end, is_range)) = range_spec {
            if is_range && start > end {
                return MentionReference {
                    original_text: original_text.to_string(),
                    raw_path: raw_path.to_string(),
                    normalized_path: abs_path,
                    workspace_relative_path: rel_path,
                    kind: MentionKind::Unknown,
                    line_range: None,
                    status: ResolutionStatus::Malformed(format!(
                        "Invalid line range: start line ({start}) exceeds end line ({end})"
                    )),
                };
            }
            Some(LineRange::new(start, end))
        } else {
            None
        };

        // 5. Inspect target on filesystem
        if abs_path.exists() {
            // Ensure canonical path doesn't escape workspace via symlinks
            if let (Ok(canonical_target), Ok(canonical_ws)) =
                (abs_path.canonicalize(), workspace_root.canonicalize())
                && !canonical_target.starts_with(&canonical_ws)
            {
                return MentionReference {
                    original_text: original_text.to_string(),
                    raw_path: raw_path.to_string(),
                    normalized_path: abs_path,
                    workspace_relative_path: rel_path,
                    kind: MentionKind::Unknown,
                    line_range: None,
                    status: ResolutionStatus::AccessDenied(
                        "Symlink resolves outside authorized workspace".to_string(),
                    ),
                };
            }

            if abs_path.is_dir() {
                MentionReference {
                    original_text: original_text.to_string(),
                    raw_path: raw_path.to_string(),
                    normalized_path: abs_path,
                    workspace_relative_path: rel_path,
                    kind: MentionKind::Directory,
                    line_range: None,
                    status: ResolutionStatus::Resolved,
                }
            } else {
                let kind = if let Some(range) = parsed_range {
                    MentionKind::FileSnippet { line_range: range }
                } else {
                    MentionKind::File
                };
                MentionReference {
                    original_text: original_text.to_string(),
                    raw_path: raw_path.to_string(),
                    normalized_path: abs_path,
                    workspace_relative_path: rel_path,
                    kind,
                    line_range: parsed_range,
                    status: ResolutionStatus::Resolved,
                }
            }
        } else {
            // Missing target
            let kind = if raw_path.ends_with('/') || raw_path.ends_with('\\') {
                MentionKind::Directory
            } else {
                MentionKind::File
            };
            MentionReference {
                original_text: original_text.to_string(),
                raw_path: raw_path.to_string(),
                normalized_path: abs_path,
                workspace_relative_path: rel_path,
                kind,
                line_range: parsed_range,
                status: ResolutionStatus::NotFound,
            }
        }
    }

    /// Injects resolved developer mentions into structured XML context for LLM consumption.
    pub fn inject_mention_context(_workspace_root: &Path, mentions: &[MentionReference]) -> String {
        if mentions.is_empty() {
            return String::new();
        }

        let mut out = String::from("\n<explicit_developer_mentions>\n");

        for mention in mentions {
            let rel_path = mention.workspace_relative_path.to_string_lossy();
            match &mention.status {
                ResolutionStatus::Resolved => match &mention.kind {
                    MentionKind::File => {
                        let content = match fs::read_to_string(&mention.normalized_path) {
                            Ok(text) => {
                                if text.len() > MAX_MENTION_FILE_BYTES {
                                    format!(
                                        "{}\n... [Truncated: showing first {} bytes of {} total] ...",
                                        &text[..MAX_MENTION_FILE_BYTES],
                                        MAX_MENTION_FILE_BYTES,
                                        text.len()
                                    )
                                } else {
                                    text
                                }
                            }
                            Err(e) => format!("Error reading file: {e}"),
                        };
                        out.push_str(&format!(
                            "  <referenced_file path=\"{}\">\n{}\n  </referenced_file>\n",
                            rel_path, content
                        ));
                    }
                    MentionKind::FileSnippet { line_range } => {
                        let snippet = match fs::read_to_string(&mention.normalized_path) {
                            Ok(text) => {
                                let lines: Vec<&str> = text.lines().collect();
                                let total = lines.len();
                                let s = line_range.start.saturating_sub(1);
                                let e = line_range.end.min(total);
                                if s < total && s <= e {
                                    lines[s..e].join("\n")
                                } else {
                                    format!(
                                        "Line range {}-{} out of bounds (file has {} lines)",
                                        line_range.start, line_range.end, total
                                    )
                                }
                            }
                            Err(e) => format!("Error reading file: {e}"),
                        };
                        out.push_str(&format!(
                            "  <referenced_snippet path=\"{}\" lines=\"{}-{}\">\n{}\n  </referenced_snippet>\n",
                            rel_path, line_range.start, line_range.end, snippet
                        ));
                    }
                    MentionKind::Directory => {
                        let mut entries = Vec::new();
                        if let Ok(dir) = fs::read_dir(&mention.normalized_path) {
                            for item in dir.flatten().take(MAX_MENTION_DIR_ENTRIES) {
                                let name = item.file_name().to_string_lossy().to_string();
                                let is_dir = item.file_type().map(|t| t.is_dir()).unwrap_or(false);
                                entries.push(if is_dir { format!("{}/", name) } else { name });
                            }
                        }
                        out.push_str(&format!(
                            "  <referenced_directory path=\"{}\">\n    {}\n  </referenced_directory>\n",
                            rel_path,
                            entries.join("\n    ")
                        ));
                    }
                    MentionKind::Unknown => {}
                },
                ResolutionStatus::NotFound => {
                    out.push_str(&format!(
                        "  <unresolved_mention path=\"{}\" reason=\"Target not found on disk\" />\n",
                        rel_path
                    ));
                }
                ResolutionStatus::AccessDenied(reason) => {
                    out.push_str(&format!(
                        "  <unresolved_mention path=\"{}\" reason=\"Access Denied: {}\" />\n",
                        rel_path, reason
                    ));
                }
                ResolutionStatus::Malformed(reason) => {
                    out.push_str(&format!(
                        "  <unresolved_mention path=\"{}\" reason=\"Malformed: {}\" />\n",
                        rel_path, reason
                    ));
                }
            }
        }

        out.push_str("</explicit_developer_mentions>\n");
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_parse_simple_file_and_dir_mentions() {
        let dir = tempdir().unwrap();
        let ws = dir.path();

        fs::create_dir_all(ws.join("src")).unwrap();
        fs::write(ws.join("src/parser.rs"), "pub fn parse() {}").unwrap();
        fs::create_dir_all(ws.join("tests")).unwrap();

        let input = "Fix @src/parser.rs and inspect @tests/";
        let parsed = MentionParser::parse(input, ws);

        assert_eq!(parsed.mentions.len(), 2);
        assert_eq!(parsed.mentions[0].raw_path, "src/parser.rs");
        assert_eq!(parsed.mentions[0].kind, MentionKind::File);
        assert_eq!(parsed.mentions[0].status, ResolutionStatus::Resolved);

        assert_eq!(parsed.mentions[1].raw_path, "tests/");
        assert_eq!(parsed.mentions[1].kind, MentionKind::Directory);
        assert_eq!(parsed.mentions[1].status, ResolutionStatus::Resolved);
    }

    #[test]
    fn test_parse_line_and_range_mentions() {
        let dir = tempdir().unwrap();
        let ws = dir.path();

        fs::create_dir_all(ws.join("src")).unwrap();
        fs::write(
            ws.join("src/parser.rs"),
            "line1\nline2\nline3\nline4\nline5",
        )
        .unwrap();

        let input = "Check @src/parser.rs:2 and range @src/parser.rs:1-3";
        let parsed = MentionParser::parse(input, ws);

        assert_eq!(parsed.mentions.len(), 2);
        assert_eq!(
            parsed.mentions[0].kind,
            MentionKind::FileSnippet {
                line_range: LineRange::single(2)
            }
        );
        assert_eq!(
            parsed.mentions[1].kind,
            MentionKind::FileSnippet {
                line_range: LineRange::new(1, 3)
            }
        );
    }

    #[test]
    fn test_reject_path_traversal() {
        let dir = tempdir().unwrap();
        let ws = dir.path();

        let input = "Steal @../../etc/passwd and @/etc/passwd";
        let parsed = MentionParser::parse(input, ws);

        assert_eq!(parsed.mentions.len(), 2);
        assert!(matches!(
            parsed.mentions[0].status,
            ResolutionStatus::AccessDenied(_)
        ));
        assert!(matches!(
            parsed.mentions[1].status,
            ResolutionStatus::AccessDenied(_)
        ));
    }

    #[test]
    fn test_reject_m31a_internal_storage() {
        let dir = tempdir().unwrap();
        let ws = dir.path();

        let input = "Inspect @.m31a/m31a.db";
        let parsed = MentionParser::parse(input, ws);

        assert_eq!(parsed.mentions.len(), 1);
        assert!(matches!(
            parsed.mentions[0].status,
            ResolutionStatus::AccessDenied(_)
        ));
    }

    #[test]
    fn test_malformed_line_range() {
        let dir = tempdir().unwrap();
        let ws = dir.path();

        fs::write(ws.join("file.rs"), "content").unwrap();
        let input = "Check @file.rs:50-10";
        let parsed = MentionParser::parse(input, ws);

        assert_eq!(parsed.mentions.len(), 1);
        assert!(matches!(
            parsed.mentions[0].status,
            ResolutionStatus::Malformed(_)
        ));
    }

    #[test]
    fn test_missing_target() {
        let dir = tempdir().unwrap();
        let ws = dir.path();

        let input = "Read @nonexistent/file.rs";
        let parsed = MentionParser::parse(input, ws);

        assert_eq!(parsed.mentions.len(), 1);
        assert_eq!(parsed.mentions[0].status, ResolutionStatus::NotFound);
    }

    #[test]
    fn test_trailing_sentence_punctuation() {
        let dir = tempdir().unwrap();
        let ws = dir.path();

        fs::write(ws.join("parser.rs"), "pub fn parse() {}").unwrap();
        let input = "Update the implementation in @parser.rs.";
        let parsed = MentionParser::parse(input, ws);

        assert_eq!(parsed.mentions.len(), 1);
        assert_eq!(parsed.mentions[0].status, ResolutionStatus::Resolved);
        assert_eq!(parsed.mentions[0].raw_path, "parser.rs");
    }
}
