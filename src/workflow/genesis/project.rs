//! Project Charter domain model, ambiguity assessment, and PROJECT.md markdown projection.

use super::errors::GenesisError;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Boundaries and scope restrictions for a project.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectBoundaries {
    pub in_scope: Vec<String>,
    pub non_goals: Vec<String>,
    pub anti_features: Vec<String>,
}

/// Technical preferences and architectural constraints.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TechnicalPreferences {
    pub languages: Vec<String>,
    pub storage: Option<String>,
    pub deployment_target: Option<String>,
    pub architecture_style: Option<String>,
    pub frameworks: Vec<String>,
}

/// Operational and production invariants.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct OperationalInvariants {
    pub scale_targets: Vec<String>,
    pub security_requirements: Vec<String>,
    pub licensing: Option<String>,
    pub performance_targets: Vec<String>,
}

/// Epistemic ambiguity assessment for a project concept.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AmbiguityAssessment {
    /// Ambiguity percentage (0 = complete certainty, 100 = total ambiguity).
    pub score_percent: u8,
    /// Specific domains or questions remaining unresolved.
    pub unresolved_areas: Vec<String>,
    /// Invariants that have been definitively resolved.
    pub resolved_invariants: Vec<String>,
    /// Whether the score is at or below the convergence threshold.
    pub converged: bool,
}

impl Default for AmbiguityAssessment {
    fn default() -> Self {
        Self {
            score_percent: 100,
            unresolved_areas: vec![
                "Target personas and primary problem definition".to_string(),
                "Scope boundaries and non-goals".to_string(),
                "Technical architecture and stack choices".to_string(),
                "Operational and security invariants".to_string(),
            ],
            resolved_invariants: Vec::new(),
            converged: false,
        }
    }
}

/// Granular workflow complexity tier for adaptive upstream depth.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowTier {
    /// Localized typo, doc, or single-token edit. Fast-path, bypasses greenfield ceremony.
    Tiny,
    /// Localized feature, test addition, or bugfix in existing codebase.
    Medium,
    /// Standard product or project creation with full domain synthesis.
    #[default]
    Standard,
    /// Major architectural shift, multi-tenancy/SaaS conversion, high-risk mutation.
    Consequential,
    /// Deprecated alias for Standard.
    #[serde(rename = "greenfield")]
    Greenfield,
}

impl PartialEq for WorkflowTier {
    fn eq(&self, other: &Self) -> bool {
        matches!(
            (self, other),
            (Self::Tiny, Self::Tiny)
                | (Self::Medium, Self::Medium)
                | (Self::Consequential, Self::Consequential)
                | (
                    Self::Standard | Self::Greenfield,
                    Self::Standard | Self::Greenfield
                )
        )
    }
}

impl Eq for WorkflowTier {}

impl std::hash::Hash for WorkflowTier {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        match self {
            Self::Tiny => 0u8.hash(state),
            Self::Medium => 1u8.hash(state),
            Self::Standard | Self::Greenfield => 2u8.hash(state),
            Self::Consequential => 3u8.hash(state),
        }
    }
}

impl WorkflowTier {
    /// True if this tier represents standard or greenfield product creation.
    pub fn is_standard(&self) -> bool {
        matches!(self, Self::Standard | Self::Greenfield)
    }
}

impl std::fmt::Display for WorkflowTier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Tiny => write!(f, "tiny"),
            Self::Medium => write!(f, "medium"),
            Self::Standard => write!(f, "standard"),
            Self::Greenfield => write!(f, "greenfield"),
            Self::Consequential => write!(f, "consequential"),
        }
    }
}

/// Discovered domain model representing target software entities, workflows, and non-goals.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct TargetDomainModel {
    /// Core target domain entities (e.g. ["Expense", "Category", "Budget"]).
    pub entities: Vec<String>,
    /// Core user and system workflows (e.g. ["Record expense", "Filter by category"]).
    pub core_workflows: Vec<String>,
    /// Inferred safe default architectural choices.
    pub inferred_defaults: Vec<String>,
    /// Out-of-scope capabilities and non-goals.
    pub non_goals: Vec<String>,
}

/// Canonical Project Charter aggregate (Pillar 1-4 of Project Genesis).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectCharter {
    pub project_name: String,
    pub overview: String,
    pub problem_statement: String,
    pub target_personas: Vec<String>,
    pub user_journeys: Vec<String>,
    pub success_metrics: Vec<String>,
    pub boundaries: ProjectBoundaries,
    pub technical_preferences: TechnicalPreferences,
    pub operational_invariants: OperationalInvariants,
    pub ambiguity_assessment: AmbiguityAssessment,
    pub confirmed_by_user: bool,
    #[serde(default)]
    pub workflow_tier: WorkflowTier,
    #[serde(default)]
    pub domain_model: TargetDomainModel,
}

impl ProjectCharter {
    /// Create a new blank charter from a project name and overview.
    pub fn new(project_name: impl Into<String>, overview: impl Into<String>) -> Self {
        Self {
            project_name: project_name.into(),
            overview: overview.into(),
            problem_statement: String::new(),
            target_personas: Vec::new(),
            user_journeys: Vec::new(),
            success_metrics: Vec::new(),
            boundaries: ProjectBoundaries::default(),
            technical_preferences: TechnicalPreferences::default(),
            operational_invariants: OperationalInvariants::default(),
            ambiguity_assessment: AmbiguityAssessment::default(),
            confirmed_by_user: false,
            workflow_tier: WorkflowTier::Greenfield,
            domain_model: TargetDomainModel::default(),
        }
    }

    pub fn with_workflow_tier(mut self, tier: WorkflowTier) -> Self {
        self.workflow_tier = tier;
        self
    }

    pub fn with_domain_model(mut self, model: TargetDomainModel) -> Self {
        self.domain_model = model;
        self
    }

    /// Validates that the charter meets minimum quality gate requirements.
    pub fn validate(&self) -> Result<(), GenesisError> {
        if self.project_name.trim().is_empty() {
            return Err(GenesisError::CharterValidation(
                "project_name cannot be empty".to_string(),
            ));
        }
        if self.overview.trim().is_empty() {
            return Err(GenesisError::CharterValidation(
                "overview cannot be empty".to_string(),
            ));
        }
        if self.problem_statement.trim().is_empty() {
            return Err(GenesisError::CharterValidation(
                "problem_statement cannot be empty".to_string(),
            ));
        }
        if self.boundaries.in_scope.is_empty() {
            return Err(GenesisError::CharterValidation(
                "in_scope boundaries must define at least one item".to_string(),
            ));
        }
        Ok(())
    }

    /// Render the charter into canonical Markdown projection format.
    pub fn to_markdown(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!("# Project Charter: {}\n\n", self.project_name));

        out.push_str("## Overview\n");
        out.push_str(self.overview.trim());
        out.push_str("\n\n");

        out.push_str("## Problem Statement\n");
        out.push_str(self.problem_statement.trim());
        out.push_str("\n\n");

        out.push_str("## Target Personas\n");
        if self.target_personas.is_empty() {
            out.push_str("- General developers\n");
        } else {
            for p in &self.target_personas {
                out.push_str(&format!("- {}\n", p));
            }
        }
        out.push('\n');

        if !self.user_journeys.is_empty() {
            out.push_str("## User Journeys\n");
            for j in &self.user_journeys {
                out.push_str(&format!("- {}\n", j));
            }
            out.push('\n');
        }

        out.push_str("## Success Metrics\n");
        if self.success_metrics.is_empty() {
            out.push_str("- Verified unit and integration tests\n");
        } else {
            for m in &self.success_metrics {
                out.push_str(&format!("- {}\n", m));
            }
        }
        out.push('\n');

        out.push_str("## Scope & Boundaries\n\n");
        out.push_str("### In-Scope\n");
        for s in &self.boundaries.in_scope {
            out.push_str(&format!("- {}\n", s));
        }
        out.push('\n');

        out.push_str("### Non-Goals\n");
        if self.boundaries.non_goals.is_empty() {
            out.push_str("- None specified\n");
        } else {
            for ng in &self.boundaries.non_goals {
                out.push_str(&format!("- {}\n", ng));
            }
        }
        out.push('\n');

        out.push_str("### Anti-Features\n");
        if self.boundaries.anti_features.is_empty() {
            out.push_str("- None specified\n");
        } else {
            for af in &self.boundaries.anti_features {
                out.push_str(&format!("- {}\n", af));
            }
        }
        out.push('\n');

        out.push_str("## Technical Preferences\n");
        if !self.technical_preferences.languages.is_empty() {
            out.push_str(&format!(
                "- **Languages**: {}\n",
                self.technical_preferences.languages.join(", ")
            ));
        }
        if let Some(ref storage) = self.technical_preferences.storage {
            out.push_str(&format!("- **Storage**: {}\n", storage));
        }
        if let Some(ref deploy) = self.technical_preferences.deployment_target {
            out.push_str(&format!("- **Deployment**: {}\n", deploy));
        }
        if let Some(ref arch) = self.technical_preferences.architecture_style {
            out.push_str(&format!("- **Architecture**: {}\n", arch));
        }
        if !self.technical_preferences.frameworks.is_empty() {
            out.push_str(&format!(
                "- **Frameworks/Libraries**: {}\n",
                self.technical_preferences.frameworks.join(", ")
            ));
        }
        out.push('\n');

        out.push_str("## Operational Invariants\n");
        if let Some(ref lic) = self.operational_invariants.licensing {
            out.push_str(&format!("- **License**: {}\n", lic));
        }
        for sec in &self.operational_invariants.security_requirements {
            out.push_str(&format!("- **Security**: {}\n", sec));
        }
        for perf in &self.operational_invariants.performance_targets {
            out.push_str(&format!("- **Performance**: {}\n", perf));
        }
        for scale in &self.operational_invariants.scale_targets {
            out.push_str(&format!("- **Scale**: {}\n", scale));
        }
        out.push('\n');

        out.push_str("## Ambiguity Assessment\n");
        out.push_str(&format!(
            "- **Score**: {}%\n",
            self.ambiguity_assessment.score_percent
        ));
        out.push_str(&format!(
            "- **Status**: {}\n",
            if self.ambiguity_assessment.converged {
                "Converged"
            } else {
                "In Progress"
            }
        ));
        if !self.ambiguity_assessment.unresolved_areas.is_empty() {
            out.push_str("- **Unresolved Areas**:\n");
            for unres in &self.ambiguity_assessment.unresolved_areas {
                out.push_str(&format!("  - {}\n", unres));
            }
        }
        if !self.ambiguity_assessment.resolved_invariants.is_empty() {
            out.push_str("- **Resolved Invariants**:\n");
            for res in &self.ambiguity_assessment.resolved_invariants {
                out.push_str(&format!("  - {}\n", res));
            }
        }
        out.push('\n');

        out.push_str(&format!(
            "<!-- user_confirmed: {} -->\n",
            self.confirmed_by_user
        ));

        out
    }

    /// Parse a `PROJECT.md` markdown document back into a structured ProjectCharter.
    pub fn from_markdown(content: &str) -> Result<Self, GenesisError> {
        let mut charter = ProjectCharter::new("", "");

        let lines: Vec<&str> = content.lines().collect();
        let mut current_section = "";
        let mut current_subsection = "";

        for line in lines {
            let trimmed = line.trim();

            if trimmed.starts_with("# Project Charter:") {
                charter.project_name = trimmed
                    .trim_start_matches("# Project Charter:")
                    .trim()
                    .to_string();
                continue;
            }

            if trimmed.starts_with("<!-- user_confirmed:") {
                let confirmed_str = trimmed
                    .trim_start_matches("<!-- user_confirmed:")
                    .trim_end_matches("-->")
                    .trim();
                charter.confirmed_by_user = confirmed_str == "true";
                continue;
            }

            if trimmed.starts_with("### ") {
                current_subsection = trimmed.trim_start_matches("### ").trim();
                continue;
            }

            if trimmed.starts_with("## ") {
                current_section = trimmed.trim_start_matches("## ").trim();
                current_subsection = "";
                continue;
            }

            if trimmed.is_empty() {
                continue;
            }

            match current_section {
                "Overview" => {
                    if !charter.overview.is_empty() {
                        charter.overview.push('\n');
                    }
                    charter.overview.push_str(trimmed);
                }
                "Problem Statement" => {
                    if !charter.problem_statement.is_empty() {
                        charter.problem_statement.push('\n');
                    }
                    charter.problem_statement.push_str(trimmed);
                }
                "Target Personas" => {
                    if let Some(item) = trimmed.strip_prefix('-') {
                        charter.target_personas.push(item.trim().to_string());
                    }
                }
                "User Journeys" => {
                    if let Some(item) = trimmed.strip_prefix('-') {
                        charter.user_journeys.push(item.trim().to_string());
                    }
                }
                "Success Metrics" => {
                    if let Some(item) = trimmed.strip_prefix('-') {
                        charter.success_metrics.push(item.trim().to_string());
                    }
                }
                "Scope & Boundaries" => match current_subsection {
                    "In-Scope" => {
                        if let Some(item) = trimmed.strip_prefix('-') {
                            charter.boundaries.in_scope.push(item.trim().to_string());
                        }
                    }
                    "Non-Goals" => {
                        if let Some(item) = trimmed.strip_prefix('-') {
                            let item_trim = item.trim();
                            if item_trim != "None specified" {
                                charter.boundaries.non_goals.push(item_trim.to_string());
                            }
                        }
                    }
                    "Anti-Features" => {
                        if let Some(item) = trimmed.strip_prefix('-') {
                            let item_trim = item.trim();
                            if item_trim != "None specified" {
                                charter.boundaries.anti_features.push(item_trim.to_string());
                            }
                        }
                    }
                    _ => {}
                },
                "Technical Preferences" => {
                    if let Some(item) = trimmed.strip_prefix('-') {
                        let item = item.trim();
                        if let Some(val) = item.strip_prefix("**Languages**:") {
                            charter.technical_preferences.languages = val
                                .split(',')
                                .map(|s| s.trim().to_string())
                                .filter(|s| !s.is_empty())
                                .collect();
                        } else if let Some(val) = item.strip_prefix("**Storage**:") {
                            charter.technical_preferences.storage = Some(val.trim().to_string());
                        } else if let Some(val) = item.strip_prefix("**Deployment**:") {
                            charter.technical_preferences.deployment_target =
                                Some(val.trim().to_string());
                        } else if let Some(val) = item.strip_prefix("**Architecture**:") {
                            charter.technical_preferences.architecture_style =
                                Some(val.trim().to_string());
                        } else if let Some(val) = item.strip_prefix("**Frameworks/Libraries**:") {
                            charter.technical_preferences.frameworks = val
                                .split(',')
                                .map(|s| s.trim().to_string())
                                .filter(|s| !s.is_empty())
                                .collect();
                        }
                    }
                }
                "Operational Invariants" => {
                    if let Some(item) = trimmed.strip_prefix('-') {
                        let item = item.trim();
                        if let Some(val) = item.strip_prefix("**License**:") {
                            charter.operational_invariants.licensing = Some(val.trim().to_string());
                        } else if let Some(val) = item.strip_prefix("**Security**:") {
                            charter
                                .operational_invariants
                                .security_requirements
                                .push(val.trim().to_string());
                        } else if let Some(val) = item.strip_prefix("**Performance**:") {
                            charter
                                .operational_invariants
                                .performance_targets
                                .push(val.trim().to_string());
                        } else if let Some(val) = item.strip_prefix("**Scale**:") {
                            charter
                                .operational_invariants
                                .scale_targets
                                .push(val.trim().to_string());
                        }
                    }
                }
                "Ambiguity Assessment" => {
                    if let Some(item) = trimmed.strip_prefix('-') {
                        let item = item.trim();
                        if let Some(val) = item.strip_prefix("**Score**:") {
                            let num_str = val.trim().trim_end_matches('%');
                            if let Ok(score) = num_str.parse::<u8>() {
                                charter.ambiguity_assessment.score_percent = score;
                            }
                        } else if let Some(val) = item.strip_prefix("**Status**:") {
                            charter.ambiguity_assessment.converged = val.trim() == "Converged";
                        }
                    }
                }
                _ => {}
            }
        }

        if charter.project_name.is_empty() {
            charter.project_name = "Untitled Project".to_string();
        }

        Ok(charter)
    }

    /// Persist the charter to `<workspace_root>/<projection_dir>/PROJECT.md`.
    pub fn save_to_dir(
        &self,
        workspace_root: &Path,
        projection_dir: &str,
    ) -> Result<PathBuf, GenesisError> {
        let dir = workspace_root.join(projection_dir);
        std::fs::create_dir_all(&dir)?;
        let file_path = dir.join("PROJECT.md");
        std::fs::write(&file_path, self.to_markdown())?;
        Ok(file_path)
    }
}
