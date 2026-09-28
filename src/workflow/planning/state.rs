//! Planning lifecycle state machine and STATE.md projection.

use crate::workflow::error::WorkflowError;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::{Path, PathBuf};

/// Ten-stage explicit planning lifecycle state machine adhering to Section 29.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Default,
)]
#[serde(rename_all = "snake_case")]
pub enum PlanningLifecycleState {
    #[default]
    RequirementsDraft,
    RequirementsAwaitingApproval,
    RequirementsApproved,
    ArchitectureDraft,
    ArchitectureAwaitingApproval,
    ArchitectureApproved,
    RoadmapDraft,
    RoadmapAwaitingApproval,
    RoadmapApproved,
    ImplementationReady,
}

impl PlanningLifecycleState {
    /// Validate whether transitioning to `target` is legal.
    pub fn can_transition_to(&self, target: PlanningLifecycleState) -> bool {
        match (self, target) {
            (Self::RequirementsDraft, Self::RequirementsAwaitingApproval) => true,
            (Self::RequirementsDraft, Self::RequirementsApproved) => true, // Auto-approval bypass if configured
            (Self::RequirementsAwaitingApproval, Self::RequirementsApproved) => true,
            (Self::RequirementsAwaitingApproval, Self::RequirementsDraft) => true, // Rejection / revision request

            (Self::RequirementsApproved, Self::ArchitectureDraft) => true,
            (Self::RequirementsApproved, Self::ArchitectureApproved) => true, // Automated batch pipeline
            (Self::ArchitectureDraft, Self::ArchitectureAwaitingApproval) => true,
            (Self::ArchitectureDraft, Self::ArchitectureApproved) => true, // Auto-approval bypass
            (Self::ArchitectureAwaitingApproval, Self::ArchitectureApproved) => true,
            (Self::ArchitectureAwaitingApproval, Self::ArchitectureDraft) => true, // Rejection / revision

            (Self::ArchitectureApproved, Self::RoadmapDraft) => true,
            (Self::ArchitectureApproved, Self::RoadmapApproved) => true, // Automated batch pipeline
            (Self::RoadmapDraft, Self::RoadmapAwaitingApproval) => true,
            (Self::RoadmapDraft, Self::RoadmapApproved) => true, // Auto-approval bypass
            (Self::RoadmapAwaitingApproval, Self::RoadmapApproved) => true,
            (Self::RoadmapAwaitingApproval, Self::RoadmapDraft) => true, // Rejection / revision

            (Self::RoadmapApproved, Self::ImplementationReady) => true,

            // Invalidation resets:
            // Downstream invalidation when requirements change
            (
                Self::ArchitectureApproved | Self::RoadmapApproved | Self::ImplementationReady,
                Self::RequirementsDraft,
            ) => true,
            // Downstream invalidation when architecture changes
            (Self::RoadmapApproved | Self::ImplementationReady, Self::ArchitectureDraft) => true,
            // Downstream invalidation when roadmap changes
            (Self::ImplementationReady, Self::RoadmapDraft) => true,

            _ => false,
        }
    }
}

impl fmt::Display for PlanningLifecycleState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RequirementsDraft => write!(f, "requirements_draft"),
            Self::RequirementsAwaitingApproval => write!(f, "requirements_awaiting_approval"),
            Self::RequirementsApproved => write!(f, "requirements_approved"),
            Self::ArchitectureDraft => write!(f, "architecture_draft"),
            Self::ArchitectureAwaitingApproval => write!(f, "architecture_awaiting_approval"),
            Self::ArchitectureApproved => write!(f, "architecture_approved"),
            Self::RoadmapDraft => write!(f, "roadmap_draft"),
            Self::RoadmapAwaitingApproval => write!(f, "roadmap_awaiting_approval"),
            Self::RoadmapApproved => write!(f, "roadmap_approved"),
            Self::ImplementationReady => write!(f, "implementation_ready"),
        }
    }
}

impl std::str::FromStr for PlanningLifecycleState {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_lowercase().replace(['-', ' '], "_").as_str() {
            "requirements_draft" => Ok(Self::RequirementsDraft),
            "requirements_awaiting_approval" => Ok(Self::RequirementsAwaitingApproval),
            "requirements_approved" => Ok(Self::RequirementsApproved),
            "architecture_draft" => Ok(Self::ArchitectureDraft),
            "architecture_awaiting_approval" => Ok(Self::ArchitectureAwaitingApproval),
            "architecture_approved" => Ok(Self::ArchitectureApproved),
            "roadmap_draft" => Ok(Self::RoadmapDraft),
            "roadmap_awaiting_approval" => Ok(Self::RoadmapAwaitingApproval),
            "roadmap_approved" => Ok(Self::RoadmapApproved),
            "implementation_ready" => Ok(Self::ImplementationReady),
            other => Err(format!("Unknown planning lifecycle state: {}", other)),
        }
    }
}

/// State machine managing planning lifecycle transitions and approvals.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlanningStateMachine {
    pub current_state: PlanningLifecycleState,
    pub history: Vec<(PlanningLifecycleState, String)>,
}

impl PlanningStateMachine {
    pub fn new() -> Self {
        Self {
            current_state: PlanningLifecycleState::RequirementsDraft,
            history: Vec::new(),
        }
    }

    pub fn transition_to(
        &mut self,
        next: PlanningLifecycleState,
        reason: impl Into<String>,
    ) -> Result<(), WorkflowError> {
        if !self.current_state.can_transition_to(next) {
            return Err(WorkflowError::ManifestValidation {
                path: None,
                workflow_id: None,
                step_key: None,
                field: Some("lifecycle_state".to_string()),
                reason: format!(
                    "Illegal planning state transition from '{}' to '{}'",
                    self.current_state, next
                ),
            });
        }
        let reason_str = reason.into();
        self.history.push((self.current_state, reason_str));
        self.current_state = next;
        Ok(())
    }
}

impl Default for PlanningStateMachine {
    fn default() -> Self {
        Self::new()
    }
}

/// Consolidated planning state aggregate projected into STATE.md (Section 28).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlanningState {
    pub project_name: String,
    pub planning_version: u32,
    pub lifecycle_state: PlanningLifecycleState,
    pub requirements_status: String,
    pub requirements_count: usize,
    pub architecture_status: String,
    pub component_count: usize,
    pub adr_status: String,
    pub adr_count: usize,
    pub roadmap_status: String,
    pub phase_count: usize,
    pub approval_status: String,
    pub current_phase: Option<String>,
    pub unresolved_decisions: Vec<String>,
    pub unresolved_risks: Vec<String>,
    pub invalidated_artifacts: Vec<String>,
    pub next_authorized_transition: String,
}

impl PlanningState {
    pub fn new(project_name: impl Into<String>, lifecycle_state: PlanningLifecycleState) -> Self {
        let state_str = lifecycle_state.to_string();
        Self {
            project_name: project_name.into(),
            planning_version: 1,
            lifecycle_state,
            requirements_status: "Draft".to_string(),
            requirements_count: 0,
            architecture_status: "Pending".to_string(),
            component_count: 0,
            adr_status: "Pending".to_string(),
            adr_count: 0,
            roadmap_status: "Pending".to_string(),
            phase_count: 0,
            approval_status: "Pending Human Sign-off".to_string(),
            current_phase: None,
            unresolved_decisions: Vec::new(),
            unresolved_risks: Vec::new(),
            invalidated_artifacts: Vec::new(),
            next_authorized_transition: state_str,
        }
    }

    /// Render canonical STATE.md format (Section 28).
    pub fn to_markdown(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!(
            "# Planning Runtime State: {}\n\n",
            self.project_name
        ));

        out.push_str(&format!(
            "- **Planning Version**: {}\n",
            self.planning_version
        ));
        out.push_str(&format!(
            "- **Lifecycle State**: `{}`\n",
            self.lifecycle_state
        ));
        out.push_str(&format!(
            "- **Requirements**: {} (Status: `{}`)\n",
            self.requirements_count, self.requirements_status
        ));
        out.push_str(&format!(
            "- **Architecture Components**: {} (Status: `{}`)\n",
            self.component_count, self.architecture_status
        ));
        out.push_str(&format!(
            "- **ADRs Registered**: {} (Status: `{}`)\n",
            self.adr_count, self.adr_status
        ));
        out.push_str(&format!(
            "- **Roadmap Phases**: {} (Status: `{}`)\n",
            self.phase_count, self.roadmap_status
        ));
        out.push_str(&format!(
            "- **Human Approval**: `{}`\n",
            self.approval_status
        ));

        if let Some(ref p) = self.current_phase {
            out.push_str(&format!("- **Active Implementation Phase**: `{}`\n", p));
        }

        out.push_str(&format!(
            "- **Next Authorized Transition**: `{}`\n\n",
            self.next_authorized_transition
        ));

        // Unresolved decisions
        out.push_str("## Unresolved Decisions\n");
        if self.unresolved_decisions.is_empty() {
            out.push_str("None pending operator resolution.\n\n");
        } else {
            for d in &self.unresolved_decisions {
                out.push_str(&format!("- {}\n", d));
            }
            out.push('\n');
        }

        // Unresolved risks
        out.push_str("## Active Planning Risks\n");
        if self.unresolved_risks.is_empty() {
            out.push_str("None flagged.\n\n");
        } else {
            for r in &self.unresolved_risks {
                out.push_str(&format!("- {}\n", r));
            }
            out.push('\n');
        }

        // Invalidated artifacts
        out.push_str("## Invalidated Artifacts\n");
        if self.invalidated_artifacts.is_empty() {
            out.push_str("All planning artifacts are valid and synchronized.\n");
        } else {
            for a in &self.invalidated_artifacts {
                out.push_str(&format!("- ⚠️ `{}` (Superseded/Stale)\n", a));
            }
        }

        out
    }

    /// Persist to `<workspace_root>/<projection_dir>/STATE.md`.
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
        let file_path = dir.join("STATE.md");
        std::fs::write(&file_path, self.to_markdown()).map_err(|e| {
            WorkflowError::ManifestValidation {
                path: Some(file_path.display().to_string()),
                workflow_id: None,
                step_key: None,
                field: None,
                reason: format!("failed to write STATE.md: {}", e),
            }
        })?;
        Ok(file_path)
    }

    /// Parse `STATE.md` markdown content back into `PlanningState`.
    pub fn from_markdown(content: &str) -> Result<Self, WorkflowError> {
        let mut project_name = "Project".to_string();
        let mut planning_version = 1;
        let mut lifecycle_state = PlanningLifecycleState::RequirementsDraft;
        let mut requirements_status = "Draft".to_string();
        let mut requirements_count = 0;
        let mut architecture_status = "Pending".to_string();
        let mut component_count = 0;
        let mut adr_status = "Pending".to_string();
        let mut adr_count = 0;
        let mut roadmap_status = "Pending".to_string();
        let mut phase_count = 0;
        let mut approval_status = "Pending Human Sign-off".to_string();
        let mut current_phase = None;
        let mut next_authorized_transition = String::new();
        let mut unresolved_decisions = Vec::new();
        let mut unresolved_risks = Vec::new();
        let mut invalidated_artifacts = Vec::new();

        let mut section = "";

        for line in content.lines() {
            let line = line.trim();
            if let Some(name) = line.strip_prefix("# Planning Runtime State:") {
                project_name = name.trim().to_string();
            } else if line.starts_with("## Unresolved Decisions") {
                section = "decisions";
            } else if line.starts_with("## Active Planning Risks") {
                section = "risks";
            } else if line.starts_with("## Invalidated Artifacts") {
                section = "invalidated";
            } else if line.starts_with("## ") {
                section = "";
            } else if line.starts_with("- **Planning Version**:") {
                if let Some(val) = line.split(':').nth(1) {
                    planning_version = val.trim().parse().unwrap_or(1);
                }
            } else if line.starts_with("- **Lifecycle State**:") {
                if let Some(val) = line.split(':').nth(1) {
                    let cleaned = val.trim().trim_matches('`');
                    lifecycle_state = cleaned
                        .parse()
                        .unwrap_or(PlanningLifecycleState::RequirementsDraft);
                }
            } else if line.starts_with("- **Requirements**:") {
                let after_colon = line.split(':').skip(1).collect::<Vec<_>>().join(":");
                let parts: Vec<&str> = after_colon.split('(').collect();
                if let Some(cnt_str) = parts.first() {
                    requirements_count = cnt_str.trim().parse().unwrap_or(0);
                }
                if parts.len() > 1 {
                    let status_part = parts[1].trim_end_matches(')').trim();
                    if let Some(st) = status_part.strip_prefix("Status:") {
                        requirements_status = st.trim().trim_matches('`').to_string();
                    }
                }
            } else if line.starts_with("- **Architecture Components**:") {
                let after_colon = line.split(':').skip(1).collect::<Vec<_>>().join(":");
                let parts: Vec<&str> = after_colon.split('(').collect();
                if let Some(cnt_str) = parts.first() {
                    component_count = cnt_str.trim().parse().unwrap_or(0);
                }
                if parts.len() > 1 {
                    let status_part = parts[1].trim_end_matches(')').trim();
                    if let Some(st) = status_part.strip_prefix("Status:") {
                        architecture_status = st.trim().trim_matches('`').to_string();
                    }
                }
            } else if line.starts_with("- **ADRs Registered**:") {
                let after_colon = line.split(':').skip(1).collect::<Vec<_>>().join(":");
                let parts: Vec<&str> = after_colon.split('(').collect();
                if let Some(cnt_str) = parts.first() {
                    adr_count = cnt_str.trim().parse().unwrap_or(0);
                }
                if parts.len() > 1 {
                    let status_part = parts[1].trim_end_matches(')').trim();
                    if let Some(st) = status_part.strip_prefix("Status:") {
                        adr_status = st.trim().trim_matches('`').to_string();
                    }
                }
            } else if line.starts_with("- **Roadmap Phases**:") {
                let after_colon = line.split(':').skip(1).collect::<Vec<_>>().join(":");
                let parts: Vec<&str> = after_colon.split('(').collect();
                if let Some(cnt_str) = parts.first() {
                    phase_count = cnt_str.trim().parse().unwrap_or(0);
                }
                if parts.len() > 1 {
                    let status_part = parts[1].trim_end_matches(')').trim();
                    if let Some(st) = status_part.strip_prefix("Status:") {
                        roadmap_status = st.trim().trim_matches('`').to_string();
                    }
                }
            } else if line.starts_with("- **Human Approval**:") {
                if let Some(val) = line.split(':').nth(1) {
                    approval_status = val.trim().trim_matches('`').to_string();
                }
            } else if line.starts_with("- **Active Implementation Phase**:") {
                if let Some(val) = line.split(':').nth(1) {
                    current_phase = Some(val.trim().trim_matches('`').to_string());
                }
            } else if line.starts_with("- **Next Authorized Transition**:") {
                if let Some(val) = line.split(':').nth(1) {
                    next_authorized_transition = val.trim().trim_matches('`').to_string();
                }
            } else if line.starts_with("- ") {
                let item = line.trim_start_matches("- ").trim();
                match section {
                    "decisions" if !item.starts_with("None") => {
                        unresolved_decisions.push(item.to_string());
                    }
                    "risks" if !item.starts_with("None") => {
                        unresolved_risks.push(item.to_string());
                    }
                    "invalidated" if !item.starts_with("All planning artifacts") => {
                        invalidated_artifacts.push(item.to_string());
                    }
                    _ => {}
                }
            }
        }

        if next_authorized_transition.is_empty() {
            next_authorized_transition = lifecycle_state.to_string();
        }

        Ok(Self {
            project_name,
            planning_version,
            lifecycle_state,
            requirements_status,
            requirements_count,
            architecture_status,
            component_count,
            adr_status,
            adr_count,
            roadmap_status,
            phase_count,
            approval_status,
            current_phase,
            unresolved_decisions,
            unresolved_risks,
            invalidated_artifacts,
            next_authorized_transition,
        })
    }
}
