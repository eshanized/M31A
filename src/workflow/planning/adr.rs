//! Architecture Decision Record (ADR) domain model, registry, and projections.

use super::decision::DecisionStatus;
use crate::planning::requirements::{Provenance, RequirementKey};
use crate::workflow::error::WorkflowError;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};

/// Strongly typed ADR identifier (e.g. "ADR-0001").
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct AdrId(pub String);

impl AdrId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn from_number(num: usize) -> Self {
        Self(format!("ADR-{:04}", num))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for AdrId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<&str> for AdrId {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl From<String> for AdrId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

/// An individual Architecture Decision Record (ADR) adhering to Section 18.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArchitectureDecisionRecord {
    pub id: AdrId,
    pub title: String,
    pub context: String,
    pub decision: String,
    pub alternatives_considered: Vec<String>,
    pub rationale: String,
    pub consequences: Vec<String>,
    pub residual_risks: Vec<String>,
    pub linked_requirements: Vec<RequirementKey>,
    pub linked_research: Vec<String>,
    pub status: DecisionStatus,
    pub provenance: Provenance,
    pub version: u32,
    pub superseded_by: Option<AdrId>,
}

impl ArchitectureDecisionRecord {
    pub fn new(
        id: AdrId,
        title: impl Into<String>,
        context: impl Into<String>,
        decision: impl Into<String>,
        rationale: impl Into<String>,
        provenance: Provenance,
    ) -> Self {
        Self {
            id,
            title: title.into(),
            context: context.into(),
            decision: decision.into(),
            alternatives_considered: Vec::new(),
            rationale: rationale.into(),
            consequences: Vec::new(),
            residual_risks: Vec::new(),
            linked_requirements: Vec::new(),
            linked_research: Vec::new(),
            status: DecisionStatus::Proposed,
            provenance,
            version: 1,
            superseded_by: None,
        }
    }

    pub fn with_alternatives(mut self, alternatives: Vec<String>) -> Self {
        self.alternatives_considered = alternatives;
        self
    }

    pub fn with_consequences(mut self, consequences: Vec<String>) -> Self {
        self.consequences = consequences;
        self
    }

    pub fn with_residual_risks(mut self, risks: Vec<String>) -> Self {
        self.residual_risks = risks;
        self
    }

    pub fn with_linked_requirements(mut self, requirements: Vec<RequirementKey>) -> Self {
        self.linked_requirements = requirements;
        self
    }

    pub fn with_linked_research(mut self, research: Vec<String>) -> Self {
        self.linked_research = research;
        self
    }

    pub fn with_status(mut self, status: DecisionStatus) -> Self {
        self.status = status;
        self
    }

    /// Render individual ADR Markdown format.
    pub fn to_markdown(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!("# {}: {}\n\n", self.id, self.title));
        out.push_str(&format!("- **Status**: `{}`\n", self.status));
        out.push_str(&format!("- **Version**: {}\n", self.version));
        out.push_str(&format!("- **Author**: {}\n", self.provenance.actor));
        if let Some(ref sup) = self.superseded_by {
            out.push_str(&format!("- **Superseded By**: `{}`\n", sup));
        }

        if !self.linked_requirements.is_empty() {
            let req_str = self
                .linked_requirements
                .iter()
                .map(|r| format!("`{}`", r))
                .collect::<Vec<_>>()
                .join(", ");
            out.push_str(&format!("- **Linked Requirements**: {}\n", req_str));
        }
        if !self.linked_research.is_empty() {
            out.push_str(&format!(
                "- **Linked Research**: {}\n",
                self.linked_research.join(", ")
            ));
        }
        out.push('\n');

        out.push_str("## Context\n");
        out.push_str(self.context.trim());
        out.push_str("\n\n");

        out.push_str("## Decision\n");
        out.push_str(self.decision.trim());
        out.push_str("\n\n");

        out.push_str("## Rationale\n");
        out.push_str(self.rationale.trim());
        out.push_str("\n\n");

        out.push_str("## Alternatives Considered\n");
        if self.alternatives_considered.is_empty() {
            out.push_str("- None documented\n\n");
        } else {
            for alt in &self.alternatives_considered {
                out.push_str(&format!("- {}\n", alt));
            }
            out.push('\n');
        }

        out.push_str("## Consequences\n");
        if self.consequences.is_empty() {
            out.push_str("- None documented\n\n");
        } else {
            for c in &self.consequences {
                out.push_str(&format!("- {}\n", c));
            }
            out.push('\n');
        }

        out.push_str("## Residual Risks\n");
        if self.residual_risks.is_empty() {
            out.push_str("- None identified\n\n");
        } else {
            for r in &self.residual_risks {
                out.push_str(&format!("- {}\n", r));
            }
            out.push('\n');
        }

        out
    }
}

/// Registry managing all project ADRs with deterministic ordering.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct AdrRegistry {
    pub adrs: BTreeMap<AdrId, ArchitectureDecisionRecord>,
}

impl AdrRegistry {
    pub fn new() -> Self {
        Self {
            adrs: BTreeMap::new(),
        }
    }

    /// Register a new ADR, rejecting duplicate IDs.
    pub fn register(&mut self, adr: ArchitectureDecisionRecord) -> Result<(), WorkflowError> {
        if self.adrs.contains_key(&adr.id) {
            return Err(WorkflowError::ManifestValidation {
                path: None,
                workflow_id: None,
                step_key: None,
                field: Some("adrs".to_string()),
                reason: format!("Duplicate ADR ID '{}'", adr.id),
            });
        }
        self.adrs.insert(adr.id.clone(), adr);
        Ok(())
    }

    pub fn get(&self, id: &AdrId) -> Option<&ArchitectureDecisionRecord> {
        self.adrs.get(id)
    }

    pub fn get_mut(&mut self, id: &AdrId) -> Option<&mut ArchitectureDecisionRecord> {
        self.adrs.get_mut(id)
    }

    /// Accept an ADR via explicit operator or runtime authorization.
    pub fn accept(&mut self, id: &AdrId, operator: &str) -> Result<(), WorkflowError> {
        let adr = self
            .adrs
            .get_mut(id)
            .ok_or_else(|| WorkflowError::ManifestValidation {
                path: None,
                workflow_id: None,
                step_key: None,
                field: Some("adrs".to_string()),
                reason: format!("ADR '{}' not found", id),
            })?;

        if !adr.status.can_transition_to(DecisionStatus::Accepted) {
            return Err(WorkflowError::ManifestValidation {
                path: None,
                workflow_id: None,
                step_key: None,
                field: Some("adrs".to_string()),
                reason: format!(
                    "Cannot transition ADR '{}' from status '{}' to 'accepted'",
                    id, adr.status
                ),
            });
        }

        adr.status = DecisionStatus::Accepted;
        adr.provenance.actor = operator.to_string();
        Ok(())
    }

    /// Reject an ADR.
    pub fn reject(
        &mut self,
        id: &AdrId,
        reason: &str,
        operator: &str,
    ) -> Result<(), WorkflowError> {
        let adr = self
            .adrs
            .get_mut(id)
            .ok_or_else(|| WorkflowError::ManifestValidation {
                path: None,
                workflow_id: None,
                step_key: None,
                field: Some("adrs".to_string()),
                reason: format!("ADR '{}' not found", id),
            })?;

        if !adr.status.can_transition_to(DecisionStatus::Rejected) {
            return Err(WorkflowError::ManifestValidation {
                path: None,
                workflow_id: None,
                step_key: None,
                field: Some("adrs".to_string()),
                reason: format!(
                    "Cannot transition ADR '{}' from status '{}' to 'rejected'",
                    id, adr.status
                ),
            });
        }

        adr.status = DecisionStatus::Rejected;
        adr.provenance.actor = operator.to_string();
        adr.provenance.reason = Some(reason.to_string());
        Ok(())
    }

    /// Supersede an existing ADR with a newly registered ADR.
    pub fn supersede(
        &mut self,
        old_id: &AdrId,
        mut new_adr: ArchitectureDecisionRecord,
    ) -> Result<(), WorkflowError> {
        if !self.adrs.contains_key(old_id) {
            return Err(WorkflowError::ManifestValidation {
                path: None,
                workflow_id: None,
                step_key: None,
                field: Some("adrs".to_string()),
                reason: format!("ADR '{}' to supersede not found", old_id),
            });
        }

        // Insert new ADR
        let new_id = new_adr.id.clone();
        new_adr.version = self.adrs[old_id].version + 1;
        self.register(new_adr)?;

        // Update old ADR
        let old = self.adrs.get_mut(old_id).unwrap();
        old.status = DecisionStatus::Superseded;
        old.superseded_by = Some(new_id);

        Ok(())
    }

    /// Render consolidated DECISIONS.md summary document.
    pub fn to_decisions_markdown(&self) -> String {
        let mut out = String::new();
        out.push_str("# Architectural Decisions (ADRs)\n\n");

        if self.adrs.is_empty() {
            out.push_str("No architectural decision records registered.\n");
            return out;
        }

        out.push_str("| ADR ID | Title | Status | Requirements | Superseded By |\n");
        out.push_str("|---|---|---|---|---|\n");

        for adr in self.adrs.values() {
            let reqs = if adr.linked_requirements.is_empty() {
                "-".to_string()
            } else {
                adr.linked_requirements
                    .iter()
                    .map(|r| format!("`{}`", r))
                    .collect::<Vec<_>>()
                    .join(", ")
            };
            let sup = adr
                .superseded_by
                .as_ref()
                .map(|s| format!("`{}`", s))
                .unwrap_or_else(|| "-".to_string());
            out.push_str(&format!(
                "| [`{}`](adr/{}.md) | {} | `{}` | {} | {} |\n",
                adr.id, adr.id, adr.title, adr.status, reqs, sup
            ));
        }

        out.push('\n');
        for adr in self.adrs.values() {
            out.push_str(&format!("## {}: {}\n\n", adr.id, adr.title));
            out.push_str(&format!("- **Status**: `{}`\n", adr.status));
            out.push_str(&format!("- **Decision**: {}\n", adr.decision.trim()));
            out.push_str(&format!("- **Rationale**: {}\n", adr.rationale.trim()));
            if !adr.consequences.is_empty() {
                out.push_str("- **Consequences**:\n");
                for c in &adr.consequences {
                    out.push_str(&format!("  - {}\n", c));
                }
            }
            out.push('\n');
        }

        out
    }

    /// Save DECISIONS.md and individual ADR files to `.planning/DECISIONS.md` and `.planning/adr/*.md`.
    pub fn save_to_dir(
        &self,
        workspace_root: &Path,
        projection_dir: &str,
    ) -> Result<PathBuf, WorkflowError> {
        let dir = workspace_root.join(projection_dir);
        let adr_dir = dir.join("adr");
        std::fs::create_dir_all(&adr_dir).map_err(|e| WorkflowError::ManifestValidation {
            path: Some(adr_dir.display().to_string()),
            workflow_id: None,
            step_key: None,
            field: None,
            reason: format!("failed to create adr directory: {}", e),
        })?;

        // 1. Write individual ADR files
        for adr in self.adrs.values() {
            let file_name = format!("{}.md", adr.id);
            let file_path = adr_dir.join(file_name);
            std::fs::write(&file_path, adr.to_markdown()).map_err(|e| {
                WorkflowError::ManifestValidation {
                    path: Some(file_path.display().to_string()),
                    workflow_id: None,
                    step_key: None,
                    field: None,
                    reason: format!("failed to write ADR {}: {}", adr.id, e),
                }
            })?;
        }

        // 2. Write master DECISIONS.md
        let decisions_path = dir.join("DECISIONS.md");
        std::fs::write(&decisions_path, self.to_decisions_markdown()).map_err(|e| {
            WorkflowError::ManifestValidation {
                path: Some(decisions_path.display().to_string()),
                workflow_id: None,
                step_key: None,
                field: None,
                reason: format!("failed to write DECISIONS.md: {}", e),
            }
        })?;

        Ok(decisions_path)
    }
}
