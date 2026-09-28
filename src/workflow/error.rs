//! Typed error model for workflow definitions, state transitions, and persistence.

use crate::error::M31AError;
use crate::ids::WorkflowRunId;
use thiserror::Error;

/// Strongly typed workflow error category.
#[derive(Debug, Error)]
pub enum WorkflowError {
    /// Declarative workflow definition failed structural or semantic validation.
    #[error("invalid workflow definition: {0}")]
    InvalidDefinition(String),

    /// State transition attempted between incompatible workflow states.
    #[error("invalid workflow state transition from '{from}' to '{to}': {reason}")]
    InvalidTransition {
        from: String,
        to: String,
        reason: String,
    },

    /// A workflow step references a non-existent upstream dependency.
    #[error("missing workflow dependency '{prerequisite}' required by step '{step}'")]
    MissingDependency { step: String, prerequisite: String },

    /// A workflow definition contains duplicate step keys.
    #[error("duplicate workflow step key '{0}'")]
    DuplicateStep(String),

    /// A cyclic dependency was detected among workflow steps.
    #[error("cyclic dependency detected in workflow: {cycle:?}")]
    CycleDetected { cycle: Vec<String> },

    /// An input or output binding is malformed or references an invalid target.
    #[error("invalid binding in step '{step}' for parameter/artifact '{binding}': {reason}")]
    InvalidBinding {
        step: String,
        binding: String,
        reason: String,
    },

    /// An artifact path violates safety constraints (e.g. absolute, traversal, internal escape).
    #[error("invalid artifact path '{path}': {reason}")]
    InvalidArtifactPath { path: String, reason: String },

    /// SQLite storage or retrieval failure.
    #[error("workflow persistence failure: {0}")]
    PersistenceFailure(String),

    /// Requested workflow run ID does not exist.
    #[error("unknown workflow run '{0}'")]
    UnknownWorkflow(WorkflowRunId),

    /// Requested workflow definition ID does not exist.
    #[error("unknown workflow definition '{0}'")]
    UnknownWorkflowDefinition(String),

    /// Requested workflow step key was not found in the run.
    #[error("unknown workflow step '{step_key}' in run '{run_id}'")]
    UnknownStep {
        run_id: WorkflowRunId,
        step_key: String,
    },

    /// The workflow is in an invalid or corrupted state.
    #[error("invalid workflow state: {0}")]
    InvalidState(String),

    /// Underlying SQLx database error.
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),

    /// JSON serialization or deserialization error.
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// Workflow manifest failed to parse as valid TOML.
    #[error("manifest parse failure: {0}")]
    ManifestParse(String),

    /// Workflow manifest failed semantic validation rules.
    #[error(
        "manifest validation error in workflow '{}', step '{}', field '{}': {reason}",
        workflow_id.as_deref().unwrap_or("<unknown>"),
        step_key.as_deref().unwrap_or("<global>"),
        field.as_deref().unwrap_or("<unknown>")
    )]
    ManifestValidation {
        path: Option<String>,
        workflow_id: Option<String>,
        step_key: Option<String>,
        field: Option<String>,
        reason: String,
    },

    /// Requested prompt contract was not found in the catalog.
    #[error("prompt contract '{id}' (version {version}) not found")]
    PromptNotFound { id: String, version: u32 },

    /// Attempted to register a duplicate prompt contract with inconsistent content.
    #[error("duplicate prompt contract '{id}' (version {version}): {reason}")]
    PromptDuplicate {
        id: String,
        version: u32,
        reason: String,
    },

    /// Prompt contract is structurally or semantically invalid.
    #[error("invalid prompt contract '{id}' (version {version}): {reason}")]
    PromptInvalid {
        id: String,
        version: u32,
        reason: String,
    },

    /// Required parameter for prompt template was missing.
    #[error("missing required prompt parameter '{parameter}' for prompt '{prompt_id}'")]
    PromptParameterMissing {
        prompt_id: String,
        parameter: String,
    },

    /// Prompt parameter failed validation.
    #[error("invalid parameter '{parameter}' for prompt '{prompt_id}': {reason}")]
    PromptParameterInvalid {
        prompt_id: String,
        parameter: String,
        reason: String,
    },

    /// Template rendering error.
    #[error("failed to render prompt '{prompt_id}': {reason}")]
    PromptRenderFailure { prompt_id: String, reason: String },

    /// Prompt version mismatch between step expectation and catalog.
    #[error(
        "version mismatch for prompt '{prompt_id}': requested {requested}, available {available}"
    )]
    PromptVersionMismatch {
        prompt_id: String,
        requested: u32,
        available: u32,
    },

    /// Agent role mismatch between workflow step and prompt contract.
    #[error(
        "role mismatch in step '{step_key}': step role '{step_role}' is incompatible with prompt '{prompt_id}' role '{prompt_role}'"
    )]
    RoleMismatch {
        step_key: String,
        step_role: String,
        prompt_id: String,
        prompt_role: String,
    },

    /// Quality gate verification failed.
    #[error("quality gate failed for step '{step_key}': {violations:?}")]
    QualityGateFailure {
        step_key: String,
        violations: Vec<String>,
    },

    /// Quality gate configuration error.
    #[error("quality gate configuration error: {reason}")]
    QualityGateConfiguration { reason: String },

    /// Provenance calculation or verification failure.
    #[error("provenance failure: {0}")]
    ProvenanceFailure(String),

    /// Path traversal or boundary escape detected.
    #[error("path security violation for '{path}': {reason}")]
    PathViolation { path: String, reason: String },

    /// Unsupported manifest or schema version.
    #[error("unsupported {kind} version {version} (supported: {supported})")]
    UnsupportedVersion {
        kind: String,
        version: u32,
        supported: String,
    },

    /// Workflow run is already active or in progress.
    #[error("workflow run '{0}' is already running")]
    WorkflowAlreadyRunning(WorkflowRunId),

    /// Step is not ready for execution due to unmet conditions.
    #[error("step '{step_key}' is not ready: {reason}")]
    StepNotReady { step_key: String, reason: String },

    /// Step dependency is not satisfied.
    #[error(
        "dependency '{dependency}' for step '{step_key}' is not satisfied (status: '{status}')"
    )]
    DependencyNotSatisfied {
        step_key: String,
        dependency: String,
        status: String,
    },

    /// A required workflow artifact is unavailable or missing.
    #[error("required artifact '{artifact_name}' unavailable for step '{step_key}'")]
    ArtifactUnavailable {
        artifact_name: String,
        step_key: String,
    },

    /// An artifact is invalid or corrupted.
    #[error("artifact '{artifact_name}' is invalid: {reason}")]
    ArtifactInvalid {
        artifact_name: String,
        reason: String,
    },

    /// Bridge failed to create underlying Mission.
    #[error("mission creation failed for step '{step_key}': {reason}")]
    MissionCreationFailed { step_key: String, reason: String },

    /// Bridge failed to submit or schedule task in TaskGraph.
    #[error("task submission failed for step '{step_key}': {reason}")]
    TaskSubmissionFailed { step_key: String, reason: String },

    /// Execution of underlying task through worker dispatcher failed.
    #[error("execution failed for step '{step_key}': {reason}")]
    ExecutionFailed { step_key: String, reason: String },

    /// Step requires human operator approval before completion.
    #[error("operator approval required for step '{step_key}': {prompt}")]
    ApprovalRequired { step_key: String, prompt: String },

    /// Step requires human operator input before execution can continue.
    #[error("operator input required for step '{step_key}': {prompt}")]
    InputRequired { step_key: String, prompt: String },

    /// Crash or state recovery failed for workflow run.
    #[error("recovery failed for workflow run '{run_id}': {reason}")]
    RecoveryFailed {
        run_id: WorkflowRunId,
        reason: String,
    },

    /// Cancellation operation failed.
    #[error("cancellation failed for workflow run '{run_id}': {reason}")]
    CancellationFailed {
        run_id: WorkflowRunId,
        reason: String,
    },

    /// Stale execution callback received from a superseded attempt.
    #[error(
        "stale execution for step '{step_key}': expected attempt {expected_attempt}, received {received_attempt}"
    )]
    StaleExecution {
        step_key: String,
        expected_attempt: u32,
        received_attempt: u32,
    },

    /// Invariant violation within the workflow runtime.
    #[error("workflow invariant violation: {0}")]
    WorkflowInvariantViolation(String),

    /// Resource budget exceeded for workflow execution.
    #[error("workflow budget exceeded: {0}")]
    BudgetExceeded(String),

    /// Policy gate denied execution of step or capability.
    #[error("workflow policy denied: {0}")]
    PolicyDenied(String),
}

impl From<WorkflowError> for M31AError {
    fn from(err: WorkflowError) -> Self {
        match err {
            WorkflowError::InvalidTransition { from, to, reason } => M31AError::transition(
                format!("transition from {} to {} failed: {}", from, to, reason),
            ),
            WorkflowError::PersistenceFailure(msg) => M31AError::persistence(msg),
            WorkflowError::UnknownWorkflow(id) => {
                M31AError::not_found(format!("workflow run {}", id))
            }
            WorkflowError::UnknownWorkflowDefinition(id) => {
                M31AError::not_found(format!("workflow definition {}", id))
            }
            WorkflowError::UnknownStep { run_id, step_key } => {
                M31AError::not_found(format!("workflow step {} in run {}", step_key, run_id))
            }
            WorkflowError::InvalidDefinition(msg) => M31AError::validation(msg),
            WorkflowError::MissingDependency { step, prerequisite } => {
                M31AError::validation(format!(
                    "step '{}' depends on missing prerequisite '{}'",
                    step, prerequisite
                ))
            }
            WorkflowError::DuplicateStep(key) => {
                M31AError::validation(format!("duplicate step key '{}'", key))
            }
            WorkflowError::CycleDetected { cycle } => {
                M31AError::validation(format!("cycle detected: {:?}", cycle))
            }
            WorkflowError::InvalidBinding {
                step,
                binding,
                reason,
            } => M31AError::validation(format!(
                "invalid binding '{}' in step '{}': {}",
                binding, step, reason
            )),
            WorkflowError::InvalidArtifactPath { path, reason } => {
                M31AError::validation(format!("invalid artifact path '{}': {}", path, reason))
            }
            WorkflowError::InvalidState(msg) => M31AError::reconstruction(msg),
            WorkflowError::Database(e) => M31AError::Database(e),
            WorkflowError::Serialization(e) => M31AError::internal(e.to_string()),
            WorkflowError::ManifestParse(msg) => M31AError::validation(msg),
            WorkflowError::ManifestValidation {
                workflow_id,
                step_key,
                field,
                reason,
                ..
            } => M31AError::validation(format!(
                "manifest validation error in workflow '{}', step '{}', field '{}': {}",
                workflow_id.as_deref().unwrap_or("<unknown>"),
                step_key.as_deref().unwrap_or("<global>"),
                field.as_deref().unwrap_or("<unknown>"),
                reason
            )),
            WorkflowError::PromptNotFound { id, version } => {
                M31AError::not_found(format!("prompt contract '{}:v{}'", id, version))
            }
            WorkflowError::PromptDuplicate {
                id,
                version,
                reason,
            } => M31AError::validation(format!(
                "duplicate prompt contract '{}:v{}': {}",
                id, version, reason
            )),
            WorkflowError::PromptInvalid {
                id,
                version,
                reason,
            } => M31AError::validation(format!(
                "invalid prompt contract '{}:v{}': {}",
                id, version, reason
            )),
            WorkflowError::PromptParameterMissing {
                prompt_id,
                parameter,
            } => M31AError::validation(format!(
                "missing required parameter '{}' for prompt '{}'",
                parameter, prompt_id
            )),
            WorkflowError::PromptParameterInvalid {
                prompt_id,
                parameter,
                reason,
            } => M31AError::validation(format!(
                "invalid parameter '{}' for prompt '{}': {}",
                parameter, prompt_id, reason
            )),
            WorkflowError::PromptRenderFailure { prompt_id, reason } => M31AError::internal(
                format!("render failure in prompt '{}': {}", prompt_id, reason),
            ),
            WorkflowError::PromptVersionMismatch {
                prompt_id,
                requested,
                available,
            } => M31AError::validation(format!(
                "prompt version mismatch for '{}': requested {}, available {}",
                prompt_id, requested, available
            )),
            WorkflowError::RoleMismatch {
                step_key,
                step_role,
                prompt_id,
                prompt_role,
            } => M31AError::validation(format!(
                "role mismatch in step '{}': step role '{}' != prompt '{}' role '{}'",
                step_key, step_role, prompt_id, prompt_role
            )),
            WorkflowError::QualityGateFailure {
                step_key,
                violations,
            } => M31AError::validation(format!(
                "quality gate failure for step '{}': {:?}",
                step_key, violations
            )),
            WorkflowError::QualityGateConfiguration { reason } => {
                M31AError::validation(format!("quality gate configuration error: {}", reason))
            }
            WorkflowError::ProvenanceFailure(msg) => M31AError::validation(msg),
            WorkflowError::PathViolation { path, reason } => {
                M31AError::validation(format!("path violation for '{}': {}", path, reason))
            }
            WorkflowError::UnsupportedVersion {
                kind,
                version,
                supported,
            } => M31AError::validation(format!(
                "unsupported {} version {} (supported: {})",
                kind, version, supported
            )),
            WorkflowError::WorkflowAlreadyRunning(id) => {
                M31AError::validation(format!("workflow run {} is already running", id))
            }
            WorkflowError::StepNotReady { step_key, reason } => {
                M31AError::validation(format!("step '{}' is not ready: {}", step_key, reason))
            }
            WorkflowError::DependencyNotSatisfied {
                step_key,
                dependency,
                status,
            } => M31AError::validation(format!(
                "dependency '{}' for step '{}' not satisfied (status: {})",
                dependency, step_key, status
            )),
            WorkflowError::ArtifactUnavailable {
                artifact_name,
                step_key,
            } => M31AError::not_found(format!(
                "artifact '{}' unavailable for step '{}'",
                artifact_name, step_key
            )),
            WorkflowError::ArtifactInvalid {
                artifact_name,
                reason,
            } => M31AError::validation(format!("artifact '{}' invalid: {}", artifact_name, reason)),
            WorkflowError::MissionCreationFailed { step_key, reason } => {
                M31AError::internal(format!(
                    "mission creation failed for step '{}': {}",
                    step_key, reason
                ))
            }
            WorkflowError::TaskSubmissionFailed { step_key, reason } => M31AError::internal(
                format!("task submission failed for step '{}': {}", step_key, reason),
            ),
            WorkflowError::ExecutionFailed { step_key, reason } => M31AError::internal(format!(
                "execution failed for step '{}': {}",
                step_key, reason
            )),
            WorkflowError::ApprovalRequired { step_key, prompt } => M31AError::validation(format!(
                "operator approval required for step '{}': {}",
                step_key, prompt
            )),
            WorkflowError::InputRequired { step_key, prompt } => M31AError::validation(format!(
                "operator input required for step '{}': {}",
                step_key, prompt
            )),
            WorkflowError::RecoveryFailed { run_id, reason } => M31AError::reconstruction(format!(
                "recovery failed for workflow run {}: {}",
                run_id, reason
            )),
            WorkflowError::CancellationFailed { run_id, reason } => {
                M31AError::cancellation(format!(
                    "cancellation failed for workflow run {}: {}",
                    run_id, reason
                ))
            }
            WorkflowError::StaleExecution {
                step_key,
                expected_attempt,
                received_attempt,
            } => M31AError::validation(format!(
                "stale execution for step '{}': expected attempt {}, received {}",
                step_key, expected_attempt, received_attempt
            )),
            WorkflowError::WorkflowInvariantViolation(msg) => M31AError::internal(msg),
            WorkflowError::BudgetExceeded(msg) => M31AError::validation(msg),
            WorkflowError::PolicyDenied(msg) => M31AError::validation(msg),
        }
    }
}

impl From<crate::prompt::PromptError> for WorkflowError {
    fn from(err: crate::prompt::PromptError) -> Self {
        match err {
            crate::prompt::PromptError::PromptNotFound { id, version } => {
                WorkflowError::PromptNotFound { id, version }
            }
            crate::prompt::PromptError::PromptDuplicate {
                id,
                version,
                reason,
            } => WorkflowError::PromptDuplicate {
                id,
                version,
                reason,
            },
            crate::prompt::PromptError::PromptInvalid {
                id,
                version,
                reason,
            } => WorkflowError::PromptInvalid {
                id,
                version,
                reason,
            },
            crate::prompt::PromptError::PromptParameterMissing {
                prompt_id,
                parameter,
            } => WorkflowError::PromptParameterMissing {
                prompt_id,
                parameter,
            },
            crate::prompt::PromptError::PromptParameterInvalid {
                prompt_id,
                parameter,
                reason,
            } => WorkflowError::PromptParameterInvalid {
                prompt_id,
                parameter,
                reason,
            },
            crate::prompt::PromptError::PromptRenderFailure { prompt_id, reason } => {
                WorkflowError::PromptRenderFailure { prompt_id, reason }
            }
            crate::prompt::PromptError::PromptVersionMismatch {
                prompt_id,
                requested,
                available,
            } => WorkflowError::PromptVersionMismatch {
                prompt_id,
                requested,
                available,
            },
            crate::prompt::PromptError::PathViolation { path, reason } => {
                WorkflowError::PathViolation { path, reason }
            }
            crate::prompt::PromptError::PromptBudgetExceeded {
                prompt_id, reason, ..
            } => WorkflowError::PromptRenderFailure { prompt_id, reason },
            crate::prompt::PromptError::PromptCompositionError { prompt_id, reason } => {
                WorkflowError::PromptRenderFailure { prompt_id, reason }
            }
            crate::prompt::PromptError::PromptSecurityViolation { prompt_id, reason } => {
                WorkflowError::PromptRenderFailure { prompt_id, reason }
            }
        }
    }
}
