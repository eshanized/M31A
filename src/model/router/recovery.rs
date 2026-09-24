//! Two-level bounded model recovery coordinator (D-07, MDL-04).
//!
//! Level 1: In-step transient retry loop strictly bounded by configured budget (default 2 retries).
//! Level 2: Persistent failure or retry exhaustion escalates to Model Fallback via ModelRouter.
//! Fallback re-evaluates hard capability requirements and NEVER weakens task or role constraints (T-07-06).

use serde::{Deserialize, Serialize};

use crate::ids::{AgentId, MissionId, TaskId};
use crate::model::persistence::invocation::{
    ModelInvocationRecord, SqliteModelInvocationRepository,
};
use crate::model::router::health::{CircuitBreakerRegistry, CircuitState};
use crate::model::router::resolver::{
    ModelCandidate, ModelResolutionError, ModelRouter, ResolvedModelSelection, RoutingRequest,
};
use crate::model::types::{ModelError, TokenUsage};

/// Recovery action decided by the two-level recovery protocol (D-07).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum StepRecoveryDecision {
    /// Level 1: Retry the active model in-step with backoff / cooldown.
    RetryInStep {
        model_name: String,
        next_attempt_number: u32,
        cooldown_secs: u64,
        reason: String,
    },
    /// Level 2: Failover to a new eligible model selection.
    FailoverModel {
        previous_model: String,
        fallback_selection: ResolvedModelSelection,
        fallback_attempt_number: u32,
        fallback_count: u32,
        reason: String,
    },
    /// Terminal failure: cannot retry and no eligible fallback models exist.
    EscalateTerminal {
        last_error: ModelError,
        reason: String,
    },
}

/// Two-level bounded recovery coordinator managing in-step retries and fallback failover (D-07).
#[derive(Debug, Clone)]
pub struct ModelRecoveryCoordinator {
    router: ModelRouter,
    max_in_step_retries: u32,
    max_model_fallbacks: u32,
}

impl Default for ModelRecoveryCoordinator {
    fn default() -> Self {
        Self::new()
    }
}

impl ModelRecoveryCoordinator {
    /// Create a new recovery coordinator with default bounds (2 in-step retries, 3 model fallbacks).
    pub fn new() -> Self {
        Self {
            router: ModelRouter::new(),
            max_in_step_retries: 2,
            max_model_fallbacks: 3,
        }
    }

    /// Builder to customize retry and fallback bounds.
    pub fn with_bounds(mut self, max_in_step_retries: u32, max_model_fallbacks: u32) -> Self {
        self.max_in_step_retries = max_in_step_retries;
        self.max_model_fallbacks = max_model_fallbacks;
        self
    }

    pub fn max_in_step_retries(&self) -> u32 {
        self.max_in_step_retries
    }

    pub fn max_model_fallbacks(&self) -> u32 {
        self.max_model_fallbacks
    }

    /// Evaluate failure outcome and determine next recovery action (D-07, T-07-06, T-07-08).
    #[allow(clippy::too_many_arguments)]
    pub fn handle_step_failure(
        &self,
        current_attempt: u32,
        fallback_count: u32,
        error: &ModelError,
        current_selection: &ResolvedModelSelection,
        request: &RoutingRequest,
        candidates: &[ModelCandidate],
        health_registry: &CircuitBreakerRegistry,
    ) -> StepRecoveryDecision {
        // Record failure in health registry first
        health_registry.record_failure(
            &current_selection.provider,
            &current_selection.model_name,
            error,
        );

        let is_transient = matches!(
            error,
            ModelError::Network(_)
                | ModelError::StreamInterrupted(_)
                | ModelError::RateLimited { .. }
                | ModelError::Http {
                    status: 429 | 500..=599,
                    ..
                }
        );

        let current_health = {
            let state = health_registry
                .get_state(&current_selection.provider, &current_selection.model_name);
            if state != CircuitState::Healthy {
                state
            } else {
                health_registry.get_state_for_model(&current_selection.model_name)
            }
        };

        // Level 1: In-step transient retry against same model
        // Allowed if error is transient, circuit is not Open, and attempt <= max_in_step_retries
        if is_transient
            && current_health != CircuitState::Open
            && current_attempt <= self.max_in_step_retries
        {
            let cooldown_secs = match error {
                ModelError::RateLimited { cooldown_secs } => *cooldown_secs,
                _ => 1,
            };

            return StepRecoveryDecision::RetryInStep {
                model_name: current_selection.model_name.clone(),
                next_attempt_number: current_attempt + 1,
                cooldown_secs,
                reason: format!(
                    "Level 1 retry in-step (attempt {}/{}): {}",
                    current_attempt + 1,
                    self.max_in_step_retries + 1,
                    error
                ),
            };
        }

        // Level 2: Persistent failure or retries exhausted -> Escalate to Model Fallback
        if fallback_count >= self.max_model_fallbacks {
            return StepRecoveryDecision::EscalateTerminal {
                last_error: error.clone(),
                reason: format!(
                    "Level 2 fallback ceiling reached ({} fallbacks exhausted)",
                    self.max_model_fallbacks
                ),
            };
        }

        // Filter candidate pool: exclude current failing model
        let fallback_candidates: Vec<ModelCandidate> = candidates
            .iter()
            .filter(|c| c.model_id != current_selection.model_name)
            .cloned()
            .collect();

        // Hard capability requirements are re-evaluated and cannot be weakened (D-07, T-07-06)
        match self
            .router
            .resolve_model(request, &fallback_candidates, health_registry)
        {
            Ok(fallback_selection) => StepRecoveryDecision::FailoverModel {
                previous_model: current_selection.model_name.clone(),
                fallback_selection,
                fallback_attempt_number: 1, // Model switching resets attempt accounting (D-07)
                fallback_count: fallback_count + 1,
                reason: format!(
                    "Level 2 failover from {} to next eligible model after failure: {}",
                    current_selection.model_name, error
                ),
            },
            Err(ModelResolutionError::NoEligibleModel(reason)) => {
                StepRecoveryDecision::EscalateTerminal {
                    last_error: error.clone(),
                    reason: format!(
                        "Level 2 fallback resolution failed (hard constraints not satisfied): {}",
                        reason
                    ),
                }
            }
        }
    }

    /// Helper to record an authoritative invocation attempt into durable SQLite telemetry (D-08).
    #[allow(clippy::too_many_arguments)]
    pub async fn record_telemetry_attempt(
        telemetry: &SqliteModelInvocationRepository,
        mission_id: MissionId,
        task_id: TaskId,
        agent_id: AgentId,
        step_number: u32,
        selection: &ResolvedModelSelection,
        attempt_number: u32,
        outcome: &str,
        usage: &TokenUsage,
        routing_reason: &str,
    ) -> Result<ModelInvocationRecord, sqlx::Error> {
        let record = ModelInvocationRecord::new(
            mission_id,
            task_id,
            agent_id,
            step_number,
            &selection.provider,
            &selection.model_name,
            attempt_number,
            outcome,
            usage,
            routing_reason,
        );
        telemetry.insert_invocation(&record).await?;
        Ok(record)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::router::resolver::ModelTier;
    use crate::state_machine::agent::AgentRole;
    use tempfile::tempdir;

    fn test_candidates() -> Vec<ModelCandidate> {
        vec![
            ModelCandidate::new(
                "meta/llama-3.1-8b-instruct",
                "nvidia",
                ModelTier::Fast,
                8192,
            ),
            ModelCandidate::new(
                "meta/llama-3.3-70b-instruct",
                "nvidia",
                ModelTier::Standard,
                32768,
            ),
            ModelCandidate::new(
                "deepseek-ai/deepseek-r1",
                "nvidia",
                ModelTier::Reasoning,
                65536,
            ),
        ]
    }

    #[test]
    fn test_level_1_transient_failure_retries_in_step() {
        let coordinator = ModelRecoveryCoordinator::new();
        let registry = CircuitBreakerRegistry::new();
        let candidates = test_candidates();
        let request = RoutingRequest::new(AgentRole::implementer(), ModelTier::Fast);

        let current_selection = ResolvedModelSelection {
            provider: "nvidia".to_string(),
            model_name: "meta/llama-3.1-8b-instruct".to_string(),
            temperature: 0.2,
            max_tokens: 8192,
        };

        let err = ModelError::Network("connection reset".to_string());

        // Attempt 1 -> in-step retry with next_attempt_number = 2
        let decision = coordinator.handle_step_failure(
            1,
            0,
            &err,
            &current_selection,
            &request,
            &candidates,
            &registry,
        );

        match decision {
            StepRecoveryDecision::RetryInStep {
                model_name,
                next_attempt_number,
                ..
            } => {
                assert_eq!(model_name, "meta/llama-3.1-8b-instruct");
                assert_eq!(next_attempt_number, 2);
            }
            other => panic!("expected RetryInStep, got {:?}", other),
        }
    }

    #[test]
    fn test_level_1_retry_exhaustion_escalates_to_level_2_fallback() {
        let coordinator = ModelRecoveryCoordinator::new().with_bounds(2, 3);
        let registry = CircuitBreakerRegistry::new();
        let candidates = test_candidates();
        let request = RoutingRequest::new(AgentRole::implementer(), ModelTier::Fast);

        let current_selection = ResolvedModelSelection {
            provider: "nvidia".to_string(),
            model_name: "meta/llama-3.1-8b-instruct".to_string(),
            temperature: 0.2,
            max_tokens: 8192,
        };

        let err = ModelError::Network("timeout".to_string());

        // Attempt 3 (since max_retries is 2, attempts 1 and 2 retried; attempt 3 exhausts retries)
        let decision = coordinator.handle_step_failure(
            3,
            0,
            &err,
            &current_selection,
            &request,
            &candidates,
            &registry,
        );

        match decision {
            StepRecoveryDecision::FailoverModel {
                previous_model,
                fallback_selection,
                fallback_attempt_number,
                fallback_count,
                ..
            } => {
                assert_eq!(previous_model, "meta/llama-3.1-8b-instruct");
                // Selected next eligible candidate (meta/llama-3.3-70b-instruct)
                assert_eq!(fallback_selection.model_name, "meta/llama-3.3-70b-instruct");
                assert_eq!(fallback_attempt_number, 1);
                assert_eq!(fallback_count, 1);
            }
            other => panic!("expected FailoverModel, got {:?}", other),
        }
    }

    #[test]
    fn test_circuit_open_immediately_escalates_to_level_2_fallback() {
        let coordinator = ModelRecoveryCoordinator::new();
        let registry = CircuitBreakerRegistry::new();
        let candidates = test_candidates();
        let request = RoutingRequest::new(AgentRole::implementer(), ModelTier::Fast);

        let current_selection = ResolvedModelSelection {
            provider: "nvidia".to_string(),
            model_name: "meta/llama-3.1-8b-instruct".to_string(),
            temperature: 0.2,
            max_tokens: 8192,
        };

        // 429 RateLimit trips circuit to Open immediately
        let err = ModelError::RateLimited { cooldown_secs: 60 };

        let decision = coordinator.handle_step_failure(
            1,
            0,
            &err,
            &current_selection,
            &request,
            &candidates,
            &registry,
        );

        match decision {
            StepRecoveryDecision::FailoverModel {
                previous_model,
                fallback_selection,
                fallback_attempt_number,
                ..
            } => {
                assert_eq!(previous_model, "meta/llama-3.1-8b-instruct");
                assert_eq!(fallback_selection.model_name, "meta/llama-3.3-70b-instruct");
                assert_eq!(fallback_attempt_number, 1);
            }
            other => panic!("expected FailoverModel, got {:?}", other),
        }
    }

    #[test]
    fn test_fallback_fails_closed_when_hard_constraints_not_met() {
        let coordinator = ModelRecoveryCoordinator::new();
        let registry = CircuitBreakerRegistry::new();

        // Pool with only 1 model
        let candidates = vec![ModelCandidate::new(
            "meta/llama-3.1-8b-instruct",
            "nvidia",
            ModelTier::Fast,
            8192,
        )];

        let request = RoutingRequest::new(AgentRole::implementer(), ModelTier::Fast);
        let current_selection = ResolvedModelSelection {
            provider: "nvidia".to_string(),
            model_name: "meta/llama-3.1-8b-instruct".to_string(),
            temperature: 0.2,
            max_tokens: 8192,
        };

        let err = ModelError::Network("server down".to_string());

        // Exhaust in-step retries
        let decision = coordinator.handle_step_failure(
            3,
            0,
            &err,
            &current_selection,
            &request,
            &candidates,
            &registry,
        );

        // No fallback models available; must escalate terminally rather than weaken requirements
        match decision {
            StepRecoveryDecision::EscalateTerminal { reason, .. } => {
                assert!(reason.contains("hard constraints not satisfied"));
            }
            other => panic!("expected EscalateTerminal, got {:?}", other),
        }
    }

    async fn seed_test_parents(
        pool: &sqlx::SqlitePool,
        mission_id: &MissionId,
        task_id: &TaskId,
        agent_id: &AgentId,
    ) {
        let now = chrono::Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT INTO missions (id, objective, status, created_at, updated_at) VALUES (?, ?, ?, ?, ?)",
        )
        .bind(mission_id.as_bytes().as_slice())
        .bind("test mission")
        .bind("active")
        .bind(&now)
        .bind(&now)
        .execute(pool)
        .await
        .unwrap();

        sqlx::query(
            "INSERT INTO tasks (id, mission_id, title, status, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(task_id.as_bytes().as_slice())
        .bind(mission_id.as_bytes().as_slice())
        .bind("test task")
        .bind("active")
        .bind(&now)
        .bind(&now)
        .execute(pool)
        .await
        .unwrap();

        sqlx::query(
            "INSERT INTO agents (id, mission_id, role, status, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(agent_id.as_bytes().as_slice())
        .bind(mission_id.as_bytes().as_slice())
        .bind("implementer")
        .bind("active")
        .bind(&now)
        .bind(&now)
        .execute(pool)
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn test_telemetry_recording_helper() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test_recovery_telemetry.db");
        let pool = crate::persistence::sqlite::schema::initialize_database(&db_path)
            .await
            .unwrap();

        let mission_id = MissionId::new();
        let task_id = TaskId::new();
        let agent_id = AgentId::new();

        seed_test_parents(&pool, &mission_id, &task_id, &agent_id).await;

        let repo = SqliteModelInvocationRepository::new(pool);

        let selection = ResolvedModelSelection {
            provider: "nvidia".to_string(),
            model_name: "meta/llama-3.1-8b-instruct".to_string(),
            temperature: 0.2,
            max_tokens: 8192,
        };

        let usage = TokenUsage::new(
            100,
            50,
            150,
            0,
            crate::model::types::UsageSource::AuthoritativeProvider,
        );

        let record = ModelRecoveryCoordinator::record_telemetry_attempt(
            &repo,
            mission_id,
            task_id,
            agent_id,
            1,
            &selection,
            1,
            "succeeded",
            &usage,
            "tier=Fast",
        )
        .await
        .unwrap();

        assert_eq!(record.step_number, 1);
        assert_eq!(record.attempt_number, 1);
        assert_eq!(record.total_tokens, 150);

        let fetched = repo.get_invocations_for_task(&task_id).await.unwrap();
        assert_eq!(fetched.len(), 1);
        assert_eq!(fetched[0].model_name, "meta/llama-3.1-8b-instruct");
    }
}
