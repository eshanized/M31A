//! Persistent Workspace-Instance Lifecycle — canonical startup authority.
//!
//! OpenCode-style instance semantics, implemented idiomatically in Rust:
//!
//! - a workspace instance is deterministically bound to its canonical
//!   workspace root (never a fresh identity per process);
//! - the instance is resolved (loaded or created) once per process and cached
//!   for the process lifetime;
//! - initialization state is durable (`.m31a/init.json` bootstrap sentinel +
//!   canonical SQLite `system_state` record) and survives process restarts;
//! - the onboarding decision depends ONLY on persistent workspace state,
//!   never on "a new process just started".
//!
//! Startup authority chain:
//!
//! ```text
//! resolve_workspace_instance()
//!   -> StartupDecision::{Initialized | NeedsOnboarding}
//!     -> optional first-run onboarding (exactly once)
//!       -> persist_successful_onboarding()
//!         -> refresh/reload instance state
//!           -> AppRuntime against that instance
//!             -> CLI/TUI projections (receive resolved state; never re-check)
//! ```
//!
//! All production startup paths MUST go through [`resolve_startup`]. Direct
//! `InitManager::new` + `is_onboarded` checks in startup code are a
//! regression (see `tests/workspace_instance_lifecycle.rs`).

use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

use super::lifecycle::{InitError, InitManager, InitState};

/// Durable key in SQLite `system_state` holding the canonical init state.
pub const DB_KEY_INIT_STATE: &str = "init_state";
/// Durable key in SQLite `system_state` binding the database to its workspace.
pub const DB_KEY_WORKSPACE_ROOT: &str = "workspace_root";

/// Lexically normalize a path without touching the filesystem.
///
/// Resolves `.`, `..`, and duplicate separators. Does not resolve symlinks;
/// prefer [`canonicalize_workspace_root`] which uses the filesystem when the
/// path exists.
fn normalize_lexically(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    if out.as_os_str().is_empty() {
        PathBuf::from(".")
    } else {
        out
    }
}

/// Resolve the canonical workspace root for a possibly-relative input.
///
/// Canonicalization rules (stable across invocations):
/// 1. relative inputs are joined onto the current working directory;
/// 2. the result is lexically normalized;
/// 3. when the path exists, `std::fs::canonicalize` resolves symlinks so
///    aliased paths (e.g. `/tmp` vs `/private/tmp`) map to one identity.
///
/// The sentinel path and instance id derive from this value, so wizard and
/// startup always address the SAME workspace even when invoked with
/// different spellings (`--workspace .` vs absolute path vs bare `m31a`).
pub fn canonicalize_workspace_root(input: &Path) -> PathBuf {
    let absolute = if input.is_absolute() {
        input.to_path_buf()
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(input)
    };
    let normalized = normalize_lexically(&absolute);
    std::fs::canonicalize(&normalized).unwrap_or(normalized)
}

/// Deterministic instance id for a canonical workspace root.
///
/// Stable across processes; never regenerated per launch. Hex-encoded
/// SHA-256 of the canonical root path (truncated to 16 bytes / 32 hex chars
/// for readability — collision resistance remains ample for local use).
pub fn workspace_instance_id(canonical_root: &Path) -> String {
    let mut hasher = Sha256::new();
    hasher.update(canonical_root.as_os_str().as_encoded_bytes());
    let digest = hasher.finalize();
    digest
        .iter()
        .take(16)
        .map(|b| format!("{b:02x}"))
        .collect::<String>()
}

/// Why a workspace requires first-run onboarding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OnboardingReason {
    /// No durable state exists for this workspace at all.
    FreshWorkspace,
    /// A previous run left a non-terminal state (Checking / Configuring /
    /// Verifying) without completing; onboarding must (re-)run.
    IncompleteFlow { state: InitState },
}

/// Persistent workspace instance: the canonical unit of startup.
///
/// Uniquely and deterministically associated with its workspace root.
/// Reconstructed from durable state on every process start; onboarding runs
/// only when durable state says the workspace is not initialized.
#[derive(Debug, Clone)]
pub struct WorkspaceInstance {
    canonical_root: PathBuf,
    data_dir: PathBuf,
    instance_id: String,
    state: InitState,
    sentinel_present: bool,
    db_initialized: bool,
}

impl WorkspaceInstance {
    /// Canonical (absolute, normalized) workspace root.
    pub fn workspace_root(&self) -> &Path {
        &self.canonical_root
    }

    /// Project-local state directory (`<root>/.m31a`).
    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    /// Deterministic instance id derived from the canonical root.
    pub fn instance_id(&self) -> &str {
        &self.instance_id
    }

    /// Current durable initialization state.
    pub fn state(&self) -> &InitState {
        &self.state
    }

    /// Whether the bootstrap sentinel file exists on disk.
    pub fn sentinel_present(&self) -> bool {
        self.sentinel_present
    }

    /// Whether the canonical SQLite record marks this workspace initialized.
    pub fn db_initialized(&self) -> bool {
        self.db_initialized
    }

    /// Canonical initialized predicate: `Ready` or `Onboarded`.
    pub fn is_initialized(&self) -> bool {
        matches!(self.state, InitState::Ready | InitState::Onboarded)
    }

    /// Strict onboarded predicate (see `InitManager::is_onboarded`).
    pub fn is_onboarded(&self) -> bool {
        self.state == InitState::Onboarded
    }

    /// Whether first-run onboarding must run for this workspace.
    pub fn needs_onboarding(&self) -> bool {
        !self.is_initialized()
    }

    /// Why onboarding is required (`None` when initialized).
    pub fn onboarding_reason(&self) -> Option<OnboardingReason> {
        if self.is_initialized() {
            return None;
        }
        if self.state == InitState::Uninitialized {
            Some(OnboardingReason::FreshWorkspace)
        } else {
            Some(OnboardingReason::IncompleteFlow {
                state: self.state.clone(),
            })
        }
    }
}

/// Canonical startup decision for a workspace.
#[derive(Debug, Clone)]
pub enum StartupDecision {
    /// Workspace is initialized: load existing state and skip onboarding.
    Initialized(WorkspaceInstance),
    /// Workspace is not initialized: run first-run onboarding exactly once.
    NeedsOnboarding {
        instance: WorkspaceInstance,
        reason: OnboardingReason,
    },
}

impl StartupDecision {
    /// Borrow the underlying instance regardless of variant.
    pub fn instance(&self) -> &WorkspaceInstance {
        match self {
            Self::Initialized(i) => i,
            Self::NeedsOnboarding { instance, .. } => instance,
        }
    }

    /// Whether onboarding is required.
    pub fn needs_onboarding(&self) -> bool {
        matches!(self, Self::NeedsOnboarding { .. })
    }
}

fn db_state_is_initialized(value: Option<&str>) -> bool {
    matches!(value, Some("Onboarded") | Some("Ready"))
}

/// Resolve (load or create) the persistent workspace instance.
///
/// Semantics (OpenCode `instance-runtime` equivalent):
/// - existing durable state -> load/reuse, heal the missing side when the
///   two authorities disagree in a reconcilable direction;
/// - no durable state -> fresh uninitialized instance (onboarding required).
///
/// Healing is deterministic and never fabricates readiness:
/// - sentinel initialized + database empty -> re-record database from sentinel;
/// - database initialized + sentinel missing/mid-flow -> restore sentinel
///   from the database record (the only path that writes `Onboarded`
///   without walking the wizard);
/// - corrupt sentinel / unsupported version / identity mismatch -> typed
///   fail-closed error, never a silent wizard launch.
pub async fn resolve_workspace_instance(
    workspace_root: &Path,
    pool: &crate::persistence::sqlite::SqlitePool,
) -> Result<WorkspaceInstance, InitError> {
    let canonical_root = canonicalize_workspace_root(workspace_root);
    let data_dir = canonical_root.join(InitManager::SENTINEL_DIR);
    std::fs::create_dir_all(&data_dir)?;
    let instance_id = workspace_instance_id(&canonical_root);

    // 1. Bootstrap sentinel (fail-closed on corruption/version skew).
    // The sentinel stays workspace-local (`<ws>/.m31a/init.json`) — it is
    // workspace identity, not application state.
    let mut manager = InitManager::new(&canonical_root)?;
    let sentinel_present = manager.is_sentinel_present();
    let sentinel_initialized = manager.is_initialized();

    // 2. Canonical database record.
    //
    // The application database is GLOBAL (platform user data, shared across
    // workspaces). Per-workspace init state is therefore namespaced by
    // instance id (`init_state:<id>`), with fallback to the legacy
    // un-namespaced keys for pre-migration single-workspace databases.
    let repo =
        crate::persistence::sqlite::repositories::SqliteSystemStateRepository::new(pool.clone());
    let namespaced_init_key = format!("{DB_KEY_INIT_STATE}:{instance_id}");
    let namespaced_root_key = format!("{DB_KEY_WORKSPACE_ROOT}:{instance_id}");
    let db_init_value = repo
        .get(&namespaced_init_key)
        .await
        .map_err(|e| InitError::Database(e.to_string()))?;
    // Synchronous fallback needs the legacy values; fetch them when the
    // namespaced key is absent.
    let db_init_value = match db_init_value {
        Some(v) => Some(v),
        None => {
            let legacy_init = repo
                .get(DB_KEY_INIT_STATE)
                .await
                .map_err(|e| InitError::Database(e.to_string()))?;
            let legacy_root = repo
                .get(DB_KEY_WORKSPACE_ROOT)
                .await
                .map_err(|e| InitError::Database(e.to_string()))?;
            let canonical_root_str = canonical_root.display().to_string();
            match (legacy_init, legacy_root) {
                (Some(init), Some(root)) if root == canonical_root_str => {
                    // Adopt legacy single-workspace state into the namespaced
                    // key so future reads are isolated.
                    let _ = repo.set(&namespaced_init_key, &init).await;
                    let _ = repo.set(&namespaced_root_key, &canonical_root_str).await;
                    Some(init)
                }
                (Some(_), Some(other)) if other != canonical_root_str => {
                    // Shared/global DB owned by another workspace: this
                    // workspace is uninitialized here (sentinel decides).
                    None
                }
                (Some(init), None) => {
                    // Legacy DB without root binding: adopt only if the
                    // sentinel agrees this workspace is initialized (else the
                    // DB may belong to another workspace that lost its root).
                    if sentinel_initialized {
                        let _ = repo.set(&namespaced_init_key, &init).await;
                        let _ = repo.set(&namespaced_root_key, &canonical_root_str).await;
                        Some(init)
                    } else {
                        None
                    }
                }
                _ => None,
            }
        }
    };
    let db_initialized = db_state_is_initialized(db_init_value.as_deref());

    // 3. Stable workspace identity: record this workspace's root under its
    // namespaced key. A divergent stored root under the SAME namespaced key
    // means this database was copied from elsewhere and must not be trusted.
    let canonical_root_str = canonical_root.display().to_string();
    match repo
        .get(&namespaced_root_key)
        .await
        .map_err(|e| InitError::Database(e.to_string()))?
    {
        Some(stored) if stored != canonical_root_str => {
            return Err(InitError::InconsistentState(format!(
                "database workspace identity mismatch: stored '{stored}' != current '{canonical_root_str}'"
            )));
        }
        Some(_) => {}
        None => {
            // Only bind when this workspace is (or becomes) initialized;
            // binding every fresh workspace eagerly would claim the global DB.
            if sentinel_initialized || db_initialized {
                repo.set(&namespaced_root_key, &canonical_root_str)
                    .await
                    .map_err(|e| InitError::Database(e.to_string()))?;
            }
        }
    }

    // 4. Reconcile + heal (namespaced for the shared global DB; legacy
    // keys are also maintained for pre-migration single-workspace readers).
    let state = if sentinel_initialized && db_initialized {
        manager.current_state().clone()
    } else if sentinel_initialized && !db_initialized {
        // Database lost or never migrated (e.g. deleted .db): re-record the
        // proven sentinel state instead of re-running onboarding.
        let sentinel_json =
            serde_json::to_string(&manager.sentinel_payload_json()?).map_err(InitError::Json)?;
        let label = match manager.current_state() {
            InitState::Ready => "Ready",
            _ => "Onboarded",
        };
        repo.record_onboarding(&sentinel_json, label)
            .await
            .map_err(|e| InitError::Database(e.to_string()))?;
        let _ = repo.set(&namespaced_init_key, label).await;
        let _ = repo
            .set(
                &format!("onboarding_sentinel:{instance_id}"),
                &sentinel_json,
            )
            .await;
        let _ = repo.set(&namespaced_root_key, &canonical_root_str).await;
        manager.current_state().clone()
    } else if !sentinel_initialized && db_initialized {
        // Sentinel lost or left mid-flow by a cancelled run while the
        // database proves completion: restore the sentinel, skip the wizard.
        manager.restore_persisted_onboarded()?;
        // Ensure the namespaced binding exists for future shared-DB reads.
        let _ = repo.set(&namespaced_root_key, &canonical_root_str).await;
        if db_init_value.as_deref() == Some("Onboarded")
            || db_init_value.as_deref() == Some("Ready")
        {
            let _ = repo
                .set(
                    &namespaced_init_key,
                    db_init_value.as_deref().unwrap_or("Onboarded"),
                )
                .await;
        }
        manager.current_state().clone()
    } else {
        manager.current_state().clone()
    };

    let instance = WorkspaceInstance {
        canonical_root: canonical_root.clone(),
        data_dir,
        instance_id,
        state,
        sentinel_present: sentinel_present || manager.is_initialized(),
        db_initialized: db_initialized || manager.is_initialized(),
    };

    Ok(instance)
}

/// Canonical startup entry: resolve the persistent instance and decide.
///
/// Resolves through the process-lifetime cache, so repeated startup
/// resolution within one process shares a single instance per workspace.
pub async fn resolve_startup(
    workspace_root: &Path,
    pool: &crate::persistence::sqlite::SqlitePool,
) -> Result<StartupDecision, InitError> {
    let shared = resolve_shared_workspace_instance(workspace_root, pool).await?;
    let instance = (*shared).clone();
    if instance.is_initialized() {
        Ok(StartupDecision::Initialized(instance))
    } else {
        let reason = instance
            .onboarding_reason()
            .unwrap_or(OnboardingReason::FreshWorkspace);
        Ok(StartupDecision::NeedsOnboarding { instance, reason })
    }
}

/// Persist a SUCCESSFUL first-run onboarding and return the refreshed instance.
///
/// Fail-closed contract (wizard-cancelled / validation-failed / persistence
/// failure must never produce `Onboarded`):
/// - drives the legal [`InitManager::complete_onboarding_walk`];
/// - migrates the record into canonical SQLite state;
/// - re-resolves the instance so the caller continues with fresh state.
///
/// Returns the re-resolved (initialized) instance.
pub async fn persist_successful_onboarding(
    workspace_root: &Path,
    pool: &crate::persistence::sqlite::SqlitePool,
) -> Result<WorkspaceInstance, InitError> {
    let canonical_root = canonicalize_workspace_root(workspace_root);
    let mut manager = InitManager::new(&canonical_root)?;
    manager.complete_and_migrate_to_db(pool).await?;
    // Mirror into the namespaced keys for the shared global DB.
    let instance_id = workspace_instance_id(&canonical_root);
    let repo =
        crate::persistence::sqlite::repositories::SqliteSystemStateRepository::new(pool.clone());
    let namespaced_init_key = format!("{DB_KEY_INIT_STATE}:{instance_id}");
    let namespaced_root_key = format!("{DB_KEY_WORKSPACE_ROOT}:{instance_id}");
    let _ = repo.set(&namespaced_init_key, "Onboarded").await;
    let _ = repo
        .set(&namespaced_root_key, &canonical_root.display().to_string())
        .await;
    invalidate_instance(&canonical_root);
    resolve_workspace_instance(&canonical_root, pool).await
}

// ---------------------------------------------------------------------------
// Process-lifetime instance cache (OpenCode instance-runtime equivalent).
// ---------------------------------------------------------------------------

static PROCESS_INSTANCES: OnceLock<Mutex<HashMap<PathBuf, Arc<WorkspaceInstance>>>> =
    OnceLock::new();

fn process_registry() -> &'static Mutex<HashMap<PathBuf, Arc<WorkspaceInstance>>> {
    PROCESS_INSTANCES.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Return the cached instance for a workspace, if the process already
/// resolved it.
pub fn cached_instance(workspace_root: &Path) -> Option<Arc<WorkspaceInstance>> {
    let canonical = canonicalize_workspace_root(workspace_root);
    process_registry()
        .lock()
        .ok()
        .and_then(|guard| guard.get(&canonical).cloned())
}

/// Resolve through the process cache: repeated resolution of the same
/// workspace within one process returns the SAME `Arc` (no competing
/// runtime instances) whenever durable state is unchanged.
pub async fn resolve_shared_workspace_instance(
    workspace_root: &Path,
    pool: &crate::persistence::sqlite::SqlitePool,
) -> Result<Arc<WorkspaceInstance>, InitError> {
    let fresh = resolve_workspace_instance(workspace_root, pool).await?;
    if let Ok(guard) = process_registry().lock()
        && let Some(cached) = guard.get(&fresh.canonical_root)
        && cached.state == fresh.state
    {
        return Ok(cached.clone());
    }
    let arc = Arc::new(fresh.clone());
    if let Ok(mut guard) = process_registry().lock() {
        guard.insert(fresh.canonical_root.clone(), arc.clone());
    }
    Ok(arc)
}

/// Explicitly re-enter first-run onboarding for a workspace.
///
/// Operator-requested recovery ONLY (`m31a init --force`). Rewinds BOTH
/// durable authorities — sentinel back to the start of the wizard flow AND
/// the canonical SQLite record to a non-initialized marker — so startup
/// reconciliation cannot "heal" the intentional re-entry away. Returns the
/// refreshed (uninitialized) instance.
pub async fn begin_explicit_reonboarding(
    workspace_root: &Path,
    pool: &crate::persistence::sqlite::SqlitePool,
) -> Result<WorkspaceInstance, InitError> {
    let canonical_root = canonicalize_workspace_root(workspace_root);
    let mut manager = InitManager::new(&canonical_root)?;
    manager.reenter_onboarding()?;
    let repo =
        crate::persistence::sqlite::repositories::SqliteSystemStateRepository::new(pool.clone());
    repo.set(DB_KEY_INIT_STATE, "Configuring")
        .await
        .map_err(|e| InitError::Database(e.to_string()))?;
    let instance_id = workspace_instance_id(&canonical_root);
    let _ = repo
        .set(&format!("{DB_KEY_INIT_STATE}:{instance_id}"), "Configuring")
        .await;
    invalidate_instance(&canonical_root);
    resolve_workspace_instance(&canonical_root, pool).await
}

/// Drop the cached instance (used after onboarding completion refresh).
pub fn invalidate_instance(workspace_root: &Path) {
    let canonical = canonicalize_workspace_root(workspace_root);
    if let Ok(mut guard) = process_registry().lock() {
        guard.remove(&canonical);
    }
}

/// Scoped instance store with its own in-memory cache.
///
/// Production code uses the process-global registry above; tests and
/// embedders that need isolation use this store. Repeated resolution of the
/// same workspace returns the same `Arc` while durable state is unchanged.
#[derive(Debug, Default)]
pub struct WorkspaceInstanceStore {
    cache: Mutex<HashMap<PathBuf, Arc<WorkspaceInstance>>>,
}

impl WorkspaceInstanceStore {
    /// Create an empty store.
    pub fn new() -> Self {
        Self {
            cache: Mutex::new(HashMap::new()),
        }
    }

    /// Resolve the instance for a workspace, reusing the cached `Arc` when
    /// durable state has not changed since the last resolution.
    pub async fn resolve(
        &self,
        workspace_root: &Path,
        pool: &crate::persistence::sqlite::SqlitePool,
    ) -> Result<Arc<WorkspaceInstance>, InitError> {
        let fresh = resolve_workspace_instance(workspace_root, pool).await?;
        let mut guard = self.cache.lock().map_err(|_| {
            InitError::InconsistentState("instance store lock poisoned".to_string())
        })?;
        if let Some(cached) = guard.get(&fresh.canonical_root)
            && cached.state == fresh.state
        {
            return Ok(cached.clone());
        }
        let arc = Arc::new(fresh.clone());
        guard.insert(fresh.canonical_root.clone(), arc.clone());
        Ok(arc)
    }

    /// Drop the cached entry for a workspace.
    pub fn invalidate(&self, workspace_root: &Path) {
        let canonical = canonicalize_workspace_root(workspace_root);
        if let Ok(mut guard) = self.cache.lock() {
            guard.remove(&canonical);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonicalization_is_stable_for_relative_and_absolute_inputs() {
        let cwd = std::env::current_dir().expect("cwd");
        let relative = Path::new(".");
        let absolute = cwd.clone();
        assert_eq!(
            canonicalize_workspace_root(relative),
            canonicalize_workspace_root(&absolute)
        );
    }

    #[test]
    fn instance_id_is_deterministic_per_workspace() {
        let a = PathBuf::from("/tmp/m31a-ws-a");
        let b = PathBuf::from("/tmp/m31a-ws-b");
        assert_eq!(workspace_instance_id(&a), workspace_instance_id(&a));
        assert_ne!(workspace_instance_id(&a), workspace_instance_id(&b));
    }
}
