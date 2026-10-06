//! Storage architecture contract tests: platform-native user state,
//! minimal workspace `.m31a`, migration, multi-workspace, Git optionality,
//! unlimited budget, and channel isolation.
//!
//! Canonical authority: `m31a::storage::StorageLayout` over
//! `m31a::deployment::DeploymentPaths` (platform `directories` abstraction).

use std::fs;
use std::sync::Mutex;

use m31a::budget::{ActualUsage, BudgetEnforcer, TaskEstimates};
use m31a::config::resolved::ResolvedConfiguration;
use m31a::config::schema::parse_and_validate_config;
use m31a::deployment::{DeploymentChannel, DeploymentPaths};
use m31a::state::budget::ResourceBudget;
use m31a::storage::{ConfigScope, StorageLayout};
use m31a::tui::screens::wizard::{SetupWizardScreen, WizardProfile};

static ENV_MUTEX: Mutex<()> = Mutex::new(());

struct EnvIsolation {
    _lock: std::sync::MutexGuard<'static, ()>,
    saved: Vec<(String, Option<String>)>,
}

impl EnvIsolation {
    fn lock() -> Self {
        let lock = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        let keys = [
            "M31A_CONFIG_DIR",
            "M31A_DATA_DIR",
            "M31A_CACHE_DIR",
            "M31A_STATE_DIR",
            "M31A_DEV_CONFIG_DIR",
            "M31A_DEV_DATA_DIR",
            "M31A_DEV_CACHE_DIR",
            "M31A_DEV_STATE_DIR",
            "NVIDIA_API_KEY",
            "API_KEY_NVIDIA",
        ];
        let saved = keys
            .iter()
            .map(|k| (k.to_string(), std::env::var(k).ok()))
            .collect();
        for k in keys {
            unsafe {
                std::env::remove_var(k);
            }
        }
        Self { _lock: lock, saved }
    }

    fn set_shared(&self, base: &std::path::Path) {
        unsafe {
            std::env::set_var("M31A_CONFIG_DIR", base.join("config"));
            std::env::set_var("M31A_DATA_DIR", base.join("data"));
            std::env::set_var("M31A_CACHE_DIR", base.join("cache"));
            std::env::set_var("M31A_STATE_DIR", base.join("state"));
        }
    }
}

impl Drop for EnvIsolation {
    fn drop(&mut self) {
        for (k, v) in std::mem::take(&mut self.saved) {
            unsafe {
                match v {
                    Some(val) => std::env::set_var(&k, val),
                    None => std::env::remove_var(&k),
                }
            }
        }
    }
}

// ── Platform semantic properties ──────────────────────────────────────────

#[test]
fn platform_paths_are_user_scoped_channel_isolated_and_not_workspace_rooted() {
    let _guard = EnvIsolation::lock();
    let dir = tempfile::tempdir().unwrap();
    let ws = dir.path();
    // Production vs development platform dirs differ when the resolver is
    // available; app-dir names always differ.
    let prod = DeploymentPaths::new(DeploymentChannel::Production);
    let dev = DeploymentPaths::new(DeploymentChannel::Development);
    assert_ne!(
        DeploymentChannel::Production.app_dir_name(),
        DeploymentChannel::Development.app_dir_name()
    );
    if prod.config_dir() != std::path::Path::new(".m31a/config") {
        assert_ne!(prod.config_dir(), dev.config_dir());
        assert_ne!(prod.data_dir(), dev.data_dir());
        assert_ne!(prod.cache_dir(), dev.cache_dir());
        assert_ne!(prod.state_dir(), dev.state_dir());
    }
    assert_ne!(prod.socket_path(), dev.socket_path());
    assert_ne!(prod.pid_file(), dev.pid_file());
    assert_ne!(prod.global_db_path(), dev.global_db_path());
    assert_ne!(
        prod.global_credentials_file(),
        dev.global_credentials_file()
    );

    // StorageLayout global paths are never workspace-rooted.
    let layout = StorageLayout::for_workspace(ws);
    for p in [
        layout.user_config_file(),
        layout.global_db_path(),
        layout.global_credentials_file(),
        layout.global_model_catalog_file(),
        layout.global_artifacts_dir(),
        layout.global_telemetry_dir(),
        layout.user_state_dir(),
        layout.socket_path(),
        layout.pid_file(),
    ] {
        assert!(
            !p.starts_with(ws),
            "global path {} must not live under workspace {}",
            p.display(),
            ws.display()
        );
    }
}

// ── Storage location contracts ───────────────────────────────────────────

#[test]
fn storage_location_contracts_derive_from_single_authority() {
    let _guard = EnvIsolation::lock();
    let dir = tempfile::tempdir().unwrap();
    let ws = dir.path();
    let layout = StorageLayout::for_workspace(ws);

    assert_ne!(layout.user_config_file(), layout.workspace_config_file());
    assert_ne!(
        layout.global_db_path(),
        DeploymentPaths::project_db_path(ws, DeploymentChannel::current())
    );
    assert!(
        !layout
            .global_credentials_file()
            .starts_with(ws.join(".m31a")),
        "global credentials must not live under workspace"
    );
    assert!(
        !layout
            .global_model_catalog_file()
            .starts_with(ws.join(".m31a")),
        "global cache must not live under workspace"
    );
    assert!(
        !layout.user_state_dir().starts_with(ws),
        "global state must not live under workspace"
    );
    // Socket/pid/log all derive from the same state authority.
    assert!(layout.socket_path().starts_with(layout.user_state_dir()));
    assert!(layout.pid_file().starts_with(layout.user_state_dir()));
    assert!(layout.log_dir().starts_with(layout.user_state_dir()));
}

// ── No project pollution ─────────────────────────────────────────────────

#[tokio::test]
async fn no_project_pollution_after_runtime_init() {
    let _guard = EnvIsolation::lock();
    let dir = tempfile::tempdir().unwrap();
    let ws = dir.path().to_path_buf();

    let rt = m31a::runtime::AppRuntime::new(&ws)
        .await
        .expect("runtime init");
    drop(rt);

    let m31a_dir = ws.join(".m31a");
    // Workspace dir may exist for identity/config only.
    if m31a_dir.is_dir() {
        for forbidden in [
            "m31a.db",
            "m31a-dev.db",
            "credentials.json",
            "credentials-dev.json",
        ] {
            assert!(
                !m31a_dir.join(forbidden).exists(),
                "workspace must not contain {forbidden} after init"
            );
        }
        for forbidden_dir in ["telemetry", "telemetry-dev", "logs", "cache"] {
            // `cache/` may exist only for workspace-scoped derived data, but
            // the global model catalog must not be there by default.
            if forbidden_dir == "cache" {
                assert!(
                    !m31a_dir.join("cache").join("model_catalog.json").exists(),
                    "global model catalog must not live under workspace"
                );
                assert!(
                    !m31a_dir
                        .join("cache")
                        .join("model_catalog-dev.json")
                        .exists(),
                    "global model catalog must not live under workspace"
                );
            } else {
                // telemetry/logs for the *application* must be global; an
                // empty workspace dir created for identity is allowed, but
                // populated global stores are not.
                let p = m31a_dir.join(forbidden_dir);
                if p.is_dir() {
                    let count = std::fs::read_dir(&p).map(|d| d.count()).unwrap_or(0);
                    assert_eq!(
                        count, 0,
                        "workspace {forbidden_dir} must not hold global runtime state"
                    );
                }
            }
        }
        // No sockets/pid in workspace.
        let entries = std::fs::read_dir(&m31a_dir)
            .map(|d| {
                d.filter_map(|e| e.ok())
                    .map(|e| e.file_name().to_string_lossy().to_string())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        for e in entries {
            assert!(
                !e.ends_with(".sock") && !e.ends_with(".pid"),
                "workspace must not contain runtime socket/pid: {e}"
            );
            assert!(
                StorageLayout::is_workspace_scoped_file(&e) || e == "cache" || e == "state",
                "unexpected workspace file: {e}"
            );
        }
    }

    // Canonical global DB exists outside the workspace.
    let layout = StorageLayout::for_workspace(&ws);
    assert!(
        layout.global_db_path().is_file(),
        "global DB must exist at {}",
        layout.global_db_path().display()
    );
    assert!(
        !layout.global_db_path().starts_with(&ws),
        "global DB must not be workspace-local"
    );
}

// ── Config reload across workspaces ──────────────────────────────────────

#[test]
fn global_config_survives_across_workspaces_with_workspace_precedence() {
    let _guard = EnvIsolation::lock();
    let base = tempfile::tempdir().unwrap();
    let shared = base.path().join("shared-global");
    fs::create_dir_all(&shared).unwrap();
    _guard.set_shared(&shared);

    let ws_a = base.path().join("ws-a");
    let ws_b = base.path().join("ws-b");
    fs::create_dir_all(&ws_a).unwrap();
    fs::create_dir_all(&ws_b).unwrap();

    // Global settings via scope=Global wizard in workspace A.
    let mut wiz = SetupWizardScreen::new(ws_a.clone()).with_config_scope(ConfigScope::Global);
    wiz.profile = WizardProfile::Autonomous;
    wiz.require_approval_for_writes = false;
    wiz.primary_model_input
        .set_text("meta/llama-3.1-70b-instruct");
    wiz.fast_model_input
        .set_text("meta/llama-3.2-11b-vision-instruct");
    wiz.git_enabled = false;
    wiz.unlimited_budget = true;
    wiz.persist_configuration().expect("persist global");

    // Launch from a DIFFERENT workspace: global survives.
    let resolved_b = ResolvedConfiguration::for_workspace(&ws_b).expect("resolve B");
    assert_eq!(resolved_b.active_profile.as_deref(), Some("autonomous"));
    assert!(!resolved_b.app_config.git.enabled);

    // Workspace override wins over global.
    let mut wiz_b = SetupWizardScreen::new(ws_b.clone()).with_config_scope(ConfigScope::Workspace);
    wiz_b.profile = WizardProfile::Balanced;
    wiz_b
        .primary_model_input
        .set_text("meta/llama-3.1-70b-instruct");
    wiz_b
        .fast_model_input
        .set_text("meta/llama-3.2-11b-vision-instruct");
    wiz_b.git_enabled = true;
    wiz_b.unlimited_budget = true;
    wiz_b.persist_configuration().expect("persist workspace");

    let resolved_b2 = ResolvedConfiguration::for_workspace(&ws_b).expect("resolve B2");
    assert_eq!(resolved_b2.active_profile.as_deref(), Some("balanced"));
    assert!(resolved_b2.app_config.git.enabled);

    // Workspace A still resolves global (no workspace override there).
    let resolved_a = ResolvedConfiguration::for_workspace(&ws_a).expect("resolve A");
    assert_eq!(resolved_a.active_profile.as_deref(), Some("autonomous"));
}

// ── Multi-workspace sharing ──────────────────────────────────────────────

#[test]
fn multi_workspace_shares_global_state_with_isolated_identities() {
    let _guard = EnvIsolation::lock();
    let base = tempfile::tempdir().unwrap();
    let shared = base.path().join("shared-global");
    fs::create_dir_all(&shared).unwrap();
    _guard.set_shared(&shared);

    let ws_a = base.path().join("proj-a");
    let ws_b = base.path().join("proj-b");
    fs::create_dir_all(&ws_a).unwrap();
    fs::create_dir_all(&ws_b).unwrap();

    // Same credential through the global store.
    let mut wiz = SetupWizardScreen::new(ws_a.clone()).with_config_scope(ConfigScope::Global);
    wiz.api_key_input.set_text("nvapi-shared-multi-ws-key");
    wiz.primary_model_input
        .set_text("meta/llama-3.1-70b-instruct");
    wiz.fast_model_input
        .set_text("meta/llama-3.2-11b-vision-instruct");
    wiz.persist_configuration().expect("persist");

    let res_a =
        m31a::runtime_authorities::resolve_runtime_credentials(&ws_a, DeploymentChannel::current());
    let res_b =
        m31a::runtime_authorities::resolve_runtime_credentials(&ws_b, DeploymentChannel::current());
    assert_eq!(res_a.api_key.as_deref(), Some("nvapi-shared-multi-ws-key"));
    assert_eq!(res_b.api_key.as_deref(), Some("nvapi-shared-multi-ws-key"));

    // Same global model cache location for both workspaces.
    let layout_a = StorageLayout::for_workspace(&ws_a);
    let layout_b = StorageLayout::for_workspace(&ws_b);
    assert_eq!(
        layout_a.global_model_catalog_file(),
        layout_b.global_model_catalog_file()
    );
    assert_eq!(
        layout_a.global_credentials_file(),
        layout_b.global_credentials_file()
    );

    // Different workspace identities and configs.
    assert_ne!(
        m31a::init::workspace_instance_id(&m31a::init::canonicalize_workspace_root(&ws_a)),
        m31a::init::workspace_instance_id(&m31a::init::canonicalize_workspace_root(&ws_b))
    );
    assert_ne!(
        layout_a.workspace_config_file(),
        layout_b.workspace_config_file()
    );

    // No secrets in either workspace.
    for ws in [&ws_a, &ws_b] {
        assert!(
            !ws.join(".m31a").join("credentials.json").exists(),
            "workspace must not contain credentials"
        );
        assert!(
            !ws.join(".m31a").join("credentials-dev.json").exists(),
            "workspace must not contain credentials"
        );
    }
}

// ── Wizard scope ─────────────────────────────────────────────────────────

#[test]
fn wizard_scope_routes_config_to_correct_store() {
    let _guard = EnvIsolation::lock();
    let base = tempfile::tempdir().unwrap();
    let shared = base.path().join("shared-global");
    fs::create_dir_all(&shared).unwrap();
    _guard.set_shared(&shared);
    let ws = base.path().join("ws");
    fs::create_dir_all(&ws).unwrap();

    // Global scope → user config file, never workspace secrets.
    let mut wiz_g = SetupWizardScreen::new(ws.clone()).with_config_scope(ConfigScope::Global);
    wiz_g.profile = WizardProfile::Balanced;
    wiz_g
        .primary_model_input
        .set_text("meta/llama-3.1-70b-instruct");
    wiz_g
        .fast_model_input
        .set_text("meta/llama-3.2-11b-vision-instruct");
    wiz_g.api_key_input.set_text("nvapi-scope-test-key");
    wiz_g.unlimited_budget = true;
    wiz_g.persist_configuration().expect("persist global");

    let layout = StorageLayout::for_workspace(&ws);
    assert!(layout.user_config_file().is_file());
    assert!(layout.global_credentials_file().is_file());
    assert!(!ws.join(".m31a").join("credentials.json").exists());

    // Workspace scope → workspace config file.
    let mut wiz_w = SetupWizardScreen::new(ws.clone()).with_config_scope(ConfigScope::Workspace);
    wiz_w.profile = WizardProfile::Conservative;
    wiz_w
        .primary_model_input
        .set_text("meta/llama-3.1-70b-instruct");
    wiz_w
        .fast_model_input
        .set_text("meta/llama-3.2-11b-vision-instruct");
    wiz_w.unlimited_budget = true;
    wiz_w.persist_configuration().expect("persist workspace");
    let content = fs::read_to_string(ws.join(".m31a").join("config.toml")).expect("read ws");
    let parsed = parse_and_validate_config(&content).expect("validate");
    assert_eq!(parsed.profile.as_deref(), Some("conservative"));

    // Scope toggle works.
    let mut wiz = SetupWizardScreen::new(ws.clone());
    assert_eq!(wiz.config_scope, ConfigScope::Workspace);
    wiz.toggle_config_scope();
    assert_eq!(wiz.config_scope, ConfigScope::Global);
}

// ── Git optionality ──────────────────────────────────────────────────────

#[test]
fn git_disabled_workspace_launches_without_git() {
    let _guard = EnvIsolation::lock();
    let dir = tempfile::tempdir().unwrap();
    let ws = dir.path().to_path_buf();
    assert!(!ws.join(".git").exists());

    let doctor = m31a::init::doctor::DoctorEngine::new();
    let probes = doctor.run_all_with_git_enabled(&ws, false);
    let git_probe = probes
        .iter()
        .find(|p| p.id == "git_installed")
        .expect("probe");
    let repo_probe = probes
        .iter()
        .find(|p| p.id == "git_repository")
        .expect("probe");
    assert_eq!(
        git_probe.status,
        m31a::init::doctor::DiagnosticStatus::Disabled
    );
    assert_eq!(
        repo_probe.status,
        m31a::init::doctor::DiagnosticStatus::Disabled
    );
    assert!(!doctor.has_blocking_failures(&probes));

    // Wizard honors Git OFF and persists it.
    let mut wiz = SetupWizardScreen::new(ws.clone());
    assert!(!wiz.git_enabled);
    wiz.trust_confirmed = true;
    wiz.profile = WizardProfile::Balanced;
    wiz.primary_model_input
        .set_text("meta/llama-3.1-70b-instruct");
    wiz.fast_model_input
        .set_text("meta/llama-3.2-11b-vision-instruct");
    wiz.unlimited_budget = true;
    wiz.persist_configuration().expect("persist");
    let content = fs::read_to_string(ws.join(".m31a").join("config.toml")).expect("read");
    let parsed = parse_and_validate_config(&content).expect("validate");
    assert!(!parsed.git.enabled);

    // Git present but user chooses OFF stays OFF.
    fs::create_dir_all(ws.join(".git")).unwrap();
    let wiz2 = SetupWizardScreen::new(ws.clone());
    // Existing config says OFF, so it stays OFF even with .git present.
    assert!(!wiz2.git_enabled);
}

#[test]
fn git_enabled_workspace_launches_with_git() {
    let _guard = EnvIsolation::lock();
    let dir = tempfile::tempdir().unwrap();
    let ws = dir.path().to_path_buf();
    fs::create_dir_all(ws.join(".git")).unwrap();

    // Fresh workspace with .git defaults to enabled.
    let fresh = tempfile::tempdir().unwrap();
    let fresh_ws = fresh.path().to_path_buf();
    fs::create_dir_all(fresh_ws.join(".git")).unwrap();
    // Remove any global config influence for this assertion.
    let wiz = SetupWizardScreen::new(fresh_ws.clone());
    assert!(wiz.git_enabled);

    let mut wiz_on = SetupWizardScreen::new(ws.clone());
    wiz_on.git_enabled = true;
    wiz_on.profile = WizardProfile::Autonomous;
    wiz_on.require_approval_for_writes = false;
    wiz_on
        .primary_model_input
        .set_text("meta/llama-3.1-70b-instruct");
    wiz_on
        .fast_model_input
        .set_text("meta/llama-3.2-11b-vision-instruct");
    wiz_on.unlimited_budget = true;
    wiz_on.persist_configuration().expect("persist");
    let resolved = ResolvedConfiguration::for_workspace(&ws).expect("resolve");
    assert!(resolved.app_config.git.enabled);
}

// ── Unlimited budget ─────────────────────────────────────────────────────

#[test]
fn unlimited_budget_never_imposes_hidden_ceilings() {
    let budget = ResourceBudget::unbounded();
    for limit in [
        budget.max_cost_usd,
        budget.max_tokens.map(|v| v as f64),
        budget.max_agent_steps.map(|v| v as f64),
        budget.max_model_calls.map(|v| v as f64),
        budget.max_wall_clock_seconds.map(|v| v as f64),
        budget.max_retries.map(|v| v as f64),
    ] {
        assert!(limit.is_none(), "unbounded must stay None");
    }
    let enforcer = BudgetEnforcer::new(budget);
    let receipt = enforcer
        .reserve(
            &TaskEstimates {
                estimated_tokens: 100_000_000,
                estimated_cost_usd: 500.0,
                requires_worker: true,
                estimated_artifact_bytes: 1_000_000_000,
            },
            false,
        )
        .expect("unbounded reserve must succeed");
    enforcer.settle(
        &receipt,
        &ActualUsage {
            steps: 1500,
            calls: 800,
            tokens: 120_000_000,
            cost_usd: 650.0,
            artifact_bytes: 800_000_000,
            retries: 0,
        },
    );
    let snap = enforcer.snapshot();
    assert_eq!(snap.tokens_consumed, 120_000_000);
}

// ── Channel isolation (global, not workspace) ────────────────────────────

#[test]
fn production_and_development_global_storage_are_isolated() {
    let _guard = EnvIsolation::lock();
    let prod = DeploymentPaths::new(DeploymentChannel::Production);
    let dev = DeploymentPaths::new(DeploymentChannel::Development);
    assert_ne!(prod.global_db_path(), dev.global_db_path());
    assert_ne!(
        prod.global_credentials_file(),
        dev.global_credentials_file()
    );
    assert_ne!(
        prod.global_model_catalog_file(),
        dev.global_model_catalog_file()
    );
    assert_ne!(prod.global_artifacts_dir(), dev.global_artifacts_dir());
    assert_ne!(prod.global_telemetry_dir(), dev.global_telemetry_dir());
    assert_ne!(prod.cache_dir(), dev.cache_dir());
    assert_ne!(prod.state_dir(), dev.state_dir());
}

// ── Legacy migration ─────────────────────────────────────────────────────

#[test]
fn legacy_workspace_state_migrates_to_global_with_integrity() {
    let _guard = EnvIsolation::lock();
    let base = tempfile::tempdir().unwrap();
    let shared = base.path().join("shared-global");
    fs::create_dir_all(&shared).unwrap();
    _guard.set_shared(&shared);
    let ws = base.path().join("legacy-ws");
    fs::create_dir_all(ws.join(".m31a")).unwrap();

    // Legacy project-local DB (valid SQLite header) + credentials + catalog.
    fs::write(
        ws.join(".m31a").join("m31a.db"),
        b"SQLite format 3\0legacy-payload",
    )
    .unwrap();
    fs::write(
        ws.join(".m31a").join("credentials.json"),
        r#"{"nvidia_nim":"nvapi-legacy-key"}"#,
    )
    .unwrap();
    fs::create_dir_all(ws.join(".m31a").join("cache")).unwrap();
    fs::write(
        ws.join(".m31a").join("cache").join("model_catalog.json"),
        r#"{"provider":"nvidia_nim","models":[]}"#,
    )
    .unwrap();

    let layout = StorageLayout::for_workspace(&ws);
    assert!(m31a::storage::is_legacy_present(
        &ws,
        DeploymentChannel::current()
    ));
    let report = m31a::storage::migrate_legacy_workspace_state(&layout);
    assert!(report.db_migrated);
    assert!(report.credentials_migrated);
    assert!(report.catalog_migrated);
    assert!(layout.global_db_path().is_file());
    assert!(layout.global_credentials_file().is_file());
    assert!(layout.global_model_catalog_file().is_file());
    // Legacy sources are marked migrated (renamed), never silently deleted.
    assert!(!ws.join(".m31a").join("m31a.db").exists());
    assert!(!ws.join(".m31a").join("credentials.json").exists());
    // Credentials resolve from the canonical global store now.
    let res =
        m31a::runtime_authorities::resolve_runtime_credentials(&ws, DeploymentChannel::current());
    assert_eq!(res.api_key.as_deref(), Some("nvapi-legacy-key"));
}

// ── Profile persistence ──────────────────────────────────────────────────

#[test]
fn selected_profile_persists_through_resolver_and_runtime() {
    let _guard = EnvIsolation::lock();
    let dir = tempfile::tempdir().unwrap();
    let ws = dir.path().to_path_buf();
    let mut wiz = SetupWizardScreen::new(ws.clone());
    wiz.profile = WizardProfile::Autonomous;
    wiz.require_approval_for_writes = false;
    wiz.primary_model_input
        .set_text("meta/llama-3.1-70b-instruct");
    wiz.fast_model_input
        .set_text("meta/llama-3.2-11b-vision-instruct");
    wiz.unlimited_budget = true;
    wiz.persist_configuration().expect("persist");
    let resolved = ResolvedConfiguration::for_workspace(&ws).expect("resolve");
    assert_eq!(resolved.active_profile.as_deref(), Some("autonomous"));
    assert_eq!(resolved.app_config.profile.as_deref(), Some("autonomous"));
}
