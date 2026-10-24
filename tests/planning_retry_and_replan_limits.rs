//! Phase 29.6 Gap D Verification Suite: Planning-Level Retry for Transient Provider Failures
//!
//! Validates:
//! 1. Transient failure retries up to configured limit (honors FailurePolicy max_retries).
//! 2. Transient failure succeeds on retry.
//! 3. Permanent error fails immediately without retrying.
//! 4. Retry-after / cooldown is honored.
//! 5. Exhausted retries surface underlying provider error accurately.
//! 6. Existing planning contracts remain intact (e.g. deterministic proposal error does not loop).

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};
use tempfile::tempdir;

use async_trait::async_trait;
use m31a::agent::model_policy::{ModelCaller, ModelProposal};
use m31a::ids::MissionId;
use m31a::kernel::seams::planner::{PlanRequest, PlanService};
use m31a::model::types::ModelError;
use m31a::planning::service::{PlanServiceImpl, is_transient_provider_error};

/// Scriptable mock ModelCaller tracking call counts and returning a sequence of responses.
struct SequenceModelCaller {
    call_count: Arc<AtomicUsize>,
    responses: Vec<Result<ModelProposal, String>>,
}

impl SequenceModelCaller {
    fn new(responses: Vec<Result<ModelProposal, String>>) -> (Self, Arc<AtomicUsize>) {
        let count = Arc::new(AtomicUsize::new(0));
        (
            Self {
                call_count: count.clone(),
                responses,
            },
            count,
        )
    }
}

#[async_trait]
impl ModelCaller for SequenceModelCaller {
    async fn call_model(&self, _context: &str) -> Result<ModelProposal, String> {
        let idx = self.call_count.fetch_add(1, Ordering::SeqCst);
        if idx < self.responses.len() {
            self.responses[idx].clone()
        } else if let Some(last) = self.responses.last() {
            last.clone()
        } else {
            Err("No response configured".to_string())
        }
    }
}

fn valid_plan_proposal() -> ModelProposal {
    let json = r#"{
        "tasks": [
            {
                "task_id": "TASK-001",
                "title": "Setup repository scaffolding",
                "dependencies": [],
                "assigned_role": "implementer",
                "acceptance_criteria": ["cargo check passes"]
            }
        ]
    }"#;
    ModelProposal::Complete {
        summary: json.to_string(),
        artifacts: vec![],
    }
}

#[tokio::test]
async fn test_gap_d_1_transient_failure_retries_up_to_configured_limit() {
    let dir = tempdir().unwrap();
    let transient_err = ModelError::Http {
        status: 503,
        message: "Service Unavailable".to_string(),
    }
    .to_string();

    let (caller, call_count) = SequenceModelCaller::new(vec![
        Err(transient_err.clone()),
        Err(transient_err.clone()),
        Err(transient_err.clone()),
    ]);

    let service = PlanServiceImpl::new(dir.path()).with_model_caller(Arc::new(caller));

    let mid = MissionId::new();
    let req = PlanRequest::new(mid, "Implement resilient database storage layer");
    let result = service.generate_initial_plan(req).await;

    assert!(result.is_err());
    // Initial attempt (0) + 2 retries (1, 2) = 3 total attempts
    assert_eq!(
        call_count.load(Ordering::SeqCst),
        3,
        "Planning must retry transient 503 failure up to max_retries (2 retries + 1 initial = 3 calls)"
    );
}

#[tokio::test]
async fn test_gap_d_2_transient_failure_succeeds_on_retry() {
    let dir = tempdir().unwrap();
    let transient_err = ModelError::Network("connection reset by peer".to_string()).to_string();

    let (caller, call_count) =
        SequenceModelCaller::new(vec![Err(transient_err), Ok(valid_plan_proposal())]);

    let service = PlanServiceImpl::new(dir.path()).with_model_caller(Arc::new(caller));

    let mid = MissionId::new();
    let req = PlanRequest::new(mid, "Implement resilient database storage layer");
    let response = service
        .generate_initial_plan(req)
        .await
        .expect("Transient failure should succeed on retry");

    assert_eq!(
        call_count.load(Ordering::SeqCst),
        2,
        "Planning should invoke caller twice when first call fails transiently and second succeeds"
    );
    assert_eq!(
        response.candidate_plan.tasks.len(),
        1,
        "Materialized plan should have 1 task from successful proposal"
    );
}

#[tokio::test]
async fn test_gap_d_3_permanent_error_fails_immediately_without_retrying() {
    let dir = tempdir().unwrap();
    let perm_err = ModelError::AuthenticationFailed.to_string();

    let (caller, call_count) = SequenceModelCaller::new(vec![
        Err(perm_err),
        Ok(valid_plan_proposal()), // Would succeed if retried, but must NOT retry
    ]);

    let service = PlanServiceImpl::new(dir.path()).with_model_caller(Arc::new(caller));

    let mid = MissionId::new();
    let req = PlanRequest::new(mid, "Implement resilient database storage layer");
    let result = service.generate_initial_plan(req).await;

    let err = result.expect_err("Permanent authentication failure must not succeed");
    assert_eq!(
        call_count.load(Ordering::SeqCst),
        1,
        "Permanent error must fail immediately on first attempt without consuming retry budget"
    );
    let msg = err.to_string();
    assert!(
        msg.contains("authentication failed"),
        "Error message must indicate authentication failure: {}",
        msg
    );
}

#[tokio::test]
async fn test_gap_d_4_retry_after_cooldown_is_honored() {
    let dir = tempdir().unwrap();
    // 1-second declared cooldown
    let rate_limit_err = ModelError::RateLimited { cooldown_secs: 1 }.to_string();

    let (caller, call_count) =
        SequenceModelCaller::new(vec![Err(rate_limit_err), Ok(valid_plan_proposal())]);

    let service = PlanServiceImpl::new(dir.path()).with_model_caller(Arc::new(caller));

    let mid = MissionId::new();
    let req = PlanRequest::new(mid, "Implement rate-limited workflow step");

    let start = Instant::now();
    let response = service
        .generate_initial_plan(req)
        .await
        .expect("Rate limited request should succeed after cooldown");
    let elapsed = start.elapsed();

    assert_eq!(call_count.load(Ordering::SeqCst), 2);
    assert!(
        elapsed >= Duration::from_millis(900),
        "Elapsed duration ({:?}) must honor declared 1s cooldown",
        elapsed
    );
    assert_eq!(response.candidate_plan.tasks.len(), 1);
}

#[tokio::test]
async fn test_gap_d_5_exhausted_retries_surface_underlying_provider_error() {
    let dir = tempdir().unwrap();
    let specific_error = "provider returned HTTP 504: Gateway Timeout connecting to NVIDIA NIM";

    let (caller, call_count) = SequenceModelCaller::new(vec![
        Err(specific_error.to_string()),
        Err(specific_error.to_string()),
        Err(specific_error.to_string()),
    ]);

    let service = PlanServiceImpl::new(dir.path()).with_model_caller(Arc::new(caller));

    let mid = MissionId::new();
    let req = PlanRequest::new(mid, "Implement upstream integration");
    let err = service
        .generate_initial_plan(req)
        .await
        .expect_err("Exhausted retries must return error");

    assert_eq!(call_count.load(Ordering::SeqCst), 3);
    let msg = err.to_string();
    assert!(
        msg.contains("Gateway Timeout connecting to NVIDIA NIM"),
        "Underlying provider error must be surfaced verbatim in planning error: {}",
        msg
    );
    assert!(
        msg.contains("504"),
        "Status code must be preserved in surfaced error: {}",
        msg
    );
}

#[tokio::test]
async fn test_gap_d_6_deterministic_proposal_error_does_not_retry_at_provider_level() {
    let dir = tempdir().unwrap();
    // Malformed JSON is a deterministic prompt/model proposal defect, not a transient transport drop
    let malformed_proposal = ModelProposal::Complete {
        summary: "This is not valid JSON at all {".to_string(),
        artifacts: vec![],
    };

    let (caller, call_count) = SequenceModelCaller::new(vec![
        Ok(malformed_proposal),
        Ok(valid_plan_proposal()), // Must NOT reach this
    ]);

    let service = PlanServiceImpl::new(dir.path()).with_model_caller(Arc::new(caller));

    let mid = MissionId::new();
    let req = PlanRequest::new(mid, "Implement malformed handling");
    let err = service
        .generate_initial_plan(req)
        .await
        .expect_err("Malformed model output must fail explicitly without retrying transport");

    assert_eq!(
        call_count.load(Ordering::SeqCst),
        1,
        "Deterministic proposal error must fail immediately without transport-level retrying"
    );
    let msg = err.to_string();
    assert!(
        msg.contains("malformed plan JSON"),
        "Surfaced error must identify malformed plan JSON: {}",
        msg
    );
}

#[test]
fn test_gap_d_error_classification_semantics() {
    // Transient errors
    assert!(is_transient_provider_error("provider returned HTTP 503: Service Unavailable").0);
    assert!(is_transient_provider_error("provider returned HTTP 502: Bad Gateway").0);
    assert!(is_transient_provider_error("provider returned HTTP 500: Internal Server Error").0);
    assert!(is_transient_provider_error("rate limit exceeded, retry after 5s").0);
    assert_eq!(
        is_transient_provider_error("rate limit exceeded, retry after 5s").1,
        Some(Duration::from_secs(5))
    );
    assert!(is_transient_provider_error("network connection or transport error: reset").0);
    assert!(is_transient_provider_error("timeout: operation timed out after 30s").0);
    assert!(is_transient_provider_error("SSE stream interrupted: broken pipe").0);
    assert!(is_transient_provider_error("endpoint unavailable: connection refused").0);

    // Permanent errors
    assert!(!is_transient_provider_error("authentication failed: invalid or expired API key").0);
    assert!(!is_transient_provider_error("missing credentials: NVIDIA_API_KEY").0);
    assert!(!is_transient_provider_error("missing configuration: endpoint not set").0);
    assert!(!is_transient_provider_error("provider returned HTTP 401: Unauthorized").0);
    assert!(!is_transient_provider_error("provider returned HTTP 403: Forbidden").0);
    assert!(!is_transient_provider_error("provider returned HTTP 400: Bad Request").0);
    assert!(!is_transient_provider_error("context window exhausted: requested 8000").0);
    assert!(!is_transient_provider_error("operation cancelled by runtime").0);
}
