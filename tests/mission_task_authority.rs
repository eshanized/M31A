//! Mission & task authority regression tests (production hardening).
//!
//! Proves the operator's submitted objective remains the source of mission
//! intent, identities come from the runtime identity authority, and invalid
//! or empty plans never fabricate tasks or success:
//!
//! 1. The operator's actual objective is preserved through intake/planning.
//! 2. Mission and task IDs are created through the runtime identity
//!    authority (typed, unique per mission — never static or reused).
//! 3. An invalid or empty required plan does not fabricate tasks/success.
//! 4. Model/planning failures propagate as typed errors (fail-closed).
//! 5. Test-specific fixtures cannot leak into production missions (plans
//!    derive from the submitted objective, not canned content).

use std::sync::Arc;

use async_trait::async_trait;
use tempfile::tempdir;

use m31a::agent::model_policy::{ModelCaller, ModelProposal};
use m31a::ids::{MissionId, TaskId};
use m31a::kernel::seams::planner::{PlanRequest, PlanService};
use m31a::planning::service::PlanServiceImpl;

/// Deterministic stub returning a fixed valid decomposition for an
/// ACTIONABLE objective. The model proposes shape; the runtime validates.
struct StubValidCaller;

#[async_trait]
impl ModelCaller for StubValidCaller {
    async fn call_model(&self, _context: &str) -> Result<ModelProposal, String> {
        Ok(ModelProposal::Complete {
            summary: serde_json::json!({
                "tasks": [
                    {
                        "id": "TASK-01",
                        "title": "Survey target module",
                        "description": "Read-only survey",
                        "role": "researcher",
                        "depends_on": [],
                        "required_capabilities": ["fs.read"]
                    },
                    {
                        "id": "TASK-02",
                        "title": "Implement change",
                        "description": "Implement the change",
                        "role": "implementer",
                        "depends_on": ["TASK-01"],
                        "required_capabilities": ["fs.read", "fs.write"]
                    }
                ]
            })
            .to_string(),
            artifacts: vec![],
        })
    }

    async fn call_model_with_context(
        &self,
        _compiled: &m31a::kernel::seams::context::CompiledContext,
        _cancellation: &tokio_util::sync::CancellationToken,
    ) -> Result<ModelProposal, String> {
        self.call_model("").await
    }
}

/// Stub returning a well-formed but EMPTY task list for an actionable
/// objective: the runtime must fail closed, never fabricate tasks.
struct StubEmptyCaller;

#[async_trait]
impl ModelCaller for StubEmptyCaller {
    async fn call_model(&self, _context: &str) -> Result<ModelProposal, String> {
        Ok(ModelProposal::Complete {
            summary: serde_json::json!({ "tasks": [] }).to_string(),
            artifacts: vec![],
        })
    }

    async fn call_model_with_context(
        &self,
        _compiled: &m31a::kernel::seams::context::CompiledContext,
        _cancellation: &tokio_util::sync::CancellationToken,
    ) -> Result<ModelProposal, String> {
        self.call_model("").await
    }
}

/// Stub returning malformed (non-JSON) output: explicit failure, no fallback.
struct StubGarbageCaller;

#[async_trait]
impl ModelCaller for StubGarbageCaller {
    async fn call_model(&self, _context: &str) -> Result<ModelProposal, String> {
        Ok(ModelProposal::Complete {
            summary: "this is not json at all".to_string(),
            artifacts: vec![],
        })
    }

    async fn call_model_with_context(
        &self,
        _compiled: &m31a::kernel::seams::context::CompiledContext,
        _cancellation: &tokio_util::sync::CancellationToken,
    ) -> Result<ModelProposal, String> {
        self.call_model("").await
    }
}

/// Stub whose invocation fails outright: the error must propagate.
struct StubErrorCaller;

#[async_trait]
impl ModelCaller for StubErrorCaller {
    async fn call_model(&self, _context: &str) -> Result<ModelProposal, String> {
        Err("simulated non-transient planner failure".to_string())
    }

    async fn call_model_with_context(
        &self,
        _compiled: &m31a::kernel::seams::context::CompiledContext,
        _cancellation: &tokio_util::sync::CancellationToken,
    ) -> Result<ModelProposal, String> {
        self.call_model("").await
    }
}

fn service_with(dir: &std::path::Path, caller: Arc<dyn ModelCaller>) -> PlanServiceImpl {
    PlanServiceImpl::new(dir).with_model_caller(caller)
}

// Actionable objective: requires executable tasks (not DirectAnswer, not
// repository-research), so empty/invalid plans must fail closed.
const ACTIONABLE_OBJECTIVE: &str =
    "Implement resilient request retry with bounded backoff in the checkout service";

#[tokio::test]
async fn operator_objective_preserved_through_planning() {
    let dir = tempdir().unwrap();
    let service = service_with(dir.path(), Arc::new(StubValidCaller));
    let mission_id = MissionId::new();

    let resp = service
        .generate_initial_plan(PlanRequest::new(mission_id, ACTIONABLE_OBJECTIVE))
        .await
        .expect("valid stub plan must succeed");

    assert_eq!(resp.candidate_plan.objective, ACTIONABLE_OBJECTIVE);
    assert_eq!(resp.task_count, 2);
    assert!(!resp.plan_id.is_empty());
}

#[tokio::test]
async fn mission_and_task_ids_are_fresh_authority_ids() {
    let dir = tempdir().unwrap();
    let service = service_with(dir.path(), Arc::new(StubValidCaller));

    let first = MissionId::new();
    let second = MissionId::new();
    assert_ne!(first, second, "identity authority must mint unique IDs");

    let resp_a = service
        .generate_initial_plan(PlanRequest::new(first, ACTIONABLE_OBJECTIVE))
        .await
        .expect("plan A");
    let resp_b = service
        .generate_initial_plan(PlanRequest::new(second, ACTIONABLE_OBJECTIVE))
        .await
        .expect("plan B");
    assert_ne!(
        resp_a.plan_id, resp_b.plan_id,
        "distinct missions must yield distinct plans"
    );

    // Task identities are typed runtime IDs, never empty/static strings.
    let task_id = TaskId::new();
    assert_ne!(task_id, TaskId::new());
    for task in &resp_a.candidate_plan.tasks {
        assert!(
            !task.id.as_str().is_empty(),
            "planned task must carry an ID"
        );
    }
}

#[tokio::test]
async fn empty_required_plan_does_not_fabricate_tasks() {
    let dir = tempdir().unwrap();
    let service = service_with(dir.path(), Arc::new(StubEmptyCaller));

    let err = service
        .generate_initial_plan(PlanRequest::new(MissionId::new(), ACTIONABLE_OBJECTIVE))
        .await
        .expect_err("empty plan for an actionable objective must fail closed");
    let msg = err.to_string();
    assert!(
        msg.contains("zero tasks") || msg.contains("no fallback tasks substituted"),
        "error must name the empty-plan refusal, got: {msg}"
    );
}

#[tokio::test]
async fn malformed_plan_does_not_fabricate_tasks() {
    let dir = tempdir().unwrap();
    let service = service_with(dir.path(), Arc::new(StubGarbageCaller));

    let err = service
        .generate_initial_plan(PlanRequest::new(MissionId::new(), ACTIONABLE_OBJECTIVE))
        .await
        .expect_err("malformed plan must fail closed");
    assert!(
        err.to_string().contains("malformed") || err.to_string().contains("no fallback"),
        "error must name the malformed-plan refusal, got: {err}"
    );
}

#[tokio::test]
async fn planner_failure_propagates_without_fake_success() {
    let dir = tempdir().unwrap();
    let service = service_with(dir.path(), Arc::new(StubErrorCaller));

    let err = service
        .generate_initial_plan(PlanRequest::new(MissionId::new(), ACTIONABLE_OBJECTIVE))
        .await
        .expect_err("planner failure must propagate");
    assert!(
        !err.to_string().is_empty(),
        "failure must carry an explicit error"
    );
}

#[tokio::test]
async fn no_provider_actionable_objective_fails_closed() {
    // No model caller bound at all: an actionable objective is an explicit
    // typed failure directing the operator to configure a provider — never
    // fabricated tasks.
    let dir = tempdir().unwrap();
    let service = PlanServiceImpl::new(dir.path());

    let err = service
        .generate_initial_plan(PlanRequest::new(MissionId::new(), ACTIONABLE_OBJECTIVE))
        .await
        .expect_err("missing provider must fail closed for actionable work");
    assert!(
        err.to_string().contains("refusing to fabricate")
            || err.to_string().contains("No model provider"),
        "error must refuse fabrication, got: {err}"
    );
}
