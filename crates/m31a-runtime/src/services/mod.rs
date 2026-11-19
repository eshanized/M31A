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
use std::sync::{Arc, Mutex, RwLock};

/// Authoritative container holding initialized shared M31A runtime services.
#[derive(Clone)]
pub struct RuntimeServices {
    pub provider: Arc<RwLock<Arc<ProviderClient>>>,
    pub storage: Storage,
    pub tools: Arc<ToolRegistry>,
    pub sandbox: Arc<SandboxRunner>,
    pub guardian: Arc<RwLock<Arc<GuardianReviewer>>>,
    pub exec_policy: ExecPolicy,
    pub hooks: Arc<HookRuntime>,
    pub mcp_manager: Arc<McpManager>,
    pub code_mode: Option<m31a_code_mode::CodeModeOrchestrator>,
    pub proc_manager: Arc<ProcessManager>,
    pub watcher: Option<Arc<WorkspaceWatcher>>,
    pub trust_store: Arc<Mutex<TrustStore>>,
    pub features_registry: Arc<FeatureRegistry>,
    pub vault: Arc<LocalEncryptedVault>,
    pub engine_runtime: Arc<RwLock<Arc<EngineAgentRuntime>>>,
    pub auditor: Arc<m31a_ebpf::ExecutionAuditor>,
    pub wasm_runtime: Arc<m31a_wasm::WasmRuntime>,
}
