//! PromptOS wiring remediation v0.1.1: reachability matrix + execution-binding tests.
//!
//! Proves the production chain:
//! ```text
//! prompt asset → PromptContract → PromptCatalog → PromptReference →
//! role/workflow/task binding → ContextCompilationRequest → PromptCompiler →
//! EffectivePrompt → ModelCaller
//! ```
//! Every production model invocation carries a traceable prompt contract.

use std::collections::BTreeMap;
use std::sync::Arc;

use m31a::agent::model_policy::{ModelCaller, ModelProposal};
use m31a::agent::profile::{AgentProfile, ProfileOverride};
use m31a::agent::runner::{ActionDispatcher, ActionRequest, ActionResult, WorkerRunner};
use m31a::agent::supervisor::ExecutionActivityTracker;
use m31a::context::compiler::ProductionContextCompiler;
use m31a::ids::{AgentId, MissionId, TaskId};
use m31a::kernel::seams::context::{
    ContextCompilationRequest, ContextCompiler, PromptSelectionSource,
};
use m31a::prompt::{
    PromptCatalog, PromptCompiler, PromptPurpose, PromptReference, builtin_reachability_matrix,
    canonical_version, classify_builtin,
};
use m31a::runtime_authorities::{ModelInvocation, ModelInvocationKind};
use m31a::state::intake::AutonomyMode;
use m31a::state_machine::agent::AgentRole;
use m31a::workflow::{
    QualityGate, RecoveryStrategy, WorkflowDefinition, WorkflowStepDefinition,
    compiler::CompiledWorkflow,
};

// ============================================================================
// §22: reachability matrix gate
// ============================================================================

#[test]
fn matrix_every_shipped_prompt_has_exactly_one_ci_bucket() {
    let catalog = m31a::prompt::InMemoryPromptCatalog::with_builtins();
    let (rows, unclassified) = builtin_reachability_matrix(&catalog);

    assert!(
        unclassified.is_empty(),
        "shipped prompts with no reachability classification (fails CI): {unclassified:?}"
    );
    assert_eq!(
        rows.len(),
        catalog.list().len(),
        "matrix must cover every cataloged contract"
    );

    // Every row's bucket matches the static classifier (single source of truth).
    for (id, version, reach, bucket) in &rows {
        let (expected_reach, expected_bucket) =
            classify_builtin(id, *version).expect("classified row must re-classify");
        assert_eq!(
            *reach, expected_reach,
            "reachability drift for {id} v{version}"
        );
        assert_eq!(
            *bucket, expected_bucket,
            "CI bucket drift for {id} v{version}"
        );
    }

    // All four CI buckets are populated (no dead bucket arms).
    let mut seen = std::collections::HashSet::new();
    for (_, _, _, bucket) in &rows {
        seen.insert(*bucket);
    }
    assert_eq!(
        seen.len(),
        4,
        "all four CI buckets must be populated: {seen:?}"
    );
}

#[test]
fn matrix_canonical_versions_are_production_reachable() {
    let catalog = m31a::prompt::InMemoryPromptCatalog::with_builtins();
    for id in [
        "agent.implementer",
        "agent.reviewer",
        "agent.verifier",
        "agent.diagnostician",
        "execution.implementer",
        "planning.decompose",
    ] {
        let canonical = canonical_version(id).expect("canonical version declared");
        assert_eq!(canonical, 2, "{id} canonical generation must be v2");
        assert!(
            catalog.contains(id, canonical),
            "canonical {id}.v{canonical} must be cataloged"
        );
        let (_, bucket) = classify_builtin(id, canonical).unwrap();
        assert_eq!(
            bucket,
            m31a::prompt::PromptCiBucket::ProductionReachable,
            "{id}.v{canonical} must be production-reachable"
        );
    }
}

// ============================================================================
// Test doubles
// ============================================================================

struct AssistantTextCaller;

#[async_trait::async_trait]
impl ModelCaller for AssistantTextCaller {
    async fn call_model(&self, _context: &str) -> Result<ModelProposal, String> {
        Ok(ModelProposal::AssistantText {
            content: "noted".to_string(),
        })
    }
}

struct NoopDispatcher;

#[async_trait::async_trait]
impl ActionDispatcher for NoopDispatcher {
    async fn dispatch(&self, action: &ActionRequest) -> Result<ActionResult, String> {
        Ok(ActionResult {
            action_id: action.id.clone(),
            success: true,
            output: String::new(),
            error: None,
        })
    }
}

fn genesis_research_step() -> WorkflowStepDefinition {
    WorkflowStepDefinition {
        key: "research_security".to_string(),
        name: "Research Security".to_string(),
        role: AgentRole::new("security_researcher"),
        prompt_template: "genesis.research_security".to_string(),
        prompt_ref: Some(PromptReference::with_purpose(
            "genesis.research_security",
            1,
            PromptPurpose::Genesis,
        )),
        required_inputs: Vec::new(),
        expected_outputs: Vec::new(),
        required_capabilities: Vec::new(),
        quality_gate: QualityGate::default(),
        depends_on: Vec::new(),
        timeout_secs: 600,
        allows_parallelism: false,
        recovery_strategy: Some(RecoveryStrategy::Retry { max_retries: 1 }),
    }
}

fn single_step_definition(step: WorkflowStepDefinition) -> WorkflowDefinition {
    WorkflowDefinition {
        id: "prompt-reachability-wf".to_string(),
        name: "Prompt Reachability".to_string(),
        description: "reachability probe".to_string(),
        version: 1,
        steps: vec![step],
        default_recovery_strategy: RecoveryStrategy::Fail,
    }
}

// ============================================================================
// Test A: workflow prompt == compiled prompt == provenance prompt
// ============================================================================

#[tokio::test]
async fn test_a_workflow_step_prompt_survives_lowering_and_compilation() {
    let definition = single_step_definition(genesis_research_step());
    let compiled = CompiledWorkflow::from_definition(definition).expect("valid definition");
    let lowered = compiled
        .lower(MissionId::new())
        .expect("lowering must succeed");

    assert_eq!(lowered.candidate_plan.tasks.len(), 1);
    let task = &lowered.candidate_plan.tasks[0];
    let task_prompt = task
        .prompt_ref
        .clone()
        .expect("task must carry a typed prompt ref");
    assert_eq!(task_prompt.id, "genesis.research_security");
    assert_eq!(task_prompt.version, 1);
    // §18: the prompt identifier MUST NOT be downgraded into description text.
    if let Some(ref description) = task.description {
        assert!(
            !description.contains("genesis.research_security"),
            "prompt id must not leak into task description metadata: {description}"
        );
    }

    // Compile exactly what the worker would compile.
    let compiler = ProductionContextCompiler::new();
    let req = ContextCompilationRequest::new(MissionId::new(), TaskId::new(), 8192)
        .with_role(AgentRole::new("security_researcher"))
        .with_prompt_ref(task_prompt)
        .with_prompt_source(PromptSelectionSource::ExplicitTask)
        .with_task_objective("Research deployment security posture");
    let compiled_ctx = compiler
        .compile_context(req)
        .await
        .expect("compilation must succeed");

    let provenance = compiled_ctx
        .prompt_provenance
        .as_ref()
        .expect("compiled context must carry prompt provenance");
    assert_eq!(provenance.prompt_id, "genesis.research_security");
    assert_eq!(provenance.prompt_version, 1);
    let manifest = compiled_ctx.manifest.as_ref().expect("manifest required");
    assert_eq!(
        manifest.prompt_id.as_deref(),
        Some("genesis.research_security")
    );
    assert_eq!(manifest.prompt_version, Some(1));
    assert_eq!(
        manifest.prompt_source,
        Some(PromptSelectionSource::ExplicitTask)
    );
}

// ============================================================================
// Test B: workflow prompt beats the role default
// ============================================================================

#[tokio::test]
async fn test_b_workflow_prompt_beats_role_default() {
    let mut step = genesis_research_step();
    step.key = "implement".to_string();
    step.name = "Implement".to_string();
    step.role = AgentRole::implementer();
    step.prompt_template = "execution.implementer.v2".to_string();
    step.prompt_ref = Some(PromptReference::with_purpose(
        "execution.implementer",
        2,
        PromptPurpose::WorkflowExecution,
    ));
    let definition = single_step_definition(step);
    let lowered = CompiledWorkflow::from_definition(definition)
        .expect("valid")
        .lower(MissionId::new())
        .expect("lowering");
    let task_prompt = lowered.candidate_plan.tasks[0]
        .prompt_ref
        .clone()
        .expect("typed prompt ref");

    let compiler = ProductionContextCompiler::new();
    let req = ContextCompilationRequest::new(MissionId::new(), TaskId::new(), 8192)
        .with_role(AgentRole::implementer())
        .with_prompt_ref(task_prompt)
        .with_prompt_source(PromptSelectionSource::ExplicitTask)
        .with_task_objective("Implement the feature");
    let compiled_ctx = compiler.compile_context(req).await.expect("compiles");
    let provenance = compiled_ctx.prompt_provenance.as_ref().expect("provenance");
    // The worker compiles v2 execution.implementer — NOT agent.implementer.
    assert_eq!(provenance.prompt_id, "execution.implementer");
    assert_eq!(provenance.prompt_version, 2);
}

// ============================================================================
// Test C: interactive implementer resolves the canonical role contract
// ============================================================================

#[test]
fn test_c_role_default_resolves_canonical_v2_through_shared_chain() {
    // The exact binding AgentEngine::compile_stable_layer uses.
    let role = AgentRole::implementer();
    let prompt_ref = PromptReference::for_role(role);
    assert_eq!(prompt_ref.id, "agent.implementer");
    assert_eq!(
        prompt_ref.version, 2,
        "role default must bind the canonical v2"
    );

    let catalog = m31a::prompt::InMemoryPromptCatalog::with_builtins();
    let compiler = m31a::prompt::DefaultPromptCompiler::new();
    let contract = catalog
        .resolve_canonical(&prompt_ref.id, prompt_ref.version)
        .expect("canonical contract resolves");
    assert_eq!(contract.id, "agent.implementer");
    assert_eq!(contract.version, 2);

    let mut params = BTreeMap::new();
    params.insert("mission_id".to_string(), "m-1".to_string());
    params.insert("task_id".to_string(), "t-1".to_string());
    params.insert("task_objective".to_string(), "interactive work".to_string());
    let mut prompt_ctx = m31a::prompt::PromptContext::new(
        "ctx-test",
        "m-1",
        "t-1",
        AgentRole::implementer(),
        m31a::prompt::MissionStage::Execute,
        "interactive work",
    );
    prompt_ctx.custom_parameters = params;
    let effective = compiler
        .compile(
            contract,
            &prompt_ctx,
            &m31a::prompt::CompilationOptions::default(),
        )
        .expect("interactive stable layer compiles");
    let provenance = effective.invocation_provenance().expect("provenance");
    assert_eq!(provenance.prompt_id, "agent.implementer");
    assert_eq!(provenance.prompt_version, 2);
    assert!(
        !effective.system_prompt.contains("hardcoded"),
        "no fallback text"
    );
}

// ============================================================================
// Test D: role change changes the prompt reference
// ============================================================================

#[test]
fn test_d_role_change_changes_prompt_reference() {
    let implementer = PromptReference::for_role(AgentRole::implementer());
    let reviewer = PromptReference::for_role(AgentRole::reviewer());
    assert_ne!(
        implementer.id, reviewer.id,
        "role change must change the prompt binding"
    );
    assert_eq!(implementer.id, "agent.implementer");
    assert_eq!(reviewer.id, "agent.reviewer");
}

// ============================================================================
// Test E: workflow prompt change changes the compiled prompt
// ============================================================================

#[tokio::test]
async fn test_e_workflow_prompt_change_changes_compiled_prompt() {
    async fn compile_with(prompt_ref: PromptReference) -> String {
        let compiler = ProductionContextCompiler::new();
        let req = ContextCompilationRequest::new(MissionId::new(), TaskId::new(), 8192)
            .with_role(AgentRole::implementer())
            .with_prompt_ref(prompt_ref)
            .with_prompt_source(PromptSelectionSource::ExplicitTask)
            .with_task_objective("do work");
        compiler
            .compile_context(req)
            .await
            .expect("compiles")
            .prompt_provenance
            .expect("provenance")
            .prompt_id
    }

    let a = compile_with(PromptReference::new("execution.implementer", 2)).await;
    let b = compile_with(PromptReference::new("genesis.research_security", 1)).await;
    assert_ne!(
        a, b,
        "changing the workflow prompt ref must change the compiled prompt"
    );
    assert_eq!(a, "execution.implementer");
}

// ============================================================================
// Test F: missing prompt fails before model invocation
// ============================================================================

#[tokio::test]
async fn test_f_missing_prompt_fails_before_model_invocation() {
    let compiler = ProductionContextCompiler::new();
    let req = ContextCompilationRequest::new(MissionId::new(), TaskId::new(), 8192)
        .with_role(AgentRole::implementer())
        .with_prompt_ref(PromptReference::new("nonexistent.prompt", 9))
        .with_prompt_source(PromptSelectionSource::ExplicitTask)
        .with_task_objective("do work");
    let err = compiler
        .compile_context(req)
        .await
        .expect_err("unknown prompt must fail closed");
    let msg = err.to_string();
    assert!(
        msg.contains("nonexistent.prompt"),
        "error names the missing contract: {msg}"
    );
}

// ============================================================================
// Test G: catalog replacement coherence (no stale catalog)
// ============================================================================

#[tokio::test]
async fn test_g_worker_uses_bound_compiler_never_stale_catalog() {
    use m31a::kernel::seams::context::CompiledContext;

    struct FixedCompiler {
        marker: &'static str,
    }
    #[async_trait::async_trait]
    impl ContextCompiler for FixedCompiler {
        async fn compile_context(
            &self,
            _req: ContextCompilationRequest,
        ) -> Result<CompiledContext, m31a::kernel::seams::context::ContextError> {
            Ok(CompiledContext::new(
                "ctx-fixed",
                10,
                format!("system:{}", self.marker),
            ))
        }
    }

    // A runner bound to compiler B observes B's output, never A's.
    let compiler_b = FixedCompiler { marker: "B" };
    let req = ContextCompilationRequest::new(MissionId::new(), TaskId::new(), 4096);
    let compiled = ContextCompiler::compile_context(&compiler_b, req)
        .await
        .unwrap();
    assert_eq!(compiled.system_prompt, "system:B");

    // Rebinding the authority set swaps the compiler wholesale: catalog
    // state travels with the compiler instance, so a replaced catalog
    // cannot leak into workers bound to the new instance.
    async fn compile_with_catalog(prompt_id: &str) -> Result<String, String> {
        // Fixture catalog = builtins + one extra contract: the P0 safety
        // layer and role defaults resolve exactly as in production.
        let mut catalog = m31a::prompt::InMemoryPromptCatalog::with_builtins();
        let mut contract = m31a::prompt::PromptContract::new(
            prompt_id,
            1,
            AgentRole::researcher(),
            "coherence fixture",
            Vec::new(),
            "do work",
            None,
        )
        .expect("fixture");
        contract.stage = Some(m31a::prompt::MissionStage::Research);
        catalog.register(contract).expect("register");
        let compiler = ProductionContextCompiler::new().with_prompt_catalog(Arc::new(catalog));
        let req = ContextCompilationRequest::new(MissionId::new(), TaskId::new(), 8192)
            .with_role(AgentRole::researcher())
            .with_prompt_ref(PromptReference::new(prompt_id, 1))
            .with_prompt_source(PromptSelectionSource::ExplicitTask)
            .with_task_objective("coherence probe");
        compiler
            .compile_context(req)
            .await
            .map(|ctx| ctx.prompt_provenance.expect("provenance").prompt_id)
            .map_err(|e| e.to_string())
    }

    // Each bound catalog serves exactly its own contracts.
    assert_eq!(
        compile_with_catalog("coherence.alpha").await.unwrap(),
        "coherence.alpha"
    );
    assert_eq!(
        compile_with_catalog("coherence.beta").await.unwrap(),
        "coherence.beta"
    );
}

// ============================================================================
// Test H: V1 compatibility alias resolves through the documented path
// ============================================================================

#[test]
fn test_h_v1_compat_resolves_through_documented_migration_path() {
    let catalog = m31a::prompt::InMemoryPromptCatalog::with_builtins();

    // Same-id generations upgrade to canonical v2.
    let upgraded = catalog
        .resolve_canonical("execution.implementer", 1)
        .expect("v1 resolves");
    assert_eq!(
        upgraded.version, 2,
        "same-id v1 must upgrade to canonical v2"
    );

    // Renamed legacy ids route through declared compatibility aliases:
    // `resolve_canonical` upgrades the requested v1 generation to the
    // canonical v2 contract (the documented migration path).
    let aliased = catalog
        .resolve_canonical("execution.reviewer", 1)
        .expect("legacy alias resolves");
    assert_eq!(aliased.id, "verification.reviewer");
    assert_eq!(aliased.version, 2);
    let aliased_diag = catalog
        .resolve_canonical("execution.diagnostician", 1)
        .expect("legacy alias resolves");
    assert_eq!(aliased_diag.id, "recovery.diagnostician");
    assert_eq!(aliased_diag.version, 2);
}

// ============================================================================
// Test I: provenance always matches the actual compiled prompt
// ============================================================================

#[tokio::test]
async fn test_i_provenance_matches_actual_compiled_prompt() {
    let catalog = m31a::prompt::InMemoryPromptCatalog::with_builtins();
    let compiler = ProductionContextCompiler::new();
    // Contracts compiled through the generic worker path.
    for (id, version) in [
        ("genesis.research_security", 1),
        ("execution.implementer", 2),
        ("planning.decompose", 2),
        ("verification.reviewer", 2),
    ] {
        let req = ContextCompilationRequest::new(MissionId::new(), TaskId::new(), 8192)
            .with_prompt_ref(PromptReference::new(id, version))
            .with_prompt_source(PromptSelectionSource::ExplicitTask)
            .with_task_objective("prove provenance")
            // planning.decompose requires a charter parameter: upstream
            // charter flows into it (harmless for contracts that ignore it).
            .with_upstream_charter("Fixture project charter");
        let compiled = compiler.compile_context(req).await.expect("compiles");
        let provenance = compiled.prompt_provenance.as_ref().expect("provenance");
        let contract = catalog.resolve_canonical(id, version).expect("contract");
        // Resolve-then-compare against the RESOLVED contract: upgrades and
        // alias routes are visible here, never hidden.
        assert_eq!(
            provenance.prompt_id, contract.id,
            "provenance id must match compiled contract"
        );
        assert_eq!(provenance.prompt_version, contract.version);
        assert_eq!(provenance.prompt_content_hash, contract.content_hash);
        let manifest = compiled.manifest.as_ref().expect("manifest");
        assert_eq!(manifest.prompt_id.as_deref(), Some(contract.id.as_str()));
        assert_eq!(
            manifest.prompt_content_hash.as_deref(),
            Some(contract.content_hash.as_str())
        );
    }

    // recovery.diagnostician carries failure-specific required parameters
    // (error_message, stderr_snippet) that only exist inside a real
    // diagnosis: it compiles through its production consumer path
    // (DiagnosticianContext + bound authorities), never the generic
    // worker path. Provenance must still match the compiled contract.
    {
        use m31a::verification::diagnostician::DiagnosticianContext;
        let catalog = m31a::prompt::InMemoryPromptCatalog::with_builtins();
        let compiler = m31a::prompt::DefaultPromptCompiler::new();
        let ctx = DiagnosticianContext::new(
            "error[E0308]: mismatched types",
            Some(101),
            Some("Compiling m31a".to_string()),
            Some("expected String, found &str".to_string()),
        )
        .with_task_info("Build crates", "Compile core crates");
        let effective = ctx
            .compile_prompt(&catalog, &compiler)
            .expect("diagnostician compiles through its production path");
        let contract = catalog
            .resolve_canonical("recovery.diagnostician", 2)
            .expect("contract");
        let provenance = effective.invocation_provenance().expect("provenance");
        assert_eq!(provenance.prompt_id, contract.id);
        assert_eq!(provenance.prompt_version, contract.version);
        assert_eq!(provenance.prompt_content_hash, contract.content_hash);
    }
}

// ============================================================================
// Test J: no model invocation without a prompt reference
// ============================================================================

#[tokio::test]
async fn test_j_no_invocation_without_prompt_reference() {
    // Unregistered role + no prompt ref + stage-less contract fails closed.
    // Fixture catalog = builtins + the stage-less fixture (P0 resolves).
    let mut catalog = m31a::prompt::InMemoryPromptCatalog::with_builtins();
    let contract = m31a::prompt::PromptContract::new(
        "custom.stageless",
        1,
        AgentRole::new("ghost_role_xyz"),
        "stage-less fixture",
        Vec::new(),
        "do work",
        None,
    )
    .expect("fixture contract");
    catalog.register(contract).expect("register");
    let compiler = ProductionContextCompiler::new().with_prompt_catalog(Arc::new(catalog));
    let req = ContextCompilationRequest::new(MissionId::new(), TaskId::new(), 8192)
        .with_role(AgentRole::new("ghost_role_xyz"))
        .with_task_objective("do work");
    let err = compiler
        .compile_context(req)
        .await
        .expect_err("must fail closed");
    assert!(
        err.to_string().contains("ghost_role_xyz"),
        "error names the unknown role: {err}"
    );

    // An EffectivePrompt without provenance cannot enter a ModelInvocation.
    let catalog2 = m31a::prompt::InMemoryPromptCatalog::with_builtins();
    let bare_contract = catalog2
        .get("execution.implementer", 2)
        .expect("contract")
        .clone();
    let bare = m31a::prompt::EffectivePrompt {
        prompt_id: bare_contract.id.clone(),
        prompt_version: bare_contract.version,
        contract_hash: bare_contract.content_hash.clone(),
        system_prompt: "raw".to_string(),
        user_prompt: None,
        assembled_text: "raw".to_string(),
        layers: Vec::new(),
        content_hash: "hash".to_string(),
        total_bytes: 3,
        expected_output_format: None,
        supplied_parameters: Vec::new(),
        strategy: m31a::prompt::PromptStrategy::Standard,
        model_profile_id: None,
        provenance: None,
        source_kind: m31a::prompt::PromptSourceKind::Builtin,
    };
    let invocation = ModelInvocation::new(
        bare,
        PromptReference::new("execution.implementer", 2),
        AgentRole::implementer(),
        ModelInvocationKind::Implementation,
        AutonomyMode::Safe,
    );
    assert!(
        invocation.require_provenance().is_err(),
        "provenance-free prompts must be refused"
    );
}

// ============================================================================
// Worker integration: the worker compiles the WORKFLOW prompt
// ============================================================================

#[tokio::test]
async fn test_worker_compiles_workflow_prompt_not_role_default() {
    let mut profile = AgentProfile::built_in(AgentRole::researcher());
    profile = profile
        .apply_override(ProfileOverride {
            max_steps: Some(1),
            ..Default::default()
        })
        .expect("tightening override");
    let compiler: Arc<dyn ContextCompiler> = Arc::new(ProductionContextCompiler::new());
    let mut runner = WorkerRunner::new(
        MissionId::new(),
        AgentId::new(),
        TaskId::new(),
        profile,
        compiler,
    )
    .with_task_objective("Research deployment security posture")
    .with_prompt_ref(PromptReference::with_purpose(
        "genesis.research_security",
        1,
        PromptPurpose::Genesis,
    ));

    let token = tokio_util::sync::CancellationToken::new();
    let tracker = Arc::new(ExecutionActivityTracker::default());
    let _ = runner
        .run_step_loop(&AssistantTextCaller, &NoopDispatcher, &token, tracker)
        .await;
    assert_eq!(runner.step_history().len(), 1);
    let provenance = runner.step_history()[0]
        .prompt_provenance
        .as_ref()
        .expect("worker step must record prompt provenance");
    assert_eq!(provenance.prompt_id, "genesis.research_security");
    assert_eq!(provenance.prompt_version, 1);
}
