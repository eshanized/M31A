//! Engine-backed hierarchical multi-agent runtime and orchestration engine.
//!
//! Each `spawn_agent` builds a genuine child [`AgentEngine`] — its own model
//! client, tool registry, sandbox policy, system prompt (standard builder plus
//! the role overlay), and history — and runs `run_turn` on a dedicated tokio
//! task. The parent steers children through the engine's steering channel
//! (`send_input`), awaits them (`wait_agent`), and cancels them
//! (`close_agent`). The spawn graph persists through the existing
//! `thread_spawn_edges` storage APIs.
//!
//! Scope notes (deviations documented in the adoption report §11):
//! - Bookkeeping constants instead of the upstream V2 config surface:
//!   max concurrent threads 8, spawn depth limit 3, child turn budget 20.
//! - The `interrupt` flag on `send_input` is advisory (the message is still
//!   delivered as steering; mid-sampling preemption is upstream-only).
//! - Children run non-interactively (`ApprovalPolicy::Never`, no guardian,
//!   `request_user_input` disabled) so they can never deadlock on an
//!   unanswered interactive oneshot.

use anyhow::{Context, Result, anyhow};
use async_trait::async_trait;
use m31a_core::prompts::{CollaborationMode, SystemPromptBuilder};
use m31a_core::types::{ChatMessage, Role, SessionMetadata};
use m31a_core::{ApprovalPolicy, CompactTokenBudget, ContextCompactor};
use m31a_execpolicy::ExecPolicy;
use m31a_hooks::HookEvent;
use m31a_provider::ProviderClient;
use m31a_storage::db::Storage;
use m31a_tools::agent_runtime::{
    AgentRuntime, AgentSpawnRequest, AgentWaitOutcome, CALLER_AGENT_ID, LiveAgentInfo, SpawnedAgent,
};
use m31a_tools::registry::ToolRegistry;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use tokio::sync::{mpsc, watch};
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};

use crate::turn_loop::{AgentEngine, EngineEvent};

/// Maximum concurrently RUNNING child agents per runtime (spawn-slot
/// reservation, upstream `max_threads`).
const MAX_CONCURRENT_THREADS: usize = 8;
/// Maximum spawn-tree depth: root engine → child = depth 1, grandchild = 2.
#[cfg(test)]
const MAX_SPAWN_DEPTH: usize = 3;
/// Turn budget for a child agent's single `run_turn`.
const CHILD_MAX_TURNS: usize = 20;

/// A reserved concurrency slot for one running child agent. Releases the
/// slot exactly once: either explicitly via [`SpawnSlot::release`] or on
/// drop (whichever comes first).
pub struct SpawnSlot {
    counter: Arc<AtomicUsize>,
    released: bool,
}

impl SpawnSlot {
    fn release(mut self) {
        if !self.released {
            self.counter.fetch_sub(1, Ordering::SeqCst);
            self.released = true;
        }
    }
}

impl Drop for SpawnSlot {
    fn drop(&mut self) {
        if !self.released {
            self.counter.fetch_sub(1, Ordering::SeqCst);
            self.released = true;
        }
    }
}

pub use m31a_core::types::{AgentRunStatus, TreeTerminationReport};

/// Closure signature for parent-mediated approval of subagent actions.
pub type ApprovalMediator = Arc<
    dyn Fn(
            &str, // subagent_id
            &crate::turn_loop::PendingApproval,
        ) -> Option<m31a_elicitation::ApprovalDecision>
        + Send
        + Sync,
>;

/// Approval request forwarded from a subagent to its parent or orchestrator.
#[derive(Debug)]
pub struct ChildApprovalRequest {
    pub agent_id: String,
    pub parent_agent_id: Option<String>,
    pub approval: crate::turn_loop::PendingApproval,
    pub response_tx: tokio::sync::oneshot::Sender<m31a_elicitation::ApprovalDecision>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputQueueActivity {
    Idle,
    Mailbox {
        agent_id: String,
        message: String,
    },
    BoardPost {
        channel: String,
        author: String,
        title: String,
    },
}

/// Everything the runtime tracks for one spawned child.
struct AgentCell {
    parent: Option<String>,
    role: String,
    task_name: String,
    steer_tx: Option<mpsc::UnboundedSender<String>>,
    cancel_token: CancellationToken,
    status_tx: watch::Sender<AgentRunStatus>,
    status_rx: watch::Receiver<AgentRunStatus>,
    activity_tx: watch::Sender<InputQueueActivity>,
    activity_rx: watch::Receiver<InputQueueActivity>,
    transcript: Mutex<Vec<String>>,
    answer: Mutex<String>,
}

/// Real child-engine multi-agent runtime. Constructed by the host and
/// injected into the `ToolRegistry` via `set_agent_runtime`.
pub struct EngineAgentRuntime {
    base_client: ProviderClient,
    tools: ToolRegistry,
    storage: Storage,
    exec_policy: ExecPolicy,
    cwd: PathBuf,
    hooks: Option<Arc<m31a_hooks::HookRuntime>>,
    /// Parent context-window budget inherited by children.
    context_window: usize,
    /// Maximum allowed spawn depth (from config, default 3).
    max_depth: u32,
    cells: Mutex<HashMap<String, Arc<AgentCell>>>,
    active_count: Arc<AtomicUsize>,
    worktree_mgr: Option<Arc<m31a_storage::WorktreeManager>>,
    pub trust_store: Arc<Mutex<m31a_trust::TrustStore>>,
    /// Multi-agent resource governance policies.
    pub governance: m31a_core::ResourceGovernance,
    /// Atomically tracked token usage aggregated across all running child agents.
    pub aggregate_tokens: Arc<AtomicUsize>,
    /// Optional parent-mediated approval request channel.
    child_approval_tx: Option<mpsc::UnboundedSender<ChildApprovalRequest>>,
    /// Optional parent-mediated approval decision handler.
    approval_mediator: Option<ApprovalMediator>,
}

impl EngineAgentRuntime {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        base_client: impl Into<ProviderClient>,
        tools: ToolRegistry,
        storage: Storage,
        exec_policy: ExecPolicy,
        cwd: PathBuf,
        hooks: Option<Arc<m31a_hooks::HookRuntime>>,
        context_window: usize,
        max_depth: u32,
    ) -> Self {
        let base_client = base_client.into();
        let governance = m31a_core::ResourceGovernance {
            max_spawn_depth: max_depth as usize,
            max_concurrent_threads: MAX_CONCURRENT_THREADS,
            max_child_turns: CHILD_MAX_TURNS,
            max_aggregate_tokens: None,
        };
        Self {
            base_client,
            tools,
            storage,
            exec_policy,
            cwd,
            hooks,
            context_window,
            max_depth,
            cells: Mutex::new(HashMap::new()),
            active_count: Arc::new(AtomicUsize::new(0)),
            worktree_mgr: None,
            trust_store: Arc::new(Mutex::new(m31a_trust::TrustStore::new())),
            governance,
            aggregate_tokens: Arc::new(AtomicUsize::new(0)),
            child_approval_tx: None,
            approval_mediator: None,
        }
    }

    /// Construct a new runtime with explicit resource governance configuration.
    #[allow(clippy::too_many_arguments)]
    pub fn new_with_governance(
        base_client: impl Into<ProviderClient>,
        tools: ToolRegistry,
        storage: Storage,
        exec_policy: ExecPolicy,
        cwd: PathBuf,
        hooks: Option<Arc<m31a_hooks::HookRuntime>>,
        context_window: usize,
        governance: m31a_core::ResourceGovernance,
    ) -> Result<Self> {
        governance.validate().map_err(|e| anyhow!(e))?;
        let max_depth = governance.max_spawn_depth as u32;
        let mut rt = Self::new(
            base_client,
            tools,
            storage,
            exec_policy,
            cwd,
            hooks,
            context_window,
            max_depth,
        );
        rt.governance = governance;
        Ok(rt)
    }

    /// Attach a resource governance configuration.
    pub fn with_governance(mut self, governance: m31a_core::ResourceGovernance) -> Self {
        self.max_depth = governance.max_spawn_depth as u32;
        self.governance = governance;
        self
    }

    /// Attach a parent approval mediator for subagent actions.
    pub fn with_approval_mediator(mut self, mediator: ApprovalMediator) -> Self {
        self.approval_mediator = Some(mediator);
        self
    }

    /// Attach an approval request channel for asynchronous parent supervision.
    pub fn with_approval_channel(
        mut self,
        tx: mpsc::UnboundedSender<ChildApprovalRequest>,
    ) -> Self {
        self.child_approval_tx = Some(tx);
        self
    }

    /// Returns current aggregate tokens consumed across all child agents.
    pub fn aggregate_tokens(&self) -> usize {
        self.aggregate_tokens.load(Ordering::SeqCst)
    }

    /// Atomically records tokens consumed by a child agent.
    pub fn record_tokens(&self, count: usize) -> usize {
        self.aggregate_tokens.fetch_add(count, Ordering::SeqCst) + count
    }

    /// Attach a trust store for workspace authorization evaluation.
    pub fn with_trust_store(mut self, trust_store: Arc<Mutex<m31a_trust::TrustStore>>) -> Self {
        self.trust_store = trust_store;
        self
    }

    /// Attach a worktree manager for git branch isolation during subagent execution.
    pub fn with_worktree_manager(mut self, mgr: Arc<m31a_storage::WorktreeManager>) -> Self {
        self.worktree_mgr = Some(mgr);
        self
    }

    /// Access the attached worktree manager, if any.
    pub fn worktree_manager(&self) -> Option<&Arc<m31a_storage::WorktreeManager>> {
        self.worktree_mgr.as_ref()
    }

    /// Spawn-tree depth contribution of the calling agent (`None` = root
    /// engine → a child spawns at depth 1).
    fn caller_depth(&self, caller: Option<&str>) -> usize {
        let mut depth = 0usize;
        let mut current = caller.map(|s| s.to_string());
        let cells = self.cells.lock().unwrap();
        while let Some(id) = current {
            depth += 1;
            current = cells.get(&id).and_then(|c| c.parent.clone());
        }
        depth
    }

    /// Resolves the AgentRole, falling back to the default like the registry.
    fn resolve_role(&self, role_name: &str) -> m31a_core::AgentRole {
        self.tools
            .role_registry
            .get(role_name)
            .cloned()
            .unwrap_or_else(m31a_core::AgentRole::default)
    }

    /// Returns the configured maximum agent spawn depth.
    pub fn max_depth(&self) -> u32 {
        self.max_depth
    }

    /// Returns current status of an agent synchronously if known.
    pub fn get_status_sync(&self, agent_id: &str) -> Option<String> {
        let cells = self.cells.lock().unwrap();
        cells
            .get(agent_id)
            .map(|c| c.status_rx.borrow().label().to_string())
    }

    /// Returns current in-memory transcript of an agent synchronously if known.
    pub fn get_transcript_sync(&self, agent_id: &str) -> Option<Vec<String>> {
        let cells = self.cells.lock().unwrap();
        cells
            .get(agent_id)
            .map(|c| c.transcript.lock().unwrap().clone())
    }

    /// Spawn-slot reservation (governance max_concurrent_threads semantics).
    pub fn reserve_slot(&self) -> Result<SpawnSlot> {
        let prev = self.active_count.fetch_add(1, Ordering::SeqCst);
        let max_threads = self.governance.max_concurrent_threads;
        if prev >= max_threads {
            self.active_count.fetch_sub(1, Ordering::SeqCst);
            return Err(anyhow!(
                "Spawn rejected: {} concurrent agent limit reached (close or wait for agents first)",
                max_threads
            ));
        }
        Ok(SpawnSlot {
            counter: Arc::clone(&self.active_count),
            released: false,
        })
    }

    pub fn ensure_depth(&self, depth: usize) -> Result<()> {
        let max = self.governance.max_spawn_depth;
        if depth > max {
            return Err(anyhow!(
                "Spawn rejected: depth limit {} exceeded (agent tree too deep)",
                max
            ));
        }
        Ok(())
    }

    /// Static depth gate check helper.
    pub fn ensure_depth_limit(depth: usize, max_depth: usize) -> Result<()> {
        if depth > max_depth {
            return Err(anyhow!(
                "Spawn rejected: depth limit {} exceeded (agent tree too deep)",
                max_depth
            ));
        }
        Ok(())
    }

    /// Folds one engine event into the child's transcript with parent approval mediation and token tracking.
    #[allow(clippy::too_many_arguments)]
    pub fn fold_event_mediated(
        agent_id: &str,
        event: EngineEvent,
        transcript: &mut Vec<String>,
        approval_mediator: Option<&ApprovalMediator>,
        child_approval_tx: Option<&mpsc::UnboundedSender<ChildApprovalRequest>>,
        parent_agent_id: Option<&str>,
        aggregate_tokens: &Arc<AtomicUsize>,
        max_aggregate_tokens: Option<usize>,
    ) -> Option<String> {
        match event {
            EngineEvent::ContentChunk(chunk) => {
                transcript.push(chunk);
                None
            }
            EngineEvent::ReasoningChunk(_) => None,
            EngineEvent::ToolCallStarted { name, .. } => {
                transcript.push(format!("[tool] {name}"));
                None
            }
            EngineEvent::ToolCallFinished {
                output, success, ..
            } => {
                let out = safe_tail_truncate(&output, 400);
                transcript.push(format!(
                    "[tool {}] {}",
                    if success { "ok" } else { "err" },
                    out
                ));
                None
            }
            EngineEvent::GuardianDenied { rationale } => {
                transcript.push(format!("[guardian denied] {rationale}"));
                None
            }
            EngineEvent::Notice(msg, is_warning) => {
                transcript.push(format!(
                    "[{}] {}",
                    if is_warning { "warn" } else { "note" },
                    msg
                ));
                None
            }
            EngineEvent::ApprovalRequested {
                approval,
                response_tx,
            } => {
                // 1. If an explicit child approval channel is wired, forward request to parent
                if let Some(tx) = child_approval_tx {
                    let req = ChildApprovalRequest {
                        agent_id: agent_id.to_string(),
                        parent_agent_id: parent_agent_id.map(|s| s.to_string()),
                        approval: approval.clone(),
                        response_tx,
                    };
                    if tx.send(req).is_ok() {
                        transcript.push(format!(
                            "[approval forwarded to parent for '{}']",
                            approval.tool_call.function.name
                        ));
                    }
                    return None;
                }

                // 2. If a parent approval mediator callback is configured, delegate decision
                if let Some(decision) = approval_mediator.and_then(|m| m(agent_id, &approval)) {
                    let is_approve =
                        !matches!(decision, m31a_elicitation::ApprovalDecision::Deny { .. });
                    let _ = response_tx.send(decision);
                    transcript.push(format!(
                        "[approval mediated by parent: {}]",
                        if is_approve { "approved" } else { "denied" }
                    ));
                    return None;
                }

                // 3. Fallback: classify tool effect; auto-approve read-only, deny mutating tools fail-closed
                let effect = m31a_execpolicy::classify_tool(
                    &approval.tool_call.function.name,
                    Some(&approval.tool_call.function.arguments),
                );
                if effect == m31a_execpolicy::ToolEffect::ReadOnly {
                    let _ = response_tx.send(m31a_elicitation::ApprovalDecision::AllowOnce);
                    transcript.push(format!(
                        "[approval auto-approved read-only tool '{}']",
                        approval.tool_call.function.name
                    ));
                } else {
                    let _ = response_tx.send(m31a_elicitation::ApprovalDecision::Deny {
                        reason: format!(
                            "Subagent '{}' cannot execute mutating tool '{}' without parent interactive approval",
                            agent_id, approval.tool_call.function.name
                        ),
                    });
                    transcript.push(format!(
                        "[approval denied for '{}': requires parent approval]",
                        approval.tool_call.function.name
                    ));
                }
                None
            }
            EngineEvent::QuestionRequested { response_tx, .. } => {
                let _ = response_tx.send(Vec::new());
                transcript.push("[question auto-dismissed]".to_string());
                None
            }
            EngineEvent::Compacted { .. } => {
                transcript.push("[context compacted]".to_string());
                None
            }
            EngineEvent::TurnComplete { messages, .. } => {
                let estimated_tokens: usize = messages
                    .iter()
                    .map(|m| m.content.as_ref().map(|c| c.len() / 4).unwrap_or(0))
                    .sum();
                let new_total = aggregate_tokens.fetch_add(estimated_tokens, Ordering::SeqCst)
                    + estimated_tokens;
                if let Some(max) = max_aggregate_tokens.filter(|&max| new_total > max) {
                    transcript.push(format!(
                        "[governance: aggregate token budget {} exceeded (total: {})]",
                        max, new_total
                    ));
                }
                messages
                    .iter()
                    .rev()
                    .find(|m| m.role == Role::Assistant)
                    .and_then(|m| m.content.clone())
                    .filter(|c| !c.trim().is_empty())
            }
            EngineEvent::TurnFailed(e) => {
                transcript.push(format!("[turn failed] {e}"));
                None
            }
        }
    }

    /// Backwards-compatible event fold helper.
    pub fn fold_event(event: EngineEvent, transcript: &mut Vec<String>) -> Option<String> {
        let dummy_tokens = Arc::new(AtomicUsize::new(0));
        Self::fold_event_mediated(
            "agent",
            event,
            transcript,
            None,
            None,
            None,
            &dummy_tokens,
            None,
        )
    }
}

/// Tail-truncates a UTF-8 string to at most `max_bytes`, keeping complete grapheme clusters
/// and prepending an ellipsis "…" if truncation occurred.
///
/// Guaranteed to never split UTF-8 code points or multi-codepoint grapheme clusters
/// (e.g., combining characters, flags, emojis).
pub fn safe_tail_truncate(s: &str, max_bytes: usize) -> String {
    if s.len() <= max_bytes {
        return s.to_string();
    }
    if max_bytes == 0 {
        return String::new();
    }

    use unicode_segmentation::UnicodeSegmentation;

    let mut total_bytes = 0;
    let mut start_byte_offset = s.len();

    for (byte_offset, grapheme) in s.grapheme_indices(true).rev() {
        if total_bytes + grapheme.len() > max_bytes {
            break;
        }
        total_bytes += grapheme.len();
        start_byte_offset = byte_offset;
    }

    if start_byte_offset == 0 {
        s.to_string()
    } else {
        format!("…{}", &s[start_byte_offset..])
    }
}

#[async_trait]
impl AgentRuntime for EngineAgentRuntime {
    async fn spawn_agent(&self, req: AgentSpawnRequest) -> Result<SpawnedAgent> {
        let caller = m31a_tools::agent_runtime::caller_agent_id();
        // Use depth from request if provided, otherwise compute
        let depth = if req.depth > 0 {
            req.depth as usize
        } else {
            self.caller_depth(caller.as_deref()) + 1
        };
        // Use max_depth from request if provided, otherwise use governance
        let max_depth = if req.max_depth > 0 {
            req.max_depth as usize
        } else {
            self.governance.max_spawn_depth
        };
        if depth > max_depth {
            return Err(anyhow!(
                "Spawn rejected: depth limit {} exceeded (current depth {})",
                max_depth,
                depth
            ));
        }
        self.ensure_depth(depth)?;

        // Enforce aggregate token budget before reserving a slot
        if let Some(max_tokens) = self.governance.max_aggregate_tokens {
            let current = self.aggregate_tokens.load(Ordering::SeqCst);
            if current >= max_tokens {
                return Err(anyhow!(
                    "Spawn rejected: Aggregate token budget limit of {} reached (current: {})",
                    max_tokens,
                    current
                ));
            }
        }

        // Spawn-slot reservation (upstream max_threads semantics). The guard
        // is handed to the child task, which releases it when run_turn ends.
        let slot = self.reserve_slot()?;

        let role = self.resolve_role(&req.role_name);
        let model = req
            .model_override
            .or_else(|| role.model_override.clone())
            .unwrap_or_else(|| self.base_client.model().to_string());
        let nickname = role
            .nickname_candidates
            .first()
            .cloned()
            .unwrap_or_else(|| "Agent".to_string());

        let agent_id = format!("agent_{}_{}", role.name, uuid::Uuid::new_v4().simple());

        // Setup isolated Git worktree if requested: fail-closed if isolation cannot be established
        let (child_cwd, child_worktree) = if req.isolate_worktree {
            let mgr = self.worktree_mgr.as_ref().ok_or_else(|| {
                anyhow!(
                    "isolate_worktree requested for agent '{}', but no WorktreeManager is configured. Refusing unisolated execution.",
                    agent_id
                )
            })?;
            let wt = mgr.create_worktree(&agent_id, None).map_err(|e| {
                anyhow!(
                    "Failed to create isolated Git worktree for agent '{}': {e}. Refusing ambient fallback.",
                    agent_id
                )
            })?;
            info!(
                agent_id = %agent_id,
                branch = %wt.branch,
                path = %wt.worktree_path.display(),
                "Spawned child agent in isolated Git worktree"
            );
            let path = wt.worktree_path.clone();
            (path, Some(wt))
        } else {
            (self.cwd.clone(), None)
        };

        // Enforce workspace trust authorization: child cannot exceed parent trust scope
        if let Err(e) = self
            .trust_store
            .lock()
            .unwrap()
            .authorize_child_agent(&self.cwd, &child_cwd)
        {
            if let (Some(wt), Some(mgr)) = (&child_worktree, &self.worktree_mgr) {
                let _ = mgr.cleanup_worktree(&wt.thread_id);
            }
            return Err(e);
        }

        // Child engine: own client/registry/policy/prompt/history, non-interactive.
        let mut child_tools = self.tools.clone();
        // Children cannot block on interactive elicitation (deadlock hazard).
        child_tools.disable("request_user_input");

        let client = self.base_client.with_model(&model);
        let mut child = AgentEngine::new(
            client,
            child_tools,
            self.storage.clone(),
            ApprovalPolicy::Never,
            self.exec_policy.clone(),
            role.sandbox_policy,
            child_cwd.clone(),
        );
        child.compactor = Arc::new(ContextCompactor::new(CompactTokenBudget {
            max_context_tokens: self.context_window,
            ..CompactTokenBudget::default()
        }));
        child.hooks = self.hooks.clone();

        let snapshot = m31a_core::prompts::PromptRuntimeSnapshot::new(
            &child_cwd,
            &model,
            &format!("{:?}", role.sandbox_policy),
            CollaborationMode::Default,
        )
        .with_role(Some(role.clone()));

        let system_prompt = SystemPromptBuilder::build_canonical(&snapshot);
        child.authoritative_system_prompt = Some(system_prompt.clone());

        let history = vec![
            ChatMessage::system(system_prompt),
            ChatMessage::user(req.prompt.clone()),
        ];

        // Transactional child launch: persist thread and spawn edge before launching task.
        // If persistence fails, roll back worktree and return typed error.
        let meta = SessionMetadata {
            id: agent_id.clone(),
            title: req.task_name.clone(),
            model: model.clone(),
            cwd: child_cwd.to_string_lossy().to_string(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            extra: HashMap::new(),
        };
        if let Err(e) =
            self.storage
                .save_thread(&meta, "", &format!("{:?}", role.sandbox_policy), "Never")
        {
            if let (Some(wt), Some(mgr)) = (&child_worktree, &self.worktree_mgr) {
                let _ = mgr.cleanup_worktree(&wt.thread_id);
            }
            return Err(anyhow!(
                "Failed to persist child thread {}: {}",
                agent_id,
                e
            ));
        }
        let parent_label = caller.clone().unwrap_or_else(|| "current".to_string());
        if let Err(e) = self.storage.upsert_spawn_edge(
            &parent_label,
            &agent_id,
            &role.name,
            "open",
            Some(depth as u32),
        ) {
            let _ = self.storage.delete_thread(&agent_id);
            if let (Some(wt), Some(mgr)) = (&child_worktree, &self.worktree_mgr) {
                let _ = mgr.cleanup_worktree(&wt.thread_id);
            }
            return Err(anyhow!(
                "Failed to persist spawn edge {}→{}: {}",
                parent_label,
                agent_id,
                e
            ));
        }

        // Steering channel: the runtime keeps the sender; run_turn consumes
        // the receiver as user steering messages.
        let (steer_tx, steer_rx) = mpsc::unbounded_channel::<String>();
        let cancel_token = CancellationToken::new();
        let (status_tx, status_rx) = watch::channel(AgentRunStatus::Running);
        let (activity_tx, activity_rx) = watch::channel(InputQueueActivity::Idle);

        let cell = Arc::new(AgentCell {
            parent: caller.clone(),
            role: role.name.clone(),
            task_name: req.task_name.clone(),
            steer_tx: Some(steer_tx),
            cancel_token: cancel_token.clone(),
            status_tx,
            status_rx: status_rx.clone(),
            activity_tx,
            activity_rx,
            transcript: Mutex::new(Vec::new()),
            answer: Mutex::new(String::new()),
        });
        self.cells
            .lock()
            .unwrap()
            .insert(agent_id.clone(), cell.clone());

        // SubagentStart hook (parent-side, observability).
        if let Some(hooks) = &self.hooks {
            let _ = hooks
                .dispatch(
                    HookEvent::SubagentStart,
                    "",
                    serde_json::json!({ "agent_id": agent_id, "role": role.name, "task": req.task_name }),
                )
                .await;
        }

        info!(agent_id = %agent_id, role = %role.name, depth, "Spawned child agent engine");

        // The child task runs run_turn wrapped in the CALLER_AGENT_ID
        // task-local so ITS spawn_agent calls attribute to this child.
        let storage = self.storage.clone();
        let task_agent_id = agent_id.clone();
        let _task_role = role.name.clone();
        let hooks = self.hooks.clone();
        let task_cell = cell.clone();
        let task_worktree = child_worktree.clone();
        let task_wt_mgr = self.worktree_mgr.clone();
        let task_child_max_turns = self.governance.max_child_turns;
        let task_approval_mediator = self.approval_mediator.clone();
        let task_child_approval_tx = self.child_approval_tx.clone();
        let task_parent = cell.parent.clone();
        let task_aggregate_tokens = Arc::clone(&self.aggregate_tokens);
        let task_max_aggregate_tokens = self.governance.max_aggregate_tokens;

        tokio::spawn(async move {
            let (event_tx, mut event_rx) = mpsc::unbounded_channel::<EngineEvent>();
            let transcript_holder = Arc::new(Mutex::new(Vec::new()));
            let transcript_clone = Arc::clone(&transcript_holder);

            let drain_agent_id = task_agent_id.clone();
            let drain_handle = tokio::spawn(async move {
                while let Some(event) = event_rx.recv().await {
                    let mut lock = transcript_clone.lock().unwrap();
                    if let Some(final_answer) = EngineAgentRuntime::fold_event_mediated(
                        &drain_agent_id,
                        event,
                        &mut lock,
                        task_approval_mediator.as_ref(),
                        task_child_approval_tx.as_ref(),
                        task_parent.as_deref(),
                        &task_aggregate_tokens,
                        task_max_aggregate_tokens,
                    ) {
                        lock.push(final_answer);
                    }
                }
            });

            let result = CALLER_AGENT_ID
                .scope(
                    task_agent_id.clone(),
                    child.run_turn(
                        &task_agent_id,
                        history,
                        event_tx,
                        steer_rx,
                        cancel_token,
                        task_child_max_turns,
                    ),
                )
                .await;

            let _ = drain_handle.await;
            let transcript = transcript_holder.lock().unwrap().clone();

            let was_cancelled = task_cell.cancel_token.is_cancelled();
            let mut status = match &result {
                Ok(()) if was_cancelled => AgentRunStatus::Cancelled,
                Ok(()) => AgentRunStatus::Completed,
                Err(_) if was_cancelled => AgentRunStatus::Cancelled,
                Err(e) => AgentRunStatus::Failed(e.to_string()),
            };

            // Two-Phase Git Preflight: If child completed in an isolated worktree, verify patch against parent HEAD
            // Do NOT auto-commit unverified changes! Mark as IntegrationPending if non-empty changes exist.
            if matches!(status, AgentRunStatus::Completed)
                && let (Some(wt), Some(mgr)) = (&task_worktree, &task_wt_mgr)
                && let Ok(diff) = mgr.export_worktree_diff(&wt.worktree_path, &wt.head_sha)
                && !diff.trim().is_empty()
            {
                match mgr.git_preflight_check(&diff) {
                    Ok(true) => {
                        info!(agent_id = %task_agent_id, "Two-phase git preflight check passed cleanly for child worktree changes; awaiting explicit integration");
                        status = AgentRunStatus::IntegrationPending;
                    }
                    Ok(false) => {
                        warn!(agent_id = %task_agent_id, "Two-phase git preflight check detected merge conflicts between child worktree and parent workspace");
                        status = AgentRunStatus::IntegrationRejected(
                            "Preflight conflict with parent HEAD".to_string(),
                        );
                    }
                    Err(e) => {
                        warn!(agent_id = %task_agent_id, error = %e, "Git preflight check encountered validation error");
                        status = AgentRunStatus::IntegrationRejected(e.to_string());
                    }
                }
            }

            let _ = task_cell.status_tx.send(status.clone());

            *task_cell.answer.lock().unwrap() = transcript
                .last()
                .cloned()
                .filter(|_| {
                    matches!(
                        status,
                        AgentRunStatus::Completed
                            | AgentRunStatus::IntegrationPending
                            | AgentRunStatus::Integrated
                    )
                })
                .unwrap_or_default();
            *task_cell.transcript.lock().unwrap() = transcript;

            // Edge status: completed / integration_pending / integrated / failed / closed.
            let edge_status = match &status {
                AgentRunStatus::Completed => "completed",
                AgentRunStatus::IntegrationPending => "integration_pending",
                AgentRunStatus::Integrated => "integrated",
                AgentRunStatus::Failed(_) => "failed",
                _ => "closed",
            };
            let _ = storage.set_spawn_edge_status(&task_agent_id, edge_status);

            if let Some(hooks) = &hooks {
                let _ = hooks
                    .dispatch(
                        HookEvent::SubagentStop,
                        "",
                        serde_json::json!({ "agent_id": task_agent_id, "status": status.label() }),
                    )
                    .await;
            }

            let _ = task_cell.status_tx.send(status);
            // Cell stays registered (terminal) — wait/list can still read it.
            slot.release();
            debug!(agent_id = %task_agent_id, "Child agent task finished");
        });

        Ok(SpawnedAgent {
            agent_id,
            nickname,
            model,
            role: role.name,
            sandbox: format!("{:?}", role.sandbox_policy),
            worktree_path: child_worktree.as_ref().map(|w| w.worktree_path.clone()),
        })
    }

    async fn send_input(&self, agent_id: &str, message: &str, _interrupt: bool) -> Result<String> {
        let cell = {
            let cells = self.cells.lock().unwrap();
            cells.get(agent_id).cloned()
        };
        let Some(cell) = cell else {
            return Err(anyhow!("Unknown agent `{agent_id}`"));
        };
        if cell.status_rx.borrow().is_terminal() {
            return Err(anyhow!(
                "Agent `{agent_id}` already finished; input not delivered"
            ));
        }
        // The interrupt flag is advisory in M31A (scoped-down §4.3): the
        // message is delivered as steering; mid-sampling preemption is
        // upstream-only. Documented in the adoption report.
        let Some(tx) = &cell.steer_tx else {
            return Err(anyhow!(
                "Agent `{agent_id}` is closed; cannot deliver input"
            ));
        };
        match tx.send(message.to_string()) {
            Ok(()) => {
                let _ = cell.activity_tx.send_replace(InputQueueActivity::Mailbox {
                    agent_id: agent_id.to_string(),
                    message: message.to_string(),
                });
                Ok(format!(
                    "Delivered message to agent `{agent_id}` (interrupt=false, delivered as steering): {message}"
                ))
            }
            Err(_) => Err(anyhow!(
                "Agent `{agent_id}` already finished; input not delivered"
            )),
        }
    }

    async fn wait_agents(
        &self,
        agent_ids: &[String],
        timeout_ms: Option<u64>,
    ) -> Result<Vec<AgentWaitOutcome>> {
        let timeout = std::time::Duration::from_millis(timeout_ms.unwrap_or(120_000));
        let deadline = tokio::time::Instant::now() + timeout;

        let mut outcomes = Vec::new();
        for id in agent_ids {
            let cell = {
                let cells = self.cells.lock().unwrap();
                cells.get(id).cloned()
            };
            let Some(cell) = cell else {
                outcomes.push(AgentWaitOutcome {
                    agent_id: id.clone(),
                    status: "unknown".to_string(),
                    answer: None,
                });
                continue;
            };

            let mut status_rx = cell.status_rx.clone();
            let mut activity_rx = cell.activity_rx.clone();
            let mut mailbox_message = None;

            let status: AgentRunStatus = loop {
                if status_rx.borrow().is_terminal() {
                    break status_rx.borrow().clone();
                }

                tokio::select! {
                    res = status_rx.changed() => {
                        match res {
                            Ok(()) => continue,
                            Err(_) => break status_rx.borrow().clone(),
                        }
                    }
                    res = activity_rx.changed() => {
                        if res.is_ok() {
                            let act = activity_rx.borrow().clone();
                            if let InputQueueActivity::Mailbox { message, .. } = act {
                                mailbox_message = Some(message);
                                break status_rx.borrow().clone();
                            }
                        }
                    }
                    _ = tokio::time::sleep_until(deadline) => {
                        break status_rx.borrow().clone();
                    }
                }
            };

            let (status_label, answer) = if let Some(msg) = mailbox_message {
                ("mailbox_activity".to_string(), Some(msg))
            } else if matches!(status, AgentRunStatus::Completed) {
                let t = cell.answer.lock().unwrap().clone();
                let ans = if t.trim().is_empty() {
                    let transcript = cell.transcript.lock().unwrap().join("\n");
                    Some(transcript)
                } else {
                    Some(t)
                };
                (status.label(), ans)
            } else {
                (status.label(), None)
            };

            outcomes.push(AgentWaitOutcome {
                agent_id: id.clone(),
                status: status_label,
                answer,
            });
        }
        Ok(outcomes)
    }

    async fn close_agent(&self, agent_id: &str) -> Result<String> {
        let cell = {
            let cells = self.cells.lock().unwrap();
            cells.get(agent_id).cloned()
        };
        let Some(cell) = cell else {
            return Err(anyhow!("Unknown agent `{agent_id}`"));
        };

        cell.cancel_token.cancel();

        let was_running = !cell.status_rx.borrow().is_terminal();
        if was_running {
            let _ = cell.status_tx.send(AgentRunStatus::CancellationRequested);
        }
        // The child task releases the slot and finalizes the edge when
        // run_turn observes cancellation.
        Ok(format!(
            "Close requested for agent `{agent_id}` (cancellation signalled)."
        ))
    }

    async fn terminate_tree(&self, agent_id: &str) -> Result<TreeTerminationReport> {
        // Collect target and all descendants from storage and active in-memory cells
        let mut to_close = vec![agent_id.to_string()];
        let mut frontier = vec![agent_id.to_string()];
        for _ in 0..self.governance.max_spawn_depth + 1 {
            let mut next = Vec::new();
            for parent in &frontier {
                if let Ok(children) = self.storage.list_spawn_children(parent, None) {
                    for c in children {
                        if !to_close.contains(&c.child_thread_id) {
                            to_close.push(c.child_thread_id.clone());
                            next.push(c.child_thread_id);
                        }
                    }
                }
            }
            if next.is_empty() {
                break;
            }
            frontier = next;
        }

        // Also check in-memory cells for any child with parent matching to_close
        {
            let cells = self.cells.lock().unwrap();
            for (id, cell) in cells.iter() {
                if let Some(parent) = &cell.parent
                    && to_close.contains(parent)
                    && !to_close.contains(id)
                {
                    to_close.push(id.clone());
                }
            }
        }

        // Signal cancellation to all agents in tree
        for id in &to_close {
            let _ = self.close_agent(id).await;
        }

        // Await all cancelled child agents to reach terminal state with timeout
        let outcomes = self
            .wait_agents(&to_close, Some(5000))
            .await
            .unwrap_or_default();

        let mut terminated_agents = Vec::new();
        let mut timed_out_agents = Vec::new();
        let mut failed_agents = Vec::new();

        for id in &to_close {
            let cell = {
                let cells = self.cells.lock().unwrap();
                cells.get(id).cloned()
            };
            let status = if let Some(c) = cell {
                c.status_rx.borrow().clone()
            } else if let Some(o) = outcomes.iter().find(|o| &o.agent_id == id) {
                match o.status.as_str() {
                    "completed" => AgentRunStatus::Completed,
                    "cancelled" => AgentRunStatus::Cancelled,
                    "closed" => AgentRunStatus::Closed,
                    s if s.starts_with("failed") => AgentRunStatus::Failed(s.to_string()),
                    _ => AgentRunStatus::Running,
                }
            } else {
                AgentRunStatus::Closed
            };

            match status {
                AgentRunStatus::Cancelled
                | AgentRunStatus::Closed
                | AgentRunStatus::Completed
                | AgentRunStatus::Integrated => {
                    terminated_agents.push(id.clone());
                    let _ = self.storage.set_spawn_edge_status(id, "closed");
                }
                AgentRunStatus::Failed(err) => {
                    terminated_agents.push(id.clone());
                    failed_agents.push((id.clone(), err));
                    let _ = self.storage.set_spawn_edge_status(id, "failed");
                }
                _ => {
                    timed_out_agents.push(id.clone());
                }
            }
        }

        let is_complete = timed_out_agents.is_empty() && failed_agents.is_empty();
        if is_complete {
            let _ = self.storage.terminate_agent_tree(agent_id);
        }

        Ok(TreeTerminationReport {
            root_id: agent_id.to_string(),
            terminated_agents,
            timed_out_agents,
            failed_agents,
            is_complete,
        })
    }

    async fn list_live(&self) -> Vec<LiveAgentInfo> {
        let cells = self.cells.lock().unwrap();
        cells
            .iter()
            .map(|(id, cell)| LiveAgentInfo {
                agent_id: id.clone(),
                role: cell.role.clone(),
                task_name: cell.task_name.clone(),
                status: cell.status_rx.borrow().label(),
            })
            .collect()
    }

    async fn integrate_agent(
        &self,
        agent_id: &str,
        commit_message: Option<&str>,
    ) -> Result<String> {
        let cell = {
            let cells = self.cells.lock().unwrap();
            cells.get(agent_id).cloned()
        };
        let Some(cell) = cell else {
            return Err(anyhow!("Unknown agent `{agent_id}`"));
        };

        let current_status = cell.status_rx.borrow().clone();
        if !matches!(
            current_status,
            AgentRunStatus::IntegrationPending | AgentRunStatus::Completed
        ) {
            return Err(anyhow!(
                "Cannot integrate agent `{agent_id}`: status is '{}', expected 'integration_pending' or 'completed'",
                current_status.label()
            ));
        }

        let mgr = self
            .worktree_mgr
            .as_ref()
            .ok_or_else(|| anyhow!("Worktree manager not configured"))?;
        let worktrees = mgr.list_worktrees()?;
        let child_wt = worktrees
            .iter()
            .find(|w| w.thread_id == agent_id)
            .ok_or_else(|| anyhow!("Worktree for agent `{agent_id}` not found"))?;

        let diff = mgr.export_worktree_diff(&child_wt.worktree_path, &child_wt.head_sha)?;
        if diff.trim().is_empty() {
            let _ = cell.status_tx.send(AgentRunStatus::Integrated);
            return Ok("Agent produced no changes; marked as integrated.".to_string());
        }

        // 1. Path validation — reject directory traversal and protected targets
        m31a_storage::WorktreeManager::validate_patch_paths(&diff)?;

        // 2. Preflight check against parent HEAD
        if !mgr.git_preflight_check(&diff)? {
            let err =
                "Preflight check failed: patch conflicts with parent repository HEAD".to_string();
            let _ = cell
                .status_tx
                .send(AgentRunStatus::IntegrationRejected(err.clone()));
            return Err(anyhow!(err));
        }

        // 3. Code review via ReviewEngine
        let review_report = m31a_review::ReviewEngine::analyze(&diff);
        if review_report.verdict == m31a_review::ReviewVerdict::RequestChanges {
            let reasons: Vec<String> = review_report
                .findings
                .iter()
                .filter(|f| {
                    f.severity == m31a_review::ReviewSeverity::Critical
                        || f.severity == m31a_review::ReviewSeverity::High
                })
                .map(|f| format!("{}: {}", f.title, f.description))
                .collect();
            let err = format!(
                "Integration rejected by Code Review Engine: {}",
                reasons.join("; ")
            );
            let _ = cell
                .status_tx
                .send(AgentRunStatus::IntegrationRejected(err.clone()));
            return Err(anyhow!(err));
        }

        // 4. Isolated worktree test verification: project detection, formatting, compilation, test execution
        let test_result = mgr.test_patch_in_isolated_worktree(&diff, |isolated_path| {
            let is_rust = isolated_path.join("Cargo.toml").exists();
            let is_node = isolated_path.join("package.json").exists();
            let is_python = isolated_path.join("pyproject.toml").exists()
                || isolated_path.join("setup.py").exists()
                || isolated_path.join("requirements.txt").exists();
            let is_go = isolated_path.join("go.mod").exists();

            if !is_rust && !is_node && !is_python && !is_go {
                return Err(anyhow!(
                    "VerificationIncomplete: unsupported project type; automated verification cannot prove correctness without recognized build manifest (Cargo.toml, package.json, pyproject.toml, or go.mod)"
                ));
            }

            fn run_timed_cmd(mut cmd: std::process::Command, label: &str, timeout: std::time::Duration) -> Result<()> {
                cmd.stdout(std::process::Stdio::piped());
                cmd.stderr(std::process::Stdio::piped());
                let mut child = cmd.spawn().with_context(|| format!("Failed to spawn {label}"))?;
                let stdout = child.stdout.take();
                let stderr = child.stderr.take();
                const MAX_CAPTURE: usize = 64 * 1024;
                let out_handle = std::thread::spawn(move || {
                    let mut buf = Vec::new();
                    if let Some(mut r) = stdout {
                        let mut chunk = [0u8; 4096];
                        while let Ok(n) = std::io::Read::read(&mut r, &mut chunk) {
                            if n == 0 { break; }
                            if buf.len() < MAX_CAPTURE {
                                let to_copy = n.min(MAX_CAPTURE - buf.len());
                                buf.extend_from_slice(&chunk[..to_copy]);
                            }
                        }
                    }
                    buf
                });
                let err_handle = std::thread::spawn(move || {
                    let mut buf = Vec::new();
                    if let Some(mut r) = stderr {
                        let mut chunk = [0u8; 4096];
                        while let Ok(n) = std::io::Read::read(&mut r, &mut chunk) {
                            if n == 0 { break; }
                            if buf.len() < MAX_CAPTURE {
                                let to_copy = n.min(MAX_CAPTURE - buf.len());
                                buf.extend_from_slice(&chunk[..to_copy]);
                            }
                        }
                    }
                    buf
                });

                let start = std::time::Instant::now();
                loop {
                    match child.try_wait() {
                        Ok(Some(status)) => {
                            let _ = out_handle.join();
                            let err_bytes = err_handle.join().unwrap_or_default();
                            if !status.success() {
                                let err_str = String::from_utf8_lossy(&err_bytes);
                                return Err(anyhow!("{label} failed (exit status {status}): {err_str}"));
                            }
                            return Ok(());
                        }
                        Ok(None) => {
                            if start.elapsed() >= timeout {
                                let _ = child.kill();
                                let _ = child.wait();
                                let _ = out_handle.join();
                                let _ = err_handle.join();
                                return Err(anyhow!("{label} timed out after {:?}", timeout));
                            }
                            std::thread::sleep(std::time::Duration::from_millis(50));
                        }
                        Err(e) => {
                            let _ = child.kill();
                            let _ = child.wait();
                            return Err(anyhow!("Failed waiting on {label}: {e}"));
                        }
                    }
                }
            }

            let cmd_timeout = std::time::Duration::from_secs(30);

            if is_rust {
                let mut fmt_cmd = std::process::Command::new("cargo");
                fmt_cmd.args(["fmt", "--all", "--", "--check"]).current_dir(isolated_path);
                let _ = run_timed_cmd(fmt_cmd, "cargo fmt", cmd_timeout);

                let mut check_cmd = std::process::Command::new("cargo");
                check_cmd.args(["check", "--all-targets"]).current_dir(isolated_path);
                run_timed_cmd(check_cmd, "cargo check", cmd_timeout)?;

                let mut test_cmd = std::process::Command::new("cargo");
                test_cmd.args(["test", "--no-fail-fast"]).current_dir(isolated_path);
                run_timed_cmd(test_cmd, "cargo test", cmd_timeout)?;
            } else if is_node {
                let mut test_cmd = std::process::Command::new("npm");
                test_cmd.args(["test", "--", "--passWithNoTests"]).current_dir(isolated_path);
                run_timed_cmd(test_cmd, "npm test", cmd_timeout)?;
            } else if is_python {
                let mut test_cmd = std::process::Command::new("python3");
                test_cmd.args(["-m", "unittest"]).current_dir(isolated_path);
                run_timed_cmd(test_cmd, "python unittest", cmd_timeout)?;
            } else if is_go {
                let mut test_cmd = std::process::Command::new("go");
                test_cmd.args(["test", "./..."]).current_dir(isolated_path);
                run_timed_cmd(test_cmd, "go test", cmd_timeout)?;
            }

            Ok(())
        });

        if let Err(e) = test_result {
            let err = format!(
                "Integration verification failed in isolated worktree: {}",
                e
            );
            let _ = cell
                .status_tx
                .send(AgentRunStatus::IntegrationRejected(err.clone()));
            return Err(anyhow!(err));
        }

        // 5. Commit validated changes to parent repo
        let default_msg = format!("feat(agent): integrate changes from agent {}", agent_id);
        let msg = commit_message.unwrap_or(&default_msg);
        let commit_sha = mgr.apply_and_commit_worktree_diff(&diff, msg, agent_id, &cell.role)?;

        let _ = cell.status_tx.send(AgentRunStatus::Integrated);
        let _ = self.storage.set_spawn_edge_status(agent_id, "integrated");

        Ok(format!(
            "Agent `{agent_id}` changes verified and integrated into parent repository: commit {}",
            commit_sha
        ))
    }

    async fn get_status(&self, agent_id: &str) -> Option<String> {
        let cells = self.cells.lock().unwrap();
        cells
            .get(agent_id)
            .map(|c| c.status_rx.borrow().label().to_string())
    }

    async fn get_transcript(&self, agent_id: &str) -> Option<Vec<String>> {
        let cells = self.cells.lock().unwrap();
        cells
            .get(agent_id)
            .map(|c| c.transcript.lock().unwrap().clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use m31a_core::SandboxPolicy;

    fn test_nim_client() -> ProviderClient {
        let api_key = m31a_core::resolve_stored_api_key()
            .unwrap_or_else(|| "nvapi-runtime-integration-token".to_string());
        let provider = m31a_provider::NimProvider::new(
            "https://integrate.api.nvidia.com/v1".to_string(),
            api_key,
            "nvidia/nemotron-3.5-lightning-30b-a3b".to_string(),
        );
        ProviderClient::from_provider(provider)
    }

    /// A runtime wired against an actual NIM client configuration — no requests are made in these
    /// bookkeeping tests (they never reach the model call).
    fn test_runtime() -> EngineAgentRuntime {
        let client = test_nim_client();
        let storage = Storage::open_in_memory().expect("in-memory storage");
        let temp_dir = std::env::temp_dir();
        let store = Arc::new(Mutex::new(m31a_trust::TrustStore::new()));
        store
            .lock()
            .unwrap()
            .trust(&temp_dir, m31a_trust::TrustLevel::SessionOnly)
            .unwrap();
        EngineAgentRuntime::new(
            client,
            ToolRegistry::new(),
            storage,
            ExecPolicy::standard(),
            temp_dir,
            None,
            128_000,
            3,
        )
        .with_trust_store(store)
    }

    #[test]
    fn test_depth_gate() {
        let rt = test_runtime();
        assert!(rt.ensure_depth(1).is_ok());
        assert!(rt.ensure_depth(MAX_SPAWN_DEPTH).is_ok());
        assert!(rt.ensure_depth(MAX_SPAWN_DEPTH + 1).is_err());
    }

    #[test]
    fn test_slot_reservation_cycle() {
        let rt = test_runtime();
        let mut guards = Vec::new();
        for _ in 0..MAX_CONCURRENT_THREADS {
            guards.push(rt.reserve_slot().expect("slot should reserve"));
        }
        assert!(rt.reserve_slot().is_err(), "limit exhausted");
        guards.pop().unwrap().release();
        assert!(rt.reserve_slot().is_ok(), "freed slot re-reservable");
    }

    #[test]
    fn test_caller_depth_walks_parent_chain() {
        let rt = test_runtime();

        // Seed a chain child_a (root child) ← child_b ← child_c.
        let mk_cell = |parent: Option<String>| {
            let (status_tx, status_rx) = watch::channel(AgentRunStatus::Completed);
            let (activity_tx, activity_rx) = watch::channel(InputQueueActivity::Idle);
            let (steer_tx, _steer_rx) = mpsc::unbounded_channel::<String>();
            Arc::new(AgentCell {
                parent,
                role: "coder".to_string(),
                task_name: "t".to_string(),
                steer_tx: Some(steer_tx),
                cancel_token: CancellationToken::new(),
                status_tx,
                status_rx,
                activity_tx,
                activity_rx,
                transcript: Mutex::new(Vec::new()),
                answer: Mutex::new(String::new()),
            })
        };
        let a = mk_cell(None);
        let b = mk_cell(Some("agent_a".to_string()));
        let c = mk_cell(Some("agent_b".to_string()));
        {
            let mut cells = rt.cells.lock().unwrap();
            cells.insert("agent_a".to_string(), a);
            cells.insert("agent_b".to_string(), b);
            cells.insert("agent_c".to_string(), c);
        }

        assert_eq!(rt.caller_depth(None), 0);
        assert_eq!(rt.caller_depth(Some("agent_a")), 1);
        assert_eq!(rt.caller_depth(Some("agent_c")), 3);
        // Unknown callers are impossible in the real flow (the task-local is
        // only set around cells this runtime created); conservatively they
        // count as one spawn from the root.
        assert_eq!(rt.caller_depth(Some("ghost_agent")), 1);
    }

    #[tokio::test]
    async fn test_unknown_agent_errors() {
        let rt = test_runtime();
        assert!(rt.send_input("ghost", "hi", false).await.is_err());
        assert!(rt.close_agent("ghost").await.is_err());
        let outs = rt
            .wait_agents(&["ghost".to_string()], Some(100))
            .await
            .unwrap();
        assert_eq!(outs[0].status, "unknown");
    }

    /// Full registry-arm lifecycle (§4.3): spawn via `ToolRegistry::execute`
    /// → close → wait, with the spawn edge persisted as closed. The child is
    /// cancelled before it is ever polled (current-thread test runtime), so
    /// no model request is attempted.
    #[tokio::test]
    async fn test_registry_spawn_close_wait_lifecycle() {
        let storage = Storage::open_in_memory().expect("in-memory storage");
        let tools = ToolRegistry::new();
        let cwd = std::env::temp_dir();
        let store = Arc::new(Mutex::new(m31a_trust::TrustStore::new()));
        store
            .lock()
            .unwrap()
            .trust(&cwd, m31a_trust::TrustLevel::SessionOnly)
            .unwrap();
        let runtime = Arc::new(
            EngineAgentRuntime::new(
                test_nim_client(),
                tools.clone(),
                storage.clone(),
                ExecPolicy::standard(),
                std::env::temp_dir(),
                None,
                128_000,
                3,
            )
            .with_trust_store(store),
        );
        tools.set_agent_runtime(runtime);
        let res = tools.execute(
            "spawn_agent",
            &serde_json::json!({"task_name": "lifecycle", "prompt": "do nothing", "role": "coder"}).to_string(),
            &cwd,
            SandboxPolicy::WorkspaceWrite,
        ).await.unwrap();
        assert!(res.contains("Spawned subagent `agent_coder_"), "got: {res}");
        assert!(res.contains("wait_agent"));

        let agent_id = res.split('`').nth(1).expect("agent id token").to_string();

        // Close before the child task is polled — deterministic cancel.
        let res = tools
            .execute(
                "close_agent",
                &serde_json::json!({"agent_id": agent_id}).to_string(),
                &cwd,
                SandboxPolicy::WorkspaceWrite,
            )
            .await
            .unwrap();
        assert!(res.contains("cancellation signalled"), "got: {res}");

        // Wait reports the terminal status.
        let res = tools
            .execute(
                "wait_agent",
                &serde_json::json!({"agent_ids": [agent_id]}).to_string(),
                &cwd,
                SandboxPolicy::WorkspaceWrite,
            )
            .await
            .unwrap();
        assert!(
            res.contains("cancelled") || res.contains("closed"),
            "got: {res}"
        );

        // Let the child task finalize; its spawn edge must be closed.
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        let edges = storage
            .list_spawn_children("current", Some("closed"))
            .unwrap();
        assert!(
            edges.iter().any(|e| e.child_thread_id == agent_id),
            "spawn edge should be closed after close_agent; edges: {edges:?}"
        );

        // list_agents surfaces the live cell.
        let res = tools
            .execute("list_agents", "{}", &cwd, SandboxPolicy::WorkspaceWrite)
            .await
            .unwrap();
        assert!(res.contains(&agent_id), "got: {res}");
    }

    #[tokio::test]
    async fn test_spawn_agent_with_worktree_isolation() {
        let repo_dir = tempfile::tempdir().unwrap();
        let _ = m31a_trust::TrustStore::new()
            .trust(repo_dir.path(), m31a_trust::TrustLevel::SessionOnly);
        let repo = git2::Repository::init(repo_dir.path()).unwrap();
        // Create initial commit
        {
            let sig = git2::Signature::now("M31A Test", "test@m31a.ai").unwrap();
            let mut index = repo.index().unwrap();
            let tree_id = index.write_tree().unwrap();
            let tree = repo.find_tree(tree_id).unwrap();
            repo.commit(Some("HEAD"), &sig, &sig, "initial", &tree, &[])
                .unwrap();
        }

        let storage = Storage::open_in_memory().unwrap();
        let tools = ToolRegistry::new();
        let wt_settings = m31a_storage::WorktreeSettings::new(repo_dir.path());
        let wt_manager = Arc::new(m31a_storage::WorktreeManager::new(wt_settings));

        let store = Arc::new(Mutex::new(m31a_trust::TrustStore::new()));
        store
            .lock()
            .unwrap()
            .trust(repo_dir.path(), m31a_trust::TrustLevel::SessionOnly)
            .unwrap();
        let runtime = Arc::new(
            EngineAgentRuntime::new(
                test_nim_client(),
                tools.clone(),
                storage,
                ExecPolicy::standard(),
                repo_dir.path().to_path_buf(),
                None,
                128_000,
                3,
            )
            .with_worktree_manager(wt_manager)
            .with_trust_store(store),
        );

        let req = AgentSpawnRequest {
            task_name: "worktree-task".to_string(),
            prompt: "isolated worktree task".to_string(),
            role_name: "coder".to_string(),
            model_override: None,
            depth: 1,
            max_depth: 3,
            isolate_worktree: true,
        };

        let spawned = runtime.spawn_agent(req).await.unwrap();
        assert!(
            spawned.worktree_path.is_some(),
            "Worktree path should be populated"
        );
        let wt_path = spawned.worktree_path.unwrap();
        assert!(
            wt_path.exists(),
            "Worktree path should exist on filesystem: {}",
            wt_path.display()
        );
    }

    #[tokio::test]
    async fn test_spawn_agent_fails_closed_when_worktree_unavailable() {
        let repo_dir = tempfile::tempdir().unwrap();
        let _ = m31a_trust::TrustStore::new()
            .trust(repo_dir.path(), m31a_trust::TrustLevel::SessionOnly);
        let storage = Storage::open_in_memory().unwrap();
        let tools = ToolRegistry::new();

        let store = Arc::new(Mutex::new(m31a_trust::TrustStore::new()));
        store
            .lock()
            .unwrap()
            .trust(repo_dir.path(), m31a_trust::TrustLevel::SessionOnly)
            .unwrap();
        // Runtime WITHOUT WorktreeManager
        let runtime = Arc::new(
            EngineAgentRuntime::new(
                test_nim_client(),
                tools.clone(),
                storage,
                ExecPolicy::standard(),
                repo_dir.path().to_path_buf(),
                None,
                128_000,
                3,
            )
            .with_trust_store(store),
        );

        let req = AgentSpawnRequest {
            task_name: "unisolated-hazard".to_string(),
            prompt: "task requiring isolation".to_string(),
            role_name: "coder".to_string(),
            model_override: None,
            depth: 1,
            max_depth: 3,
            isolate_worktree: true,
        };

        let res = runtime.spawn_agent(req).await;
        assert!(
            res.is_err(),
            "Must fail closed when isolation requested but unavailable"
        );
        let err_msg = res.unwrap_err().to_string();
        assert!(
            err_msg.contains("no WorktreeManager is configured")
                || err_msg.contains("Refusing unisolated execution"),
            "Error was: {err_msg}"
        );
    }

    #[tokio::test]
    async fn test_child_agent_rejected_in_unauthorized_workspace() {
        let repo_dir = tempfile::tempdir().unwrap();
        // Do NOT trust repo_dir -> untrusted directory
        let storage = Storage::open_in_memory().unwrap();
        let tools = ToolRegistry::new();
        let runtime = Arc::new(EngineAgentRuntime::new(
            test_nim_client(),
            tools.clone(),
            storage,
            ExecPolicy::standard(),
            repo_dir.path().to_path_buf(),
            None,
            128_000,
            3,
        ));

        let req = AgentSpawnRequest {
            task_name: "unauthorized-child".to_string(),
            prompt: "task in untrusted directory".to_string(),
            role_name: "coder".to_string(),
            model_override: None,
            depth: 1,
            max_depth: 3,
            isolate_worktree: false,
        };

        let res = runtime.spawn_agent(req).await;
        assert!(
            res.is_err(),
            "Must reject child agent spawn in untrusted workspace"
        );
        let err_msg = res.unwrap_err().to_string();
        assert!(
            err_msg.contains("untrusted") || err_msg.contains("REJECTED"),
            "Error message should mention untrusted or rejected: {err_msg}"
        );
    }

    #[test]
    fn test_safe_tail_truncate_ascii() {
        let text = "abcdefghijklmnopqrstuvwxyz";
        assert_eq!(safe_tail_truncate(text, 10), "…qrstuvwxyz");
        assert_eq!(safe_tail_truncate(text, 26), text);
        assert_eq!(safe_tail_truncate(text, 30), text);
        assert_eq!(safe_tail_truncate("", 10), "");
        assert_eq!(safe_tail_truncate(text, 0), "");
    }

    #[test]
    fn test_safe_tail_truncate_cjk() {
        // "你好世界测试" (6 chars, 18 UTF-8 bytes: each char is 3 bytes)
        let text = "你好世界测试";
        // 9 bytes = 3 CJK chars: "界测试"
        assert_eq!(safe_tail_truncate(text, 9), "…界测试");
        // 10 bytes: 3 chars fit (9 bytes), 4 chars = 12 bytes > 10. Must not split 4th char!
        assert_eq!(safe_tail_truncate(text, 10), "…界测试");
        // 11 bytes: still only 3 chars fit
        assert_eq!(safe_tail_truncate(text, 11), "…界测试");
        // 12 bytes: 4 chars fit: "世界测试"
        assert_eq!(safe_tail_truncate(text, 12), "…世界测试");
        // Full length: 18 bytes
        assert_eq!(safe_tail_truncate(text, 18), text);
    }

    #[test]
    fn test_safe_tail_truncate_emoji() {
        // 🦀 is 4 bytes, 🚀 is 4 bytes, 🎉 is 4 bytes. Total 12 bytes.
        let text = "🦀🚀🎉";
        // 4 bytes = 1 emoji "🎉"
        assert_eq!(safe_tail_truncate(text, 4), "…🎉");
        // 5, 6, 7 bytes: cannot split 🚀 (needs 8 bytes). Must safely yield "…🎉"
        assert_eq!(safe_tail_truncate(text, 5), "…🎉");
        assert_eq!(safe_tail_truncate(text, 6), "…🎉");
        assert_eq!(safe_tail_truncate(text, 7), "…🎉");
        assert_eq!(safe_tail_truncate(text, 8), "…🚀🎉");
        assert_eq!(safe_tail_truncate(text, 12), text);
    }

    #[test]
    fn test_safe_tail_truncate_combining_characters() {
        // 'e' + '\u{0301}' (combining acute accent) = 1 grapheme cluster (3 bytes: 'e' is 1 byte, acute is 2 bytes)
        let e_acute = "e\u{0301}";
        let text = format!("abc{}", e_acute);
        // Truncating to 3 bytes should retain the entire grapheme cluster "e\u{0301}"
        assert_eq!(safe_tail_truncate(&text, 3), format!("…{}", e_acute));
        // Truncating to 2 bytes cannot split the combining character cluster, so yields empty tail "…"
        assert_eq!(safe_tail_truncate(&text, 2), "…");
    }

    #[test]
    fn test_safe_tail_truncate_multi_byte_exactly_across_boundary() {
        // String of 398 ASCII 'a's followed by 1 CJK char '中' (3 bytes) -> total 401 bytes
        let mut text = "a".repeat(398);
        text.push('中'); // '中' occupies byte offsets 398..401
        assert_eq!(text.len(), 401);

        // Limit = 400 bytes.
        // Old unsafe code: `&output[401 - 400..]` = `&output[1..]` which was fine for ascii.
        // But what if boundary is right inside '中'?
        // E.g.: "a".repeat(399) + '中' (3 bytes) -> total 402 bytes.
        // Limit = 400. 402 - 400 = byte index 2.
        // If string had '中' at index 1..4, slicing index 2 panics!
        let evil = format!("x中{}", "a".repeat(398));
        // evil.len() = 1 + 3 + 398 = 402 bytes.
        // If max_bytes = 400, evil[402 - 400..] = evil[2..] which is in the MIDDLE of '中'!
        // safe_tail_truncate MUST NOT panic and must return valid UTF-8!
        let truncated = safe_tail_truncate(&evil, 400);
        assert!(truncated.starts_with('…'));
        assert_eq!(truncated, format!("…{}", "a".repeat(398)));
    }

    #[test]
    fn test_safe_tail_truncate_very_large_string() {
        let large = "🦀你好world".repeat(10_000); // 160,000 bytes
        let truncated = safe_tail_truncate(&large, 400);
        assert!(truncated.starts_with('…'));
        assert!(truncated.len() <= 400 + '…'.len_utf8());
        // Verify valid UTF-8
        std::str::from_utf8(truncated.as_bytes()).unwrap();
    }

    #[test]
    fn test_resource_governance_validation_and_application() {
        let gov = m31a_core::ResourceGovernance {
            max_concurrent_threads: 4,
            max_spawn_depth: 2,
            max_child_turns: 10,
            max_aggregate_tokens: Some(50_000),
        };
        assert!(gov.validate().is_ok());

        let invalid = m31a_core::ResourceGovernance {
            max_concurrent_threads: 0,
            max_spawn_depth: 2,
            max_child_turns: 10,
            max_aggregate_tokens: None,
        };
        assert!(invalid.validate().is_err());

        let rt = test_runtime().with_governance(gov.clone());
        assert_eq!(rt.governance.max_concurrent_threads, 4);
        assert_eq!(rt.governance.max_spawn_depth, 2);
        assert_eq!(rt.governance.max_child_turns, 10);
        assert_eq!(rt.governance.max_aggregate_tokens, Some(50_000));
    }

    #[tokio::test]
    async fn test_parent_mediated_approval_flow() {
        let (resp_tx, resp_rx) = tokio::sync::oneshot::channel();
        let approval = crate::turn_loop::PendingApproval {
            tool_call: m31a_core::types::ToolCall {
                id: "call-1".to_string(),
                call_type: "function".to_string(),
                function: m31a_core::types::FunctionCall {
                    name: "write_file".to_string(),
                    arguments: r#"{"path": "test.txt", "content": "hello"}"#.to_string(),
                },
            },
            reason: "Mutating file write".to_string(),
            command: None,
        };
        let event = EngineEvent::ApprovalRequested {
            approval,
            response_tx: resp_tx,
        };

        let mediator: ApprovalMediator = Arc::new(|_agent_id, pending| {
            if pending.tool_call.function.name == "write_file" {
                Some(m31a_elicitation::ApprovalDecision::AllowOnce)
            } else {
                Some(m31a_elicitation::ApprovalDecision::Deny {
                    reason: "Not permitted".to_string(),
                })
            }
        });

        let mut transcript = Vec::new();
        let tokens = Arc::new(AtomicUsize::new(0));
        let res = EngineAgentRuntime::fold_event_mediated(
            "child_1",
            event,
            &mut transcript,
            Some(&mediator),
            None,
            None,
            &tokens,
            None,
        );
        assert!(res.is_none());
        assert!(transcript[0].contains("[approval mediated by parent: approved]"));

        let decision = resp_rx.await.unwrap();
        assert!(matches!(
            decision,
            m31a_elicitation::ApprovalDecision::AllowOnce
        ));
    }

    #[tokio::test]
    async fn test_subagent_unapproved_mutation_denied_fail_closed() {
        let (resp_tx, resp_rx) = tokio::sync::oneshot::channel();
        let approval = crate::turn_loop::PendingApproval {
            tool_call: m31a_core::types::ToolCall {
                id: "call-2".to_string(),
                call_type: "function".to_string(),
                function: m31a_core::types::FunctionCall {
                    name: "write_file".to_string(),
                    arguments: r#"{"path": "test.txt"}"#.to_string(),
                },
            },
            reason: "Mutating file write".to_string(),
            command: None,
        };
        let event = EngineEvent::ApprovalRequested {
            approval,
            response_tx: resp_tx,
        };

        let mut transcript = Vec::new();
        let tokens = Arc::new(AtomicUsize::new(0));
        // No mediator and no channel -> fail-closed denial
        EngineAgentRuntime::fold_event_mediated(
            "child_2",
            event,
            &mut transcript,
            None,
            None,
            None,
            &tokens,
            None,
        );
        assert!(
            transcript[0].contains("[approval denied for 'write_file': requires parent approval]")
        );

        let decision = resp_rx.await.unwrap();
        assert!(matches!(
            decision,
            m31a_elicitation::ApprovalDecision::Deny { .. }
        ));
    }

    #[tokio::test]
    async fn test_subagent_read_only_tool_auto_approved() {
        let (resp_tx, resp_rx) = tokio::sync::oneshot::channel();
        let approval = crate::turn_loop::PendingApproval {
            tool_call: m31a_core::types::ToolCall {
                id: "call-3".to_string(),
                call_type: "function".to_string(),
                function: m31a_core::types::FunctionCall {
                    name: "read_file".to_string(),
                    arguments: r#"{"path": "README.md"}"#.to_string(),
                },
            },
            reason: "Read file".to_string(),
            command: None,
        };
        let event = EngineEvent::ApprovalRequested {
            approval,
            response_tx: resp_tx,
        };

        let mut transcript = Vec::new();
        let tokens = Arc::new(AtomicUsize::new(0));
        EngineAgentRuntime::fold_event_mediated(
            "child_3",
            event,
            &mut transcript,
            None,
            None,
            None,
            &tokens,
            None,
        );
        assert!(transcript[0].contains("[approval auto-approved read-only tool 'read_file']"));

        let decision = resp_rx.await.unwrap();
        assert!(matches!(
            decision,
            m31a_elicitation::ApprovalDecision::AllowOnce
        ));
    }

    #[test]
    fn test_aggregate_token_budget_enforcement() {
        let gov = m31a_core::ResourceGovernance {
            max_concurrent_threads: 4,
            max_spawn_depth: 3,
            max_child_turns: 10,
            max_aggregate_tokens: Some(100),
        };
        let rt = test_runtime().with_governance(gov);
        assert_eq!(rt.aggregate_tokens(), 0);

        rt.record_tokens(50);
        assert_eq!(rt.aggregate_tokens(), 50);

        rt.record_tokens(60);
        assert_eq!(rt.aggregate_tokens(), 110);
        // Tokens recorded atomically across hierarchy
    }
}
