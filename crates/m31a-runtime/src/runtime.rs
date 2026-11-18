//! Sovereign M31A application runtime controller.
//!
//! Exposes strongly-typed public APIs for headless runtime initialization, turn execution,
//! approval and question routing, model switching, presentation-independent event streaming,
//! and graceful teardown.

use crate::config::RuntimeConfig;
use crate::context::{ActiveTurnHandle, PendingApprovalEntry, PendingQuestionEntry, TurnRequest};
use crate::error::RuntimeError;
use crate::events::RuntimeEvent;
use crate::lifecycle::{RuntimeState, ShutdownReport};
use crate::services::RuntimeServices;
use chrono::Utc;
use m31a_agent_engine::{AgentEngine, EngineEvent};
use m31a_core::LocalEncryptedVault;
use m31a_core::features::FeatureRegistry;
use m31a_core::prompts::CollaborationMode;
use m31a_core::types::ChatMessage;
use m31a_guardian::GuardianReviewer;
use m31a_hooks::HookRuntime;
use m31a_mcp::McpManager;
use m31a_proc::ProcessManager;
use m31a_provider::ProviderClient;
use m31a_provider::models::ModelInfo;
use m31a_sandbox::SandboxRunner;
use m31a_storage::Storage;
use m31a_tools::ToolRegistry;
use m31a_trust::TrustStore;
use m31a_watcher::WorkspaceWatcher;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, RwLock};
use std::time::Duration;
use tokio::sync::{broadcast, mpsc, Mutex as AsyncMutex};
use tokio_util::sync::CancellationToken;

/// Authoritative central application runtime for M31A.
#[derive(Clone)]
pub struct M31aRuntime {
    config: Arc<RuntimeConfig>,
    services: RuntimeServices,
    available_models: Arc<RwLock<Vec<ModelInfo>>>,
    active_model: Arc<RwLock<String>>,
    state: Arc<RwLock<RuntimeState>>,
    admission_lock: Arc<Mutex<()>>,
    shutdown_report: Arc<AsyncMutex<Option<ShutdownReport>>>,
    active_turns: Arc<Mutex<HashMap<String, ActiveTurnHandle>>>,
    pending_approvals: Arc<Mutex<HashMap<String, PendingApprovalEntry>>>,
    pending_questions: Arc<Mutex<HashMap<String, PendingQuestionEntry>>>,
    event_broadcaster: broadcast::Sender<RuntimeEvent>,
    cancel_token: CancellationToken,
}

/// Ensures turn registry and elicitation entries are cleared when a turn task exits.
struct TurnCleanupGuard {
    turn_id: String,
    active_turns: Arc<Mutex<HashMap<String, ActiveTurnHandle>>>,
    pending_approvals: Arc<Mutex<HashMap<String, PendingApprovalEntry>>>,
    pending_questions: Arc<Mutex<HashMap<String, PendingQuestionEntry>>>,
}

impl Drop for TurnCleanupGuard {
    fn drop(&mut self) {
        if let Ok(mut turns) = self.active_turns.lock() {
            turns.remove(&self.turn_id);
        }
        if let Ok(mut approvals) = self.pending_approvals.lock() {
            approvals.retain(|_, entry| entry.turn_id != self.turn_id);
        }
        if let Ok(mut questions) = self.pending_questions.lock() {
            questions.retain(|_, entry| entry.turn_id != self.turn_id);
        }
    }
}

impl M31aRuntime {
    /// Internal constructor invoked by `RuntimeBuilder`.
    pub(crate) fn new_internal(
        config: RuntimeConfig,
        services: RuntimeServices,
        available_models: Vec<ModelInfo>,
    ) -> Self {
        let (event_broadcaster, _) = broadcast::channel(2048);
        let active_model = config.core.model.clone();
        let active_turns = Arc::new(Mutex::new(HashMap::new()));
        let pending_approvals = Arc::new(Mutex::new(HashMap::new()));
        let pending_questions = Arc::new(Mutex::new(HashMap::new()));
        let cancel_token = CancellationToken::new();

        Self {
            config: Arc::new(config),
            services,
            available_models: Arc::new(RwLock::new(available_models)),
            active_model: Arc::new(RwLock::new(active_model)),
            state: Arc::new(RwLock::new(RuntimeState::Ready)),
            admission_lock: Arc::new(Mutex::new(())),
            shutdown_report: Arc::new(AsyncMutex::new(None)),
            active_turns,
            pending_approvals,
            pending_questions,
            event_broadcaster,
            cancel_token,
        }
    }

    /// Current lifecycle state of the runtime.
    pub fn state(&self) -> RuntimeState {
        *self.state.read().unwrap()
    }

    /// Access the validated runtime configuration.
    pub fn config(&self) -> &RuntimeConfig {
        &self.config
    }

    /// Access the underlying container of shared runtime services.
    pub fn services(&self) -> &RuntimeServices {
        &self.services
    }

    /// Provider client instance.
    pub fn provider(&self) -> Arc<ProviderClient> {
        Arc::clone(&self.services.provider.read().unwrap())
    }

    /// SQLite storage service handle.
    pub fn storage(&self) -> &Storage {
        &self.services.storage
    }

    /// Tool registry handle.
    pub fn tools(&self) -> Arc<ToolRegistry> {
        Arc::clone(&self.services.tools)
    }

    /// Sandbox runner handle.
    pub fn sandbox(&self) -> Arc<SandboxRunner> {
        Arc::clone(&self.services.sandbox)
    }

    /// Guardian reviewer handle.
    pub fn guardian(&self) -> Arc<GuardianReviewer> {
        Arc::clone(&self.services.guardian.read().unwrap())
    }

    /// Hook runtime handle.
    pub fn hooks(&self) -> Arc<HookRuntime> {
        Arc::clone(&self.services.hooks)
    }

    /// MCP manager handle.
    pub fn mcp_manager(&self) -> Arc<McpManager> {
        Arc::clone(&self.services.mcp_manager)
    }

    /// Process supervisor handle.
    pub fn proc_manager(&self) -> Arc<ProcessManager> {
        Arc::clone(&self.services.proc_manager)
    }

    /// Optional workspace watcher handle.
    pub fn watcher(&self) -> Option<Arc<WorkspaceWatcher>> {
        self.services.watcher.as_ref().map(Arc::clone)
    }

    /// Persistent workspace trust store handle.
    pub fn trust_store(&self) -> Arc<Mutex<TrustStore>> {
        Arc::clone(&self.services.trust_store)
    }

    /// Encrypted credentials vault handle.
    pub fn vault(&self) -> Arc<LocalEncryptedVault> {
        Arc::clone(&self.services.vault)
    }

    /// Feature registry handle.
    pub fn features_registry(&self) -> Arc<FeatureRegistry> {
        Arc::clone(&self.services.features_registry)
    }

    /// Child agent engine runtime handle.
    pub fn engine_runtime(&self) -> Arc<m31a_agent_engine::agents::EngineAgentRuntime> {
        Arc::clone(&self.services.engine_runtime.read().unwrap())
    }

    /// Currently active model slug.
    pub fn active_model(&self) -> String {
        self.active_model.read().unwrap().clone()
    }

    /// Available model catalog and cached entries.
    pub fn available_models(&self) -> Vec<ModelInfo> {
        self.available_models.read().unwrap().clone()
    }

    /// Subscribe to presentation-independent runtime events.
    pub fn subscribe_events(&self) -> broadcast::Receiver<RuntimeEvent> {
        self.event_broadcaster.subscribe()
    }

    /// Broadcast an event across all active subscribers.
    pub fn broadcast_event(&self, event: RuntimeEvent) {
        let _ = self.event_broadcaster.send(event);
    }

    /// Construct a newly configured `AgentEngine` backed by the runtime's services.
    pub fn create_agent_engine(
        &self,
        thread_cwd: Option<PathBuf>,
        collaboration_mode: CollaborationMode,
    ) -> AgentEngine {
        let cwd = thread_cwd.unwrap_or_else(|| self.config.cwd.clone());
        let guardian = if self.config.core.approval == m31a_core::ApprovalPolicy::ApproveForMe {
            Some(self.guardian())
        } else {
            None
        };

        crate::services::agent::create_agent_engine(
            self.provider().as_ref().clone(),
            (*self.services.tools).clone(),
            self.services.storage.clone(),
            self.config.core.approval,
            self.config.exec_policy.clone(),
            self.config.core.sandbox,
            cwd,
            guardian,
            Some(Arc::clone(&self.services.hooks)),
            self.services.code_mode.clone(),
            self.config.core.routing_enabled,
            self.config.core.tool_output_token_limit,
            collaboration_mode,
            self.config.core.dangerously_bypass,
        )
    }

    /// Execute a turn, streaming real-time `RuntimeEvent`s through an asynchronous channel.
    pub async fn execute_turn(
        &self,
        request: TurnRequest,
    ) -> Result<mpsc::Receiver<RuntimeEvent>, RuntimeError> {
        let admission = self.admission_lock.lock().map_err(|_| {
            RuntimeError::Shutdown("Runtime admission lock is poisoned".to_string())
        })?;
        if !self.state().is_operational() {
            return Err(RuntimeError::InvalidState {
                message: "Cannot execute turn: runtime is not ready".to_string(),
                current: self.state(),
            });
        }

        // Resolve the requested workspace before applying trust policy or constructing an engine.
        let requested_workspace = request
            .workspace_root
            .as_deref()
            .unwrap_or(&self.config.cwd);
        let workspace = requested_workspace.canonicalize().map_err(|error| {
            RuntimeError::Config(format!(
                "Unable to resolve workspace '{}': {error}",
                requested_workspace.display()
            ))
        })?;
        if !workspace.is_dir() {
            return Err(RuntimeError::Config(format!(
                "Workspace path is not a directory: {}",
                workspace.display()
            )));
        }

        crate::services::workspace::verify_workspace_trust(
            &self.services.trust_store,
            &workspace,
            request.allow_untrusted_workspace,
        )?;

        let turn_id = uuid::Uuid::new_v4().to_string();
        let thread_id = request.thread_id.clone();

        let mut engine = self.create_agent_engine(
            Some(workspace.clone()),
            request.collaboration_mode,
        );

        let active_model = self.active_model();
        let system_prompt = crate::services::agent::build_system_prompt(
            &workspace,
            &active_model,
            self.config.core.model_provider.as_deref(),
            self.config.core.sandbox,
            self.config.core.approval,
            request.collaboration_mode,
            self.config.core.system_prompt.clone(),
            &self.services.tools,
            self.services.code_mode.as_ref(),
            self.config.core.agents.max_depth,
        );
        engine.authoritative_system_prompt = Some(system_prompt.clone());

        let history = vec![
            ChatMessage::system(system_prompt),
            ChatMessage::user(request.prompt.clone()),
        ];

        let (eng_tx, mut eng_rx) = mpsc::unbounded_channel();
        let (steer_tx, steer_rx) = mpsc::unbounded_channel();
        let (out_tx, out_rx) = mpsc::channel(1024);
        let turn_cancel_token = self.cancel_token.child_token();

        let handle = ActiveTurnHandle {
            thread_id: thread_id.clone(),
            turn_id: turn_id.clone(),
            steer_tx,
            cancel_token: turn_cancel_token.clone(),
        };

        self.active_turns
            .lock()
            .unwrap()
            .insert(turn_id.clone(), handle);

        drop(admission);

        let cleanup = TurnCleanupGuard {
            turn_id: turn_id.clone(),
            active_turns: Arc::clone(&self.active_turns),
            pending_approvals: Arc::clone(&self.pending_approvals),
            pending_questions: Arc::clone(&self.pending_questions),
        };

        let runtime = self.clone();
        let tid = thread_id.clone();
        let turn = turn_id.clone();
        let max_steps = request.max_steps;

        // Broadcast turn submitted & admitted
        let submit_ev = RuntimeEvent::TurnSubmitted {
            thread_id: tid.clone(),
            turn_id: turn.clone(),
        };
        let admit_ev = RuntimeEvent::TurnAdmitted {
            thread_id: tid.clone(),
            turn_id: turn.clone(),
        };
        let _ = out_tx.send(submit_ev.clone()).await;
        runtime.broadcast_event(submit_ev);
        let _ = out_tx.send(admit_ev.clone()).await;
        runtime.broadcast_event(admit_ev);

        tokio::spawn(async move {
            let _cleanup = cleanup;
            let run_fut = engine.run_turn(
                &tid,
                history,
                eng_tx,
                steer_rx,
                turn_cancel_token,
                max_steps,
            );
            tokio::pin!(run_fut);

            let mut finished = false;

            while !finished {
                tokio::select! {
                    res = &mut run_fut => {
                        finished = true;
                        match res {
                            Ok(_messages) => {
                                let ev = RuntimeEvent::TurnCompleted {
                                    thread_id: tid.clone(),
                                    turn_id: turn.clone(),
                                    usage: m31a_core::types::TokenUsage::default(),
                                };
                                let _ = out_tx.send(ev.clone()).await;
                                runtime.broadcast_event(ev);
                            }
                            Err(e) => {
                                let ev = RuntimeEvent::TurnFailed {
                                    thread_id: tid.clone(),
                                    turn_id: turn.clone(),
                                    error: e.to_string(),
                                };
                                let _ = out_tx.send(ev.clone()).await;
                                runtime.broadcast_event(ev);
                            }
                        }
                    }
                    Some(ev) = eng_rx.recv() => {
                        let runtime_ev = match ev {
                            EngineEvent::ContentChunk(delta) => RuntimeEvent::ContentDelta {
                                thread_id: tid.clone(),
                                turn_id: turn.clone(),
                                delta,
                            },
                            EngineEvent::ReasoningChunk(delta) => RuntimeEvent::ReasoningDelta {
                                thread_id: tid.clone(),
                                turn_id: turn.clone(),
                                delta,
                            },
                            EngineEvent::ToolCallStarted { id, name, args } => RuntimeEvent::ToolStarted {
                                thread_id: tid.clone(),
                                turn_id: turn.clone(),
                                call_id: id,
                                tool_name: name,
                                arguments: args,
                            },
                            EngineEvent::ToolCallFinished { id, output, success } => RuntimeEvent::ToolFinished {
                                thread_id: tid.clone(),
                                turn_id: turn.clone(),
                                call_id: id,
                                tool_name: String::new(),
                                output,
                                success,
                            },
                            EngineEvent::ApprovalRequested { approval, response_tx } => {
                                let req_id = uuid::Uuid::new_v4().to_string();
                                runtime.pending_approvals.lock().unwrap().insert(
                                    req_id.clone(),
                                    PendingApprovalEntry {
                                        thread_id: tid.clone(),
                                        turn_id: turn.clone(),
                                        request_id: req_id.clone(),
                                        response_tx,
                                    },
                                );
                                RuntimeEvent::ApprovalRequested {
                                    thread_id: tid.clone(),
                                    turn_id: turn.clone(),
                                    request_id: req_id,
                                    command: approval.command.unwrap_or_default(),
                                    reason: approval.reason,
                                }
                            }
                            EngineEvent::QuestionRequested { question, response_tx } => {
                                let qid = uuid::Uuid::new_v4().to_string();
                                runtime.pending_questions.lock().unwrap().insert(
                                    qid.clone(),
                                    PendingQuestionEntry {
                                        thread_id: tid.clone(),
                                        turn_id: turn.clone(),
                                        question_id: qid.clone(),
                                        response_tx,
                                    },
                                );
                                RuntimeEvent::QuestionRequested {
                                    thread_id: tid.clone(),
                                    turn_id: turn.clone(),
                                    question_id: qid,
                                    question: question.question,
                                    options: question.options,
                                    multi_select: question.is_multi_select,
                                }
                            }
                            EngineEvent::TurnComplete { usage, .. } => RuntimeEvent::TurnCompleted {
                                thread_id: tid.clone(),
                                turn_id: turn.clone(),
                                usage,
                            },
                            EngineEvent::TurnFailed(err) => RuntimeEvent::TurnFailed {
                                thread_id: tid.clone(),
                                turn_id: turn.clone(),
                                error: err,
                            },
                            _ => continue,
                        };
                        let _ = out_tx.send(runtime_ev.clone()).await;
                        runtime.broadcast_event(runtime_ev);
                    }
                }
            }

        });

        Ok(out_rx)
    }

    /// Cancel a specific running turn.
    pub fn cancel_turn(&self, thread_id: &str, turn_id: &str) -> Result<(), RuntimeError> {
        let handle = self
            .active_turns
            .lock()
            .unwrap()
            .get(turn_id)
            .cloned()
            .filter(|handle| handle.thread_id == thread_id)
            .ok_or_else(|| RuntimeError::TurnNotFound(turn_id.to_string()))?;

        if !handle.cancel_token.is_cancelled() {
            handle.cancel_token.cancel();
            self.broadcast_event(RuntimeEvent::TurnCancelled {
                thread_id: handle.thread_id,
                turn_id: turn_id.to_string(),
            });
        }
        Ok(())
    }

    /// Cancel all currently active turns.
    pub fn cancel_all_turns(&self) -> usize {
        let turns: Vec<ActiveTurnHandle> = self
            .active_turns
            .lock()
            .unwrap()
            .values()
            .cloned()
            .collect();

        let count = turns.len();
        for turn in turns {
            if !turn.cancel_token.is_cancelled() {
                turn.cancel_token.cancel();
                self.broadcast_event(RuntimeEvent::TurnCancelled {
                    thread_id: turn.thread_id,
                    turn_id: turn.turn_id,
                });
            }
        }
        count
    }

    /// Deliver mid-stream steering input to an active turn.
    pub fn steer_turn(
        &self,
        _thread_id: &str,
        turn_id: &str,
        message: &str,
    ) -> Result<(), RuntimeError> {
        let lock = self.active_turns.lock().unwrap();
        let handle = lock
            .get(turn_id)
            .ok_or_else(|| RuntimeError::TurnNotFound(turn_id.to_string()))?;

        handle
            .steer_tx
            .send(message.to_string())
            .map_err(|e| RuntimeError::Turn(format!("Failed to deliver steer message: {e}")))
    }

    /// Respond to an elicitation approval request.
    pub fn respond_approval(
        &self,
        request_id: &str,
        decision: m31a_elicitation::ApprovalDecision,
    ) -> Result<(), RuntimeError> {
        let entry = self
            .pending_approvals
            .lock()
            .unwrap()
            .remove(request_id)
            .ok_or_else(|| RuntimeError::ApprovalNotFound(request_id.to_string()))?;

        let _ = entry.response_tx.send(decision);
        Ok(())
    }

    /// Respond to an interactive question request.
    pub fn respond_question(
        &self,
        question_id: &str,
        answers: Vec<String>,
    ) -> Result<(), RuntimeError> {
        let entry = self
            .pending_questions
            .lock()
            .unwrap()
            .remove(question_id)
            .ok_or_else(|| RuntimeError::QuestionNotFound(question_id.to_string()))?;

        let _ = entry.response_tx.send(answers);
        Ok(())
    }

    /// Build a new provider and replace the active model across runtime services.
    pub fn switch_model(&self, new_model: &str) -> Result<(), RuntimeError> {
        if self.active_model() == new_model {
            return Ok(());
        }

        let new_client = crate::services::provider::init_provider_client(
            self.config.core.model_provider.as_deref(),
            new_model,
            &self.config.core.api_base,
            self.config.core.api_key.as_deref(),
            self.config.reasoning_effort.clone(),
        )?;
        self.activate_model(new_model, new_client)
    }

    /// Commit a provider client that has already passed the caller's activation checks.
    pub fn activate_model(
        &self,
        new_model: &str,
        new_client: ProviderClient,
    ) -> Result<(), RuntimeError> {
        let _admission = self.admission_lock.lock().map_err(|_| {
            RuntimeError::Shutdown("Runtime admission lock is poisoned".to_string())
        })?;
        if !self.state().is_operational() {
            return Err(RuntimeError::InvalidState {
                message: "Cannot switch model: runtime is not ready".to_string(),
                current: self.state(),
            });
        }

        let old_model = self.active_model();
        if old_model == new_model {
            return Ok(());
        }

        let model_context_window = m31a_provider::find_model(new_model)
            .map(|info| info.context_window as usize)
            .or_else(|| {
                m31a_provider::find_canonical_model(new_model)
                    .and_then(|model| model.context_window.map(|window| window as usize))
            })
            .unwrap_or(self.config.core.model_context_window);

        let guardian = Arc::new(GuardianReviewer::new(new_model));
        let engine_runtime = Arc::new(
            m31a_agent_engine::agents::EngineAgentRuntime::new(
                new_client.clone(),
                (*self.services.tools).clone(),
                self.services.storage.clone(),
                self.config.exec_policy.clone(),
                self.config.cwd.clone(),
                Some(Arc::clone(&self.services.hooks)),
                model_context_window,
                self.config.core.agents.max_depth as u32,
            )
            .with_trust_store(Arc::clone(&self.services.trust_store)),
        );

        self.services.tools.set_agent_runtime(Arc::clone(&engine_runtime));
        *self.services.provider.write().unwrap() = Arc::new(new_client);
        *self.services.guardian.write().unwrap() = guardian;
        *self.services.engine_runtime.write().unwrap() = engine_runtime;
        *self.active_model.write().unwrap() = new_model.to_string();

        self.broadcast_event(RuntimeEvent::ModelChanged {
            old_model,
            new_model: new_model.to_string(),
        });

        Ok(())
    }

    /// Stop once and return the same truthful report to concurrent and repeated callers.
    pub async fn shutdown(&self) -> Result<ShutdownReport, RuntimeError> {
        let mut cached_report = self.shutdown_report.lock().await;
        if let Some(report) = cached_report.as_ref() {
            return Ok(report.clone());
        }

        let admission = self.admission_lock.lock().map_err(|_| {
            RuntimeError::Shutdown("Runtime admission lock is poisoned".to_string())
        })?;
        {
            let mut state = self.state.write().unwrap();
            if *state == RuntimeState::Stopped {
                let report = ShutdownReport::default();
                *cached_report = Some(report.clone());
                return Ok(report);
            }
            *state = RuntimeState::ShuttingDown;
        }
        drop(admission);

        self.broadcast_event(RuntimeEvent::ShuttingDown);
        self.cancel_token.cancel();

        let active_turns_cancelled = self.cancel_all_turns();
        let turns_drained = tokio::time::timeout(Duration::from_secs(10), async {
            loop {
                let is_empty = self
                    .active_turns
                    .lock()
                    .map(|turns| turns.is_empty())
                    .unwrap_or(false);
                if is_empty {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(25)).await;
            }
        })
        .await
        .is_ok();

        let stopped_processes = self.services.proc_manager.stop_all().await;
        let runtime_processes_stopped = stopped_processes
            .iter()
            .filter(|process| process.stopped)
            .count();
        let processes_remaining = self
            .services
            .proc_manager
            .list()
            .await
            .iter()
            .any(|process| process.status.is_running());

        // The tool registry owns additional sandbox, background-process, MCP, and shell resources.
        let tool_shutdown = self.services.tools.shutdown().await;
        let background_processes_stopped = runtime_processes_stopped
            + tool_shutdown.background_processes_stopped
            + tool_shutdown.sandbox_processes_stopped;

        let watcher_stopped = if let Some(watcher) = &self.services.watcher {
            watcher.stop();
            !watcher.is_running()
        } else {
            false
        };
        let watcher_cleanly_stopped = self
            .services
            .watcher
            .as_ref()
            .map(|watcher| !watcher.is_running())
            .unwrap_or(true);

        let mcp_servers_stopped = tool_shutdown.mcp_servers_stopped;
        let mcp_servers_remaining = !self.services.mcp_manager.list_server_names().await.is_empty();

        let report = ShutdownReport {
            cleanly_stopped: turns_drained
                && !processes_remaining
                && watcher_cleanly_stopped
                && tool_shutdown.persistent_shell_stopped
                && !mcp_servers_remaining,
            background_processes_stopped,
            active_turns_cancelled,
            watcher_stopped,
            mcp_servers_stopped,
            timestamp: Utc::now(),
        };

        *self.state.write().unwrap() = RuntimeState::Stopped;
        *cached_report = Some(report.clone());

        self.broadcast_event(RuntimeEvent::ShutdownComplete {
            report: report.clone(),
        });

        Ok(report)
    }
}
