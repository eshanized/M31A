//! Stage 10: Capture & Normalization Stage (TL-03, per D-09, D-11).

use std::sync::Arc;
use std::time::Duration;

use sha2::{Digest, Sha256};

use crate::persistence::artifacts::fs_store::ArtifactStore;
use crate::pipeline::capture::OutputCaptureManager;
use crate::pipeline::error::ToolError;
use crate::pipeline::stages::execution::RawExecutionState;
use crate::telemetry::redactor::SecretRedactor;

/// Explicit security-classified output evidence produced by Stage 10 (Finding E).
///
/// Disambiguates data classification across the pipeline:
/// - `raw_output`: raw execution evidence preserved for internal verification
/// - `model_visible_output`: bounded, context-safe representation provided to the agent/model
/// - `diagnostic_output`: sanitized evidence safe for logging, telemetry, and reporting
/// - `audit_digest`: SHA-256 cryptographic digest of raw execution output for durable auditing
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PipelineOutputEvidence {
    pub raw_output: String,
    pub model_visible_output: String,
    pub diagnostic_output: String,
    pub audit_digest: String,
}

impl PipelineOutputEvidence {
    pub fn new(raw: String, model_visible: String) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(raw.as_bytes());
        let audit_digest = format!("{:x}", hasher.finalize());
        let diagnostic_output = SecretRedactor::new().redact_text(&raw);

        Self {
            raw_output: raw,
            model_visible_output: model_visible,
            diagnostic_output,
            audit_digest,
        }
    }

    pub fn empty() -> Self {
        Self {
            raw_output: String::new(),
            model_visible_output: String::new(),
            diagnostic_output: String::new(),
            audit_digest: format!("{:x}", Sha256::digest(b"")),
        }
    }
}

/// Typed state envelope containing normalized result with explicit security contracts.
pub struct NormalizedResultState {
    pub action_id: String,
    pub tool_id: String,
    pub success: bool,
    pub output: String,
    pub error: Option<ToolError>,
    pub duration: Duration,
    pub evidence: PipelineOutputEvidence,
}

impl NormalizedResultState {
    pub fn raw_output(&self) -> &str {
        &self.evidence.raw_output
    }

    pub fn model_visible_output(&self) -> &str {
        &self.evidence.model_visible_output
    }

    pub fn diagnostic_output(&self) -> &str {
        &self.evidence.diagnostic_output
    }

    pub fn audit_digest(&self) -> &str {
        &self.evidence.audit_digest
    }
}

/// Stage 10: Captures output, applies budget-aware artifact externalization, and normalizes outcome.
pub struct CaptureNormalizationStage;

impl CaptureNormalizationStage {
    pub async fn execute(
        state: RawExecutionState,
        capture_manager: &OutputCaptureManager,
        artifact_store: Option<&Arc<dyn ArtifactStore>>,
    ) -> NormalizedResultState {
        let action_id = state.action.id;
        let tool_id = state.tool_id;
        let duration = state.duration;

        match state.outcome {
            Ok(value) => {
                // Extract clean text representation of output
                let raw_output = if let Some(s) = value.as_str() {
                    s.to_string()
                } else {
                    serde_json::to_string_pretty(&value).unwrap_or_else(|_| value.to_string())
                };

                // Apply budget ceilings: inline vs external artifact
                match capture_manager
                    .process_output(&raw_output, artifact_store)
                    .await
                {
                    Ok(processed_output) => {
                        let evidence =
                            PipelineOutputEvidence::new(raw_output, processed_output.clone());
                        NormalizedResultState {
                            action_id,
                            tool_id,
                            success: true,
                            output: processed_output,
                            error: None,
                            duration,
                            evidence,
                        }
                    }
                    Err(artifact_err) => {
                        // Fail closed on artifactization failure (D-11)
                        NormalizedResultState {
                            action_id,
                            tool_id,
                            success: false,
                            output: String::new(),
                            error: Some(artifact_err),
                            duration,
                            evidence: PipelineOutputEvidence::empty(),
                        }
                    }
                }
            }
            Err(tool_err) => NormalizedResultState {
                action_id,
                tool_id,
                success: false,
                output: String::new(),
                error: Some(tool_err),
                duration,
                evidence: PipelineOutputEvidence::empty(),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pipeline_output_evidence_classifications() {
        let raw = "Sensitive execution log containing key: nvapi-secret12345678901234567890 and output data";
        let model_visible = "Bounded output: 1234 bytes";
        let evidence = PipelineOutputEvidence::new(raw.to_string(), model_visible.to_string());

        // 1. Raw output preserves exact data for verification
        assert_eq!(evidence.raw_output, raw);

        // 2. Model visible matches bounded projection
        assert_eq!(evidence.model_visible_output, model_visible);

        // 3. Diagnostic output scrubs credentials
        assert!(
            !evidence
                .diagnostic_output
                .contains("nvapi-secret12345678901234567890")
        );
        assert!(evidence.diagnostic_output.contains("[REDACTED:NVIDIA_KEY]"));

        // 4. Audit digest is valid 64-char SHA256 hex string
        assert_eq!(evidence.audit_digest.len(), 64);
        let expected_hash = format!("{:x}", sha2::Sha256::digest(raw.as_bytes()));
        assert_eq!(evidence.audit_digest, expected_hash);
    }

    #[test]
    fn test_pipeline_output_evidence_empty() {
        let empty = PipelineOutputEvidence::empty();
        assert!(empty.raw_output.is_empty());
        assert!(empty.model_visible_output.is_empty());
        assert!(empty.diagnostic_output.is_empty());
        assert_eq!(empty.audit_digest.len(), 64);
    }
}
