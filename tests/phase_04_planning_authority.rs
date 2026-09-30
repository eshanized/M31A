//! Integration tests for Planning Subsystem Authority and Machine-Readable State (FINDING-06, PLN-03, PLN-05).

use async_trait::async_trait;
use m31a::agent::model_policy::{ModelCaller, ModelProposal};
use m31a::ids::MissionId;
use m31a::kernel::seams::planner::{PlanRequest, PlanService};
use m31a::planning::projections::mission_projections_dir;
use m31a::planning::service::PlanServiceImpl;
use std::fs;
use std::sync::Arc;
use tempfile::tempdir;

/// Deterministic model stub returning a fixed valid decomposition.
/// Planning authority boundary (Phase 27): the model proposes task shape;
/// the runtime validates and persists it.
struct StubDecomposeCaller;

#[async_trait]
impl ModelCaller for StubDecomposeCaller {
    async fn call_model(&self, _context: &str) -> Result<ModelProposal, String> {
        Ok(ModelProposal::Complete {
            summary: serde_json::json!({
                "tasks": [
                    {
                        "id": "TASK-01",
                        "title": "Survey ledger data structures",
                        "description": "Read-only survey of existing types",
                        "role": "researcher",
                        "depends_on": [],
                        "required_capabilities": ["fs.read"]
                    },
                    {
                        "id": "TASK-02",
                        "title": "Implement ledger core transitions",
                        "description": "Implement state transitions",
                        "role": "implementer",
                        "depends_on": ["TASK-01"],
                        "required_capabilities": ["fs.read", "fs.write"]
                    },
                    {
                        "id": "TASK-03",
                        "title": "Verify ledger invariants",
                        "description": "Run verification suite",
                        "role": "verifier",
                        "depends_on": ["TASK-02"],
                        "required_capabilities": ["fs.read", "cargo.test", "evidence.record"]
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

fn stub_service(dir: &std::path::Path) -> PlanServiceImpl {
    PlanServiceImpl::new(dir).with_model_caller(Arc::new(StubDecomposeCaller))
}

#[tokio::test]
async fn test_has_valid_plan_inspects_machine_readable_state_not_markdown() {
    let dir = tempdir().unwrap();
    let service = stub_service(dir.path());
    let mission_id = MissionId::new();

    // 1. Initially no plan exists
    assert!(!service.has_valid_plan(mission_id).await.unwrap());

    // 2. Generate initial plan
    let resp = service
        .generate_initial_plan(PlanRequest::new(
            mission_id,
            "Implement resilient distributed ledger",
        ))
        .await
        .unwrap();

    assert_eq!(resp.task_count, 3);
    assert_eq!(resp.candidate_plan.tasks.len(), 3);

    // 3. Plan should now be valid
    assert!(service.has_valid_plan(mission_id).await.unwrap());

    let proj_dir = mission_projections_dir(dir.path(), mission_id);
    let plan_md = proj_dir.join("PLAN.md");
    let plan_json = proj_dir.join("plan.json");

    assert!(plan_md.is_file(), "PLAN.md projection must be created");
    assert!(
        plan_json.is_file(),
        "plan.json machine-readable state must be created (FINDING-06)"
    );

    // 4. Deleting or modifying PLAN.md must NOT invalidate has_valid_plan
    fs::remove_file(&plan_md).unwrap();
    assert!(
        !plan_md.exists(),
        "PLAN.md is removed to simulate projection deletion"
    );
    assert!(
        service.has_valid_plan(mission_id).await.unwrap(),
        "has_valid_plan must rely on machine-readable plan.json, not PLAN.md (FINDING-06)"
    );

    // 5. Corrupting plan.json causes has_valid_plan to return false
    fs::write(&plan_json, "{ broken json").unwrap();
    assert!(
        !service.has_valid_plan(mission_id).await.unwrap(),
        "corrupt plan.json must fail closed"
    );

    // 6. Removing plan.json causes has_valid_plan to return false
    fs::remove_file(&plan_json).unwrap();
    assert!(
        !service.has_valid_plan(mission_id).await.unwrap(),
        "missing plan.json must report no valid plan"
    );
}

#[tokio::test]
async fn test_plan_response_provides_valid_candidate_plan_for_materialization() {
    let dir = tempdir().unwrap();
    let service = stub_service(dir.path());
    let mission_id = MissionId::new();

    let resp = service
        .generate_initial_plan(PlanRequest::new(
            mission_id,
            "Build satellite navigation controller",
        ))
        .await
        .unwrap();

    // Verify candidate plan contains complete task definitions ready for TaskGraphMaterializer
    assert!(!resp.candidate_plan.tasks.is_empty());
    for task in &resp.candidate_plan.tasks {
        assert!(!task.id.as_str().is_empty());
        assert!(!task.objective.is_empty());
        assert!(!task.capabilities.is_empty());
    }
}
