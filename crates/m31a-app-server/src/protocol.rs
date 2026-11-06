//! JSON-RPC 2.0 protocol specifications and method definitions for M31A App Server.
//!
//! Defines RPC request/response schemas, error codes, notification types, and session parameters.

use anyhow::{Context, Result, anyhow, bail};
use m31a_agent_engine::agents::EngineAgentRuntime;
use m31a_agent_engine::speculative::{SpeculativeArena, SpeculativeBranchConfig};
use m31a_agent_engine::workflow::{WorkflowExecutionConfig, WorkflowOrchestrator};
use m31a_agent_engine::{AgentEngine, EngineEvent};
use m31a_core::types::ChatMessage;
use m31a_core::{ApprovalPolicy, CollaborationMode, Config, SkillLoader, SystemPromptBuilder};
use m31a_execpolicy::ExecPolicy;
use m31a_guardian::GuardianReviewer;
use m31a_provider::get_nvidia_model_catalog;
use m31a_provider::{ProviderBuilder, ProviderConfig, ProviderType};
use m31a_storage::Storage;
use m31a_tools::ToolRegistry;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::{broadcast, mpsc, oneshot};
use tokio_util::sync::CancellationToken;
use tracing::info;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: Option<serde_json::Value>,
    pub method: String,
    #[serde(default)]
    pub params: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcError {
    pub code: i64,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcNotification {
    pub jsonrpc: String,
    pub method: String,
    #[serde(default)]
    pub params: Option<serde_json::Value>,
}

#[derive(Clone)]
pub struct ActiveTurnHandle {
    pub thread_id: String,
    pub turn_id: String,
    pub steer_tx: mpsc::UnboundedSender<String>,
    pub cancel_token: CancellationToken,
}

#[derive(Debug)]
pub struct PendingApprovalEntry {
    pub thread_id: String,
    pub turn_id: String,
    pub response_tx: oneshot::Sender<m31a_elicitation::ApprovalDecision>,
}

#[derive(Debug)]
pub struct PendingQuestionEntry {
    pub thread_id: String,
    pub turn_id: String,
    pub response_tx: oneshot::Sender<Vec<String>>,
}

#[derive(Clone)]
pub struct AppServer {
    config: Arc<Config>,
    storage: Storage,
    tools: Arc<ToolRegistry>,
    /// Shared hooks runtime: parent engine + §4.3 child engines dispatch
    /// through the same hook configuration.
    hooks: Arc<m31a_hooks::HookRuntime>,
    broadcaster: broadcast::Sender<JsonRpcNotification>,
    pub admission: Arc<crate::turn_admission::TurnAdmissionController>,
    pub multiplexer: Arc<crate::connection_multiplexer::ConnectionMultiplexer>,
    pub lifecycle: Arc<crate::daemon_lifecycle::DaemonLifecycleCoordinator>,
    pub resource_validator: Arc<crate::resource_validator::ResourceValidator>,
    pub trust_store: Arc<Mutex<m31a_trust::TrustStore>>,
    pub pending_approvals: Arc<Mutex<HashMap<String, PendingApprovalEntry>>>,
    pub pending_questions: Arc<Mutex<HashMap<String, PendingQuestionEntry>>>,
    pub active_turns: Arc<Mutex<HashMap<String, ActiveTurnHandle>>>,
}

impl AppServer {
    pub fn new(config: Arc<Config>, storage: Storage, tools: Arc<ToolRegistry>) -> Result<Self> {
        let (broadcaster, _) = broadcast::channel(1024);
        let admission = Arc::new(crate::turn_admission::TurnAdmissionController::default());
        let multiplexer = Arc::new(crate::connection_multiplexer::ConnectionMultiplexer::default());
        let lifecycle = Arc::new(crate::daemon_lifecycle::DaemonLifecycleCoordinator::new(
            Arc::clone(&admission),
            Arc::clone(&multiplexer),
            Some(m31a_core::uds::default_daemon_socket_path()),
        ));
        let pending_approvals = Arc::new(Mutex::new(HashMap::new()));
        let pending_questions = Arc::new(Mutex::new(HashMap::new()));
        let active_turns = Arc::new(Mutex::new(HashMap::new()));
        let resource_validator = Arc::new(
            crate::resource_validator::ResourceValidator::with_storage(storage.clone()),
        );
        let trust_store = Arc::new(Mutex::new(m31a_trust::TrustStore::new()));

        let hooks = Arc::new(m31a_hooks::HookRuntime::new(
            m31a_hooks::HookConfig::discover(
                &std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")),
            ),
        ));

        // Construct provider and agent runtime before initializing the server.
        // This ensures the server always has a real runtime or fails early.
        let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
        let provider_type = ProviderType::resolve(
            config.model_provider.as_deref(),
            &config.model,
            Some(&config.api_base),
        )?;
        let provider_config = ProviderConfig {
            provider_type,
            api_base: Some(config.api_base.clone()),
            api_key: config.api_key.clone(),
            model: config.model.clone(),
            extra: std::collections::HashMap::new(),
        };
        let provider = ProviderBuilder::build_client(provider_config).map_err(|err| {
            anyhow::anyhow!(
                "Failed to initialize provider for model '{}': {}",
                config.model,
                err
            )
        })?;

        let engine_runtime = Arc::new(m31a_agent_engine::agents::EngineAgentRuntime::new(
            provider,
            (*tools).clone(),
            storage.clone(),
            ExecPolicy::standard(),
            cwd.clone(),
            Some(hooks.clone()),
            config.model_context_window,
            config.agents.max_depth as u32,
        ));
        tools.set_agent_runtime(engine_runtime.clone());

        Ok(Self {
            config,
            storage,
            tools,
            hooks,
            broadcaster,
            admission,
            multiplexer,
            lifecycle,
            resource_validator,
            trust_store,
            pending_approvals,
            pending_questions,
            active_turns,
        })
    }

    pub fn subscribe(&self) -> broadcast::Receiver<JsonRpcNotification> {
        self.broadcaster.subscribe()
    }

    pub fn notify(&self, method: impl Into<String>, params: Option<serde_json::Value>) {
        let notif = JsonRpcNotification {
            jsonrpc: "2.0".to_string(),
            method: method.into(),
            params,
        };
        let _ = self.broadcaster.send(notif);
    }

    pub async fn run_stdio(&self) -> Result<()> {
        info!("M31A App Server running in stdio transport mode");
        let stdin = tokio::io::stdin();
        let mut reader = BufReader::new(stdin).lines();
        let stdout = tokio::io::stdout();
        let stdout_writer = Arc::new(tokio::sync::Mutex::new(stdout));

        // Background worker to stream broadcast notifications to stdout
        let mut notif_rx = self.subscribe();
        let stdout_writer_clone = Arc::clone(&stdout_writer);
        tokio::spawn(async move {
            while let Ok(notif) = notif_rx.recv().await {
                if let Ok(mut out_str) = serde_json::to_string(&notif) {
                    out_str.push('\n');
                    let mut lock = stdout_writer_clone.lock().await;
                    let _ = lock.write_all(out_str.as_bytes()).await;
                    let _ = lock.flush().await;
                }
            }
        });

        while let Some(line) = reader.next_line().await? {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            let response = match serde_json::from_str::<JsonRpcRequest>(line) {
                Ok(req) => self.handle_request(req).await,
                Err(err) => JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id: None,
                    result: None,
                    error: Some(JsonRpcError {
                        code: -32700,
                        message: format!("Parse error: {}", err),
                        data: None,
                    }),
                },
            };

            let mut out_str = serde_json::to_string(&response)?;
            out_str.push('\n');
            let mut lock = stdout_writer.lock().await;
            lock.write_all(out_str.as_bytes()).await?;
            lock.flush().await?;
        }

        Ok(())
    }

    #[cfg(unix)]
    pub async fn run_uds(&self, socket_path: &std::path::Path) -> Result<()> {
        info!(
            "M31A App Server listening on Unix domain socket: {:?}",
            socket_path
        );

        let listener = m31a_core::uds::bind_unix_listener(socket_path)
            .await
            .with_context(|| format!("Failed to bind Unix domain socket at {:?}", socket_path))?;

        let arc_self = Arc::new(self.clone());

        loop {
            let (stream, _) = listener.accept().await?;
            let server_clone = arc_self.clone();
            let mut notif_rx = server_clone.subscribe();

            tokio::spawn(async move {
                let (reader_half, writer_half) = stream.into_split();
                let mut reader = BufReader::new(reader_half).lines();
                let writer = Arc::new(tokio::sync::Mutex::new(writer_half));

                // Notification streaming task for this client
                let writer_notif = Arc::clone(&writer);
                let notif_worker = tokio::spawn(async move {
                    while let Ok(notif) = notif_rx.recv().await {
                        if let Ok(mut out_str) = serde_json::to_string(&notif) {
                            out_str.push('\n');
                            let mut lock = writer_notif.lock().await;
                            if lock.write_all(out_str.as_bytes()).await.is_err() {
                                break;
                            }
                            let _ = lock.flush().await;
                        }
                    }
                });

                while let Ok(Some(line)) = reader.next_line().await {
                    let line = line.trim();
                    if line.is_empty() {
                        continue;
                    }

                    let response = match serde_json::from_str::<JsonRpcRequest>(line) {
                        Ok(req) => server_clone.handle_request(req).await,
                        Err(err) => JsonRpcResponse {
                            jsonrpc: "2.0".to_string(),
                            id: None,
                            result: None,
                            error: Some(JsonRpcError {
                                code: -32700,
                                message: format!("Parse error: {}", err),
                                data: None,
                            }),
                        },
                    };

                    if let Ok(mut out_str) = serde_json::to_string(&response) {
                        out_str.push('\n');
                        let mut lock = writer.lock().await;
                        if lock.write_all(out_str.as_bytes()).await.is_err() {
                            break;
                        }
                        let _ = lock.flush().await;
                    }
                }

                notif_worker.abort();
            });
        }
    }

    #[cfg(not(unix))]
    pub async fn run_uds(&self, socket_path: &std::path::Path) -> Result<()> {
        anyhow::bail!(
            "Unix domain sockets are not supported on Windows (path: {:?})",
            socket_path
        );
    }

    /// §2.3: drives a real `AgentEngine` turn for an admitted request, bridging
    /// engine events onto the notification broadcaster and the thread
    /// multiplexer. Runs detached from the request loop; completion is reported
    /// as notifications and persisted onto the turn row.
    fn spawn_engine_turn(&self, thread_id: &str, turn_id: &str, prompt: &str) {
        let Some(ref api_key) = self.config.api_key else {
            self.notify(
                "notification/turn_failed",
                Some(serde_json::json!({
                    "turn_id": turn_id,
                    "thread_id": thread_id,
                    "error": "NVIDIA_API_KEY is not set",
                })),
            );
            return;
        };

        // Prefer the thread's recorded cwd; fall back to the process cwd.
        let cwd = self
            .storage
            .get_thread(thread_id)
            .ok()
            .flatten()
            .map(|t| PathBuf::from(t.cwd))
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

        let provider_type = match ProviderType::resolve(
            self.config.model_provider.as_deref(),
            &self.config.model,
            Some(&self.config.api_base),
        ) {
            Ok(pt) => pt,
            Err(e) => {
                self.notify(
                    "notification/turn_failed",
                    Some(serde_json::json!({
                        "turn_id": turn_id,
                        "thread_id": thread_id,
                        "error": format!("Failed to resolve provider: {e}"),
                    })),
                );
                return;
            }
        };
        let provider_config = ProviderConfig {
            provider_type,
            api_base: Some(self.config.api_base.clone()),
            api_key: Some(api_key.to_string()),
            model: self.config.model.clone(),
            extra: std::collections::HashMap::new(),
        };
        let provider = match ProviderBuilder::build_client(provider_config) {
            Ok(p) => p,
            Err(e) => {
                self.notify(
                    "notification/turn_failed",
                    Some(serde_json::json!({
                        "turn_id": turn_id,
                        "thread_id": thread_id,
                        "error": format!("Failed to initialize provider: {e}"),
                    })),
                );
                return;
            }
        };
        let mut engine = AgentEngine::new(
            provider,
            (*self.tools).clone(),
            self.storage.clone(),
            self.config.approval,
            ExecPolicy::standard(),
            self.config.sandbox,
            cwd.clone(),
        );
        if self.config.approval == ApprovalPolicy::ApproveForMe {
            engine.guardian = Some(Arc::new(GuardianReviewer::new(&self.config.model)));
        }
        engine.routing_enabled = self.config.routing_enabled;
        engine.tool_output_token_limit = self.config.tool_output_token_limit;
        engine.code_mode = if self
            .config
            .features
            .get("code_mode")
            .copied()
            .unwrap_or(false)
        {
            Some(m31a_code_mode::CodeModeOrchestrator::new())
        } else {
            None
        };
        engine.hooks = Some(self.hooks.clone());
        engine.dangerously_bypass = self.config.dangerously_bypass;

        let snapshot = m31a_core::prompts::PromptRuntimeSnapshot::new(
            &cwd,
            &self.config.model,
            &format!("{:?}", self.config.sandbox),
            CollaborationMode::Default,
        )
        .with_user_custom_instructions(self.config.system_prompt.clone())
        .with_provider(
            self.config
                .model_provider
                .as_deref()
                .unwrap_or("NVIDIA NIM / Upstream Provider"),
        );

        let system_instructions = SystemPromptBuilder::build_canonical(&snapshot);
        engine.authoritative_system_prompt = Some(system_instructions.clone());
        let history = vec![
            ChatMessage::system(system_instructions),
            ChatMessage::user(prompt.to_string()),
        ];

        let (eng_tx, mut eng_rx) = mpsc::unbounded_channel();
        let (steer_tx, steer_rx) = mpsc::unbounded_channel();
        let cancel_token = CancellationToken::new();

        self.active_turns.lock().unwrap().insert(
            turn_id.to_string(),
            ActiveTurnHandle {
                thread_id: thread_id.to_string(),
                turn_id: turn_id.to_string(),
                steer_tx,
                cancel_token: cancel_token.clone(),
            },
        );

        let server = self.clone();
        let thread = thread_id.to_string();
        let turn = turn_id.to_string();
        tokio::spawn(async move {
            let run_fut = engine.run_turn(&thread, history, eng_tx, steer_rx, cancel_token, 20);
            tokio::pin!(run_fut);
            loop {
                tokio::select! {
                    res = &mut run_fut => {
                        if let Err(e) = res {
                            server.notify("notification/turn_failed", Some(serde_json::json!({
                                "turn_id": turn,
                                "thread_id": thread,
                                "error": e.to_string(),
                            })));
                        }
                        break;
                    }
                    ev = eng_rx.recv() => {
                        if let Some(ev) = ev {
                            server.bridge_engine_event(&turn, &thread, ev);
                        }
                    }
                }
            }

            // Drain events buffered while the run future finished.
            while let Some(ev) = eng_rx.recv().await {
                server.bridge_engine_event(&turn, &thread, ev);
            }

            // Clean up active turn handle
            server.active_turns.lock().unwrap().remove(&turn);

            let _ = server.storage.save_turn(&turn, &thread, 1, "completed");
            server.notify(
                "notification/turn_completed",
                Some(serde_json::json!({ "turn_id": turn, "thread_id": thread })),
            );
        });
    }

    /// Maps an engine event onto the app-server notification surfaces.
    /// Approval requests and user-input questions are registered into pending
    /// maps so clients can respond interactively.
    pub fn bridge_engine_event(&self, turn_id: &str, thread_id: &str, ev: EngineEvent) {
        let (method, params): (&str, serde_json::Value) = match ev {
            EngineEvent::ContentChunk(chunk) => (
                "notification/turn_content_chunk",
                serde_json::json!({ "turn_id": turn_id, "thread_id": thread_id, "chunk": chunk }),
            ),
            EngineEvent::ReasoningChunk(chunk) => (
                "notification/turn_reasoning_chunk",
                serde_json::json!({ "turn_id": turn_id, "thread_id": thread_id, "chunk": chunk }),
            ),
            EngineEvent::ToolCallStarted { id, name, args } => (
                "notification/tool_started",
                serde_json::json!({ "turn_id": turn_id, "thread_id": thread_id, "call_id": id, "tool": name, "arguments": args }),
            ),
            EngineEvent::ToolCallFinished {
                id,
                output,
                success,
            } => (
                "notification/tool_finished",
                serde_json::json!({ "turn_id": turn_id, "thread_id": thread_id, "call_id": id, "success": success, "output": output }),
            ),
            EngineEvent::GuardianDenied { rationale } => (
                "notification/guardian_denied",
                serde_json::json!({ "turn_id": turn_id, "thread_id": thread_id, "rationale": rationale }),
            ),
            EngineEvent::Notice(msg, is_warning) => (
                "notification/notice",
                serde_json::json!({ "turn_id": turn_id, "thread_id": thread_id, "message": msg, "warning": is_warning }),
            ),
            EngineEvent::ApprovalRequested {
                approval,
                response_tx,
            } => {
                let approval_id = format!("appr_{}", uuid::Uuid::new_v4().simple());
                self.pending_approvals.lock().unwrap().insert(
                    approval_id.clone(),
                    PendingApprovalEntry {
                        thread_id: thread_id.to_string(),
                        turn_id: turn_id.to_string(),
                        response_tx,
                    },
                );
                let params = serde_json::json!({
                    "approval_id": approval_id,
                    "turn_id": turn_id,
                    "thread_id": thread_id,
                    "tool": approval.tool_call.function.name,
                    "arguments": approval.tool_call.function.arguments,
                    "command": approval.command,
                    "reason": approval.reason,
                });
                self.notify("notification/approval_requested", Some(params.clone()));
                self.multiplexer.broadcast_thread(
                    thread_id,
                    JsonRpcNotification {
                        jsonrpc: "2.0".to_string(),
                        method: "notification/approval_requested".to_string(),
                        params: Some(params),
                    },
                );
                return;
            }
            EngineEvent::QuestionRequested {
                question,
                response_tx,
            } => {
                let question_id = format!("quest_{}", uuid::Uuid::new_v4().simple());
                self.pending_questions.lock().unwrap().insert(
                    question_id.clone(),
                    PendingQuestionEntry {
                        thread_id: thread_id.to_string(),
                        turn_id: turn_id.to_string(),
                        response_tx,
                    },
                );
                let params = serde_json::json!({
                    "question_id": question_id,
                    "turn_id": turn_id,
                    "thread_id": thread_id,
                    "prompt": question.question,
                    "options": question.options,
                    "is_multi_select": question.is_multi_select,
                });
                self.notify("notification/question_requested", Some(params.clone()));
                self.multiplexer.broadcast_thread(
                    thread_id,
                    JsonRpcNotification {
                        jsonrpc: "2.0".to_string(),
                        method: "notification/question_requested".to_string(),
                        params: Some(params),
                    },
                );
                return;
            }
            EngineEvent::Compacted {
                window_id,
                original_tokens,
                compacted_tokens,
            } => (
                "notification/compaction",
                serde_json::json!({
                    "turn_id": turn_id,
                    "thread_id": thread_id,
                    "window_id": window_id,
                    "original_tokens": original_tokens,
                    "compacted_tokens": compacted_tokens,
                }),
            ),
            EngineEvent::TurnComplete { usage, messages } => {
                let last = messages
                    .iter()
                    .rev()
                    .find(|m| m.role == m31a_core::types::Role::Assistant)
                    .and_then(|m| m.content.clone())
                    .unwrap_or_default();
                (
                    "notification/turn_message",
                    serde_json::json!({ "turn_id": turn_id, "thread_id": thread_id, "message": last, "usage": usage }),
                )
            }
            EngineEvent::TurnFailed(err) => (
                "notification/turn_failed",
                serde_json::json!({ "turn_id": turn_id, "thread_id": thread_id, "error": err }),
            ),
        };

        self.notify(method, Some(params.clone()));
        self.multiplexer.broadcast_thread(
            thread_id,
            JsonRpcNotification {
                jsonrpc: "2.0".to_string(),
                method: method.to_string(),
                params: Some(params),
            },
        );
    }

    pub fn workflow_orchestrator(
        &self,
        workspace_root: Option<&std::path::Path>,
    ) -> WorkflowOrchestrator {
        let provider = if let Some(ref api_key) = self.config.api_key {
            let pt = ProviderType::resolve(
                self.config.model_provider.as_deref(),
                &self.config.model,
                Some(&self.config.api_base),
            )
            .unwrap_or(ProviderType::Nim);

            let provider_config = ProviderConfig {
                provider_type: pt,
                api_base: Some(self.config.api_base.clone()),
                api_key: Some(api_key.clone()),
                model: self.config.model.clone(),
                extra: std::collections::HashMap::new(),
            };
            ProviderBuilder::build_client(provider_config).ok()
        } else {
            None
        };

        let exec_policy = ExecPolicy::default();
        let runtime = if let (Some(client), Some(ws)) = (provider.as_ref(), workspace_root) {
            Some(Arc::new(EngineAgentRuntime::new(
                client.clone(),
                self.tools.as_ref().clone(),
                self.storage.clone(),
                exec_policy.clone(),
                ws.to_path_buf(),
                Some(self.hooks.clone()),
                self.config.model_context_window,
                3,
            )))
        } else {
            None
        };

        let mut orch = WorkflowOrchestrator::new(
            self.storage.clone(),
            runtime,
            provider,
            Some(self.tools.as_ref().clone()),
            Some(exec_policy),
        );

        if let Some(ws) = workspace_root {
            let settings = m31a_storage::worktree::WorktreeSettings::new(ws.to_path_buf());
            orch = orch.with_worktree_manager(Arc::new(
                m31a_storage::worktree::WorktreeManager::new(settings),
            ));
        }

        orch
    }

    pub async fn handle_request(&self, req: JsonRpcRequest) -> JsonRpcResponse {
        let id = req.id.clone();
        let params_val = req.params.clone().unwrap_or(serde_json::Value::Null);

        // Centralized RPC resource validation before dispatch
        if let Err(val_err) = self
            .resource_validator
            .validate_rpc(&req.method, &params_val)
        {
            return JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id,
                result: None,
                error: Some(JsonRpcError {
                    code: val_err.jsonrpc_code(),
                    message: val_err.to_string(),
                    data: None,
                }),
            };
        }

        match self.dispatch(&req.method, req.params).await {
            Ok(result) => JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id,
                result: Some(result),
                error: None,
            },
            Err(err) => {
                let code = if err.to_string().contains("Access denied")
                    || err.to_string().contains("Permission denied")
                {
                    -32003
                } else {
                    -32603
                };
                JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id,
                    result: None,
                    error: Some(JsonRpcError {
                        code,
                        message: err.to_string(),
                        data: None,
                    }),
                }
            }
        }
    }

    async fn dispatch(
        &self,
        method: &str,
        params: Option<serde_json::Value>,
    ) -> Result<serde_json::Value> {
        let params = params.unwrap_or(serde_json::Value::Null);

        match method {
            "initialize" => Ok(serde_json::json!({
                "server_name": "m31a-app-server",
                "version": "0.1.13",
                "protocol_version": "2.0",
                "capabilities": {
                    "threads": true,
                    "turns": true,
                    "interactive_approvals": true,
                    "turn_steering": true,
                    "turn_interrupt": true,
                    "code_mode": true,
                    "guardian_review": true,
                    "message_board": true,
                    "skills": true,
                    "sandbox": true
                }
            })),

            "server/diagnostics" => {
                let has_api_key = self.config.api_key.is_some();
                let bwrap_installed = std::process::Command::new("which")
                    .arg("bwrap")
                    .output()
                    .map(|o| o.status.success())
                    .unwrap_or(false);
                Ok(serde_json::json!({
                    "status": "healthy",
                    "api_key_configured": has_api_key,
                    "bubblewrap_available": bwrap_installed,
                    "active_model": self.config.model,
                    "sandbox_policy": format!("{:?}", self.config.sandbox),
                    "approval_policy": format!("{:?}", self.config.approval)
                }))
            }

            "config/read" => Ok(self.config.sanitized_value()),

            "models/list" | "model/list" => {
                let catalog = get_nvidia_model_catalog();
                Ok(serde_json::to_value(catalog)?)
            }

            "skills/list" => {
                let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
                let skills = SkillLoader::discover_skills(&cwd);
                Ok(serde_json::to_value(skills)?)
            }

            "instructions/list" => {
                let cwd = params
                    .get("cwd")
                    .and_then(|v| v.as_str())
                    .map(PathBuf::from)
                    .unwrap_or_else(|| {
                        std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
                    });
                let instructions = m31a_core::HierarchicalInstructions::discover(&cwd, None);
                Ok(serde_json::to_value(instructions.sources_summary())?)
            }

            "tools/list" => {
                let defs = self.tools.definitions();
                Ok(serde_json::to_value(defs)?)
            }

            "thread/create" | "thread/start" => {
                let cwd = params.get("cwd").and_then(|v| v.as_str()).unwrap_or(".");
                let cwd_path = PathBuf::from(cwd);
                // Remote clients cannot grant themselves trust using allow_untrusted: true!
                self.trust_store
                    .lock()
                    .unwrap()
                    .authorize(&cwd_path, true, false)?;

                let model = params
                    .get("model")
                    .and_then(|v| v.as_str())
                    .unwrap_or(&self.config.model);
                let session = m31a_core::Session::new(model, cwd_path);
                self.storage
                    .save_thread(&session.metadata, "", "workspace-write", "on-request")?;

                Ok(serde_json::to_value(&session.metadata)?)
            }

            "thread/read" => {
                let id = params
                    .get("thread_id")
                    .or_else(|| params.get("id"))
                    .and_then(|v| v.as_str())
                    .context("Missing 'thread_id'")?;
                let thread = self
                    .storage
                    .get_thread(id)?
                    .ok_or_else(|| anyhow!("Thread not found: {id}"))?;
                let items = self.storage.list_items(id)?;
                Ok(serde_json::json!({
                    "thread": thread,
                    "items": items
                }))
            }

            "thread/list" => {
                let limit = params.get("limit").and_then(|v| v.as_u64()).unwrap_or(50) as usize;
                let threads = self.storage.list_threads(limit)?;
                Ok(serde_json::to_value(threads)?)
            }

            "workspace/trust_request" => {
                let path = params
                    .get("path")
                    .and_then(|v| v.as_str())
                    .context("Missing 'path'")?;
                let reason = params
                    .get("reason")
                    .and_then(|v| v.as_str())
                    .unwrap_or("User requested workspace execution");
                let target = PathBuf::from(path);
                let decision = self.trust_store.lock().unwrap().evaluate(&target);
                Ok(serde_json::json!({
                    "path": target.display().to_string(),
                    "decision": format!("{:?}", decision),
                    "reason": reason,
                }))
            }

            "workspace/trust_grant" => {
                let path = params
                    .get("path")
                    .and_then(|v| v.as_str())
                    .context("Missing 'path'")?;
                let level_str = params
                    .get("level")
                    .and_then(|v| v.as_str())
                    .unwrap_or("session_only");
                let level = match level_str {
                    "trusted" => m31a_trust::TrustLevel::Trusted,
                    "session_only" => m31a_trust::TrustLevel::SessionOnly,
                    "denied" => m31a_trust::TrustLevel::Denied,
                    _ => m31a_trust::TrustLevel::Untrusted,
                };
                let target = PathBuf::from(path);
                self.trust_store.lock().unwrap().trust(&target, level)?;
                self.resource_validator.record_audit(
                    "workspace_trust_grant",
                    true,
                    &format!("Granted trust level {:?} to '{}'", level, target.display()),
                );
                Ok(serde_json::json!({
                    "status": "granted",
                    "path": target.display().to_string(),
                    "trust_level": format!("{:?}", level),
                }))
            }

            "thread/archive" => {
                let id = params
                    .get("thread_id")
                    .or_else(|| params.get("id"))
                    .and_then(|v| v.as_str())
                    .context("Missing 'thread_id'")?;
                self.storage.archive_thread(id)?;
                Ok(serde_json::json!({ "archived": true, "thread_id": id }))
            }

            "thread/unarchive" => {
                let id = params
                    .get("thread_id")
                    .or_else(|| params.get("id"))
                    .and_then(|v| v.as_str())
                    .context("Missing 'thread_id'")?;
                self.storage.unarchive_thread(id)?;
                Ok(serde_json::json!({ "unarchived": true, "thread_id": id }))
            }

            "thread/delete" => {
                let id = params
                    .get("thread_id")
                    .or_else(|| params.get("id"))
                    .and_then(|v| v.as_str())
                    .context("Missing 'thread_id'")?;
                self.storage.delete_thread(id)?;
                Ok(serde_json::json!({ "deleted": true, "thread_id": id }))
            }

            "thread/fork" => {
                let parent_id = params
                    .get("thread_id")
                    .or_else(|| params.get("id"))
                    .and_then(|v| v.as_str())
                    .context("Missing 'thread_id'")?;
                let new_title = params
                    .get("title")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Forked Session");
                let forked = self.storage.fork_thread(parent_id, new_title)?;
                Ok(serde_json::to_value(forked)?)
            }

            "thread/resume" => {
                let id = params
                    .get("thread_id")
                    .or_else(|| params.get("id"))
                    .and_then(|v| v.as_str())
                    .context("Missing 'thread_id'")?;
                let thread = self
                    .storage
                    .get_thread(id)?
                    .ok_or_else(|| anyhow!("Thread not found: {id}"))?;
                Ok(serde_json::to_value(thread)?)
            }

            "thread/section/create" => {
                let name = params
                    .get("name")
                    .and_then(|v| v.as_str())
                    .context("Missing 'name' parameter")?;
                let color = params.get("color").and_then(|v| v.as_str());
                let icon = params.get("icon").and_then(|v| v.as_str());
                let section = self.storage.create_thread_section(name, color, icon)?;
                Ok(serde_json::to_value(section)?)
            }

            "thread/section/list" => {
                let sections = self.storage.list_thread_sections()?;
                Ok(serde_json::to_value(sections)?)
            }

            "thread/section/rename" => {
                let section_id = params
                    .get("section_id")
                    .and_then(|v| v.as_str())
                    .context("Missing 'section_id'")?;
                let name = params
                    .get("name")
                    .and_then(|v| v.as_str())
                    .context("Missing 'name'")?;
                let color = params.get("color").and_then(|v| v.as_str());
                let icon = params.get("icon").and_then(|v| v.as_str());
                let ok = self
                    .storage
                    .rename_thread_section(section_id, name, color, icon)?;
                Ok(serde_json::json!({ "renamed": ok, "section_id": section_id }))
            }

            "thread/section/delete" => {
                let section_id = params
                    .get("section_id")
                    .and_then(|v| v.as_str())
                    .context("Missing 'section_id'")?;
                let ok = self.storage.delete_thread_section(section_id)?;
                Ok(serde_json::json!({ "deleted": ok, "section_id": section_id }))
            }

            "thread/section/move" => {
                let thread_id = params
                    .get("thread_id")
                    .and_then(|v| v.as_str())
                    .context("Missing 'thread_id'")?;
                let section_id = params.get("section_id").and_then(|v| v.as_str());
                let ok = self.storage.move_thread_to_section(thread_id, section_id)?;
                Ok(
                    serde_json::json!({ "moved": ok, "thread_id": thread_id, "section_id": section_id }),
                )
            }

            "thread/section/threads" => {
                let section_id = params.get("section_id").and_then(|v| v.as_str());
                let limit = params.get("limit").and_then(|v| v.as_u64()).unwrap_or(50) as usize;
                let threads = self.storage.list_threads_by_section(section_id, limit)?;
                Ok(serde_json::to_value(threads)?)
            }

            "command/exec" => {
                let cmd = params
                    .get("command")
                    .and_then(|v| v.as_str())
                    .context("Missing 'command'")?;
                let cwd = params
                    .get("cwd")
                    .and_then(|v| v.as_str())
                    .map(PathBuf::from)
                    .unwrap_or_else(|| {
                        std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
                    });

                self.notify(
                    "notification/tool_started",
                    Some(serde_json::json!({
                        "tool": "exec_command",
                        "command": cmd,
                        "cwd": cwd.to_string_lossy(),
                    })),
                );

                let res = self
                    .tools
                    .execute(
                        "exec_command",
                        &serde_json::json!({ "command": cmd }).to_string(),
                        &cwd,
                        self.config.sandbox,
                    )
                    .await?;

                self.notify(
                    "notification/tool_finished",
                    Some(serde_json::json!({
                        "tool": "exec_command",
                        "command": cmd,
                        "output_length": res.len(),
                    })),
                );

                Ok(serde_json::json!({ "output": res }))
            }

            "turn/start" => {
                let thread_id = params
                    .get("thread_id")
                    .and_then(|v| v.as_str())
                    .context("Missing 'thread_id'")?;
                let prompt = params
                    .get("prompt")
                    .and_then(|v| v.as_str())
                    .context("Missing 'prompt'")?;
                let thread = self
                    .storage
                    .get_thread(thread_id)?
                    .ok_or_else(|| anyhow!("Thread not found: {thread_id}"))?;
                let thread_cwd = PathBuf::from(&thread.cwd);
                // Remote clients cannot grant themselves trust using allow_untrusted: true!
                self.trust_store
                    .lock()
                    .unwrap()
                    .authorize(&thread_cwd, true, false)?;

                let turn_id = format!("turn_{}", uuid::Uuid::new_v4().simple());

                let admission_req = crate::turn_admission::TurnAdmissionRequest::new(
                    thread_id,
                    crate::turn_admission::TurnPriority::UserInteractive,
                    prompt,
                );

                match self.admission.submit_turn(admission_req) {
                    crate::turn_admission::TurnAdmissionDecision::Admitted { lease_id } => {
                        self.storage
                            .save_turn(&turn_id, thread_id, 1, "in_progress")?;
                        self.storage.save_item(
                            &format!("item_{}", uuid::Uuid::new_v4().simple()),
                            thread_id,
                            Some(&turn_id),
                            "user",
                            prompt,
                        )?;

                        self.notify(
                            "notification/turn_started",
                            Some(serde_json::json!({
                                "turn_id": turn_id,
                                "thread_id": thread_id,
                                "lease_id": lease_id,
                            })),
                        );

                        self.multiplexer.broadcast_thread(
                            thread_id,
                            JsonRpcNotification {
                                jsonrpc: "2.0".to_string(),
                                method: "notification/turn_started".to_string(),
                                params: Some(serde_json::json!({
                                    "turn_id": turn_id,
                                    "thread_id": thread_id,
                                    "lease_id": lease_id,
                                })),
                            },
                        );

                        // §2.3: the admitted turn actually runs the agent engine,
                        // streaming engine events as notifications.
                        self.spawn_engine_turn(thread_id, &turn_id, prompt);

                        Ok(serde_json::json!({
                            "turn_id": turn_id,
                            "thread_id": thread_id,
                            "lease_id": lease_id,
                            "status": "started"
                        }))
                    }
                    crate::turn_admission::TurnAdmissionDecision::SteeredIntoActiveTurn {
                        active_lease_id,
                    } => Ok(serde_json::json!({
                        "thread_id": thread_id,
                        "status": "steered_into_active_turn",
                        "active_lease_id": active_lease_id,
                    })),
                    crate::turn_admission::TurnAdmissionDecision::Queued {
                        queue_position,
                        estimated_wait_ms,
                    } => Ok(serde_json::json!({
                        "thread_id": thread_id,
                        "turn_id": turn_id,
                        "status": "queued",
                        "queue_position": queue_position,
                        "estimated_wait_ms": estimated_wait_ms,
                    })),
                    crate::turn_admission::TurnAdmissionDecision::RejectedBusy { reason } => {
                        bail!("Turn admission rejected: {}", reason);
                    }
                }
            }

            "turn/finish" => {
                let thread_id = params
                    .get("thread_id")
                    .and_then(|v| v.as_str())
                    .context("Missing 'thread_id'")?;
                let lease_id_str = params
                    .get("lease_id")
                    .and_then(|v| v.as_str())
                    .context("Missing 'lease_id'")?;
                let lease_id =
                    uuid::Uuid::parse_str(lease_id_str).context("Invalid lease_id UUID")?;
                let next_turn = self.admission.complete_turn(thread_id, lease_id);
                self.notify(
                    "notification/turn_finished",
                    Some(serde_json::json!({
                        "thread_id": thread_id,
                        "lease_id": lease_id_str,
                    })),
                );
                Ok(serde_json::json!({
                    "thread_id": thread_id,
                    "status": "finished",
                    "has_next_queued": next_turn.is_some(),
                }))
            }

            "turn/approve" | "turn/decision" | "approval/respond" => {
                let approval_id = params
                    .get("approval_id")
                    .or_else(|| params.get("id"))
                    .and_then(|v| v.as_str())
                    .context("Missing 'approval_id'")?;
                let decision_val = params.get("decision");
                let reason = params
                    .get("reason")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                let command = params.get("command").and_then(|v| v.as_str());

                let decision = if let Some(new_cmd) = command {
                    m31a_elicitation::ApprovalDecision::EditCommand {
                        new_command: new_cmd.to_string(),
                    }
                } else if let Some(d_str) = decision_val.and_then(|v| v.as_str()) {
                    match d_str.to_lowercase().as_str() {
                        "allow" | "allow_once" | "yes" | "accept" => {
                            m31a_elicitation::ApprovalDecision::AllowOnce
                        }
                        "allow_all" | "allow_always" | "always_allow" | "accept_always" => {
                            let pattern = params
                                .get("pattern")
                                .and_then(|v| v.as_str())
                                .unwrap_or("*");
                            m31a_elicitation::ApprovalDecision::AlwaysAllowPattern(
                                pattern.to_string(),
                            )
                        }
                        "allow_file" | "always_allow_file" => {
                            let path = params
                                .get("path")
                                .and_then(|v| v.as_str())
                                .context("Missing 'path' for allow_file decision")?;
                            m31a_elicitation::ApprovalDecision::AllowFile {
                                path: path.to_string(),
                            }
                        }
                        _ => m31a_elicitation::ApprovalDecision::Deny {
                            reason: reason.unwrap_or_else(|| "Denied by client".to_string()),
                        },
                    }
                } else if let Some(b) = decision_val.and_then(|v| v.as_bool()) {
                    if b {
                        m31a_elicitation::ApprovalDecision::AllowOnce
                    } else {
                        m31a_elicitation::ApprovalDecision::Deny {
                            reason: reason.unwrap_or_else(|| "Denied by client".to_string()),
                        }
                    }
                } else {
                    anyhow::bail!("Missing explicit 'decision' for approval response");
                };

                let mut pending = self.pending_approvals.lock().unwrap();
                let entry = pending.get(approval_id).ok_or_else(|| {
                    anyhow!("Approval request '{approval_id}' not found or already resolved")
                })?;

                if let Some(param_tid) = params.get("thread_id").and_then(|v| v.as_str())
                    && param_tid != entry.thread_id
                {
                    bail!(
                        "Conflicting thread identifier: expected '{}', got '{}'",
                        entry.thread_id,
                        param_tid
                    );
                }

                let entry = pending.remove(approval_id).unwrap();
                let _ = entry.response_tx.send(decision);
                Ok(serde_json::json!({
                    "approval_id": approval_id,
                    "resolved": true,
                }))
            }

            "turn/deny" => {
                let approval_id = params
                    .get("approval_id")
                    .or_else(|| params.get("id"))
                    .and_then(|v| v.as_str())
                    .context("Missing 'approval_id'")?;
                let reason = params
                    .get("reason")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Denied by client");

                let mut pending = self.pending_approvals.lock().unwrap();
                let entry = pending.get(approval_id).ok_or_else(|| {
                    anyhow!("Approval request '{approval_id}' not found or already resolved")
                })?;

                if let Some(param_tid) = params.get("thread_id").and_then(|v| v.as_str())
                    && param_tid != entry.thread_id
                {
                    bail!(
                        "Conflicting thread identifier: expected '{}', got '{}'",
                        entry.thread_id,
                        param_tid
                    );
                }

                let entry = pending.remove(approval_id).unwrap();
                let _ = entry
                    .response_tx
                    .send(m31a_elicitation::ApprovalDecision::Deny {
                        reason: reason.to_string(),
                    });
                Ok(serde_json::json!({
                    "approval_id": approval_id,
                    "resolved": true,
                    "denied": true,
                }))
            }

            "turn/answer" | "question/respond" => {
                let question_id = params
                    .get("question_id")
                    .or_else(|| params.get("id"))
                    .and_then(|v| v.as_str())
                    .context("Missing 'question_id'")?;
                let answers: Vec<String> =
                    if let Some(arr) = params.get("answers").and_then(|v| v.as_array()) {
                        arr.iter()
                            .filter_map(|x| x.as_str().map(|s| s.to_string()))
                            .collect()
                    } else if let Some(s) = params.get("answer").and_then(|v| v.as_str()) {
                        vec![s.to_string()]
                    } else {
                        Vec::new()
                    };

                let mut pending = self.pending_questions.lock().unwrap();
                let entry = pending.get(question_id).ok_or_else(|| {
                    anyhow!("Question request '{question_id}' not found or already resolved")
                })?;

                if let Some(param_tid) = params.get("thread_id").and_then(|v| v.as_str())
                    && param_tid != entry.thread_id
                {
                    bail!(
                        "Conflicting thread identifier: expected '{}', got '{}'",
                        entry.thread_id,
                        param_tid
                    );
                }

                let entry = pending.remove(question_id).unwrap();
                let _ = entry.response_tx.send(answers.clone());
                Ok(serde_json::json!({
                    "question_id": question_id,
                    "resolved": true,
                    "answers": answers,
                }))
            }

            "turn/steer" => {
                let message = params
                    .get("message")
                    .or_else(|| params.get("prompt"))
                    .and_then(|v| v.as_str())
                    .context("Missing 'message' or 'prompt'")?;
                let turn_id = params.get("turn_id").and_then(|v| v.as_str());
                let thread_id = params.get("thread_id").and_then(|v| v.as_str());

                if turn_id.is_none() && thread_id.is_none() {
                    bail!(
                        "Missing target resource identifier: either 'turn_id' or 'thread_id' must be provided"
                    );
                }

                let active = self.active_turns.lock().unwrap();
                let handle = if let Some(tid) = turn_id {
                    let h = active
                        .get(tid)
                        .ok_or_else(|| anyhow!("Active turn '{tid}' not found"))?;
                    if let Some(thid) = thread_id
                        && h.thread_id != thid
                    {
                        bail!(
                            "Conflicting identifiers: turn '{tid}' belongs to thread '{}', got '{thid}'",
                            h.thread_id
                        );
                    }
                    h
                } else if let Some(thid) = thread_id {
                    active
                        .values()
                        .find(|h| h.thread_id == thid)
                        .ok_or_else(|| anyhow!("No active turn running for thread '{thid}'"))?
                } else {
                    bail!("Missing target identifier");
                };

                handle
                    .steer_tx
                    .send(message.to_string())
                    .map_err(|e| anyhow!("Failed to send steer message: {e}"))?;
                Ok(serde_json::json!({
                    "steered": true,
                    "turn_id": handle.turn_id,
                    "thread_id": handle.thread_id,
                }))
            }

            "turn/interrupt" => {
                let turn_id = params.get("turn_id").and_then(|v| v.as_str());
                let thread_id = params.get("thread_id").and_then(|v| v.as_str());

                if turn_id.is_none() && thread_id.is_none() {
                    bail!(
                        "Missing target resource identifier: either 'turn_id' or 'thread_id' must be provided"
                    );
                }

                let active = self.active_turns.lock().unwrap();
                let handle = if let Some(tid) = turn_id {
                    let h = active
                        .get(tid)
                        .ok_or_else(|| anyhow!("Active turn '{tid}' not found"))?;
                    if let Some(thid) = thread_id
                        && h.thread_id != thid
                    {
                        bail!(
                            "Conflicting identifiers: turn '{tid}' belongs to thread '{}', got '{thid}'",
                            h.thread_id
                        );
                    }
                    h
                } else if let Some(thid) = thread_id {
                    active
                        .values()
                        .find(|h| h.thread_id == thid)
                        .ok_or_else(|| anyhow!("No active turn running for thread '{thid}'"))?
                } else {
                    bail!("Missing target identifier");
                };

                handle.cancel_token.cancel();
                let cancelled_turn_id = handle.turn_id.clone();
                let cancelled_thread_id = handle.thread_id.clone();
                drop(active);

                self.notify(
                    "notification/turn_interrupted",
                    Some(serde_json::json!({
                        "turn_id": cancelled_turn_id,
                        "thread_id": cancelled_thread_id,
                    })),
                );

                Ok(serde_json::json!({
                    "interrupted": true,
                    "turn_id": cancelled_turn_id,
                    "thread_id": cancelled_thread_id,
                }))
            }

            "checkpoint/create" => {
                let thread_id = params
                    .get("thread_id")
                    .and_then(|v| v.as_str())
                    .context("Missing 'thread_id'")?;
                let label = params
                    .get("label")
                    .and_then(|v| v.as_str())
                    .context("Missing 'label'")?;
                let git_commit = params.get("git_commit").and_then(|v| v.as_str());
                let turn_index = params
                    .get("turn_index")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(1) as usize;
                let modified_files: Vec<String> = params
                    .get("modified_files")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|x| x.as_str().map(|s| s.to_string()))
                            .collect()
                    })
                    .unwrap_or_default();

                let record = self.storage.create_checkpoint(
                    thread_id,
                    label,
                    git_commit,
                    turn_index,
                    &modified_files,
                    None,
                )?;

                self.notify(
                    "notification/checkpoint_created",
                    Some(serde_json::json!({
                        "id": record.id,
                        "thread_id": record.thread_id,
                        "label": record.label,
                        "turn_index": record.turn_index,
                    })),
                );

                Ok(serde_json::to_value(record)?)
            }

            "checkpoint/list" => {
                let thread_id = params
                    .get("thread_id")
                    .and_then(|v| v.as_str())
                    .context("Missing 'thread_id'")?;
                let list = self.storage.list_checkpoints(thread_id)?;
                Ok(serde_json::to_value(list)?)
            }

            "checkpoint/restore" => {
                let checkpoint_id = params
                    .get("checkpoint_id")
                    .or_else(|| params.get("id"))
                    .and_then(|v| v.as_str())
                    .context("Missing 'checkpoint_id'")?;
                let revert_conv = params
                    .get("revert_conversation")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(true);

                let checkpoint = self
                    .storage
                    .get_checkpoint(checkpoint_id)?
                    .ok_or_else(|| anyhow!("Checkpoint '{checkpoint_id}' not found"))?;

                if let Some(param_tid) = params.get("thread_id").and_then(|v| v.as_str())
                    && param_tid != checkpoint.thread_id
                {
                    bail!(
                        "Conflicting thread identifier: checkpoint belongs to '{}', got '{param_tid}'",
                        checkpoint.thread_id
                    );
                }

                let record = self
                    .storage
                    .restore_checkpoint(checkpoint_id, revert_conv)?;

                self.notify(
                    "notification/checkpoint_restored",
                    Some(serde_json::json!({
                        "id": record.id,
                        "thread_id": record.thread_id,
                        "turn_index": record.turn_index,
                    })),
                );

                Ok(serde_json::to_value(record)?)
            }

            "locks/list" => {
                let locks = m31a_core::ExecutionLockManager::global().active_locks();
                Ok(serde_json::to_value(locks)?)
            }

            "locks/acquire" => {
                let path = params
                    .get("path")
                    .and_then(|v| v.as_str())
                    .context("Missing 'path'")?;
                let mode_str = params
                    .get("mode")
                    .and_then(|v| v.as_str())
                    .unwrap_or("read");
                let holder = params
                    .get("holder")
                    .and_then(|v| v.as_str())
                    .unwrap_or("app-client");
                let reason = params
                    .get("reason")
                    .and_then(|v| v.as_str())
                    .unwrap_or("remote request");
                let timeout_ms = params
                    .get("timeout_ms")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(5000);

                let mode = match mode_str {
                    "write" => m31a_core::LockMode::Write,
                    _ => m31a_core::LockMode::Read,
                };

                let token = m31a_core::ExecutionLockManager::global()
                    .acquire_leased(
                        std::path::Path::new(path),
                        mode,
                        holder,
                        reason,
                        std::time::Duration::from_millis(timeout_ms),
                    )
                    .await
                    .map_err(|e| anyhow!("{e}"))?;

                self.notify(
                    "notification/lock_acquired",
                    Some(serde_json::json!({
                        "token": token,
                        "path": path,
                        "mode": mode_str,
                        "holder": holder,
                    })),
                );

                Ok(serde_json::json!({ "token": token, "path": path, "mode": mode_str }))
            }

            "locks/release" => {
                let token = params
                    .get("token")
                    .or_else(|| params.get("lock_id"))
                    .and_then(|v| v.as_str())
                    .context("Missing 'token' or 'lock_id'")?;
                let released = m31a_core::ExecutionLockManager::global().release_leased(token);

                self.notify(
                    "notification/lock_released",
                    Some(serde_json::json!({
                        "token": token,
                        "released": released,
                    })),
                );

                Ok(serde_json::json!({ "token": token, "released": released }))
            }

            "session/compact" => {
                let thread_id = params
                    .get("thread_id")
                    .and_then(|v| v.as_str())
                    .context("Missing 'thread_id'")?;
                let keep_recent = params
                    .get("keep_recent_turns")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(6) as usize;

                let mut session = m31a_core::Session::new(&self.config.model, ".");
                let items = self.storage.list_items(thread_id)?;
                for (item_type, content, _) in items {
                    match item_type.as_str() {
                        "user" => session.push_message(m31a_core::ChatMessage::user(content)),
                        "assistant" => session
                            .push_message(m31a_core::ChatMessage::assistant(Some(content), None)),
                        _ => {}
                    }
                }

                let summary = session.compact_history(keep_recent);
                Ok(serde_json::json!({
                    "thread_id": thread_id,
                    "compacted": summary.is_some(),
                    "summary": summary,
                }))
            }

            "board/post" => {
                let channel = params
                    .get("channel")
                    .and_then(|v| v.as_str())
                    .unwrap_or("general");
                let author = params
                    .get("author")
                    .and_then(|v| v.as_str())
                    .unwrap_or("agent");
                let body = params
                    .get("body")
                    .and_then(|v| v.as_str())
                    .context("Missing 'body'")?;
                let payload = params
                    .get("payload")
                    .cloned()
                    .unwrap_or(serde_json::json!({}));

                let post_id = self
                    .storage
                    .post_to_board("default", channel, author, body, payload)?;
                Ok(serde_json::json!({ "post_id": post_id }))
            }

            "board/list" => {
                let channel = params
                    .get("channel")
                    .and_then(|v| v.as_str())
                    .unwrap_or("general");
                let limit = params.get("limit").and_then(|v| v.as_u64()).unwrap_or(50) as usize;
                let posts = self.storage.list_posts("default", channel, limit)?;
                Ok(serde_json::to_value(posts)?)
            }

            "goal/set" | "session/goal/set" => {
                let thread_id = params
                    .get("thread_id")
                    .or_else(|| params.get("id"))
                    .and_then(|v| v.as_str())
                    .context("Missing 'thread_id'")?;
                let objective = params
                    .get("objective")
                    .and_then(|v| v.as_str())
                    .context("Missing 'objective'")?;
                let budget = params.get("budget").and_then(|v| v.as_u64());
                let goal = self.storage.set_goal(thread_id, objective, budget)?;
                Ok(serde_json::to_value(goal)?)
            }

            "goal/get" | "session/goal/get" => {
                let thread_id = params
                    .get("thread_id")
                    .or_else(|| params.get("id"))
                    .and_then(|v| v.as_str())
                    .context("Missing 'thread_id'")?;
                let goal = self.storage.get_goal(thread_id)?;
                Ok(serde_json::to_value(goal)?)
            }

            "goal/clear" | "session/goal/clear" => {
                let thread_id = params
                    .get("thread_id")
                    .or_else(|| params.get("id"))
                    .and_then(|v| v.as_str())
                    .context("Missing 'thread_id'")?;
                self.storage.clear_goal(thread_id)?;
                Ok(serde_json::json!({ "cleared": true, "thread_id": thread_id }))
            }

            "goal/update" | "session/goal/update" => {
                let thread_id = params
                    .get("thread_id")
                    .or_else(|| params.get("id"))
                    .and_then(|v| v.as_str())
                    .context("Missing 'thread_id'")?;
                let status = params
                    .get("status")
                    .and_then(|v| v.as_str())
                    .context("Missing 'status'")?;
                self.storage.update_goal_status(thread_id, status)?;
                Ok(serde_json::json!({ "updated": true, "thread_id": thread_id, "status": status }))
            }

            "inbox/enqueue" | "session/inbox/enqueue" => {
                let thread_id = params
                    .get("thread_id")
                    .or_else(|| params.get("id"))
                    .and_then(|v| v.as_str())
                    .context("Missing 'thread_id'")?;
                let kind = params
                    .get("kind")
                    .and_then(|v| v.as_str())
                    .unwrap_or("prompt");
                let payload = params
                    .get("payload")
                    .or_else(|| params.get("prompt"))
                    .and_then(|v| v.as_str())
                    .context("Missing 'payload'")?;
                let item = self.storage.enqueue_inbox(thread_id, kind, payload)?;
                Ok(serde_json::to_value(item)?)
            }

            "inbox/dequeue" | "session/inbox/dequeue" => {
                let thread_id = params
                    .get("thread_id")
                    .or_else(|| params.get("id"))
                    .and_then(|v| v.as_str())
                    .context("Missing 'thread_id'")?;
                let item = self.storage.dequeue_inbox(thread_id)?;
                Ok(serde_json::to_value(item)?)
            }

            "inbox/list" | "session/inbox/list" => {
                let thread_id = params
                    .get("thread_id")
                    .or_else(|| params.get("id"))
                    .and_then(|v| v.as_str())
                    .context("Missing 'thread_id'")?;
                let items = self.storage.list_inbox(thread_id)?;
                Ok(serde_json::to_value(items)?)
            }

            "claims/active" | "session/claims/active" => {
                let thread_id = params
                    .get("thread_id")
                    .or_else(|| params.get("id"))
                    .and_then(|v| v.as_str())
                    .context("Missing 'thread_id'")?;
                let claim = self.storage.get_active_claim(thread_id)?;
                Ok(serde_json::to_value(claim)?)
            }

            "claims/recover" | "session/claims/recover" => {
                let recovered = self.storage.recover_crashed_claims()?;
                Ok(serde_json::to_value(recovered)?)
            }

            // Autonomous Workflow & Speculative Execution RPC endpoints
            "workflow/create" => {
                let ws_str = params
                    .get("workspace_root")
                    .and_then(|v| v.as_str())
                    .context("Missing 'workspace_root'")?;
                let objective = params
                    .get("objective")
                    .and_then(|v| v.as_str())
                    .context("Missing 'objective'")?;
                let workflow_type = params
                    .get("workflow_type")
                    .and_then(|v| v.as_str())
                    .unwrap_or("autonomous");
                let thread_id = params.get("thread_id").and_then(|v| v.as_str());
                let cfg: Option<WorkflowExecutionConfig> = params
                    .get("config")
                    .and_then(|v| serde_json::from_value(v.clone()).ok());

                let ws_path = PathBuf::from(ws_str);
                let orch = self.workflow_orchestrator(Some(&ws_path));
                let run =
                    orch.create_workflow(&ws_path, objective, workflow_type, thread_id, cfg)?;

                let auto_plan = params
                    .get("auto_plan")
                    .or_else(|| params.get("plan_now"))
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);

                if auto_plan {
                    let _plan = orch.generate_plan(&run.id).await?;
                    let auto_execute = params
                        .get("auto_execute")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);
                    if auto_execute {
                        let completed = orch.execute_workflow(&run.id).await?;
                        return Ok(serde_json::to_value(completed)?);
                    }
                }

                Ok(serde_json::to_value(run)?)
            }

            "workflow/status" => {
                let wf_id = params
                    .get("workflow_id")
                    .or_else(|| params.get("id"))
                    .and_then(|v| v.as_str())
                    .context("Missing 'workflow_id'")?;
                let store = self.storage.workflow_store();
                let run = store
                    .get_workflow(wf_id)?
                    .context(format!("Workflow '{}' not found", wf_id))?;
                Ok(serde_json::to_value(run)?)
            }

            "workflow/inspect" => {
                let wf_id = params
                    .get("workflow_id")
                    .or_else(|| params.get("id"))
                    .and_then(|v| v.as_str())
                    .context("Missing 'workflow_id'")?;
                let store = self.storage.workflow_store();
                let run = store
                    .get_workflow(wf_id)?
                    .context(format!("Workflow '{}' not found", wf_id))?;
                let plan = store.get_workflow_plan(wf_id, None)?;
                let tasks = store.list_tasks(wf_id)?;
                let artifacts = store.list_artifacts(wf_id)?;

                Ok(serde_json::json!({
                    "workflow": run,
                    "plan": plan,
                    "tasks": tasks,
                    "artifacts": artifacts,
                }))
            }

            "workflow/resume" => {
                let wf_id = params
                    .get("workflow_id")
                    .or_else(|| params.get("id"))
                    .and_then(|v| v.as_str())
                    .context("Missing 'workflow_id'")?;
                let store = self.storage.workflow_store();
                let run = store
                    .get_workflow(wf_id)?
                    .context(format!("Workflow '{}' not found", wf_id))?;

                let orch = self.workflow_orchestrator(Some(&run.workspace_root));
                let resumed = orch.resume_workflow(wf_id).await?;
                Ok(serde_json::to_value(resumed)?)
            }

            "workflow/cancel" => {
                let wf_id = params
                    .get("workflow_id")
                    .or_else(|| params.get("id"))
                    .and_then(|v| v.as_str())
                    .context("Missing 'workflow_id'")?;
                let reason = params
                    .get("reason")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Cancelled via App Server RPC request");

                let orch = self.workflow_orchestrator(None);
                orch.cancel_workflow(wf_id, reason).await?;

                let store = self.storage.workflow_store();
                let run = store
                    .get_workflow(wf_id)?
                    .context(format!("Workflow '{}' not found", wf_id))?;
                Ok(serde_json::to_value(run)?)
            }

            "workflow/events" => {
                let wf_id = params
                    .get("workflow_id")
                    .or_else(|| params.get("id"))
                    .and_then(|v| v.as_str())
                    .context("Missing 'workflow_id'")?;
                let since = params
                    .get("since_epoch_ms")
                    .or_else(|| params.get("since"))
                    .and_then(|v| v.as_i64());

                let store = self.storage.workflow_store();
                let events = store.list_events(wf_id, since)?;
                Ok(serde_json::to_value(events)?)
            }

            "workflow/artifacts" => {
                let wf_id = params
                    .get("workflow_id")
                    .or_else(|| params.get("id"))
                    .and_then(|v| v.as_str())
                    .context("Missing 'workflow_id'")?;
                let store = self.storage.workflow_store();
                let artifacts = store.list_artifacts(wf_id)?;
                Ok(serde_json::to_value(artifacts)?)
            }

            "workflow/task" => {
                let wf_id = params
                    .get("workflow_id")
                    .and_then(|v| v.as_str())
                    .context("Missing 'workflow_id'")?;
                let task_id = params
                    .get("task_id")
                    .or_else(|| params.get("id"))
                    .and_then(|v| v.as_str())
                    .context("Missing 'task_id'")?;
                let store = self.storage.workflow_store();
                let task = store.get_task(wf_id, task_id)?.context(format!(
                    "Task '{}' in workflow '{}' not found",
                    task_id, wf_id
                ))?;
                Ok(serde_json::to_value(task)?)
            }

            "workflow/approve" => {
                let wf_id = params
                    .get("workflow_id")
                    .or_else(|| params.get("id"))
                    .and_then(|v| v.as_str())
                    .context("Missing 'workflow_id'")?;
                let approval_id = params
                    .get("approval_id")
                    .or_else(|| params.get("task_id"))
                    .and_then(|v| v.as_str())
                    .context("Missing 'approval_id'")?;
                let actor = params
                    .get("actor")
                    .and_then(|v| v.as_str())
                    .unwrap_or("app-server-rpc");
                let reason = params.get("reason").and_then(|v| v.as_str());

                let store = self.storage.workflow_store();
                let run = store
                    .get_workflow(wf_id)?
                    .context(format!("Workflow '{}' not found", wf_id))?;

                let orch = self.workflow_orchestrator(Some(&run.workspace_root));
                orch.approve_integration(wf_id, approval_id, actor, reason)
                    .await?;

                self.notify(
                    "notification/workflow_state_changed",
                    Some(serde_json::json!({
                        "workflow_id": wf_id,
                        "approval_id": approval_id,
                        "action": "approved",
                        "actor": actor,
                    })),
                );

                Ok(serde_json::json!({
                    "success": true,
                    "workflow_id": wf_id,
                    "approval_id": approval_id,
                    "status": "approved",
                }))
            }

            "workflow/reject" => {
                let wf_id = params
                    .get("workflow_id")
                    .or_else(|| params.get("id"))
                    .and_then(|v| v.as_str())
                    .context("Missing 'workflow_id'")?;
                let approval_id = params
                    .get("approval_id")
                    .or_else(|| params.get("task_id"))
                    .and_then(|v| v.as_str())
                    .context("Missing 'approval_id'")?;
                let actor = params
                    .get("actor")
                    .and_then(|v| v.as_str())
                    .unwrap_or("app-server-rpc");
                let reason = params
                    .get("reason")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Rejected via App Server RPC");

                let store = self.storage.workflow_store();
                let run = store
                    .get_workflow(wf_id)?
                    .context(format!("Workflow '{}' not found", wf_id))?;

                let orch = self.workflow_orchestrator(Some(&run.workspace_root));
                orch.reject_integration(wf_id, approval_id, actor, Some(reason))
                    .await?;

                self.notify(
                    "notification/workflow_state_changed",
                    Some(serde_json::json!({
                        "workflow_id": wf_id,
                        "approval_id": approval_id,
                        "action": "rejected",
                        "actor": actor,
                        "reason": reason,
                    })),
                );

                Ok(serde_json::json!({
                    "success": true,
                    "workflow_id": wf_id,
                    "approval_id": approval_id,
                    "status": "rejected",
                }))
            }

            "workflow/approvals" => {
                let wf_id = params
                    .get("workflow_id")
                    .or_else(|| params.get("id"))
                    .and_then(|v| v.as_str())
                    .context("Missing 'workflow_id'")?;
                let store = self.storage.workflow_store();
                let approvals = store.list_approvals(wf_id)?;
                Ok(serde_json::to_value(approvals)?)
            }

            "speculative/evaluations" => {
                if let Some(wf_id) = params.get("workflow_id").and_then(|v| v.as_str()) {
                    let store = self.storage.workflow_store();
                    let artifacts = store.list_artifacts(wf_id)?;
                    let spec_artifacts: Vec<_> = artifacts
                        .into_iter()
                        .filter(|a| a.artifact_type == "speculative_report")
                        .collect();
                    return Ok(serde_json::to_value(spec_artifacts)?);
                }

                let ws_str = params
                    .get("workspace_root")
                    .and_then(|v| v.as_str())
                    .context("Missing 'workspace_root' or 'workflow_id'")?;
                let mission = params
                    .get("mission")
                    .or_else(|| params.get("objective"))
                    .and_then(|v| v.as_str())
                    .context("Missing 'mission'")?;
                let timeout_secs = params
                    .get("timeout_seconds")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(300);
                let auto_promote = params
                    .get("auto_promote")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);

                let candidates: Vec<SpeculativeBranchConfig> =
                    if let Some(cands_val) = params.get("candidates") {
                        serde_json::from_value(cands_val.clone()).unwrap_or_else(|_| {
                            SpeculativeArena::generate_diverse_hypotheses(mission, 3)
                        })
                    } else {
                        SpeculativeArena::generate_diverse_hypotheses(mission, 3)
                    };

                let orch = self.workflow_orchestrator(Some(std::path::Path::new(ws_str)));
                let executor: Option<Arc<dyn m31a_agent_engine::speculative::CandidateExecutor>> =
                    m31a_agent_engine::speculative::create_candidate_executor(
                        orch.base_client().cloned(),
                        orch.tools().cloned(),
                        Some(self.storage.clone()),
                        orch.exec_policy().cloned(),
                        Some(self.hooks.clone()),
                    )
                    .ok();

                let options = m31a_agent_engine::speculative::SpeculateOptions {
                    workspace_root: std::path::PathBuf::from(ws_str),
                    mission: mission.to_string(),
                    candidates,
                    timeout_per_candidate: std::time::Duration::from_secs(timeout_secs),
                    auto_promote_winner: auto_promote,
                    require_approval_for_promotion: false,
                    max_concurrency: 4,
                    executor,
                };

                let report = SpeculativeArena::compete_with_options(options).await?;

                Ok(serde_json::to_value(report)?)
            }

            _ => Err(anyhow!("Method not found: {}", method)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_app_server_initialize_and_methods() {
        let config = Arc::new(Config {
            model: "nvidia/nemotron-3.5-lightning-30b-a3b".to_string(),
            model_provider: Some("nim".to_string()),
            api_key: m31a_core::resolve_stored_api_key()
                .or_else(|| Some("nvapi-runtime-integration-token".to_string())),
            ..Default::default()
        });
        let storage = Storage::open_in_memory().unwrap();
        let tools = Arc::new(ToolRegistry::new());
        let server = AppServer::new(config, storage, tools).unwrap();

        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(serde_json::json!(1)),
            method: "initialize".to_string(),
            params: None,
        };

        // Public method works without identity
        let res = server.handle_request(req).await;
        assert!(res.error.is_none());
        let val = res.result.unwrap();
        assert_eq!(val.get("server_name").unwrap(), "m31a-app-server");

        // Local methods succeed without authentication tokens in local-first mode
        let req_models = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(serde_json::json!(2)),
            method: "models/list".to_string(),
            params: None,
        };
        let res_models = server.handle_request(req_models.clone()).await;
        assert!(res_models.error.is_none());

        // Unknown methods fail closed
        let req_unknown = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(serde_json::json!(99)),
            method: "non_existent/method".to_string(),
            params: None,
        };
        let res_unknown = server.handle_request(req_unknown).await;
        assert!(res_unknown.error.is_some());

        // Test thread lifecycle: untrusted workspace fails closed
        let req_untrusted = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(serde_json::json!(3)),
            method: "thread/create".to_string(),
            params: Some(serde_json::json!({"cwd": "/tmp"})),
        };
        let res_untrusted = server.handle_request(req_untrusted).await;
        assert!(
            res_untrusted.error.is_some(),
            "Untrusted workspace without authorization must fail closed"
        );

        // Legitimate workspace trust grant succeeds
        let req_grant = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(serde_json::json!(4)),
            method: "workspace/trust_grant".to_string(),
            params: Some(serde_json::json!({"path": "/tmp", "level": "session_only"})),
        };
        let res_grant = server.handle_request(req_grant).await;
        assert!(res_grant.error.is_none());

        // Thread creation now succeeds in authorized workspace
        let req_thread = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(serde_json::json!(5)),
            method: "thread/create".to_string(),
            params: Some(serde_json::json!({"cwd": "/tmp"})),
        };
        let res_thread = server.handle_request(req_thread).await;
        assert!(res_thread.error.is_none());
        let thread_id = res_thread
            .result
            .unwrap()
            .get("id")
            .unwrap()
            .as_str()
            .unwrap()
            .to_string();

        // Fork thread
        let req_fork = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(serde_json::json!(4)),
            method: "thread/fork".to_string(),
            params: Some(serde_json::json!({"thread_id": thread_id, "title": "Forked Branch"})),
        };
        let res_fork = server.handle_request(req_fork).await;
        assert!(res_fork.error.is_none());

        // Test checkpoint creation and notification
        let mut notif_rx = server.subscribe();
        let req_cp = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(serde_json::json!(5)),
            method: "checkpoint/create".to_string(),
            params: Some(serde_json::json!({
                "thread_id": thread_id,
                "label": "AppServer Snapshot",
                "turn_index": 1,
                "modified_files": ["crates/lib.rs"]
            })),
        };
        let res_cp = server.handle_request(req_cp).await;
        assert!(res_cp.error.is_none());
        let cp_val = res_cp.result.unwrap();
        let cp_id = cp_val.get("id").unwrap().as_str().unwrap().to_string();

        // Verify broadcast notification arrived
        let notif = notif_rx.try_recv().unwrap();
        assert_eq!(notif.method, "notification/checkpoint_created");
        assert_eq!(
            notif.params.unwrap().get("id").unwrap().as_str().unwrap(),
            cp_id
        );

        // Test checkpoint list
        let req_list_cp = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(serde_json::json!(6)),
            method: "checkpoint/list".to_string(),
            params: Some(serde_json::json!({ "thread_id": thread_id })),
        };
        let res_list_cp = server.handle_request(req_list_cp).await;
        assert!(res_list_cp.error.is_none());
        let list_val = res_list_cp.result.unwrap();
        assert_eq!(list_val.as_array().unwrap().len(), 1);

        // Test locks list
        let req_locks = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(serde_json::json!(7)),
            method: "locks/list".to_string(),
            params: None,
        };
        let res_locks = server.handle_request(req_locks).await;
        assert!(res_locks.error.is_none());

        // Test thread sections RPC
        let req_sec_create = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(serde_json::json!(8)),
            method: "thread/section/create".to_string(),
            params: Some(serde_json::json!({
                "name": "Audit Reviews",
                "color": "#00ff00",
                "icon": "shield"
            })),
        };
        let res_sec_create = server.handle_request(req_sec_create).await;
        assert!(res_sec_create.error.is_none());
        let sec_id = res_sec_create
            .result
            .unwrap()
            .get("id")
            .unwrap()
            .as_str()
            .unwrap()
            .to_string();

        let req_sec_list = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(serde_json::json!(9)),
            method: "thread/section/list".to_string(),
            params: None,
        };
        let res_sec_list = server.handle_request(req_sec_list).await;
        assert!(res_sec_list.error.is_none());
        assert!(!res_sec_list.result.unwrap().as_array().unwrap().is_empty());

        let req_sec_move = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(serde_json::json!(10)),
            method: "thread/section/move".to_string(),
            params: Some(serde_json::json!({
                "thread_id": thread_id,
                "section_id": sec_id
            })),
        };
        let res_sec_move = server.handle_request(req_sec_move).await;
        assert!(res_sec_move.error.is_none());
        assert_eq!(res_sec_move.result.unwrap().get("moved").unwrap(), true);

        // Test interactive approval flow
        let (resp_tx, resp_rx) = oneshot::channel();
        let approval = m31a_agent_engine::PendingApproval {
            tool_call: m31a_core::types::ToolCall {
                id: "call_1".to_string(),
                call_type: "function".to_string(),
                function: m31a_core::types::FunctionCall {
                    name: "exec_command".to_string(),
                    arguments: "{\"command\": \"cargo test\"}".to_string(),
                },
            },
            reason: "Policy requires confirmation".to_string(),
            command: Some("cargo test".to_string()),
        };
        server.bridge_engine_event(
            "turn_test_1",
            &thread_id,
            EngineEvent::ApprovalRequested {
                approval,
                response_tx: resp_tx,
            },
        );

        let notif_appr = notif_rx.try_recv().unwrap();
        assert_eq!(notif_appr.method, "notification/approval_requested");
        let appr_id = notif_appr
            .params
            .as_ref()
            .unwrap()
            .get("approval_id")
            .unwrap()
            .as_str()
            .unwrap()
            .to_string();

        // Client approves the turn
        let req_approve = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(serde_json::json!(11)),
            method: "turn/approve".to_string(),
            params: Some(serde_json::json!({
                "thread_id": thread_id,
                "approval_id": appr_id,
                "decision": "allow",
            })),
        };
        let res_approve = server.handle_request(req_approve).await;
        assert!(res_approve.error.is_none());
        assert_eq!(res_approve.result.unwrap().get("resolved").unwrap(), true);

        let decision = resp_rx.await.unwrap();
        assert_eq!(decision, m31a_elicitation::ApprovalDecision::AllowOnce);

        // Test question/answer flow
        let (q_tx, q_rx) = oneshot::channel();
        let question = m31a_elicitation::QuestionItem {
            id: "q1".to_string(),
            question: "Select architecture style".to_string(),
            options: vec!["REST".to_string(), "gRPC".to_string()],
            is_multi_select: false,
            selected_indices: std::collections::HashSet::new(),
            cursor_idx: 0,
            custom_input: String::new(),
            is_custom_focused: false,
        };
        server.bridge_engine_event(
            "turn_test_2",
            &thread_id,
            EngineEvent::QuestionRequested {
                question,
                response_tx: q_tx,
            },
        );
        let notif_q = notif_rx.try_recv().unwrap();
        assert_eq!(notif_q.method, "notification/question_requested");
        let q_id = notif_q
            .params
            .as_ref()
            .unwrap()
            .get("question_id")
            .unwrap()
            .as_str()
            .unwrap()
            .to_string();

        let req_answer = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(serde_json::json!(12)),
            method: "turn/answer".to_string(),
            params: Some(serde_json::json!({
                "thread_id": thread_id,
                "question_id": q_id,
                "answers": ["gRPC"],
            })),
        };
        let res_answer = server.handle_request(req_answer).await;
        assert!(res_answer.error.is_none());
        assert_eq!(res_answer.result.unwrap().get("resolved").unwrap(), true);

        let answers = q_rx.await.unwrap();
        assert_eq!(answers, vec!["gRPC".to_string()]);

        // Test steer & interrupt on active turn
        server
            .storage
            .save_turn("turn_active_1", &thread_id, 1, "in_progress")
            .unwrap();
        let (steer_tx, mut steer_rx) = mpsc::unbounded_channel();
        let cancel_token = CancellationToken::new();
        server.active_turns.lock().unwrap().insert(
            "turn_active_1".to_string(),
            ActiveTurnHandle {
                thread_id: thread_id.clone(),
                turn_id: "turn_active_1".to_string(),
                steer_tx,
                cancel_token: cancel_token.clone(),
            },
        );

        let req_steer = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(serde_json::json!(13)),
            method: "turn/steer".to_string(),
            params: Some(serde_json::json!({
                "thread_id": thread_id,
                "turn_id": "turn_active_1",
                "message": "Focus on security",
            })),
        };
        let res_steer = server.handle_request(req_steer).await;
        assert!(res_steer.error.is_none());
        assert_eq!(steer_rx.recv().await.unwrap(), "Focus on security");

        let req_interrupt = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(serde_json::json!(14)),
            method: "turn/interrupt".to_string(),
            params: Some(serde_json::json!({
                "thread_id": thread_id,
                "turn_id": "turn_active_1",
            })),
        };
        let res_interrupt = server.handle_request(req_interrupt).await;
        assert!(res_interrupt.error.is_none());
        assert!(cancel_token.is_cancelled());

        // Test goal endpoints
        let req_goal = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(serde_json::json!(15)),
            method: "goal/set".to_string(),
            params: Some(serde_json::json!({
                "thread_id": thread_id,
                "objective": "Build M31A App Server RPC parity",
                "budget": 50000,
            })),
        };
        let res_goal = server.handle_request(req_goal).await;
        assert!(res_goal.error.is_none());

        let req_get_goal = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(serde_json::json!(16)),
            method: "goal/get".to_string(),
            params: Some(serde_json::json!({
                "thread_id": thread_id,
            })),
        };
        let res_get_goal = server.handle_request(req_get_goal).await;
        assert!(res_get_goal.error.is_none());
        assert_eq!(
            res_get_goal.result.unwrap().get("objective").unwrap(),
            "Build M31A App Server RPC parity"
        );

        // Test inbox endpoints
        let req_inbox = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(serde_json::json!(17)),
            method: "inbox/enqueue".to_string(),
            params: Some(serde_json::json!({
                "thread_id": thread_id,
                "kind": "prompt",
                "payload": "Deferred user message",
            })),
        };
        let res_inbox = server.handle_request(req_inbox).await;
        assert!(res_inbox.error.is_none());

        let req_deq = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(serde_json::json!(18)),
            method: "inbox/dequeue".to_string(),
            params: Some(serde_json::json!({
                "thread_id": thread_id,
            })),
        };
        let res_deq = server.handle_request(req_deq).await;
        assert!(res_deq.error.is_none());
        assert_eq!(
            res_deq.result.unwrap().get("payload").unwrap(),
            "Deferred user message"
        );
    }

    #[tokio::test]
    async fn test_workflow_rpc_endpoints() {
        let config = Arc::new(Config {
            model: "nvidia/nemotron-3.5-lightning-30b-a3b".to_string(),
            model_provider: Some("nim".to_string()),
            ..Default::default()
        });
        let storage = Storage::open_in_memory().unwrap();
        let tools = Arc::new(ToolRegistry::new());
        let server = AppServer::new(config, storage.clone(), tools).unwrap();

        let temp_dir = tempfile::tempdir().unwrap();
        let ws_path = temp_dir.path().to_str().unwrap().to_string();

        // 1. workflow/create
        let req_create = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(serde_json::json!(1)),
            method: "workflow/create".to_string(),
            params: Some(serde_json::json!({
                "workspace_root": ws_path,
                "objective": "Build high throughput cache",
                "workflow_type": "autonomous",
            })),
        };
        let res_create = server.handle_request(req_create).await;
        assert!(
            res_create.error.is_none(),
            "workflow/create should succeed: {:?}",
            res_create.error
        );
        let run_val = res_create.result.unwrap();
        let wf_id = run_val.get("id").unwrap().as_str().unwrap().to_string();
        assert_eq!(run_val.get("state").unwrap(), "created");

        // 2. workflow/status
        let req_status = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(serde_json::json!(2)),
            method: "workflow/status".to_string(),
            params: Some(serde_json::json!({ "workflow_id": wf_id })),
        };
        let res_status = server.handle_request(req_status).await;
        assert!(res_status.error.is_none());
        assert_eq!(res_status.result.unwrap().get("id").unwrap(), &wf_id);

        // 3. workflow/inspect
        let req_inspect = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(serde_json::json!(3)),
            method: "workflow/inspect".to_string(),
            params: Some(serde_json::json!({ "workflow_id": wf_id })),
        };
        let res_inspect = server.handle_request(req_inspect).await;
        assert!(res_inspect.error.is_none());
        let inspect_val = res_inspect.result.unwrap();
        assert!(inspect_val.get("workflow").is_some());
        assert!(inspect_val.get("tasks").is_some());

        // 4. workflow/events
        let req_events = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(serde_json::json!(4)),
            method: "workflow/events".to_string(),
            params: Some(serde_json::json!({ "workflow_id": wf_id })),
        };
        let res_events = server.handle_request(req_events).await;
        assert!(res_events.error.is_none());
        let events = res_events.result.unwrap().as_array().unwrap().clone();
        assert!(
            !events.is_empty(),
            "Should record initial workflow created event"
        );

        // 5. workflow/artifacts
        let req_artifacts = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(serde_json::json!(5)),
            method: "workflow/artifacts".to_string(),
            params: Some(serde_json::json!({ "workflow_id": wf_id })),
        };
        let res_artifacts = server.handle_request(req_artifacts).await;
        assert!(res_artifacts.error.is_none());

        // 6. speculative/evaluations with workflow_id
        let req_spec = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(serde_json::json!(6)),
            method: "speculative/evaluations".to_string(),
            params: Some(serde_json::json!({ "workflow_id": wf_id })),
        };
        let res_spec = server.handle_request(req_spec).await;
        assert!(res_spec.error.is_none());

        // 7. workflow/cancel
        let req_cancel = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(serde_json::json!(7)),
            method: "workflow/cancel".to_string(),
            params: Some(serde_json::json!({
                "workflow_id": wf_id,
                "reason": "Integration test shutdown",
            })),
        };
        let res_cancel = server.handle_request(req_cancel).await;
        assert!(res_cancel.error.is_none());
        assert_eq!(
            res_cancel.result.unwrap().get("state").unwrap(),
            "cancelled"
        );
    }
}
