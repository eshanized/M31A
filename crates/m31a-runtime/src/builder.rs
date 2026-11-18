//! Asynchronous builder and dependency injector for `M31aRuntime`.

use crate::config::RuntimeConfig;
use crate::error::RuntimeError;
use crate::runtime::M31aRuntime;
use crate::services::RuntimeServices;
use m31a_core::LocalEncryptedVault;
use m31a_core::features::FeatureRegistry;
use m31a_execpolicy::ExecPolicy;
use m31a_mcp::McpManager;
use m31a_provider::ProviderClient;
use m31a_storage::Storage;
use m31a_trust::TrustStore;
use std::collections::HashSet;
use std::path::PathBuf;
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

    /// Asynchronously assemble and initialize all runtime subsystems.
    pub async fn build(self) -> Result<M31aRuntime, RuntimeError> {
        self.build_sync()
    }

    /// Synchronously assemble and initialize all runtime subsystems.
    pub fn build_sync(self) -> Result<M31aRuntime, RuntimeError> {
        self.config.validate()?;

        let cwd = self.config.cwd.clone();
        let active_model = self.config.core.model.clone();
        let available_models = crate::services::provider::discover_available_models(&active_model);

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

        let mut storage = match self.storage {
            Some(storage) => storage,
            None => crate::services::storage::init_storage(None)?,
        };

        let vault = match self.vault {
            Some(vault) => vault,
            None => {
                let vault_dir = cwd.join(".m31a-vault");
                std::fs::create_dir_all(&vault_dir).map_err(RuntimeError::Io)?;
                Arc::new(
                    LocalEncryptedVault::new(vault_dir)
                        .map_err(|e| RuntimeError::Security(e.to_string()))?,
                )
            }
        };

        let requested_mcp = self.mcp_manager;
        let mut tools = match self.tools {
            Some(tools) => tools,
            None => crate::services::tools::init_tool_registry(
                Some(storage.clone()),
                vault.clone(),
                requested_mcp
                    .clone()
                    .unwrap_or_else(|| {
                        crate::services::mcp::init_mcp_manager(self.config.core.mcp.clone())
                    }),
                self.config.core.extra_dirs.clone(),
                &cwd,
                self.config.enable_web_search,
            )?,
        };

        // The runtime is the canonical owner. Injected compatibility objects are
        // normalized onto the same storage, vault, MCP manager, and execution policy.
        if let Some(tool_storage) = tools.storage_handle() {
            storage = tool_storage;
        }
        tools.set_storage(storage.clone());
        tools.set_vault(vault.clone());
        let mcp_manager = requested_mcp.unwrap_or_else(|| tools.mcp_manager());
        tools.set_mcp_manager(Arc::clone(&mcp_manager));

        let exec_policy = Arc::new(RwLock::new(self.config.exec_policy.clone()));
        tools.set_exec_policy_handle(Arc::clone(&exec_policy));
        if !self.config.enable_web_search {
            tools.disable("web_search");
        }
        if let Some(session_id) = self.config.session_id.as_deref() {
            tools.set_session_id(session_id);
        }
        let tools = Arc::new(tools);

        let trust_store = match self.trust_store {
            Some(trust_store) => trust_store,
            None => crate::services::workspace::init_trust_store(),
        };

        let code_mode = if self
            .config
            .core
            .features
            .get("code_mode")
            .copied()
            .unwrap_or(false)
        {
            let code_runtime = m31a_code_mode::CodeModeRuntime::new()
                .with_trust_store(Arc::clone(&trust_store));
            Some(m31a_code_mode::CodeModeOrchestrator::with_runtime(code_runtime))
        } else {
            None
        };

        let hooks = Arc::new(crate::services::hooks::init_hooks(&cwd, &mcp_manager));
        let guardian = crate::services::guardian::init_guardian(&active_model);
        let proc_manager = Arc::clone(&tools.proc_supervisor);
        let sandbox = Arc::new(tools.sandbox_runner());

        let watcher =
            crate::services::workspace::init_workspace_watcher(&cwd, self.config.enable_watcher)?;

        let mut features_registry = FeatureRegistry::new();
        features_registry.sync_from_map(&self.config.core.features);
        let features_registry = Arc::new(features_registry);

        let engine_runtime = crate::services::agent::build_engine_runtime(
            provider.clone(),
            &tools,
            &storage,
            Arc::clone(&exec_policy),
            cwd.clone(),
            Some(Arc::clone(&hooks)),
            self.config.core.model_context_window,
            self.config.core.agents.max_depth as u32,
            Arc::clone(&trust_store),
        );

        let state = Arc::new(RwLock::new(crate::services::RuntimeServiceState {
            config: self.config.clone(),
            provider: Arc::new(provider),
            guardian,
            hooks,
            engine_runtime,
            watcher,
        }));

        let services = RuntimeServices {
            state: Arc::clone(&state),
            storage,
            tools: Arc::clone(&tools),
            sandbox,
            exec_policy,
            mcp_manager,
            code_mode,
            proc_manager,
            trust_store,
            features_registry,
            vault,
            session_allowed_files: Arc::new(RwLock::new(HashSet::new())),
        };

        // Install a stable delegating adapter so model/workspace commits are
        // immediately visible to every cloned tool registry.
        let adapter = Arc::new(crate::services::agent::RuntimeAgentRuntime::new(state));
        tools.set_agent_runtime(adapter);

        Ok(M31aRuntime::new_internal(services, available_models))
    }
}

/// Path type alias retained for builder consumers that inspect the workspace root.
pub type RuntimeWorkspace = PathBuf;
