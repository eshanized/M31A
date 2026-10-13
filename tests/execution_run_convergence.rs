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
    assert!(!PlanningChoice::is_explicit_planning_request(
        "Fix the null pointer in auth.rs"
    ));
    assert!(!PlanningChoice::is_explicit_planning_request(
        "Add a retry loop to the worker"
    ));
}

#[test]
fn test_planning_choice_resolve_for_request() {
    // Explicit flag wins over everything.
    assert_eq!(
        PlanningChoice::resolve_for_request(
            "fix bug",
            Some(PlanningChoice::ProjectPlanning),
            Some(PlanningChoice::DirectExecution),
            true,
        ),
        Some(PlanningChoice::ProjectPlanning)
    );
    // Explicit planning text counts as consent even headless.
    assert_eq!(
        PlanningChoice::resolve_for_request("init project please", None, None, true),
        Some(PlanningChoice::ProjectPlanning)
    );
    // Stored session choice is reused.
    assert_eq!(
        PlanningChoice::resolve_for_request(
            "fix bug",
            None,
            Some(PlanningChoice::ProjectPlanning),
            false,
        ),
        Some(PlanningChoice::ProjectPlanning)
    );
    // Headless without intent is deterministic direct execution (never hangs,
    // never silently opts into project-doc generation).
    assert_eq!(
        PlanningChoice::resolve_for_request("fix bug", None, None, true),
        Some(PlanningChoice::DirectExecution)
    );
    // Interactive without stored choice must be asked (None → caller emits
    // PlanningChoiceRequired instead of guessing).
    assert_eq!(
        PlanningChoice::resolve_for_request("fix bug", None, None, false),
        None
    );
    // Planning-declined behavior: direct execution performs no Genesis.
    assert_eq!(
        PlanningChoice::resolve_for_request(
            "fix bug",
            Some(PlanningChoice::DirectExecution),
            None,
            false,
        ),
        Some(PlanningChoice::DirectExecution)
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

#[test]
fn test_projection_root_symlink_rejected_for_writes() {
    let temp = tempfile::tempdir().unwrap();
    let ws = temp.path();
    let outside = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(ws.join(".m31a")).unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(outside.path(), ws.join(".m31a").join("project")).unwrap();
    #[cfg(unix)]
    {
        let res = StorageLayout::validate_projection_write(ws, ".m31a/project", "PROJECT.md");
        assert!(
            res.is_err(),
            "symlinked project root must be rejected for writes"
        );
    }
}

// ── Migration root-symlink rejection ──────────────────────────────────────

#[test]
fn test_legacy_source_root_symlink_rejected() {
    let temp = tempfile::tempdir().unwrap();
    let ws = temp.path();
    let outside = tempfile::tempdir().unwrap();
    std::fs::write(outside.path().join("SECRET.md"), "external secret\n").unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(outside.path(), ws.join(".planning")).unwrap();
    #[cfg(unix)]
    {
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
}

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
    #[cfg(unix)]
    std::os::unix::fs::symlink(outside.path(), ws.join(".m31a").join("project")).unwrap();
    #[cfg(unix)]
    {
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

#[test]
fn test_project_context_rejects_symlinked_root() {
    let temp = tempfile::tempdir().unwrap();
    let ws = temp.path();
    let outside = tempfile::tempdir().unwrap();
    std::fs::write(outside.path().join("PROJECT.md"), "# EXTERNAL\n").unwrap();
    std::fs::create_dir_all(ws.join(".m31a")).unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(outside.path(), ws.join(".m31a").join("project")).unwrap();
    #[cfg(unix)]
    {
        let ctx = m31a::prompt::context::DurableStateContext::load_from_workspace(ws);
        assert!(
            ctx.project_charter.is_none(),
            "symlinked project root must not enter model context"
        );
    }
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

fn make_test_auth(
    session_id: &m31a::ids::SessionId,
    plan_rev: u32,
    task_rev: u32,
) -> m31a::planning::review::ExecutionAuthorization {
    m31a::planning::review::ExecutionAuthorization::new(
        session_id.to_string(),
        plan_rev,
        task_rev,
        "operator",
    )
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
    // Auth binds revision 2 while the candidate plan carries revision 1.
    let auth = make_test_auth(&session.id, 2, 1);

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
    let auth = make_test_auth(&other_session.id, 1, 1);

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
    let mut auth = make_test_auth(&session.id, 1, 1);
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
