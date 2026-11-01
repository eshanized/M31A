//! Parser for unified and search/replace patch blocks.
//!
//! Extracts file targets, line ranges, and before/after hunk lines from patch strings.

use std::path::PathBuf;
use thiserror::Error;

pub const BEGIN_PATCH_MARKER: &str = "*** Begin Patch";
pub const END_PATCH_MARKER: &str = "*** End Patch";
pub const ADD_FILE_MARKER: &str = "*** Add File: ";
pub const DELETE_FILE_MARKER: &str = "*** Delete File: ";
pub const UPDATE_FILE_MARKER: &str = "*** Update File: ";
pub const MOVE_TO_MARKER: &str = "*** Move to: ";
pub const EOF_MARKER: &str = "*** End of File";
pub const CHANGE_CONTEXT_MARKER: &str = "@@ ";
pub const EMPTY_CHANGE_CONTEXT_MARKER: &str = "@@";

#[derive(Debug, PartialEq, Error, Clone)]
pub enum ParseError {
    #[error("invalid patch: {0}")]
    InvalidPatchError(String),
    #[error("invalid hunk at line {line_number}: {message}")]
    InvalidHunkError { message: String, line_number: usize },
}

#[derive(Debug, PartialEq, Clone)]
pub enum Hunk {
    AddFile {
        path: PathBuf,
        contents: String,
    },
    DeleteFile {
        path: PathBuf,
    },
    UpdateFile {
        path: PathBuf,
        move_path: Option<PathBuf>,
        chunks: Vec<UpdateFileChunk>,
    },
}

impl Hunk {
    pub fn path(&self) -> &PathBuf {
        match self {
            Hunk::AddFile { path, .. } => path,
            Hunk::DeleteFile { path } => path,
            Hunk::UpdateFile {
                move_path: Some(path),
                ..
            } => path,
            Hunk::UpdateFile { path, .. } => path,
        }
    }
}

#[derive(Debug, Default, PartialEq, Clone)]
pub struct UpdateFileChunk {
    pub change_context: Option<String>,
    pub old_lines: Vec<String>,
    pub new_lines: Vec<String>,
    pub context_line_indices: Vec<(usize, usize)>,
    pub is_end_of_file: bool,
}

impl UpdateFileChunk {
    pub fn push_context_line(&mut self, line: String) {
        self.context_line_indices
            .push((self.old_lines.len(), self.new_lines.len()));
        self.old_lines.push(line.clone());
        self.new_lines.push(line);
    }
}

#[derive(Debug, Clone)]
pub struct ApplyPatchArgs {
    pub hunks: Vec<Hunk>,
    pub patch: String,
}

pub fn parse_patch(patch: &str) -> Result<ApplyPatchArgs, ParseError> {
    let mut clean = patch.trim();

    // 1. Strip markdown fences ```patch ... ``` or ```diff ... ``` or ``` ... ```
    if clean.starts_with("```")
        && let Some(first_newline) = clean.find('\n')
    {
        let inner = &clean[first_newline + 1..];
        if let Some(end_fence) = inner.rfind("```") {
            clean = inner[..end_fence].trim();
        } else {
            clean = inner.trim();
        }
    }

    // 2. Strip heredoc wrappers (e.g. apply_patch <<'EOF' ... EOF)
    if (clean.starts_with("apply_patch <<") || clean.starts_with("cat <<"))
        && let Some(first_newline) = clean.find('\n')
    {
        let inner = &clean[first_newline + 1..];
        if let Some(end_eof) = inner.rfind("\nEOF") {
            clean = inner[..end_eof].trim();
        } else if let Some(stripped) = inner.strip_suffix("EOF") {
            clean = stripped.trim();
        } else {
            clean = inner.trim();
        }
    }

    // 3. Extract boundary from *** Begin Patch up to *** End Patch
    let normalized_patch = if let Some(begin_idx) = clean.find(BEGIN_PATCH_MARKER) {
        let after_begin = &clean[begin_idx..];
        if let Some(end_idx) = after_begin.rfind(END_PATCH_MARKER) {
            after_begin[..end_idx + END_PATCH_MARKER.len()]
                .trim()
                .to_string()
        } else {
            format!("{}\n{}", after_begin.trim(), END_PATCH_MARKER)
        }
    } else {
        clean.to_string()
    };

    let mut parser = StreamingPatchParser::default();
    parser.push_delta(&normalized_patch)?;
    let hunks = parser.finish()?;
    Ok(ApplyPatchArgs {
        hunks,
        patch: normalized_patch,
    })
}

#[derive(Debug, Default, Clone)]
pub struct StreamingPatchParser {
    line_buffer: String,
    state: StreamingParserState,
    line_number: usize,
}

#[derive(Debug, Default, Clone)]
struct StreamingParserState {
    mode: StreamingParserMode,
    hunks: Vec<Hunk>,
}

#[derive(Debug, Default, Clone, Copy)]
enum StreamingParserMode {
    #[default]
    NotStarted,
    StartedPatch,
    AddFile,
    DeleteFile,
    UpdateFile {
        hunk_line_number: usize,
    },
    EndedPatch,
}

impl StreamingPatchParser {
    fn ensure_update_hunk_is_not_empty(&self, line: &str) -> Result<(), ParseError> {
        if let Some(Hunk::UpdateFile { path, chunks, .. }) = self.state.hunks.last() {
            if chunks.is_empty()
                && let StreamingParserMode::UpdateFile { hunk_line_number } = self.state.mode
            {
                return Err(ParseError::InvalidHunkError {
                    message: format!("Update file hunk for path '{}' is empty", path.display()),
                    line_number: hunk_line_number,
                });
            }
            if chunks
                .last()
                .is_some_and(|chunk| chunk.old_lines.is_empty() && chunk.new_lines.is_empty())
            {
                if line == END_PATCH_MARKER {
                    return Err(ParseError::InvalidHunkError {
                        message: "Update hunk does not contain any lines".to_string(),
                        line_number: self.line_number,
                    });
                }
                return Err(ParseError::InvalidHunkError {
                    message: format!(
                        "Unexpected line found in update hunk: '{line}'. Every line should start with ' ', '+', or '-'"
                    ),
                    line_number: self.line_number,
                });
            }
        }
        Ok(())
    }

    fn handle_hunk_headers_and_end_patch(&mut self, trimmed: &str) -> Result<bool, ParseError> {
        if trimmed == END_PATCH_MARKER {
            self.ensure_update_hunk_is_not_empty(trimmed)?;
            self.state.mode = StreamingParserMode::EndedPatch;
            return Ok(true);
        }
        if let Some(path) = trimmed.strip_prefix(ADD_FILE_MARKER) {
            self.ensure_update_hunk_is_not_empty(trimmed)?;
            self.state.hunks.push(Hunk::AddFile {
                path: PathBuf::from(path.trim()),
                contents: String::new(),
            });
            self.state.mode = StreamingParserMode::AddFile;
            return Ok(true);
        }
        if let Some(path) = trimmed.strip_prefix(DELETE_FILE_MARKER) {
            self.ensure_update_hunk_is_not_empty(trimmed)?;
            self.state.hunks.push(Hunk::DeleteFile {
                path: PathBuf::from(path.trim()),
            });
            self.state.mode = StreamingParserMode::DeleteFile;
            return Ok(true);
        }
        if let Some(path) = trimmed.strip_prefix(UPDATE_FILE_MARKER) {
            self.ensure_update_hunk_is_not_empty(trimmed)?;
            self.state.hunks.push(Hunk::UpdateFile {
                path: PathBuf::from(path.trim()),
                move_path: None,
                chunks: Vec::new(),
            });
            self.state.mode = StreamingParserMode::UpdateFile {
                hunk_line_number: self.line_number,
            };
            return Ok(true);
        }
        Ok(false)
    }

    pub fn push_delta(&mut self, delta: &str) -> Result<Vec<Hunk>, ParseError> {
        for ch in delta.chars() {
            if ch == '\n' {
                let mut line = std::mem::take(&mut self.line_buffer);
                line.truncate(line.strip_suffix('\r').map_or(line.len(), str::len));
                self.line_number += 1;
                self.process_line(&line)?;
            } else {
                self.line_buffer.push(ch);
            }
        }
        Ok(self.state.hunks.clone())
    }

    pub fn finish(&mut self) -> Result<Vec<Hunk>, ParseError> {
        if !self.line_buffer.is_empty() {
            let line = std::mem::take(&mut self.line_buffer);
            self.line_number += 1;
            if line.trim() == END_PATCH_MARKER {
                self.ensure_update_hunk_is_not_empty(line.trim())?;
                self.state.mode = StreamingParserMode::EndedPatch;
            } else {
                self.process_line(&line)?;
            }
        }

        if !matches!(self.state.mode, StreamingParserMode::EndedPatch) {
            return Err(ParseError::InvalidPatchError(
                "The last line of the patch must be '*** End Patch'".to_string(),
            ));
        }

        Ok(self.state.hunks.clone())
    }

    fn process_line(&mut self, line: &str) -> Result<(), ParseError> {
        let trimmed = line.trim();
        match self.state.mode {
            StreamingParserMode::NotStarted => {
                if trimmed == BEGIN_PATCH_MARKER {
                    self.state.mode = StreamingParserMode::StartedPatch;
                    return Ok(());
                }
                Err(ParseError::InvalidPatchError(format!(
                    "The first line of the patch must be '*** Begin Patch', got: '{line}'"
                )))
            }
            StreamingParserMode::StartedPatch => {
                if self.handle_hunk_headers_and_end_patch(trimmed)? {
                    return Ok(());
                }
                Err(ParseError::InvalidHunkError {
                    message: format!(
                        "'{trimmed}' is not a valid hunk header. Valid headers: '*** Add File: ...', '*** Delete File: ...', '*** Update File: ...'"
                    ),
                    line_number: self.line_number,
                })
            }
            StreamingParserMode::AddFile => {
                if self.handle_hunk_headers_and_end_patch(trimmed)? {
                    return Ok(());
                }
                if trimmed == EOF_MARKER {
                    return Ok(());
                }
                if let Some(line_to_add) = line.strip_prefix('+')
                    && let Some(Hunk::AddFile { contents, .. }) = self.state.hunks.last_mut()
                {
                    contents.push_str(line_to_add);
                    contents.push('\n');
                    return Ok(());
                }
                Err(ParseError::InvalidHunkError {
                    message: format!(
                        "Unexpected line in add file hunk: '{line}'. Lines to add must start with '+'"
                    ),
                    line_number: self.line_number,
                })
            }
            StreamingParserMode::DeleteFile => {
                if self.handle_hunk_headers_and_end_patch(trimmed)? {
                    return Ok(());
                }
                Err(ParseError::InvalidHunkError {
                    message: format!("Delete file hunk should not contain lines: '{trimmed}'"),
                    line_number: self.line_number,
                })
            }
            StreamingParserMode::UpdateFile { hunk_line_number } => {
                let update_line = line.trim_end();
                if self.handle_hunk_headers_and_end_patch(update_line)? {
                    return Ok(());
                }

                if let Some(Hunk::UpdateFile {
                    move_path, chunks, ..
                }) = self.state.hunks.last_mut()
                {
                    if chunks.last().is_some_and(|chunk| chunk.is_end_of_file) {
                        if update_line.is_empty() {
                            return Ok(());
                        }
                        if update_line != EMPTY_CHANGE_CONTEXT_MARKER
                            && !update_line.starts_with(CHANGE_CONTEXT_MARKER)
                        {
                            return Err(ParseError::InvalidHunkError {
                                message: format!(
                                    "Expected update hunk to start with a @@ context marker, got: '{line}'"
                                ),
                                line_number: self.line_number,
                            });
                        }
                    }

                    if chunks.is_empty()
                        && move_path.is_none()
                        && let Some(move_to_path) = update_line.strip_prefix(MOVE_TO_MARKER)
                    {
                        *move_path = Some(PathBuf::from(move_to_path.trim()));
                        self.state.mode = StreamingParserMode::UpdateFile { hunk_line_number };
                        return Ok(());
                    }

                    if update_line == EMPTY_CHANGE_CONTEXT_MARKER {
                        chunks.push(UpdateFileChunk::default());
                        self.state.mode = StreamingParserMode::UpdateFile { hunk_line_number };
                        return Ok(());
                    }

                    if let Some(change_context) = update_line.strip_prefix(CHANGE_CONTEXT_MARKER) {
                        chunks.push(UpdateFileChunk {
                            change_context: Some(change_context.to_string()),
                            ..UpdateFileChunk::default()
                        });
                        self.state.mode = StreamingParserMode::UpdateFile { hunk_line_number };
                        return Ok(());
                    }

                    if update_line == EOF_MARKER {
                        if let Some(chunk) = chunks.last_mut() {
                            chunk.is_end_of_file = true;
                        }
                        self.state.mode = StreamingParserMode::UpdateFile { hunk_line_number };
                        return Ok(());
                    }

                    if line.is_empty() {
                        if chunks.is_empty() {
                            chunks.push(UpdateFileChunk::default());
                        }
                        if let Some(chunk) = chunks.last_mut() {
                            chunk.push_context_line(String::new());
                        }
                        self.state.mode = StreamingParserMode::UpdateFile { hunk_line_number };
                        return Ok(());
                    }

                    if let Some(ctx_line) = line.strip_prefix(' ') {
                        if chunks.is_empty() {
                            chunks.push(UpdateFileChunk::default());
                        }
                        if let Some(chunk) = chunks.last_mut() {
                            chunk.push_context_line(ctx_line.to_string());
                        }
                        self.state.mode = StreamingParserMode::UpdateFile { hunk_line_number };
                        return Ok(());
                    }

                    if let Some(added) = line.strip_prefix('+') {
                        if chunks.is_empty() {
                            chunks.push(UpdateFileChunk::default());
                        }
                        if let Some(chunk) = chunks.last_mut() {
                            chunk.new_lines.push(added.to_string());
                        }
                        self.state.mode = StreamingParserMode::UpdateFile { hunk_line_number };
                        return Ok(());
                    }

                    if let Some(removed) = line.strip_prefix('-') {
                        if chunks.is_empty() {
                            chunks.push(UpdateFileChunk::default());
                        }
                        if let Some(chunk) = chunks.last_mut() {
                            chunk.old_lines.push(removed.to_string());
                        }
                        self.state.mode = StreamingParserMode::UpdateFile { hunk_line_number };
                        return Ok(());
                    }

                    return Err(ParseError::InvalidHunkError {
                        message: format!(
                            "Unexpected line in update hunk: '{line}'. Must begin with ' ', '+', or '-'"
                        ),
                        line_number: self.line_number,
                    });
                }

                Ok(())
            }
            StreamingParserMode::EndedPatch => {
                if !trimmed.is_empty() {
                    return Err(ParseError::InvalidPatchError(format!(
                        "Unexpected content after '*** End Patch': '{trimmed}'"
                    )));
                }
                Ok(())
            }
        }
    }
}
