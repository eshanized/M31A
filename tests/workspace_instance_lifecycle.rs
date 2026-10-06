//! Workspace-Instance Lifecycle Regression Suite.
//!
//! Guards the first-run onboarding lifecycle against the production bug where
//! every `m31a` invocation re-launched the setup wizard:
//!
//! - (A) fresh workspace reports not initialized;
//! - (B) first startup requires onboarding;
//! - (C) successful onboarding durably persists (sentinel + SQLite agree);
//! - (D) new-process simulation (fresh resolver) still reports initialized;
//! - (E) second startup does NOT trigger onboarding;
//! - (F) a different new workspace requires onboarding independently;
//! - (G) cancelled/partial onboarding leaves the workspace uninitialized;
//! - (H) corrupt / version-skewed / identity-mismatched state errors
//!   deterministically (fail-closed, never a silent wizard);
//! - (I) TUI startup on an initialized workspace goes directly to cockpit
//!   (no wizard) and the TUI layer holds no startup authority;
//! - (J) CLI startup on an initialized workspace does not enter the wizard
//!   (`m31a init` is idempotent, `--force` re-enters explicitly);
//! - (K) runtime reconstruction reuses existing persistent state
//!   (stable identity across reloads);
//! - (L) same-process duplicate resolution returns the same instance.
//!
//! Architectural guards assert a single canonical startup authority.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tempfile::TempDir;

use m31a::init::{
    InitError, InitManager, InitState, OnboardingReason, SetupStep, StartupDecision,
    WorkspaceInstanceStore, canonicalize_workspace_root, persist_successful_onboarding,
    resolve_shared_workspace_instance, resolve_startup, resolve_workspace_instance,
    workspace_instance_id,
};
use m31a::persistence::sqlite::initialize_database;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

async fn test_pool(ws: &Path) -> sqlx::SqlitePool {
    let data_dir = ws.join(".m31a");
    fs::create_dir_all(&data_dir).expect("create .m31a");
    initialize_database(&data_dir.join("m31a.db"))
        .await
        .expect("initialize database")
}

fn fresh_workspace() -> TempDir {
    tempfile::tempdir().expect("create temp workspace")
}

/// Simulate the first-run wizard completing successfully through the
/// canonical production completion path (no UI involved).
async fn complete_onboarding_like_wizard(ws: &Path, pool: &sqlx::SqlitePool) {
    persist_successful_onboarding(ws, pool)
        .await
        .expect("canonical onboarding completion must succeed");
}

// ---------------------------------------------------------------------------
// A/B/C/D/E: full fresh -> onboard -> restart lifecycle
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_a_fresh_workspace_reports_not_initialized() {
    let tmp = fresh_workspace();
    let ws = tmp.path();
    let pool = test_pool(ws).await;

    let decision = resolve_startup(ws, &pool).await.expect("resolve startup");
    assert!(
        decision.needs_onboarding(),
        "fresh workspace must require onboarding"
    );
    match &decision {
        StartupDecision::NeedsOnboarding { reason, .. } => {
            assert_eq!(*reason, OnboardingReason::FreshWorkspace);
        }
        StartupDecision::Initialized(_) => panic!("fresh workspace must not be initialized"),
    }
    assert!(!decision.instance().is_initialized());
    assert!(!decision.instance().is_onboarded());
}

#[tokio::test]
async fn test_b_first_startup_requires_onboarding() {
    let tmp = fresh_workspace();
    let ws = tmp.path();
    let pool = test_pool(ws).await;

    // The startup authority — not process freshness — demands onboarding.
    let instance = resolve_workspace_instance(ws, &pool)
        .await
        .expect("resolve instance");
    assert!(instance.needs_onboarding());
    assert_eq!(
        instance.onboarding_reason(),
        Some(OnboardingReason::FreshWorkspace)
    );
}

#[tokio::test]
async fn test_c_successful_onboarding_persists_sentinel_and_db() {
    let tmp = fresh_workspace();
    let ws = tmp.path();
    let pool = test_pool(ws).await;

    complete_onboarding_like_wizard(ws, &pool).await;

    // Sentinel says Onboarded ...
    let manager = InitManager::new(ws).expect("reload manager");
    assert!(manager.is_onboarded());
    assert!(manager.is_initialized());
    assert!(manager.is_sentinel_present());

    // ... and the canonical SQLite record agrees.
    let row: (String,) = sqlx::query_as("SELECT value FROM system_state WHERE key = 'init_state'")
        .fetch_one(&pool)
        .await
        .expect("init_state row must exist");
    assert_eq!(row.0, "Onboarded");

    let decision = resolve_startup(ws, &pool).await.expect("resolve");
    assert!(!decision.needs_onboarding());
}

#[tokio::test]
async fn test_d_initialization_survives_process_restart_simulation() {
    let tmp = fresh_workspace();
    let ws = tmp.path();
    let pool = test_pool(ws).await;
    complete_onboarding_like_wizard(ws, &pool).await;

    // Simulate a brand-new process: drop the pool, build a fresh resolver and
    // a fresh connection against the same workspace files.
    drop(pool);
    let data_dir = ws.join(".m31a");
    let pool2 = initialize_database(&data_dir.join("m31a.db"))
        .await
        .expect("reopen database");
    let store = WorkspaceInstanceStore::new();
    let instance = store
        .resolve(ws, &pool2)
        .await
        .expect("re-resolve after restart");
    assert!(
        instance.is_initialized(),
        "workspace MUST still report initialized after process restart"
    );
    assert!(instance.is_onboarded());

    let decision = resolve_startup(ws, &pool2).await.expect("resolve");
    assert!(
        !decision.needs_onboarding(),
        "second process must NOT trigger onboarding"
    );
}

#[tokio::test]
async fn test_e_second_startup_skips_onboarding() {
    let tmp = fresh_workspace();
    let ws = tmp.path();
    let pool = test_pool(ws).await;
    complete_onboarding_like_wizard(ws, &pool).await;

    // First post-onboarding startup ...
    let first = resolve_startup(ws, &pool).await.expect("first startup");
    assert!(!first.needs_onboarding());

    // ... and a third invocation behaves identically (no wizard twice).
    m31a::init::invalidate_instance(ws);
    let second = resolve_startup(ws, &pool).await.expect("second startup");
    assert!(!second.needs_onboarding());
    assert!(matches!(second, StartupDecision::Initialized(_)));
}

// ---------------------------------------------------------------------------
// F: workspace scoping
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_f_new_workspace_requires_onboarding_independently() {
    let tmp_a = fresh_workspace();
    let tmp_b = fresh_workspace();
    let pool_a = test_pool(tmp_a.path()).await;
    let pool_b = test_pool(tmp_b.path()).await;

    complete_onboarding_like_wizard(tmp_a.path(), &pool_a).await;

    // Workspace A stays initialized ...
    let decision_a = resolve_startup(tmp_a.path(), &pool_a).await.expect("a");
    assert!(!decision_a.needs_onboarding());

    // ... while workspace B is independent and still requires onboarding.
    let decision_b = resolve_startup(tmp_b.path(), &pool_b).await.expect("b");
    assert!(decision_b.needs_onboarding());

    // Identities are distinct and stable.
    assert_ne!(
        decision_a.instance().instance_id(),
        decision_b.instance().instance_id()
    );
}

// ---------------------------------------------------------------------------
// G: cancelled / partial onboarding never completes
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_g_cancelled_onboarding_leaves_workspace_uninitialized() {
    let tmp = fresh_workspace();
    let ws = tmp.path();
    let pool = test_pool(ws).await;

    // Simulate a wizard run that advances two steps then is cancelled
    // (Esc): durable state exists but is mid-flow, never Onboarded.
    let mut mgr = InitManager::new(ws).expect("manager");
    mgr.transition_to(InitState::Checking).expect("checking");
    mgr.transition_to(InitState::Configuring(SetupStep::WorkspaceTrust))
        .expect("step 1");
    // ... cancellation: no further transitions, no migration to SQLite.

    let decision = resolve_startup(ws, &pool).await.expect("resolve");
    assert!(
        decision.needs_onboarding(),
        "cancelled onboarding must still require onboarding"
    );
    assert!(!decision.instance().is_initialized());
    assert!(!decision.instance().is_onboarded());
    assert_eq!(
        decision.instance().onboarding_reason(),
        Some(OnboardingReason::IncompleteFlow {
            state: InitState::Configuring(SetupStep::WorkspaceTrust)
        })
    );

    // The database must NOT claim onboarding either.
    let row: Option<(String,)> =
        sqlx::query_as("SELECT value FROM system_state WHERE key = 'init_state'")
            .fetch_optional(&pool)
            .await
            .expect("query init_state");
    assert!(
        row.is_none(),
        "cancelled onboarding must not write an init_state record"
    );
}

#[tokio::test]
async fn test_g2_direct_jump_to_onboarded_remains_rejected() {
    // The original bug: `transition_to(Onboarded)` from Uninitialized was
    // attempted and its error swallowed. The jump must stay illegal so no
    // path can fake completion.
    let tmp = fresh_workspace();
    let mut mgr = InitManager::new(tmp.path()).expect("manager");
    let err = mgr
        .transition_to(InitState::Onboarded)
        .expect_err("direct Uninitialized -> Onboarded jump must fail");
    assert!(
        matches!(err, InitError::InvalidTransition { .. }),
        "direct Uninitialized -> Onboarded must stay an invalid transition, got: {err}"
    );
    assert!(!mgr.is_onboarded());
}

#[tokio::test]
async fn test_g3_wizard_validation_gates_cannot_be_skipped() {
    // Reaching the final page without satisfying gates must not complete.
    let tmp = fresh_workspace();
    let mut wizard = m31a::tui::screens::wizard::SetupWizardScreen::new(tmp.path().to_path_buf());
    assert_eq!(wizard.current_step(), SetupStep::WorkspaceTrust);
    assert!(!wizard.can_advance(), "trust gate must block advancement");
    assert!(!wizard.advance(), "advance without trust must fail");
    assert_eq!(wizard.current_step(), SetupStep::WorkspaceTrust);
}

// ---------------------------------------------------------------------------
// H: corrupted state is fail-closed and explicit
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_h_corrupt_sentinel_is_an_explicit_error_not_a_wizard() {
    let tmp = fresh_workspace();
    let ws = tmp.path();
    let pool = test_pool(ws).await;

    fs::write(ws.join(".m31a").join("init.json"), "{ not valid json")
        .expect("write corrupt sentinel");

    let err = resolve_startup(ws, &pool)
        .await
        .expect_err("corrupt sentinel must fail resolution");
    assert!(
        matches!(err, InitError::CorruptedSentinel { .. }),
        "expected CorruptedSentinel, got: {err}"
    );
}

#[tokio::test]
async fn test_h2_unsupported_version_is_rejected_fail_closed() {
    let tmp = fresh_workspace();
    let ws = tmp.path();
    let pool = test_pool(ws).await;

    let payload = serde_json::json!({
        "version": 9999u32,
        "state": "Onboarded",
        "step_data": {},
        "created_at": "2026-01-01T00:00:00Z",
        "updated_at": "2026-01-01T00:00:00Z",
    });
    fs::write(
        ws.join(".m31a").join("init.json"),
        serde_json::to_string_pretty(&payload).unwrap(),
    )
    .expect("write version-skewed sentinel");

    let err = resolve_startup(ws, &pool)
        .await
        .expect_err("unsupported version must fail resolution");
    assert!(
        matches!(err, InitError::UnsupportedVersion { .. }),
        "expected UnsupportedVersion, got: {err}"
    );
}

#[tokio::test]
async fn test_h3_workspace_identity_mismatch_is_rejected() {
    let tmp = fresh_workspace();
    let ws = tmp.path();
    let pool = test_pool(ws).await;
    complete_onboarding_like_wizard(ws, &pool).await;

    // Tamper the stored identity (simulates a database copied from another
    // workspace): resolution must refuse to trust it silently. The
    // application DB is global/shared, so identity is namespaced by
    // instance id; tamper the namespaced key (and the legacy key for
    // pre-migration single-workspace DBs).
    let instance_id = workspace_instance_id(&canonicalize_workspace_root(ws));
    let namespaced_root = format!("workspace_root:{instance_id}");
    sqlx::query("UPDATE system_state SET value = ? WHERE key = ?")
        .bind("/elsewhere/attacker-workspace")
        .bind(&namespaced_root)
        .execute(&pool)
        .await
        .expect("tamper namespaced identity");
    sqlx::query("UPDATE system_state SET value = ? WHERE key = 'workspace_root'")
        .bind("/elsewhere/attacker-workspace")
        .execute(&pool)
        .await
        .expect("tamper legacy identity");
    m31a::init::invalidate_instance(ws);

    let err = resolve_startup(ws, &pool)
        .await
        .expect_err("identity mismatch must fail resolution");
    assert!(
        matches!(err, InitError::InconsistentState(_)),
        "expected InconsistentState, got: {err}"
    );
}

#[tokio::test]
async fn test_h4_legacy_sentinel_without_version_stays_valid() {
    // Backward compatibility: files written before versioning (e.g. the
    // golden PTY fixture shape) must keep loading.
    let tmp = fresh_workspace();
    let ws = tmp.path();
    let pool = test_pool(ws).await;

    let legacy = serde_json::json!({
        "state": "Onboarded",
        "step_data": {},
        "created_at": "2026-09-19T00:00:00Z",
        "updated_at": "2026-09-19T00:00:00Z"
    });
    fs::write(ws.join(".m31a").join("init.json"), legacy.to_string())
        .expect("write legacy sentinel");

    let decision = resolve_startup(ws, &pool)
        .await
        .expect("legacy sentinel must load");
    assert!(
        !decision.needs_onboarding(),
        "legacy Onboarded sentinel must count as initialized"
    );
}

// ---------------------------------------------------------------------------
// Healing: sentinel/DB disagreement reconciles without a wizard
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_heal_db_recreated_from_sentinel_without_onboarding() {
    let tmp = fresh_workspace();
    let ws = tmp.path();
    let pool = test_pool(ws).await;
    complete_onboarding_like_wizard(ws, &pool).await;

    // Simulate a lost database (fresh empty DB file): the sentinel proves
    // prior completion, so startup heals the DB instead of wizarding.
    drop(pool);
    let db_path = ws.join(".m31a").join("m31a.db");
    fs::remove_file(&db_path).expect("delete db");
    let pool2 = initialize_database(&db_path)
        .await
        .expect("recreate database");
    m31a::init::invalidate_instance(ws);

    let decision = resolve_startup(ws, &pool2)
        .await
        .expect("healing resolve must succeed");
    assert!(
        !decision.needs_onboarding(),
        "sentinel-proven workspace must not re-run onboarding after DB loss"
    );
}

#[tokio::test]
async fn test_heal_sentinel_restored_from_db_without_onboarding() {
    let tmp = fresh_workspace();
    let ws = tmp.path();
    let pool = test_pool(ws).await;
    complete_onboarding_like_wizard(ws, &pool).await;

    // Simulate a lost sentinel with the database intact.
    fs::remove_file(ws.join(".m31a").join("init.json")).expect("delete sentinel");
    m31a::init::invalidate_instance(ws);

    let decision = resolve_startup(ws, &pool)
        .await
        .expect("healing resolve must succeed");
    assert!(
        !decision.needs_onboarding(),
        "db-proven workspace must not re-run onboarding after sentinel loss"
    );
    assert!(
        ws.join(".m31a").join("init.json").exists(),
        "sentinel must be restored from the database record"
    );
}

// ---------------------------------------------------------------------------
// Config vs initialization are distinct authorities
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_config_exists_without_onboarding_is_not_initialized() {
    let tmp = fresh_workspace();
    let ws = tmp.path();
    let pool = test_pool(ws).await;

    // A user can have configuration without ever completing onboarding.
    fs::write(
        ws.join(".m31a").join("config.toml"),
        "[provider]\ndefault = \"nvidia_nim\"\n",
    )
    .expect("write config");

    let decision = resolve_startup(ws, &pool).await.expect("resolve");
    assert!(
        decision.needs_onboarding(),
        "config.toml presence must NOT imply initialization"
    );
}

// ---------------------------------------------------------------------------
// I/J: TUI + CLI entry behavior on initialized workspaces
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_i_initialized_workspace_goes_directly_to_cockpit() {
    let tmp = fresh_workspace();
    let ws = tmp.path();
    let pool = test_pool(ws).await;
    complete_onboarding_like_wizard(ws, &pool).await;

    // TUI startup consumes the same canonical decision: initialized means
    // the cockpit path with no wizard involvement.
    let decision = resolve_startup(ws, &pool).await.expect("resolve");
    assert!(
        matches!(decision, StartupDecision::Initialized(_)),
        "initialized workspace must resolve to the cockpit path"
    );
    assert!(!decision.needs_onboarding());
}

#[tokio::test]
async fn test_j_cli_init_is_idempotent_and_force_reenters() {
    use m31a::cli::dispatch::{CliDispatcher, RuntimeCommand};

    let tmp = fresh_workspace();
    let ws = tmp.path();
    let pool = test_pool(ws).await;
    let bus = Arc::new(m31a::events::bus::BroadcastEventBus::new(16));
    let dispatcher = CliDispatcher::production(pool.clone(), ws.to_path_buf(), bus);

    // Fresh workspace: explicit `m31a init` reports not-initialized without
    // faking completion.
    let output = dispatcher
        .dispatch(RuntimeCommand::RunInit { force: false })
        .await
        .expect("dispatch init");
    assert_eq!(output.exit_code, 0);
    assert!(
        !resolve_startup(ws, &pool)
            .await
            .expect("resolve")
            .instance()
            .is_initialized(),
        "`m31a init` must not fake onboarding completion"
    );

    // After real completion, `m31a init` is a no-op success ...
    complete_onboarding_like_wizard(ws, &pool).await;
    let output = dispatcher
        .dispatch(RuntimeCommand::RunInit { force: false })
        .await
        .expect("dispatch init when done");
    assert!(output.text.contains("already initialized"));

    // ... while `--force` explicitly re-enters onboarding (recovery path).
    let output = dispatcher
        .dispatch(RuntimeCommand::RunInit { force: true })
        .await
        .expect("dispatch init --force");
    assert_eq!(output.exit_code, 0);
    m31a::init::invalidate_instance(ws);
    let decision = resolve_startup(ws, &pool).await.expect("resolve");
    assert!(
        decision.needs_onboarding(),
        "`m31a init --force` must re-enter onboarding"
    );
}

// ---------------------------------------------------------------------------
// K: stable identity + reconstruction reuse
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_k_workspace_identity_stable_across_reload() {
    let tmp = fresh_workspace();
    let ws = tmp.path();
    let pool = test_pool(ws).await;

    let first = resolve_workspace_instance(ws, &pool)
        .await
        .expect("first resolve");
    complete_onboarding_like_wizard(ws, &pool).await;
    m31a::init::invalidate_instance(ws);
    let second = resolve_workspace_instance(ws, &pool)
        .await
        .expect("second resolve");

    assert_eq!(first.instance_id(), second.instance_id());
    assert_eq!(first.workspace_root(), second.workspace_root());
    assert_eq!(first.data_dir(), second.data_dir());

    // Relative vs absolute spellings of the same workspace resolve to one
    // canonical root (wizard/startup can no longer diverge).
    let relative_like = PathBuf::from(".");
    let cwd_guard = std::env::current_dir().expect("cwd");
    std::env::set_current_dir(ws).expect("chdir into workspace");
    let via_relative = canonicalize_workspace_root(&relative_like);
    std::env::set_current_dir(cwd_guard).expect("restore cwd");
    assert_eq!(via_relative, second.workspace_root().to_path_buf());

    // Instance id is deterministic.
    assert_eq!(
        workspace_instance_id(second.workspace_root()),
        second.instance_id()
    );
}

// ---------------------------------------------------------------------------
// L: same-process duplicate resolution shares one instance
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_l_same_process_duplicate_resolution_shares_instance() {
    let tmp = fresh_workspace();
    let ws = tmp.path();
    let pool = test_pool(ws).await;
    complete_onboarding_like_wizard(ws, &pool).await;

    let store = WorkspaceInstanceStore::new();
    let first = store.resolve(ws, &pool).await.expect("first");
    let second = store.resolve(ws, &pool).await.expect("second");
    assert!(
        Arc::ptr_eq(&first, &second),
        "same workspace resolved twice in one process must share one instance"
    );

    let shared_a = resolve_shared_workspace_instance(ws, &pool)
        .await
        .expect("shared a");
    let shared_b = resolve_shared_workspace_instance(ws, &pool)
        .await
        .expect("shared b");
    assert!(
        Arc::ptr_eq(&shared_a, &shared_b),
        "process-global registry must not create competing instances"
    );
}

// ---------------------------------------------------------------------------
// Architectural regression guards: ONE canonical startup authority
// ---------------------------------------------------------------------------

fn repo_file(relative: &str) -> String {
    let manifest = env!("CARGO_MANIFEST_DIR");
    fs::read_to_string(Path::new(manifest).join(relative))
        .unwrap_or_else(|_| panic!("read {relative}"))
}

#[test]
fn test_arch_single_startup_authority_in_main() {
    let main_rs = repo_file("src/main.rs");
    // The canonical resolver is the only onboarding decision point.
    assert!(
        main_rs.contains("resolve_startup"),
        "main.rs must resolve startup through the canonical instance authority"
    );
    assert!(
        !main_rs.contains("InitManager::new"),
        "main.rs must not construct InitManager directly (use resolve_startup)"
    );
    assert!(
        !main_rs.contains("is_onboarded()"),
        "main.rs must not call is_onboarded directly (consume StartupDecision)"
    );
    assert!(
        !main_rs.contains("transition_to("),
        "main.rs must never transition init state directly"
    );
    // The setup wizard may only be driven inside run_tui_or_fallback (the
    // single designated first-run path receiving the resolved decision).
    let wizard_uses = main_rs
        .match_indices("SetupWizardScreen")
        .map(|(i, _)| i)
        .collect::<Vec<_>>();
    assert!(
        !wizard_uses.is_empty(),
        "run_tui_or_fallback must drive the first-run wizard"
    );
    let fallback_fn = main_rs
        .find("async fn run_tui_or_fallback")
        .expect("run_tui_or_fallback must exist");
    for pos in wizard_uses {
        assert!(
            pos > fallback_fn,
            "SetupWizardScreen must only appear inside run_tui_or_fallback"
        );
    }
}

#[test]
fn test_arch_tui_holds_no_startup_authority() {
    for relative in [
        "src/tui/app.rs",
        "src/tui/shell/workspace.rs",
        "src/tui/screens/wizard.rs",
    ] {
        let content = repo_file(relative);
        assert!(
            !content.contains("InitManager"),
            "{relative} must not reference InitManager (TUI is a projection)"
        );
        assert!(
            !content.contains("is_onboarded"),
            "{relative} must not decide onboarding"
        );
        assert!(
            !content.contains("init.json"),
            "{relative} must not touch the sentinel file"
        );
    }
}

#[test]
fn test_arch_no_silent_transition_swallow_in_startup() {
    // The original defect pattern: `let _ = ...transition_to(Onboarded)` and
    // `let _ = wizard.persist_configuration()`. Persistence results for
    // lifecycle transitions may never be silently discarded.
    for relative in [
        "src/main.rs",
        "src/tui/app.rs",
        "src/tui/shell/workspace.rs",
    ] {
        let content = repo_file(relative);
        for (idx, line) in content.lines().enumerate() {
            let trimmed = line.trim();
            let discards = trimmed.starts_with("let _ =");
            let lifecycle_call = trimmed.contains("transition_to")
                || trimmed.contains("persist_configuration")
                || trimmed.contains("persist_sentinel")
                || trimmed.contains("persist_successful_onboarding")
                || trimmed.contains("complete_and_migrate_to_db")
                || trimmed.contains("record_onboarding");
            assert!(
                !(discards && lifecycle_call),
                "{relative}:{} silently discards a lifecycle persistence result: {trimmed}",
                idx + 1
            );
        }
    }
    let wizard_rs = repo_file("src/tui/screens/wizard.rs");
    assert!(
        !wizard_rs.contains("transition_to"),
        "wizard screen must not transition init state (completion owns that)"
    );
}
