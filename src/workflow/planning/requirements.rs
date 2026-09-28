//! Authoritative requirements document model and Markdown projections.
//!
//! Re-exports and extends M31A's authoritative `EngineeringRequirement` domain model.

use crate::planning::requirements::{
    EngineeringRequirement, EpistemicStatus, RequirementCategory, RequirementKey,
    RequirementPriority,
};
use crate::workflow::error::WorkflowError;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

// Re-export core requirements domain types for callers
pub use crate::planning::requirements::{
    EngineeringRequirement as DomainEngineeringRequirement,
    EpistemicRevision as DomainEpistemicRevision, EpistemicStatus as DomainEpistemicStatus,
    Provenance as DomainProvenance, ProvenanceSourceType as DomainProvenanceSourceType,
    RequirementCategory as DomainRequirementCategory, RequirementKey as DomainRequirementKey,
    RequirementPriority as DomainRequirementPriority, TrustLevel as DomainTrustLevel,
};

/// Complete, validated requirements specification document.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RequirementsDocument {
    pub project_name: String,
    pub scope: String,
    pub requirements: Vec<EngineeringRequirement>,
    pub unknown_requirements: Vec<String>,
    pub requirement_decisions: Vec<String>,
    pub unresolved_questions: Vec<String>,
    pub version: u32,
}

impl RequirementsDocument {
    pub fn new(project_name: impl Into<String>, scope: impl Into<String>) -> Self {
        Self {
            project_name: project_name.into(),
            scope: scope.into(),
            requirements: Vec::new(),
            unknown_requirements: Vec::new(),
            requirement_decisions: Vec::new(),
            unresolved_questions: Vec::new(),
            version: 1,
        }
    }

    /// Sort requirements deterministically by RequirementKey.
    pub fn sort_deterministic(&mut self) {
        self.requirements.sort_by(|a, b| a.key.cmp(&b.key));
        self.unknown_requirements.sort();
        self.requirement_decisions.sort();
        self.unresolved_questions.sort();
    }

    /// Filter requirements by category.
    pub fn by_category(&self, category: RequirementCategory) -> Vec<&EngineeringRequirement> {
        self.requirements
            .iter()
            .filter(|r| r.category == category)
            .collect()
    }

    /// Find a requirement by key.
    pub fn get(&self, key: &RequirementKey) -> Option<&EngineeringRequirement> {
        self.requirements.iter().find(|r| &r.key == key)
    }

    /// Add a requirement ensuring key uniqueness.
    pub fn add_requirement(&mut self, req: EngineeringRequirement) -> Result<(), WorkflowError> {
        if self.requirements.iter().any(|r| r.key == req.key) {
            return Err(WorkflowError::ManifestValidation {
                path: None,
                workflow_id: None,
                step_key: None,
                field: Some("requirements".to_string()),
                reason: format!("Duplicate requirement key '{}'", req.key),
            });
        }
        self.requirements.push(req);
        Ok(())
    }

    /// Add or update an existing requirement by key.
    pub fn add_or_update(&mut self, req: EngineeringRequirement) {
        if let Some(existing) = self.requirements.iter_mut().find(|r| r.key == req.key) {
            *existing = req;
        } else {
            self.requirements.push(req);
        }
    }

    /// Render canonical Markdown projection into REQUIREMENTS.md format.
    pub fn to_markdown(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!("# Requirements: {}\n\n", self.project_name));

        out.push_str("## Scope\n");
        out.push_str(self.scope.trim());
        out.push_str("\n\n");

        // Helper to format requirement list
        let render_req_group = |buf: &mut String, title: &str, reqs: &[&EngineeringRequirement]| {
            buf.push_str(&format!("## {}\n", title));
            if reqs.is_empty() {
                buf.push_str("None registered.\n\n");
                return;
            }
            buf.push_str("| ID | Title | Priority | Status | Rationale |\n");
            buf.push_str("|---|---|---|---|---|\n");
            for r in reqs {
                let title = r.title.as_deref().unwrap_or(r.statement());
                let rationale = r.rationale.as_deref().unwrap_or("-");
                buf.push_str(&format!(
                    "| `{}` | {} | `{}` | `{}` | {} |\n",
                    r.key, title, r.priority, r.current_status, rationale
                ));
            }
            buf.push('\n');
        };

        render_req_group(
            &mut out,
            "Functional Requirements",
            &self.by_category(RequirementCategory::Functional),
        );
        render_req_group(
            &mut out,
            "Non-Functional Requirements",
            &self.by_category(RequirementCategory::NonFunctional),
        );
        render_req_group(
            &mut out,
            "Security Requirements",
            &self.by_category(RequirementCategory::Security),
        );
        render_req_group(
            &mut out,
            "Operational Requirements",
            &self.by_category(RequirementCategory::Operational),
        );
        render_req_group(
            &mut out,
            "Compatibility Requirements",
            &self.by_category(RequirementCategory::Compatibility),
        );

        // Acceptance Criteria
        out.push_str("## Acceptance Criteria\n");
        let with_criteria: Vec<_> = self
            .requirements
            .iter()
            .filter(|r| !r.satisfaction_criteria.is_empty())
            .collect();
        if with_criteria.is_empty() {
            out.push_str("No explicit acceptance criteria specified.\n\n");
        } else {
            for r in with_criteria {
                out.push_str(&format!("### Criteria for `{}`\n", r.key));
                for c in &r.satisfaction_criteria {
                    out.push_str(&format!("- {}\n", c));
                }
                out.push('\n');
            }
        }

        // Traceability Summary
        out.push_str("## Traceability\n");
        out.push_str("| Requirement ID | Category | Source Location | Dependencies |\n");
        out.push_str("|---|---|---|---|\n");
        for r in &self.requirements {
            let loc = r.provenance.location.as_deref().unwrap_or("-");
            let deps = if r.dependencies.is_empty() {
                "-".to_string()
            } else {
                r.dependencies
                    .iter()
                    .map(|d| d.to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            };
            out.push_str(&format!(
                "| `{}` | `{}` | {} | {} |\n",
                r.key, r.category, loc, deps
            ));
        }
        out.push('\n');

        // Deferred / Unknown Requirements
        out.push_str("## Deferred / Unknown Requirements\n");
        let deferred: Vec<_> = self
            .requirements
            .iter()
            .filter(|r| {
                r.priority == RequirementPriority::Deferred
                    || r.current_status == EpistemicStatus::Unknown
            })
            .collect();
        if deferred.is_empty() && self.unknown_requirements.is_empty() {
            out.push_str("None deferred or marked unknown.\n\n");
        } else {
            for r in deferred {
                out.push_str(&format!(
                    "- `{}`: {} (Status: `{}`, Priority: `{}`)\n",
                    r.key,
                    r.title.as_deref().unwrap_or(r.statement()),
                    r.current_status,
                    r.priority
                ));
            }
            for u in &self.unknown_requirements {
                out.push_str(&format!("- Unknown: {}\n", u));
            }
            out.push('\n');
        }

        // Requirement Decisions
        out.push_str("## Requirement Decisions\n");
        if self.requirement_decisions.is_empty() {
            out.push_str("No custom requirement decisions recorded.\n\n");
        } else {
            for d in &self.requirement_decisions {
                out.push_str(&format!("- {}\n", d));
            }
            out.push('\n');
        }

        out
    }

    /// Persist to `<workspace_root>/<projection_dir>/REQUIREMENTS.md`.
    pub fn save_to_dir(
        &self,
        workspace_root: &Path,
        projection_dir: &str,
    ) -> Result<PathBuf, WorkflowError> {
        let dir = workspace_root.join(projection_dir);
        std::fs::create_dir_all(&dir).map_err(|e| WorkflowError::ManifestValidation {
            path: Some(dir.display().to_string()),
            workflow_id: None,
            step_key: None,
            field: None,
            reason: format!("failed to create planning directory: {}", e),
        })?;
        let file_path = dir.join("REQUIREMENTS.md");
        std::fs::write(&file_path, self.to_markdown()).map_err(|e| {
            WorkflowError::ManifestValidation {
                path: Some(file_path.display().to_string()),
                workflow_id: None,
                step_key: None,
                field: None,
                reason: format!("failed to write REQUIREMENTS.md: {}", e),
            }
        })?;
        Ok(file_path)
    }
}
