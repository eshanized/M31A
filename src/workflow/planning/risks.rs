//! Risk register, assumptions, unknowns, and RISKS.md projection.

use crate::planning::requirements::{Provenance, RequirementKey};
pub use crate::planning::risks::{
    Criticality, PlanningAssumption, PlanningRisk, PlanningUnknown, UnknownFate,
};
use crate::workflow::error::WorkflowError;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::{Path, PathBuf};

/// Qualitative likelihood rating (Section 21).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Default,
)]
#[serde(rename_all = "snake_case")]
pub enum QualitativeLikelihood {
    #[default]
    Low,
    Medium,
    High,
}

impl fmt::Display for QualitativeLikelihood {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Low => write!(f, "low"),
            Self::Medium => write!(f, "medium"),
            Self::High => write!(f, "high"),
        }
    }
}

/// Qualitative impact rating (Section 21).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Default,
)]
#[serde(rename_all = "snake_case")]
pub enum QualitativeImpact {
    Low,
    #[default]
    Medium,
    High,
    Critical,
}

impl fmt::Display for QualitativeImpact {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Low => write!(f, "low"),
            Self::Medium => write!(f, "medium"),
            Self::High => write!(f, "high"),
            Self::Critical => write!(f, "critical"),
        }
    }
}

/// Status of a risk entry.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Default,
)]
#[serde(rename_all = "snake_case")]
pub enum RiskStatus {
    #[default]
    Open,
    Mitigated,
    Accepted,
    Transferred,
}

impl fmt::Display for RiskStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Open => write!(f, "open"),
            Self::Mitigated => write!(f, "mitigated"),
            Self::Accepted => write!(f, "accepted"),
            Self::Transferred => write!(f, "transferred"),
        }
    }
}

/// A structured entry in the risk register adhering to Section 21.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RiskEntry {
    pub id: String,
    pub description: String,
    pub source: String,
    pub likelihood: QualitativeLikelihood,
    pub impact: QualitativeImpact,
    pub affected_requirements: Vec<RequirementKey>,
    pub affected_components: Vec<String>,
    pub mitigation: String,
    pub verification_approach: String,
    pub status: RiskStatus,
    pub provenance: Provenance,
}

impl RiskEntry {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: impl Into<String>,
        description: impl Into<String>,
        source: impl Into<String>,
        likelihood: QualitativeLikelihood,
        impact: QualitativeImpact,
        mitigation: impl Into<String>,
        verification_approach: impl Into<String>,
        provenance: Provenance,
    ) -> Self {
        Self {
            id: id.into(),
            description: description.into(),
            source: source.into(),
            likelihood,
            impact,
            affected_requirements: Vec::new(),
            affected_components: Vec::new(),
            mitigation: mitigation.into(),
            verification_approach: verification_approach.into(),
            status: RiskStatus::Open,
            provenance,
        }
    }

    pub fn with_requirements(mut self, reqs: Vec<RequirementKey>) -> Self {
        self.affected_requirements = reqs;
        self
    }

    pub fn with_components(mut self, comps: Vec<String>) -> Self {
        self.affected_components = comps;
        self
    }
}

/// Consolidated Risk Register containing risks, assumptions, and unknowns.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct RiskRegister {
    pub risks: Vec<RiskEntry>,
    pub assumptions: Vec<PlanningAssumption>,
    pub unknowns: Vec<PlanningUnknown>,
}

impl RiskRegister {
    pub fn new() -> Self {
        Self {
            risks: Vec::new(),
            assumptions: Vec::new(),
            unknowns: Vec::new(),
        }
    }

    pub fn add_risk(&mut self, risk: RiskEntry) {
        self.risks.push(risk);
    }

    pub fn add_assumption(&mut self, assumption: PlanningAssumption) {
        self.assumptions.push(assumption);
    }

    pub fn add_unknown(&mut self, unknown: PlanningUnknown) {
        self.unknowns.push(unknown);
    }

    /// Sort items deterministically.
    pub fn sort_deterministic(&mut self) {
        self.risks.sort_by(|a, b| a.id.cmp(&b.id));
        self.assumptions.sort_by(|a, b| a.id.cmp(&b.id));
        self.unknowns.sort_by(|a, b| a.id.cmp(&b.id));
    }

    /// Render canonical RISKS.md format.
    pub fn to_markdown(&self) -> String {
        let mut out = String::new();
        out.push_str("# Project Risk Register\n\n");

        out.push_str("## Identified Risks\n");
        if self.risks.is_empty() {
            out.push_str("No active risks recorded.\n\n");
        } else {
            out.push_str(
                "| ID | Description | Likelihood | Impact | Status | Mitigation | Verification |\n",
            );
            out.push_str("|---|---|---|---|---|---|---|\n");
            for r in &self.risks {
                out.push_str(&format!(
                    "| `{}` | {} | `{}` | `{}` | `{}` | {} | {} |\n",
                    r.id,
                    r.description,
                    r.likelihood,
                    r.impact,
                    r.status,
                    r.mitigation,
                    r.verification_approach
                ));
            }
            out.push('\n');

            for r in &self.risks {
                out.push_str(&format!("### `{}` — {}\n", r.id, r.description));
                out.push_str(&format!("- **Source**: {}\n", r.source));
                out.push_str(&format!("- **Likelihood**: `{}`\n", r.likelihood));
                out.push_str(&format!("- **Impact**: `{}`\n", r.impact));
                if !r.affected_requirements.is_empty() {
                    let req_str = r
                        .affected_requirements
                        .iter()
                        .map(|k| format!("`{}`", k))
                        .collect::<Vec<_>>()
                        .join(", ");
                    out.push_str(&format!("- **Affected Requirements**: {}\n", req_str));
                }
                if !r.affected_components.is_empty() {
                    let comp_str = r
                        .affected_components
                        .iter()
                        .map(|c| format!("`{}`", c))
                        .collect::<Vec<_>>()
                        .join(", ");
                    out.push_str(&format!("- **Affected Components**: {}\n", comp_str));
                }
                out.push_str(&format!("- **Mitigation Strategy**: {}\n", r.mitigation));
                out.push_str(&format!(
                    "- **Verification Approach**: {}\n\n",
                    r.verification_approach
                ));
            }
        }

        out.push_str("## Planning Assumptions\n");
        if self.assumptions.is_empty() {
            out.push_str("No explicit assumptions recorded.\n\n");
        } else {
            for a in &self.assumptions {
                let strat = a
                    .resolution_strategy
                    .as_deref()
                    .unwrap_or("Verify during execution");
                out.push_str(&format!(
                    "- **`{}`** (Criticality: `{}`): {} (Resolution: {})\n",
                    a.id, a.criticality, a.description, strat
                ));
            }
            out.push('\n');
        }

        out.push_str("## Unknowns & Investigation Plans\n");
        if self.unknowns.is_empty() {
            out.push_str("No unresolved unknowns recorded.\n\n");
        } else {
            for u in &self.unknowns {
                let plan = u
                    .discovery_plan
                    .as_deref()
                    .unwrap_or("Investigate during phase execution");
                out.push_str(&format!(
                    "- **`{}`** (Criticality: `{}`): {} (Plan: {})\n",
                    u.id, u.criticality, u.description, plan
                ));
            }
            out.push('\n');
        }

        out
    }

    /// Persist to `<workspace_root>/<projection_dir>/RISKS.md`.
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
        let file_path = dir.join("RISKS.md");
        std::fs::write(&file_path, self.to_markdown()).map_err(|e| {
            WorkflowError::ManifestValidation {
                path: Some(file_path.display().to_string()),
                workflow_id: None,
                step_key: None,
                field: None,
                reason: format!("failed to write RISKS.md: {}", e),
            }
        })?;
        Ok(file_path)
    }
}
