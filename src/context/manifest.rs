//! Context compilation audit manifest model (D-16, CTX-04).
//!
//! Emits auditable `ContextCompilationManifest` records tracking before/after token counts,
//! section priorities, provenance, and reverse-compaction decisions.

use crate::context::evidence::{
    ContextSelectionStatus, EvidenceManifestRecord, OmittedEvidenceRecord,
};
use crate::context::priority::ContextPriority;
use serde::{Deserialize, Serialize};

/// Detailed entry for an individual context section within the compilation manifest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManifestSectionEntry {
    pub section_id: String,
    pub priority: ContextPriority,
    pub original_tokens: usize,
    pub final_tokens: usize,
    /// Compaction action: "retained_full", "compacted_excerpt", "compacted_summary", "dropped", "externalized"
    pub compaction_action: String,
    pub provenance: String,
}

/// Auditable manifest recording the decisions made during context assembly and compaction (D-16, CTX-04).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextCompilationManifest {
    pub revision: u32,
    pub total_budget: usize,
    pub reserved_headroom: usize,
    pub compiled_tokens: usize,
    pub sections: Vec<ManifestSectionEntry>,
    #[serde(default)]
    pub selected_evidence: Vec<EvidenceManifestRecord>,
    #[serde(default)]
    pub omitted_evidence: Vec<OmittedEvidenceRecord>,
    #[serde(default)]
    pub repository_revision: Option<String>,
    #[serde(default)]
    pub selection_status: ContextSelectionStatus,
    /// Prompt contract id actually compiled (resolved canonical identity).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt_id: Option<String>,
    /// Prompt contract version actually compiled.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt_version: Option<u32>,
    /// Which binding won prompt selection precedence.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt_source: Option<crate::kernel::seams::context::PromptSelectionSource>,
    /// Content hash of the prompt contract actually compiled.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt_content_hash: Option<String>,
}

impl ContextCompilationManifest {
    pub fn new(
        revision: u32,
        total_budget: usize,
        reserved_headroom: usize,
        compiled_tokens: usize,
        sections: Vec<ManifestSectionEntry>,
    ) -> Self {
        Self {
            revision,
            total_budget,
            reserved_headroom,
            compiled_tokens,
            sections,
            selected_evidence: Vec::new(),
            omitted_evidence: Vec::new(),
            repository_revision: None,
            selection_status: ContextSelectionStatus::Complete,
            prompt_id: None,
            prompt_version: None,
            prompt_source: None,
            prompt_content_hash: None,
        }
    }

    pub fn with_evidence(
        mut self,
        selected_evidence: Vec<EvidenceManifestRecord>,
        omitted_evidence: Vec<OmittedEvidenceRecord>,
        repository_revision: Option<String>,
        selection_status: ContextSelectionStatus,
    ) -> Self {
        self.selected_evidence = selected_evidence;
        self.omitted_evidence = omitted_evidence;
        self.repository_revision = repository_revision;
        self.selection_status = selection_status;
        self
    }

    pub fn to_contract(&self) -> crate::kernel::seams::context::ContextCompilationContract {
        self.into()
    }
}

impl From<ManifestSectionEntry> for crate::kernel::seams::context::ContextSectionContract {
    fn from(entry: ManifestSectionEntry) -> Self {
        Self {
            section_id: entry.section_id,
            original_tokens: entry.original_tokens,
            final_tokens: entry.final_tokens,
            compaction_action: entry.compaction_action,
            provenance: entry.provenance,
        }
    }
}

impl From<&ManifestSectionEntry> for crate::kernel::seams::context::ContextSectionContract {
    fn from(entry: &ManifestSectionEntry) -> Self {
        Self {
            section_id: entry.section_id.clone(),
            original_tokens: entry.original_tokens,
            final_tokens: entry.final_tokens,
            compaction_action: entry.compaction_action.clone(),
            provenance: entry.provenance.clone(),
        }
    }
}

impl From<ContextCompilationManifest>
    for crate::kernel::seams::context::ContextCompilationContract
{
    fn from(manifest: ContextCompilationManifest) -> Self {
        Self {
            revision: manifest.revision,
            total_budget: manifest.total_budget,
            reserved_headroom: manifest.reserved_headroom,
            compiled_tokens: manifest.compiled_tokens,
            sections: manifest.sections.into_iter().map(Into::into).collect(),
            selected_evidence: manifest
                .selected_evidence
                .into_iter()
                .map(Into::into)
                .collect(),
            omitted_evidence: manifest
                .omitted_evidence
                .into_iter()
                .map(Into::into)
                .collect(),
            repository_revision: manifest.repository_revision,
            selection_status: manifest.selection_status.as_str().to_string(),
            prompt_id: manifest.prompt_id,
            prompt_version: manifest.prompt_version,
            prompt_source: manifest.prompt_source,
            prompt_content_hash: manifest.prompt_content_hash,
        }
    }
}

impl From<&ContextCompilationManifest>
    for crate::kernel::seams::context::ContextCompilationContract
{
    fn from(manifest: &ContextCompilationManifest) -> Self {
        Self {
            revision: manifest.revision,
            total_budget: manifest.total_budget,
            reserved_headroom: manifest.reserved_headroom,
            compiled_tokens: manifest.compiled_tokens,
            sections: manifest.sections.iter().map(Into::into).collect(),
            selected_evidence: manifest.selected_evidence.iter().map(Into::into).collect(),
            omitted_evidence: manifest.omitted_evidence.iter().map(Into::into).collect(),
            repository_revision: manifest.repository_revision.clone(),
            selection_status: manifest.selection_status.as_str().to_string(),
            prompt_id: manifest.prompt_id.clone(),
            prompt_version: manifest.prompt_version,
            prompt_source: manifest.prompt_source,
            prompt_content_hash: manifest.prompt_content_hash.clone(),
        }
    }
}
