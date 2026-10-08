//! Filesystem model-facing tools consuming FileSystemService (TL-02, D-07).

pub mod editor;
pub mod workspace_patch;
pub use editor::{EditDiagnostic, EditMode, EditSuccess, FileEditOp, RobustFileEditor};

use crate::capability::family::CapabilityFamily;
use crate::capability::traits::fs::FileSystemService;
use crate::tools::definition::{ResourceLimits, ToolExecutionContext, TypedTool};
use crate::tools::error::ToolError;
use crate::tools::risk::RiskClass;
use async_trait::async_trait;
use regex::Regex;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Arc;

fn get_fs(ctx: &ToolExecutionContext) -> Result<Arc<dyn FileSystemService>, ToolError> {
    ctx.capability_registry
        .filesystem()
        .ok_or_else(|| ToolError::capability_unavailable("filesystem", None))
}

// ---------------------------------------------------------------------------
// 1. read_file
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ReadFileInput {
    pub path: String,
    pub offset: Option<u64>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ReadFileOutput {
    pub path: String,
    pub content: String,
    pub bytes_read: usize,
}

pub struct ReadFileTool;

#[async_trait]
impl TypedTool for ReadFileTool {
    type Input = ReadFileInput;
    type Output = ReadFileOutput;

    fn id(&self) -> &str {
        "read_file"
    }

    fn description(&self) -> &str {
        "Read file content from the workspace, optionally bounded by byte offset and limit."
    }

    fn required_capabilities(&self) -> &[CapabilityFamily] {
        &[CapabilityFamily::Filesystem]
    }

    fn base_risk(&self) -> RiskClass {
        RiskClass::ReadOnly
    }

    fn resource_limits(&self) -> ResourceLimits {
        ResourceLimits::new(30, 10 * 1024 * 1024)
    }

    async fn execute(
        &self,
        ctx: &ToolExecutionContext,
        input: Self::Input,
    ) -> Result<Self::Output, ToolError> {
        let fs = get_fs(ctx)?;
        let clean_path = input.path.trim().trim_start_matches('@');
        let path = Path::new(clean_path);
        let raw_bytes = fs.read_file(path, input.offset, input.limit).await?;
        let bytes_read = raw_bytes.len();
        let content = String::from_utf8_lossy(&raw_bytes).to_string();

        Ok(ReadFileOutput {
            path: clean_path.to_string(),
            content,
            bytes_read,
        })
    }
}

// ---------------------------------------------------------------------------
// 2. write_file
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct WriteFileInput {
    pub path: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct WriteFileOutput {
    pub path: String,
    pub bytes_written: usize,
}

pub struct WriteFileTool;

pub(crate) fn normalize_file_content(path: &str, content: &str) -> String {
    let mut s = content.trim().to_string();

    // If the model erroneously echoed the entire JSON response from read_file or similar tools
    while s.starts_with('{') && s.contains("\"content\"") {
        if let Ok(serde_json::Value::Object(map)) = serde_json::from_str::<serde_json::Value>(&s)
            && let Some(serde_json::Value::String(inner)) = map.get("content")
        {
            s = inner.trim().to_string();
            continue;
        }
        break;
    }

    // Strip markdown code fences if erroneously emitted by the model
    if s.starts_with("```")
        && let Some(first_line_end) = s.find('\n')
    {
        let after_fence = &s[first_line_end + 1..];
        if let Some(closing) = after_fence.rfind("```") {
            s = after_fence[..closing].trim().to_string();
        }
    }

    if path.ends_with(".rs") || path.ends_with(".toml") {
        if !s.contains('\n') && s.contains("\\n") {
            s = s.replace("\\n", "\n").replace("\\t", "\t");
        }
        if s.contains(";\\n") || s.contains("{\\n") || s.contains("}\\n") {
            s = s
                .replace(";\\n", ";\n")
                .replace("{\\n", "{\n")
                .replace("}\\n", "}\n")
                .replace(",\\n", ",\n");
        }
        if s.contains("\\\"") {
            s = s.replace("\\\"", "\"");
        }
        return s;
    }
    s
}

pub(crate) fn normalize_edit_content(_path: &str, content: &str) -> String {
    let mut s = content.to_string();

    // If the model erroneously echoed the entire JSON response from read_file or similar tools
    while s.trim().starts_with('{') && s.contains("\"content\"") {
        if let Ok(serde_json::Value::Object(map)) =
            serde_json::from_str::<serde_json::Value>(s.trim())
            && let Some(serde_json::Value::String(inner)) = map.get("content")
        {
            s = inner.clone();
            continue;
        }
        break;
    }

    // Strip markdown code fences if erroneously emitted by the model while preserving inner indentation
    let trimmed = s.trim_start();
    if trimmed.starts_with("```")
        && let Some(first_line_end) = trimmed.find('\n')
    {
        let after_fence = &trimmed[first_line_end + 1..];
        if let Some(closing) = after_fence.rfind("```") {
            s = after_fence[..closing].to_string();
        }
    }

    // Only unescape literal "\\n" if the entire string contains ZERO actual newlines (raw unparsed JSON string)
    // AND clearly has multiple escaped newlines.
    if !s.contains('\n') && (s.contains(";\\n") || s.contains("{\\n") || s.contains("}\\n")) {
        s = s
            .replace(";\\n", ";\n")
            .replace("{\\n", "{\n")
            .replace("}\\n", "}\n")
            .replace(",\\n", ",\n");
    }

    s
}

#[async_trait]
impl TypedTool for WriteFileTool {
    type Input = WriteFileInput;
    type Output = WriteFileOutput;

    fn id(&self) -> &str {
        "write_file"
    }

    fn description(&self) -> &str {
        "Write full content to a file within the workspace, creating parent directories if needed."
    }

    fn required_capabilities(&self) -> &[CapabilityFamily] {
        &[CapabilityFamily::Filesystem]
    }

    fn base_risk(&self) -> RiskClass {
        RiskClass::LowRiskMutation
    }

    fn resource_limits(&self) -> ResourceLimits {
        ResourceLimits::new(30, 10 * 1024 * 1024)
    }

    async fn execute(
        &self,
        ctx: &ToolExecutionContext,
        input: Self::Input,
    ) -> Result<Self::Output, ToolError> {
        let fs = get_fs(ctx)?;
        let clean_path = input.path.trim().trim_start_matches('@');
        let path = Path::new(clean_path);
        let normalized = normalize_file_content(clean_path, &input.content);

        // Safeguard against catastrophic file shrinkage on existing files (GAP-01):
        // If the file already exists and has substantial content (>= 20 lines),
        // reject replacements that delete over 70% of lines (new_lines < orig_lines * 3 / 10).
        if let Ok(raw_existing) = fs.read_file(path, None, None).await {
            let existing_text = String::from_utf8_lossy(&raw_existing);
            let orig_lines = existing_text.lines().count();
            let new_lines = normalized.lines().count();
            if orig_lines >= 20 && (new_lines * 10) < (orig_lines * 3) {
                let drop_pct = (orig_lines.saturating_sub(new_lines)) * 100 / orig_lines;
                return Err(ToolError::precondition_failed(
                    format!(
                        "Catastrophic file shrinkage rejected for '{}': existing file has {} lines, proposed replacement has {} lines ({}% reduction). For additive or localized modifications on existing files, use 'edit_file' for targeted surgical replacements instead of overwriting the entire file.",
                        clean_path, orig_lines, new_lines, drop_pct
                    ),
                    Some("Use the 'edit_file' tool with exact 'old_content' and 'new_content' to make targeted changes without erasing existing definitions.".to_string()),
                ));
            }
        }

        let bytes_written = fs.write_file(path, normalized.as_bytes()).await?;

        Ok(WriteFileOutput {
            path: clean_path.to_string(),
            bytes_written,
        })
    }
}

// ---------------------------------------------------------------------------
// 3. edit_file
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct EditFileInput {
    pub path: String,
    #[serde(default)]
    pub old_content: String,
    #[serde(default)]
    pub new_content: String,
    #[serde(default, deserialize_with = "deserialize_flexible_option_usize")]
    pub start_line: Option<usize>,
    #[serde(default, deserialize_with = "deserialize_flexible_option_usize")]
    pub end_line: Option<usize>,
    #[serde(default)]
    pub mode: Option<String>,
    #[serde(default)]
    pub patch: Option<String>,
}

pub fn deserialize_flexible_option_usize<'de, D>(deserializer: D) -> Result<Option<usize>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    struct FlexibleUsizeVisitor;

    impl<'de> serde::de::Visitor<'de> for FlexibleUsizeVisitor {
        type Value = Option<usize>;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("an integer, a numeric string, or null")
        }

        fn visit_none<E>(self) -> Result<Self::Value, E> {
            Ok(None)
        }

        fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
        where
            D: serde::Deserializer<'de>,
        {
            deserializer.deserialize_any(self)
        }

        fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E> {
            Ok(Some(v as usize))
        }

        fn visit_i64<E>(self, v: i64) -> Result<Self::Value, E> {
            if v >= 0 {
                Ok(Some(v as usize))
            } else {
                Ok(None)
            }
        }

        fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            let trimmed = v.trim();
            if trimmed.is_empty() {
                Ok(None)
            } else if let Ok(u) = trimmed.parse::<usize>() {
                Ok(Some(u))
            } else {
                Err(E::custom(format!("invalid integer: '{v}'")))
            }
        }

        fn visit_unit<E>(self) -> Result<Self::Value, E> {
            Ok(None)
        }
    }

    deserializer.deserialize_option(FlexibleUsizeVisitor)
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct EditFileOutput {
    pub path: String,
    pub success: bool,
    #[serde(default)]
    pub lines_modified: usize,
    #[serde(default)]
    pub start_line: usize,
    #[serde(default)]
    pub end_line: usize,
    #[serde(default)]
    pub details: String,
}

pub struct EditFileTool;

#[async_trait]
impl TypedTool for EditFileTool {
    type Input = EditFileInput;
    type Output = EditFileOutput;

    fn id(&self) -> &str {
        "edit_file"
    }

    fn description(&self) -> &str {
        "Perform surgical code modifications: substring replacement, line-range replacement, insertion, deletion, or unified diff."
    }

    fn required_capabilities(&self) -> &[CapabilityFamily] {
        &[CapabilityFamily::Filesystem]
    }

    fn base_risk(&self) -> RiskClass {
        RiskClass::LowRiskMutation
    }

    fn resource_limits(&self) -> ResourceLimits {
        ResourceLimits::new(30, 1024 * 1024)
    }

    async fn execute(
        &self,
        ctx: &ToolExecutionContext,
        input: Self::Input,
    ) -> Result<Self::Output, ToolError> {
        let fs = get_fs(ctx)?;
        let clean_path = input.path.trim().trim_start_matches('@');
        let path = Path::new(clean_path);

        let normalized_new = normalize_edit_content(clean_path, &input.new_content);
        let normalized_old = normalize_edit_content(clean_path, &input.old_content);

        if normalized_old.is_empty()
            && input.start_line.is_none()
            && input.patch.is_none()
            && input.mode.as_deref() != Some("patch")
        {
            return Err(ToolError::validation(
                "old_content cannot be empty for edit_file".to_string(),
                Some("Specify 'old_content' for substring replacement, 'start_line' for line-range editing, or 'patch' for diff application.".to_string()),
            ));
        }

        let raw = fs.read_file(path, None, None).await.map_err(|e| match e {
            crate::capability::CapabilityError::NotFound { .. } => ToolError::resource_not_found(
                clean_path,
                Some("File not found in workspace. Use 'write_file' to create a new file or 'list_files' to check existing files.".to_string()),
            ),
            other => ToolError::from(other),
        })?;
        let original_text = String::from_utf8(raw).map_err(|e| {
            ToolError::validation(
                format!("File '{}' is not valid UTF-8: {}", clean_path, e),
                Some("Only UTF-8 encoded text files can be edited.".to_string()),
            )
        })?;

        let op = if let Some(ref patch_str) = input.patch {
            FileEditOp::Patch { patch: patch_str }
        } else if input.mode.as_deref() == Some("patch") {
            FileEditOp::Patch {
                patch: &normalized_new,
            }
        } else if let Some(start_line) = input.start_line {
            let end_line = input.end_line.unwrap_or(start_line);
            match input.mode.as_deref() {
                Some("insert_before") => FileEditOp::Insert {
                    line_number: start_line,
                    content: &normalized_new,
                    after: false,
                },
                Some("insert_after") | Some("insert") => FileEditOp::Insert {
                    line_number: start_line,
                    content: &normalized_new,
                    after: true,
                },
                Some("delete") => FileEditOp::Delete {
                    start_line,
                    end_line,
                    expected_old: if normalized_old.is_empty() {
                        None
                    } else {
                        Some(&normalized_old)
                    },
                },
                _ => FileEditOp::LineRange {
                    start_line,
                    end_line,
                    new_content: &normalized_new,
                    expected_old: if normalized_old.is_empty() {
                        None
                    } else {
                        Some(&normalized_old)
                    },
                },
            }
        } else if !normalized_old.is_empty() {
            FileEditOp::Substring {
                old_content: &normalized_old,
                new_content: &normalized_new,
            }
        } else {
            return Err(ToolError::validation(
                "old_content cannot be empty for edit_file".to_string(),
                Some("Specify 'old_content' for substring replacement, 'start_line' for line-range editing, or 'patch' for diff application.".to_string()),
            ));
        };

        match RobustFileEditor::apply(&original_text, &op) {
            Ok(success) => {
                fs.write_file(path, success.new_content.as_bytes()).await?;
                Ok(EditFileOutput {
                    path: clean_path.to_string(),
                    success: true,
                    lines_modified: success.lines_modified,
                    start_line: success.start_line,
                    end_line: success.end_line,
                    details: success.description,
                })
            }
            Err(EditDiagnostic::AmbiguousMatch {
                match_count,
                line_numbers,
                message,
            }) => Err(ToolError::precondition_failed(
                format!(
                    "Failed to edit '{}': ambiguous match ({} occurrences at lines {:?}): {}",
                    clean_path, match_count, line_numbers, message
                ),
                Some(format!(
                    "Target matched multiple times at lines {:?}. Specify 'start_line' and 'end_line' to target a specific range, or provide more surrounding lines.",
                    line_numbers
                )),
            )),
            Err(EditDiagnostic::NotFound {
                total_lines,
                nearest_start_line,
                nearest_end_line,
                nearby_context,
                suggestion,
                ..
            }) => {
                let mut err_msg = format!(
                    "Failed to edit '{}': target content not found in '{}' (file has {} lines). Ensure 'old_content' matches existing code in the file exactly (use 'read_file' to inspect the file first). If previous edits modified this file, call 'read_file' to check current state, or use 'write_file' to rewrite the whole file cleanly.",
                    clean_path, clean_path, total_lines
                );
                if let (Some(s), Some(e)) = (nearest_start_line, nearest_end_line) {
                    err_msg.push_str(&format!(
                        "\nNearest candidate match at lines {}-{}:\n{}",
                        s, e, nearby_context
                    ));
                } else if !nearby_context.is_empty() {
                    err_msg.push_str(&format!("\nFile context:\n{}", nearby_context));
                }
                Err(ToolError::execution_failed(err_msg, None, Some(suggestion)))
            }
            Err(EditDiagnostic::ContentMismatch {
                start_line,
                end_line,
                expected,
                actual,
            }) => Err(ToolError::execution_failed(
                format!(
                    "Failed to edit '{}': content mismatch at lines {}-{}.\nExpected:\n{}\nActual in file:\n{}",
                    clean_path, start_line, end_line, expected, actual
                ),
                None,
                Some(format!(
                    "Call 'read_file' on '{}' lines {}-{} to view actual content before editing.",
                    clean_path, start_line, end_line
                )),
            )),
            Err(EditDiagnostic::OutOfBounds {
                start_line,
                end_line,
                total_lines,
                message,
            }) => Err(ToolError::validation(
                format!(
                    "Failed to edit '{}': line range {}-{} out of bounds (file has {} lines): {}",
                    clean_path, start_line, end_line, total_lines, message
                ),
                Some(format!("Use 1-indexed line numbers between 1 and {}.", total_lines)),
            )),
            Err(EditDiagnostic::InvalidPatch {
                hunk_index,
                message,
                context,
            }) => Err(ToolError::validation(
                format!(
                    "Failed to apply patch to '{}' at hunk {}: {}.\nHunk context:\n{}",
                    clean_path, hunk_index, message, context
                ),
                Some("Ensure patch context lines match target file lines.".to_string()),
            )),
            Err(EditDiagnostic::UnsafeShrinkage {
                original_lines,
                new_lines,
                reduction_percent,
                message,
            }) => Err(ToolError::precondition_failed(
                format!(
                    "Catastrophic file shrinkage rejected for '{}': existing file has {} lines, proposed edit leaves {} lines ({}% reduction): {}",
                    clean_path, original_lines, new_lines, reduction_percent, message
                ),
                Some("For additive or localized modifications, perform surgical edits instead of deleting entire definitions.".to_string()),
            )),
            Err(EditDiagnostic::InvalidInput(msg)) => Err(ToolError::validation(
                format!("Failed to edit '{}': {}", clean_path, msg),
                None,
            )),
        }
    }
}

// ---------------------------------------------------------------------------
// 4. apply_patch
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ApplyPatchInput {
    pub path: String,
    pub patch: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ApplyPatchOutput {
    pub path: String,
    pub applied: bool,
}

pub struct ApplyPatchTool;

#[async_trait]
impl TypedTool for ApplyPatchTool {
    type Input = ApplyPatchInput;
    type Output = ApplyPatchOutput;

    fn id(&self) -> &str {
        "apply_patch"
    }

    fn description(&self) -> &str {
        "Apply a unified diff patch to a file in the workspace."
    }

    fn required_capabilities(&self) -> &[CapabilityFamily] {
        &[CapabilityFamily::Filesystem]
    }

    fn base_risk(&self) -> RiskClass {
        RiskClass::HighRiskMutation
    }

    fn resource_limits(&self) -> ResourceLimits {
        ResourceLimits::new(30, 1024 * 1024)
    }

    async fn execute(
        &self,
        ctx: &ToolExecutionContext,
        input: Self::Input,
    ) -> Result<Self::Output, ToolError> {
        let fs = get_fs(ctx)?;
        let clean_path = input.path.trim().trim_start_matches('@');

        // Multi-file unified diff detection
        if clean_path.is_empty()
            || input.patch.contains("diff --git ")
            || input.patch.contains("\n--- ")
            || input.patch.starts_with("--- ")
        {
            match workspace_patch::WorkspacePatchEngine::apply_workspace_patch(
                fs.as_ref(),
                &input.patch,
            )
            .await
            {
                Ok(report) => {
                    let summary_path = if !clean_path.is_empty() {
                        clean_path.to_string()
                    } else if !report.files_modified.is_empty() {
                        report.files_modified[0].clone()
                    } else if !report.files_created.is_empty() {
                        report.files_created[0].clone()
                    } else {
                        "workspace".to_string()
                    };
                    return Ok(ApplyPatchOutput {
                        path: summary_path,
                        applied: report.success,
                    });
                }
                Err(diag) => {
                    if clean_path.is_empty() {
                        return Err(ToolError::precondition_failed(
                            format!("Failed to apply multi-file patch: {diag}"),
                            Some(
                                "Ensure patch context lines match existing workspace files"
                                    .to_string(),
                            ),
                        ));
                    }
                    // If a specific file path was provided, proceed to single-file fallback
                }
            }
        }

        let path = Path::new(clean_path);
        let raw = fs.read_file(path, None, None).await?;
        let original_content = String::from_utf8(raw)
            .map_err(|e| ToolError::validation(format!("file is not valid UTF-8: {e}"), None))?;

        match RobustFileEditor::apply(
            &original_content,
            &FileEditOp::Patch {
                patch: &input.patch,
            },
        ) {
            Ok(success) => {
                fs.write_file(path, success.new_content.as_bytes()).await?;
                Ok(ApplyPatchOutput {
                    path: clean_path.to_string(),
                    applied: true,
                })
            }
            Err(diag) => {
                // Try workspace patch engine for this single file
                let single_diff = if !input.patch.contains("--- ") {
                    format!("--- a/{clean_path}\n+++ b/{clean_path}\n{}", input.patch)
                } else {
                    input.patch.clone()
                };
                if let Ok(report) = workspace_patch::WorkspacePatchEngine::apply_workspace_patch(
                    fs.as_ref(),
                    &single_diff,
                )
                .await
                {
                    if report.success {
                        return Ok(ApplyPatchOutput {
                            path: clean_path.to_string(),
                            applied: true,
                        });
                    }
                }

                Err(ToolError::precondition_failed(
                    format!("Failed to apply patch to '{clean_path}': {diag}"),
                    Some("Ensure patch context lines match existing file content".to_string()),
                ))
            }
        }
    }
}

// ---------------------------------------------------------------------------
// 4b. apply_workspace_patch (Issue 14)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ApplyWorkspacePatchInput {
    pub patch: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ApplyWorkspacePatchOutput {
    pub applied: bool,
    pub files_modified: Vec<String>,
    pub files_created: Vec<String>,
    pub files_deleted: Vec<String>,
    pub files_renamed: Vec<(String, String)>,
    pub total_hunks_applied: usize,
}

pub struct ApplyWorkspacePatchTool;

#[async_trait]
impl TypedTool for ApplyWorkspacePatchTool {
    type Input = ApplyWorkspacePatchInput;
    type Output = ApplyWorkspacePatchOutput;

    fn id(&self) -> &str {
        "apply_workspace_patch"
    }

    fn description(&self) -> &str {
        "Apply a multi-file unified git diff across the workspace, creating, modifying, deleting, or renaming files atomically."
    }

    fn required_capabilities(&self) -> &[CapabilityFamily] {
        &[CapabilityFamily::Filesystem]
    }

    fn base_risk(&self) -> RiskClass {
        RiskClass::HighRiskMutation
    }

    fn resource_limits(&self) -> ResourceLimits {
        ResourceLimits::new(60, 5 * 1024 * 1024)
    }

    async fn execute(
        &self,
        ctx: &ToolExecutionContext,
        input: Self::Input,
    ) -> Result<Self::Output, ToolError> {
        let fs = get_fs(ctx)?;
        let report =
            workspace_patch::WorkspacePatchEngine::apply_workspace_patch(fs.as_ref(), &input.patch)
                .await
                .map_err(|diag| {
                    ToolError::precondition_failed(
                        format!("Failed to apply workspace patch: {diag}"),
                        Some("Check unified diff syntax and context lines".to_string()),
                    )
                })?;

        Ok(ApplyWorkspacePatchOutput {
            applied: report.success,
            files_modified: report.files_modified,
            files_created: report.files_created,
            files_deleted: report.files_deleted,
            files_renamed: report.files_renamed,
            total_hunks_applied: report.total_hunks_applied,
        })
    }
}

// ---------------------------------------------------------------------------
// 5. list_files
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct ListFilesInput {
    pub path: Option<String>,
    pub recursive: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ListFilesOutput {
    pub files: Vec<String>,
}

pub struct ListFilesTool;

#[async_trait]
impl TypedTool for ListFilesTool {
    type Input = ListFilesInput;
    type Output = ListFilesOutput;

    fn id(&self) -> &str {
        "list_files"
    }

    fn description(&self) -> &str {
        "List files in a workspace directory, optionally recursive."
    }

    fn required_capabilities(&self) -> &[CapabilityFamily] {
        &[CapabilityFamily::Filesystem]
    }

    fn base_risk(&self) -> RiskClass {
        RiskClass::ReadOnly
    }

    fn resource_limits(&self) -> ResourceLimits {
        ResourceLimits::new(30, 1024 * 1024)
    }

    async fn execute(
        &self,
        ctx: &ToolExecutionContext,
        input: Self::Input,
    ) -> Result<Self::Output, ToolError> {
        let fs = get_fs(ctx)?;
        let path_str = input.path.as_deref().unwrap_or(".");
        let path = Path::new(path_str);
        let recursive = input.recursive.unwrap_or(false);

        let paths = fs.list_files(path, recursive).await?;
        let files = paths
            .into_iter()
            .map(|p| {
                if let Ok(rel) = p.strip_prefix(&ctx.workspace_root) {
                    rel.display().to_string()
                } else {
                    p.display().to_string()
                }
            })
            .collect();

        Ok(ListFilesOutput { files })
    }
}

// ---------------------------------------------------------------------------
// 6. glob
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GlobInput {
    pub pattern: String,
    pub path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GlobOutput {
    pub pattern: String,
    pub matches: Vec<String>,
}

pub struct GlobTool;

#[async_trait]
impl TypedTool for GlobTool {
    type Input = GlobInput;
    type Output = GlobOutput;

    fn id(&self) -> &str {
        "glob"
    }

    fn description(&self) -> &str {
        "Find workspace files matching a glob pattern (e.g. '**/*.rs', 'src/*.toml')."
    }

    fn required_capabilities(&self) -> &[CapabilityFamily] {
        &[CapabilityFamily::Filesystem]
    }

    fn base_risk(&self) -> RiskClass {
        RiskClass::ReadOnly
    }

    fn resource_limits(&self) -> ResourceLimits {
        ResourceLimits::new(30, 1024 * 1024)
    }

    async fn execute(
        &self,
        ctx: &ToolExecutionContext,
        input: Self::Input,
    ) -> Result<Self::Output, ToolError> {
        let fs = get_fs(ctx)?;
        let base_path_str = input.path.as_deref().unwrap_or(".");
        let base_path = Path::new(base_path_str);
        let paths = fs.list_files(base_path, true).await?;

        let regex_pattern = glob_to_regex(&input.pattern)?;
        let re = Regex::new(&regex_pattern).map_err(|e| {
            ToolError::validation(
                format!("invalid glob pattern '{}': {e}", input.pattern),
                Some("Check glob syntax, e.g. **/*.rs".to_string()),
            )
        })?;

        let mut matches = Vec::new();
        for p in paths {
            let rel = p
                .strip_prefix(&ctx.workspace_root)
                .unwrap_or(&p)
                .display()
                .to_string();

            if re.is_match(&rel) {
                matches.push(rel);
            }
        }

        Ok(GlobOutput {
            pattern: input.pattern,
            matches,
        })
    }
}

fn glob_to_regex(glob: &str) -> Result<String, ToolError> {
    let mut regex = String::from("^");
    let mut chars = glob.chars().peekable();

    while let Some(c) = chars.next() {
        match c {
            '*' => {
                if chars.peek() == Some(&'*') {
                    chars.next();
                    // Match across directory separators
                    if chars.peek() == Some(&'/') {
                        chars.next();
                        regex.push_str("(?:.*/)?");
                    } else {
                        regex.push_str(".*");
                    }
                } else {
                    // Match within path segment
                    regex.push_str("[^/]*");
                }
            }
            '?' => regex.push_str("[^/]"),
            '.' | '(' | ')' | '+' | '|' | '^' | '$' | '@' | '%' => {
                regex.push('\\');
                regex.push(c);
            }
            other => regex.push(other),
        }
    }
    regex.push('$');
    Ok(regex)
}

// ---------------------------------------------------------------------------
// 7. grep
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GrepInput {
    pub pattern: String,
    pub path: Option<String>,
    pub case_insensitive: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GrepMatch {
    pub file: String,
    pub line_number: usize,
    pub line_content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GrepOutput {
    pub pattern: String,
    pub matches: Vec<GrepMatch>,
}

pub struct GrepTool;

#[async_trait]
impl TypedTool for GrepTool {
    type Input = GrepInput;
    type Output = GrepOutput;

    fn id(&self) -> &str {
        "grep"
    }

    fn description(&self) -> &str {
        "Search file contents in the workspace using regular expressions or plain text."
    }

    fn required_capabilities(&self) -> &[CapabilityFamily] {
        &[CapabilityFamily::Filesystem]
    }

    fn base_risk(&self) -> RiskClass {
        RiskClass::ReadOnly
    }

    fn resource_limits(&self) -> ResourceLimits {
        ResourceLimits::new(30, 2 * 1024 * 1024)
    }

    async fn execute(
        &self,
        ctx: &ToolExecutionContext,
        input: Self::Input,
    ) -> Result<Self::Output, ToolError> {
        let fs = get_fs(ctx)?;
        let case_insensitive = input.case_insensitive.unwrap_or(false);

        let re = regex::RegexBuilder::new(&input.pattern)
            .case_insensitive(case_insensitive)
            .build()
            .map_err(|e| {
                ToolError::validation(
                    format!("invalid regex pattern '{}': {e}", input.pattern),
                    Some("Verify regex syntax".to_string()),
                )
            })?;

        let search_path_str = input.path.as_deref().unwrap_or(".");
        let search_path = Path::new(search_path_str);

        let files: Vec<PathBuf> = if search_path.is_file() {
            vec![search_path.to_path_buf()]
        } else {
            fs.list_files(search_path, true).await?
        };

        let mut matches = Vec::new();
        const MAX_MATCHES: usize = 200;

        for file_path in files {
            if matches.len() >= MAX_MATCHES {
                break;
            }
            if let Ok(meta) = fs.file_metadata(&file_path).await
                && (meta.is_dir || meta.size_bytes > 5 * 1024 * 1024)
            {
                continue;
            }
            if let Ok(bytes) = fs.read_file(&file_path, None, None).await {
                let text = String::from_utf8_lossy(&bytes);
                let rel_file = file_path
                    .strip_prefix(&ctx.workspace_root)
                    .unwrap_or(&file_path)
                    .display()
                    .to_string();

                for (idx, line) in text.lines().enumerate() {
                    if re.is_match(line) {
                        matches.push(GrepMatch {
                            file: rel_file.clone(),
                            line_number: idx + 1,
                            line_content: line.to_string(),
                        });
                        if matches.len() >= MAX_MATCHES {
                            break;
                        }
                    }
                }
            }
        }

        Ok(GrepOutput {
            pattern: input.pattern,
            matches,
        })
    }
}

// ---------------------------------------------------------------------------
// 8. create_directory
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CreateDirectoryInput {
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CreateDirectoryOutput {
    pub path: String,
    pub created: bool,
}

pub struct CreateDirectoryTool;

#[async_trait]
impl TypedTool for CreateDirectoryTool {
    type Input = CreateDirectoryInput;
    type Output = CreateDirectoryOutput;

    fn id(&self) -> &str {
        "create_directory"
    }

    fn description(&self) -> &str {
        "Create a directory and any necessary parent directories within the workspace boundary."
    }

    fn required_capabilities(&self) -> &[CapabilityFamily] {
        &[CapabilityFamily::Filesystem]
    }

    fn base_risk(&self) -> RiskClass {
        RiskClass::LowRiskMutation
    }

    async fn execute(
        &self,
        ctx: &ToolExecutionContext,
        input: Self::Input,
    ) -> Result<Self::Output, ToolError> {
        let fs = get_fs(ctx)?;
        let path = Path::new(&input.path);
        fs.create_directory(path).await?;
        Ok(CreateDirectoryOutput {
            path: input.path,
            created: true,
        })
    }
}

// ---------------------------------------------------------------------------
// 9. delete_file
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DeleteFileInput {
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DeleteFileOutput {
    pub path: String,
    pub deleted: bool,
}

pub struct DeleteFileTool;

#[async_trait]
impl TypedTool for DeleteFileTool {
    type Input = DeleteFileInput;
    type Output = DeleteFileOutput;

    fn id(&self) -> &str {
        "delete_file"
    }

    fn description(&self) -> &str {
        "Delete a file within the workspace boundary. Protected control directories (.git, .m31a) cannot be deleted."
    }

    fn required_capabilities(&self) -> &[CapabilityFamily] {
        &[CapabilityFamily::Filesystem]
    }

    fn base_risk(&self) -> RiskClass {
        RiskClass::HighRiskMutation
    }

    async fn execute(
        &self,
        ctx: &ToolExecutionContext,
        input: Self::Input,
    ) -> Result<Self::Output, ToolError> {
        let fs = get_fs(ctx)?;
        let path = Path::new(&input.path);
        fs.delete_file(path).await?;
        Ok(DeleteFileOutput {
            path: input.path,
            deleted: true,
        })
    }
}

// ---------------------------------------------------------------------------
// 10. move_file
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct MoveFileInput {
    pub source: String,
    pub destination: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct MoveFileOutput {
    pub source: String,
    pub destination: String,
    pub moved: bool,
}

pub struct MoveFileTool;

#[async_trait]
impl TypedTool for MoveFileTool {
    type Input = MoveFileInput;
    type Output = MoveFileOutput;

    fn id(&self) -> &str {
        "move_file"
    }

    fn description(&self) -> &str {
        "Move a file to a new path within the workspace boundary."
    }

    fn required_capabilities(&self) -> &[CapabilityFamily] {
        &[CapabilityFamily::Filesystem]
    }

    fn base_risk(&self) -> RiskClass {
        RiskClass::LowRiskMutation
    }

    async fn execute(
        &self,
        ctx: &ToolExecutionContext,
        input: Self::Input,
    ) -> Result<Self::Output, ToolError> {
        let fs = get_fs(ctx)?;
        let src = Path::new(&input.source);
        let dst = Path::new(&input.destination);
        fs.move_file(src, dst).await?;
        Ok(MoveFileOutput {
            source: input.source,
            destination: input.destination,
            moved: true,
        })
    }
}

// ---------------------------------------------------------------------------
// 11. rename_file
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RenameFileInput {
    pub source: String,
    pub destination: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RenameFileOutput {
    pub source: String,
    pub destination: String,
    pub renamed: bool,
}

pub struct RenameFileTool;

#[async_trait]
impl TypedTool for RenameFileTool {
    type Input = RenameFileInput;
    type Output = RenameFileOutput;

    fn id(&self) -> &str {
        "rename_file"
    }

    fn description(&self) -> &str {
        "Rename a file within the workspace boundary."
    }

    fn required_capabilities(&self) -> &[CapabilityFamily] {
        &[CapabilityFamily::Filesystem]
    }

    fn base_risk(&self) -> RiskClass {
        RiskClass::LowRiskMutation
    }

    async fn execute(
        &self,
        ctx: &ToolExecutionContext,
        input: Self::Input,
    ) -> Result<Self::Output, ToolError> {
        let fs = get_fs(ctx)?;
        let src = Path::new(&input.source);
        let dst = Path::new(&input.destination);
        fs.rename_file(src, dst).await?;
        Ok(RenameFileOutput {
            source: input.source,
            destination: input.destination,
            renamed: true,
        })
    }
}

// ---------------------------------------------------------------------------
// 12. copy_file
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CopyFileInput {
    pub source: String,
    pub destination: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CopyFileOutput {
    pub source: String,
    pub destination: String,
    pub bytes_copied: u64,
}

pub struct CopyFileTool;

#[async_trait]
impl TypedTool for CopyFileTool {
    type Input = CopyFileInput;
    type Output = CopyFileOutput;

    fn id(&self) -> &str {
        "copy_file"
    }

    fn description(&self) -> &str {
        "Copy a file to a new path within the workspace boundary."
    }

    fn required_capabilities(&self) -> &[CapabilityFamily] {
        &[CapabilityFamily::Filesystem]
    }

    fn base_risk(&self) -> RiskClass {
        RiskClass::LowRiskMutation
    }

    async fn execute(
        &self,
        ctx: &ToolExecutionContext,
        input: Self::Input,
    ) -> Result<Self::Output, ToolError> {
        let fs = get_fs(ctx)?;
        let src = Path::new(&input.source);
        let dst = Path::new(&input.destination);
        let bytes_copied = fs.copy_file(src, dst).await?;
        Ok(CopyFileOutput {
            source: input.source,
            destination: input.destination,
            bytes_copied,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capability::providers::local_fs::LocalFileSystemProvider;
    use crate::capability::registry::CapabilityRegistry;
    use tempfile::TempDir;
    use tokio_util::sync::CancellationToken;

    fn create_test_ctx(dir: &TempDir) -> ToolExecutionContext {
        let fs_provider = Arc::new(LocalFileSystemProvider::new(dir.path()).unwrap());
        let registry = Arc::new(CapabilityRegistry::new());
        registry.register_filesystem(fs_provider);
        ToolExecutionContext::new(registry, dir.path().to_path_buf(), CancellationToken::new())
    }

    #[tokio::test]
    async fn test_write_file_new_file_succeeds() {
        let temp = TempDir::new().unwrap();
        let ctx = create_test_ctx(&temp);
        let tool = WriteFileTool;

        let input = WriteFileInput {
            path: "new_file.txt".to_string(),
            content: "hello world\n".to_string(),
        };
        let out = tool.execute(&ctx, input).await.unwrap();
        assert_eq!(out.path, "new_file.txt");
        assert!(out.bytes_written > 0);

        let content = std::fs::read_to_string(temp.path().join("new_file.txt")).unwrap();
        assert_eq!(content, "hello world");
    }

    #[tokio::test]
    async fn test_write_file_catastrophic_shrinkage_rejected() {
        let temp = TempDir::new().unwrap();
        let ctx = create_test_ctx(&temp);
        let tool = WriteFileTool;

        // Create a 50-line file
        let mut original = String::new();
        for i in 1..=50 {
            original.push_str(&format!(
                "// line {}\npub fn item_{}() -> usize {{ {} }}\n",
                i, i, i
            ));
        }
        std::fs::write(temp.path().join("storage.rs"), &original).unwrap();

        // Attempt a destructive overwrite with only 4 lines (e.g. 6-line snippet)
        let destructive_input = WriteFileInput {
            path: "storage.rs".to_string(),
            content:
                "pub fn reset(&mut self) -> u64 {\n    self.counter = 0;\n    self.counter\n}\n"
                    .to_string(),
        };

        let err = tool.execute(&ctx, destructive_input).await.unwrap_err();
        let err_msg = err.to_string();
        assert!(
            err_msg.contains("Catastrophic file shrinkage rejected"),
            "Expected shrinkage rejection error, got: {}",
            err_msg
        );
        assert!(
            err_msg.contains("edit_file"),
            "Expected suggestion to use edit_file"
        );

        // Verify the original file content was preserved!
        let preserved = std::fs::read_to_string(temp.path().join("storage.rs")).unwrap();
        assert_eq!(preserved, original);
    }

    #[tokio::test]
    async fn test_write_file_normal_update_succeeds() {
        let temp = TempDir::new().unwrap();
        let ctx = create_test_ctx(&temp);
        let tool = WriteFileTool;

        // Create a 25-line file
        let mut original = String::new();
        for i in 1..=25 {
            original.push_str(&format!("line {}\n", i));
        }
        std::fs::write(temp.path().join("code.rs"), &original).unwrap();

        // Update with 22 lines (not catastrophic shrinkage)
        let mut updated = String::new();
        for i in 1..=22 {
            updated.push_str(&format!("updated line {}\n", i));
        }

        let input = WriteFileInput {
            path: "code.rs".to_string(),
            content: updated.clone(),
        };
        let out = tool.execute(&ctx, input).await.unwrap();
        assert!(out.bytes_written > 0);

        let content = std::fs::read_to_string(temp.path().join("code.rs")).unwrap();
        assert_eq!(content, updated.trim());
    }

    #[tokio::test]
    async fn test_edit_file_success() {
        let temp = TempDir::new().unwrap();
        let ctx = create_test_ctx(&temp);
        let edit_tool = EditFileTool;

        let initial = "struct State {\n    counter: u64,\n}\n";
        std::fs::write(temp.path().join("state.rs"), initial).unwrap();

        let input = EditFileInput {
            path: "state.rs".to_string(),
            old_content: "struct State {\n    counter: u64,\n}\n".to_string(),
            new_content: "struct State {\n    counter: u64,\n    version: u32,\n}\n".to_string(),
            ..Default::default()
        };

        let out = edit_tool.execute(&ctx, input).await.unwrap();
        assert!(out.success);

        let result = std::fs::read_to_string(temp.path().join("state.rs")).unwrap();
        assert!(result.contains("version: u32"));
    }

    #[tokio::test]
    async fn test_edit_file_old_content_empty_rejected() {
        let temp = TempDir::new().unwrap();
        let ctx = create_test_ctx(&temp);
        let edit_tool = EditFileTool;

        let input = EditFileInput {
            path: "test.rs".to_string(),
            old_content: "".to_string(),
            new_content: "some new content".to_string(),
            ..Default::default()
        };

        let err = edit_tool.execute(&ctx, input).await.unwrap_err();
        assert!(err.to_string().contains("old_content cannot be empty"));
    }

    #[tokio::test]
    async fn test_edit_file_not_found_returns_clear_error() {
        let temp = TempDir::new().unwrap();
        let ctx = create_test_ctx(&temp);
        let edit_tool = EditFileTool;

        std::fs::write(temp.path().join("foo.rs"), "fn bar() {}\n").unwrap();

        let input = EditFileInput {
            path: "foo.rs".to_string(),
            old_content: "fn nonexistent() {}".to_string(),
            new_content: "fn bar() {}".to_string(),
            ..Default::default()
        };

        let err = edit_tool.execute(&ctx, input).await.unwrap_err();
        assert!(err.to_string().contains("Failed to edit"));
        assert!(err.to_string().contains("Ensure 'old_content' matches"));
    }
}
