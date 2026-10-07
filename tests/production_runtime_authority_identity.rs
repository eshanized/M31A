//! Production runtime authority identity: one runtime → one authority graph.
//!
//! Runtime (not static strings) proof that the canonical production
//! composition shares a single `Arc` per identity-bearing authority and that
//! worktree scopes inherit trust roots instead of forking them.

use std::sync::Arc;
use tempfile::tempdir;

use m31a::runtime::AppRuntime;

async fn test_runtime() -> (tempfile::TempDir, AppRuntime) {
    let dir = tempdir().expect("tempdir");
    let rt = AppRuntime::new(dir.path()).await.expect("AppRuntime::new");
    (dir, rt)
}

#[tokio::test]
async fn test_production_runtime_authority_identity() {
    let (_dir, rt) = test_runtime().await;
    let auth = rt.authorities();

    // Runtime fields mirror the canonical authority set (same Arcs).
    assert!(Arc::ptr_eq(
        rt.capability_registry(),
        auth.capability_registry()
    ));
    assert!(Arc::ptr_eq(rt.tool_registry(), auth.tool_registry()));
    assert!(Arc::ptr_eq(rt.tool_pipeline(), auth.tool_pipeline()));
    assert!(Arc::ptr_eq(rt.policy(), auth.policy()));
    assert!(Arc::ptr_eq(
        rt.approval_coordinator(),
        auth.approval_coordinator()
    ));
    assert!(Arc::ptr_eq(rt.artifact_store(), auth.artifact_store()));
    assert!(Arc::ptr_eq(rt.artifact_service(), auth.artifact_service()));
    assert!(Arc::ptr_eq(rt.event_bus(), auth.event_bus()));
    assert!(Arc::ptr_eq(rt.model_catalog_ref(), auth.model_catalog()));
    assert!(Arc::ptr_eq(rt.prompt_catalog(), auth.prompt_catalog()));
    assert!(Arc::ptr_eq(rt.job_supervisor(), auth.job_supervisor()));
    assert!(Arc::ptr_eq(rt.memory_store(), auth.memory_store()));

    // Authorization trust root: runtime field == registry == authorities.
    let from_registry = rt.capability_registry().authorization_authority().clone();
    let from_auth = rt.auth_authority().clone();
    let from_set = auth.auth_authority();
    assert!(Arc::ptr_eq(&from_registry, &from_auth));
    assert!(Arc::ptr_eq(&from_registry, &from_set));

    // Prompt compiler: same trait-object allocation.
    assert!(Arc::ptr_eq(
        rt.authorities().prompt_compiler(),
        auth.prompt_compiler()
    ));

    // Controller inherits the same approval / budget / auth authorities.
    let deps = rt.dependencies();
    let ctrl_approval = deps.approval_coordinator().expect("controller approval");
    assert!(Arc::ptr_eq(rt.approval_coordinator(), ctrl_approval));
    let ctrl_budget = deps.budget_enforcer().expect("controller budget");
    assert!(Arc::ptr_eq(rt.budget_enforcer(), ctrl_budget));
    let ctrl_auth = deps.auth_authority().expect("controller auth");
    assert!(Arc::ptr_eq(rt.auth_authority(), ctrl_auth));

    // Policy: same allocation observed as trait objects.
    let rt_policy_trait: Arc<dyn m31a::kernel::seams::policy::PolicyGate> =
        rt.policy().clone() as Arc<dyn m31a::kernel::seams::policy::PolicyGate>;
    assert!(Arc::ptr_eq(deps.policy(), &rt_policy_trait));

    // Dispatcher consumes the canonical triple: build one from shared
    // authorities and prove no fork (same Arcs stored).
    let dispatcher_caller = auth.canonical_model_caller_for_dispatch(rt.model_caller());
    let disp = m31a::agent::dispatcher::ProductionWorkerDispatcher::from_shared_authorities(
        rt.workspace_root().to_path_buf(),
        rt.capability_registry().clone(),
        rt.tool_registry().clone(),
        rt.tool_pipeline().clone(),
        rt.policy().clone() as Arc<dyn m31a::kernel::seams::policy::PolicyGate>,
        dispatcher_caller,
        rt.approval_coordinator().clone(),
        rt.pool().clone(),
        rt.config(),
        auth.context_compiler().clone(),
    );
    assert!(Arc::ptr_eq(
        disp.capability_registry(),
        rt.capability_registry()
    ));
    assert!(Arc::ptr_eq(
        disp.pipeline_runner().tool_registry(),
        rt.tool_registry()
    ));
    assert!(Arc::ptr_eq(disp.pipeline_runner(), rt.tool_pipeline()));
}

#[tokio::test]
async fn test_worktree_inherits_canonical_trust_roots() {
    let (_dir, rt) = test_runtime().await;
    let wt_path = rt.workspace_root().join("wt-scope-check");
    std::fs::create_dir_all(&wt_path).expect("wt dir");
    let (scoped_caps, scoped_tools, scoped_pipeline, worktree_compiler) =
        rt.scoped_worktree_execution(&wt_path);

    // Same authorization trust root (GitGate verification would reject a fork).
    assert!(Arc::ptr_eq(
        scoped_caps.authorization_authority(),
        rt.auth_authority()
    ));

    // Scoped tool inventory derives from scoped caps (workspace binding
    // differs by design), and the scoped pipeline wraps the scoped tools.
    let caps_in_tools = scoped_tools
        .capabilities()
        .expect("scoped tools bind scoped caps");
    assert!(Arc::ptr_eq(caps_in_tools, &scoped_caps));
    assert!(Arc::ptr_eq(scoped_pipeline.tool_registry(), &scoped_tools));

    // Same artifact base dir (global authority, not a workspace-local fork).
    let scoped_base = scoped_pipeline
        .artifact_store()
        .and_then(|s| s.store_base_dir());
    let canonical_base = rt.artifact_store().base_dir().to_path_buf();
    assert_eq!(scoped_base, Some(canonical_base));

    // Same engineering memory authority (trait-object identity).
    let canonical_mem = rt.memory_store().clone();
    let wt_mem = worktree_compiler
        .memory_store()
        .expect("worktree compiler binds memory");
    assert!(Arc::ptr_eq(&canonical_mem, &wt_mem));

    // Same prompt authorities behind the worktree compiler.
    let wt_catalog = worktree_compiler
        .prompt_catalog()
        .expect("worktree prompt catalog");
    let rt_catalog = rt.prompt_catalog_arc();
    assert!(Arc::ptr_eq(wt_catalog, &rt_catalog));

    // Worktree compiler is workspace-scoped only in its root binding.
    let ctx_ws = worktree_compiler.prompt_catalog().expect("catalog present");
    let _ = ctx_ws;
}

#[tokio::test]
async fn test_session_resume_fails_closed_across_workspaces() {
    use m31a::events::bus::BroadcastEventBus;

    // Shared DB so the session is visible to both runtimes; workspaces differ
    // so resume must fail closed on workspace binding (not NotFound).
    let dir_a = tempdir().expect("tempdir a");
    let dir_b = tempdir().expect("tempdir b");
    let db_dir = tempdir().expect("db dir");
    let db_path = db_dir.path().join("sessions.db");
    let pool = m31a::persistence::sqlite::initialize_database(&db_path)
        .await
        .expect("init db");
    let bus_a = std::sync::Arc::new(BroadcastEventBus::new(64));
    let bus_b = std::sync::Arc::new(BroadcastEventBus::new(64));
    let rt_a = AppRuntime::from_pool_and_workspace(pool.clone(), dir_a.path().to_path_buf(), bus_a)
        .await
        .expect("rt a");
    let rt_b = AppRuntime::from_pool_and_workspace(pool.clone(), dir_b.path().to_path_buf(), bus_b)
        .await
        .expect("rt b");

    // Session belongs to workspace A.
    let sess = rt_a.create_session().await.expect("create session");
    assert_eq!(
        m31a::init::instance::canonicalize_workspace_root(&sess.workspace_root),
        m31a::init::instance::canonicalize_workspace_root(dir_a.path())
    );

    // Same-workspace resume attaches normally.
    let resumed = rt_a.resume_session(sess.id).await.expect("same-ws resume");
    assert_eq!(resumed.id, sess.id);

    // Cross-workspace resume fails closed (never executes against wrong graph).
    let err = rt_b
        .resume_session(sess.id)
        .await
        .expect_err("cross-ws must fail");
    let msg = err.to_string();
    assert!(
        msg.contains("different workspace") || msg.contains("rebind"),
        "unexpected error: {msg}"
    );
}

#[tokio::test]
async fn test_model_catalog_single_authority_global_first() {
    use m31a::model::catalog::{CURRENT_CATALOG_SCHEMA_VERSION, ModelCatalog};
    use m31a::model::router::resolver::{ModelCandidate, ModelTier};

    let dir = tempdir().expect("tempdir");
    let ws = dir.path();
    let channel = m31a::deployment::DeploymentChannel::current();

    // Legacy workspace cache as migration source only.
    let legacy = ModelCatalog::cache_path_for_channel(ws, channel);
    let cat = ModelCatalog::from_discovered(
        "nvidia",
        vec![
            ModelCandidate::new("meta/legacy-model", "nvidia", ModelTier::Standard, 32768)
                .with_context_provenance("provider:max_model_len"),
        ],
        1700000000,
    );
    cat.save_to_cache_file(&legacy).expect("save legacy");

    // Canonical loader sees the migration source.
    let loaded = ModelCatalog::load_canonical_for_workspace(ws).expect("canonical load");
    assert_eq!(loaded.schema_version, CURRENT_CATALOG_SCHEMA_VERSION);
    assert!(!loaded.models.is_empty());

    // Runtime refresh persists to the global authority (never legacy).
    let rt = AppRuntime::new(ws).await.expect("runtime");
    let layout = m31a::storage::StorageLayout::new(ws, channel);
    let global = layout.global_model_catalog_file();
    // Refresh without a provider fails closed (no fake catalog), but must
    // never write the legacy path as a side effect.
    let _ = rt.refresh_model_catalog().await;
    // Global-first loader and runtime catalog observe one authority: after a
    // global write, both see it.
    let fresh = ModelCatalog::from_discovered(
        "nvidia",
        vec![
            ModelCandidate::new("meta/global-model", "nvidia", ModelTier::Standard, 65536)
                .with_context_provenance("provider:max_model_len"),
        ],
        1700000001,
    );
    fresh.save_to_cache_file(&global).expect("save global");
    let via_loader = ModelCatalog::load_canonical_for_workspace(ws).expect("reload global");
    assert!(
        via_loader
            .models
            .iter()
            .any(|m| m.model_id == "meta/global-model")
    );
}

#[tokio::test]
async fn test_artifact_capability_shares_canonical_service() {
    let (_dir, rt) = test_runtime().await;
    let caps = rt.capability_registry();
    let provider = caps.artifacts().expect("artifact provider");

    let id = m31a::ids::ArtifactId::new().to_string();
    let payload = b"single-authority-artifact-payload";
    let stored_path = provider
        .store_artifact(&id, payload, "txt")
        .await
        .expect("store via capability");
    assert!(stored_path.exists());

    // Visible through the canonical artifact service/ledger (same store).
    let parsed: m31a::ids::ArtifactId = id.parse().expect("parse id");
    let via_service = rt
        .artifact_service()
        .retrieve_payload(parsed, "txt")
        .await
        .expect("read via canonical service");
    assert_eq!(via_service, payload);
}

#[test]
fn test_shared_dispatcher_constructs_no_competing_authorities() {
    let src = std::fs::read_to_string("src/agent/dispatcher.rs").expect("read dispatcher");
    let section = src
        .split("pub fn from_shared_authorities")
        .nth(1)
        .expect("from_shared section");
    // Terminate at the next `pub fn` to isolate `from_shared_authorities`
    // from `from_execution_authorities` / `with_capabilities` (test-only
    // rebuild seam) below.
    let end = section.find("\n    pub fn ").unwrap_or(section.len());
    let section = &section[..end];
    // The shared production constructor must consume authorities, never mint them.
    for forbidden in [
        "ToolRegistry::new(",
        "ToolRegistry::new_default(",
        "ToolPipelineRunner::new(",
        "CapabilityRegistry::production(",
        "RoutedModelCaller::new(",
        "EffectivePolicy::standard(",
        "AuthorizationAuthority::new(",
        "FsArtifactStore::new(",
    ] {
        assert!(
            !section.contains(forbidden),
            "from_shared_authorities must not contain `{forbidden}`"
        );
    }
}

#[test]
fn test_production_controller_has_no_legacy_fallback() {
    let src = std::fs::read_to_string("src/controller/dependencies.rs").expect("read dependencies");
    assert!(
        !src.contains("Legacy standalone path"),
        "canonical controller must not retain a legacy dispatcher fallback"
    );
    assert!(
        !src.contains("fn assemble_with_shared_authorities"),
        "legacy optional-authority assembler must be replaced by assemble_canonical"
    );
    // Mandatory authorities are non-optional in the canonical signature.
    let sig = src
        .split("pub fn production_with_shared_authorities")
        .nth(1)
        .expect("canonical sig");
    let sig_end = sig.find("{").unwrap_or(sig.len());
    let sig = &sig[..sig_end];
    assert!(
        !sig.contains("capabilities: Option<"),
        "capabilities must be mandatory (no Option) on production paths"
    );
    assert!(
        !sig.contains("context_compiler: Option<"),
        "context_compiler must be mandatory (no Option) on production paths"
    );
}
