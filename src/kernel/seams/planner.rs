use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::ids::{MissionId, TaskId};
use crate::kernel::plan::CandidatePlan;

/// Structured upstream product and architectural context prepared prior to canonical planning.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct UpstreamPlanContext {
    pub project_name: String,
    pub charter: String,
    pub architecture: String,
    #[serde(default)]
    pub requirements: Vec<String>,
    #[serde(default)]
    pub assumptions: Vec<String>,
    #[serde(default)]
    pub decisions: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub research_summary: Option<String>,
    #[serde(default)]
    pub workflow_tier: String,
    /// Unresolved epistemic unknowns from discovery and research.
    #[serde(default)]
    pub unknowns: Vec<String>,
    /// Critical decisions requiring explicit operator direction (UserDecisionRequired).
    #[serde(default)]
    pub user_decisions: Vec<String>,
    /// Epistemic invariants definitively resolved earlier during discovery/synthesis.
    #[serde(default)]
    pub resolved_invariants: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PlanRequest {
    pub mission_id: MissionId,
    pub objective: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upstream_context: Option<UpstreamPlanContext>,
}

impl PlanRequest {
    pub fn new(mission_id: MissionId, objective: impl Into<String>) -> Self {
        Self {
            mission_id,
            objective: objective.into(),
            upstream_context: None,
        }
    }

    pub fn with_upstream_context(mut self, ctx: UpstreamPlanContext) -> Self {
        self.upstream_context = Some(ctx);
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PlanResponse {
    pub plan_id: String,
    pub task_count: usize,
    pub candidate_plan: CandidatePlan,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReplanRequest {
    pub mission_id: MissionId,
    pub failed_task_id: TaskId,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ReplanResponse {
    pub new_plan_id: String,
    pub modified_tasks: Vec<TaskId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub candidate_plan: Option<CandidatePlan>,
}

#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum PlanError {
    #[error("plan generation failed: {0}")]
    GenerationFailed(String),

    #[error("replan failed: {0}")]
    ReplanFailed(String),
}

#[async_trait]
pub trait PlanService: Send + Sync {
    async fn has_valid_plan(&self, mission_id: MissionId) -> Result<bool, PlanError>;
    async fn generate_initial_plan(&self, req: PlanRequest) -> Result<PlanResponse, PlanError>;
    async fn replan(&self, req: ReplanRequest) -> Result<ReplanResponse, PlanError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plan_dto_serde() {
        let req = PlanRequest::new(MissionId::new(), "Build M31A");
        let serialized = serde_json::to_string(&req).unwrap();
        let deserialized: PlanRequest = serde_json::from_str(&serialized).unwrap();
        assert_eq!(req, deserialized);

        let req_with_context = req.with_upstream_context(UpstreamPlanContext {
            project_name: "ExpenseTracker".to_string(),
            charter: "Charter details".to_string(),
            architecture: "Modular CLI".to_string(),
            requirements: vec!["REQ-01".to_string()],
            assumptions: vec!["ASSUME-01".to_string()],
            decisions: vec!["DEC-01".to_string()],
            research_summary: Some("Summary".to_string()),
            workflow_tier: "greenfield".to_string(),
            unknowns: vec!["UNK-01".to_string()],
            user_decisions: vec!["DECIDE-01".to_string()],
            resolved_invariants: vec!["INV-01".to_string()],
        });
        let s2 = serde_json::to_string(&req_with_context).unwrap();
        let d2: PlanRequest = serde_json::from_str(&s2).unwrap();
        assert_eq!(req_with_context, d2);
    }

    #[test]
    fn test_replan_dto_serde() {
        let req = ReplanRequest {
            mission_id: MissionId::new(),
            failed_task_id: TaskId::new(),
            reason: "Compiler error".to_string(),
        };
        let serialized = serde_json::to_string(&req).unwrap();
        let deserialized: ReplanRequest = serde_json::from_str(&serialized).unwrap();
        assert_eq!(req, deserialized);
    }
}
