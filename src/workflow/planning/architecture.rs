//! Architecture models, component topologies, security trust boundaries, and ARCHITECTURE.md projection.

use crate::planning::requirements::RequirementKey;
use crate::workflow::error::WorkflowError;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::{Path, PathBuf};

/// Status of an architecture component, supporting brownfield existing/modified/new differentiation (Section 38).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Default,
)]
#[serde(rename_all = "snake_case")]
pub enum ComponentStatus {
    Existing,
    Modified,
    #[default]
    New,
    Removed,
}

impl fmt::Display for ComponentStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Existing => write!(f, "existing"),
            Self::Modified => write!(f, "modified"),
            Self::New => write!(f, "new"),
            Self::Removed => write!(f, "removed"),
        }
    }
}

/// A discrete architectural component with explicit responsibilities and dependencies (Section 15).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArchitectureComponent {
    pub id: String,
    pub name: String,
    pub subsystem: String,
    pub responsibility: String,
    pub depends_on: Vec<String>,
    pub requirement_refs: Vec<RequirementKey>,
    pub status: ComponentStatus,
}

impl ArchitectureComponent {
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        subsystem: impl Into<String>,
        responsibility: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            subsystem: subsystem.into(),
            responsibility: responsibility.into(),
            depends_on: Vec::new(),
            requirement_refs: Vec::new(),
            status: ComponentStatus::New,
        }
    }

    pub fn with_depends_on(mut self, depends_on: Vec<String>) -> Self {
        self.depends_on = depends_on;
        self
    }

    pub fn with_requirements(mut self, requirements: Vec<RequirementKey>) -> Self {
        self.requirement_refs = requirements;
        self
    }

    pub fn with_status(mut self, status: ComponentStatus) -> Self {
        self.status = status;
        self
    }
}

/// Higher-level grouping of related components.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArchitectureSubsystem {
    pub id: String,
    pub name: String,
    pub description: String,
    pub components: Vec<String>,
}

impl ArchitectureSubsystem {
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        description: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            description: description.into(),
            components: Vec::new(),
        }
    }
}

/// Interface contract between components.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArchitectureInterface {
    pub id: String,
    pub name: String,
    pub provider_component: String,
    pub consumer_components: Vec<String>,
    pub contract_type: String,
    pub description: String,
}

impl ArchitectureInterface {
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        provider: impl Into<String>,
        contract_type: impl Into<String>,
        description: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            provider_component: provider.into(),
            consumer_components: Vec::new(),
            contract_type: contract_type.into(),
            description: description.into(),
        }
    }
}

/// Data store entity.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArchitectureDataStore {
    pub id: String,
    pub name: String,
    pub store_type: String,
    pub persistence_guarantees: String,
    pub owning_component: String,
}

/// Security trust boundary (Section 20).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TrustBoundary {
    pub id: String,
    pub name: String,
    pub description: String,
    pub inside_components: Vec<String>,
    pub outside_components: Vec<String>,
    pub boundary_controls: Vec<String>,
    pub authentication_required: bool,
    pub authorization_rules: Vec<String>,
}

/// Deployment boundary and topology specification.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeploymentBoundary {
    pub id: String,
    pub name: String,
    pub target: String,
    pub resource_bounds: Option<String>,
    pub components: Vec<String>,
}

/// Consolidated system architecture specification document.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArchitectureDocument {
    pub project_name: String,
    pub architectural_goals: Vec<String>,
    pub system_context: String,
    pub subsystems: Vec<ArchitectureSubsystem>,
    pub components: Vec<ArchitectureComponent>,
    pub interfaces: Vec<ArchitectureInterface>,
    pub data_stores: Vec<ArchitectureDataStore>,
    pub trust_boundaries: Vec<TrustBoundary>,
    pub deployment_boundaries: Vec<DeploymentBoundary>,
    pub data_flow: Vec<String>,
    pub persistence_model: String,
    pub observability_considerations: Vec<String>,
    pub scalability_considerations: Vec<String>,
    pub failure_recovery_model: String,
    pub architectural_invariants: Vec<String>,
    pub unresolved_design_questions: Vec<String>,
    pub adr_refs: Vec<String>,
    pub version: u32,
}

impl ArchitectureDocument {
    pub fn new(project_name: impl Into<String>, system_context: impl Into<String>) -> Self {
        Self {
            project_name: project_name.into(),
            architectural_goals: Vec::new(),
            system_context: system_context.into(),
            subsystems: Vec::new(),
            components: Vec::new(),
            interfaces: Vec::new(),
            data_stores: Vec::new(),
            trust_boundaries: Vec::new(),
            deployment_boundaries: Vec::new(),
            data_flow: Vec::new(),
            persistence_model: String::new(),
            observability_considerations: Vec::new(),
            scalability_considerations: Vec::new(),
            failure_recovery_model: String::new(),
            architectural_invariants: Vec::new(),
            unresolved_design_questions: Vec::new(),
            adr_refs: Vec::new(),
            version: 1,
        }
    }

    /// Sort components and subsystems deterministically.
    pub fn sort_deterministic(&mut self) {
        self.subsystems.sort_by(|a, b| a.id.cmp(&b.id));
        self.components.sort_by(|a, b| a.id.cmp(&b.id));
        self.interfaces.sort_by(|a, b| a.id.cmp(&b.id));
        self.data_stores.sort_by(|a, b| a.id.cmp(&b.id));
        self.trust_boundaries.sort_by(|a, b| a.id.cmp(&b.id));
        self.deployment_boundaries.sort_by(|a, b| a.id.cmp(&b.id));
        self.architectural_goals.sort();
        self.data_flow.sort();
        self.observability_considerations.sort();
        self.scalability_considerations.sort();
        self.architectural_invariants.sort();
        self.unresolved_design_questions.sort();
        self.adr_refs.sort();
    }

    /// Add a component ensuring unique ID.
    pub fn add_component(&mut self, comp: ArchitectureComponent) -> Result<(), WorkflowError> {
        if self.components.iter().any(|c| c.id == comp.id) {
            return Err(WorkflowError::ManifestValidation {
                path: None,
                workflow_id: None,
                step_key: None,
                field: Some("components".to_string()),
                reason: format!("Duplicate component ID '{}'", comp.id),
            });
        }
        self.components.push(comp);
        Ok(())
    }

    /// Render canonical ARCHITECTURE.md format (Section 16).
    pub fn to_markdown(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!("# System Architecture: {}\n\n", self.project_name));

        // Architectural Goals
        out.push_str("## Architectural Goals\n");
        if self.architectural_goals.is_empty() {
            out.push_str(
                "- Establish robust, modular architecture adhering to runtime invariants.\n\n",
            );
        } else {
            for g in &self.architectural_goals {
                out.push_str(&format!("- {}\n", g));
            }
            out.push('\n');
        }

        // System Context
        out.push_str("## System Context\n");
        out.push_str(self.system_context.trim());
        out.push_str("\n\n");

        // Subsystem Boundaries
        out.push_str("## Subsystem Boundaries\n");
        if self.subsystems.is_empty() {
            out.push_str("Unified subsystem architecture.\n\n");
        } else {
            for sub in &self.subsystems {
                out.push_str(&format!("### {}\n", sub.name));
                out.push_str(&format!("- **ID**: `{}`\n", sub.id));
                out.push_str(&format!("- **Description**: {}\n", sub.description));
                let comps = if sub.components.is_empty() {
                    "-".to_string()
                } else {
                    sub.components
                        .iter()
                        .map(|c| format!("`{}`", c))
                        .collect::<Vec<_>>()
                        .join(", ")
                };
                out.push_str(&format!("- **Components**: {}\n\n", comps));
            }
        }

        // Component Responsibilities & Dependencies
        out.push_str("## Component Model\n");
        out.push_str("| ID | Name | Subsystem | Status | Depends On | Requirements |\n");
        out.push_str("|---|---|---|---|---|---|\n");
        for comp in &self.components {
            let deps = if comp.depends_on.is_empty() {
                "-".to_string()
            } else {
                comp.depends_on
                    .iter()
                    .map(|d| format!("`{}`", d))
                    .collect::<Vec<_>>()
                    .join(", ")
            };
            let reqs = if comp.requirement_refs.is_empty() {
                "-".to_string()
            } else {
                comp.requirement_refs
                    .iter()
                    .map(|r| format!("`{}`", r))
                    .collect::<Vec<_>>()
                    .join(", ")
            };
            out.push_str(&format!(
                "| `{}` | {} | `{}` | `{}` | {} | {} |\n",
                comp.id, comp.name, comp.subsystem, comp.status, deps, reqs
            ));
        }
        out.push('\n');

        // Detailed responsibilities
        for comp in &self.components {
            out.push_str(&format!("### `{}` — {}\n", comp.id, comp.name));
            out.push_str(&format!("- **Responsibility**: {}\n", comp.responsibility));
            out.push_str(&format!("- **Lifecycle Status**: `{}`\n\n", comp.status));
        }

        // Interface Contracts
        out.push_str("## Interface Contracts\n");
        if self.interfaces.is_empty() {
            out.push_str("Interfaces defined within component boundaries.\n\n");
        } else {
            out.push_str("| Interface ID | Provider | Contract Type | Description |\n");
            out.push_str("|---|---|---|---|\n");
            for iface in &self.interfaces {
                out.push_str(&format!(
                    "| `{}` | `{}` | `{}` | {} |\n",
                    iface.id, iface.provider_component, iface.contract_type, iface.description
                ));
            }
            out.push('\n');
        }

        // Data Flow & Persistence
        out.push_str("## Data Flow & Persistence Model\n");
        if !self.persistence_model.is_empty() {
            out.push_str(&format!(
                "- **Persistence Model**: {}\n",
                self.persistence_model
            ));
        }
        if !self.data_stores.is_empty() {
            out.push_str("\n### Data Stores\n");
            for ds in &self.data_stores {
                out.push_str(&format!(
                    "- **`{}`** ({}): Owning Component `{}`. Guarantees: {}\n",
                    ds.id, ds.store_type, ds.owning_component, ds.persistence_guarantees
                ));
            }
        }
        if !self.data_flow.is_empty() {
            out.push_str("\n### Data Flows\n");
            for df in &self.data_flow {
                out.push_str(&format!("- {}\n", df));
            }
        }
        out.push('\n');

        // Security Boundaries (Section 20)
        out.push_str("## Security Architecture & Trust Boundaries\n");
        if self.trust_boundaries.is_empty() {
            out.push_str("Standard unified process trust domain.\n\n");
        } else {
            for tb in &self.trust_boundaries {
                out.push_str(&format!("### Trust Boundary: {}\n", tb.name));
                out.push_str(&format!("- **ID**: `{}`\n", tb.id));
                out.push_str(&format!("- **Description**: {}\n", tb.description));
                out.push_str(&format!(
                    "- **Authentication Required**: {}\n",
                    tb.authentication_required
                ));
                out.push_str(&format!(
                    "- **Inside Components**: {}\n",
                    tb.inside_components.join(", ")
                ));
                out.push_str(&format!(
                    "- **Outside Components**: {}\n",
                    tb.outside_components.join(", ")
                ));
                if !tb.boundary_controls.is_empty() {
                    out.push_str("- **Controls**:\n");
                    for c in &tb.boundary_controls {
                        out.push_str(&format!("  - {}\n", c));
                    }
                }
                out.push('\n');
            }
        }

        // Deployment Topology
        out.push_str("## Deployment Topology\n");
        if self.deployment_boundaries.is_empty() {
            out.push_str("Single binary host deployment target.\n\n");
        } else {
            for db in &self.deployment_boundaries {
                out.push_str(&format!("### Target: {}\n", db.name));
                out.push_str(&format!("- **Packaging/Target**: `{}`\n", db.target));
                if let Some(ref rb) = db.resource_bounds {
                    out.push_str(&format!("- **Resource Bounds**: {}\n", rb));
                }
                out.push_str(&format!(
                    "- **Hosted Components**: {}\n\n",
                    db.components.join(", ")
                ));
            }
        }

        // Failure & Recovery Model
        out.push_str("## Failure & Recovery Model\n");
        if self.failure_recovery_model.is_empty() {
            out.push_str("Fail-fast with bounded retries and crash recovery checkpoints.\n\n");
        } else {
            out.push_str(&format!("{}\n\n", self.failure_recovery_model.trim()));
        }

        // Architectural Invariants
        out.push_str("## Architectural Invariants\n");
        if self.architectural_invariants.is_empty() {
            out.push_str("- The model proposes. The runtime decides.\n\n");
        } else {
            for inv in &self.architectural_invariants {
                out.push_str(&format!("- {}\n", inv));
            }
            out.push('\n');
        }

        // Unresolved Questions
        out.push_str("## Unresolved Design Questions\n");
        if self.unresolved_design_questions.is_empty() {
            out.push_str("None identified.\n\n");
        } else {
            for q in &self.unresolved_design_questions {
                out.push_str(&format!("- {}\n", q));
            }
            out.push('\n');
        }

        // ADR References
        out.push_str("## Associated ADRs\n");
        if self.adr_refs.is_empty() {
            out.push_str("See [DECISIONS.md](DECISIONS.md) for architectural decision records.\n");
        } else {
            for adr in &self.adr_refs {
                out.push_str(&format!("- [`{}`](adr/{}.md)\n", adr, adr));
            }
        }

        out
    }

    /// Persist to `<workspace_root>/<projection_dir>/ARCHITECTURE.md`.
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
        let file_path = dir.join("ARCHITECTURE.md");
        std::fs::write(&file_path, self.to_markdown()).map_err(|e| {
            WorkflowError::ManifestValidation {
                path: Some(file_path.display().to_string()),
                workflow_id: None,
                step_key: None,
                field: None,
                reason: format!("failed to write ARCHITECTURE.md: {}", e),
            }
        })?;
        Ok(file_path)
    }
}
