//! Phase 29 golden autonomous-engineering suite.
//!
//! Classification (Phase 29.5): DETERMINISTIC_CONTRACT_TEST — runtime
//! orchestration regression. The model is scripted and deterministic
//! (`RoutingMock`, `ScriptedDiagnosis`); every other seam is production:
//! real tools in temp workspaces, real verification runners, real SQLite
//! state, real bounded recovery. Assertions target behavior and evidence,
//! never mock internals. This file proves orchestration, NOT model
//! intelligence — authoritative behavioral proof lives in
//! `tests/phase_29_5_real_model.rs` (real NVIDIA model).

use async_trait::async_trait;
use std::collections::{HashMap, VecDeque};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use tempfile::tempdir;
use tokio_util::sync::CancellationToken;

use m31a::agent::model_policy::{ModelCaller, ModelProposal, ModelToolCall};
use m31a::kernel::seams::context::CompiledContext;
use m31a::runtime::AppRuntime;

// ============================================================================
// Scripted routing mock: deterministic, content-routed, call-counting
// ============================================================================

/// Routes prompts by content markers so research/planning/diagnosis/steps
/// each receive deterministic responses through the real orchestration.
/// Research tasks receive plain completions (plus research artifacts written
/// to disk once, mirroring the established harness pattern).
struct RoutingMock {
    calls: AtomicUsize,
    prompts_seen: Mutex<Vec<String>>,
    plan_json: String,
    scripts: Mutex<HashMap<String, VecDeque<ModelProposal>>>,
    workspace: std::path::PathBuf,
}

impl RoutingMock {
    fn new(plan_json: String, workspace: std::path::PathBuf) -> Self {
        Self {
            calls: AtomicUsize::new(0),
            prompts_seen: Mutex::new(Vec::new()),
            plan_json,
            scripts: Mutex::new(HashMap::new()),
            workspace,
        }
    }

    fn with_task_script(self, key: &str, steps: Vec<ModelProposal>) -> Self {
        self.scripts
            .lock()
            .unwrap()
            .insert(key.to_string(), steps.into());
        self
    }

    fn call_count(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }

    fn planning_prompt_seen(&self) -> Option<String> {
        self.prompts_seen
            .lock()
            .unwrap()
            .iter()
            .find(|p| p.contains("Goal to Decompose"))
            .cloned()
    }

    fn write_research_artifacts(&self) {
        let research_dir = self.workspace.join(".planning").join("research");
        let _ = std::fs::create_dir_all(&research_dir);
        for (name, body) in [
            ("STACK.md", "# Technology Stack\n\nRust 2024 edition.\n"),
            (
                "FEATURES.md",
                "# Product Features\n\nCore domain workflows.\n",
            ),
            (
                "ARCHITECTURE.md",
                "# System Architecture\n\nLayered modular design.\n",
            ),
            (
                "PITFALLS.md",
                "# Known Pitfalls\n\nAvoid unverified state.\n",
            ),
            (
                "SECURITY.md",
                "# Security Notes\n\nValidate untrusted inputs.\n",
            ),
            (
                "DEPLOYMENT.md",
                "# Deployment\n\nSingle local deployable unit.\n",
            ),
            (
                "SUMMARY.md",
                "# Research Summary\n\nEvidence-backed baseline.\n",
            ),
        ] {
            let _ = std::fs::write(research_dir.join(name), body);
        }
    }
}

fn action(tool: &str, params: serde_json::Value) -> ModelProposal {
    ModelProposal::ToolCalls {
        calls: vec![ModelToolCall::new(tool, params)],
    }
}

fn complete(summary: &str) -> ModelProposal {
    ModelProposal::Complete {
        summary: summary.to_string(),
        artifacts: Vec::new(),
    }
}

#[async_trait]
impl ModelCaller for RoutingMock {
    async fn call_model(&self, context: &str) -> Result<ModelProposal, String> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.prompts_seen.lock().unwrap().push(context.to_string());
        if context.contains("Goal to Decompose") {
            return Ok(complete(&self.plan_json.clone()));
        }
        // Diagnostician prompts fall through to heuristic-compatible output;
        // recovery degrades honestly when the model cannot help.
        Ok(complete("No further model input available."))
    }

    async fn call_model_with_context(
        &self,
        compiled: &CompiledContext,
        _cancellation: &CancellationToken,
    ) -> Result<ModelProposal, String> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let mut text = compiled.system_prompt.clone();
        for m in &compiled.messages {
            text.push_str(&format!("{:?}", m));
        }
        // Research steps (titled "Research: …") need their dimension
        // artifacts on disk for artifact verification, mirroring the
        // established harness pattern. Task steps must never fabricate
        // research files as a side effect.
        if text.contains("Research:") {
            self.write_research_artifacts();
        }
        let mut scripts = self.scripts.lock().unwrap();
        for (key, queue) in scripts.iter_mut() {
            if text.contains(key.as_str()) {
                if let Some(next) = queue.pop_front() {
                    return Ok(next);
                }
                return Ok(complete("Task step complete."));
            }
        }
        Ok(complete("Step complete, no further actions."))
    }
}

fn init_git_repo(path: &std::path::Path) {
    assert!(
        Command::new("git")
            .args(["init", "-b", "main"])
            .current_dir(path)
            .status()
            .expect("git init failed")
            .success()
    );
    let _ = Command::new("git")
        .args(["config", "user.name", "M31A Test Agent"])
        .current_dir(path)
        .status();
    let _ = Command::new("git")
        .args(["config", "user.email", "agent@m31a.local"])
        .current_dir(path)
        .status();
}

fn commit_all(path: &std::path::Path, msg: &str) {
    assert!(
        Command::new("git")
            .args(["add", "-A"])
            .current_dir(path)
            .status()
            .expect("git add failed")
            .success()
    );
    assert!(
        Command::new("git")
            .args(["commit", "-m", msg])
            .current_dir(path)
            .status()
            .expect("git commit failed")
            .success()
    );
}

// ============================================================================
// Fixtures
// ============================================================================

const BROKEN_LIB_RS: &str = r#"//! Widget turnaround calculator.

pub fn turnaround() -> i32 {
    // Incorrect baseline: returns zero instead of the measured value.
    0
}
"#;

const FIXED_LIB_RS: &str = r#"//! Widget turnaround calculator.

pub fn turnaround() -> i32 {
    // Measured widget turnaround in hours, verified by widget tests.
    5
}
"#;

const WIDGET_TEST_RS: &str = r#"use widget::turnaround;

#[test]
fn test_alpha_ok() {
    assert_eq!(turnaround(), 5);
}

#[test]
fn test_beta_ok() {
    assert!(turnaround() >= 0);
}
"#;

const WIDGET_CARGO_TOML: &str = r#"[package]
name = "widget"
version = "0.1.0"
edition = "2021"

[dependencies]
"#;

/// Rust fixture crate with one failing and one passing test.
fn setup_widget_fixture(path: &std::path::Path) {
    init_git_repo(path);
    std::fs::write(path.join(".gitignore"), "/target\n.m31a\n").unwrap();
    std::fs::write(path.join("Cargo.toml"), WIDGET_CARGO_TOML).unwrap();
    std::fs::create_dir_all(path.join("src")).unwrap();
    std::fs::write(path.join("src/lib.rs"), BROKEN_LIB_RS).unwrap();
    std::fs::create_dir_all(path.join("tests")).unwrap();
    std::fs::write(path.join("tests/widget_test.rs"), WIDGET_TEST_RS).unwrap();
    // Generate + commit the lockfile first so the later worktree merge never
    // collides with an untracked Cargo.lock (same precaution as the
    // established golden repair fixture).
    let _ = Command::new("cargo")
        .args(["generate-lockfile"])
        .current_dir(path)
        .status();
    commit_all(path, "Initial widget crate with failing alpha test");
    // Baseline: crate compiles, alpha fails, beta passes.
    let check = Command::new("cargo")
        .args(["test", "--test", "widget_test", "--no-run"])
        .current_dir(path)
        .output()
        .expect("cargo unavailable");
    assert!(check.status.success(), "fixture must compile");
    let run = Command::new("cargo")
        .args(["test", "--test", "widget_test"])
        .current_dir(path)
        .output()
        .expect("cargo test failed to invoke");
    assert!(!run.status.success(), "alpha must fail pre-mission");
}

// ============================================================================
// Golden 1 — bug fix with declared-command verification + recovery (F/G/H)
// ============================================================================

/// The task declares `automated_test:<alpha-only command>`. The mock first
/// completes without fixing (verification MUST fail), then repairs. Mission
/// completion plus the persisted check row prove the declared command ran —
/// the default `cargo test` is never what verified the task.
#[tokio::test]
async fn golden_bugfix_declared_command_verified_with_recovery() {
    let dir = tempdir().unwrap();
    let repo = dir.path();
    setup_widget_fixture(repo);

    let plan = serde_json::json!({ "tasks": [{
        "id": "TASK-01",
        "title": "Repair widget turnaround",
        "description": "Fix turnaround() to return the measured value of 5",
        "depends_on": [],
        "required_capabilities": ["fs.read", "fs.write", "cargo.test"],
        "role": "implementer",
        "verification": "automated_test:cargo test --test widget_test -- --exact test_alpha_ok",
        "completion_criteria": ["test_alpha_ok passes"],
        "requirement_keys": ["REQ-FUNC-01"],
    }] })
    .to_string();

    let mock = Arc::new(RoutingMock::new(plan, repo.to_path_buf()).with_task_script(
        "Repair widget",
        vec![
            // Turn 1: write a still-broken implementation (compiles, but
            // alpha keeps failing).
            action(
                "write_file",
                serde_json::json!({
                    "path": "src/lib.rs",
                    "content": "//! Widget turnaround calculator.\n\npub fn turnaround() -> i32 {\n    1\n}\n",
                }),
            ),
            // Turn 2: run the always-green beta subset -> passing evidence
            // recorded AFTER the mutation (satisfies the runner gate).
            action(
                "run_tests",
                serde_json::json!({"args": ["--test", "widget_test", "--", "--exact", "test_beta_ok"]}),
            ),
            // Turn 3: complete (gate satisfied) -> controller Tier3 with the
            // DECLARED alpha-only command MUST fail -> ClassifyFailure ->
            // bounded Retry with memory diagnosis -> re-execution.
            complete("Widget turnaround repaired."),
            // Turn 4 (post-retry, fresh worker): genuinely fixed implementation.
            action(
                "write_file",
                serde_json::json!({"path": "src/lib.rs", "content": FIXED_LIB_RS}),
            ),
            // Turn 5: observe the green suite, then complete.
            action("run_tests", serde_json::json!({"args": ["--test", "widget_test"]})),
            // Turn 6: complete -> Tier3 passes -> evidence-gated completion.
            complete("Widget turnaround repaired and verified."),
        ],
    ));
    let runtime = AppRuntime::new(repo)
        .await
        .expect("runtime")
        .with_model_caller(mock.clone());

    let summary = runtime
        .run_mission(
            "Fix the widget turnaround function so tests pass",
            Some("autonomous"),
            false,
        )
        .await
        .expect("mission executes");
    assert_eq!(summary.status, "Completed");
    assert!(summary.tasks_completed >= 1);

    // Working software: fixed file on disk, full suite green.
    let lib = std::fs::read_to_string(repo.join("src/lib.rs")).unwrap();
    assert!(lib.contains('5'), "implementation must carry the fix");
    assert!(!lib.contains("returns zero"), "stale content must be gone");
    let post = Command::new("cargo")
        .args(["test", "--test", "widget_test"])
        .current_dir(repo)
        .output()
        .expect("post cargo test");
    assert!(post.status.success(), "full suite must pass after mission");

    // Declared-command proof: a persisted verification check names the exact
    // declared command (not the default `cargo test`).
    let rows: Vec<(String,)> = sqlx::query_as(
        "SELECT command_or_tool FROM verification_checks WHERE command_or_tool LIKE '%--exact%'",
    )
    .fetch_all(runtime.pool())
    .await
    .expect("query checks");
    assert!(
        !rows.is_empty(),
        "a verification check must record the declared test command"
    );

    // Recovery memory: the initial failure persisted a diagnosis row.
    use m31a::memory::repository::EngineeringMemoryStore;
    let diags =
        m31a::memory::repository::SqliteEngineeringMemoryRepository::new(runtime.pool().clone());
    let found = diags
        .list_diagnoses(summary.mission_id, None)
        .await
        .expect("list diagnoses");
    assert!(
        !found.is_empty(),
        "execution failure must persist a memory diagnosis"
    );
}

// ============================================================================
// Golden 2 — greenfield expense tracker on a toolchain-free workspace (A/H)
// ============================================================================

/// Empty directory, minimal intent, no Rust/Python markers: verification
/// must stay target-neutral (not-applicable tiers pass explicitly) and the
/// mission must deliver real files verified by artifact existence.
#[tokio::test]
async fn golden_greenfield_expense_tracker_delivers_artifacts() {
    let dir = tempdir().unwrap();
    let repo = dir.path();
    init_git_repo(repo);
    std::fs::write(repo.join(".gitkeep"), "").unwrap();
    commit_all(repo, "empty start");

    let plan = serde_json::json!({ "tasks": [
        {
            "id": "TASK-01",
            "title": "Create expense ledger module",
            "description": "Implement expense.py with add_expense and total functions",
            "depends_on": [],
            "required_capabilities": ["fs.read", "fs.write"],
            "role": "implementer",
            "verification": "artifact_inspection",
            "expected_outputs": ["expense.py"],
            "completion_criteria": ["expense.py exists with add_expense function"],
            "requirement_keys": ["REQ-FUNC-01"],
        },
        {
            "id": "TASK-02",
            "title": "Verify expense ledger",
            "description": "Confirm the ledger module exists and is importable",
            "depends_on": ["TASK-01"],
            "required_capabilities": ["fs.read"],
            "role": "verifier",
            "verification": "artifact_inspection",
            "expected_outputs": ["expense.py"],
            "completion_criteria": ["expense.py present on disk"],
        },
    ] })
    .to_string();

    let ledger = "def add_expense(ledger, amount, category):\n    ledger.append({\"amount\": amount, \"category\": category})\n\n\ndef total(ledger):\n    return sum(e[\"amount\"] for e in ledger)\n";
    let mock = Arc::new(RoutingMock::new(plan, repo.to_path_buf()).with_task_script(
        "ledger module",
        vec![
            action(
                "write_file",
                serde_json::json!({"path": "expense.py", "content": ledger}),
            ),
            complete("Expense ledger module implemented."),
        ],
    ));
    let runtime = AppRuntime::new(repo)
        .await
        .expect("runtime")
        .with_model_caller(mock.clone());

    let summary = runtime
        .run_mission("build me an expense tracker", Some("autonomous"), false)
        .await
        .expect("mission executes");
    assert_eq!(summary.status, "Completed");
    assert!(summary.tasks_completed >= 2);

    // Working software, not placeholders.
    let content = std::fs::read_to_string(repo.join("expense.py")).expect("ledger exists");
    assert!(content.contains("def add_expense"));
    assert!(content.contains("def total"));
    for forbidden in ["TODO", "placeholder", "DomainItem", "pass  #"] {
        assert!(
            !content.contains(forbidden),
            "generated artifact must not contain {forbidden}"
        );
    }

    // Research ran through the generic pipeline and stayed honest: the
    // planning prompt seen by the model must carry upstream requirements.
    let planning_prompt = mock
        .planning_prompt_seen()
        .expect("planning prompt captured");
    assert!(
        planning_prompt.contains("REQ-"),
        "planning must observe upstream requirements, got: {}",
        &planning_prompt[..planning_prompt.len().min(400)]
    );
}

// ============================================================================
// Golden 3 — trivial change stays cheap (C/E + economics §25)
// ============================================================================

/// "fix typo in README" must skip genesis research, ask zero questions, and
/// complete with strictly fewer model calls than a greenfield mission.
#[tokio::test]
async fn golden_trivial_change_stays_cheap() {
    // Metric support: a trivial change asks zero discovery questions
    // (Medium tier converges immediately) and skips research by decision.
    {
        use m31a::workflow::genesis::discovery::{DiscoverySession, classify_workflow_tier};
        use m31a::workflow::genesis::{GenesisRequest, WorkflowTier, WorkspaceEnvironment};
        let probe_dir = tempdir().unwrap();
        let env = WorkspaceEnvironment::probe(probe_dir.path()).unwrap();
        assert_eq!(
            classify_workflow_tier("fix typo in README", &env),
            WorkflowTier::Medium
        );
        let mut session = DiscoverySession::new(
            GenesisRequest::new("fix typo in README", probe_dir.path()),
            env,
        );
        assert!(
            session.next_turn().expect("turn check").is_none(),
            "trivial change must ask zero questions"
        );
        assert!(!session.requires_user_decision());
    }
    let trivial_dir = tempdir().unwrap();
    let trivial_repo = trivial_dir.path();
    init_git_repo(trivial_repo);
    std::fs::write(
        trivial_repo.join("README.md"),
        "# Widget\n\nA usefull tool.\n",
    )
    .unwrap();
    commit_all(trivial_repo, "readme with typo");

    let trivial_plan = serde_json::json!({ "tasks": [{
        "id": "TASK-01",
        "title": "Fix README typo",
        "description": "Correct 'usefull' to 'useful' in README.md",
        "depends_on": [],
        "required_capabilities": ["fs.read", "fs.write"],
        "role": "implementer",
        "verification": "artifact_inspection",
        "expected_outputs": ["README.md"],
        "completion_criteria": ["README.md contains 'useful'"],
    }] })
    .to_string();
    let trivial_mock = Arc::new(
        RoutingMock::new(trivial_plan, trivial_repo.to_path_buf()).with_task_script(
            "README typo",
            vec![
                action(
                    "write_file",
                    serde_json::json!({
                        "path": "README.md",
                        "content": "# Widget\n\nA useful tool.\n"
                    }),
                ),
                complete("Typo fixed."),
            ],
        ),
    );
    let trivial_runtime = AppRuntime::new(trivial_repo)
        .await
        .expect("runtime")
        .with_model_caller(trivial_mock.clone());
    let trivial_summary = trivial_runtime
        .run_mission("fix typo in README", Some("autonomous"), false)
        .await
        .expect("trivial mission executes");
    assert_eq!(trivial_summary.status, "Completed");
    let readme = std::fs::read_to_string(trivial_repo.join("README.md")).unwrap();
    assert!(readme.contains("useful"));
    // No genesis research artifacts for a fast-path change.
    assert!(
        !trivial_repo.join(".planning/research").exists()
            || std::fs::read_dir(trivial_repo.join(".planning/research"))
                .map(|mut d| d.next().is_none())
                .unwrap_or(true),
        "trivial change must not run research"
    );

    // --- Greenfield mission (same harness, minimal plan) ---
    let green_dir = tempdir().unwrap();
    let green_repo = green_dir.path();
    init_git_repo(green_repo);
    std::fs::write(green_repo.join(".gitkeep"), "").unwrap();
    commit_all(green_repo, "empty start");
    let green_plan = serde_json::json!({ "tasks": [{
        "id": "TASK-01",
        "title": "Create notes module",
        "description": "Write notes.py with a capture function",
        "depends_on": [],
        "required_capabilities": ["fs.read", "fs.write"],
        "role": "implementer",
        "verification": "artifact_inspection",
        "expected_outputs": ["notes.py"],
        "completion_criteria": ["notes.py exists"],
    }] })
    .to_string();
    let green_mock = Arc::new(
        RoutingMock::new(green_plan, green_repo.to_path_buf()).with_task_script(
            "notes module",
            vec![
                action(
                    "write_file",
                    serde_json::json!({
                        "path": "notes.py",
                        "content": "def capture(notes, text):\n    notes.append(text)\n"
                    }),
                ),
                complete("Notes module done."),
            ],
        ),
    );
    let green_runtime = AppRuntime::new(green_repo)
        .await
        .expect("runtime")
        .with_model_caller(green_mock.clone());
    let green_summary = green_runtime
        .run_mission("build me a notes app", Some("autonomous"), false)
        .await
        .expect("greenfield mission executes");
    assert_eq!(green_summary.status, "Completed");

    // Proportionality: trivial work costs strictly fewer model calls.
    assert!(
        trivial_mock.call_count() < green_mock.call_count(),
        "trivial ({}) must cost less than greenfield ({})",
        trivial_mock.call_count(),
        green_mock.call_count()
    );
}

// ============================================================================
// Deterministic (model-free) behavioral tests — generic machinery only
// ============================================================================

use m31a::kernel::seams::planner::{PlanRequest, PlanService};
use m31a::planning::requirements::RequirementPriority;
use m31a::planning::service::PlanServiceImpl;
use m31a::workflow::genesis::discovery::{
    DiscoverySession, classify_workflow_tier, extract_unknowns, infer_domain_model,
};
use m31a::workflow::genesis::{GenesisRequest, ProjectCharter, WorkflowTier, WorkspaceEnvironment};
use m31a::workflow::planning::architecture_synthesizer::ArchitectureSynthesizer;
use m31a::workflow::planning::requirements_synthesizer::RequirementsSynthesizer;
use m31a::workflow::planning::risks::RiskRegister;
use m31a::workflow::planning::roadmap::RoadmapSynthesizer;

fn charter_for(prompt: &str, scopes: Vec<&str>) -> ProjectCharter {
    let mut charter = ProjectCharter::new(prompt, prompt);
    charter.boundaries.in_scope = scopes.into_iter().map(str::to_string).collect();
    charter.problem_statement = prompt.to_string();
    charter
}

/// Three materially different domains must yield materially different
/// engineering artifacts from the same generic pipeline (no model).
#[test]
fn golden_three_domains_differ_without_new_code() {
    let risks = RiskRegister::new();
    let mut artifacts = Vec::new();
    for (name, scopes) in [
        ("Expense Tracker", vec!["Record expense with amount"]),
        (
            "Student Tasks",
            vec!["Create assignment with due date", "Remind before deadline"],
        ),
        (
            "Inventory System",
            vec![
                "Receive stock shipment",
                "Pick order from shelves",
                "Count cycle inventory",
            ],
        ),
    ] {
        let charter = charter_for(name, scopes);
        let reqs = RequirementsSynthesizer::synthesize(&charter, None, &[], None, None).unwrap();
        let (arch, _) =
            ArchitectureSynthesizer::synthesize(&reqs, &charter, None, &[], None).unwrap();
        let roadmap =
            RoadmapSynthesizer::synthesize(&reqs, &arch, &adrs_empty(), &risks, &charter, None)
                .unwrap();
        let req_text: Vec<_> = reqs
            .requirements
            .iter()
            .map(|r| r.description.clone())
            .collect();
        let sub_ids: Vec<_> = arch.subsystems.iter().map(|s| s.id.clone()).collect();
        let phase_names: Vec<_> = roadmap.phases.iter().map(|p| p.name.clone()).collect();
        artifacts.push((req_text, sub_ids, phase_names));
    }
    // Pairwise differentiation on all three artifact kinds.
    for i in 0..3 {
        for j in (i + 1)..3 {
            assert_ne!(
                artifacts[i].0, artifacts[j].0,
                "requirements must differ by domain"
            );
            assert_ne!(
                artifacts[i].1, artifacts[j].1,
                "architecture identities must differ by domain"
            );
            assert_ne!(
                artifacts[i].2, artifacts[j].2,
                "roadmap phases must differ by domain"
            );
        }
    }
}

fn adrs_empty() -> m31a::workflow::planning::adr::AdrRegistry {
    m31a::workflow::planning::adr::AdrRegistry::new()
}

/// Consequential decisions surface instead of being silently invented.
#[test]
fn golden_consequential_decision_surfaced_not_invented() {
    let dir = tempdir().unwrap();
    let env = WorkspaceEnvironment::probe(dir.path()).unwrap();
    let prompt = "turn this project into a multi-tenant SaaS with live billing";
    assert_eq!(
        classify_workflow_tier(prompt, &env),
        WorkflowTier::Consequential
    );
    let unknowns = extract_unknowns(prompt, WorkflowTier::Consequential);
    assert!(
        unknowns.iter().any(|u| matches!(
            u.fate,
            m31a::planning::risks::UnknownFate::UserDecisionRequired
                | m31a::planning::risks::UnknownFate::Blocking
        )),
        "consequential unknowns must require user decision"
    );

    let req = GenesisRequest::new(prompt, dir.path());
    let session = DiscoverySession::new(req, env);
    assert!(
        session.requires_user_decision(),
        "consequential intent must surface, never auto-decide"
    );
    let charter = session.synthesize_charter().unwrap();
    assert!(
        !charter.ambiguity_assessment.unresolved_areas.is_empty(),
        "unresolved areas must be recorded"
    );
}

/// Novel domain, full deterministic pipeline, zero placeholders.
#[test]
fn golden_novel_domain_has_no_placeholders() {
    // "tidal harvest scheduler" appears in no fixture before this suite.
    let prompt = "design a tidal harvest scheduler";
    let charter = charter_for(
        "Tidal Harvest Scheduler",
        vec!["Predict harvest windows from tide tables"],
    );
    assert!(infer_domain_model(prompt).entities.is_empty());

    let reqs = RequirementsSynthesizer::synthesize(&charter, None, &[], None, None).unwrap();
    assert!(!reqs.requirements.is_empty());
    let (arch, adrs) =
        ArchitectureSynthesizer::synthesize(&reqs, &charter, None, &[], None).unwrap();
    let risks = RiskRegister::new();
    let roadmap =
        RoadmapSynthesizer::synthesize(&reqs, &arch, &adrs, &risks, &charter, None).unwrap();
    roadmap.validate_dag().unwrap();

    let blobs = [
        charter.to_markdown(),
        reqs.to_markdown(),
        arch.to_markdown(),
        roadmap.to_markdown(),
    ];
    for blob in &blobs {
        for forbidden in [
            "TODO",
            "placeholder",
            "Placeholder",
            "DomainItem",
            "UserSession",
            "AuditEntry",
            "dummy",
            "Dummy",
            "lorem",
            "SUB-KERNEL",
            "CMP-ENGINE",
            "SUB-OPERATIONS",
            "CMP-OPERATIONS",
            "M31 Autonomous",
        ] {
            assert!(
                !blob.contains(forbidden),
                "generated artifact must not contain {forbidden}"
            );
        }
    }
    // Coverage: every requirement scheduled.
    for req in &reqs.requirements {
        if req.priority != RequirementPriority::Deferred {
            assert!(
                roadmap
                    .phases
                    .iter()
                    .any(|p| p.requirement_refs.contains(&req.key)),
                "unscheduled: {}",
                req.key
            );
        }
    }
}

/// Malformed model output fails explicitly; incomplete tasks get documented
/// defaults (never invented content).
#[tokio::test]
async fn golden_malformed_and_incomplete_model_output() {
    use m31a::agent::model_policy::{ModelCaller, ModelProposal};

    struct Garbage;
    #[async_trait::async_trait]
    impl ModelCaller for Garbage {
        async fn call_model(&self, _c: &str) -> Result<ModelProposal, String> {
            Ok(ModelProposal::Complete {
                summary: "not json at all {{{".to_string(),
                artifacts: vec![],
            })
        }
        async fn call_model_with_context(
            &self,
            _c: &m31a::kernel::seams::context::CompiledContext,
            _t: &tokio_util::sync::CancellationToken,
        ) -> Result<ModelProposal, String> {
            self.call_model("").await
        }
    }
    let dir = tempdir().unwrap();
    let svc = PlanServiceImpl::new(dir.path()).with_model_caller(Arc::new(Garbage));
    let err = svc
        .generate_initial_plan(PlanRequest::new(
            m31a::ids::MissionId::new(),
            "Build a cache layer",
        ))
        .await
        .expect_err("malformed plan must fail explicitly");
    assert!(matches!(
        err,
        m31a::kernel::seams::planner::PlanError::GenerationFailed(_)
    ));

    // Incomplete: minimal fields only -> valid plan with documented defaults.
    struct Sparse;
    #[async_trait::async_trait]
    impl ModelCaller for Sparse {
        async fn call_model(&self, _c: &str) -> Result<ModelProposal, String> {
            Ok(ModelProposal::Complete {
                summary: serde_json::json!({ "tasks": [{
                    "id": "TASK-01", "title": "Probe widget behavior",
                }] })
                .to_string(),
                artifacts: vec![],
            })
        }
        async fn call_model_with_context(
            &self,
            _c: &m31a::kernel::seams::context::CompiledContext,
            _t: &tokio_util::sync::CancellationToken,
        ) -> Result<ModelProposal, String> {
            self.call_model("").await
        }
    }
    let dir2 = tempdir().unwrap();
    let svc2 = PlanServiceImpl::new(dir2.path()).with_model_caller(Arc::new(Sparse));
    let resp = svc2
        .generate_initial_plan(PlanRequest::new(
            m31a::ids::MissionId::new(),
            "Probe widget behavior",
        ))
        .await
        .expect("sparse plan must succeed with defaults");
    assert_eq!(resp.task_count, 1);
    let task = &resp.candidate_plan.tasks[0];
    // Documented defaults: inferred role, empty criteria (never invented).
    assert_eq!(
        task.role,
        m31a::state_machine::agent::AgentRole::researcher()
    );
    assert!(task.completion_criteria.is_empty());
    assert!(task.requirement_keys.is_empty());
}

/// Handoff proposals never complete tasks (false-success prevention).
#[tokio::test]
async fn golden_handoff_never_completes_task() {
    use m31a::agent::model_policy::TestModelCaller;
    use m31a::agent::supervisor::ExecutionActivityTracker;
    use m31a::agent::{ActionDispatcher, ActionRequest, ActionResult, AgentProfile, WorkerRunner};
    use m31a::state_machine::agent::AgentRole;

    struct NoopDispatcher;
    #[async_trait::async_trait]
    impl ActionDispatcher for NoopDispatcher {
        async fn dispatch(&self, action: &ActionRequest) -> Result<ActionResult, String> {
            Ok(ActionResult {
                action_id: action.id.clone(),
                success: true,
                output: "noop".to_string(),
                error: None,
            })
        }
    }

    let model = TestModelCaller::with_proposal(ModelProposal::Handoff {
        target_role: "reviewer".to_string(),
        reason: "needs review".to_string(),
    });
    let profile = AgentProfile::built_in(AgentRole::implementer());
    let mut runner = WorkerRunner::new_isolated_test(
        m31a::ids::MissionId::new(),
        m31a::ids::AgentId::new(),
        m31a::ids::TaskId::new(),
        profile,
    );
    let token = CancellationToken::new();
    let tracker = Arc::new(ExecutionActivityTracker::default());
    let outcome = runner
        .run_step_loop(&model, &NoopDispatcher, &token, tracker)
        .await;
    assert!(
        !matches!(
            outcome,
            m31a::agent::supervisor::AgentOutcome::Succeeded { .. }
        ),
        "handoff must never report success, got {:?}",
        outcome
    );
}

/// Compiled worker context carries task evidence, not just the raw sentence.
#[tokio::test]
async fn golden_context_carries_task_evidence() {
    use m31a::context::compiler::ProductionContextCompiler;
    use m31a::kernel::seams::context::{ContextCompilationRequest, ContextCompiler};

    let compiler = ProductionContextCompiler::new();
    let req =
        ContextCompilationRequest::new(m31a::ids::MissionId::new(), m31a::ids::TaskId::new(), 8192)
            .with_task_objective("Repair widget turnaround")
            .with_role(m31a::state_machine::agent::AgentRole::implementer())
            .with_task_description("Fix turnaround() to return the measured value of 5")
            .with_task_criteria(vec!["test_alpha_ok passes".to_string()])
            .with_task_requirement_keys(vec!["REQ-FUNC-01".to_string()])
            .with_task_assumptions(vec!["Widget API is stable".to_string()])
            .with_upstream_requirements(vec!["REQ-FUNC-01: repair turnaround".to_string()])
            .with_upstream_decisions(vec!["ADR-0001: fix in place, no rewrite".to_string()])
            .with_upstream_research_summary("Evidence: turnaround measured at 5.".to_string());
    let compiled = compiler.compile_context(req).await.expect("compiles");
    for expected in [
        "measured value of 5",
        "test_alpha_ok passes",
        "REQ-FUNC-01",
        "Widget API is stable",
        "no rewrite",
        "measured at 5",
    ] {
        assert!(
            compiled.system_prompt.contains(expected),
            "context must carry evidence: {expected}"
        );
    }
    // The 7-layer contract sections join the sent prompt (no longer discarded).
    assert!(
        compiled.system_prompt.contains("Acceptance Criteria")
            || compiled.system_prompt.contains("acceptance criteria"),
        "criteria contract must be present"
    );
}

/// Model-backed diagnosis flows into recovery decisions; garbage degrades
/// to heuristic without failing recovery.
#[tokio::test]
async fn golden_model_diagnosis_flows_into_recovery() {
    use m31a::kernel::seams::recovery::{
        FailureClassification, RecoveryAction, RecoveryEngine, RecoveryStrategyRequest,
    };
    use m31a::recovery::adapter::ProductionRecoveryEngine;

    struct ScriptedDiagnosis {
        body: String,
    }
    #[async_trait::async_trait]
    impl ModelCaller for ScriptedDiagnosis {
        async fn call_model(&self, _c: &str) -> Result<ModelProposal, String> {
            Ok(ModelProposal::Complete {
                summary: self.body.clone(),
                artifacts: vec![],
            })
        }
        async fn call_model_with_context(
            &self,
            _c: &m31a::kernel::seams::context::CompiledContext,
            _t: &tokio_util::sync::CancellationToken,
        ) -> Result<ModelProposal, String> {
            self.call_model("").await
        }
    }

    // Model says escalate -> recovery escalates (model reasoning honored).
    let engine = ProductionRecoveryEngine::new(None).with_diagnostician(Arc::new(
        m31a::verification::diagnostician::ModelDiagnostician::new().with_model_caller(Arc::new(
            ScriptedDiagnosis {
                body: serde_json::json!({
                    "failure_class": "timeout",
                    "root_cause": "widget harness deadlock",
                    "recommendation": "escalate",
                })
                .to_string(),
            },
        )),
    ));
    let action = engine
        .determine_recovery(
            RecoveryStrategyRequest::new(
                m31a::ids::MissionId::new(),
                m31a::ids::TaskId::new(),
                FailureClassification::Timeout,
                0,
            )
            .with_error_message("deadline exceeded after 60s"),
        )
        .await
        .expect("recovery decides");
    assert!(
        matches!(action, RecoveryAction::Escalate { .. }),
        "model escalate recommendation must flow through, got {action:?}"
    );

    // Model garbage -> heuristic fallback, recovery still decides (no halt).
    let engine2 = ProductionRecoveryEngine::new(None).with_diagnostician(Arc::new(
        m31a::verification::diagnostician::ModelDiagnostician::new().with_model_caller(Arc::new(
            ScriptedDiagnosis {
                body: "not json {{{".to_string(),
            },
        )),
    ));
    let action2 = engine2
        .determine_recovery(
            RecoveryStrategyRequest::new(
                m31a::ids::MissionId::new(),
                m31a::ids::TaskId::new(),
                FailureClassification::Compilation,
                0,
            )
            .with_error_message("error[E0432]: unresolved import"),
        )
        .await
        .expect("recovery must not fail when diagnosis degrades");
    assert!(
        matches!(action2, RecoveryAction::Retry { .. }),
        "heuristic fallback must retry honestly, got {action2:?}"
    );
}

/// Post-completion memory explains WHY (decisions + failure diagnoses).
#[tokio::test]
async fn golden_memory_explains_why() {
    use m31a::memory::repository::SqliteEngineeringMemoryRepository;

    let dir = tempdir().unwrap();
    let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
    m31a::persistence::sqlite::run_migrations(&pool)
        .await
        .expect("migrations");
    let _ = dir;
    use m31a::memory::repository::EngineeringMemoryStore;
    let store = SqliteEngineeringMemoryRepository::new(pool);

    // Genesis-style decision with rationale + alternatives.
    let mut decision = m31a::kernel::memory::EngineeringDecision::new(
        "dec-test-sqlite-wal",
        m31a::kernel::memory::MemoryScope::Project,
        "Adopt SQLite WAL for expense persistence",
        "expense tracker needs durable single-file storage",
        "SQLite WAL selected",
        "Simplicity and transactional guarantees for target workload",
        "phase-29-golden",
    );
    decision.status = m31a::kernel::memory::DecisionStatus::Accepted;
    store.save_decision(&decision).await.expect("save");
    let listed = store.list_decisions(None, None, None).await.expect("list");
    let found = listed
        .iter()
        .find(|d| d.title.contains("SQLite WAL"))
        .expect("decision retrievable");
    assert!(found.rationale.contains("transactional guarantees"));
    assert_eq!(found.status, m31a::kernel::memory::DecisionStatus::Accepted);
}
