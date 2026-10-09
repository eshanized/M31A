//! Typed event vocabulary covering all domain concepts (KRN-03)

use crate::ids::{
    AgentId, ArtifactId, CheckpointId, HandoffId, JobId, MissionId, RequirementId, TaskGraphId,
    TaskId, ToolCallId, WorkflowRunId, WorkflowStepRunId,
};
use crate::model::types::TokenUsage;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Bounded task summary projected during task graph materialization (D-12, DAG-01, TUI-03).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TaskSummary {
    pub id: TaskId,
    pub title: String,
    pub role: crate::state_machine::agent::AgentRole,
    pub status: crate::state_machine::TaskState,
    pub dependencies: Vec<TaskId>,
}

/// EventType enum covering all domain concepts from KRN-03:
/// mission, requirement, plan, task, agent, tool, job, verification,
/// review, policy, checkpoint, artifact, context, and recovery events.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", content = "payload")]
pub enum EventType {
    // Mission events
    MissionStarted {
        mission_id: MissionId,
        objective: String,
    },
    MissionCompleted {
        mission_id: MissionId,
    },
    MissionFailed {
        mission_id: MissionId,
        reason: String,
    },
    MissionCancelled {
        mission_id: MissionId,
        reason: String,
    },
    MissionPaused {
        mission_id: MissionId,
        reason: String,
    },
    MissionResumed {
        mission_id: MissionId,
    },
    MissionStateChanged {
        mission_id: MissionId,
        from: String,
        to: String,
    },

    // Session events
    SessionStarted {
        session_id: crate::ids::SessionId,
        mission_id: MissionId,
    },
    SessionResumed {
        session_id: crate::ids::SessionId,
        mission_id: Option<MissionId>,
    },

    // Requirement events
    RequirementExtracted {
        requirement_id: RequirementId,
        mission_id: MissionId,
        description: String,
    },
    RequirementVerified {
        requirement_id: RequirementId,
        verified: bool,
        evidence: String,
    },

    // Plan events
    PlanCreated {
        plan_id: String,
        mission_id: MissionId,
        summary: String,
    },
    PlanValidated {
        plan_id: String,
        valid: bool,
        issues: Vec<String>,
    },

    // Human-governed lifecycle and pre-execution review events.
    PlanReviewRequired {
        session_id: String,
        plan_id: String,
        revision: u32,
    },
    PlanRevisionCreated {
        session_id: String,
        plan_id: String,
        revision: u32,
        author: String,
    },
    PlanAccepted {
        session_id: String,
        plan_id: String,
        revision: u32,
    },
    PlanRejected {
        session_id: String,
        plan_id: String,
        revision: u32,
        reason: String,
    },
    TasksReviewRequired {
        session_id: String,
        plan_revision: u32,
        task_revision: u32,
        task_count: usize,
    },
    TaskRevisionCreated {
        session_id: String,
        task_revision: u32,
        plan_revision: u32,
        author: String,
    },
    TasksAccepted {
        session_id: String,
        task_revision: u32,
        plan_revision: u32,
    },
    ExecutionAuthorizationRequired {
        session_id: String,
        plan_revision: u32,
        task_revision: u32,
        message: String,
    },
    ExecutionAuthorized {
        session_id: String,
        authorization_id: uuid::Uuid,
        authorized_by: String,
    },
    ExecutionAuthorizationRejected {
        session_id: String,
        reason: String,
    },

    // Task events
    TaskCreated {
        task_id: TaskId,
        mission_id: MissionId,
        title: String,
        requirement_id: Option<RequirementId>,
    },
    TaskStarted {
        task_id: TaskId,
        mission_id: MissionId,
        agent_id: AgentId,
    },
    TaskCompleted {
        task_id: TaskId,
        mission_id: MissionId,
        result: String,
    },
    TaskFailed {
        task_id: TaskId,
        mission_id: MissionId,
        error: String,
    },
    TaskCancelled {
        task_id: TaskId,
        mission_id: MissionId,
        reason: String,
    },
    TaskAttemptFailed {
        task_id: TaskId,
        mission_id: MissionId,
        attempt: u32,
        error: String,
    },
    TaskRetryScheduled {
        task_id: TaskId,
        mission_id: MissionId,
        attempt: u32,
        retry_delay_ms: u64,
    },
    TaskSkipped {
        task_id: TaskId,
        mission_id: MissionId,
        reason: String,
    },
    TaskNeedsReview {
        task_id: TaskId,
        mission_id: MissionId,
        reason: String,
    },

    // Agent events
    AgentSpawned {
        agent_id: AgentId,
        mission_id: MissionId,
        role: String,
    },
    AgentStarted {
        agent_id: AgentId,
        mission_id: MissionId,
        task_id: TaskId,
    },
    AgentStepCompleted {
        agent_id: AgentId,
        task_id: TaskId,
        step_number: u32,
        steps_remaining: u32,
    },
    AgentHandoffRecorded {
        handoff_id: HandoffId,
        mission_id: MissionId,
        source_agent_id: AgentId,
        target_role: String,
        reason: String,
    },
    AgentCompleted {
        agent_id: AgentId,
        mission_id: MissionId,
        summary: String,
    },
    AgentFailed {
        agent_id: AgentId,
        mission_id: MissionId,
        error: String,
    },
    AgentCancelled {
        agent_id: AgentId,
        mission_id: MissionId,
        reason: String,
    },

    // Tool events
    ToolRequested {
        tool_call_id: ToolCallId,
        agent_id: AgentId,
        tool_name: String,
        arguments: HashMap<String, serde_json::Value>,
    },
    ToolCompleted {
        tool_call_id: ToolCallId,
        agent_id: AgentId,
        result: String,
    },
    ToolFailed {
        tool_call_id: ToolCallId,
        agent_id: AgentId,
        error: String,
    },

    // Job events
    JobStarted {
        job_id: JobId,
        mission_id: MissionId,
        job_type: String,
    },
    JobCompleted {
        job_id: JobId,
        mission_id: MissionId,
        artifacts: Vec<ArtifactId>,
    },
    JobFailed {
        job_id: JobId,
        mission_id: MissionId,
        error: String,
    },

    // Verification events
    VerificationStarted {
        verification_id: String,
        mission_id: MissionId,
        target: String,
    },
    VerificationCompleted {
        verification_id: String,
        mission_id: MissionId,
        passed: bool,
        evidence: String,
    },

    // Review events
    ReviewRequested {
        review_id: String,
        mission_id: MissionId,
        reviewer: String,
    },
    ReviewCompleted {
        review_id: String,
        mission_id: MissionId,
        approved: bool,
        comments: String,
    },

    // Policy events
    PolicyEvaluated {
        policy_id: String,
        decision: String,
        reason: String,
    },
    PolicyDenied {
        policy_id: String,
        reason: String,
    },

    // Checkpoint events
    CheckpointCreated {
        checkpoint_id: CheckpointId,
        mission_id: MissionId,
        description: String,
    },
    CheckpointRestored {
        checkpoint_id: CheckpointId,
        mission_id: MissionId,
    },

    // Artifact events
    ArtifactCreated {
        artifact_id: ArtifactId,
        mission_id: MissionId,
        path: String,
        content_type: String,
    },

    // Context events
    ContextCompiled {
        context_id: String,
        mission_id: MissionId,
        token_count: usize,
    },

    // Prompt compilation and lifecycle events.
    PromptCompiled {
        mission_id: MissionId,
        task_id: TaskId,
        prompt_id: String,
        prompt_version: u32,
        effective_prompt_hash: String,
        strategy: String,
        total_bytes: usize,
    },
    PromptCompilationFailed {
        mission_id: MissionId,
        task_id: TaskId,
        prompt_id: String,
        prompt_version: u32,
        error: String,
    },

    // Recovery events
    RecoveryAttempted {
        recovery_id: String,
        mission_id: MissionId,
        strategy: String,
        success: bool,
    },

    // Model usage telemetry events (D-08, MDL-05, TUI-03)
    ModelUsageUpdated {
        invocation_id: Option<uuid::Uuid>,
        mission_id: Option<MissionId>,
        task_id: Option<TaskId>,
        provider: String,
        model: String,
        usage: TokenUsage,
        cumulative_usage: Option<TokenUsage>,
        cost_usd: Option<f64>,
        cost_provenance: crate::model::types::CostProvenance,
    },

    // Controller & Escalation events (D-04)
    ControllerCycleStarted {
        mission_id: MissionId,
        cycle: u64,
    },
    ControllerStageTransitioned {
        mission_id: MissionId,
        cycle: u64,
        from_stage: String,
        to_stage: String,
    },
    ControllerDecisionModeChanged {
        mission_id: MissionId,
        from: crate::state_machine::AutonomyMode,
        to: crate::state_machine::AutonomyMode,
    },
    ControllerHalted {
        mission_id: MissionId,
        reason: String,
    },
    OperatorEscalationRequested {
        request_id: String,
        mission_id: MissionId,
        reason: String,
        timeout_seconds: Option<u64>,
        #[serde(default)]
        tool_name: Option<String>,
        #[serde(default)]
        parameters_summary: Option<String>,
        #[serde(default)]
        risk_tier: Option<String>,
        #[serde(default)]
        agent_role: Option<String>,
    },
    EscalationTimedOut {
        request_id: String,
        mission_id: MissionId,
    },
    ApprovalResolved {
        request_id: String,
        decision: String,
        resolved_by: String,
    },

    // Scheduler & Task DAG events (D-12, DAG-01, DAG-02, DAG-04)
    TaskGraphMaterialized {
        graph_id: TaskGraphId,
        mission_id: MissionId,
        revision: u32,
        task_count: usize,
        #[serde(default)]
        tasks: Vec<TaskSummary>,
    },
    WaveTierComputed {
        graph_id: TaskGraphId,
        wave_index: usize,
        task_ids: Vec<TaskId>,
    },
    ResourceLeased {
        lease_id: uuid::Uuid,
        task_id: TaskId,
        resource_key: String,
        lock_mode: String,
    },
    ResourceReleased {
        lease_id: uuid::Uuid,
        task_id: TaskId,
        resource_key: String,
    },
    ResourceRevoked {
        lease_id: uuid::Uuid,
        task_id: TaskId,
        resource_key: String,
        reason: String,
    },
    CriticalPathRecalculated {
        graph_id: TaskGraphId,
        critical_tasks: Vec<TaskId>,
        projected_duration_secs: u64,
    },
    TaskBlocked {
        task_id: TaskId,
        mission_id: MissionId,
        reason: String,
    },
    TaskUnblocked {
        task_id: TaskId,
        mission_id: MissionId,
    },
    TaskNeedsReviewRequested {
        task_id: TaskId,
        mission_id: MissionId,
    },
    TaskReviewed {
        task_id: TaskId,
        mission_id: MissionId,
        approved: bool,
        reviewer: String,
    },
    TaskRetried {
        task_id: TaskId,
        mission_id: MissionId,
        attempt: u32,
        max_retries: u32,
    },
    TaskSuperseded {
        task_id: TaskId,
        mission_id: MissionId,
        replacement_task_id: Option<TaskId>,
    },

    // Git & repository events (GST-04, D-04, D-05)
    GitStateChanged {
        workspace_branch: String,
        execution_branch: Option<String>,
        is_clean: bool,
    },
    RepositoryDriftDetected {
        mission_id: MissionId,
        expected_hash: String,
        actual_hash: String,
        modified_files: Vec<String>,
    },
    WorktreeIntegrationStarted {
        mission_id: MissionId,
        target_branch: String,
    },
    WorktreeIntegrated {
        mission_id: MissionId,
        target_branch: String,
        merge_commit: String,
    },
    WorktreeIntegrationConflict {
        mission_id: MissionId,
        target_branch: String,
        conflict_files: Vec<String>,
    },

    // Workflow lifecycle events (WFL-01, Package 3)
    WorkflowStarted {
        workflow_run_id: WorkflowRunId,
        definition_id: String,
    },
    WorkflowCompleted {
        workflow_run_id: WorkflowRunId,
    },
    WorkflowFailed {
        workflow_run_id: WorkflowRunId,
        reason: String,
    },
    WorkflowCancelled {
        workflow_run_id: WorkflowRunId,
        reason: String,
    },
    WorkflowPaused {
        workflow_run_id: WorkflowRunId,
        reason: String,
    },
    WorkflowResumed {
        workflow_run_id: WorkflowRunId,
    },
    WorkflowStepStarted {
        workflow_run_id: WorkflowRunId,
        step_run_id: WorkflowStepRunId,
        step_key: String,
        mission_id: Option<MissionId>,
        attempt: u32,
    },
    WorkflowStepCompleted {
        workflow_run_id: WorkflowRunId,
        step_run_id: WorkflowStepRunId,
        step_key: String,
    },
    WorkflowStepFailed {
        workflow_run_id: WorkflowRunId,
        step_run_id: WorkflowStepRunId,
        step_key: String,
        reason: String,
    },
    WorkflowStepBlocked {
        workflow_run_id: WorkflowRunId,
        step_run_id: WorkflowStepRunId,
        step_key: String,
        reason: String,
    },
    WorkflowStepAwaitingApproval {
        workflow_run_id: WorkflowRunId,
        step_run_id: WorkflowStepRunId,
        step_key: String,
    },
    WorkflowStepAwaitingInput {
        workflow_run_id: WorkflowRunId,
        step_run_id: WorkflowStepRunId,
        step_key: String,
    },
    WorkflowStepSkipped {
        workflow_run_id: WorkflowRunId,
        step_run_id: WorkflowStepRunId,
        step_key: String,
    },
    WorkflowStepRetrying {
        workflow_run_id: WorkflowRunId,
        step_run_id: WorkflowStepRunId,
        step_key: String,
        attempt: u32,
    },
    WorkflowArtifactRecorded {
        workflow_run_id: WorkflowRunId,
        step_run_id: WorkflowStepRunId,
        artifact_id: ArtifactId,
        name: String,
        path: String,
    },
    QualityGateEvaluated {
        workflow_run_id: WorkflowRunId,
        step_run_id: WorkflowStepRunId,
        step_key: String,
        tier: String,
        passed: bool,
    },
    // Project Genesis events (GEN-01, Package 4)
    GenesisStarted {
        request_id: String,
        mode: String,
    },
    EnvironmentDetected {
        mode: String,
        facts_count: usize,
    },
    DiscoveryStarted {
        request_id: String,
    },
    DiscoveryTurnCompleted {
        turn_number: u32,
        facts_gathered: usize,
        ambiguity_score: u8,
    },
    DiscoveryConverged {
        total_turns: u32,
        ambiguity_score: u8,
        reason: String,
    },
    ProjectCharterProduced {
        path: String,
        ambiguity_score: u8,
    },
    ProjectCharterAwaitingApproval {
        path: String,
    },
    ResearchDecisionMade {
        execute_research: bool,
        dimensions: Vec<String>,
        reason: String,
    },
    ResearchDimensionStarted {
        dimension: String,
    },
    ResearchDimensionCompleted {
        dimension: String,
        evidence_count: usize,
    },
    ResearchDimensionFailed {
        dimension: String,
        error: String,
    },
    ResearchSynthesisStarted {
        dimensions_count: usize,
    },
    ResearchSynthesisCompleted {
        summary_path: String,
        uncertainties_count: usize,
    },

    // Engineering memory and architectural decision events.
    EngineeringDecisionRecorded {
        id: String,
        scope: String,
        title: String,
        status: String,
    },
    EngineeringDecisionSuperseded {
        id: String,
        superseded_by: String,
    },
    EngineeringAssumptionRecorded {
        id: uuid::Uuid,
        mission_id: MissionId,
        statement: String,
        status: String,
    },
    EngineeringAssumptionStatusChanged {
        id: uuid::Uuid,
        mission_id: MissionId,
        from_status: String,
        to_status: String,
        reason: Option<String>,
    },
    ReviewFindingRecorded {
        id: uuid::Uuid,
        mission_id: MissionId,
        task_id: TaskId,
        file_path: String,
        severity: String,
    },
    ReviewFindingStatusChanged {
        id: uuid::Uuid,
        mission_id: MissionId,
        status: String,
        rationale: Option<String>,
    },
    FailureDiagnosisRecorded {
        id: uuid::Uuid,
        mission_id: MissionId,
        task_id: TaskId,
        failure_signature: String,
        failure_class: String,
    },
    VerificationRecordUpdated {
        id: uuid::Uuid,
        mission_id: MissionId,
        requirement_key: String,
        validity: String,
    },
}

impl EventType {
    /// Get a human-readable name for the event type
    pub fn name(&self) -> &'static str {
        match self {
            EventType::MissionStarted { .. } => "MissionStarted",
            EventType::MissionCompleted { .. } => "MissionCompleted",
            EventType::MissionFailed { .. } => "MissionFailed",
            EventType::MissionCancelled { .. } => "MissionCancelled",
            EventType::MissionPaused { .. } => "MissionPaused",
            EventType::MissionResumed { .. } => "MissionResumed",
            EventType::MissionStateChanged { .. } => "MissionStateChanged",
            EventType::SessionStarted { .. } => "SessionStarted",
            EventType::SessionResumed { .. } => "SessionResumed",
            EventType::RequirementExtracted { .. } => "RequirementExtracted",
            EventType::RequirementVerified { .. } => "RequirementVerified",
            EventType::PlanCreated { .. } => "PlanCreated",
            EventType::PlanValidated { .. } => "PlanValidated",
            EventType::PlanReviewRequired { .. } => "PlanReviewRequired",
            EventType::PlanRevisionCreated { .. } => "PlanRevisionCreated",
            EventType::PlanAccepted { .. } => "PlanAccepted",
            EventType::PlanRejected { .. } => "PlanRejected",
            EventType::TasksReviewRequired { .. } => "TasksReviewRequired",
            EventType::TaskRevisionCreated { .. } => "TaskRevisionCreated",
            EventType::TasksAccepted { .. } => "TasksAccepted",
            EventType::ExecutionAuthorizationRequired { .. } => "ExecutionAuthorizationRequired",
            EventType::ExecutionAuthorized { .. } => "ExecutionAuthorized",
            EventType::ExecutionAuthorizationRejected { .. } => "ExecutionAuthorizationRejected",
            EventType::TaskCreated { .. } => "TaskCreated",
            EventType::TaskStarted { .. } => "TaskStarted",
            EventType::TaskCompleted { .. } => "TaskCompleted",
            EventType::TaskFailed { .. } => "TaskFailed",
            EventType::TaskCancelled { .. } => "TaskCancelled",
            EventType::TaskAttemptFailed { .. } => "TaskAttemptFailed",
            EventType::TaskRetryScheduled { .. } => "TaskRetryScheduled",
            EventType::TaskSkipped { .. } => "TaskSkipped",
            EventType::TaskNeedsReview { .. } => "TaskNeedsReview",
            EventType::AgentSpawned { .. } => "AgentSpawned",
            EventType::AgentStarted { .. } => "AgentStarted",
            EventType::AgentStepCompleted { .. } => "AgentStepCompleted",
            EventType::AgentHandoffRecorded { .. } => "AgentHandoffRecorded",
            EventType::AgentCompleted { .. } => "AgentCompleted",
            EventType::AgentFailed { .. } => "AgentFailed",
            EventType::AgentCancelled { .. } => "AgentCancelled",
            EventType::ToolRequested { .. } => "ToolRequested",
            EventType::ToolCompleted { .. } => "ToolCompleted",
            EventType::ToolFailed { .. } => "ToolFailed",
            EventType::JobStarted { .. } => "JobStarted",
            EventType::JobCompleted { .. } => "JobCompleted",
            EventType::JobFailed { .. } => "JobFailed",
            EventType::VerificationStarted { .. } => "VerificationStarted",
            EventType::VerificationCompleted { .. } => "VerificationCompleted",
            EventType::ReviewRequested { .. } => "ReviewRequested",
            EventType::ReviewCompleted { .. } => "ReviewCompleted",
            EventType::PolicyEvaluated { .. } => "PolicyEvaluated",
            EventType::PolicyDenied { .. } => "PolicyDenied",
            EventType::CheckpointCreated { .. } => "CheckpointCreated",
            EventType::CheckpointRestored { .. } => "CheckpointRestored",
            EventType::ArtifactCreated { .. } => "ArtifactCreated",
            EventType::ContextCompiled { .. } => "ContextCompiled",
            EventType::PromptCompiled { .. } => "PromptCompiled",
            EventType::PromptCompilationFailed { .. } => "PromptCompilationFailed",
            EventType::RecoveryAttempted { .. } => "RecoveryAttempted",
            EventType::ControllerCycleStarted { .. } => "ControllerCycleStarted",
            EventType::ControllerStageTransitioned { .. } => "ControllerStageTransitioned",
            EventType::ControllerDecisionModeChanged { .. } => "ControllerDecisionModeChanged",
            EventType::ControllerHalted { .. } => "ControllerHalted",
            EventType::OperatorEscalationRequested { .. } => "OperatorEscalationRequested",
            EventType::EscalationTimedOut { .. } => "EscalationTimedOut",
            EventType::TaskGraphMaterialized { .. } => "TaskGraphMaterialized",
            EventType::WaveTierComputed { .. } => "WaveTierComputed",
            EventType::ResourceLeased { .. } => "ResourceLeased",
            EventType::ResourceReleased { .. } => "ResourceReleased",
            EventType::ResourceRevoked { .. } => "ResourceRevoked",
            EventType::CriticalPathRecalculated { .. } => "CriticalPathRecalculated",
            EventType::TaskBlocked { .. } => "TaskBlocked",
            EventType::TaskUnblocked { .. } => "TaskUnblocked",
            EventType::TaskNeedsReviewRequested { .. } => "TaskNeedsReviewRequested",
            EventType::TaskReviewed { .. } => "TaskReviewed",
            EventType::TaskRetried { .. } => "TaskRetried",
            EventType::TaskSuperseded { .. } => "TaskSuperseded",
            EventType::GitStateChanged { .. } => "GitStateChanged",
            EventType::ModelUsageUpdated { .. } => "ModelUsageUpdated",
            EventType::RepositoryDriftDetected { .. } => "RepositoryDriftDetected",
            EventType::WorktreeIntegrationStarted { .. } => "WorktreeIntegrationStarted",
            EventType::WorktreeIntegrated { .. } => "WorktreeIntegrated",
            EventType::WorktreeIntegrationConflict { .. } => "WorktreeIntegrationConflict",
            EventType::ApprovalResolved { .. } => "ApprovalResolved",
            EventType::WorkflowStarted { .. } => "WorkflowStarted",
            EventType::WorkflowCompleted { .. } => "WorkflowCompleted",
            EventType::WorkflowFailed { .. } => "WorkflowFailed",
            EventType::WorkflowCancelled { .. } => "WorkflowCancelled",
            EventType::WorkflowPaused { .. } => "WorkflowPaused",
            EventType::WorkflowResumed { .. } => "WorkflowResumed",
            EventType::WorkflowStepStarted { .. } => "WorkflowStepStarted",
            EventType::WorkflowStepCompleted { .. } => "WorkflowStepCompleted",
            EventType::WorkflowStepFailed { .. } => "WorkflowStepFailed",
            EventType::WorkflowStepBlocked { .. } => "WorkflowStepBlocked",
            EventType::WorkflowStepAwaitingApproval { .. } => "WorkflowStepAwaitingApproval",
            EventType::WorkflowStepAwaitingInput { .. } => "WorkflowStepAwaitingInput",
            EventType::WorkflowStepSkipped { .. } => "WorkflowStepSkipped",
            EventType::WorkflowStepRetrying { .. } => "WorkflowStepRetrying",
            EventType::WorkflowArtifactRecorded { .. } => "WorkflowArtifactRecorded",
            EventType::QualityGateEvaluated { .. } => "QualityGateEvaluated",
            EventType::GenesisStarted { .. } => "GenesisStarted",
            EventType::EnvironmentDetected { .. } => "EnvironmentDetected",
            EventType::DiscoveryStarted { .. } => "DiscoveryStarted",
            EventType::DiscoveryTurnCompleted { .. } => "DiscoveryTurnCompleted",
            EventType::DiscoveryConverged { .. } => "DiscoveryConverged",
            EventType::ProjectCharterProduced { .. } => "ProjectCharterProduced",
            EventType::ProjectCharterAwaitingApproval { .. } => "ProjectCharterAwaitingApproval",
            EventType::ResearchDecisionMade { .. } => "ResearchDecisionMade",
            EventType::ResearchDimensionStarted { .. } => "ResearchDimensionStarted",
            EventType::ResearchDimensionCompleted { .. } => "ResearchDimensionCompleted",
            EventType::ResearchDimensionFailed { .. } => "ResearchDimensionFailed",
            EventType::ResearchSynthesisStarted { .. } => "ResearchSynthesisStarted",
            EventType::ResearchSynthesisCompleted { .. } => "ResearchSynthesisCompleted",
            EventType::EngineeringDecisionRecorded { .. } => "EngineeringDecisionRecorded",
            EventType::EngineeringDecisionSuperseded { .. } => "EngineeringDecisionSuperseded",
            EventType::EngineeringAssumptionRecorded { .. } => "EngineeringAssumptionRecorded",
            EventType::EngineeringAssumptionStatusChanged { .. } => {
                "EngineeringAssumptionStatusChanged"
            }
            EventType::ReviewFindingRecorded { .. } => "ReviewFindingRecorded",
            EventType::ReviewFindingStatusChanged { .. } => "ReviewFindingStatusChanged",
            EventType::FailureDiagnosisRecorded { .. } => "FailureDiagnosisRecorded",
            EventType::VerificationRecordUpdated { .. } => "VerificationRecordUpdated",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::{
        AgentId, ArtifactId, CheckpointId, JobId, MissionId, RequirementId, TaskId, ToolCallId,
    };

    #[test]
    fn test_event_type_variants_count() {
        // Verify we have at least 30 variants (as per acceptance criteria)
        let events = vec![
            EventType::MissionStarted {
                mission_id: MissionId::new(),
                objective: "test".to_string(),
            },
            EventType::MissionCompleted {
                mission_id: MissionId::new(),
            },
            EventType::MissionFailed {
                mission_id: MissionId::new(),
                reason: "test".to_string(),
            },
            EventType::MissionCancelled {
                mission_id: MissionId::new(),
                reason: "test".to_string(),
            },
            EventType::RequirementExtracted {
                requirement_id: RequirementId::new(),
                mission_id: MissionId::new(),
                description: "test".to_string(),
            },
            EventType::RequirementVerified {
                requirement_id: RequirementId::new(),
                verified: true,
                evidence: "test".to_string(),
            },
            EventType::PlanCreated {
                plan_id: "p1".to_string(),
                mission_id: MissionId::new(),
                summary: "test".to_string(),
            },
            EventType::PlanValidated {
                plan_id: "p1".to_string(),
                valid: true,
                issues: vec![],
            },
            EventType::TaskCreated {
                task_id: TaskId::new(),
                mission_id: MissionId::new(),
                title: "test".to_string(),
                requirement_id: None,
            },
            EventType::TaskStarted {
                task_id: TaskId::new(),
                mission_id: MissionId::new(),
                agent_id: AgentId::new(),
            },
            EventType::TaskCompleted {
                task_id: TaskId::new(),
                mission_id: MissionId::new(),
                result: "test".to_string(),
            },
            EventType::TaskFailed {
                task_id: TaskId::new(),
                mission_id: MissionId::new(),
                error: "test".to_string(),
            },
            EventType::TaskCancelled {
                task_id: TaskId::new(),
                mission_id: MissionId::new(),
                reason: "test".to_string(),
            },
            EventType::AgentSpawned {
                agent_id: AgentId::new(),
                mission_id: MissionId::new(),
                role: "test".to_string(),
            },
            EventType::AgentCompleted {
                agent_id: AgentId::new(),
                mission_id: MissionId::new(),
                summary: "test".to_string(),
            },
            EventType::AgentFailed {
                agent_id: AgentId::new(),
                mission_id: MissionId::new(),
                error: "test".to_string(),
            },
            EventType::ToolRequested {
                tool_call_id: ToolCallId::new(),
                agent_id: AgentId::new(),
                tool_name: "test".to_string(),
                arguments: HashMap::new(),
            },
            EventType::ToolCompleted {
                tool_call_id: ToolCallId::new(),
                agent_id: AgentId::new(),
                result: "test".to_string(),
            },
            EventType::ToolFailed {
                tool_call_id: ToolCallId::new(),
                agent_id: AgentId::new(),
                error: "test".to_string(),
            },
            EventType::JobStarted {
                job_id: JobId::new(),
                mission_id: MissionId::new(),
                job_type: "test".to_string(),
            },
            EventType::JobCompleted {
                job_id: JobId::new(),
                mission_id: MissionId::new(),
                artifacts: vec![],
            },
            EventType::JobFailed {
                job_id: JobId::new(),
                mission_id: MissionId::new(),
                error: "test".to_string(),
            },
            EventType::VerificationStarted {
                verification_id: "v1".to_string(),
                mission_id: MissionId::new(),
                target: "test".to_string(),
            },
            EventType::VerificationCompleted {
                verification_id: "v1".to_string(),
                mission_id: MissionId::new(),
                passed: true,
                evidence: "test".to_string(),
            },
            EventType::ReviewRequested {
                review_id: "r1".to_string(),
                mission_id: MissionId::new(),
                reviewer: "test".to_string(),
            },
            EventType::ReviewCompleted {
                review_id: "r1".to_string(),
                mission_id: MissionId::new(),
                approved: true,
                comments: "test".to_string(),
            },
            EventType::PolicyEvaluated {
                policy_id: "pol1".to_string(),
                decision: "ALLOW".to_string(),
                reason: "test".to_string(),
            },
            EventType::PolicyDenied {
                policy_id: "pol1".to_string(),
                reason: "test".to_string(),
            },
            EventType::CheckpointCreated {
                checkpoint_id: CheckpointId::new(),
                mission_id: MissionId::new(),
                description: "test".to_string(),
            },
            EventType::CheckpointRestored {
                checkpoint_id: CheckpointId::new(),
                mission_id: MissionId::new(),
            },
            EventType::ArtifactCreated {
                artifact_id: ArtifactId::new(),
                mission_id: MissionId::new(),
                path: "test".to_string(),
                content_type: "text".to_string(),
            },
            EventType::ContextCompiled {
                context_id: "ctx1".to_string(),
                mission_id: MissionId::new(),
                token_count: 100,
            },
            EventType::RecoveryAttempted {
                recovery_id: "rec1".to_string(),
                mission_id: MissionId::new(),
                strategy: "test".to_string(),
                success: true,
            },
        ];
        assert!(events.len() >= 30, "Should have at least 30 event variants");
    }

    #[test]
    fn test_event_type_serde_roundtrip() {
        let mission_id = MissionId::new();
        let event = EventType::MissionStarted {
            mission_id,
            objective: "Test mission".to_string(),
        };
        let json = serde_json::to_string(&event).unwrap();
        let parsed: EventType = serde_json::from_str(&json).unwrap();
        assert_eq!(event.name(), parsed.name());
    }

    #[test]
    fn test_event_type_name() {
        let event = EventType::MissionCompleted {
            mission_id: MissionId::new(),
        };
        assert_eq!(event.name(), "MissionCompleted");

        let event = EventType::TaskFailed {
            task_id: TaskId::new(),
            mission_id: MissionId::new(),
            error: "test".to_string(),
        };
        assert_eq!(event.name(), "TaskFailed");
    }

    #[test]
    fn test_all_domains_covered() {
        // Verify all KRN-03 domains have at least one event
        let domains = [
            (
                "mission",
                vec![
                    "MissionStarted",
                    "MissionCompleted",
                    "MissionFailed",
                    "MissionCancelled",
                ],
            ),
            (
                "requirement",
                vec!["RequirementExtracted", "RequirementVerified"],
            ),
            ("plan", vec!["PlanCreated", "PlanValidated"]),
            (
                "task",
                vec![
                    "TaskCreated",
                    "TaskStarted",
                    "TaskCompleted",
                    "TaskFailed",
                    "TaskCancelled",
                ],
            ),
            (
                "agent",
                vec!["AgentSpawned", "AgentCompleted", "AgentFailed"],
            ),
            ("tool", vec!["ToolRequested", "ToolCompleted", "ToolFailed"]),
            ("job", vec!["JobStarted", "JobCompleted", "JobFailed"]),
            (
                "verification",
                vec!["VerificationStarted", "VerificationCompleted"],
            ),
            ("review", vec!["ReviewRequested", "ReviewCompleted"]),
            ("policy", vec!["PolicyEvaluated", "PolicyDenied"]),
            (
                "checkpoint",
                vec!["CheckpointCreated", "CheckpointRestored"],
            ),
            ("artifact", vec!["ArtifactCreated"]),
            ("context", vec!["ContextCompiled"]),
            ("prompt", vec!["PromptCompiled", "PromptCompilationFailed"]),
            ("recovery", vec!["RecoveryAttempted"]),
        ];

        for (_domain, expected_names) in domains {
            for name in expected_names {
                // Just verify we can construct an event of each type
                let _ = match name {
                    "MissionStarted" => EventType::MissionStarted {
                        mission_id: MissionId::new(),
                        objective: "test".to_string(),
                    },
                    "MissionCompleted" => EventType::MissionCompleted {
                        mission_id: MissionId::new(),
                    },
                    "MissionFailed" => EventType::MissionFailed {
                        mission_id: MissionId::new(),
                        reason: "test".to_string(),
                    },
                    "MissionCancelled" => EventType::MissionCancelled {
                        mission_id: MissionId::new(),
                        reason: "test".to_string(),
                    },
                    "RequirementExtracted" => EventType::RequirementExtracted {
                        requirement_id: RequirementId::new(),
                        mission_id: MissionId::new(),
                        description: "test".to_string(),
                    },
                    "RequirementVerified" => EventType::RequirementVerified {
                        requirement_id: RequirementId::new(),
                        verified: true,
                        evidence: "test".to_string(),
                    },
                    "PlanCreated" => EventType::PlanCreated {
                        plan_id: "p1".to_string(),
                        mission_id: MissionId::new(),
                        summary: "test".to_string(),
                    },
                    "PlanValidated" => EventType::PlanValidated {
                        plan_id: "p1".to_string(),
                        valid: true,
                        issues: vec![],
                    },
                    "TaskCreated" => EventType::TaskCreated {
                        task_id: TaskId::new(),
                        mission_id: MissionId::new(),
                        title: "test".to_string(),
                        requirement_id: None,
                    },
                    "TaskStarted" => EventType::TaskStarted {
                        task_id: TaskId::new(),
                        mission_id: MissionId::new(),
                        agent_id: AgentId::new(),
                    },
                    "TaskCompleted" => EventType::TaskCompleted {
                        task_id: TaskId::new(),
                        mission_id: MissionId::new(),
                        result: "test".to_string(),
                    },
                    "TaskFailed" => EventType::TaskFailed {
                        task_id: TaskId::new(),
                        mission_id: MissionId::new(),
                        error: "test".to_string(),
                    },
                    "TaskCancelled" => EventType::TaskCancelled {
                        task_id: TaskId::new(),
                        mission_id: MissionId::new(),
                        reason: "test".to_string(),
                    },
                    "AgentSpawned" => EventType::AgentSpawned {
                        agent_id: AgentId::new(),
                        mission_id: MissionId::new(),
                        role: "test".to_string(),
                    },
                    "AgentCompleted" => EventType::AgentCompleted {
                        agent_id: AgentId::new(),
                        mission_id: MissionId::new(),
                        summary: "test".to_string(),
                    },
                    "AgentFailed" => EventType::AgentFailed {
                        agent_id: AgentId::new(),
                        mission_id: MissionId::new(),
                        error: "test".to_string(),
                    },
                    "ToolRequested" => EventType::ToolRequested {
                        tool_call_id: ToolCallId::new(),
                        agent_id: AgentId::new(),
                        tool_name: "test".to_string(),
                        arguments: HashMap::new(),
                    },
                    "ToolCompleted" => EventType::ToolCompleted {
                        tool_call_id: ToolCallId::new(),
                        agent_id: AgentId::new(),
                        result: "test".to_string(),
                    },
                    "ToolFailed" => EventType::ToolFailed {
                        tool_call_id: ToolCallId::new(),
                        agent_id: AgentId::new(),
                        error: "test".to_string(),
                    },
                    "JobStarted" => EventType::JobStarted {
                        job_id: JobId::new(),
                        mission_id: MissionId::new(),
                        job_type: "test".to_string(),
                    },
                    "JobCompleted" => EventType::JobCompleted {
                        job_id: JobId::new(),
                        mission_id: MissionId::new(),
                        artifacts: vec![],
                    },
                    "JobFailed" => EventType::JobFailed {
                        job_id: JobId::new(),
                        mission_id: MissionId::new(),
                        error: "test".to_string(),
                    },
                    "VerificationStarted" => EventType::VerificationStarted {
                        verification_id: "v1".to_string(),
                        mission_id: MissionId::new(),
                        target: "test".to_string(),
                    },
                    "VerificationCompleted" => EventType::VerificationCompleted {
                        verification_id: "v1".to_string(),
                        mission_id: MissionId::new(),
                        passed: true,
                        evidence: "test".to_string(),
                    },
                    "ReviewRequested" => EventType::ReviewRequested {
                        review_id: "r1".to_string(),
                        mission_id: MissionId::new(),
                        reviewer: "test".to_string(),
                    },
                    "ReviewCompleted" => EventType::ReviewCompleted {
                        review_id: "r1".to_string(),
                        mission_id: MissionId::new(),
                        approved: true,
                        comments: "test".to_string(),
                    },
                    "PolicyEvaluated" => EventType::PolicyEvaluated {
                        policy_id: "pol1".to_string(),
                        decision: "ALLOW".to_string(),
                        reason: "test".to_string(),
                    },
                    "PolicyDenied" => EventType::PolicyDenied {
                        policy_id: "pol1".to_string(),
                        reason: "test".to_string(),
                    },
                    "CheckpointCreated" => EventType::CheckpointCreated {
                        checkpoint_id: CheckpointId::new(),
                        mission_id: MissionId::new(),
                        description: "test".to_string(),
                    },
                    "CheckpointRestored" => EventType::CheckpointRestored {
                        checkpoint_id: CheckpointId::new(),
                        mission_id: MissionId::new(),
                    },
                    "ArtifactCreated" => EventType::ArtifactCreated {
                        artifact_id: ArtifactId::new(),
                        mission_id: MissionId::new(),
                        path: "test".to_string(),
                        content_type: "text".to_string(),
                    },
                    "ContextCompiled" => EventType::ContextCompiled {
                        context_id: "ctx1".to_string(),
                        mission_id: MissionId::new(),
                        token_count: 100,
                    },
                    "PromptCompiled" => EventType::PromptCompiled {
                        mission_id: MissionId::new(),
                        task_id: TaskId::new(),
                        prompt_id: "agent.implementer".to_string(),
                        prompt_version: 1,
                        effective_prompt_hash: "test_hash".to_string(),
                        strategy: "standard".to_string(),
                        total_bytes: 1024,
                    },
                    "PromptCompilationFailed" => EventType::PromptCompilationFailed {
                        mission_id: MissionId::new(),
                        task_id: TaskId::new(),
                        prompt_id: "agent.implementer".to_string(),
                        prompt_version: 1,
                        error: "test error".to_string(),
                    },
                    "RecoveryAttempted" => EventType::RecoveryAttempted {
                        recovery_id: "rec1".to_string(),
                        mission_id: MissionId::new(),
                        strategy: "test".to_string(),
                        success: true,
                    },
                    "ControllerCycleStarted" => EventType::ControllerCycleStarted {
                        mission_id: MissionId::new(),
                        cycle: 1,
                    },
                    "ControllerStageTransitioned" => EventType::ControllerStageTransitioned {
                        mission_id: MissionId::new(),
                        cycle: 1,
                        from_stage: "Observe".to_string(),
                        to_stage: "IdentifyReadyWork".to_string(),
                    },
                    "ControllerDecisionModeChanged" => EventType::ControllerDecisionModeChanged {
                        mission_id: MissionId::new(),
                        from: crate::state_machine::AutonomyMode::Safe,
                        to: crate::state_machine::AutonomyMode::Autonomous,
                    },
                    "ControllerHalted" => EventType::ControllerHalted {
                        mission_id: MissionId::new(),
                        reason: "MissionCompleted".to_string(),
                    },
                    "OperatorEscalationRequested" => EventType::OperatorEscalationRequested {
                        request_id: "req1".to_string(),
                        mission_id: MissionId::new(),
                        reason: "needs approval".to_string(),
                        timeout_seconds: Some(60),
                        tool_name: Some("exec_command".to_string()),
                        parameters_summary: Some("cargo test".to_string()),
                        risk_tier: Some("High".to_string()),
                        agent_role: Some("coder".to_string()),
                    },
                    "EscalationTimedOut" => EventType::EscalationTimedOut {
                        request_id: "req1".to_string(),
                        mission_id: MissionId::new(),
                    },
                    _ => panic!("Unknown event: {}", name),
                };
            }
        }
    }

    #[test]
    fn test_controller_lifecycle_events_serde_roundtrip() {
        let mission_id = MissionId::new();
        let events = vec![
            EventType::ControllerCycleStarted {
                mission_id,
                cycle: 42,
            },
            EventType::ControllerStageTransitioned {
                mission_id,
                cycle: 42,
                from_stage: "Observe".into(),
                to_stage: "IdentifyReadyWork".into(),
            },
            EventType::ControllerDecisionModeChanged {
                mission_id,
                from: crate::state_machine::AutonomyMode::Safe,
                to: crate::state_machine::AutonomyMode::Autonomous,
            },
            EventType::ControllerHalted {
                mission_id,
                reason: "Cancelled".into(),
            },
            EventType::OperatorEscalationRequested {
                request_id: "esc-123".into(),
                mission_id,
                reason: "Sensitive operation".into(),
                timeout_seconds: Some(120),
                tool_name: Some("bash".into()),
                parameters_summary: Some("rm -rf".into()),
                risk_tier: Some("Critical".into()),
                agent_role: Some("executor".into()),
            },
            EventType::EscalationTimedOut {
                request_id: "esc-123".into(),
                mission_id,
            },
        ];

        for event in events {
            let serialized = serde_json::to_string(&event).unwrap();
            let deserialized: EventType = serde_json::from_str(&serialized).unwrap();
            assert_eq!(event, deserialized);
        }
    }

    #[test]
    fn test_scheduler_events_serde_roundtrip() {
        let mission_id = MissionId::new();
        let graph_id = TaskGraphId::new();
        let task_id = TaskId::new();
        let lease_id = uuid::Uuid::now_v7();

        let events = vec![
            EventType::TaskGraphMaterialized {
                graph_id,
                mission_id,
                revision: 1,
                task_count: 5,
                tasks: vec![],
            },
            EventType::WaveTierComputed {
                graph_id,
                wave_index: 0,
                task_ids: vec![task_id],
            },
            EventType::ResourceLeased {
                lease_id,
                task_id,
                resource_key: "workspace/src/lib.rs".to_string(),
                lock_mode: "Exclusive".to_string(),
            },
            EventType::ResourceReleased {
                lease_id,
                task_id,
                resource_key: "workspace/src/lib.rs".to_string(),
            },
            EventType::ResourceRevoked {
                lease_id,
                task_id,
                resource_key: "workspace/src/lib.rs".to_string(),
                reason: "Timeout".to_string(),
            },
            EventType::CriticalPathRecalculated {
                graph_id,
                critical_tasks: vec![task_id],
                projected_duration_secs: 120,
            },
            EventType::TaskBlocked {
                task_id,
                mission_id,
                reason: "Dependency failed".to_string(),
            },
            EventType::TaskUnblocked {
                task_id,
                mission_id,
            },
            EventType::TaskNeedsReviewRequested {
                task_id,
                mission_id,
            },
            EventType::TaskReviewed {
                task_id,
                mission_id,
                approved: true,
                reviewer: "architect".to_string(),
            },
            EventType::TaskRetried {
                task_id,
                mission_id,
                attempt: 1,
                max_retries: 3,
            },
            EventType::TaskSuperseded {
                task_id,
                mission_id,
                replacement_task_id: Some(TaskId::new()),
            },
        ];

        for event in events {
            assert!(!event.name().is_empty());
            let serialized = serde_json::to_string(&event).unwrap();
            let deserialized: EventType = serde_json::from_str(&serialized).unwrap();
            assert_eq!(event, deserialized);
        }
    }

    #[test]
    fn test_engineering_memory_events_serde_roundtrip() {
        let mission_id = MissionId::new();
        let task_id = TaskId::new();
        let event_id = uuid::Uuid::now_v7();

        let events = vec![
            EventType::EngineeringDecisionRecorded {
                id: "ADR-0001".to_string(),
                scope: "project".to_string(),
                title: "Use SQLite for Persistence".to_string(),
                status: "accepted".to_string(),
            },
            EventType::EngineeringDecisionSuperseded {
                id: "ADR-0001".to_string(),
                superseded_by: "ADR-0002".to_string(),
            },
            EventType::EngineeringAssumptionRecorded {
                id: event_id,
                mission_id,
                statement: "AuthService owns token generation".to_string(),
                status: "active".to_string(),
            },
            EventType::EngineeringAssumptionStatusChanged {
                id: event_id,
                mission_id,
                from_status: "active".to_string(),
                to_status: "invalidated".to_string(),
                reason: Some("File refactored".to_string()),
            },
            EventType::ReviewFindingRecorded {
                id: event_id,
                mission_id,
                task_id,
                file_path: "src/auth.rs".to_string(),
                severity: "warning".to_string(),
            },
            EventType::ReviewFindingStatusChanged {
                id: event_id,
                mission_id,
                status: "addressed".to_string(),
                rationale: Some("Fixed input validation".to_string()),
            },
            EventType::FailureDiagnosisRecorded {
                id: event_id,
                mission_id,
                task_id,
                failure_signature: "sig-123".to_string(),
                failure_class: "compilation".to_string(),
            },
            EventType::VerificationRecordUpdated {
                id: event_id,
                mission_id,
                requirement_key: "REQ-01".to_string(),
                validity: "valid".to_string(),
            },
        ];

        for event in events {
            assert!(!event.name().is_empty());
            let serialized = serde_json::to_string(&event).unwrap();
            let deserialized: EventType = serde_json::from_str(&serialized).unwrap();
            assert_eq!(event, deserialized);
        }
    }
}
