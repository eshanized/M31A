//! Asynchronous builder and dependency injector for `M31aRuntime`.

use crate::config::RuntimeConfig;
use crate::error::RuntimeError;
use crate::runtime::M31aRuntime;
use crate::services::RuntimeServices;
use m31a_agent_engine::agents::EngineAgentRuntime;
use m31a_core::LocalEncryptedVault;
use m31a_core::features::FeatureRegistry;
use m31a_mcp::McpManager;
use m31a_proc::{ProcessManager, ProcessManagerConfig};
use m31a_provider::ProviderClient;
use m31a_storage::Storage;
use m31a_trust::TrustStore;
use std::sync::{Arc, Mutex, RwLock};

/// Fluent builder for constructing an `M31aRuntime` with optional dependency overrides.
pub struct RuntimeBuilder {
    config: RuntimeConfig,
    storage: Option<Storage>,
    vault: Option<Arc<LocalEncryptedVault>>,
    mcp_manager: Option<Arc<McpManager>>,
    trust_store: Option<Arc<Mutex<TrustStore>>>,
    provider_client: Option<ProviderClient>,
    tools: Option<m31a_tools::ToolRegistry>,
}

impl RuntimeBuilder {
    /// Create a new builder with the validated runtime configuration.
    pub fn new(config: RuntimeConfig) -> Self {
        Self {
            config,
            storage: None,
            vault: None,
            mcp_manager: None,
            trust_store: None,
            provider_client: None,
            tools: None,
        }
    }

    /// Inject an existing tool registry instance.
    pub fn with_tools(mut self, tools: m31a_tools::ToolRegistry) -> Self {
        self.tools = Some(tools);
        self
    }

    /// Inject an existing storage backend instance (useful for test isolation).
    pub fn with_storage(mut self, storage: Storage) -> Self {
        self.storage = Some(storage);
        self
    }

    /// Inject an existing encrypted vault instance.
    pub fn with_vault(mut self, vault: Arc<LocalEncryptedVault>) -> Self {
        self.vault = Some(vault);
        self
    }

    /// Inject an existing MCP manager instance.
    pub fn with_mcp_manager(mut self, mcp_manager: Arc<McpManager>) -> Self {
        self.mcp_manager = Some(mcp_manager);
        self
    }

    /// Inject an existing trust store instance.
    pub fn with_trust_store(mut self, trust_store: Arc<Mutex<TrustStore>>) -> Self {
        self.trust_store = Some(trust_store);
        self
    }

    /// Inject an existing provider client (useful for test mocks or pinned connections).
    pub fn with_provider_client(mut self, client: ProviderClient) -> Self {
        self.provider_client = Some(client);
        self
    }

    /// Assemble and initialize all runtime subsystems through the async-compatible API.
    ///
    /// Subsystem constructors are currently synchronous; callers needing to avoid blocking an
    /// async executor should invoke this builder from a blocking initialization boundary.
    pub async fn build(self) -> Result<M31aRuntime, RuntimeError> {
        self.build_sync()
    }

    /// Synchronously assemble and initialize all runtime subsystems.
    pub fn build_sync(mut self) -> Result<M31aRuntime, RuntimeError> {
        self.config.validate()?;
        self.config.cwd = self.config.cwd.canonicalize().map_err(RuntimeError::Io)?;

        let cwd = self.config.cwd.clone();
        let active_model = self.config.core.model.clone();

        // 1. Discover models catalog
        let available_models = crate::services::provider::discover_available_models(&active_model);

        // 2. Initialize provider client
        let provider = match self.provider_client {
            Some(client) => client,
            None => crate::services::provider::init_provider_client(
                self.config.core.model_provider.as_deref(),
                &active_model,
                &self.config.core.api_base,
                self.config.core.api_key.as_deref(),
                self.config.reasoning_effort.clone(),
            )?,
        };
        let provider = Arc::new(RwLock::new(Arc::new(provider)));

        // 3. Initialize storage
        let storage = match self.storage {
            Some(s) => s,
            None => crate::services::storage::init_storage(None)?,
        };

        // 4. Initialize vault
        let vault = match self.vault {
            Some(v) => v,
            None => {
                let vault_dir = cwd.join(".m31a-vault");
                std::fs::create_dir_all(&vault_dir).map_err(|error| {
                    RuntimeError::Security(format!(
                        "Failed to create encrypted vault directory '{}': {error}",
                        vault_dir.display()
                    ))
                })?;
                let metadata = std::fs::symlink_metadata(&vault_dir).map_err(|error| {
                    RuntimeError::Security(format!(
                        "Failed to inspect encrypted vault directory '{}': {error}",
                        vault_dir.display()
                    ))
                })?;
                if metadata.file_type().is_symlink() || !metadata.is_dir() {
                    return Err(RuntimeError::Security(format!(
                        "Encrypted vault path must be a real directory, not a symlink or file: '{}'",
                        vault_dir.display()
                    )));
                }
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    std::fs::set_permissions(
                        &vault_dir,
                        std::fs::Permissions::from_mode(0o700),
                    )
                    .map_err(|error| {
                        RuntimeError::Security(format!(
                            "Failed to restrict encrypted vault permissions for '{}': {error}",
                            vault_dir.display()
                        ))
                    })?;
                }
                Arc::new(
                    LocalEncryptedVault::new(vault_dir)
                        .map_err(|e| RuntimeError::Security(e.to_string()))?,
                )
            }
        };

        // 5. Initialize MCP manager
        let mcp_manager = match self.mcp_manager {
            Some(m) => m,
            None => crate::services::mcp::init_mcp_manager(self.config.core.mcp.clone()),
        };

        // 6. Initialize trust store
        let trust_store = match self.trust_store {
            Some(t) => t,
            None => crate::services::workspace::init_trust_store(),
        };

        // 7. Security components: Sandbox & Guardian
        let sandbox = Arc::new(crate::services::sandbox::init_sandbox());
        let guardian = Arc::new(RwLock::new(
            crate::services::guardian::init_guardian(&active_model),
        ));

        // 8. Lifecycle hooks runtime
        let hooks = crate::services::hooks::init_hooks(&cwd);

        // 9. Code mode orchestrator (if feature enabled)
        let code_mode = if self
            .config
            .core
            .features
            .get("code_mode")
            .copied()
            .unwrap_or(false)
        {
            Some(m31a_code_mode::CodeModeOrchestrator::new())
        } else {
            None
        };

        // 10. Process supervisor
        let proc_manager = Arc::new(ProcessManager::new(ProcessManagerConfig::default()));

        // 11. Workspace watcher
        let watcher =
            crate::services::workspace::init_workspace_watcher(&cwd, self.config.enable_watcher)?;

        // 12. Feature registry
        let features_registry = Arc::new(FeatureRegistry::new());

        // 13. Tool registry
        let tools = match self.tools {
            Some(mut t) => {
                if !self.config.enable_web_search {
                    t.disable("web_search");
                }
                t
            }
            None => crate::services::tools::init_tool_registry(
                Some(storage.clone()),
                vault.clone(),
                mcp_manager.clone(),
                self.config.core.extra_dirs.clone(),
                &cwd,
                self.config.enable_web_search,
            )?,
        };

        // 14. Child agent engine runtime
        let engine_runtime = Arc::new(
            EngineAgentRuntime::new(
                provider.read().unwrap().as_ref().clone(),
                tools.clone(),
                storage.clone(),
                self.config.exec_policy.clone(),
                cwd.clone(),
                Some(hooks.clone()),
                self.config.core.model_context_window,
                self.config.core.agents.max_depth as u32,
            )
            .with_trust_store(trust_store.clone()),
        );
        tools.set_agent_runtime(engine_runtime.clone());
        let engine_runtime = Arc::new(RwLock::new(engine_runtime));
        let tools = Arc::new(tools);

        let services = RuntimeServices {
            provider,
            storage,
            tools,
            sandbox,
            guardian,
            exec_policy: self.config.exec_policy.clone(),
            hooks,
            mcp_manager,
            code_mode,
            proc_manager,
            watcher,
            trust_store,
            features_registry,
            vault,
            engine_runtime,
        };

        Ok(M31aRuntime::new_internal(
            self.config,
            services,
            available_models,
        ))
    }
}
