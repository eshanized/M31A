//! Authoritative runtime service definitions and dependency container.

pub mod agent;
pub mod guardian;
pub mod hooks;
pub mod mcp;
pub mod provider;
pub mod sandbox;
pub mod storage;
pub mod tools;
pub mod workspace;

use crate::config::RuntimeConfig;
use crate::error::RuntimeError;
use m31a_agent_engine::agents::EngineAgentRuntime;
use m31a_core::LocalEncryptedVault;
use m31a_core::features::FeatureRegistry;
use m31a_execpolicy::ExecPolicy;
use m31a_guardian::GuardianReviewer;
use m31a_hooks::HookRuntime;
use m31a_mcp::McpManager;
use m31a_proc::ProcessManager;
use m31a_provider::ProviderClient;
use m31a_sandbox::SandboxRunner;
use m31a_storage::Storage;
use m31a_tools::ToolRegistry;
use m31a_trust::TrustStore;
use m31a_watcher::WorkspaceWatcher;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, RwLock};

/// Mutable runtime-owned state that must change atomically together.
///
/// Provider, Guardian, hooks, the child-agent runtime, workspace configuration,
/// and the watcher are committed under one lock so a turn cannot observe a
/// partially-applied model or workspace switch.
#[derive(Clone)]
pub(crate) struct RuntimeServiceState {
    pub config: RuntimeConfig,
    pub provider: Arc<ProviderClient>,
    pub guardian: Arc<GuardianReviewer>,
    pub hooks: Arc<HookRuntime>,
    pub engine_runtime: Arc<EngineAgentRuntime>,
    pub watcher: Option<Arc<WorkspaceWatcher>>,
}

/// Consistent snapshot of the runtime's replaceable services.
#[derive(Clone)]
pub struct RuntimeServiceSnapshot {
    pub config: RuntimeConfig,
    pub provider: Arc<ProviderClient>,
    pub guardian: Arc<GuardianReviewer>,
    pub hooks: Arc<HookRuntime>,
    pub engine_runtime: Arc<EngineAgentRuntime>,
    pub watcher: Option<Arc<WorkspaceWatcher>>,
}

/// Authoritative container holding initialized shared M31A runtime services.
#[derive(Clone)]
pub struct RuntimeServices {
    pub(crate) state: Arc<RwLock<RuntimeServiceState>>,
    pub storage: Storage,
    pub tools: Arc<ToolRegistry>,
    pub sandbox: Arc<SandboxRunner>,
    pub exec_policy: Arc<RwLock<ExecPolicy>>,
    pub mcp_manager: Arc<McpManager>,
    pub code_mode: Option<m31a_code_mode::CodeModeOrchestrator>,
    pub proc_manager: Arc<ProcessManager>,
    pub trust_store: Arc<Mutex<TrustStore>>,
    pub features_registry: Arc<FeatureRegistry>,
    pub vault: Arc<LocalEncryptedVault>,
    /// Session-scoped file authorization shared by every runtime-created engine.
    pub session_allowed_files: Arc<RwLock<HashSet<PathBuf>>>,
}

impl RuntimeServices {
    /// Returns an internally consistent snapshot of replaceable services.
    pub fn snapshot(&self) -> RuntimeServiceSnapshot {
        let state = self.state.read().unwrap_or_else(|e| e.into_inner());
        RuntimeServiceSnapshot {
            config: state.config.clone(),
            provider: Arc::clone(&state.provider),
            guardian: Arc::clone(&state.guardian),
            hooks: Arc::clone(&state.hooks),
            engine_runtime: Arc::clone(&state.engine_runtime),
            watcher: state.watcher.as_ref().map(Arc::clone),
        }
    }

    /// Returns the effective runtime configuration.
    pub fn config(&self) -> RuntimeConfig {
        self.snapshot().config
    }

    /// Returns the active provider client.
    pub fn provider(&self) -> Arc<ProviderClient> {
        Arc::clone(&self.state.read().unwrap_or_else(|e| e.into_inner()).provider)
    }

    /// Returns the active Guardian reviewer.
    pub fn guardian(&self) -> Arc<GuardianReviewer> {
        Arc::clone(&self.state.read().unwrap_or_else(|e| e.into_inner()).guardian)
    }

    /// Returns the active lifecycle-hook runtime.
    pub fn hooks(&self) -> Arc<HookRuntime> {
        Arc::clone(&self.state.read().unwrap_or_else(|e| e.into_inner()).hooks)
    }

    /// Returns the active child-agent runtime.
    pub fn engine_runtime(&self) -> Arc<EngineAgentRuntime> {
        Arc::clone(
            &self
                .state
                .read()
                .unwrap_or_else(|e| e.into_inner())
                .engine_runtime,
        )
    }

    /// Returns the active workspace watcher, if enabled.
    pub fn watcher(&self) -> Option<Arc<WorkspaceWatcher>> {
        self.state
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .watcher
            .as_ref()
            .map(Arc::clone)
    }

    /// Returns the shared code-mode orchestrator, if enabled.
    pub fn code_mode(&self) -> Option<&m31a_code_mode::CodeModeOrchestrator> {
        self.code_mode.as_ref()
    }

    /// Atomically commits a new provider model and its dependent services.
    pub(crate) fn replace_model(
        &self,
        client: ProviderClient,
        model: &str,
        context_window: Option<usize>,
    ) -> Result<(), RuntimeError> {
        if model.trim().is_empty() {
            return Err(RuntimeError::Config(
                "Model identifier cannot be empty".to_string(),
            ));
        }

        let current = self.snapshot();
        let guardian = Arc::new(GuardianReviewer::new(model));
        let mut next_config = current.config.clone();
        next_config.core.model = model.to_string();
        if let Some(context_window) = context_window {
            next_config.core.model_context_window = context_window;
        }

        let engine_runtime = agent::build_engine_runtime(
            client.clone(),
            &self.tools,
            &self.storage,
            Arc::clone(&self.exec_policy),
            next_config.cwd.clone(),
            Some(Arc::clone(&current.hooks)),
            next_config.core.model_context_window,
            next_config.core.agents.max_depth as u32,
            Arc::clone(&self.trust_store),
        );

        let mut state = self.state.write().unwrap_or_else(|e| e.into_inner());
        state.config = next_config;
        state.provider = Arc::new(client);
        state.guardian = guardian;
        state.engine_runtime = engine_runtime;
        Ok(())
    }

    /// Atomically commits a workspace switch and rebuilds workspace-scoped services.
    pub(crate) async fn replace_workspace(
        &self,
        new_cwd: PathBuf,
        watcher_enabled: bool,
    ) -> Result<RuntimeServiceSnapshot, RuntimeError> {
        if !new_cwd.exists() || !new_cwd.is_dir() {
            return Err(RuntimeError::Config(format!(
                "Workspace directory does not exist or is not a directory: {}",
                new_cwd.display()
            )));
        }

        let current = self.snapshot();
        let hooks = Arc::new(hooks::init_hooks(&new_cwd, &self.mcp_manager));
        let watcher = workspace::init_workspace_watcher(&new_cwd, watcher_enabled)?;
        let engine_runtime = agent::build_engine_runtime(
            (*current.provider).clone(),
            &self.tools,
            &self.storage,
            Arc::clone(&self.exec_policy),
            new_cwd.clone(),
            Some(Arc::clone(&hooks)),
            current.config.core.model_context_window,
            current.config.core.agents.max_depth as u32,
            Arc::clone(&self.trust_store),
        );

        let mut next_config = current.config.clone();
        next_config.cwd = new_cwd.clone();

        {
            let mut state = self.state.write().unwrap_or_else(|e| e.into_inner());
            state.config = next_config;
            state.hooks = hooks;
            state.engine_runtime = engine_runtime;
            state.watcher = watcher;
        }

        if let Some(old_watcher) = current.watcher.as_ref() {
            old_watcher.stop();
        }

        let mut shell = self.tools.persistent_shell.lock().await;
        shell.set_cwd(new_cwd);

        Ok(self.snapshot())
    }

    /// Builds a workspace-scoped child-agent runtime from canonical services.
    pub(crate) fn scoped_engine_runtime(
        &self,
        cwd: &Path,
    ) -> Result<Arc<EngineAgentRuntime>, RuntimeError> {
        let current = self.snapshot();
        Ok(agent::build_engine_runtime(
            (*current.provider).clone(),
            &self.tools,
            &self.storage,
            Arc::clone(&self.exec_policy),
            cwd.to_path_buf(),
            Some(current.hooks),
            current.config.core.model_context_window,
            current.config.core.agents.max_depth as u32,
            Arc::clone(&self.trust_store),
        ))
    }
}
