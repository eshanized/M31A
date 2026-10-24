//! Phase 7 Integration & Verification Test Suite: Prompt Provenance, Telemetry & Observability.
//!
//! Verifies:
//! 1. Distinctness of Contract Hash, Effective Prompt Hash, and Context Digest.
//! 2. Determinism of Context Digest across identical context manifests.
//! 3. Semantic sensitivity of Context Digest across all context dimensions.
//! 4. ModelProfile and PromptStrategy provenance recording during compilation.
//! 5. Seven-layer metadata breakdown and reverse compaction telemetry.
//! 6. Event serialization and backward-compatible deserialization for StepProvenance and AgentStepRecord.
//! 7. Multi-tier secret redaction and privacy boundaries (no raw prompts, no hidden reasoning).
//! 8. End-to-end provenance correlation across Mission, Task, Step, Agent, and Model telemetry.
//! 9. SQLite Migration 015 persistence roundtrip for ModelInvocationRecord with PromptInvocationProvenance.
//! 10. Fail-closed budget protection for immutable and protected layers (L0, L1, L2, L6).

use chrono::Utc;
use m31a::agent::AgentStepRecord;
use m31a::events::types::EventType;
use m31a::ids::{AgentId, MissionId, TaskId};
use m31a::model::persistence::invocation::{
    ModelInvocationRecord, SqliteModelInvocationRepository,
};
use m31a::model::types::{TokenUsage, UsageSource};
use m31a::persistence::sqlite::schema::initialize_database;
use m31a::prompt::compiler::{CompilationOptions, DefaultPromptCompiler, PromptCompiler};
use m31a::prompt::composer::PromptLayerKind;
use m31a::prompt::context::{MissionStage, PromptContext};
use m31a::prompt::contract::{PromptContract, RUNTIME_SAFETY_INVARIANTS};
use m31a::prompt::error::PromptError;
use m31a::prompt::model_profile::ModelProfile;
use m31a::prompt::parameter::PromptParameter;
use m31a::prompt::provenance::{PromptInvocationProvenance, PromptProvenance};
use m31a::prompt::strategy::PromptStrategy;
use m31a::state_machine::agent::AgentRole;
use m31a::telemetry::redactor::SecretRedactor;
use m31a::workflow::provenance::StepProvenance;
use tempfile::tempdir;

fn create_test_contract(id: &str, role: AgentRole) -> PromptContract {
    PromptContract::new(
        id,
        1,
        role,
        format!("Test contract for {}", id),
        vec![PromptParameter {
            name: "task_spec".to_string(),
            description: "Task specification".to_string(),
            is_required: true,
            default_value: None,
        }],
        "Execute task objective: {{ task_spec }}",
        Some("structured_plan".to_string()),
    )
    .expect("test contract creates cleanly")
}

fn create_base_context() -> PromptContext {
    PromptContext::new(
        "ctx-001",
        "mission-alpha",
        "task-100",
        AgentRole::implementer(),
        MissionStage::Execute,
        "Implement PromptOS Phase 7 provenance tracking",
    )
    .with_parameter(
        "task_spec",
        "Implement PromptOS Phase 7 provenance tracking",
    )
}

// ---------------------------------------------------------------------------
// 1. Distinctness of Hashes
// ---------------------------------------------------------------------------
#[test]
fn test_distinct_hashes_contract_effective_context() {
    let contract = create_test_contract("agent.implementer", AgentRole::implementer());
    let context = create_base_context()
        .with_user_intent("Add provenance telemetry")
        .with_charter("Phase 7 charter specification")
        .with_repo_file("src/prompt/provenance.rs", "pub struct PromptProvenance {}");

    let compiler = DefaultPromptCompiler::new();
    let options = CompilationOptions::default();
    let effective = compiler
        .compile(&contract, &context, &options)
        .expect("compilation succeeds");

    let contract_hash = &contract.content_hash;
    let effective_hash = &effective.content_hash;
    let context_digest = context.compute_context_digest();

    // 1. All three hashes are non-empty and 64 hex characters (SHA-256)
    assert_eq!(contract_hash.len(), 64);
    assert_eq!(effective_hash.len(), 64);
    assert_eq!(context_digest.len(), 64);

    // 2. All three hashes are strictly distinct from each other
    assert_ne!(
        contract_hash, effective_hash,
        "contract template hash must not match effective compiled hash"
    );
    assert_ne!(
        effective_hash, &context_digest,
        "effective prompt hash must not match context digest"
    );
    assert_ne!(
        contract_hash, &context_digest,
        "contract hash must not match context digest"
    );

    // 3. The effective prompt provenance reflects these exact distinct hashes
    let prov = effective
        .provenance
        .as_ref()
        .expect("provenance must be present");
    assert_eq!(&prov.invocation.prompt_content_hash, contract_hash);
    assert_eq!(&prov.invocation.effective_prompt_hash, effective_hash);
    assert_eq!(&prov.invocation.context_digest, &context_digest);
}

// ---------------------------------------------------------------------------
// 2. Determinism of Context Digest
// ---------------------------------------------------------------------------
#[test]
fn test_context_digest_determinism() {
    let ctx1 = create_base_context()
        .with_user_intent("Deterministic test intent")
        .with_charter("Project charter memory")
        .with_requirements("Requirement 1: Auditability")
        .with_architecture("Architecture: L0-L9")
        .with_upstream_artifact("plan_step", "plan.md", "# Plan\nDetailed plan")
        .with_tool_output("read_file", "call_1", "file contents", false)
        .with_repo_topology("src/ lib.rs")
        .with_repo_file("src/main.rs", "fn main() {}")
        .with_quality_gate(vec!["cargo check passes".to_string()])
        .with_parameter("extra_key", "extra_val");

    let ctx2 = create_base_context()
        .with_user_intent("Deterministic test intent")
        .with_charter("Project charter memory")
        .with_requirements("Requirement 1: Auditability")
        .with_architecture("Architecture: L0-L9")
        .with_upstream_artifact("plan_step", "plan.md", "# Plan\nDetailed plan")
        .with_tool_output("read_file", "call_1", "file contents", false)
        .with_repo_topology("src/ lib.rs")
        .with_repo_file("src/main.rs", "fn main() {}")
        .with_quality_gate(vec!["cargo check passes".to_string()])
        .with_parameter("extra_key", "extra_val");

    let digest1 = ctx1.compute_context_digest();
    let digest2 = ctx2.compute_context_digest();

    assert_eq!(
        digest1, digest2,
        "identical contexts must produce identical context digests"
    );
}

// ---------------------------------------------------------------------------
// 3. Semantic Sensitivity Matrix
// ---------------------------------------------------------------------------
#[test]
fn test_context_digest_sensitivity_matrix() {
    let base = create_base_context();
    let base_digest = base.compute_context_digest();

    // Changing role alters digest
    let mut role_diff = base.clone();
    role_diff.role = AgentRole::reviewer();
    assert_ne!(base_digest, role_diff.compute_context_digest());

    // Changing stage alters digest
    let mut stage_diff = base.clone();
    stage_diff.stage = MissionStage::Review;
    assert_ne!(base_digest, stage_diff.compute_context_digest());

    // Changing mission_id alters digest
    let mut mission_diff = base.clone();
    mission_diff.mission_id = "mission-beta".to_string();
    assert_ne!(base_digest, mission_diff.compute_context_digest());

    // Changing task_id alters digest
    let mut task_diff = base.clone();
    task_diff.task_id = "task-200".to_string();
    assert_ne!(base_digest, task_diff.compute_context_digest());

    // Changing task objective alters digest
    let mut obj_diff = base.clone();
    obj_diff.task_objective.task_objective = "A different task objective".to_string();
    assert_ne!(base_digest, obj_diff.compute_context_digest());

    // Adding task criteria alters digest
    let criteria_diff = base
        .clone()
        .with_quality_gate(vec!["must pass lint".to_string()]);
    assert_ne!(base_digest, criteria_diff.compute_context_digest());

    // Adding user input alters digest
    let user_diff = base.clone().with_user_intent("Deploy to staging");
    assert_ne!(base_digest, user_diff.compute_context_digest());

    // Adding charter alters digest
    let charter_diff = base.clone().with_charter("Mission Charter v2");
    assert_ne!(base_digest, charter_diff.compute_context_digest());

    // Adding upstream artifact alters digest
    let artifact_diff = base
        .clone()
        .with_upstream_artifact("step_0", "out.txt", "artifact data");
    assert_ne!(base_digest, artifact_diff.compute_context_digest());

    // Adding tool output alters digest
    let tool_diff = base
        .clone()
        .with_tool_output("cargo", "call_99", "Compiling...", false);
    assert_ne!(base_digest, tool_diff.compute_context_digest());

    // Adding repo file alters digest
    let file_diff = base.clone().with_repo_file("src/test.rs", "fn test() {}");
    assert_ne!(base_digest, file_diff.compute_context_digest());

    // Adding custom parameter alters digest
    let param_diff = base.clone().with_parameter("custom_foo", "bar");
    assert_ne!(base_digest, param_diff.compute_context_digest());
}

// ---------------------------------------------------------------------------
// 4. ModelProfile and PromptStrategy Provenance Recording
// ---------------------------------------------------------------------------
#[test]
fn test_model_profile_and_strategy_provenance_recording() {
    let contract = create_test_contract("agent.reviewer", AgentRole::reviewer());
    let context = create_base_context();
    let compiler = DefaultPromptCompiler::new();

    let mut profile = ModelProfile::conservative_default();
    profile.model_id = "meta/llama-3.1-8b-instruct".to_string();
    profile.provider = "nvidia".to_string();
    profile.context_capacity = 32_768;
    profile.supports_parallel_tools = false;

    let options = CompilationOptions::default()
        .with_profile(profile)
        .with_strategy(PromptStrategy::Constrained);

    let effective = compiler
        .compile(&contract, &context, &options)
        .expect("compiles successfully");

    let prov = effective.provenance.as_ref().expect("provenance recorded");

    // Invocation provenance records profile and strategy accurately
    assert_eq!(prov.invocation.model_id, "meta/llama-3.1-8b-instruct");
    assert_eq!(prov.invocation.provider, "nvidia");
    assert_eq!(prov.invocation.prompt_strategy, PromptStrategy::Constrained);
    assert_eq!(prov.invocation.output_contract_id, "structured_plan");
    assert_eq!(prov.invocation.prompt_version, 1);
    assert_eq!(prov.role, AgentRole::implementer());
    assert_eq!(prov.stage, MissionStage::Execute);

    // EffectivePrompt helper invocation_provenance() works
    let invoc = effective
        .invocation_provenance()
        .expect("invocation provenance present");
    assert_eq!(invoc.model_id, "meta/llama-3.1-8b-instruct");
    assert_eq!(invoc.prompt_strategy, PromptStrategy::Constrained);
}

// ---------------------------------------------------------------------------
// 5. Seven-Layer Metadata Breakdown and Reverse Compaction Telemetry
// ---------------------------------------------------------------------------
#[test]
fn test_layer_metadata_and_compaction_telemetry() {
    let contract = create_test_contract("agent.implementer", AgentRole::implementer());
    let compiler = DefaultPromptCompiler::new();

    // Build context with all layers (L0..L6)
    let context = create_base_context()
        .with_user_intent("Build full telemetry test")
        .with_charter("Charter memory for L3 durable state testing")
        .with_upstream_artifact("step_prev", "artifact.json", "{\"status\": \"ok\"}")
        .with_repo_file("src/telemetry.rs", "pub fn record_telemetry() {}")
        .with_quality_gate(vec!["Assertion 1: Verified".to_string()]);

    // Scenario A: Unconstrained budget (all layers included)
    let effective = compiler
        .compile(&contract, &context, &CompilationOptions::default())
        .expect("compilation succeeds");

    let prov = effective.provenance.as_ref().expect("provenance present");
    assert!(!prov.has_dropped_layers());
    assert_eq!(prov.dropped_layers().len(), 0);
    assert!(prov.retained_layers().contains(&PromptLayerKind::L0Safety));
    assert!(prov.retained_layers().contains(&PromptLayerKind::L1Role));
    assert!(
        prov.retained_layers()
            .contains(&PromptLayerKind::L2Objective)
    );
    assert!(
        prov.retained_layers()
            .contains(&PromptLayerKind::L3DurableState)
    );
    assert!(
        prov.retained_layers()
            .contains(&PromptLayerKind::L4Evidence)
    );
    assert!(
        prov.retained_layers()
            .contains(&PromptLayerKind::L5RepoContext)
    );
    assert!(
        prov.retained_layers()
            .contains(&PromptLayerKind::L6QualityGate)
    );

    // Verify all 7 layers have metadata entries
    assert_eq!(prov.layers.len(), 7);
    for layer in &prov.layers {
        assert!(layer.included, "layer {:?} should be included", layer.kind);
        assert!(
            layer.byte_size > 0,
            "layer {:?} has non-zero size",
            layer.kind
        );
        if PromptProvenance::is_protected_layer(layer.kind) {
            assert!(layer.is_protected);
        } else {
            assert!(!layer.is_protected);
        }
    }

    // Scenario B: Constrained budget forcing reverse compaction of L5 first
    // Calculate size of protected layers + L3 + L4
    let l0_l1_l2_l6_size: usize = prov
        .layers
        .iter()
        .filter(|l| l.is_protected)
        .map(|l| l.byte_size)
        .sum();
    let l3_size = prov
        .layers
        .iter()
        .find(|l| l.kind == PromptLayerKind::L3DurableState)
        .unwrap()
        .byte_size;
    let l4_size = prov
        .layers
        .iter()
        .find(|l| l.kind == PromptLayerKind::L4Evidence)
        .unwrap()
        .byte_size;

    // Budget fits protected + L3 + L4, but NOT L5
    let constrained_budget = l0_l1_l2_l6_size + l3_size + l4_size + 10;
    let options = CompilationOptions::default().with_budget(constrained_budget);

    let effective_compacted = compiler
        .compile(&contract, &context, &options)
        .expect("compilation succeeds under budget");

    let prov_compacted = effective_compacted
        .provenance
        .as_ref()
        .expect("provenance present");
    assert!(prov_compacted.has_dropped_layers());
    assert_eq!(
        prov_compacted.dropped_layers(),
        &[PromptLayerKind::L5RepoContext]
    );
    assert!(
        prov_compacted
            .retained_layers()
            .contains(&PromptLayerKind::L3DurableState)
    );
    assert!(
        prov_compacted
            .retained_layers()
            .contains(&PromptLayerKind::L4Evidence)
    );

    // Verify layer metadata reflects dropped status for L5
    let l5_meta = prov_compacted
        .layers
        .iter()
        .find(|l| l.kind == PromptLayerKind::L5RepoContext)
        .expect("L5 metadata present");
    assert!(!l5_meta.included);
    assert!(!l5_meta.is_protected);

    // Scenario C: Tighter budget forcing dropping of L5 and L4
    let tighter_budget = l0_l1_l2_l6_size + l3_size + 10;
    let options_tighter = CompilationOptions::default().with_budget(tighter_budget);

    let effective_tighter = compiler
        .compile(&contract, &context, &options_tighter)
        .expect("compiles under tighter budget");

    let prov_tighter = effective_tighter.provenance.as_ref().unwrap();
    assert_eq!(
        prov_tighter.dropped_layers(),
        &[PromptLayerKind::L4Evidence, PromptLayerKind::L5RepoContext]
    );
}

// ---------------------------------------------------------------------------
// 6. Event Serialization and Backward-Compatible Deserialization
// ---------------------------------------------------------------------------
#[test]
fn test_event_serialization_and_backward_compatibility() {
    let mission_id = MissionId::new();
    let task_id = TaskId::new();

    // 1. PromptCompiled event roundtrip
    let compiled_event = EventType::PromptCompiled {
        mission_id,
        task_id,
        prompt_id: "agent.implementer".to_string(),
        prompt_version: 1,
        effective_prompt_hash: "abcd1234efgh5678".to_string(),
        strategy: "standard".to_string(),
        total_bytes: 2048,
    };
    let json = serde_json::to_string(&compiled_event).expect("serializes");
    let deserialized: EventType = serde_json::from_str(&json).expect("deserializes");
    assert_eq!(compiled_event, deserialized);
    assert_eq!(compiled_event.name(), "PromptCompiled");

    // 2. PromptCompilationFailed event roundtrip
    let failed_event = EventType::PromptCompilationFailed {
        mission_id,
        task_id,
        prompt_id: "agent.planner".to_string(),
        prompt_version: 2,
        error: "Budget exceeded".to_string(),
    };
    let json_failed = serde_json::to_string(&failed_event).expect("serializes");
    let deserialized_failed: EventType = serde_json::from_str(&json_failed).expect("deserializes");
    assert_eq!(failed_event, deserialized_failed);
    assert_eq!(failed_event.name(), "PromptCompilationFailed");

    // 3. StepProvenance backward-compatible deserialization (without Phase 7 fields)
    let legacy_step_json = r#"{
        "step_key": "impl_step",
        "prompt_id": "agent.implementer",
        "prompt_version": 1,
        "prompt_content_hash": "hash123",
        "role": "implementer"
    }"#;
    let step_prov: StepProvenance =
        serde_json::from_str(legacy_step_json).expect("deserializes legacy JSON");
    assert_eq!(step_prov.step_key, "impl_step");
    assert_eq!(step_prov.effective_prompt_hash, None);
    assert_eq!(step_prov.context_digest, None);
    assert_eq!(step_prov.prompt_strategy, None);

    // 4. StepProvenance with Phase 7 fields
    let invoc_prov = PromptInvocationProvenance::new(
        "agent.implementer",
        1,
        "hash123",
        PromptStrategy::Standard,
        "eff_hash_789",
        "default",
        "default",
        "ctx_digest_456",
        "markdown",
        Utc::now(),
    );
    let enriched_step = step_prov.with_invocation_provenance(&invoc_prov);
    assert_eq!(
        enriched_step.effective_prompt_hash.as_deref(),
        Some("eff_hash_789")
    );
    assert_eq!(
        enriched_step.context_digest.as_deref(),
        Some("ctx_digest_456")
    );
    assert_eq!(
        enriched_step.prompt_strategy,
        Some(PromptStrategy::Standard)
    );

    // 5. AgentStepRecord backward-compatible deserialization (without prompt_provenance)
    let legacy_agent_step_json = r#"{
        "step_number": 1,
        "model_proposal_summary": "action:read_file",
        "actions_executed": [],
        "started_at": "2026-09-21T12:00:00Z",
        "completed_at": "2026-09-21T12:00:01Z"
    }"#;
    let agent_step: AgentStepRecord =
        serde_json::from_str(legacy_agent_step_json).expect("deserializes legacy agent step");
    assert_eq!(agent_step.step_number, 1);
    assert_eq!(agent_step.prompt_provenance, None);

    // 6. AgentStepRecord with prompt_provenance
    let enriched_agent_step = agent_step.with_prompt_provenance(invoc_prov);
    assert!(enriched_agent_step.prompt_provenance.is_some());
}

// ---------------------------------------------------------------------------
// 7. Multi-Tier Secret Redaction and Privacy Boundaries
// ---------------------------------------------------------------------------
#[test]
fn test_secret_redaction_and_privacy_boundaries() {
    let redactor = SecretRedactor::new();

    // 1. Inadvertent secrets in metadata string fields are scrubbed by sanitized()
    let raw_prov = PromptInvocationProvenance::new(
        "agent.implementer",
        1,
        "content_hash_1234",
        PromptStrategy::Standard,
        "effective_hash_5678",
        "sk-ant-api03-abcdef1234567890abcdef1234567890", // inadvertent secret in model_id
        "Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.token.signature", // inadvertent bearer in provider
        "context_digest_9999",
        "AKIAIOSFODNN7EXAMPLE", // inadvertent AWS key in output_contract_id
        Utc::now(),
    );

    let sanitized = raw_prov.sanitized(&redactor);

    assert_eq!(sanitized.model_id, "[REDACTED:API_TOKEN]");
    assert_eq!(sanitized.provider, "[REDACTED:BEARER_TOKEN]");
    assert_eq!(sanitized.output_contract_id, "[REDACTED:AWS_KEY]");
    assert_eq!(sanitized.effective_prompt_hash, "effective_hash_5678");
    assert_eq!(sanitized.context_digest, "context_digest_9999");

    // 2. Privacy invariant: Provenance does NOT store raw prompt text or raw user intent
    let contract = create_test_contract("agent.implementer", AgentRole::implementer());
    let context = create_base_context()
        .with_user_intent("super sensitive raw intent text that should never be in provenance");
    let compiler = DefaultPromptCompiler::new();
    let effective = compiler
        .compile(&contract, &context, &CompilationOptions::default())
        .unwrap();
    let prov = effective.provenance.unwrap();

    let prov_json = serde_json::to_string(&prov).unwrap();
    assert!(
        !prov_json.contains("super sensitive raw intent text"),
        "raw user intent must never appear in serialized PromptProvenance"
    );
    assert!(
        !prov_json.contains(RUNTIME_SAFETY_INVARIANTS),
        "raw prompt template body must never appear in serialized PromptProvenance"
    );
}

// ---------------------------------------------------------------------------
// 8. End-to-End Provenance Correlation
// ---------------------------------------------------------------------------
#[test]
fn test_end_to_end_provenance_correlation() {
    let mission_id = MissionId::new();
    let task_id = TaskId::new();
    let agent_id = AgentId::new();

    let contract = create_test_contract("agent.implementer", AgentRole::implementer());
    let context = PromptContext::new(
        "ctx-corr-1",
        mission_id.to_string(),
        task_id.to_string(),
        AgentRole::implementer(),
        MissionStage::Execute,
        "Correlation test task",
    )
    .with_parameter("task_spec", "Correlation test task");

    let compiler = DefaultPromptCompiler::new();
    let effective = compiler
        .compile(&contract, &context, &CompilationOptions::default())
        .unwrap();
    let invoc_prov = effective
        .invocation_provenance()
        .expect("invocation provenance");

    // 1. AgentStepRecord correlation
    let step_record = AgentStepRecord {
        step_number: 1,
        ..Default::default()
    }
    .with_prompt_provenance(invoc_prov.clone());

    assert_eq!(
        step_record
            .prompt_provenance
            .as_ref()
            .unwrap()
            .effective_prompt_hash,
        effective.content_hash
    );

    // 2. StepProvenance correlation
    let step_prov = StepProvenance::new(
        "step_1",
        &contract.id,
        contract.version,
        &contract.content_hash,
        AgentRole::implementer(),
    )
    .with_invocation_provenance(&invoc_prov);

    assert_eq!(
        step_prov.effective_prompt_hash.as_deref(),
        Some(effective.content_hash.as_str())
    );

    // 3. ModelInvocationRecord correlation
    let usage = TokenUsage::new(300, 150, 450, 0, UsageSource::AuthoritativeProvider);
    let invoc_prov_json = serde_json::to_string(&invoc_prov).unwrap();

    let model_record = ModelInvocationRecord::new(
        mission_id,
        task_id,
        agent_id,
        1,
        "nvidia",
        "meta/llama-3.3-70b-instruct",
        1,
        "succeeded",
        &usage,
        "optimal tier",
    )
    .with_prompt_provenance(&invoc_prov_json);

    assert_eq!(
        model_record.prompt_provenance.as_deref(),
        Some(invoc_prov_json.as_str())
    );

    // Verify all 3 records share the exact same prompt contract ID, version, and content hash
    let deserialized_prov: PromptInvocationProvenance =
        serde_json::from_str(model_record.prompt_provenance.as_deref().unwrap()).unwrap();
    assert_eq!(deserialized_prov.prompt_id, contract.id);
    assert_eq!(deserialized_prov.prompt_version, contract.version);
    assert_eq!(
        deserialized_prov.effective_prompt_hash,
        effective.content_hash
    );
    assert_eq!(
        deserialized_prov.context_digest,
        context.compute_context_digest()
    );
}

// ---------------------------------------------------------------------------
// 9. SQLite Migration 015 Persistence Roundtrip
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_sqlite_migration_015_persistence_roundtrip() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("phase7_telemetry.db");
    let pool = initialize_database(&db_path)
        .await
        .expect("initializes database with migration 015");

    let mission_id = MissionId::new();
    let task_id = TaskId::new();
    let agent_id = AgentId::new();

    let now = Utc::now().to_rfc3339();
    sqlx::query("INSERT INTO missions (id, objective, status, created_at, updated_at) VALUES (?, ?, ?, ?, ?)")
        .bind(mission_id.as_bytes().as_slice())
        .bind("Phase 7 Verification Mission")
        .bind("active")
        .bind(&now)
        .bind(&now)
        .execute(&pool)
        .await
        .unwrap();

    sqlx::query("INSERT INTO tasks (id, mission_id, title, status, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)")
        .bind(task_id.as_bytes().as_slice())
        .bind(mission_id.as_bytes().as_slice())
        .bind("Provenance persistence task")
        .bind("active")
        .bind(&now)
        .bind(&now)
        .execute(&pool)
        .await
        .unwrap();

    sqlx::query("INSERT INTO agents (id, mission_id, role, status, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)")
        .bind(agent_id.as_bytes().as_slice())
        .bind(mission_id.as_bytes().as_slice())
        .bind("implementer")
        .bind("active")
        .bind(&now)
        .bind(&now)
        .execute(&pool)
        .await
        .unwrap();

    let repo = SqliteModelInvocationRepository::new(pool);

    let invoc_prov = PromptInvocationProvenance::new(
        "agent.implementer",
        1,
        "contract_sha256_hash",
        PromptStrategy::Standard,
        "effective_prompt_sha256_hash",
        "meta/llama-3.3-70b-instruct",
        "nvidia",
        "context_manifest_digest",
        "structured_plan",
        Utc::now(),
    );
    let prov_json = serde_json::to_string(&invoc_prov).unwrap();

    let usage = TokenUsage::new(400, 200, 600, 0, UsageSource::AuthoritativeProvider);
    let record = ModelInvocationRecord::new(
        mission_id,
        task_id,
        agent_id,
        1,
        "nvidia",
        "meta/llama-3.3-70b-instruct",
        1,
        "succeeded",
        &usage,
        "tier=Standard",
    )
    .with_prompt_provenance(&prov_json);

    repo.insert_invocation(&record)
        .await
        .expect("insert model invocation succeeds");

    let task_records = repo
        .get_invocations_for_task(&task_id)
        .await
        .expect("query by task succeeds");
    assert_eq!(task_records.len(), 1);

    let retrieved = &task_records[0];
    assert_eq!(retrieved.id, record.id);
    assert_eq!(
        retrieved.prompt_provenance.as_deref(),
        Some(prov_json.as_str())
    );

    // Verify roundtrip deserialization of stored JSON
    let parsed_prov: PromptInvocationProvenance =
        serde_json::from_str(retrieved.prompt_provenance.as_deref().unwrap())
            .expect("deserializes stored JSON to PromptInvocationProvenance");
    assert_eq!(parsed_prov.prompt_id, "agent.implementer");
    assert_eq!(
        parsed_prov.effective_prompt_hash,
        "effective_prompt_sha256_hash"
    );
    assert_eq!(parsed_prov.context_digest, "context_manifest_digest");
}

// ---------------------------------------------------------------------------
// 10. Fail-Closed Budget Invariant for Protected Layers
// ---------------------------------------------------------------------------
#[test]
fn test_protected_layer_invariant_under_budget_pressure() {
    let contract = create_test_contract("agent.implementer", AgentRole::implementer());
    let context = create_base_context();
    let compiler = DefaultPromptCompiler::new();

    // Set budget smaller than required protected layers (L0 alone is ~330 bytes)
    let tiny_options = CompilationOptions::default().with_budget(100);

    let result = compiler.compile(&contract, &context, &tiny_options);
    assert!(
        matches!(result, Err(PromptError::PromptBudgetExceeded { .. })),
        "compiler must fail closed when protected layers exceed budget"
    );
}
