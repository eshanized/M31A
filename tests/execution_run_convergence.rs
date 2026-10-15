//! ExecutionRun convergence regression suite (Phases B–F).
//!
//! Deterministic, offline coverage for the converged architecture:
//! 1. New Task/TaskGraph/WorkflowStep records carry the canonical `run_id`.
//! 2. `PlanningChoice` single contract: explicit/headless/stored/interactive.
//! 3. Projection-dir enforcement: only `.m31a/project/` is honored.
//! 4. Migration rejects legacy-source and destination root symlinks.
//! 5. Project-context reads reject a symlinked project root.
//! 6. Invalid authorization creates no ExecutionRun and starts no work.
//!
//! No network access, no model calls.

use m31a::ids::ExecutionRunId;
use m31a::runtime::PlanningChoice;
use m31a::storage::StorageLayout;

// ── Planning-choice contract ──────────────────────────────────────────────

#[test]
fn test_planning_choice_explicit_request_counts_as_consent() {
    assert!(PlanningChoice::is_explicit_planning_request(
        "Please init project and generate a roadmap"
    ));
    assert!(PlanningChoice::is_explicit_planning_request(
        "/plan create a plan for this repo"
    ));
    assert!(PlanningChoice::is_explicit_planning_request(
        "Generate planning documents for the workspace"
    ));
    assert!(PlanningChoice::is_explicit_planning_request(
        "generate roadmap"
    ));
    assert!(!PlanningChoice::is_explicit_planning_request(
        "Fix the null pointer in auth.rs"
    ));
    assert!(!PlanningChoice::is_explicit_planning_request(
        "Add a retry loop to the worker"
    ));
}

#[test]
fn test_planning_choice_false_consent_prevention() {
    // Negated phrases containing "planning documents" must NOT count as consent
    assert!(!PlanningChoice::is_explicit_planning_request(
        "Do not generate planning documents. Work directly on this task."
    ));
    assert!(!PlanningChoice::is_explicit_planning_request(
        "don't generate planning documents"
    ));
    assert!(!PlanningChoice::is_explicit_planning_request(
        "no planning documents please"
    ));
    assert!(!PlanningChoice::is_explicit_planning_request(
        "Work directly without generating planning documents"
    ));

    // Governance commands on candidate plans must NOT count as consent for project Genesis
    assert!(!PlanningChoice::is_explicit_planning_request(
        "/plan accept"
    ));
    assert!(!PlanningChoice::is_explicit_planning_request(
        "/plan reject wrong scope"
    ));
    assert!(!PlanningChoice::is_explicit_planning_request(
        "/plan revise add tests"
    ));
    assert!(!PlanningChoice::is_explicit_planning_request(
        "/plan edit {}"
    ));
    assert!(!PlanningChoice::is_explicit_planning_request("/plan regen"));

    // Explicit refusal detection
    assert!(PlanningChoice::is_explicit_refusal(
        "Do not generate planning documents. Work directly on this task."
    ));
    assert!(PlanningChoice::is_explicit_refusal("/no-plan"));
    assert!(PlanningChoice::is_explicit_refusal("/no-plan execute task"));
    assert!(PlanningChoice::is_explicit_refusal(
        "work directly on this task"
    ));
}

#[test]
fn test_planning_choice_resolve_for_request() {
    // 1. Explicit flag wins over everything.
    assert_eq!(
        PlanningChoice::resolve_for_request(
            "fix bug",
            Some(PlanningChoice::ProjectPlanning),
            Some(PlanningChoice::DirectExecution),
            true,
        ),
        Some(PlanningChoice::ProjectPlanning)
    );
    assert_eq!(
        PlanningChoice::resolve_for_request(
            "init project",
            Some(PlanningChoice::DirectExecution),
            Some(PlanningChoice::ProjectPlanning),
            false,
        ),
        Some(PlanningChoice::DirectExecution)
    );

    // 2. Explicit refusal in text overrides stored preference.
    assert_eq!(
        PlanningChoice::resolve_for_request(
            "Do not generate planning documents. Work directly on this task.",
            None,
            Some(PlanningChoice::ProjectPlanning),
            false,
        ),
        Some(PlanningChoice::DirectExecution)
    );
    assert_eq!(
        PlanningChoice::resolve_for_request(
            "/no-plan fix bug",
            None,
            Some(PlanningChoice::ProjectPlanning),
            false,
        ),
        Some(PlanningChoice::DirectExecution)
    );

    // 3. Governance commands do not trigger project planning.
    assert_eq!(
        PlanningChoice::resolve_for_request("/plan accept", None, None, true),
        Some(PlanningChoice::DirectExecution)
    );
    assert_eq!(
        PlanningChoice::resolve_for_request("/plan accept", None, None, false),
        None
    );

    // 4. Explicit planning text counts as consent even headless.
    assert_eq!(
        PlanningChoice::resolve_for_request("init project please", None, None, true),
        Some(PlanningChoice::ProjectPlanning)
    );

    // 5. Stored choice for the pending request is reused when not refused.
    assert_eq!(
        PlanningChoice::resolve_for_request(
            "fix bug",
            None,
            Some(PlanningChoice::ProjectPlanning),
            false,
        ),
        Some(PlanningChoice::ProjectPlanning)
    );

    // 6. Headless without intent is deterministic direct execution.
    assert_eq!(
        PlanningChoice::resolve_for_request("fix bug", None, None, true),
        Some(PlanningChoice::DirectExecution)
    );

    // 7. Interactive without stored choice must be asked.
    assert_eq!(
        PlanningChoice::resolve_for_request("fix bug", None, None, false),
        None
    );
    assert_eq!(
        PlanningChoice::resolve_for_request("I want to build a compiler", None, None, false),
        None
    );
}

// ── Projection-dir enforcement ────────────────────────────────────────────

#[test]
fn test_projection_dir_canonicalization() {
    use m31a::workflow::genesis::GenesisOptions;
    assert_eq!(
        GenesisOptions::canonicalize_projection_dir(".m31a/project"),
        ".m31a/project"
    );
    assert_eq!(
        GenesisOptions::canonicalize_projection_dir(".planning"),
        ".m31a/project"
    );
    assert_eq!(
        GenesisOptions::canonicalize_projection_dir(""),
        ".m31a/project"
    );
    // Absolute escapes, traversal, and alternate destinations all fall back
    // to the canonical root (a config field must never override the boundary).
    assert_eq!(
        GenesisOptions::canonicalize_projection_dir("/tmp/evil"),
        ".m31a/project"
    );
    assert_eq!(
        GenesisOptions::canonicalize_projection_dir("../escape"),
        ".m31a/project"
    );
    assert_eq!(
        GenesisOptions::canonicalize_projection_dir("docs/plans"),
        ".m31a/project"
    );
    assert_eq!(
        GenesisOptions::canonicalize_projection_dir(".m31a/other"),
        ".m31a/project"
    );
}

#[test]
fn test_projection_write_validation() {
    let temp = tempfile::tempdir().unwrap();
    let ws = temp.path();
    std::fs::create_dir_all(ws.join(".m31a").join("project")).unwrap();

    // Canonical write validates.
    let ok = StorageLayout::validate_projection_write(ws, ".m31a/project", "PROJECT.md");
    assert!(ok.is_ok(), "canonical write must validate: {ok:?}");

    // Alternate destinations rejected.
    assert!(StorageLayout::validate_projection_write(ws, "/tmp/evil", "PROJECT.md").is_err());
    assert!(StorageLayout::validate_projection_write(ws, "../escape", "PROJECT.md").is_err());
    assert!(StorageLayout::validate_projection_write(ws, "docs/plans", "PROJECT.md").is_err());

    // Traversal filenames rejected.
    assert!(StorageLayout::validate_projection_write(ws, ".m31a/project", "../evil.md").is_err());
    assert!(StorageLayout::validate_projection_write(ws, ".m31a/project", "/abs.md").is_err());

    // Nested research path inside the canonical root validates.
    let nested =
        StorageLayout::validate_projection_write(ws, ".m31a/project", "research/SUMMARY.md");
    assert!(nested.is_ok());
}

#[cfg(unix)]
#[test]
fn test_projection_root_symlink_rejected_for_writes() {
    let temp = tempfile::tempdir().unwrap();
    let ws = temp.path();
    let outside = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(ws.join(".m31a")).unwrap();
    std::os::unix::fs::symlink(outside.path(), ws.join(".m31a").join("project")).unwrap();
    let res = StorageLayout::validate_projection_write(ws, ".m31a/project", "PROJECT.md");
    assert!(
        res.is_err(),
        "symlinked project root must be rejected for writes"
    );
}

// ── Migration root-symlink rejection ──────────────────────────────────────

#[cfg(unix)]
#[test]
fn test_legacy_source_root_symlink_rejected() {
    let temp = tempfile::tempdir().unwrap();
    let ws = temp.path();
    let outside = tempfile::tempdir().unwrap();
    std::fs::write(outside.path().join("SECRET.md"), "external secret\n").unwrap();
    std::os::unix::fs::symlink(outside.path(), ws.join(".planning")).unwrap();
    let report = m31a::storage::migration::migrate_legacy_planning_documents(ws);
    assert!(
        report.errors.iter().any(|e| e.contains("symlink")),
        "legacy source symlink must be rejected, got: {:?}",
        report.errors
    );
    assert!(
        report.files_migrated.is_empty(),
        "no files may migrate from a symlinked source"
    );
    // External content must not be copied into the project namespace.
    assert!(!ws.join(".m31a").join("project").join("SECRET.md").exists());
}

#[cfg(unix)]
#[test]
fn test_destination_project_root_symlink_rejected() {
    let temp = tempfile::tempdir().unwrap();
    let ws = temp.path();
    let outside = tempfile::tempdir().unwrap();
    // Legacy source with one real file.
    std::fs::create_dir_all(ws.join(".planning")).unwrap();
    std::fs::write(ws.join(".planning").join("PROJECT.md"), "# Legacy\n").unwrap();
    // Destination root is a symlink to an external directory.
    std::fs::create_dir_all(ws.join(".m31a")).unwrap();
    std::os::unix::fs::symlink(outside.path(), ws.join(".m31a").join("project")).unwrap();
    let report = m31a::storage::migration::migrate_legacy_planning_documents(ws);
    assert!(
        report.errors.iter().any(|e| e.contains("symlink")),
        "destination root symlink must be rejected, got: {:?}",
        report.errors
    );
    assert!(
        report.files_migrated.is_empty(),
        "no files may migrate into a symlinked destination"
    );
}

#[test]
fn test_migration_never_overwrites_modified_conflict_copy() {
    let temp = tempfile::tempdir().unwrap();
    let ws = temp.path();
    std::fs::create_dir_all(ws.join(".planning")).unwrap();
    std::fs::write(ws.join(".planning").join("A.md"), "source v1\n").unwrap();

    // First migration copies the file.
    let r1 = m31a::storage::migration::migrate_legacy_planning_documents(ws);
    assert_eq!(r1.files_migrated.len(), 1);

    // Operator modifies the destination.
    std::fs::write(
        ws.join(".m31a").join("project").join("A.md"),
        "operator edit\n",
    )
    .unwrap();

    // Second migration must preserve both via a conflict file.
    let r2 = m31a::storage::migration::migrate_legacy_planning_documents(ws);
    assert_eq!(r2.conflicts_detected.len(), 1);
    let conflict = &r2.conflicts_detected[0];
    assert_eq!(std::fs::read_to_string(conflict).unwrap(), "source v1\n");

    // Operator edits the conflict copy; a third migration must NOT overwrite it.
    std::fs::write(conflict, "operator conflict edit\n").unwrap();
    let r3 = m31a::storage::migration::migrate_legacy_planning_documents(ws);
    assert!(
        r3.files_migrated.is_empty(),
        "repeated migration must not overwrite anything"
    );
    assert_eq!(
        std::fs::read_to_string(conflict).unwrap(),
        "operator conflict edit\n",
        "modified conflict copy must be preserved"
    );
    assert_eq!(
        std::fs::read_to_string(ws.join(".m31a").join("project").join("A.md")).unwrap(),
        "operator edit\n",
        "operator destination edit must be preserved"
    );
}

// ── Project-context root containment ───────────────────────────────────────

#[cfg(unix)]
#[test]
fn test_project_context_rejects_symlinked_root() {
    let temp = tempfile::tempdir().unwrap();
    let ws = temp.path();
    let outside = tempfile::tempdir().unwrap();
    std::fs::write(outside.path().join("PROJECT.md"), "# EXTERNAL\n").unwrap();
    std::fs::create_dir_all(ws.join(".m31a")).unwrap();
    std::os::unix::fs::symlink(outside.path(), ws.join(".m31a").join("project")).unwrap();
    let ctx = m31a::prompt::context::DurableStateContext::load_from_workspace(ws);
    assert!(
        ctx.project_charter.is_none(),
        "symlinked project root must not enter model context"
    );
}

// ── run_id domain ownership ───────────────────────────────────────────────

#[test]
fn test_task_and_graph_carry_canonical_run_id() {
    let run = ExecutionRunId::new();
    let task = m31a::state::Task::new(m31a::ids::TaskId::new(), run, "Do work".to_string());
    assert_eq!(task.canonical_run_id(), run);
    assert_eq!(task.run_id, Some(run));

    let graph = m31a::dag::graph::TaskGraph::build_from_records(
        m31a::ids::TaskGraphId::new(),
        run,
        1,
        "plan-1".to_string(),
        "active".to_string(),
        [(task.id, task)].into_iter().collect(),
        Default::default(),
        chrono::Utc::now(),
        chrono::Utc::now(),
    );
    assert_eq!(graph.canonical_run_id(), run);
}

#[tokio::test]
async fn test_new_taskgraph_records_persist_run_id() {
    use m31a::dag::materializer::TaskGraphMaterializer;
    use m31a::kernel::plan::{CandidatePlan, CandidateTask};
    use m31a::persistence::sqlite::repositories::TaskGraphRepository;
    use m31a::persistence::sqlite::repositories::task_graph::SqliteTaskGraphRepository;
    use m31a::persistence::sqlite::schema::initialize_database;

    let temp = tempfile::tempdir().unwrap();
    let pool = initialize_database(&temp.path().join("runid.db"))
        .await
        .unwrap();
    let run_id = ExecutionRunId::new();
    // FK target: execution_runs row must exist for run_id references.
    {
        let repo = m31a::persistence::sqlite::repositories::SqliteExecutionRunRepository::new(
            pool.clone(),
        );
        let run = m31a::state::ExecutionRun::new(run_id, "Test objective".to_string());
        repo.insert(&run).await.expect("seed execution run");
    }

    let task = CandidateTask::new(
        "t1",
        "First task",
        m31a::state_machine::agent::AgentRole::implementer(),
        m31a::kernel::plan::VerificationStrategy::Compilation,
        m31a::kernel::plan::ResourceEstimate::default(),
    );
    let plan = CandidatePlan::new("plan-1", "Test objective", vec![task]);
    let materializer = TaskGraphMaterializer::new(pool.clone());
    let graph = materializer
        .materialize_in_lane(
            run_id,
            &plan,
            m31a::dag::materializer::MaterializationLane::InternalTest,
        )
        .await
        .expect("materialize");

    // New rows must have a valid canonical run_id (not NULL).
    let run_id_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM task_graphs WHERE run_id IS NOT NULL")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(run_id_count, 1, "new task_graphs row must persist run_id");
    let task_run_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM tasks WHERE run_id IS NOT NULL")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(task_run_count, 1, "new tasks row must persist run_id");

    // Reads resolve through either identity.
    let repo = SqliteTaskGraphRepository::new(pool.clone());
    let loaded = repo
        .get_active_graph(run_id)
        .await
        .unwrap()
        .expect("active graph");
    assert_eq!(loaded.canonical_run_id(), run_id);
    assert_eq!(loaded.id, graph.id);
}

#[tokio::test]
async fn test_new_workflow_step_records_persist_run_id() {
    use m31a::persistence::sqlite::schema::initialize_database;
    use m31a::workflow::repository::{SqliteWorkflowRepository, WorkflowRepository};

    let temp = tempfile::tempdir().unwrap();
    let pool = initialize_database(&temp.path().join("wfrun.db"))
        .await
        .unwrap();
    let repo = SqliteWorkflowRepository::new(pool.clone());

    // Minimal workflow run + step with canonical run linkage.
    let run_id = ExecutionRunId::new();
    {
        let repo = m31a::persistence::sqlite::repositories::SqliteExecutionRunRepository::new(
            pool.clone(),
        );
        let run = m31a::state::ExecutionRun::new(run_id, "Workflow test".to_string());
        repo.insert(&run).await.expect("seed execution run");
    }
    let run = m31a::workflow::state::WorkflowRun::new(
        "test-flow",
        1,
        temp.path().to_path_buf(),
        m31a::workflow::state::WorkflowMode::Standard,
    );
    let workflow_id = run.id;
    repo.create_run(&run).await.expect("create run");

    let mut step = m31a::workflow::state::WorkflowStepRun::new(workflow_id, "step-a");
    step.mission_id = Some(run_id);
    step.execution_run_id = Some(run_id);
    repo.create_step_run(&step).await.expect("create step");

    let count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM workflow_step_runs WHERE run_id IS NOT NULL")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(count, 1, "new step row must persist run_id");

    let loaded = repo
        .get_step_run(step.id)
        .await
        .unwrap()
        .expect("step exists");
    assert_eq!(loaded.execution_run_id, Some(run_id));
}

// ── Authorization ordering (fail-closed before any state) ─────────────────

/// Invalid authorizations must be rejected BEFORE any ExecutionRun row,
/// session association, or TaskGraph materialization exists. This proves the
/// authorization-first ordering: no work starts and no misleading `executing`
/// status is left behind.
async fn setup_auth_test_runtime() -> (tempfile::TempDir, std::sync::Arc<m31a::runtime::AppRuntime>)
{
    let dir = tempfile::tempdir().unwrap();
    let runtime = m31a::runtime::AppRuntime::new(dir.path())
        .await
        .expect("AppRuntime::new");
    (dir, std::sync::Arc::new(runtime))
}

fn make_test_plan() -> m31a::kernel::plan::CandidatePlan {
    let task = m31a::kernel::plan::CandidateTask::new(
        "t1",
        "Do authorized work",
        m31a::state_machine::agent::AgentRole::implementer(),
        m31a::kernel::plan::VerificationStrategy::Compilation,
        m31a::kernel::plan::ResourceEstimate::default(),
    );
    // Revision defaults to 1.
    m31a::kernel::plan::CandidatePlan::new("plan-auth-1", "Authorized objective", vec![task])
}

async fn make_valid_test_auth(
    runtime: &m31a::runtime::AppRuntime,
    session_id: &m31a::ids::SessionId,
    plan: &m31a::kernel::plan::CandidatePlan,
    plan_rev: u32,
    task_rev: u32,
) -> m31a::planning::review::ExecutionAuthorization {
    let plan_hash = m31a::planning::review::PlanRevision::compute_content_hash(plan);
    let task_hash = m31a::planning::review::TaskRevision::compute_tasks_hash(&plan.tasks);
    let policy_hash = runtime.policy().active_policy_hash().to_string();
    let ws = runtime.workspace_root().display().to_string();
    let auth = m31a::planning::review::ExecutionAuthorization::new(
        session_id.to_string(),
        plan_rev,
        task_rev,
        "operator",
    )
    .with_content_hashes(plan_hash, task_hash)
    .with_execution_binding(
        policy_hash,
        ws,
        "implementer",
        "Safe",
        std::time::Duration::from_secs(3600),
    );

    let coord = runtime.create_pre_execution_coordinator();
    let repo = coord.lifecycle_repo();
    let sid_str = session_id.to_string();
    repo.save_execution_authorization(&auth)
        .await
        .expect("save auth");
    let state = m31a::persistence::sqlite::repositories::PersistedLifecycleState {
        session_id: sid_str.clone(),
        stage: m31a::state_machine::lifecycle::LifecycleStage::ExecutionAuthorized,
        plan_revision: plan_rev,
        task_revision: task_rev,
        authorization_id: Some(auth.id),
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };
    repo.save_lifecycle_state(&state)
        .await
        .expect("save lifecycle state");
    auth
}

async fn execution_run_count(pool: &sqlx::SqlitePool) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM execution_runs")
        .fetch_one(pool)
        .await
        .unwrap()
}

#[tokio::test]
async fn test_stale_plan_revision_creates_no_run() {
    let (_dir, runtime) = setup_auth_test_runtime().await;
    let session = runtime.create_session().await.expect("create session");
    let plan = make_test_plan();
    // Auth binds revision 2 while candidate plan has revision 1.
    let auth = make_valid_test_auth(&runtime, &session.id, &plan, 2, 1).await;

    let err = runtime
        .start_authorized_execution(session.id, &plan, auth)
        .await
        .expect_err("stale plan revision must be rejected");
    assert!(
        err.to_string().contains("revision"),
        "unexpected error: {err}"
    );
    assert_eq!(
        execution_run_count(runtime.pool()).await,
        0,
        "rejected authorization must leave zero ExecutionRun rows"
    );
}

#[tokio::test]
async fn test_wrong_session_binding_creates_no_run() {
    let (_dir, runtime) = setup_auth_test_runtime().await;
    let session = runtime.create_session().await.expect("create session");
    let other_session = runtime.create_session().await.expect("create session");
    let plan = make_test_plan();
    // Auth governs a different session.
    let auth = make_valid_test_auth(&runtime, &other_session.id, &plan, 1, 1).await;

    let err = runtime
        .start_authorized_execution(session.id, &plan, auth)
        .await
        .expect_err("cross-session authorization must be rejected");
    assert!(
        err.to_string().to_lowercase().contains("session"),
        "unexpected error: {err}"
    );
    assert_eq!(
        execution_run_count(runtime.pool()).await,
        0,
        "rejected authorization must leave zero ExecutionRun rows"
    );
}

#[tokio::test]
async fn test_denied_authorization_creates_no_run() {
    let (_dir, runtime) = setup_auth_test_runtime().await;
    let session = runtime.create_session().await.expect("create session");
    let plan = make_test_plan();
    let mut auth = make_valid_test_auth(&runtime, &session.id, &plan, 1, 1).await;
    auth.decision = m31a::planning::review::AuthorizationDecision::Rejected;

    let err = runtime
        .start_authorized_execution(session.id, &plan, auth)
        .await
        .expect_err("denied authorization must be rejected");
    assert!(
        err.to_string().to_lowercase().contains("invalid")
            || err.to_string().to_lowercase().contains("denied"),
        "unexpected error: {err}"
    );
    assert_eq!(
        execution_run_count(runtime.pool()).await,
        0,
        "rejected authorization must leave zero ExecutionRun rows"
    );
    let graph_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM task_graphs")
        .fetch_one(runtime.pool())
        .await
        .unwrap();
    assert_eq!(
        graph_count, 0,
        "rejected authorization must materialize zero TaskGraphs"
    );
}

#[tokio::test]
async fn test_exact_plan_hash_mismatch_creates_no_run() {
    let (_dir, runtime) = setup_auth_test_runtime().await;
    let session = runtime.create_session().await.expect("create session");
    let plan = make_test_plan();
    let mut auth = make_valid_test_auth(&runtime, &session.id, &plan, 1, 1).await;
    auth.plan_content_hash = Some("tampered-or-mismatched-plan-hash".to_string());

    let err = runtime
        .start_authorized_execution(session.id, &plan, auth)
        .await
        .expect_err("exact plan content hash mismatch must be rejected");
    assert!(
        err.to_string().to_lowercase().contains("hash")
            || err.to_string().to_lowercase().contains("integrity"),
        "unexpected error: {err}"
    );
    assert_eq!(
        execution_run_count(runtime.pool()).await,
        0,
        "rejected authorization must leave zero ExecutionRun rows"
    );
    let graph_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM task_graphs")
        .fetch_one(runtime.pool())
        .await
        .unwrap();
    assert_eq!(
        graph_count, 0,
        "rejected authorization must materialize zero TaskGraphs"
    );
}

#[tokio::test]
async fn test_exact_tasks_hash_mismatch_creates_no_run() {
    let (_dir, runtime) = setup_auth_test_runtime().await;
    let session = runtime.create_session().await.expect("create session");
    let plan = make_test_plan();
    let mut auth = make_valid_test_auth(&runtime, &session.id, &plan, 1, 1).await;
    auth.task_content_hash = Some("tampered-or-mismatched-tasks-hash".to_string());

    let err = runtime
        .start_authorized_execution(session.id, &plan, auth)
        .await
        .expect_err("exact tasks content hash mismatch must be rejected");
    assert!(
        err.to_string().to_lowercase().contains("hash")
            || err.to_string().to_lowercase().contains("integrity"),
        "unexpected error: {err}"
    );
    assert_eq!(
        execution_run_count(runtime.pool()).await,
        0,
        "rejected authorization must leave zero ExecutionRun rows"
    );
}

#[tokio::test]
async fn test_f15_missing_mandatory_plan_hash_creates_no_run() {
    let (_dir, runtime) = setup_auth_test_runtime().await;
    let session = runtime.create_session().await.expect("create session");
    let plan = make_test_plan();
    let mut auth = make_valid_test_auth(&runtime, &session.id, &plan, 1, 1).await;
    auth.plan_content_hash = None;

    let err = runtime
        .start_authorized_execution(session.id, &plan, auth)
        .await
        .expect_err("missing plan_content_hash must be rejected");
    assert!(err.to_string().contains("plan_content_hash"));
    assert_eq!(execution_run_count(runtime.pool()).await, 0);
}

#[tokio::test]
async fn test_f15_missing_mandatory_task_hash_creates_no_run() {
    let (_dir, runtime) = setup_auth_test_runtime().await;
    let session = runtime.create_session().await.expect("create session");
    let plan = make_test_plan();
    let mut auth = make_valid_test_auth(&runtime, &session.id, &plan, 1, 1).await;
    auth.task_content_hash = None;

    let err = runtime
        .start_authorized_execution(session.id, &plan, auth)
        .await
        .expect_err("missing task_content_hash must be rejected");
    assert!(err.to_string().contains("task_content_hash"));
    assert_eq!(execution_run_count(runtime.pool()).await, 0);
}

#[tokio::test]
async fn test_f15_missing_mandatory_workspace_root_creates_no_run() {
    let (_dir, runtime) = setup_auth_test_runtime().await;
    let session = runtime.create_session().await.expect("create session");
    let plan = make_test_plan();
    let mut auth = make_valid_test_auth(&runtime, &session.id, &plan, 1, 1).await;
    auth.workspace_root = None;

    let err = runtime
        .start_authorized_execution(session.id, &plan, auth)
        .await
        .expect_err("missing workspace_root must be rejected");
    assert!(err.to_string().contains("workspace_root"));
    assert_eq!(execution_run_count(runtime.pool()).await, 0);
}

#[tokio::test]
async fn test_f15_missing_mandatory_policy_hash_creates_no_run() {
    let (_dir, runtime) = setup_auth_test_runtime().await;
    let session = runtime.create_session().await.expect("create session");
    let plan = make_test_plan();
    let mut auth = make_valid_test_auth(&runtime, &session.id, &plan, 1, 1).await;
    auth.policy_hash = None;

    let err = runtime
        .start_authorized_execution(session.id, &plan, auth)
        .await
        .expect_err("missing policy_hash must be rejected");
    assert!(err.to_string().contains("policy_hash"));
    assert_eq!(execution_run_count(runtime.pool()).await, 0);
}

#[tokio::test]
async fn test_f15_missing_mandatory_autonomy_mode_creates_no_run() {
    let (_dir, runtime) = setup_auth_test_runtime().await;
    let session = runtime.create_session().await.expect("create session");
    let plan = make_test_plan();
    let mut auth = make_valid_test_auth(&runtime, &session.id, &plan, 1, 1).await;
    auth.autonomy_mode = None;

    let err = runtime
        .start_authorized_execution(session.id, &plan, auth)
        .await
        .expect_err("missing autonomy_mode must be rejected");
    assert!(err.to_string().contains("autonomy_mode"));
    assert_eq!(execution_run_count(runtime.pool()).await, 0);
}

#[tokio::test]
async fn test_f15_revoked_durable_grant_creates_no_run() {
    let (_dir, runtime) = setup_auth_test_runtime().await;
    let session = runtime.create_session().await.expect("create session");
    let plan = make_test_plan();
    let auth = make_valid_test_auth(&runtime, &session.id, &plan, 1, 1).await;

    // Revoke/invalidate durable grant in lifecycle_repo
    let coord = runtime.create_pre_execution_coordinator();
    coord
        .lifecycle_repo()
        .invalidate_authorizations(&session.id.to_string(), "Operator revoked approval")
        .await
        .expect("invalidate");

    let err = runtime
        .start_authorized_execution(session.id, &plan, auth)
        .await
        .expect_err("revoked grant must be rejected");
    assert!(err.to_string().contains("no longer authorized"));
    assert_eq!(execution_run_count(runtime.pool()).await, 0);
    let graph_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM task_graphs")
        .fetch_one(runtime.pool())
        .await
        .unwrap();
    assert_eq!(graph_count, 0);
}

#[tokio::test]
async fn test_f15_superseded_authorization_id_creates_no_run() {
    let (_dir, runtime) = setup_auth_test_runtime().await;
    let session = runtime.create_session().await.expect("create session");
    let plan = make_test_plan();
    let mut auth = make_valid_test_auth(&runtime, &session.id, &plan, 1, 1).await;

    // Tamper with presented authorization ID so it doesn't match durable grant pointer
    auth.id = uuid::Uuid::now_v7();

    let err = runtime
        .start_authorized_execution(session.id, &plan, auth)
        .await
        .expect_err("superseded or forged authorization ID must be rejected");
    assert!(err.to_string().contains("Authorization ID mismatch"));
    assert_eq!(execution_run_count(runtime.pool()).await, 0);
    let graph_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM task_graphs")
        .fetch_one(runtime.pool())
        .await
        .unwrap();
    assert_eq!(graph_count, 0);
}

#[tokio::test]
async fn test_f15_unauthorized_lifecycle_stage_creates_no_run() {
    let (_dir, runtime) = setup_auth_test_runtime().await;
    let session = runtime.create_session().await.expect("create session");
    let plan = make_test_plan();
    let auth = make_valid_test_auth(&runtime, &session.id, &plan, 1, 1).await;

    // Reset lifecycle stage to IntentActive
    let coord = runtime.create_pre_execution_coordinator();
    let mut state = coord
        .lifecycle_repo()
        .load_lifecycle_state(&session.id.to_string())
        .await
        .unwrap()
        .unwrap();
    state.stage = m31a::state_machine::lifecycle::LifecycleStage::IntentActive;
    coord
        .lifecycle_repo()
        .save_lifecycle_state(&state)
        .await
        .unwrap();

    let err = runtime
        .start_authorized_execution(session.id, &plan, auth)
        .await
        .expect_err("non-ExecutionAuthorized stage must be rejected");
    assert!(err.to_string().contains("ExecutionAuthorized"));
    assert_eq!(execution_run_count(runtime.pool()).await, 0);
}

#[tokio::test]
async fn test_workflow_run_persists_canonical_run_id() {
    use m31a::workflow::repository::{SqliteWorkflowRepository, WorkflowRepository};
    use m31a::workflow::state::{WorkflowMode, WorkflowRun, WorkflowRunState};

    let (dir, runtime) = setup_auth_test_runtime().await;
    let repo = SqliteWorkflowRepository::new(runtime.pool().clone());

    let run_id = ExecutionRunId::new();
    let exec_run = m31a::state::ExecutionRun::new(run_id, "workflow execution".into());
    let run_repo = m31a::persistence::sqlite::repositories::SqliteExecutionRunRepository::new(
        runtime.pool().clone(),
    );
    run_repo.insert(&exec_run).await.expect("insert exec run");

    let run = WorkflowRun::new(
        "test-workflow",
        1,
        dir.path().to_path_buf(),
        WorkflowMode::Autonomous,
    )
    .with_run_id(run_id);
    assert_eq!(run.run_id, Some(run_id));

    repo.create_run(&run).await.expect("create run");

    let loaded = repo
        .get_run(run.id)
        .await
        .expect("get run")
        .expect("run must exist");
    assert_eq!(
        loaded.run_id,
        Some(run_id),
        "persisted WorkflowRun must preserve canonical run_id"
    );
    assert_eq!(loaded.status, WorkflowRunState::Pending);

    let by_exec_run = repo
        .get_run_by_execution_run_id(run_id)
        .await
        .expect("get by execution run id")
        .expect("run must exist");
    assert_eq!(by_exec_run.id, run.id);
}

#[tokio::test]
async fn test_storage_projection_rejects_symlink_and_workspace_escape() {
    use tempfile::tempdir;

    let dir = tempdir().expect("tempdir");
    let root = dir.path();

    // 1. Outside workspace escape must fail
    assert!(
        StorageLayout::validate_projection_write(root, "../outside", "roadmap.md").is_err(),
        "writing outside workspace must fail validation"
    );
    assert!(
        StorageLayout::validate_projection_write(root, ".m31a/project", "../escaped.md").is_err(),
        "path traversal in filename must fail validation"
    );

    // 2. Normal project document write under .m31a/project/ must pass
    let valid_proj = StorageLayout::validate_projection_write(root, ".m31a/project", "roadmap.md");
    assert!(
        valid_proj.is_ok(),
        "writing inside .m31a/project/ must pass validation"
    );
    assert_eq!(
        valid_proj.unwrap(),
        root.join(".m31a").join("project").join("roadmap.md")
    );

    // 3. Sensitive control directory access (e.g. projection_dir = ".m31a" directly) must fail
    assert!(
        StorageLayout::validate_projection_write(root, ".m31a", "m31a.db").is_err(),
        "writing to control dir without project subpath must fail validation"
    );
}

#[tokio::test]
async fn test_recovery_attempt_canonical_run_and_nullable_task_fk() {
    let (_dir, runtime) = setup_auth_test_runtime().await;
    let tracker = m31a::recovery::budget::RecoveryBudgetTracker::default();
    let mid = m31a::ids::MissionId::new();
    let tid = m31a::ids::TaskId::new();

    // Insert ExecutionRun so run_id foreign key constraint is satisfied.
    let exec_run = m31a::state::ExecutionRun::new(mid, "test objective".into());
    let run_repo = m31a::persistence::sqlite::repositories::SqliteExecutionRunRepository::new(
        runtime.pool().clone(),
    );
    run_repo.insert(&exec_run).await.unwrap();

    // 1. Record attempt with a non-existent task_id (e.g. mission-level recovery).
    // Must succeed without foreign key failure, binding task_id as NULL and run_id as mid.
    let rec = m31a::recovery::budget::RecoveryAttemptRecord {
        mission_id: mid,
        task_id: tid,
        failure_class: m31a::kernel::seams::recovery::FailureClassification::Transient,
        strategy: "test_strategy".into(),
        attempt_number: 1,
        budget_consumed: 1,
        remaining_class_budget: 0,
        remaining_overall_budget: 0,
        backoff_delay_ms: 0,
        action_taken: "test_action".into(),
        result: "test_result".into(),
        mutation_fingerprint: None,
        semantic_signature: None,
    };
    let attempt_id = tracker
        .record_attempt(runtime.pool(), rec.clone())
        .await
        .expect("record_attempt must succeed even with non-existent task_id");

    let row: (Option<Vec<u8>>, Option<Vec<u8>>) =
        sqlx::query_as("SELECT run_id, task_id FROM recovery_attempts WHERE id = ?")
            .bind(attempt_id.as_bytes().as_slice())
            .fetch_one(runtime.pool())
            .await
            .expect("fetch recorded attempt");

    assert_eq!(
        row.0.as_deref(),
        Some(mid.as_bytes().as_slice()),
        "canonical run_id must be populated"
    );
    assert!(
        row.1.is_none(),
        "non-existent task_id must be recorded as NULL to avoid FK failure"
    );

    // 2. When Task exists, record_attempt records both run_id and task_id.
    let task = m31a::state::task::Task::new(tid, mid, "task 1".into());
    let task_repo = m31a::persistence::sqlite::repositories::task::SqliteTaskRepository::new(
        runtime.pool().clone(),
    );
    task_repo.insert(&task).await.unwrap();

    let attempt_id_2 = tracker
        .record_attempt(runtime.pool(), rec)
        .await
        .expect("record_attempt must succeed with existing task_id");

    let row2: (Option<Vec<u8>>, Option<Vec<u8>>) =
        sqlx::query_as("SELECT run_id, task_id FROM recovery_attempts WHERE id = ?")
            .bind(attempt_id_2.as_bytes().as_slice())
            .fetch_one(runtime.pool())
            .await
            .expect("fetch recorded attempt 2");

    assert_eq!(row2.0.as_deref(), Some(mid.as_bytes().as_slice()));
    assert_eq!(row2.1.as_deref(), Some(tid.as_bytes().as_slice()));
}
