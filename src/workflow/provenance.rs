//! Provenance tracking and cryptographic content hashing for compiled workflows.

use crate::state_machine::agent::AgentRole;
use crate::workflow::error::WorkflowError;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

use crate::prompt::provenance::PromptInvocationProvenance;
use crate::prompt::strategy::PromptStrategy;

/// Cryptographic provenance record for an individual workflow step and its prompt contract.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StepProvenance {
    /// Step key within the workflow.
    pub step_key: String,
    /// Resolved prompt contract identifier.
    pub prompt_id: String,
    /// Exact version number of the prompt contract.
    pub prompt_version: u32,
    /// Canonical SHA-256 hash of the prompt contract.
    pub prompt_content_hash: String,
    /// Agent role assigned to the step.
    pub role: AgentRole,
    /// Compiled effective prompt hash (if compiled).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effective_prompt_hash: Option<String>,
    /// Prompt strategy applied (if compiled).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt_strategy: Option<PromptStrategy>,
    /// Context digest (if compiled).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_digest: Option<String>,
    /// Expected output contract identifier (if compiled).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_contract_id: Option<String>,
}

impl StepProvenance {
    /// Construct a new StepProvenance record with contract identity and role.
    pub fn new(
        step_key: impl Into<String>,
        prompt_id: impl Into<String>,
        prompt_version: u32,
        prompt_content_hash: impl Into<String>,
        role: AgentRole,
    ) -> Self {
        Self {
            step_key: step_key.into(),
            prompt_id: prompt_id.into(),
            prompt_version,
            prompt_content_hash: prompt_content_hash.into(),
            role,
            effective_prompt_hash: None,
            prompt_strategy: None,
            context_digest: None,
            output_contract_id: None,
        }
    }

    /// Builder method to attach compiled invocation provenance metadata.
    pub fn with_invocation_provenance(mut self, prov: &PromptInvocationProvenance) -> Self {
        self.effective_prompt_hash = Some(prov.effective_prompt_hash.clone());
        self.prompt_strategy = Some(prov.prompt_strategy);
        self.context_digest = Some(prov.context_digest.clone());
        self.output_contract_id = Some(prov.output_contract_id.clone());
        self
    }
}

/// Cryptographic provenance record capturing the exact compilation origin of a workflow.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowProvenance {
    /// Manifest workflow identifier.
    pub manifest_id: String,
    /// Manifest format version.
    pub manifest_version: u32,
    /// Canonical SHA-256 hash of the manifest content.
    pub manifest_hash: String,
    /// Version number of the workflow definition.
    pub workflow_version: u32,
    /// Provenance per step, ordered deterministically by step key.
    pub step_provenance: BTreeMap<String, StepProvenance>,
    /// Composite SHA-256 hash anchoring the entire compiled workflow state.
    pub composite_hash: String,
    /// Workspace-relative source path of the manifest (if loaded from a file).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_path: Option<String>,
    /// Exact UTC timestamp when compilation occurred.
    pub compiled_at: DateTime<Utc>,
}

impl WorkflowProvenance {
    /// Construct a new WorkflowProvenance record and compute its composite hash.
    pub fn new(
        manifest_id: impl Into<String>,
        manifest_version: u32,
        manifest_hash: impl Into<String>,
        workflow_version: u32,
        step_provenance: BTreeMap<String, StepProvenance>,
        source_path: Option<String>,
        compiled_at: DateTime<Utc>,
    ) -> Self {
        let manifest_id_str = manifest_id.into();
        let manifest_hash_str = manifest_hash.into();

        let composite_hash = Self::calculate_composite_hash(&manifest_hash_str, &step_provenance);

        Self {
            manifest_id: manifest_id_str,
            manifest_version,
            manifest_hash: manifest_hash_str,
            workflow_version,
            step_provenance,
            composite_hash,
            source_path,
            compiled_at,
        }
    }

    /// Calculate deterministic composite hash combining the manifest hash and all step prompt hashes.
    pub fn calculate_composite_hash(
        manifest_hash: &str,
        step_provenance: &BTreeMap<String, StepProvenance>,
    ) -> String {
        let mut hasher = Sha256::new();
        hasher.update(manifest_hash.as_bytes());
        hasher.update(b"|");

        for (step_key, prov) in step_provenance {
            hasher.update(step_key.as_bytes());
            hasher.update(b":");
            hasher.update(prov.prompt_id.as_bytes());
            hasher.update(b":v");
            hasher.update(prov.prompt_version.to_be_bytes());
            hasher.update(b":");
            hasher.update(prov.prompt_content_hash.as_bytes());
            hasher.update(b":");
            hasher.update(prov.role.as_str().as_bytes());
            hasher.update(b";");
        }

        format!("{:x}", hasher.finalize())
    }

    /// Verify this provenance record against active manifest content and prompt hashes.
    pub fn verify(
        &self,
        current_manifest_hash: &str,
        current_prompt_hashes: &BTreeMap<String, String>,
    ) -> Result<(), WorkflowError> {
        if self.manifest_hash != current_manifest_hash {
            return Err(WorkflowError::ProvenanceFailure(format!(
                "manifest hash mismatch: compiled with '{}', current is '{}'",
                self.manifest_hash, current_manifest_hash
            )));
        }

        for (step_key, step_prov) in &self.step_provenance {
            match current_prompt_hashes.get(&step_prov.prompt_id) {
                None => {
                    return Err(WorkflowError::ProvenanceFailure(format!(
                        "prompt '{}' for step '{}' missing from active catalog",
                        step_prov.prompt_id, step_key
                    )));
                }
                Some(current_hash) => {
                    if &step_prov.prompt_content_hash != current_hash {
                        return Err(WorkflowError::ProvenanceFailure(format!(
                            "prompt '{}' for step '{}' hash mismatch: compiled with '{}', current is '{}'",
                            step_prov.prompt_id,
                            step_key,
                            step_prov.prompt_content_hash,
                            current_hash
                        )));
                    }
                }
            }
        }

        Ok(())
    }
}
