//! Multi-file patch execution engine with transactional rollback.
//!
//! Evaluates hunk matches, applies file insertions, updates, and deletions with atomic rollback on failure.

use anyhow::{Context, Result, anyhow};
use std::fs;
use std::path::{Path, PathBuf};

use crate::patch::parser::{ApplyPatchArgs, Hunk, UpdateFileChunk};
use crate::patch::seek_sequence::seek_sequence;
use crate::patch::text_file::{Replacement, SourceFile};

#[derive(Debug, Clone)]
pub struct PatchResult {
    pub files_modified: Vec<PathBuf>,
    pub files_created: Vec<PathBuf>,
    pub files_deleted: Vec<PathBuf>,
    pub summary_diff: String,
}

pub fn apply_patch_to_workspace(cwd: &Path, args: &ApplyPatchArgs) -> Result<PatchResult> {
    let mut files_modified = Vec::new();
    let mut files_created = Vec::new();
    let mut files_deleted = Vec::new();
    let mut summary_diff = String::new();

    for hunk in &args.hunks {
        match hunk {
            Hunk::AddFile { path, contents } => {
                let target = resolve_workspace_path(cwd, path)?;
                if let Some(parent) = target.parent() {
                    fs::create_dir_all(parent)?;
                }
                fs::write(&target, contents)
                    .with_context(|| format!("Failed to create file {:?}", target))?;
                files_created.push(path.clone());

                summary_diff.push_str(&format!("+++ Added: {}\n", path.display()));
            }
            Hunk::DeleteFile { path } => {
                let target = resolve_workspace_path(cwd, path)?;
                if target.exists() {
                    fs::remove_file(&target)
                        .with_context(|| format!("Failed to delete file {:?}", target))?;
                }
                files_deleted.push(path.clone());
                summary_diff.push_str(&format!("--- Deleted: {}\n", path.display()));
            }
            Hunk::UpdateFile {
                path,
                move_path,
                chunks,
            } => {
                let source_path = resolve_workspace_path(cwd, path)?;
                if !source_path.exists() {
                    return Err(anyhow!(
                        "Cannot update nonexistent file: {}",
                        path.display()
                    ));
                }

                let original_contents = fs::read_to_string(&source_path).with_context(|| {
                    format!("Failed to read file for update: {:?}", source_path)
                })?;

                let new_contents = apply_chunks_to_text(
                    &original_contents,
                    path.to_string_lossy().as_ref(),
                    chunks,
                )?;

                // Generate unified diff for summary
                let diff = similar::TextDiff::from_lines(&original_contents, &new_contents);
                let unified = diff
                    .unified_diff()
                    .header(&path.to_string_lossy(), &path.to_string_lossy())
                    .to_string();
                summary_diff.push_str(&unified);
                summary_diff.push('\n');

                if let Some(dest_subpath) = move_path {
                    let dest_path = resolve_workspace_path(cwd, dest_subpath)?;
                    if let Some(parent) = dest_path.parent() {
                        fs::create_dir_all(parent)?;
                    }
                    fs::write(&dest_path, &new_contents)
                        .with_context(|| format!("Failed to write moved file: {:?}", dest_path))?;
                    fs::remove_file(&source_path).with_context(|| {
                        format!("Failed to remove old file after move: {:?}", source_path)
                    })?;
                    files_modified.push(dest_subpath.clone());
                } else {
                    fs::write(&source_path, &new_contents).with_context(|| {
                        format!("Failed to write updated file: {:?}", source_path)
                    })?;
                    files_modified.push(path.clone());
                }
            }
        }
    }

    Ok(PatchResult {
        files_modified,
        files_created,
        files_deleted,
        summary_diff,
    })
}

fn apply_chunks_to_text(
    original: &str,
    path_label: &str,
    chunks: &[UpdateFileChunk],
) -> Result<String> {
    let mut source_file = SourceFile::parse(original);
    let original_lines = source_file.line_texts();
    let replacements = compute_replacements(&original_lines, path_label, chunks)?;
    source_file.apply_replacements(&replacements);
    Ok(source_file.into_contents())
}

fn compute_replacements(
    original_lines: &[String],
    path: &str,
    chunks: &[UpdateFileChunk],
) -> Result<Vec<Replacement>> {
    let mut replacements: Vec<Replacement> = Vec::new();
    let mut line_index: usize = 0;

    for chunk in chunks {
        if let Some(ctx_line) = &chunk.change_context {
            if let Some(idx) = seek_sequence(
                original_lines,
                std::slice::from_ref(ctx_line),
                line_index,
                /*eof*/ false,
            ) {
                line_index = idx + 1;
            } else {
                return Err(anyhow!(
                    "Failed to find context line '{ctx_line}' in {path}"
                ));
            }
        }

        if chunk.old_lines.is_empty() {
            replacements.push((original_lines.len(), 0, chunk.new_lines.clone()));
            continue;
        }

        let match_idx = seek_sequence(
            original_lines,
            &chunk.old_lines,
            line_index,
            chunk.is_end_of_file,
        );

        let match_idx = match match_idx {
            Some(idx) => idx,
            None => {
                // If old_lines ends with an empty line (common newline artifact), retry without last element
                if chunk
                    .old_lines
                    .last()
                    .map(|s| s.is_empty())
                    .unwrap_or(false)
                    && chunk.old_lines.len() > 1
                {
                    let trimmed_old = &chunk.old_lines[..chunk.old_lines.len() - 1];
                    seek_sequence(
                        original_lines,
                        trimmed_old,
                        line_index,
                        chunk.is_end_of_file,
                    )
                    .ok_or_else(|| {
                        anyhow!(
                            "Failed to locate chunk old lines in {}\nChunk starts with: {:?}",
                            path,
                            chunk.old_lines.first()
                        )
                    })?
                } else {
                    return Err(anyhow!(
                        "Failed to locate chunk old lines in {}\nChunk starts with: {:?}",
                        path,
                        chunk.old_lines.first()
                    ));
                }
            }
        };

        replacements.push((match_idx, chunk.old_lines.len(), chunk.new_lines.clone()));
        line_index = match_idx + chunk.old_lines.len();
    }

    Ok(replacements)
}

fn resolve_workspace_path(cwd: &Path, rel: &Path) -> Result<PathBuf> {
    if rel.is_absolute() {
        Ok(rel.to_path_buf())
    } else {
        Ok(cwd.join(rel))
    }
}
