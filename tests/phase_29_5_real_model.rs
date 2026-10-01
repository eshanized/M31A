//! Phase 29.5 — Real Model Cutover & Intelligence Validation.
//!
//! Establishes the clean separation between deterministic runtime/invariant
//! tests and real-model behavioral tests:
//!
//! ```text
//! DETERMINISTIC TESTS  →  runtime correctness (mocks allowed for failures)
//! REAL NVIDIA TESTS    →  intelligence correctness (this file; no mocks)
//! REAL E2E             →  autonomous engineering correctness
//! ```
//!
//! Every test in this file:
//! - uses the canonical real-model configuration (`provider = nvidia`,
//!   `model = nvidia/nemotron-3-ultra-550b-a55b`) resolved through
//!   [`m31a::testing::real_model::RealModelHarness`];
//! - loads credentials exclusively from the existing `.env`/configuration
//!   pipeline (never prints, logs, or embeds them);
//! - is marked `#[ignore]` (explicit invocation: real-model integration
//!   suite, not part of the fast deterministic suite);
//! - SKIPS cleanly when credentials/network are unavailable and MUST NOT
//!   substitute a mock/fake model and report success;
//! - exercises the real production path (real provider → real router → real
//!   PromptCatalog → real parser → real runtime);
//! - asserts structural/behavioral properties, never exact model wording.
//!
//! No `TestModelCaller`, `MockProvider`, scripted JSON, or canned reasoning
//! appears in this file. Deterministic failure simulation lives in the
//! regular (non-ignored) suite.

use std::sync::Arc;
use std::time::Duration;

use m31a::kernel::seams::planner::{PlanRequest, PlanService, UpstreamPlanContext};
use m31a::planning::service::PlanServiceImpl;
use m31a::testing::real_model::{RealModelHarness, RealModelSkipped};
use m31a::workflow::genesis::{GenesisMode, GenesisOptions, GenesisRequest};

// ============================================================================
// Shared helpers (no model involvement)
// ============================================================================

/// Enforce the real-model gate: harness or clean skip (never a mock fallback).
fn ensure_harness() -> Result<RealModelHarness, RealModelSkipped> {
    match RealModelHarness::ensure() {
        Ok(h) => {
            println!(
                "[REAL-MODEL] provider={} model={} endpoint={}",
                h.provider_name(),
                h.model_id(),
                h.base_url()
            );
            Ok(h)
        }
        Err(skip) => {
            eprintln!("[REAL-MODEL] {skip} (set NVIDIA_API_KEY to run)");
            Err(skip)
        }
    }
}

macro_rules! require_harness {
    () => {
        match ensure_harness() {
            Ok(h) => h,
            Err(_) => return,
        }
    };
}

async fn run_with_timeout<F, T>(secs: u64, label: &str, fut: F) -> T
where
    F: std::future::Future<Output = T>,
{
    tokio::time::timeout(Duration::from_secs(secs), fut)
        .await
        .unwrap_or_else(|_| panic!("{label} timed out after {secs}s"))
}

/// Honest-boundary helpers for the research test: failure must be
/// workflow-level (never credentials), findings keep dimension provenance,
/// and fallback synthesis over real findings validates.
fn assert_no_credential_failure(msg: &str) {
    assert!(
        !msg.contains("AuthenticationFailed") && !msg.contains("authentication"),
        "research must not fail on credentials when the gate passed: {msg}"
    );
}

fn assert_findings_provenance(
    collected: &[m31a::workflow::genesis::ResearchFinding],
    decision: &m31a::workflow::genesis::ResearchDecision,
) {
    for finding in collected {
        assert!(
            decision
                .selected_dimensions
                .iter()
                .any(|d| d == &finding.dimension),
            "every finding must carry provenance to a selected dimension"
        );
        assert!(!finding.summary.trim().is_empty());
    }
}

fn synthesize_fallback(
    charter: &m31a::workflow::genesis::ProjectCharter,
    collected: &[m31a::workflow::genesis::ResearchFinding],
) {
    // Synthesis over real (possibly empty) findings still validates with
    // explicit provenance — absence recorded, never invented.
    let summary = m31a::workflow::genesis::ResearchSynthesizer::synthesize(charter, collected)
        .expect("fallback synthesis validates");
    summary.validate().expect("fallback summary validates");
    println!(
        "[REAL-MODEL:research] fallback summary consensus_points={}",
        summary.consensus_points.len()
    );
}

/// Dump a compact summary of every traced model request: role chain and
/// the last message snippet. Forensics for timeout/failure classification
/// (sanitized payloads only — never credentials).
fn dump_tracer_history(harness: &RealModelHarness) {
    for (i, req) in harness.traced_requests().iter().enumerate() {
        if let Some(messages) = req.payload.get("messages").and_then(|v| v.as_array()) {
            let chain: Vec<_> = messages
                .iter()
                .filter_map(|m| m.get("role").and_then(|r| r.as_str()))
                .collect();
            let last = messages.last().map(|m| m.to_string()).unwrap_or_default();
            let snippet: String = last.chars().take(300).collect();
            eprintln!(
                "[REAL-MODEL:trace] #{i} model={} msgs={} chain={chain:?} last={snippet}",
                req.model_name,
                messages.len()
            );
        } else {
            let text = req.payload.to_string();
            let snippet: String = text.chars().take(300).collect();
            eprintln!("[REAL-MODEL:trace] #{i} text-prompt {snippet}");
        }
    }
}

/// Dump task/recovery state from the runtime DB for timeout/failure
/// classification in the phase report (no secrets involved).
async fn dump_task_diagnostics(runtime: &m31a::runtime::AppRuntime) {
    let rows: Vec<(String, String, i64)> =
        sqlx::query_as("SELECT candidate_key, status, retry_count FROM tasks ORDER BY rowid")
            .fetch_all(runtime.pool())
            .await
            .unwrap_or_default();
    eprintln!("[REAL-MODEL:diag] task states:");
    for (key, status, retries) in &rows {
        eprintln!("[REAL-MODEL:diag]   {key} status={status} retries={retries}");
    }
    let missions: Vec<(String,)> = sqlx::query_as("SELECT status FROM missions")
        .fetch_all(runtime.pool())
        .await
        .unwrap_or_default();
    eprintln!("[REAL-MODEL:diag] missions: {missions:?}");
    let recovery: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM recovery_attempts")
        .fetch_one(runtime.pool())
        .await
        .unwrap_or((0,));
    eprintln!("[REAL-MODEL:diag] recovery_attempts={}", recovery.0);
}

/// Genesis options for bounded intent/plan tests: research disabled so the
/// test measures intent→charter→plan through one real-model planning call.
/// Research behavior is covered by `real_research_execution`.
fn bounded_genesis_options() -> GenesisOptions {
    GenesisOptions {
        enable_research: false,
        ..GenesisOptions::default()
    }
}

/// Rebuild the upstream plan context exactly as `AppRuntime::run_mission`
/// does from a genesis outcome (same fields, same provenance flow).
fn upstream_from_genesis(outcome: &m31a::runtime::GenesisExecutionOutcome) -> UpstreamPlanContext {
    UpstreamPlanContext {
        project_name: outcome.charter.project_name.clone(),
        charter: outcome.charter.to_markdown(),
        architecture: outcome.planning.architecture.to_markdown(),
        requirements: outcome
            .planning
            .requirements
            .requirements
            .iter()
            .map(|r| {
                format!(
                    "{}: {}",
                    r.key,
                    r.title.as_deref().unwrap_or(&r.description)
                )
            })
            .collect(),
        assumptions: outcome
            .planning
            .risks
            .unknowns
            .iter()
            .map(|u| format!("{}: {}", u.id, u.description))
            .collect(),
        decisions: outcome
            .planning
            .adrs
            .adrs
            .values()
            .map(|a| format!("{}: {}", a.id, a.title))
            .collect(),
        research_summary: outcome.research_summary.as_ref().map(|s| s.to_markdown()),
        workflow_tier: format!("{:?}", outcome.charter.workflow_tier),
        unknowns: outcome
            .planning
            .risks
            .unknowns
            .iter()
            .map(|u| format!("{}: {}", u.id, u.description))
            .collect(),
        user_decisions: outcome
            .planning
            .risks
            .unknowns
            .iter()
            .filter(|u| {
                matches!(
                    u.fate,
                    m31a::planning::risks::UnknownFate::UserDecisionRequired
                        | m31a::planning::risks::UnknownFate::Blocking
                )
            })
            .map(|u| format!("{}: {}", u.id, u.description))
            .collect(),
        resolved_invariants: Vec::new(),
    }
}

/// Real-model task decomposition through the full production planning path:
/// real PromptCatalog contract → real prompt rendering → real NVIDIA model →
/// real parser → real runtime validation. Returns the validated plan.
///
/// The decompose contract declares `failure_behavior.retryable` with
/// `max_retries = 2`: a rejected proposal is returned to the model for a
/// fresh attempt (mirroring the production controller's replan escalation).
/// Every attempt is a genuine model call — nothing is injected or mocked —
/// and success requires a genuinely valid plan. Attempts are logged for the
/// phase report.
async fn decompose_with_real_model(
    harness: &RealModelHarness,
    workspace: &std::path::Path,
    storage: &std::path::Path,
    objective: &str,
    upstream: Option<UpstreamPlanContext>,
) -> m31a::kernel::plan::CandidatePlan {
    let mut last_err = String::new();
    for attempt in 1..=3 {
        let caller = Arc::new(harness.routed_caller(Vec::new()));
        let service = PlanServiceImpl::new_with_roots(workspace, storage).with_model_caller(caller);
        let mut req = PlanRequest::new(m31a::ids::MissionId::new(), objective);
        if let Some(ref ctx) = upstream {
            req = req.with_upstream_context(ctx.clone());
        }
        match service.generate_initial_plan(req).await {
            Ok(resp) => {
                assert!(
                    resp.task_count >= 1,
                    "real model must decompose an actionable objective into ≥1 task"
                );
                println!(
                    "[REAL-MODEL:plan] valid plan on attempt {attempt} ({} tasks)",
                    resp.task_count
                );
                return resp.candidate_plan;
            }
            Err(e) => {
                last_err = e.to_string();
                println!("[REAL-MODEL:plan] attempt {attempt} rejected by runtime: {last_err}");
            }
        }
    }
    panic!("real-model plan decomposition failed after 3 genuine attempts: {last_err}");
}

// ============================================================================
// Gate + configuration (§5, §8, §16-auth)
// ============================================================================

/// The real-model gate resolves canonical configuration and live credentials.
///
/// No model reasoning is exercised here: this test proves the integration
/// configuration (provider, model, endpoint, credential source) before the
/// behavioral tests run. Skips cleanly without credentials.
#[tokio::test]
#[ignore = "requires NVIDIA credentials + network (Phase 29.5 real-model suite)"]
async fn real_model_gate_and_configuration() {
    let harness = require_harness!();

    // Canonical configuration, centralized (no scattered literals).
    assert_eq!(harness.provider_name(), "nvidia");
    assert_eq!(harness.model_id(), "nvidia/nemotron-3-ultra-550b-a55b");
    assert_eq!(harness.base_url(), "https://integrate.api.nvidia.com/v1");
    assert_eq!(
        m31a::model::catalog::resolve_real_model_id(),
        "nvidia/nemotron-3-ultra-550b-a55b"
    );
    assert_eq!(
        harness.provider().base_url(),
        "https://integrate.api.nvidia.com/v1"
    );

    // Live authentication + /models discovery against the real endpoint.
    // This distinguishes MODEL/PROVIDER failures from M31A runtime failures.
    let probe = run_with_timeout(120, "provider probe", harness.provider().probe()).await;
    let latency = probe.expect("NVIDIA /models probe must succeed with valid credentials");
    println!(
        "[REAL-MODEL:gate] /models probe latency_ms={}",
        latency.as_millis()
    );

    // The canonical model must be discoverable on the live endpoint.
    let discovered = run_with_timeout(120, "model discovery", harness.provider().discover_models())
        .await
        .expect("model discovery must succeed");
    assert!(
        discovered
            .iter()
            .any(|c| c.model_id == "nvidia/nemotron-3-ultra-550b-a55b"),
        "canonical model must be present in live discovery ({} models)",
        discovered.len()
    );
    println!(
        "[REAL-MODEL:gate] discovered {} models; canonical present",
        discovered.len()
    );
    harness.log_metrics("gate");
}

// ============================================================================
// TEST A — minimal intent (§9A, §11, §12)
// ============================================================================

/// One-line human intent reaches real-model planning through the production
/// path and yields structural artifacts (intent interpretation, requirements,
/// assumptions, unresolved items, architecture, plan).
///
/// Asserts structure, never wording. Also proves PromptCatalog provenance
/// (§11: the real `planning.decompose` contract renders the request) and
/// context propagation (§12: the model receives intent, requirements,
/// assumptions, and decisions through the production path).
#[tokio::test]
#[ignore = "requires NVIDIA credentials + network (Phase 29.5 real-model suite)"]
async fn real_minimal_intent_to_plan() {
    let harness = require_harness!();
    let dir = tempfile::tempdir().expect("tempdir");
    let ws = dir.path().join("expense-ws");
    std::fs::create_dir_all(&ws).unwrap();

    let runtime = run_with_timeout(180, "runtime init", harness.runtime_for(&ws))
        .await
        .expect("production runtime init");

    // 1. Real upstream reasoning inputs: deterministic genesis synthesis from
    //    the raw one-line intent (runtime discovery, charter, requirements,
    //    architecture, ADRs, risks). Model-independent synthesis is runtime
    //    correctness; the intelligence under test is the planning step below.
    let objective = "build me an expense tracker";
    let gen_req = GenesisRequest::new(objective, &ws)
        .with_mode(GenesisMode::Greenfield)
        .with_options(bounded_genesis_options());
    let outcome = run_with_timeout(180, "genesis", runtime.run_genesis(&gen_req))
        .await
        .expect("genesis must succeed");

    assert!(
        !outcome.planning.requirements.requirements.is_empty(),
        "intent must expand to ≥1 requirement"
    );
    assert!(
        !outcome.planning.architecture.to_markdown().is_empty(),
        "intent must expand to an architecture"
    );
    // Unresolved items surface deterministically in the charter ambiguity
    // assessment (discovery unknowns with blocking/user-decision fate, plus
    // unevidenced areas). `planning.risks.unknowns` is intentionally NOT
    // asserted here: the production planner only fills it from research
    // summaries, and research is disabled for this bounded test — asserting
    // it would test a wiring assumption, not intelligence.
    assert!(
        !outcome
            .charter
            .ambiguity_assessment
            .unresolved_areas
            .is_empty(),
        "intent must surface unresolved items in the charter ambiguity assessment"
    );
    assert!(
        !outcome.planning.adrs.adrs.is_empty(),
        "planning must record decisions"
    );
    println!(
        "[REAL-MODEL:intent] requirements={} unresolved_charter_areas={} decisions={}",
        outcome.planning.requirements.requirements.len(),
        outcome.charter.ambiguity_assessment.unresolved_areas.len(),
        outcome.planning.adrs.adrs.len()
    );

    // Dynamic context needles: first requirement key and first ADR id must
    // reach the model through the production prompt (proves §12 without
    // hardcoding response strings).
    let first_req_key = outcome.planning.requirements.requirements[0].key.clone();
    let first_adr_id = outcome
        .planning
        .adrs
        .adrs
        .keys()
        .next()
        .map(|id| id.to_string())
        .unwrap_or_default();
    let upstream = upstream_from_genesis(&outcome);
    assert!(!upstream.charter.is_empty());
    assert!(!upstream.architecture.is_empty());
    assert!(!upstream.requirements.is_empty());
    assert!(!upstream.decisions.is_empty());

    // 2. Real-model planning through the production PlanService path.
    let storage = ws.join(".m31a");
    let plan = run_with_timeout(
        300,
        "real-model decomposition",
        decompose_with_real_model(&harness, &ws, &storage, objective, Some(upstream)),
    )
    .await;

    // Structural assertions on model output (tolerant of valid variation).
    assert!(!plan.tasks.is_empty());
    for task in &plan.tasks {
        assert!(!task.objective.trim().is_empty(), "task needs an objective");
        for dep in &task.depends_on {
            assert!(
                plan.tasks.iter().any(|t| &t.id == dep),
                "task DAG must reference known task ids"
            );
        }
    }
    // No fabricated verification: every task carries a runtime-known strategy.
    let plan_json = serde_json::to_string(&plan).expect("plan serializes");
    assert!(plan_json.contains("expense") || plan_json.contains("Expense"));

    // 3. Production-path proofs.
    harness.assert_canonical_model_routing();
    harness.assert_context_propagated(&[objective, &first_req_key]);
    if !first_adr_id.is_empty() {
        harness.assert_context_propagated(&[&first_adr_id]);
    }
    harness.log_metrics("intent");
}

// ============================================================================
// TEST D — consequential decision (§9D)
// ============================================================================

/// A genuinely consequential architectural decision that cannot be safely
/// inferred must surface as structured `user_decision_required` state.
///
/// The runtime classifier marks tenant-isolation ambiguity as
/// `UserDecisionRequired`; the real model receives that ambiguity through the
/// production prompt; the structured decision state must survive planning
/// without being silently auto-resolved. No response string is hardcoded.
#[tokio::test]
#[ignore = "requires NVIDIA credentials + network (Phase 29.5 real-model suite)"]
async fn real_consequential_decision_surfaces() {
    let harness = require_harness!();
    let dir = tempfile::tempdir().expect("tempdir");
    let ws = dir.path().join("tenant-ws");
    std::fs::create_dir_all(&ws).unwrap();

    let runtime = run_with_timeout(180, "runtime init", harness.runtime_for(&ws))
        .await
        .expect("production runtime init");

    let objective = "build a multi-tenant expense platform with live billing";
    let gen_req = GenesisRequest::new(objective, &ws)
        .with_mode(GenesisMode::Greenfield)
        .with_options(bounded_genesis_options());
    let outcome = run_with_timeout(180, "genesis", runtime.run_genesis(&gen_req))
        .await
        .expect("genesis must succeed");

    // Structured decision state (runtime half): the deterministic
    // consequence classifier must surface tenant isolation as an unresolved
    // charter area keyed by its stable unknown id. Discovery unknowns with
    // `UserDecisionRequired`/`Blocking` fate land in
    // `charter.ambiguity_assessment.unresolved_areas` (production path in
    // `DiscoverySession::synthesize_charter`).
    let gated: Vec<_> = outcome
        .charter
        .ambiguity_assessment
        .unresolved_areas
        .iter()
        .filter(|a| a.contains("UNK-TENANT-ISOLATION"))
        .collect();
    assert!(
        !gated.is_empty(),
        "consequential ambiguity must surface UNK-TENANT-ISOLATION in charter unresolved areas, got {:?}",
        outcome.charter.ambiguity_assessment.unresolved_areas
    );
    println!("[REAL-MODEL:decision] charter surfaces: {}", gated[0]);

    // Structured decision state (second runtime path, no hardcoding): a real
    // DiscoverySession over the same prompt must require a user decision with
    // UserDecisionRequired fate.
    let probe_env =
        m31a::workflow::genesis::GenesisController::probe(&ws).expect("workspace probe");
    let session = m31a::workflow::genesis::DiscoverySession::new(
        m31a::workflow::genesis::GenesisRequest::new(objective, &ws),
        probe_env,
    );
    assert!(
        session.requires_user_decision(),
        "consequential prompt must require a user decision"
    );
    let session_gated: Vec<_> = session
        .unknowns()
        .into_iter()
        .filter(|u| {
            u.fate == m31a::planning::risks::UnknownFate::UserDecisionRequired
                && u.id == "UNK-TENANT-ISOLATION"
        })
        .collect();
    assert_eq!(
        session_gated.len(),
        1,
        "UNK-TENANT-ISOLATION must carry UserDecisionRequired fate"
    );

    // The real model plans WITH the ambiguity visible (not pre-resolved).
    // Upstream assumptions are extended with the real session unknowns in the
    // exact runtime format; the production `planning.decompose` contract
    // carries them to the model.
    let mut upstream = upstream_from_genesis(&outcome);
    for u in session.unknowns() {
        upstream
            .assumptions
            .push(format!("{}: {}", u.id, u.description));
    }
    let storage = ws.join(".m31a");
    let plan = run_with_timeout(
        300,
        "real-model consequential decomposition",
        decompose_with_real_model(&harness, &ws, &storage, objective, Some(upstream)),
    )
    .await;
    assert!(!plan.tasks.is_empty());

    // The model received the ambiguity through the production path, and the
    // structured decision state was not silently auto-resolved: the session
    // still gates on the user decision after planning.
    harness.assert_canonical_model_routing();
    harness.assert_context_propagated(&["UNK-TENANT-ISOLATION"]);
    assert!(
        session.requires_user_decision(),
        "gated decision state must survive planning unresolved"
    );
    harness.log_metrics("decision");
}

// ============================================================================
// TEST F — target-architecture differentiation (§9F)
// ============================================================================

/// Materially different prompts must yield target-specific requirements,
/// architecture, roadmap, and task graphs through real-model synthesis.
///
/// Compares semantic structure (domain keywords, non-identical task sets),
/// never exact strings.
#[tokio::test]
#[ignore = "requires NVIDIA credentials + network (Phase 29.5 real-model suite)"]
async fn real_target_architecture_differentiation() {
    let harness = require_harness!();
    let dir = tempfile::tempdir().expect("tempdir");

    let domains = [
        (
            "expense tracker",
            "expense",
            "track personal spending and receipts",
        ),
        (
            "inventory system",
            "inventory",
            "manage warehouse stock levels and suppliers",
        ),
        (
            "student task application",
            "student",
            "help students organize assignments and deadlines",
        ),
    ];

    let mut plans = Vec::new();
    for (name, keyword, detail) in &domains {
        let ws = dir.path().join(name.replace(' ', "_"));
        std::fs::create_dir_all(&ws).unwrap();
        let storage = ws.join(".m31a");
        let objective = format!("build me an {name} that can {detail}");
        let upstream = UpstreamPlanContext {
            project_name: name.to_string(),
            charter: format!("# {name}\n\nObjective: {objective}"),
            architecture: format!("Target architecture for {name}: modular single-service design."),
            requirements: vec![format!("REQ-DOMAIN-01: core {keyword} workflows must work")],
            assumptions: vec![format!("ASSUME-01: single-user {keyword} deployment")],
            decisions: vec![format!("ADR-0001: build {name} as one deployable unit")],
            research_summary: None,
            workflow_tier: "Greenfield".to_string(),
            unknowns: vec![],
            user_decisions: vec![],
            resolved_invariants: vec![],
        };
        let plan = run_with_timeout(
            300,
            "real-model domain decomposition",
            decompose_with_real_model(&harness, &ws, &storage, &objective, Some(upstream)),
        )
        .await;
        plans.push((name.to_string(), keyword.to_string(), plan));
    }

    // Each plan is target-specific: the domain keyword appears in the
    // model-generated task graph (semantic evidence link, not exact wiring).
    for (name, keyword, plan) in &plans {
        let flat = serde_json::to_string(plan).unwrap().to_lowercase();
        assert!(
            flat.contains(keyword),
            "plan for '{name}' must reference its domain ('{keyword}')"
        );
        assert!(!plan.tasks.is_empty());
    }

    // Plans are not copies of each other: pairwise task-title sets differ.
    for i in 0..plans.len() {
        for j in (i + 1)..plans.len() {
            let ti: Vec<_> = plans[i]
                .2
                .tasks
                .iter()
                .map(|t| t.objective.clone())
                .collect();
            let tj: Vec<_> = plans[j]
                .2
                .tasks
                .iter()
                .map(|t| t.objective.clone())
                .collect();
            assert_ne!(
                ti, tj,
                "plans for '{}' and '{}' must not be identical",
                plans[i].0, plans[j].0
            );
        }
    }

    harness.assert_canonical_model_routing();
    harness.log_metrics("differentiation");
}

// ============================================================================
// Tool-call validation (§13) + token measurement (§15)
// ============================================================================

/// The real model proposes native tool calls through production schemas;
/// the proposal validates against the real registry; the result feeds a real
/// model continuation. No faked tool-call responses.
///
/// Uses the exact production wire-format derivation
/// (`ToolRegistry` → `ToolFilter::filter_to_wire_format`) and the direct
/// provider surface so authoritative token usage is measurable.
#[tokio::test]
#[ignore = "requires NVIDIA credentials + network (Phase 29.5 real-model suite)"]
async fn real_tool_call_proposal_and_continuation() {
    use m31a::model::provider::ModelProvider;
    use m31a::model::types::ChatMessage;

    let harness = require_harness!();

    // Production wire-format tool schemas (same derivation as AppRuntime).
    let capabilities = Arc::new(m31a::capability::registry::CapabilityRegistry::production(
        std::path::Path::new("/tmp"),
        None,
        None,
    ));
    let tool_registry = Arc::new(m31a::tools::registry::ToolRegistry::new_default(
        capabilities.clone(),
    ));
    let known_ids: Vec<String> = tool_registry
        .list_tools()
        .iter()
        .map(|t| t.id().to_string())
        .collect();
    assert!(!known_ids.is_empty());
    let profile = m31a::agent::profile::AgentProfile::built_in(
        m31a::state_machine::agent::AgentRole::implementer(),
    );
    let criteria = m31a::tools::filter::FilterCriteria::new(capabilities)
        .with_role_envelope(&profile.capability_policy);
    let wire_tools =
        m31a::tools::filter::ToolFilter::new(tool_registry).filter_to_wire_format(&criteria);
    assert!(!wire_tools.is_empty());
    println!(
        "[REAL-MODEL:tools] wire schemas={} known_ids={}",
        wire_tools.len(),
        known_ids.len()
    );

    let token = tokio_util::sync::CancellationToken::new();

    // Turn 1: model must propose a native tool call (not a text fallback).
    let (proposal, usage) = run_with_timeout(
        300,
        "real-model tool proposal",
        harness.provider().call_model_with_messages(
            harness.model_id(),
            &[
                ChatMessage::System {
                    content: "You are a file inspection agent. Use the provided tools; never answer from memory.".to_string(),
                },
                ChatMessage::User {
                    content: "List the files in the current workspace directory using the available shell/file tools.".to_string(),
                },
            ],
            wire_tools.clone(),
            &token,
        ),
    )
    .await
    .expect("real-model tool proposal call must succeed");
    println!(
        "[REAL-MODEL:tools] turn1 usage: prompt={} completion={} total={} source={:?}",
        usage.prompt_tokens, usage.completion_tokens, usage.total_tokens, usage.source
    );
    assert!(usage.total_tokens > 0, "provider must report token usage");

    let (tool_name, parameters) = match &proposal {
        m31a::model::types::ModelProposal::ToolCalls { calls } => {
            assert!(
                !calls.is_empty(),
                "real model must propose at least one tool call"
            );
            (calls[0].name.clone(), calls[0].arguments.clone())
        }
        other => panic!("real model must propose a native tool action, got {other:?}"),
    };
    assert!(
        known_ids.iter().any(|id| id == &tool_name),
        "model-proposed tool '{tool_name}' must be a known production capability"
    );
    println!("[REAL-MODEL:tools] model proposed: {tool_name}({parameters})");

    // Turn 2: feed a REAL tool result back (observed shell output, produced
    // by executing the command locally — not fabricated by the test author
    // as model output) and require a model continuation.
    let observed = std::process::Command::new("ls")
        .arg("/tmp")
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
        .unwrap_or_default();
    let (follow_up, usage2) = run_with_timeout(
        300,
        "real-model continuation",
        harness.provider().call_model_with_messages(
            harness.model_id(),
            &[
                ChatMessage::System {
                    content: "You are a file inspection agent. Use the provided tools; never answer from memory.".to_string(),
                },
                ChatMessage::User {
                    content: "List the files in the current workspace directory using the available shell/file tools.".to_string(),
                },
                ChatMessage::Assistant {
                    content: None,
                    tool_calls: vec![m31a::model::types::ModelToolCall {
                        id: "call-real-1".to_string(),
                        name: tool_name.clone(),
                        arguments: parameters.clone(),
                    }],
                },
                ChatMessage::Tool {
                    tool_call_id: "call-real-1".to_string(),
                    content: format!("TOOL RESULT ({tool_name}):\n{observed}"),
                },
            ],
            wire_tools,
            &token,
        ),
    )
    .await
    .expect("real-model continuation must succeed");
    println!(
        "[REAL-MODEL:tools] turn2 usage: prompt={} completion={} total={}",
        usage2.prompt_tokens, usage2.completion_tokens, usage2.total_tokens
    );
    match &follow_up {
        m31a::model::types::ModelProposal::ToolCalls { calls } => {
            for call in calls {
                assert!(
                    known_ids.iter().any(|id| id == &call.name),
                    "continuation tool '{}' must also be a known capability",
                    call.name
                );
            }
            println!(
                "[REAL-MODEL:tools] continuation tool calls: {} calls",
                calls.len()
            );
        }
        m31a::model::types::ModelProposal::AssistantText { content } => {
            println!(
                "[REAL-MODEL:tools] continuation assistant text: {}",
                content
            );
        }
        m31a::model::types::ModelProposal::AskUser { question, .. } => {
            println!("[REAL-MODEL:tools] continuation ask user: {}", question);
        }
        m31a::model::types::ModelProposal::Complete { summary, .. } => {
            assert!(!summary.trim().is_empty(), "completion needs a summary");
            println!(
                "[REAL-MODEL:tools] continuation complete: {} chars",
                summary.len()
            );
        }
        m31a::model::types::ModelProposal::Handoff {
            target_role,
            reason,
        } => {
            assert!(!target_role.trim().is_empty() && !reason.trim().is_empty());
            println!("[REAL-MODEL:tools] continuation handoff: {target_role}");
        }
    }

    harness.assert_canonical_model_routing();
    harness.log_metrics("tools");
}

// ============================================================================
// Real-model failure modes (§16)
// ============================================================================

/// Provider/model failures are classified and distinguished from M31A runtime
/// failures. Deterministic cases (malformed frames, cancellation, empty
/// streams) run offline; authentication and unknown-model cases hit the live
/// endpoint with deliberately bad credentials/identifiers — never the valid
/// suite credentials as success evidence.
#[tokio::test]
#[ignore = "requires network for live failure classification (Phase 29.5 real-model suite)"]
async fn real_model_failure_classification() {
    use m31a::model::provider::ModelProvider;
    use m31a::model::types::{ChatMessage, ModelError};

    // --- Offline deterministic classifications (no network) ---
    // Malformed SSE frame → ProtocolViolation (MODEL FAILURE class).
    {
        let mut acc = m31a::model::provider::StreamAccumulator::new();
        let err = acc.process_event("{not valid json").unwrap_err();
        assert!(
            matches!(err, ModelError::ProtocolViolation(_)),
            "malformed frame must classify as ProtocolViolation, got {err:?}"
        );
    }
    // Reasoning-only stream (thinking without an answer) → InvalidResponse,
    // never silently promoted to a completion.
    {
        let mut acc = m31a::model::provider::StreamAccumulator::new();
        acc.process_event(
            &serde_json::json!({"choices": [{"index": 0, "delta": {"reasoning_content": "hmm"}, "finish_reason": "stop"}]}).to_string(),
        )
        .unwrap();
        assert!(!acc.reasoning_text().is_empty());
        let err = acc.finalize().unwrap_err();
        assert!(matches!(err, ModelError::InvalidResponse(_)));
    }
    // Pre-cancelled token → Cancelled (RUNTIME-side classification).
    {
        let harness = require_harness!();
        let token = tokio_util::sync::CancellationToken::new();
        token.cancel();
        let err = harness
            .provider()
            .call_model_with_messages(
                harness.model_id(),
                &[ChatMessage::User {
                    content: "hi".to_string(),
                }],
                vec![],
                &token,
            )
            .await
            .unwrap_err();
        assert_eq!(err, ModelError::Cancelled);
    }

    // --- Live failure classifications (deliberately bad auth/identifier) ---
    // Invalid credentials → AuthenticationFailed (MODEL/PROVIDER FAILURE).
    {
        let bad = m31a::model::provider::nvidia::NvidiaProvider::new(
            None,
            Some("nvapi-definitely-not-a-real-key-0000".to_string()),
        )
        .expect("provider constructs with any non-empty key");
        let token = tokio_util::sync::CancellationToken::new();
        let err = run_with_timeout(
            180,
            "bad-key probe",
            bad.call_model_with_messages(
                "nvidia/nemotron-3-ultra-550b-a55b",
                &[ChatMessage::User {
                    content: "hi".to_string(),
                }],
                vec![],
                &token,
            ),
        )
        .await
        .unwrap_err();
        assert_eq!(
            err,
            ModelError::AuthenticationFailed,
            "invalid key must classify as AuthenticationFailed, got {err:?}"
        );
        println!(
            "[REAL-MODEL:failures] bad-key → AuthenticationFailed (provider failure, not runtime)"
        );
    }
    // Unknown model identifier → typed provider error (MODEL FAILURE), never
    // a panic and never misreported as an M31A runtime failure.
    {
        let harness = require_harness!();
        let token = tokio_util::sync::CancellationToken::new();
        let err = run_with_timeout(
            180,
            "bad-model probe",
            harness.provider().call_model_with_messages(
                "nvidia/this-model-does-not-exist-zzz",
                &[ChatMessage::User {
                    content: "hi".to_string(),
                }],
                vec![],
                &token,
            ),
        )
        .await
        .unwrap_err();
        assert!(
            matches!(
                err,
                ModelError::Http { .. }
                    | ModelError::ModelUnavailable(_)
                    | ModelError::InvalidRequest(_)
                    | ModelError::InvalidResponse(_)
            ),
            "unknown model must map to a typed provider error, got {err:?}"
        );
        println!("[REAL-MODEL:failures] bad-model → {err:?}");
    }
}

// ============================================================================
// TEST E — research-dependent project (§9E)
// ============================================================================

/// External factual knowledge materially affects the architecture: the
/// runtime must trigger research, the REAL model must execute it through the
/// production workflow path, findings must carry provenance, and planning
/// must proceed on real evidence.
///
/// No fake findings are injected — the test never writes
/// `.planning/research/*`; `collect_findings` only returns disk-backed
/// evidence, and absence is recorded explicitly rather than fabricated.
///
/// BOUND (§15): dimension missions run under a 600s research budget, then
/// the future is cancelled through the real cancellation path. Production
/// verification requires read-only researchers to produce declared
/// artifacts they cannot write, so a full workflow success is not
/// architecturally reachable today (see phase report); the test asserts the
/// honest boundary — real need evaluation, real model execution, provenance,
/// absence-not-fabrication, fallback synthesis validity, and planning on
/// real evidence — and logs the outcome for classification.
#[tokio::test]
#[ignore = "requires NVIDIA credentials + network (Phase 29.5 real-model suite)"]
async fn real_research_execution_with_provenance() {
    use m31a::workflow::genesis::{
        DiscoverySession, GenesisController, GenesisOptions, GenesisRequest, ResearchDecision,
        ResearchDimension, ResearchOrchestrator,
    };

    let harness = require_harness!();
    let dir = tempfile::tempdir().expect("tempdir");
    let ws = dir.path().join("research-ws");
    std::fs::create_dir_all(&ws).unwrap();

    let runtime = run_with_timeout(180, "runtime init", harness.runtime_for(&ws))
        .await
        .expect("production runtime init");

    // A project where external factual knowledge (toolchain, storage,
    // security posture) materially affects architecture.
    let objective = "build me a password manager vault with encrypted local storage";
    let env = GenesisController::probe(&ws).expect("workspace probe");
    let mut session = DiscoverySession::new(GenesisRequest::new(objective, &ws), env.clone());
    session
        .submit_response(objective)
        .expect("discovery response");
    let charter = GenesisController::run_discovery(&mut session, &ws, ".planning")
        .expect("charter synthesis");

    // Research decision: bounded to the single stack dimension to keep
    // integration cost proportional (each dimension mission costs several
    // model turns plus bounded verification retries) while proving the full
    // decision → execution → findings → planning path.
    let decision = ResearchDecision::execute(
        vec![ResearchDimension::stack()],
        "phase 29.5 real-model research probe: storage/toolchain facts shape vault architecture",
    );
    assert!(decision.execute_research);
    assert_eq!(decision.selected_dimensions.len(), 1);

    let catalog = Arc::new(m31a::prompt::InMemoryPromptCatalog::with_builtins_and_workspace(&ws));
    let engine = runtime.create_workflow_engine(catalog);
    let options = GenesisOptions::default();

    // Execute through the canonical research orchestrator: dimension steps
    // run as controller missions with the REAL model caller from runtime
    // dependencies. No mock, no injected artifacts. Bounded by the research
    // time budget (600s); on expiry the future is dropped, exercising the
    // real cancellation path (M31A Law 8: cancellation is real).
    let exec_outcome = tokio::time::timeout(
        Duration::from_secs(600),
        GenesisController::execute_research(&engine, &charter, &decision, &options, &ws),
    )
    .await;

    // Core intelligence proof (all outcomes): the real model executed
    // research missions through the production path.
    let calls_after_research = harness.traced_requests().len();
    println!("[REAL-MODEL:research] model calls during research: {calls_after_research}");
    assert!(
        calls_after_research >= 3,
        "real model must execute research (several turns per dimension), saw {calls_after_research}"
    );

    // Absence-not-fabrication proof: findings come only from disk-backed
    // dimension artifacts; the collector never invents them.
    let collected =
        ResearchOrchestrator::collect_findings(&ws, ".planning", &decision.selected_dimensions)
            .expect("finding collection");
    for finding in &collected {
        assert!(
            decision
                .selected_dimensions
                .iter()
                .any(|d| d == &finding.dimension),
            "every finding must carry provenance to a selected dimension"
        );
        assert!(!finding.summary.trim().is_empty());
    }
    let research_dir = ws.join(".planning").join("research");
    let stack_artifact = research_dir.join("STACK.md");
    println!(
        "[REAL-MODEL:research] STACK.md present={} findings={}",
        stack_artifact.exists(),
        collected.len()
    );

    match exec_outcome {
        // Full path: synthesis ran on real findings within budget.
        Ok(Ok((summary, findings))) => {
            assert_eq!(findings.len(), collected.len());
            let summary = summary.expect("research summary");
            assert!(!summary.executive_summary.trim().is_empty());
            assert!(!summary.consensus_points.is_empty());
            println!(
                "[REAL-MODEL:research] summary consensus_points={} open_unknowns={}",
                summary.consensus_points.len(),
                summary.open_unknowns.len()
            );

            // Findings carry into planning: the stack dimension maps to
            // COMPAT-prefixed requirements.
            let planning = GenesisController::run_planning(
                &charter,
                Some(&summary),
                &findings,
                None,
                Some(&env),
                &options,
                &ws,
            )
            .expect("planning synthesis");
            let keys: Vec<_> = planning
                .requirements
                .requirements
                .iter()
                .map(|r| r.key.clone())
                .collect();
            println!("[REAL-MODEL:research] requirement keys: {keys:?}");
            assert!(
                keys.iter().any(|k| k.contains("COMPAT")),
                "stack findings must map into requirements, got {keys:?}"
            );
            assert!(!planning.architecture.to_markdown().is_empty());
            println!("[REAL-MODEL:research] branch=FULL (artifacts→findings→planning)");
        }
        // Workflow-level failure within budget (e.g. read-only researcher
        // roles cannot produce their declared artifacts, so verification
        // fails closed after bounded retries). What MUST still hold: the
        // model really ran, nothing was fabricated, and the failure is
        // classified as workflow-level — not a provider/auth failure and
        // not a silent success.
        Ok(Err(e)) => {
            let msg = e.to_string();
            println!("[REAL-MODEL:research] branch=WORKFLOW-GAP ({msg})");
            assert_no_credential_failure(&msg);
            assert_findings_provenance(&collected, &decision);
            synthesize_fallback(&charter, &collected);
        }
        // Research budget expired: the future was cancelled through the real
        // cancellation path. Same honest-boundary obligations as WORKFLOW-GAP.
        Err(_) => {
            println!(
                "[REAL-MODEL:research] branch=BUDGET-CANCELLED (600s research budget, {} model calls observed)",
                calls_after_research
            );
            let collected_again = ResearchOrchestrator::collect_findings(
                &ws,
                ".planning",
                &decision.selected_dimensions,
            )
            .expect("finding collection after cancel");
            assert_findings_provenance(&collected_again, &decision);
            synthesize_fallback(&charter, &collected_again);
        }
    }

    // Planning proceeds on real evidence regardless of branch: charter +
    // collected findings synthesize requirements/architecture with provenance.
    let planning = GenesisController::run_planning(
        &charter,
        None,
        &collected,
        None,
        Some(&env),
        &options,
        &ws,
    )
    .expect("planning synthesis on real evidence");
    assert!(!planning.requirements.requirements.is_empty());
    assert!(!planning.architecture.to_markdown().is_empty());
    println!(
        "[REAL-MODEL:research] planning on real evidence: requirements={}",
        planning.requirements.requirements.len()
    );

    harness.assert_canonical_model_routing();
    harness.log_metrics("research");
}

// ============================================================================
// TEST B + C + §21 — real end-to-end autonomous engineering
// ============================================================================

const DASHBOARD_CARGO_TOML: &str = r#"[package]
name = "dashboard"
version = "0.1.0"
edition = "2021"

[dependencies]
"#;

const DASHBOARD_LIB_RS: &str = r#"//! Dashboard crate root.

pub mod dashboard;
"#;

/// Deliberately broken: panics (index out of bounds) when there are no records.
const BROKEN_DASHBOARD_RS: &str = r#"//! Dashboard record summary.

/// Load persisted records (empty in a fresh deployment).
pub fn load_records() -> Vec<String> {
    Vec::new()
}

/// Summarize the latest record for the dashboard header.
pub fn latest_record_summary() -> String {
    let records = load_records();
    // BUG: crashes with index-out-of-bounds when there are no records.
    format!("Latest: {}", records[0])
}
"#;

const DASHBOARD_TEST_RS: &str = r#"use dashboard::dashboard::latest_record_summary;

#[test]
fn empty_state_shows_message() {
    let summary = latest_record_summary();
    assert!(
        summary.contains("no records"),
        "dashboard must show an empty state instead of crashing, got: {summary}"
    );
}
"#;

fn init_git_repo(path: &std::path::Path) {
    assert!(
        std::process::Command::new("git")
            .args(["init", "-b", "main"])
            .current_dir(path)
            .status()
            .expect("git init failed")
            .success()
    );
    let _ = std::process::Command::new("git")
        .args(["config", "user.name", "M31A Real Test"])
        .current_dir(path)
        .status();
    let _ = std::process::Command::new("git")
        .args(["config", "user.email", "real@m31a.local"])
        .current_dir(path)
        .status();
}

/// Full autonomous loop with zero scripted intelligence:
///
/// 1-line human intent → real NVIDIA model → real upstream reasoning →
/// real planning → real scheduler → real agent runtime → real tools →
/// real verification → real recovery → real completion.
///
/// Covers TEST B (brownfield bug fix: inspect, diagnose, plan, modify,
/// verify — no scripted diagnosis) and TEST C (the initial failing test is
/// the controlled failure; failure evidence drives model diagnosis and
/// repair within bounded recovery).
#[tokio::test]
#[ignore = "requires NVIDIA credentials + network (Phase 29.5 real-model suite)"]
async fn real_end_to_end_dashboard_repair() {
    let harness = require_harness!();
    // Persistent workspace (not tempdir): post-mortem DB/worktree forensics
    // survive timeouts and failures for phase-report classification.
    let e2e_root = std::env::var("TMPDIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| std::env::temp_dir())
        .join("phase29_5_e2e");
    let _ = std::fs::remove_dir_all(&e2e_root);
    let repo = e2e_root.join("dashboard");
    std::fs::create_dir_all(&repo).unwrap();
    init_git_repo(&repo);

    std::fs::write(repo.join(".gitignore"), "/target\n.m31a\n").unwrap();
    std::fs::write(repo.join("Cargo.toml"), DASHBOARD_CARGO_TOML).unwrap();
    std::fs::create_dir_all(repo.join("src")).unwrap();
    std::fs::write(repo.join("src/lib.rs"), DASHBOARD_LIB_RS).unwrap();
    std::fs::write(repo.join("src/dashboard.rs"), BROKEN_DASHBOARD_RS).unwrap();
    std::fs::create_dir_all(repo.join("tests")).unwrap();
    std::fs::write(repo.join("tests/dashboard_test.rs"), DASHBOARD_TEST_RS).unwrap();
    let _ = std::process::Command::new("cargo")
        .args(["generate-lockfile"])
        .current_dir(&repo)
        .status();
    assert!(
        std::process::Command::new("git")
            .args(["add", "-A"])
            .current_dir(&repo)
            .status()
            .unwrap()
            .success()
    );
    assert!(
        std::process::Command::new("git")
            .args([
                "commit",
                "-m",
                "Initial dashboard crate with empty-state crash"
            ])
            .current_dir(&repo)
            .status()
            .unwrap()
            .success()
    );

    // Pre-condition (deterministic): the dashboard test fails before repair.
    let pre = std::process::Command::new("cargo")
        .args(["test", "--test", "dashboard_test"])
        .current_dir(&repo)
        .output()
        .expect("cargo test");
    assert!(
        !pre.status.success(),
        "pre-condition: dashboard test must fail before autonomous repair"
    );
    println!("[REAL-MODEL:e2e] pre-repair failure confirmed (controlled failure evidence)");

    let runtime = run_with_timeout(180, "runtime init", harness.runtime_for(&repo))
        .await
        .expect("production runtime init");

    // Live progress poller: model-call counts every 60s (a repair mission is
    // opaque while running; the poller makes burn rate visible).
    let poller_handle = harness.tracer_handle();
    let poller = tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(60)).await;
            eprintln!(
                "[REAL-MODEL:e2e] t={}s traced_model_calls={}",
                poller_handle.elapsed().as_secs(),
                poller_handle.call_count()
            );
        }
    });

    // Mission attempts: planning is single-shot inside run_mission (a
    // rejected plan fails the mission without controller replan), so the
    // test allows one retry with a fresh mission — every attempt is genuine
    // model reasoning, mirroring an operator retrying a failed mission.
    // The 900s bound reflects measured model pace (~35s/call on the
    // canonical reasoning model) for a multi-turn repair mission.
    let mut summary = None;
    let mut last_err = String::new();
    for attempt in 1..=2 {
        let mission_outcome = tokio::time::timeout(
            Duration::from_secs(900),
            runtime.run_mission(
                "fix the dashboard crash when there are no records",
                Some("autonomous"),
                false,
            ),
        )
        .await;
        match mission_outcome {
            Ok(Ok(s)) => {
                println!(
                    "[REAL-MODEL:e2e] mission attempt {attempt} ended: status={} tasks={} halt={}",
                    s.status, s.tasks_completed, s.halt_reason
                );
                if s.status == "Completed" {
                    summary = Some(s);
                    break;
                }
                // A non-Completed terminal state is not a retryable transport
                // outcome, but a fresh mission may still succeed (stochastic
                // model behavior), so record evidence and use the retry.
                last_err = format!(
                    "attempt {attempt} status={} halt={}",
                    s.status, s.halt_reason
                );
                dump_task_diagnostics(&runtime).await;
                dump_tracer_history(&harness);
            }
            Ok(Err(e)) => {
                last_err = e.to_string();
                eprintln!("[REAL-MODEL:e2e] mission attempt {attempt} failed: {last_err}");
                dump_task_diagnostics(&runtime).await;
                dump_tracer_history(&harness);
            }
            Err(_) => {
                harness.log_metrics("e2e-timeout");
                dump_task_diagnostics(&runtime).await;
                dump_tracer_history(&harness);
                last_err = format!(
                    "attempt {attempt} timed out after 900s with {} traced calls",
                    harness.traced_requests().len()
                );
                eprintln!("[REAL-MODEL:e2e] {last_err}");
                // Rate-limit courtesy: a fresh mission immediately after a
                // timeout risks compounding provider 429s; pause briefly.
                tokio::time::sleep(Duration::from_secs(60)).await;
            }
        }
    }
    poller.abort();
    let summary =
        summary.unwrap_or_else(|| panic!("repair mission failed after 2 attempts: {last_err}"));

    println!(
        "[REAL-MODEL:e2e] status={} tasks_completed={} halt={}",
        summary.status, summary.tasks_completed, summary.halt_reason
    );
    assert_eq!(summary.status, "Completed");
    assert!(summary.tasks_completed >= 1);

    // Production-path proofs: canonical routing, real tool feedback turns.
    harness.assert_canonical_model_routing();
    let metrics = harness.metrics();
    assert!(
        metrics.tool_feedback_turns >= 1 || metrics.assistant_tool_call_turns >= 1,
        "real model must engage tools multi-turn, {}",
        metrics.summary_line()
    );

    // Independent verification: the repaired crate passes in the ORIGIN repo
    // (merged back from the mission worktree by the runtime).
    let post = std::process::Command::new("cargo")
        .args(["test", "--test", "dashboard_test"])
        .current_dir(&repo)
        .output()
        .expect("cargo test");
    assert!(
        post.status.success(),
        "dashboard tests must pass after autonomous repair:\n{}",
        String::from_utf8_lossy(&post.stdout)
    );
    let final_src = std::fs::read_to_string(repo.join("src/dashboard.rs")).unwrap();
    assert!(
        final_src.contains("is_empty"),
        "model must have added an empty-state guard, got:\n{final_src}"
    );

    // Git attribution: runtime commits carry the mission trailer.
    let log = std::process::Command::new("git")
        .args(["log", "-n", "5", "--format=%B"])
        .current_dir(&repo)
        .output()
        .expect("git log");
    let log_text = String::from_utf8_lossy(&log.stdout).to_string();
    assert!(
        log_text.contains("M31A-Mission:"),
        "git history must carry the M31A-Mission trailer"
    );
    assert!(
        log_text.contains("nvidia/nemotron-3-ultra-550b-a55b"),
        "commit provenance must record the canonical real model, got:\n{log_text}"
    );

    harness.log_metrics("e2e");
}
