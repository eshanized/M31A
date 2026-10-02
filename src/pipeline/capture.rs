//! Budget-aware tool output capture and artifact externalization (TL-03, per D-11).

use crate::ids::ArtifactId;
use crate::persistence::artifacts::fs_store::{ArtifactStore, EvidenceClassification};
use crate::pipeline::error::ToolError;
use std::sync::Arc;

pub const DEFAULT_MAX_INLINE_BYTES: usize = 16384; // 16KB
pub const DEFAULT_MAX_INLINE_LINES: usize = 100;
pub const PREVIEW_HEAD_LINES: usize = 50;
pub const PREVIEW_TAIL_LINES: usize = 50;

/// Manages tool output budgeting, inline ceilings, and artifact promotion (D-11).
#[derive(Debug, Clone)]
pub struct OutputCaptureManager {
    max_inline_bytes: usize,
    max_inline_lines: usize,
}

impl Default for OutputCaptureManager {
    fn default() -> Self {
        Self::new(DEFAULT_MAX_INLINE_BYTES, DEFAULT_MAX_INLINE_LINES)
    }
}

impl OutputCaptureManager {
    /// Create a new capture manager with custom byte and line ceilings.
    pub fn new(max_inline_bytes: usize, max_inline_lines: usize) -> Self {
        Self {
            max_inline_bytes,
            max_inline_lines,
        }
    }

    pub fn max_inline_bytes(&self) -> usize {
        self.max_inline_bytes
    }

    pub fn max_inline_lines(&self) -> usize {
        self.max_inline_lines
    }

    /// Evaluates raw output against inline budget thresholds.
    /// If within budget, returns output inline.
    /// If exceeding budget, externalizes to `ArtifactStore` and generates bounded preview.
    /// If artifact persistence fails, fails closed with typed error (D-11).
    pub async fn process_output(
        &self,
        raw_output: &str,
        artifact_store: Option<&Arc<dyn ArtifactStore>>,
    ) -> Result<String, ToolError> {
        let total_bytes = raw_output.len();
        let lines: Vec<&str> = raw_output.lines().collect();
        let total_lines = lines.len();

        if total_bytes <= self.max_inline_bytes && total_lines <= self.max_inline_lines {
            return Ok(raw_output.to_string());
        }

        // Output exceeds budget: must externalize to ArtifactStore (D-11)
        let Some(store) = artifact_store else {
            return Err(ToolError::resource_exhausted(
                "ARTIFACT_STORE_MISSING",
                format!(
                    "Tool output exceeds inline budget ({} bytes, {} lines), but no artifact store is configured",
                    total_bytes, total_lines
                ),
                Some("Configure an artifact store to handle oversized tool outputs".to_string()),
            ));
        };

        let artifact_id = ArtifactId::new();
        // P1-03: oversized externalized output is privileged raw evidence —
        // store it WITH explicit classification. Only the bounded preview
        // below (scrubbed at Stage 10) may reach model context.
        if let Err(e) = store
            .store_classified(
                artifact_id,
                raw_output.as_bytes(),
                "txt",
                EvidenceClassification::RawPrivileged,
            )
            .await
        {
            return Err(ToolError::resource_exhausted(
                "ARTIFACT_STORAGE_FAILED",
                format!(
                    "Failed to externalize oversized tool output ({} bytes): {}",
                    total_bytes, e
                ),
                Some(
                    "Ensure artifact storage has sufficient disk space and write permissions"
                        .to_string(),
                ),
            ));
        }

        // Generate bounded preview: head 50 + tail 50 lines
        let preview = if total_lines <= PREVIEW_HEAD_LINES + PREVIEW_TAIL_LINES {
            raw_output.to_string()
        } else {
            let head = lines[..PREVIEW_HEAD_LINES].join("\n");
            let tail = lines[total_lines - PREVIEW_TAIL_LINES..].join("\n");
            let omitted = total_lines - (PREVIEW_HEAD_LINES + PREVIEW_TAIL_LINES);
            format!(
                "{}\n\n... [{} lines omitted] ...\n\n{}",
                head, omitted, tail
            )
        };

        // Format required by D-11:
        // "[Output truncated. Full {total_bytes} bytes stored as ArtifactId: {artifact_id}. Bounded preview:\n{preview}]"
        Ok(format!(
            "[Output truncated. Full {} bytes stored as ArtifactId: {}. Bounded preview:\n{}]",
            total_bytes, artifact_id, preview
        ))
    }
}
