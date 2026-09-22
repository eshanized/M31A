//! Dynamic artifact externalization and structural preview generation (D-15, CTX-03).
//!
//! Offloads oversized evidence to `FsArtifactStore` before exhausting context budget,
//! compiling bounded structural previews (head/tail/error lines, code outlines, JSON projections)
//! and stable `ArtifactId` locators into agent context.

use crate::error::M31AError;
use crate::ids::ArtifactId;
use crate::persistence::artifacts::fs_store::ArtifactStore;
use sha2::{Digest, Sha256};
use std::sync::Arc;

/// Admission threshold governing when content is inlined vs externalized to artifact storage (D-15).
#[derive(Debug, Clone)]
pub struct AdmissionThreshold {
    pub max_inline_bytes: usize,
}

impl Default for AdmissionThreshold {
    fn default() -> Self {
        Self {
            max_inline_bytes: 1024,
        }
    }
}

/// Result of evaluating an evidence payload against admission thresholds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExternalizeResult {
    /// Payload is within budget and safe to inline directly.
    Inlined(String),
    /// Payload exceeded budget and was offloaded to storage with bounded preview.
    Externalized {
        artifact_id: ArtifactId,
        size_bytes: usize,
        sha256: String,
        preview: String,
        locator: String,
    },
}

impl ExternalizeResult {
    /// Returns the text to be included in compiled context.
    pub fn context_representation(&self) -> &str {
        match self {
            ExternalizeResult::Inlined(s) => s.as_str(),
            ExternalizeResult::Externalized { locator, .. } => locator.as_str(),
        }
    }
}

/// Service that evaluates payload size, persists oversized items to `ArtifactStore`, and builds previews.
pub struct ArtifactExternalizer<S: ArtifactStore> {
    store: Arc<S>,
    threshold: AdmissionThreshold,
}

impl<S: ArtifactStore> ArtifactExternalizer<S> {
    pub fn new(store: Arc<S>) -> Self {
        Self {
            store,
            threshold: AdmissionThreshold::default(),
        }
    }

    pub fn with_threshold(store: Arc<S>, threshold: AdmissionThreshold) -> Self {
        Self { store, threshold }
    }

    pub fn threshold(&self) -> &AdmissionThreshold {
        &self.threshold
    }

    /// Externalize `payload` to storage if larger than `max_inline_bytes`, generating preview and locator.
    pub async fn externalize_if_needed(
        &self,
        payload: &str,
        file_hint: Option<&str>,
    ) -> Result<ExternalizeResult, M31AError> {
        if payload.len() <= self.threshold.max_inline_bytes {
            return Ok(ExternalizeResult::Inlined(payload.to_string()));
        }

        let artifact_id = ArtifactId::new();
        let mut hasher = Sha256::new();
        hasher.update(payload.as_bytes());
        let hash = format!("{:x}", hasher.finalize());

        let ext = file_hint
            .and_then(|h| h.rsplit('.').next())
            .unwrap_or("txt");

        self.store
            .store(artifact_id, payload.as_bytes(), ext)
            .await?;

        let preview = generate_preview(payload, file_hint);
        let locator = format!(
            "[Artifact externalized: id={}, size={} bytes, sha256={}]\n[Preview]\n{}",
            artifact_id,
            payload.len(),
            hash,
            preview
        );

        Ok(ExternalizeResult::Externalized {
            artifact_id,
            size_bytes: payload.len(),
            sha256: hash,
            preview,
            locator,
        })
    }
}

/// Generate a deterministic, bounded structural preview appropriate to the content type (D-15).
pub fn generate_preview(content: &str, file_hint: Option<&str>) -> String {
    let trimmed = content.trim();

    // 1. JSON projection if payload parses as valid JSON object or array
    if (trimmed.starts_with('{') || trimmed.starts_with('['))
        && (file_hint.is_none() || file_hint.is_some_and(|h| h.ends_with(".json")))
        && let Ok(val) = serde_json::from_str::<serde_json::Value>(content)
    {
        return format_json_preview(&val);
    }

    // 2. Code outline if file hint indicates a programming language
    if let Some(hint) = file_hint
        && is_code_extension(hint)
    {
        let outline = extract_code_outline(content);
        if !outline.is_empty() {
            return outline;
        }
    }

    // 3. Log / General text: Head 10 lines + Tail 10 lines + error lines
    format_log_preview(content)
}

fn is_code_extension(path: &str) -> bool {
    let lower = path.to_lowercase();
    lower.ends_with(".rs")
        || lower.ends_with(".py")
        || lower.ends_with(".ts")
        || lower.ends_with(".js")
        || lower.ends_with(".go")
        || lower.ends_with(".java")
        || lower.ends_with(".c")
        || lower.ends_with(".cpp")
        || lower.ends_with(".h")
}

fn extract_code_outline(content: &str) -> String {
    let mut signatures = Vec::new();
    for (line_idx, line) in content.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with("pub fn ")
            || trimmed.starts_with("fn ")
            || trimmed.starts_with("pub struct ")
            || trimmed.starts_with("struct ")
            || trimmed.starts_with("pub enum ")
            || trimmed.starts_with("enum ")
            || trimmed.starts_with("pub trait ")
            || trimmed.starts_with("trait ")
            || trimmed.starts_with("impl ")
            || trimmed.starts_with("def ")
            || trimmed.starts_with("class ")
            || trimmed.starts_with("async def ")
            || trimmed.starts_with("export function ")
            || trimmed.starts_with("export class ")
            || trimmed.starts_with("export interface ")
            || trimmed.starts_with("interface ")
        {
            signatures.push(format!("L{:04}: {}", line_idx + 1, trimmed));
            if signatures.len() >= 30 {
                signatures.push("... [outline truncated]".to_string());
                break;
            }
        }
    }

    if signatures.len() >= 2 {
        format!(
            "--- Structural Code Outline ({} symbols) ---\n{}",
            signatures.len(),
            signatures.join("\n")
        )
    } else {
        String::new()
    }
}

fn format_json_preview(val: &serde_json::Value) -> String {
    match val {
        serde_json::Value::Object(map) => {
            let mut lines = Vec::new();
            lines.push(format!("JSON Object ({} keys):", map.len()));
            for (k, v) in map.iter().take(20) {
                let type_desc = match v {
                    serde_json::Value::String(s) => format!("string (len {})", s.len()),
                    serde_json::Value::Number(n) => format!("number ({})", n),
                    serde_json::Value::Bool(b) => format!("bool ({})", b),
                    serde_json::Value::Array(a) => format!("array (len {})", a.len()),
                    serde_json::Value::Object(o) => format!("object ({} keys)", o.len()),
                    serde_json::Value::Null => "null".to_string(),
                };
                lines.push(format!("  - \"{}\": {}", k, type_desc));
            }
            if map.len() > 20 {
                lines.push(format!(
                    "  ... ({} additional keys omitted)",
                    map.len() - 20
                ));
            }
            lines.join("\n")
        }
        serde_json::Value::Array(arr) => {
            format!(
                "JSON Array ({} items, first item type: {:?})",
                arr.len(),
                arr.first()
                    .map(|v| match v {
                        serde_json::Value::Object(_) => "object",
                        serde_json::Value::Array(_) => "array",
                        serde_json::Value::String(_) => "string",
                        serde_json::Value::Number(_) => "number",
                        serde_json::Value::Bool(_) => "bool",
                        serde_json::Value::Null => "null",
                    })
                    .unwrap_or("none")
            )
        }
        _ => "JSON Value".to_string(),
    }
}

fn format_log_preview(content: &str) -> String {
    let lines: Vec<&str> = content.lines().collect();
    if lines.len() <= 25 {
        return content.to_string();
    }

    let head_count = 10.min(lines.len());
    let head = &lines[..head_count];

    let tail_start = lines.len().saturating_sub(10);
    let tail = &lines[tail_start..];

    // Find errors / failure lines in middle
    let mut error_lines = Vec::new();
    let mid_start = head_count;
    let mid_end = tail_start;

    if mid_start < mid_end {
        for (idx, line) in lines[mid_start..mid_end].iter().enumerate() {
            let lower = line.to_lowercase();
            if lower.contains("error")
                || lower.contains("failed")
                || lower.contains("failure")
                || lower.contains("panic")
            {
                error_lines.push(format!("L{:04}: {}", mid_start + idx + 1, line));
                if error_lines.len() >= 10 {
                    error_lines.push("... [additional errors omitted]".to_string());
                    break;
                }
            }
        }
    }

    let mut result = Vec::new();
    result.push(format!("--- Head ({} lines) ---", head.len()));
    result.extend(head.iter().map(|s| s.to_string()));

    if !error_lines.is_empty() {
        result.push(format!(
            "\n--- Error / Failure Lines ({} found) ---",
            error_lines.len()
        ));
        result.extend(error_lines);
    }

    result.push(format!("\n--- Tail ({} lines) ---", tail.len()));
    result.extend(tail.iter().map(|s| s.to_string()));

    result.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::persistence::artifacts::fs_store::FsArtifactStore;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_inlined_under_threshold() {
        let dir = tempdir().unwrap();
        let store = Arc::new(FsArtifactStore::new(dir.path()));
        let externalizer = ArtifactExternalizer::new(store);

        let small = "small payload";
        let res = externalizer
            .externalize_if_needed(small, None)
            .await
            .unwrap();
        assert_eq!(res, ExternalizeResult::Inlined(small.to_string()));
        assert_eq!(res.context_representation(), small);
    }

    #[tokio::test]
    async fn test_externalized_oversized_payload() {
        let dir = tempdir().unwrap();
        let store = Arc::new(FsArtifactStore::new(dir.path()));
        let externalizer = ArtifactExternalizer::new(store);

        let oversized = "line of log data\n".repeat(100); // > 1024 bytes
        let res = externalizer
            .externalize_if_needed(&oversized, Some("output.log"))
            .await
            .unwrap();

        match res {
            ExternalizeResult::Externalized {
                artifact_id,
                size_bytes,
                sha256,
                preview,
                locator,
            } => {
                assert_eq!(size_bytes, oversized.len());
                assert!(!sha256.is_empty());
                assert!(locator.contains(&artifact_id.to_string()));
                assert!(locator.contains("[Preview]"));
                assert!(preview.contains("--- Head (10 lines) ---"));
                assert!(preview.contains("--- Tail (10 lines) ---"));
            }
            ExternalizeResult::Inlined(_) => panic!("expected externalized payload"),
        }
    }

    #[test]
    fn test_generate_preview_log_with_errors() {
        let mut lines = Vec::new();
        for i in 1..=50 {
            if i == 25 {
                lines.push("CRITICAL: task failed with connection error".to_string());
            } else if i == 30 {
                lines.push("thread 'main' panicked at assertion".to_string());
            } else {
                lines.push(format!("normal log line {}", i));
            }
        }
        let content = lines.join("\n");
        let preview = generate_preview(&content, Some("run.log"));

        assert!(preview.contains("--- Head (10 lines) ---"));
        assert!(preview.contains("--- Error / Failure Lines"));
        assert!(preview.contains("CRITICAL: task failed with connection error"));
        assert!(preview.contains("thread 'main' panicked at assertion"));
        assert!(preview.contains("--- Tail (10 lines) ---"));
    }

    #[test]
    fn test_generate_preview_code_outline() {
        let code = r#"
pub struct Engine {
    pub name: String,
}

impl Engine {
    pub fn new() -> Self {
        Self { name: "test".to_string() }
    }

    fn helper(&self) -> bool {
        true
    }
}

pub enum State {
    Running,
    Stopped,
}
"#;
        let preview = generate_preview(code, Some("engine.rs"));
        assert!(preview.contains("--- Structural Code Outline"));
        assert!(preview.contains("pub struct Engine"));
        assert!(preview.contains("impl Engine"));
        assert!(preview.contains("pub fn new()"));
        assert!(preview.contains("pub enum State"));
    }

    #[test]
    fn test_generate_preview_json_projection() {
        let json_str = r#"{"name": "test", "count": 42, "items": [1, 2, 3], "nested": {"a": 1}}"#;
        let preview = generate_preview(json_str, Some("config.json"));
        assert!(preview.contains("JSON Object (4 keys):"));
        assert!(preview.contains("- \"name\": string (len 4)"));
        assert!(preview.contains("- \"count\": number (42)"));
        assert!(preview.contains("- \"items\": array (len 3)"));
        assert!(preview.contains("- \"nested\": object (1 keys)"));
    }
}
