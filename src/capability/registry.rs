//! Central capability registry (CTL-01, CTL-02, CTL-03).

use crate::capability::error::CapabilityError;
use crate::capability::family::CapabilityFamily;
use crate::capability::health::{CapabilityHealthState, CapabilityHealthTracker, HealthConfig};
use crate::capability::instance::CapabilityInstance;
use crate::capability::permissions::{CapabilityOperation, RiskClass};
use crate::capability::traits::*;
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, RwLock};

/// Central registry managing the 15 core capability families, instance descriptors,
/// operational health tracking, and provider service handles (CTL-01, CTL-02, CTL-03).
pub struct CapabilityRegistry {
    instances: RwLock<HashMap<String, CapabilityInstance>>,
    health_trackers: RwLock<HashMap<String, CapabilityHealthTracker>>,
    health_config: HealthConfig,

    // Provider service seams (Arc<dyn ServiceTrait>)
    fs: RwLock<Option<Arc<dyn FileSystemService>>>,
    shell: RwLock<Option<Arc<dyn ShellService>>>,
    process: RwLock<Option<Arc<dyn ProcessService>>>,
    terminal: RwLock<Option<Arc<dyn TerminalService>>>,
    repo: RwLock<Option<Arc<dyn RepositoryService>>>,
    git: RwLock<Option<Arc<dyn GitService>>>,
    web: RwLock<Option<Arc<dyn WebService>>>,
    network: RwLock<Option<Arc<dyn NetworkService>>>,
    jobs: RwLock<Option<Arc<dyn JobService>>>,
    sandbox: RwLock<Option<Arc<dyn SandboxService>>>,
    model: RwLock<Option<Arc<dyn ModelService>>>,
    memory: RwLock<Option<Arc<dyn MemoryService>>>,
    verification: RwLock<Option<Arc<dyn VerificationService>>>,
    artifacts: RwLock<Option<Arc<dyn ArtifactStoreService>>>,
    telemetry: RwLock<Option<Arc<dyn TelemetryService>>>,
    /// Runtime authorization minting authority shared by governed providers
    /// and model-facing tools. The SAME instance must be shared by every
    /// registry in one runtime scope so gates minted in one place verify in
    /// another; forking it would fork authorization trust.
    auth_authority: Arc<crate::git::AuthorizationAuthority>,
}

impl Default for CapabilityRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl CapabilityRegistry {
    /// Create a new empty CapabilityRegistry with default health configuration.
    pub fn new() -> Self {
        Self::with_health_config(HealthConfig::default())
    }

    /// Create a CapabilityRegistry with custom health configuration.
    pub fn with_health_config(health_config: HealthConfig) -> Self {
        Self {
            instances: RwLock::new(HashMap::new()),
            health_trackers: RwLock::new(HashMap::new()),
            health_config,
            fs: RwLock::new(None),
            shell: RwLock::new(None),
            process: RwLock::new(None),
            terminal: RwLock::new(None),
            repo: RwLock::new(None),
            git: RwLock::new(None),
            web: RwLock::new(None),
            network: RwLock::new(None),
            jobs: RwLock::new(None),
            sandbox: RwLock::new(None),
            model: RwLock::new(None),
            memory: RwLock::new(None),
            verification: RwLock::new(None),
            artifacts: RwLock::new(None),
            telemetry: RwLock::new(None),
            auth_authority: Arc::new(crate::git::AuthorizationAuthority::new()),
        }
    }

    /// Construct a fully populated production CapabilityRegistry with real local services (CTL-01, CTL-02, F-02).
    pub fn production(
        workspace_root: impl AsRef<Path>,
        bus: Option<Arc<dyn crate::events::EventBus>>,
        model_caller: Option<Arc<dyn crate::agent::model_policy::ModelCaller>>,
    ) -> Self {
        use crate::capability::permissions::CapabilityPermissions;
        use crate::capability::providers::*;

        let root = workspace_root.as_ref();
        let _ = std::fs::create_dir_all(root);
        let reg = Self::new();

        // 1. Filesystem
        if let Ok(fs_prov) = LocalFileSystemProvider::new(root) {
            reg.register_filesystem(Arc::new(fs_prov));
            reg.register_instance(CapabilityInstance::new(
                "fs.local",
                "Local Workspace Filesystem",
                "1.0.0",
                CapabilityFamily::Filesystem,
                "local_fs",
                CapabilityPermissions::full_access().with_path_scope(root),
            ));
        }

        // 2. Process & Shell
        let proc_prov = Arc::new(LocalProcessProvider::new(root.to_path_buf()));
        reg.register_process(proc_prov.clone());
        reg.register_shell(proc_prov);
        reg.register_instance(CapabilityInstance::new(
            "process.local",
            "Local Process Execution",
            "1.0.0",
            CapabilityFamily::Process,
            "local_process",
            CapabilityPermissions::full_access(),
        ));
        reg.register_instance(CapabilityInstance::new(
            "shell.local",
            "Local Shell Execution",
            "1.0.0",
            CapabilityFamily::Shell,
            "local_process",
            CapabilityPermissions::full_access(),
        ));

        // 3. Jobs (canonical global spool: channel-isolated platform state,
        // never workspace-local; dev runs can never observe production spools).
        let channel = crate::deployment::DeploymentChannel::current();
        let spool_dir = crate::storage::StorageLayout::new(root, channel).job_spool_dir();
        let job_prov = Arc::new(LocalJobProvider::new(
            root.to_path_buf(),
            Arc::new(crate::process::job::JobSupervisor::new(spool_dir)),
        ));
        reg.register_jobs(job_prov);
        reg.register_instance(CapabilityInstance::new(
            "jobs.local",
            "Local Background Jobs",
            "1.0.0",
            CapabilityFamily::Jobs,
            "local_jobs",
            CapabilityPermissions::full_access(),
        ));

        // 4. Git
        let git_prov = Arc::new(CliGitProvider::new(root));
        reg.register_git(git_prov);
        reg.register_instance(CapabilityInstance::new(
            "git.cli",
            "Git Command-Line Interface",
            "1.0.0",
            CapabilityFamily::Git,
            "cli_git",
            CapabilityPermissions::full_access(),
        ));

        // 5. Repository
        let repo_prov = Arc::new(RepositoryGraphProvider::new(root, None));
        reg.register_repository(repo_prov);
        reg.register_instance(CapabilityInstance::new(
            "repo.local",
            "Local Repository Intelligence",
            "1.0.0",
            CapabilityFamily::Repository,
            "repo_graph",
            CapabilityPermissions::full_access(),
        ));

        // 6. Sandbox
        let sandbox_prov = Arc::new(LocalSandboxProvider::new());
        reg.register_sandbox(sandbox_prov);
        reg.register_instance(CapabilityInstance::new(
            "sandbox.local",
            "Local Process Sandbox",
            "1.0.0",
            CapabilityFamily::Sandbox,
            "local_sandbox",
            CapabilityPermissions::full_access(),
        ));

        // 7. Terminal
        let term_prov = Arc::new(LocalTerminalProvider::new());
        reg.register_terminal(term_prov);
        reg.register_instance(CapabilityInstance::new(
            "terminal.local",
            "Local Interactive Terminal",
            "1.0.0",
            CapabilityFamily::Terminal,
            "local_terminal",
            CapabilityPermissions::full_access(),
        ));

        // 8. Memory
        let mem_prov = Arc::new(LocalMemoryProvider::new());
        reg.register_memory(mem_prov);
        reg.register_instance(CapabilityInstance::new(
            "memory.local",
            "Local In-Memory Context",
            "1.0.0",
            CapabilityFamily::Memory,
            "local_memory",
            CapabilityPermissions::full_access(),
        ));

        // 9. Network
        let net_prov = Arc::new(LocalNetworkProvider::new());
        reg.register_network(net_prov);
        reg.register_instance(CapabilityInstance::new(
            "network.local",
            "Local Network Prober",
            "1.0.0",
            CapabilityFamily::Network,
            "local_network",
            CapabilityPermissions::full_access(),
        ));

        // 10. Web
        let web_prov = Arc::new(LocalWebProvider::new());
        reg.register_web(web_prov);
        reg.register_instance(CapabilityInstance::new(
            "web.local",
            "Local Web Client",
            "1.0.0",
            CapabilityFamily::Web,
            "local_web",
            CapabilityPermissions::full_access(),
        ));

        // 11. Verification
        let verif_prov = Arc::new(LocalVerificationProvider::new(root));
        reg.register_verification(verif_prov);
        reg.register_instance(CapabilityInstance::new(
            "verification.local",
            "Local Test & Verification Runner",
            "1.0.0",
            CapabilityFamily::Verification,
            "local_verification",
            CapabilityPermissions::full_access(),
        ));

        // 12. Artifacts (canonical workspace artifact authority:
        // project-specific outputs stay workspace-scoped via StorageLayout).
        let artifacts_dir = crate::storage::StorageLayout::new(
            root,
            crate::deployment::DeploymentChannel::current(),
        )
        .workspace_artifacts_dir();
        let art_prov = Arc::new(FsArtifactStoreProvider::new(artifacts_dir));
        reg.register_artifacts(art_prov);
        reg.register_instance(CapabilityInstance::new(
            "artifacts.local",
            "Local Filesystem Artifact Store",
            "1.0.0",
            CapabilityFamily::Artifacts,
            "fs_artifacts",
            CapabilityPermissions::full_access(),
        ));

        // 13. Model
        if let Some(caller) = model_caller {
            let model_prov = Arc::new(ModelCallerProvider::new(caller, "default"));
            reg.register_model(model_prov);
            reg.register_instance(CapabilityInstance::new(
                "model.caller",
                "Model Reasoning Engine",
                "1.0.0",
                CapabilityFamily::Model,
                "model_caller",
                CapabilityPermissions::full_access(),
            ));
        }

        // 14. Telemetry
        let tel_prov = match bus {
            Some(b) => Arc::new(EventBusProvider::with_bus(b)),
            None => Arc::new(EventBusProvider::new()),
        };
        reg.register_telemetry(tel_prov);
        reg.register_instance(CapabilityInstance::new(
            "telemetry.local",
            "Local Event & Telemetry Bus",
            "1.0.0",
            CapabilityFamily::Telemetry,
            "event_bus",
            CapabilityPermissions::full_access(),
        ));

        reg
    }

    /// Register a capability instance descriptor.
    pub fn register_instance(&self, instance: CapabilityInstance) {
        let mut trackers = self.health_trackers.write().unwrap();
        trackers.entry(instance.id.clone()).or_insert_with(|| {
            CapabilityHealthTracker::with_initial_state(self.health_config.clone(), instance.health)
        });

        let mut instances = self.instances.write().unwrap();
        instances.insert(instance.id.clone(), instance);
    }

    /// Lookup a capability instance by ID.
    pub fn get_instance(&self, id: &str) -> Option<CapabilityInstance> {
        let instances = self.instances.read().unwrap();
        let mut inst = instances.get(id)?.clone();

        // Overlay dynamic health state from tracker
        if let Ok(trackers) = self.health_trackers.write()
            && let Some(tracker) = trackers.get(id)
        {
            inst.health = tracker.peek_state();
        }
        Some(inst)
    }

    /// List all registered capability instances with updated health state.
    pub fn list_capabilities(&self) -> Vec<CapabilityInstance> {
        let instances = self.instances.read().unwrap();
        let trackers = self.health_trackers.read().unwrap();

        instances
            .values()
            .map(|inst| {
                let mut updated = inst.clone();
                if let Some(tracker) = trackers.get(&inst.id) {
                    updated.health = tracker.peek_state();
                }
                updated
            })
            .collect()
    }

    /// Check if a capability family is currently available (at least one registered instance is admissible).
    pub fn is_available(&self, family: &CapabilityFamily) -> bool {
        let instances = self.instances.read().unwrap();
        let trackers = self.health_trackers.read().unwrap();

        instances.values().any(|inst| {
            if &inst.family != family {
                return false;
            }
            if let Some(tracker) = trackers.get(&inst.id) {
                tracker.peek_state().is_admissible()
            } else {
                inst.health.is_admissible()
            }
        })
    }

    /// Record execution success for an instance.
    pub fn record_success(&self, instance_id: &str) {
        if let Ok(mut trackers) = self.health_trackers.write()
            && let Some(tracker) = trackers.get_mut(instance_id)
        {
            tracker.record_success();
        }
    }

    /// Record execution failure for an instance, differentiating infrastructure faults from semantic errors.
    pub fn record_failure(
        &self,
        instance_id: &str,
        is_infra_fault: bool,
        reason: impl Into<String>,
    ) {
        if let Ok(mut trackers) = self.health_trackers.write()
            && let Some(tracker) = trackers.get_mut(instance_id)
        {
            tracker.record_failure(is_infra_fault, reason);
        }
    }

    /// Query dynamic health state for a capability instance.
    pub fn get_health(&self, instance_id: &str) -> Option<CapabilityHealthState> {
        let mut trackers = self.health_trackers.write().ok()?;
        let tracker = trackers.get_mut(instance_id)?;
        Some(tracker.current_state())
    }

    /// Validate that an operation, path, domain, and risk are permitted for an instance and that the instance is healthy.
    pub fn validate_access(
        &self,
        instance_id: &str,
        op: CapabilityOperation,
        path: Option<&Path>,
        domain: Option<&str>,
        risk: Option<RiskClass>,
    ) -> Result<(), CapabilityError> {
        let instance = self.get_instance(instance_id).ok_or_else(|| {
            CapabilityError::NotFound(format!(
                "capability instance '{instance_id}' not registered"
            ))
        })?;

        if !instance.health.is_admissible() {
            return Err(CapabilityError::Unavailable(format!(
                "capability '{instance_id}' is currently {:?}",
                instance.health
            )));
        }

        instance.permissions.validate(op, path, domain, risk)
    }

    // --- Provider Handle Registration & Retrieval (CTL-02) ---

    pub fn register_filesystem(&self, provider: Arc<dyn FileSystemService>) {
        *self.fs.write().unwrap() = Some(provider);
    }
    pub fn filesystem(&self) -> Option<Arc<dyn FileSystemService>> {
        self.fs.read().unwrap().clone()
    }

    pub fn register_shell(&self, provider: Arc<dyn ShellService>) {
        *self.shell.write().unwrap() = Some(provider);
    }
    pub fn shell(&self) -> Option<Arc<dyn ShellService>> {
        self.shell.read().unwrap().clone()
    }

    pub fn register_process(&self, provider: Arc<dyn ProcessService>) {
        *self.process.write().unwrap() = Some(provider);
    }
    pub fn process(&self) -> Option<Arc<dyn ProcessService>> {
        self.process.read().unwrap().clone()
    }

    pub fn register_terminal(&self, provider: Arc<dyn TerminalService>) {
        *self.terminal.write().unwrap() = Some(provider);
    }
    pub fn terminal(&self) -> Option<Arc<dyn TerminalService>> {
        self.terminal.read().unwrap().clone()
    }

    pub fn register_repository(&self, provider: Arc<dyn RepositoryService>) {
        *self.repo.write().unwrap() = Some(provider);
    }
    pub fn repository(&self) -> Option<Arc<dyn RepositoryService>> {
        self.repo.read().unwrap().clone()
    }

    pub fn register_git(&self, provider: Arc<dyn GitService>) {
        *self.git.write().unwrap() = Some(provider);
    }
    pub fn git(&self) -> Option<Arc<dyn GitService>> {
        self.git.read().unwrap().clone()
    }

    /// Runtime authorization minting authority shared by this scope.
    pub fn authorization_authority(&self) -> &Arc<crate::git::AuthorizationAuthority> {
        &self.auth_authority
    }

    pub fn register_web(&self, provider: Arc<dyn WebService>) {
        *self.web.write().unwrap() = Some(provider);
    }
    pub fn web(&self) -> Option<Arc<dyn WebService>> {
        self.web.read().unwrap().clone()
    }

    pub fn register_network(&self, provider: Arc<dyn NetworkService>) {
        *self.network.write().unwrap() = Some(provider);
    }
    pub fn network(&self) -> Option<Arc<dyn NetworkService>> {
        self.network.read().unwrap().clone()
    }

    pub fn register_jobs(&self, provider: Arc<dyn JobService>) {
        *self.jobs.write().unwrap() = Some(provider);
    }
    pub fn jobs(&self) -> Option<Arc<dyn JobService>> {
        self.jobs.read().unwrap().clone()
    }

    pub fn register_sandbox(&self, provider: Arc<dyn SandboxService>) {
        *self.sandbox.write().unwrap() = Some(provider);
    }
    pub fn sandbox(&self) -> Option<Arc<dyn SandboxService>> {
        self.sandbox.read().unwrap().clone()
    }

    pub fn register_model(&self, provider: Arc<dyn ModelService>) {
        *self.model.write().unwrap() = Some(provider);
    }
    pub fn model(&self) -> Option<Arc<dyn ModelService>> {
        self.model.read().unwrap().clone()
    }

    pub fn register_memory(&self, provider: Arc<dyn MemoryService>) {
        *self.memory.write().unwrap() = Some(provider);
    }
    pub fn memory(&self) -> Option<Arc<dyn MemoryService>> {
        self.memory.read().unwrap().clone()
    }

    pub fn register_verification(&self, provider: Arc<dyn VerificationService>) {
        *self.verification.write().unwrap() = Some(provider);
    }
    pub fn verification(&self) -> Option<Arc<dyn VerificationService>> {
        self.verification.read().unwrap().clone()
    }

    pub fn register_artifacts(&self, provider: Arc<dyn ArtifactStoreService>) {
        *self.artifacts.write().unwrap() = Some(provider);
    }
    pub fn artifacts(&self) -> Option<Arc<dyn ArtifactStoreService>> {
        self.artifacts.read().unwrap().clone()
    }

    pub fn register_telemetry(&self, provider: Arc<dyn TelemetryService>) {
        *self.telemetry.write().unwrap() = Some(provider);
    }
    pub fn telemetry(&self) -> Option<Arc<dyn TelemetryService>> {
        self.telemetry.read().unwrap().clone()
    }
}
