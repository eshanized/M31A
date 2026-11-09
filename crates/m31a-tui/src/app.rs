//! Primary TUI application controller managing event loops, state transitions, and input modes.
//!
//! Coordinates user interactions, terminal keyboard events, streaming message updates, and modal views.

use anyhow::Result;
use m31a_core::config::Config;
use m31a_core::prompts::{CollaborationMode, SystemPromptBuilder};
use m31a_core::skills::SkillLoader;
use m31a_core::types::{ChatMessage, SessionMetadata, TokenUsage};
use m31a_core::{ApprovalPolicy, HookEngine, MemoryManager, SandboxPolicy, WorktreeManager};
use m31a_execpolicy::ExecPolicy;
use m31a_guardian::GuardianReviewer;
use m31a_provider::models::{ModelInfo, get_nvidia_model_catalog};
use m31a_provider::{ProviderBuilder, ProviderClient, ProviderConfig, ProviderType};
use m31a_sandbox::runner::SandboxRunner;
use m31a_storage::db::Storage;
use m31a_tools::registry::ToolRegistry;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot};

use crate::composer::ChatComposer;
use crate::modals::{ApprovalActionKind, ModalState, PendingApproval};
use crate::theme::{ThemeVariant, init_theme};
use crate::transcript::{NoticeLevel, TranscriptItem};
use crate::turn::{EventCorrelation, TurnState, TurnStateMachine};
use m31a_mcp::McpManager;

#[derive(Debug, Clone, PartialEq)]
pub enum AppStatus {
    Idle,
    Submitted,
    AwaitingAdmission,
    Reasoning(String),
    Streaming(String),
    ExecutingTool { tool_name: String },
    AwaitingApproval,
    AwaitingQuestion,
    Compacting,
    Cancelling,
    Failed(String),
}

impl From<&crate::turn::TurnState> for AppStatus {
    fn from(state: &crate::turn::TurnState) -> Self {
        match state {
            crate::turn::TurnState::Idle | crate::turn::TurnState::Completed { .. } => {
                AppStatus::Idle
            }
            crate::turn::TurnState::Submitted { .. } => AppStatus::Submitted,
            crate::turn::TurnState::AwaitingAdmission { .. } => AppStatus::AwaitingAdmission,
            crate::turn::TurnState::Thinking { preview, .. } => {
                AppStatus::Reasoning(preview.clone())
            }
            crate::turn::TurnState::Streaming { .. } => AppStatus::Streaming(String::new()),
            crate::turn::TurnState::ExecutingTool { tool_name, .. } => AppStatus::ExecutingTool {
                tool_name: tool_name.clone(),
            },
            crate::turn::TurnState::AwaitingApproval { .. } => AppStatus::AwaitingApproval,
            crate::turn::TurnState::AwaitingQuestion { .. } => AppStatus::AwaitingQuestion,
            crate::turn::TurnState::Compacting { .. } => AppStatus::Compacting,
            crate::turn::TurnState::CancellationRequested { .. }
            | crate::turn::TurnState::Cancelling { .. } => AppStatus::Cancelling,
            crate::turn::TurnState::Failed { error, .. } => AppStatus::Failed(error.clone()),
            crate::turn::TurnState::Recovering { .. } => AppStatus::Idle,
        }
    }
}

#[derive(Debug)]
pub enum TuiEvent {
    Correlated {
        correlation: EventCorrelation,
        event: Box<TuiEvent>,
    },
    ContentChunk(String),
    ReasoningChunk(String),
    ToolCallStarted {
        id: String,
        name: String,
        args: String,
    },
    ToolCallFinished {
        id: String,
        output: String,
        success: bool,
    },
    ShellCommandFinished {
        id: String,
        output: String,
        success: bool,
        duration_ms: u64,
    },
    Notice(String, NoticeLevel),
    ProviderStatus(String),
    ApprovalRequested {
        approval: PendingApproval,
        response_tx: oneshot::Sender<m31a_elicitation::ApprovalDecision>,
    },
    QuestionRequested {
        question: m31a_elicitation::QuestionItem,
        response_tx: oneshot::Sender<Vec<String>>,
    },
    Compacted {
        original_tokens: usize,
        compacted_tokens: usize,
    },
    TurnComplete {
        usage: TokenUsage,
        messages: Vec<ChatMessage>,
    },
    TurnFailed(String),
    WorkspaceChanged(m31a_watcher::WorkspaceChangeBatch),
    ModelsDiscovered(Vec<ModelInfo>),
    CodeModeResult(String),
    ProcessListUpdated(Vec<m31a_proc::ProcessInfo>),
    ProcessOutputReady {
        id: String,
        label: String,
        lines: Vec<m31a_proc::ProcessLine>,
    },
    SetupConnectivityResult {
        generation: u64,
        status: crate::modals::ConnectivityStatus,
        details: Option<crate::modals::ConnectivityDetails>,
        error: Option<String>,
    },
    ModelHealthResult {
        model: String,
        state: crate::model_selector::ModelHealthState,
    },
    ModelActivationResult {
        slug: String,
        candidate_client: ProviderClient,
        result: std::result::Result<(), String>,
    },
    SessionResumeResult {
        thread_id: String,
        client: ProviderClient,
        thread: m31a_storage::db::ThreadRecord,
        result: std::result::Result<(), String>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FocusState {
    #[default]
    Composer,
    Transcript,
    ContextPanel,
    Screen,
}

pub struct App {
    pub config: Config,
    pub cwd: PathBuf,
    pub active_model: String,
    pub available_models: Vec<ModelInfo>,
    pub nim_client: ProviderClient,
    pub storage: Storage,
    pub tools: ToolRegistry,
    pub sandbox_runner: SandboxRunner,
    pub guardian: Arc<GuardianReviewer>,
    /// Hook dispatcher (report §4.4): built from discovered hooks config.
    pub hooks: Arc<m31a_hooks::HookRuntime>,
    /// Code-mode orchestrator (report §2.4): hosted on the App so cells and the
    /// session KV store persist across turns; `None` when the feature is off.
    pub code_mode: Option<m31a_code_mode::CodeModeOrchestrator>,
    pub exec_policy: ExecPolicy,
    pub collaboration_mode: CollaborationMode,

    pub transcript: Vec<TranscriptItem>,
    pub composer: ChatComposer,
    pub scroll_offset: usize,
    pub auto_scroll: bool,
    pub modal_stack: crate::modals::ModalStack,
    /// Asynchronous prompt queue for typing and submitting prompts while agent executes (Ctrl+Enter)
    pub queued_prompts: std::collections::VecDeque<String>,
    /// CSI 1004 terminal focus tracking state (true = terminal window focused, false = blurred)
    pub is_focused: bool,
    pub turn_state: TurnStateMachine,
    pub session_epoch: u64,
    pub current_turn_id: Option<String>,
    pub turn_start_time: Option<std::time::Instant>,
    pub turn_ttft_ms: Option<u64>,
    pub active_persona: Option<String>,
    pub mcp_manager: Arc<McpManager>,
    pub tokens_used: u64,
    pub session_id: String,
    pub should_quit: bool,
    pub pinned_context: Vec<m31a_core::DynamicContextItem>,

    pub chat_history: Vec<ChatMessage>,
    pub approval_tx: Option<oneshot::Sender<m31a_elicitation::ApprovalDecision>>,
    pub question_tx: Option<oneshot::Sender<Vec<String>>>,
    pub steer_tx: Option<mpsc::UnboundedSender<String>>,
    pub cancel_token: Option<tokio_util::sync::CancellationToken>,
    pub event_tx: mpsc::Sender<TuiEvent>,
    pub event_rx: mpsc::Receiver<TuiEvent>,
    pub last_turn_critical_path: Option<m31a_core::CausalPathDurations>,
    pub direct_tool_timings: Arc<m31a_core::DirectToolTimings>,
    pub active_goal: Option<m31a_core::goal_steering::GoalObjective>,

    // Advanced modular TUI capabilities
    pub session_metrics: m31a_analytics::SessionTokenMetrics,
    pub mentions_popup: m31a_composer::MentionsPopup,
    pub slash_popup: m31a_composer::SlashPopup,
    pub paste_detector: m31a_composer::PasteBurstDetector,
    pub terminal_caps: m31a_term::TerminalCapabilities,

    // Deep runtime integration components
    pub chat_widget: crate::chatwidget::ChatWidgetState,
    pub bottom_pane: crate::bottom_pane::BottomPaneState,
    pub transcript_virtualizer: crate::transcript_view::TranscriptVirtualizer,
    pub proc_supervisor: Arc<m31a_proc::ProcessManager>,
    pub watcher: Option<Arc<m31a_watcher::WorkspaceWatcher>>,
    /// §5.3: SHA-1 digests of the last system-prompt build, so prompt
    /// refreshes can report exactly which world-state sections changed.
    pub world_state_digests: std::collections::BTreeMap<String, String>,
    /// §7.1: feature registry for experimental features menu.
    pub features_registry: Arc<m31a_core::features::FeatureRegistry>,
    /// §7.6: double-press Ctrl+C tracker.
    pub last_ctrl_c_press: Option<std::time::Instant>,
    /// §7.6: quick dials (numbered reusable prompts with HUD).
    pub quick_dials: std::collections::BTreeMap<u8, String>,
    /// Mouse selection coordinates for terminal copy-on-select.
    pub mouse_selection: Option<crate::mouse::MouseSelection>,
    /// Queue for buffering interrupts during active turn streaming
    pub deferred_interrupts: crate::interrupts::DeferredInterruptQueue,
    /// Persistent workspace trust store and directory boundary evaluator
    pub trust_store: Arc<std::sync::Mutex<m31a_trust::TrustStore>>,
    pub engine_runtime: Arc<m31a_agent_engine::agents::EngineAgentRuntime>,
    pub agent_runtime: Arc<dyn m31a_tools::agent_runtime::AgentRuntime>,
    pub vault: Arc<m31a_core::LocalEncryptedVault>,
    pub terminal_width: u16,
    pub terminal_height: u16,
    /// Terminal chime / audio bell on turn completion, approval, and failures
    pub sound_enabled: bool,
    /// Active thread ID when drilling down into a subagent conversation (`None` for main session).
    pub active_thread_id: Option<String>,
    /// Currently active screen out of the 10 primary developer screens
    pub current_screen: crate::screens::Screen,
    /// Whether the collapsible contextual inspector side pane is open (Ctrl+B / F12)
    pub inspector_open: bool,
    /// Selected item index within the active screen
    pub screen_selected_index: usize,
    /// Text filter input for screens with searchable tables
    pub screen_filter: String,
    /// Whether the screen text filter input is currently receiving keyboard input
    pub screen_filter_active: bool,
    /// Last provider connectivity test outcome message
    pub provider_status_message: Option<String>,
    /// Stateful model list for the dedicated Models screen
    pub models_screen_state: Option<crate::model_selector::ModelListState>,
    /// Active focus target for predictable keyboard routing
    pub focus: FocusState,
    /// Currently selected tab in the contextual workspace panel
    pub context_panel_tab: crate::panel::PanelTab,
    /// Width percentage allocated to contextual workspace panel (20..50, default 30)
    pub context_panel_width_pct: u16,
    /// Preserved composer draft text per session ID
    pub session_drafts: std::collections::HashMap<String, String>,
    /// Preserved transcript scroll position per session ID
    pub session_scrolls: std::collections::HashMap<String, usize>,
}

fn classify_connectivity_error(err_str: &str) -> crate::modals::ConnectivityStatus {
    let lower = err_str.to_lowercase();
    if lower.contains("401")
        || lower.contains("403")
        || lower.contains("unauthorized")
        || lower.contains("forbidden")
        || lower.contains("invalid api key")
        || lower.contains("authentication")
        || lower.contains("bad api key")
        || lower.contains("missing api key")
        || lower.contains("api key required")
    {
        crate::modals::ConnectivityStatus::AuthenticationFailed
    } else if lower.contains("404")
        || lower.contains("not found")
        || lower.contains("model not found")
        || lower.contains("unknown model")
        || lower.contains("unsupported model")
        || lower.contains("does not exist")
    {
        crate::modals::ConnectivityStatus::ModelUnsupported
    } else if lower.contains("timeout") || lower.contains("timed out") || lower.contains("deadline")
    {
        crate::modals::ConnectivityStatus::TimedOut
    } else if lower.contains("connection refused")
        || lower.contains("dns")
        || lower.contains("resolve")
        || lower.contains("unreachable")
        || lower.contains("network is down")
        || lower.contains("failed to connect")
    {
        crate::modals::ConnectivityStatus::Unreachable
    } else {
        crate::modals::ConnectivityStatus::Failed
    }
}

impl App {
    pub fn new(config: Config) -> Result<Self> {
        let cwd = std::env::current_dir()?;
        let storage = Storage::open_default()?;
        Self::new_isolated(config, storage, cwd)
    }

    /// Isolated constructor for tests and sandboxed runs guaranteeing no writes to default `~/.m31a`.
    pub fn new_test(mut config: Config, temp_dir: &std::path::Path) -> Result<Self> {
        let db_path = temp_dir.join("m31a_test.db");
        let storage = Storage::open(&db_path)?;
        let vault_dir = temp_dir.join("vault");
        std::fs::create_dir_all(&vault_dir)?;
        let vault = Arc::new(m31a_core::LocalEncryptedVault::new(vault_dir)?);
        let mcp_manager = Arc::new(m31a_mcp::McpManager::new(config.mcp.clone()));
        let trust_store = Arc::new(std::sync::Mutex::new(m31a_trust::TrustStore::new()));
        if config.api_key.is_none() {
            config.api_key = Some("test-credential".to_string());
        }
        if config.model == "mock-model"
            || config.model == "mock-testing-model"
            || config.model.is_empty()
        {
            config.model = "nvidia/nemotron-3.5-lightning-30b-a3b".to_string();
        }
        if config.model_provider.as_deref() == Some("mock") {
            config.model_provider = Some("nim".to_string());
        }
        Self::new_isolated_with_deps(
            config,
            storage,
            temp_dir.to_path_buf(),
            vault,
            mcp_manager,
            trust_store,
        )
    }

    /// Full isolated constructor taking explicit storage backend and workspace directory.
    pub fn new_isolated(config: Config, storage: Storage, cwd: PathBuf) -> Result<Self> {
        let vault_dir = cwd.join(".m31a-vault");
        let _ = std::fs::create_dir_all(&vault_dir);
        let vault = Arc::new(m31a_core::LocalEncryptedVault::new(vault_dir)?);
        let mcp_manager = Arc::new(m31a_mcp::McpManager::new(config.mcp.clone()));
        let trust_store = Arc::new(std::sync::Mutex::new(m31a_trust::TrustStore::new()));
        Self::new_isolated_with_deps(config, storage, cwd, vault, mcp_manager, trust_store)
    }

    /// Fully dependency-injected isolated constructor.
    pub fn new_isolated_with_deps(
        config: Config,
        storage: Storage,
        cwd: PathBuf,
        vault: Arc<m31a_core::LocalEncryptedVault>,
        mcp_manager: Arc<McpManager>,
        trust_store: Arc<std::sync::Mutex<m31a_trust::TrustStore>>,
    ) -> Result<Self> {
        let active_model = config.model.clone();
        let cached_models = m31a_provider::load_cached_models();
        let mut available_models = get_nvidia_model_catalog();
        for m in cached_models {
            if !available_models
                .iter()
                .any(|existing| existing.slug == m.slug)
            {
                available_models.push(m);
            }
        }
        if !available_models.iter().any(|m| m.slug == active_model) {
            let model_info = if let Some(m) = m31a_provider::find_model(&active_model) {
                m
            } else if let Some(c) = m31a_provider::find_canonical_model(&active_model) {
                ModelInfo {
                    slug: c.id.clone(),
                    display_name: c.name.clone(),
                    description: c.description.clone(),
                    context_window: c.context_window.unwrap_or(0),
                    max_output_tokens: c.max_output_tokens.unwrap_or(0),
                    supports_tools: c.capabilities.tool_calling,
                    supports_thinking: c.capabilities.reasoning,
                    supports_vision: c.capabilities.vision,
                    supports_audio: c.capabilities.audio,
                    originator: c.publisher.clone(),
                    ..ModelInfo::default()
                }
            } else {
                ModelInfo {
                    slug: active_model.clone(),
                    display_name: active_model.clone(),
                    description: format!("Configured custom model: {}", active_model),
                    context_window: 0,
                    max_output_tokens: 0,
                    supports_tools: false,
                    supports_thinking: false,
                    supports_vision: false,
                    supports_audio: false,
                    originator: "custom".to_string(),
                    ..ModelInfo::default()
                }
            };
            available_models.insert(0, model_info);
        }

        let provider_type = ProviderType::resolve(
            config.model_provider.as_deref(),
            &active_model,
            Some(&config.api_base),
        )?;
        let provider_config = ProviderConfig {
            provider_type,
            api_base: Some(config.api_base.clone()),
            api_key: config.api_key.clone().filter(|k| !k.trim().is_empty()),
            model: active_model.clone(),
            extra: std::collections::HashMap::new(),
        };
        m31a_provider::validate_provider_config(&provider_config)?;
        let nim_client = ProviderBuilder::build_client(provider_config)?
            .with_reasoning_effort(config.reasoning_effort.clone());

        let tools = ToolRegistry::new_isolated(
            Some(storage.clone()),
            vault.clone(),
            mcp_manager.clone(),
            config.extra_dirs.clone(),
            &cwd,
        );
        let sandbox_runner = SandboxRunner::new();
        let guardian = Arc::new(GuardianReviewer::new(&active_model));
        let hooks = Arc::new(m31a_hooks::HookRuntime::new(
            m31a_hooks::HookConfig::discover(&cwd),
        ));
        let code_mode = if config.features.get("code_mode").copied().unwrap_or(false) {
            Some(m31a_code_mode::CodeModeOrchestrator::new())
        } else {
            None
        };
        let exec_policy = ExecPolicy::standard();

        let engine_runtime = Arc::new(
            m31a_agent_engine::agents::EngineAgentRuntime::new(
                nim_client.clone(),
                tools.clone(),
                storage.clone(),
                exec_policy.clone(),
                cwd.clone(),
                Some(hooks.clone()),
                config.model_context_window,
                config.agents.max_depth as u32,
            )
            .with_trust_store(trust_store.clone()),
        );
        let tools = tools;
        tools.set_agent_runtime(engine_runtime.clone());
        let collaboration_mode = CollaborationMode::Default;

        let session_id = uuid::Uuid::new_v4().to_string();

        let (event_tx, event_rx) = mpsc::channel(1024);

        let snapshot = m31a_core::prompts::PromptRuntimeSnapshot::new(
            &cwd,
            &active_model,
            &format!("{:?}", config.sandbox),
            collaboration_mode,
        )
        .with_user_custom_instructions(config.system_prompt.clone())
        .with_provider(
            config
                .model_provider
                .as_deref()
                .unwrap_or("NVIDIA NIM / Upstream Provider"),
        );

        let initial_state = SystemPromptBuilder::sections_canonical(&snapshot);
        let initial_sys_prompt = initial_state.render();
        let world_state_digests = initial_state.section_digests();
        let chat_history = vec![ChatMessage::system(initial_sys_prompt)];

        let mut bottom_pane = crate::bottom_pane::BottomPaneState::new(
            &active_model,
            format!("{:?}", config.sandbox),
        );
        bottom_pane.reasoning_effort = config.reasoning_effort.clone();
        bottom_pane.git_status = Self::detect_git_status(&cwd);
        let max_context = config.model_context_window;
        bottom_pane.token_meter = Some(crate::bottom_pane::TokenContextMeter::new(0, max_context));
        let chat_widget = crate::chatwidget::ChatWidgetState::new(24);
        let transcript_virtualizer = crate::transcript_view::TranscriptVirtualizer::new();
        let proc_supervisor = tools.proc_supervisor.clone();

        // Initialize theme based on user configuration or ambient terminal probe
        if let Some(theme_name) = &config.tui.theme {
            match theme_name.to_lowercase().as_str() {
                "light" => init_theme(ThemeVariant::Light),
                "contrast" | "high-contrast" | "hc" => init_theme(ThemeVariant::HighContrast),
                "everblush" | "dark" => init_theme(ThemeVariant::Everblush),
                _ => init_theme(ThemeVariant::Everblush),
            }
        } else {
            let detected = crate::terminal_detection::detect_terminal_theme_variant();
            init_theme(detected);
        }

        if let Some(ref icon_pref) = config.tui.icons {
            crate::theme::set_icon_mode(Some(crate::icons::IconMode::parse_str(icon_pref)));
        } else {
            crate::theme::set_icon_mode(None);
        }

        // Dynamically fetch models from provider endpoint in background if cache is stale (daily TTL) or empty
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            let api_base_models = config.api_base.clone();
            let api_key_models = config.api_key.clone().unwrap_or_default();
            let event_tx_models = event_tx.clone();
            handle.spawn(async move {
                if m31a_provider::is_models_cache_stale(
                    m31a_provider::DEFAULT_MODELS_CACHE_TTL_SECS,
                ) && let Ok(discovered) =
                    m31a_provider::fetch_remote_models(&api_base_models, &api_key_models).await
                    && !discovered.is_empty()
                {
                    m31a_provider::save_cached_models(&discovered);
                    let _ = event_tx_models
                        .send(TuiEvent::ModelsDiscovered(discovered))
                        .await;
                }
            });
        }

        let mut watcher_opt = None;
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            let watcher = Arc::new(m31a_watcher::WorkspaceWatcher::new(cwd.clone(), None));
            let mut watcher_rx = watcher.subscribe();
            watcher.start();

            let event_tx_watcher = event_tx.clone();
            handle.spawn(async move {
                while let Ok(batch) = watcher_rx.recv().await {
                    let _ = event_tx_watcher
                        .send(TuiEvent::WorkspaceChanged(batch))
                        .await;
                }
            });
            watcher_opt = Some(watcher);
        }

        let config_features = config.features.clone();

        let mut app = Self {
            config,
            cwd,
            active_model,
            available_models,
            nim_client,
            storage,
            tools,
            sandbox_runner,
            guardian,
            hooks,
            code_mode,
            exec_policy,
            collaboration_mode,
            world_state_digests,
            transcript: Vec::new(),
            composer: ChatComposer::new(),
            scroll_offset: 0,
            auto_scroll: true,
            modal_stack: crate::modals::ModalStack::new(),
            queued_prompts: std::collections::VecDeque::new(),
            is_focused: true,
            turn_state: crate::turn::TurnStateMachine::new(),
            session_epoch: 1,
            current_turn_id: None,
            turn_start_time: None,
            turn_ttft_ms: None,
            active_persona: None,
            mcp_manager,
            tokens_used: 0,
            session_id: session_id.clone(),
            should_quit: false,
            pinned_context: Vec::new(),
            chat_history,
            approval_tx: None,
            question_tx: None,
            steer_tx: None,
            cancel_token: None,
            event_tx,
            event_rx,
            last_turn_critical_path: None,
            direct_tool_timings: Arc::new(m31a_core::DirectToolTimings::new()),
            active_goal: None,
            session_metrics: m31a_analytics::SessionTokenMetrics::default(),
            mentions_popup: m31a_composer::MentionsPopup::new(),
            slash_popup: m31a_composer::SlashPopup::new(),
            paste_detector: m31a_composer::PasteBurstDetector::new(),
            terminal_caps: m31a_term::TerminalCapabilities::detect(),
            chat_widget,
            bottom_pane,
            transcript_virtualizer,
            proc_supervisor,
            watcher: watcher_opt,
            features_registry: {
                let mut reg = m31a_core::features::FeatureRegistry::new();
                reg.sync_from_map(&config_features);
                Arc::new(reg)
            },
            last_ctrl_c_press: None,
            quick_dials: {
                let mut map = std::collections::BTreeMap::new();
                map.insert(
                    1,
                    "Summarize recent changes and verify workspace".to_string(),
                );
                map.insert(2, "Run tests and diagnose failures".to_string());
                map.insert(3, "Explain architecture and code structure".to_string());
                map.insert(4, "Review code changes for security issues".to_string());
                map
            },
            mouse_selection: None,
            deferred_interrupts: crate::interrupts::DeferredInterruptQueue::new(),
            trust_store,
            engine_runtime: engine_runtime.clone(),
            agent_runtime: engine_runtime,
            vault,
            terminal_width: 80,
            terminal_height: 24,
            sound_enabled: true,
            active_thread_id: None,
            current_screen: crate::screens::Screen::Chat,
            inspector_open: false,
            screen_selected_index: 0,
            screen_filter: String::new(),
            screen_filter_active: false,
            provider_status_message: None,
            models_screen_state: None,
            focus: FocusState::Composer,
            context_panel_tab: crate::panel::PanelTab::Files,
            context_panel_width_pct: 30,
            session_drafts: std::collections::HashMap::new(),
            session_scrolls: std::collections::HashMap::new(),
        };

        // Initialize welcome message in transcript with clean structural separation
        let welcome = format!(
            "M31A Autonomous Software Engineering Runtime\n\
             • Runtime:  v0.1.14\n\
             • Model:    {}\n\
             • Policy:   Sandbox ({:?}) │ Approval ({:?})\n\
             • Mode:     {:?}\n\
             • Guidance: Type /help for command index or press F1-F10 for screens",
            app.active_model, app.config.sandbox, app.config.approval, app.collaboration_mode
        );
        app.transcript
            .push(TranscriptItem::notice(welcome, NoticeLevel::Info));

        // Evaluate workspace trust and git hook armor
        let decision = app.trust_store.lock().unwrap().evaluate(&app.cwd);
        match decision {
            m31a_trust::TrustDecision::Trusted => {
                app.transcript.push(TranscriptItem::notice(
                    format!(
                        "Workspace verified: '{}' is a trusted directory.",
                        app.cwd.display()
                    ),
                    NoticeLevel::Success,
                ));
            }
            m31a_trust::TrustDecision::SessionOnly => {
                app.transcript.push(TranscriptItem::notice(
                    format!(
                        "Workspace trusted for this session only: '{}'.",
                        app.cwd.display()
                    ),
                    NoticeLevel::Info,
                ));
            }
            m31a_trust::TrustDecision::RequiresConfirmation {
                ref path,
                ref reason,
            } => {
                app.transcript.push(TranscriptItem::notice(
                    format!(
                        "Untrusted workspace: '{}' is not registered in trust store ({reason}).\nType `/trust add` to trust permanently, or `/trust audit` to inspect git hook security.",
                        path.display()
                    ),
                    NoticeLevel::Warning,
                ));
            }
            m31a_trust::TrustDecision::Denied { ref path } => {
                app.transcript.push(TranscriptItem::notice(
                    format!(
                        "Access denied: '{}' is marked Denied by workspace trust security policy.",
                        path.display()
                    ),
                    NoticeLevel::Error,
                ));
            }
        }

        app.sync_virtualizer();

        // Save session metadata
        let meta = SessionMetadata {
            id: session_id,
            title: "New M31A Session".to_string(),
            model: app.active_model.clone(),
            cwd: app.cwd.to_string_lossy().to_string(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            extra: std::collections::HashMap::new(),
        };
        let _ = app
            .storage
            .save_thread(&meta, "", "WorkspaceWrite", "OnRequest");

        Ok(app)
    }

    pub fn detect_git_status(cwd: &std::path::Path) -> Option<crate::bottom_pane::GitStatusBadge> {
        let branch_out = std::process::Command::new("git")
            .args(["rev-parse", "--abbrev-ref", "HEAD"])
            .current_dir(cwd)
            .output()
            .ok()?;
        if !branch_out.status.success() {
            return None;
        }
        let branch = String::from_utf8_lossy(&branch_out.stdout)
            .trim()
            .to_string();
        if branch.is_empty() {
            return None;
        }

        let status_out = std::process::Command::new("git")
            .args(["status", "--porcelain"])
            .current_dir(cwd)
            .output()
            .ok()?;
        let status_text = String::from_utf8_lossy(&status_out.stdout);
        let dirty_count = status_text.lines().filter(|l| !l.trim().is_empty()).count();
        Some(crate::bottom_pane::GitStatusBadge::new(
            branch,
            dirty_count > 0,
            dirty_count,
        ))
    }

    pub fn switch_screen(&mut self, screen: crate::screens::Screen) {
        self.current_screen = screen;
        self.screen_selected_index = 0;
        self.screen_filter.clear();
        self.screen_filter_active = false;

        // Initialize model list state for Models screen
        if screen == crate::screens::Screen::Models {
            let mut state = crate::model_selector::ModelListState::new(
                self.available_models.clone(),
                self.active_model.clone(),
            );
            state.set_viewport_height(20);
            self.models_screen_state = Some(state);
        } else {
            self.models_screen_state = None;
        }
    }

    pub fn next_screen(&mut self) {
        let next_idx = (self.current_screen as usize + 1) % crate::screens::Screen::ALL.len();
        self.switch_screen(crate::screens::Screen::ALL[next_idx]);
    }

    pub fn prev_screen(&mut self) {
        let prev_idx = if self.current_screen as usize == 0 {
            crate::screens::Screen::ALL.len() - 1
        } else {
            self.current_screen as usize - 1
        };
        self.switch_screen(crate::screens::Screen::ALL[prev_idx]);
    }

    pub fn toggle_inspector(&mut self) {
        self.inspector_open = !self.inspector_open;
    }

    pub fn cycle_panel_tab_next(&mut self) {
        let all = crate::panel::PanelTab::ALL;
        let pos = all
            .iter()
            .position(|t| *t == self.context_panel_tab)
            .unwrap_or(0);
        self.context_panel_tab = all[(pos + 1) % all.len()];
    }

    pub fn cycle_panel_tab_prev(&mut self) {
        let all = crate::panel::PanelTab::ALL;
        let pos = all
            .iter()
            .position(|t| *t == self.context_panel_tab)
            .unwrap_or(0);
        self.context_panel_tab = if pos > 0 {
            all[pos - 1]
        } else {
            all[all.len() - 1]
        };
    }

    pub fn set_panel_tab(&mut self, tab: crate::panel::PanelTab) {
        self.context_panel_tab = tab;
    }

    pub fn resize_panel_wider(&mut self) {
        self.context_panel_width_pct = (self.context_panel_width_pct + 5).min(50);
    }

    pub fn resize_panel_narrower(&mut self) {
        self.context_panel_width_pct = (self.context_panel_width_pct.saturating_sub(5)).max(20);
    }

    pub fn save_current_session_draft(&mut self) {
        if !self.composer.buffer.is_empty() {
            self.session_drafts
                .insert(self.session_id.clone(), self.composer.buffer.clone());
        } else {
            self.session_drafts.remove(&self.session_id);
        }
        self.session_scrolls
            .insert(self.session_id.clone(), self.scroll_offset);
    }

    pub fn restore_session_draft(&mut self, thread_id: &str) {
        if let Some(draft) = self.session_drafts.get(thread_id) {
            self.composer.buffer = draft.clone();
            self.composer.cursor = draft.len();
        } else {
            self.composer.buffer.clear();
            self.composer.cursor = 0;
        }
        if let Some(scroll) = self.session_scrolls.get(thread_id) {
            self.scroll_offset = *scroll;
        }
    }

    pub fn create_new_session(&mut self, custom_title: Option<&str>) -> Result<String> {
        self.save_current_session_draft();
        let new_id = uuid::Uuid::new_v4().to_string();
        let title = custom_title.unwrap_or("New Session").to_string();
        let meta = SessionMetadata {
            id: new_id.clone(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            title: title.clone(),
            model: self.active_model.clone(),
            cwd: self.cwd.to_string_lossy().to_string(),
            extra: std::collections::HashMap::new(),
        };
        let _ = self.storage.save_thread(
            &meta,
            "",
            &format!("{:?}", self.config.sandbox),
            &format!("{:?}", self.config.approval),
        );
        self.session_id = new_id.clone();
        self.transcript.clear();
        self.chat_history.clear();
        self.composer.clear();
        self.scroll_offset = 0;
        self.current_screen = crate::screens::Screen::Chat;
        self.transcript.push(TranscriptItem::notice(
            format!(
                "Started new development session '{}' ({})",
                title,
                &new_id[..new_id.len().min(8)]
            ),
            NoticeLevel::Success,
        ));
        self.sync_virtualizer();
        Ok(new_id)
    }

    pub fn sync_virtualizer(&mut self) {
        self.transcript_virtualizer =
            crate::transcript_view::TranscriptVirtualizer::from_transcript_items(&self.transcript);
        let width = if self.terminal_width > 4 {
            self.terminal_width - 4
        } else {
            80
        };
        let total_lines = self.transcript_virtualizer.total_line_count(width);
        self.chat_widget
            .scroll_controller
            .on_new_content(total_lines);
    }

    /// Derived canonical AppStatus derived directly from TurnStateMachine.
    pub fn app_status(&self) -> AppStatus {
        AppStatus::from(self.turn_state.state())
    }

    /// Pushes a new modal onto the modal stack, activating it.
    pub fn push_modal(&mut self, modal: ModalState) {
        self.modal_stack.push(modal);
    }

    /// Pops the active modal from the modal stack, restoring the underlying modal if any.
    pub fn pop_modal(&mut self) -> Option<ModalState> {
        self.modal_stack.pop()
    }

    /// Replaces the topmost active modal on the modal stack.
    pub fn replace_modal(&mut self, modal: ModalState) {
        self.modal_stack.replace(modal);
    }

    /// Clears all stacked modals and dismisses overlays.
    pub fn clear_modals(&mut self) {
        self.modal_stack.clear();
    }

    /// Accessor for currently active topmost modal.
    pub fn current_modal(&self) -> Option<&ModalState> {
        self.modal_stack.current()
    }

    /// Mutable accessor for currently active topmost modal.
    pub fn current_modal_mut(&mut self) -> Option<&mut ModalState> {
        self.modal_stack.current_mut()
    }

    /// Returns the current nesting depth of modals.
    pub fn modal_depth(&self) -> usize {
        self.modal_stack.len()
    }

    /// Returns true if an overlay modal is currently active.
    pub fn has_active_modal(&self) -> bool {
        !self.modal_stack.is_empty()
    }

    /// Applies a reasoning effort override across config, active client, and bottom pane transactionally.
    pub fn apply_reasoning_effort(&mut self, new_effort: Option<String>) -> anyhow::Result<()> {
        let normalized = match new_effort.as_deref() {
            None | Some("none") | Some("") => None,
            Some("low") => Some("low".to_string()),
            Some("medium") => Some("medium".to_string()),
            Some("high") => Some("high".to_string()),
            Some(other) => anyhow::bail!(
                "Invalid reasoning effort '{other}'. Valid options: low, medium, high, none"
            ),
        };

        let mut candidate_config = self.config.clone();
        candidate_config.reasoning_effort = normalized.clone();
        candidate_config
            .save()
            .map_err(|e| anyhow::anyhow!("Failed to persist reasoning effort: {e}"))?;

        self.config = candidate_config;
        self.nim_client = self
            .nim_client
            .clone()
            .with_reasoning_effort(normalized.clone());
        self.bottom_pane.reasoning_effort = normalized.clone();

        let label = normalized.as_deref().unwrap_or("none (default)");
        self.transcript.push(TranscriptItem::notice(
            format!("Reasoning effort override set to: `{label}` (Usage: `/effort [low|medium|high|none]`)"),
            NoticeLevel::Success,
        ));
        self.sync_virtualizer();
        if self.auto_scroll {
            self.scroll_to_bottom();
        }
        Ok(())
    }

    /// Transactionally applies a setup configuration candidate to disk and the live runtime.
    pub fn apply_setup_configuration(
        &mut self,
        candidate: crate::modals::SetupConfiguration,
    ) -> anyhow::Result<()> {
        // 1. Validate provider configuration
        let provider_config = m31a_provider::ProviderConfig {
            provider_type: candidate.provider_type,
            api_key: candidate.api_key.clone(),
            api_base: if candidate.api_base.trim().is_empty() {
                None
            } else {
                Some(candidate.api_base.clone())
            },
            model: candidate.model.clone(),
            extra: std::collections::HashMap::new(),
        };

        m31a_provider::validate_provider_config(&provider_config)
            .map_err(|e| anyhow::anyhow!("Invalid provider configuration: {e}"))?;

        // 2. Build candidate client and apply current reasoning effort
        let mut candidate_client = m31a_provider::ProviderBuilder::build_client(provider_config)
            .map_err(|e| anyhow::anyhow!("Failed to build provider client: {e}"))?;
        candidate_client =
            candidate_client.with_reasoning_effort(self.config.reasoning_effort.clone());

        // 3. Prepare replacement runtime components without mutating live application
        let new_engine_runtime = Arc::new(
            m31a_agent_engine::agents::EngineAgentRuntime::new(
                candidate_client.clone(),
                self.tools.clone(),
                self.storage.clone(),
                self.exec_policy.clone(),
                self.cwd.clone(),
                Some(self.hooks.clone()),
                self.config.model_context_window,
                self.config.agents.max_depth as u32,
            )
            .with_trust_store(self.trust_store.clone()),
        );
        let new_guardian = Arc::new(GuardianReviewer::new(&candidate.model));

        // 4. Persistence phase: serialize candidate configuration clone atomically
        let mut candidate_config = self.config.clone();
        candidate_config.model_provider = Some(candidate.provider_type.to_string());
        candidate_config.api_base = candidate.api_base.clone();
        if let Some(ref key) = candidate.api_key {
            candidate_config.api_key = Some(key.clone());
        }
        candidate_config.model = candidate.model.clone();
        candidate_config.sandbox = candidate.sandbox;
        candidate_config.approval = candidate.approval;

        candidate_config
            .save()
            .map_err(|e| anyhow::anyhow!("Failed to persist setup configuration: {e}"))?;

        // 5. Commit phase: single atomic transition to in-memory state
        self.config = candidate_config;
        self.active_model = candidate.model.clone();
        self.bottom_pane.active_model = candidate.model.clone();
        self.bottom_pane.sandbox_policy = format!("{:?}", candidate.sandbox);
        self.nim_client = candidate_client;
        self.tools.set_agent_runtime(new_engine_runtime.clone());
        self.engine_runtime = new_engine_runtime.clone();
        self.agent_runtime = new_engine_runtime;
        self.guardian = new_guardian;
        if let Some(ref mut ms) = self.models_screen_state {
            ms.set_active_model(candidate.model.clone());
        }
        self.refresh_system_prompt();

        self.transcript.push(TranscriptItem::notice(
            format!(
                "Setup configuration saved. Active model set to `{}`.",
                self.active_model
            ),
            NoticeLevel::Success,
        ));
        self.sync_virtualizer();
        if self.auto_scroll {
            self.scroll_to_bottom();
        }
        Ok(())
    }

    /// Persists configuration adjustments made inside the Setup Wizard transactionally.
    pub fn persist_setup_wizard_config(
        &mut self,
        wizard: &crate::modals::SetupWizardState,
    ) -> anyhow::Result<()> {
        let candidate = wizard.build_candidate_configuration(&self.config);
        self.apply_setup_configuration(candidate)
    }

    /// Launches an asynchronous verification of provider connectivity and model availability.
    pub fn verify_setup_connectivity_async(&self, wizard: &mut crate::modals::SetupWizardState) {
        match wizard.connectivity_status {
            crate::modals::ConnectivityStatus::Validating
            | crate::modals::ConnectivityStatus::TestingConnection
            | crate::modals::ConnectivityStatus::CheckingModel => {
                return;
            }
            _ => {}
        }

        wizard.connectivity_generation = wizard.connectivity_generation.wrapping_add(1);
        let generation = wizard.connectivity_generation;
        wizard.connectivity_status = crate::modals::ConnectivityStatus::Validating;
        wizard.connectivity_details = None;
        wizard.connectivity_error = None;
        wizard.skipped_verification = false;

        let candidate = wizard.build_candidate_configuration(&self.config);
        let event_tx = self.event_tx.clone();

        let task = async move {
            let provider_config = m31a_provider::ProviderConfig {
                provider_type: candidate.provider_type,
                api_key: candidate.api_key.clone(),
                api_base: if candidate.api_base.trim().is_empty() {
                    None
                } else {
                    Some(candidate.api_base.clone())
                },
                model: candidate.model.clone(),
                extra: std::collections::HashMap::new(),
            };

            if let Err(e) = m31a_provider::validate_provider_config(&provider_config) {
                let err_msg = format!("Invalid configuration: {e}");
                let status = classify_connectivity_error(&err_msg);
                let _ = event_tx
                    .send(TuiEvent::SetupConnectivityResult {
                        generation,
                        status,
                        details: None,
                        error: Some(err_msg),
                    })
                    .await;
                return;
            }

            let client = match m31a_provider::ProviderBuilder::build_client(provider_config) {
                Ok(c) => c,
                Err(e) => {
                    let err_msg = format!("Client construction failed: {e}");
                    let status = classify_connectivity_error(&err_msg);
                    let _ = event_tx
                        .send(TuiEvent::SetupConnectivityResult {
                            generation,
                            status,
                            details: None,
                            error: Some(err_msg),
                        })
                        .await;
                    return;
                }
            };

            let _ = event_tx
                .send(TuiEvent::SetupConnectivityResult {
                    generation,
                    status: crate::modals::ConnectivityStatus::TestingConnection,
                    details: None,
                    error: None,
                })
                .await;

            let start_time = std::time::Instant::now();
            let conn_res =
                tokio::time::timeout(std::time::Duration::from_secs(8), client.check_connection())
                    .await;

            match conn_res {
                Err(_) => {
                    let _ = event_tx
                        .send(TuiEvent::SetupConnectivityResult {
                            generation,
                            status: crate::modals::ConnectivityStatus::TimedOut,
                            details: None,
                            error: Some("Connection check timed out after 8s".to_string()),
                        })
                        .await;
                    return;
                }
                Ok(Err(e)) => {
                    let err_msg = format!("Endpoint connection failed: {e}");
                    let status = classify_connectivity_error(&err_msg);
                    let _ = event_tx
                        .send(TuiEvent::SetupConnectivityResult {
                            generation,
                            status,
                            details: None,
                            error: Some(err_msg),
                        })
                        .await;
                    return;
                }
                Ok(Ok(())) => {}
            }

            let latency_ms = start_time.elapsed().as_millis() as u64;

            let _ = event_tx
                .send(TuiEvent::SetupConnectivityResult {
                    generation,
                    status: crate::modals::ConnectivityStatus::CheckingModel,
                    details: None,
                    error: None,
                })
                .await;

            let model_res = tokio::time::timeout(
                std::time::Duration::from_secs(8),
                client.test_model_availability(),
            )
            .await;

            match model_res {
                Err(_) => {
                    let _ = event_tx
                        .send(TuiEvent::SetupConnectivityResult {
                            generation,
                            status: crate::modals::ConnectivityStatus::TimedOut,
                            details: Some(crate::modals::ConnectivityDetails {
                                endpoint_verified: true,
                                model_verified: false,
                                model_check_unsupported: false,
                                latency_ms: Some(latency_ms),
                                message: "Endpoint connected, but model availability check timed out (8s)".to_string(),
                            }),
                            error: Some("Model availability check timed out after 8s".to_string()),
                        })
                        .await;
                }
                Ok(Ok(())) => {
                    let _ = event_tx
                        .send(TuiEvent::SetupConnectivityResult {
                            generation,
                            status: crate::modals::ConnectivityStatus::Connected,
                            details: Some(crate::modals::ConnectivityDetails {
                                endpoint_verified: true,
                                model_verified: true,
                                model_check_unsupported: false,
                                latency_ms: Some(latency_ms),
                                message: format!(
                                    "Connection and model `{}` verified successfully",
                                    candidate.model
                                ),
                            }),
                            error: None,
                        })
                        .await;
                }
                Ok(Err(e)) => {
                    let err_str = e.to_string();
                    let lower = err_str.to_lowercase();
                    if lower.contains("not supported") || lower.contains("unsupported") {
                        let _ = event_tx
                            .send(TuiEvent::SetupConnectivityResult {
                                generation,
                                status: crate::modals::ConnectivityStatus::Connected,
                                details: Some(crate::modals::ConnectivityDetails {
                                    endpoint_verified: true,
                                    model_verified: false,
                                    model_check_unsupported: true,
                                    latency_ms: Some(latency_ms),
                                    message: format!(
                                        "Endpoint verified ({latency_ms}ms). Model availability not independently verified."
                                    ),
                                }),
                                error: None,
                            })
                            .await;
                    } else {
                        let status = classify_connectivity_error(&err_str);
                        let _ = event_tx
                            .send(TuiEvent::SetupConnectivityResult {
                                generation,
                                status,
                                details: Some(crate::modals::ConnectivityDetails {
                                    endpoint_verified: true,
                                    model_verified: false,
                                    model_check_unsupported: false,
                                    latency_ms: Some(latency_ms),
                                    message: format!("Model availability check failed: {err_str}"),
                                }),
                                error: Some(format!("Model check failed: {err_str}")),
                            })
                            .await;
                    }
                }
            }
        };

        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            handle.spawn(task);
        } else {
            std::thread::spawn(move || {
                if let Ok(rt) = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                {
                    rt.block_on(task);
                }
            });
        }
    }

    /// Asynchronously probes the health and availability of a specific model slug.
    pub fn probe_model_health_async(&self, slug: &str) {
        let client = self.nim_client.with_model(slug);
        let event_tx = self.event_tx.clone();
        let model = slug.to_string();

        let task = async move {
            let start = std::time::Instant::now();
            let result = tokio::time::timeout(
                std::time::Duration::from_secs(8),
                client.test_model_availability(),
            )
            .await;

            let latency_ms = start.elapsed().as_millis() as u64;

            let state = match result {
                Ok(Ok(())) => crate::model_selector::ModelHealthState::Healthy { latency_ms },
                Ok(Err(e)) => {
                    let err_msg = e.to_string();
                    if err_msg.contains("not supported") || err_msg.contains("unsupported") {
                        crate::model_selector::ModelHealthState::Unsupported
                    } else {
                        crate::model_selector::ModelHealthState::Unhealthy { message: err_msg }
                    }
                }
                Err(_) => crate::model_selector::ModelHealthState::Unhealthy {
                    message: "Probe timed out (8s)".to_string(),
                },
            };

            let _ = event_tx
                .send(TuiEvent::ModelHealthResult { model, state })
                .await;
        };

        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            handle.spawn(task);
        } else {
            std::thread::spawn(move || {
                if let Ok(rt) = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                {
                    rt.block_on(task);
                }
            });
        }
    }

    /// Applies an approval action exactly as labeled by the one approval surface.
    ///
    /// Returns `true` when the approval was resolved and the modal can close. Returns
    /// `false` when the action opens a follow-up view (diff preview, feedback input) or
    /// when the action is not available for this request. An unavailable action never
    /// falls back to allowing the tool.
    pub fn resolve_approval_action(
        &mut self,
        approval: PendingApproval,
        action: ApprovalActionKind,
    ) -> bool {
        match action {
            ApprovalActionKind::AllowOnce => {
                self.send_approval_decision(m31a_elicitation::ApprovalDecision::AllowOnce);
                true
            }
            ApprovalActionKind::AllowRule => {
                let Some(rule) = approval.suggested_rule.clone() else {
                    return false;
                };
                self.install_prefix_rule(&rule, "Approval dialog: allow rule");
                self.send_approval_decision(
                    m31a_elicitation::ApprovalDecision::AlwaysAllowPattern(rule),
                );
                true
            }
            ApprovalActionKind::AllowFile => {
                let Some(path) = approval.allow_file.clone() else {
                    return false;
                };
                self.send_approval_decision(m31a_elicitation::ApprovalDecision::AllowFile { path });
                true
            }
            ApprovalActionKind::ViewDiff => {
                let Some(diff) = approval.diff_text() else {
                    return false;
                };
                self.push_modal(ModalState::QuickDiffPreview {
                    diff_text: diff.lines().map(|line| line.to_string()).collect(),
                    return_to_approval: approval,
                    scroll: 0,
                });
                false
            }
            ApprovalActionKind::DeclineWithFeedback => {
                self.push_modal(ModalState::DeclineFeedbackInput {
                    approval,
                    feedback_buffer: String::new(),
                });
                false
            }
            ApprovalActionKind::Cancel => {
                self.send_approval_decision(m31a_elicitation::ApprovalDecision::Deny {
                    reason: "Operation denied by user.".to_string(),
                });
                true
            }
        }
    }

    pub(crate) fn send_approval_decision(&mut self, decision: m31a_elicitation::ApprovalDecision) {
        if let Some(tx) = self.approval_tx.take() {
            let _ = tx.send(decision);
        }
    }

    /// Installs a durable prefix allow-rule in the in-memory policy used by this session.
    fn install_prefix_rule(&mut self, rule: &str, justification: &str) {
        if let Ok(prefix_rule) = m31a_execpolicy::PrefixRule::from_pattern(
            rule,
            m31a_execpolicy::Decision::Allow,
            Some(format!("{justification}: {rule}")),
        ) {
            self.exec_policy.add_prefix_rule(prefix_rule);
        }
    }

    pub fn update_status_badges(&mut self) {
        self.bottom_pane.todo_summary = m31a_tools::get_todo_badge_for_session(&self.session_id);
        if let Ok(Some(g)) = self.storage.get_goal(&self.session_id) {
            let label = if g.status == "active" {
                format!("\"{}\"", g.objective)
            } else {
                format!("{} (\"{}\")", g.status, g.objective)
            };
            self.bottom_pane.goal_summary = Some(label);
        } else {
            self.bottom_pane.goal_summary = None;
        }

        // Sync real-time stream metrics (velocity, TTFT)
        self.bottom_pane.stream_metrics =
            self.chat_widget.streaming_buffer.last_or_active_metrics();

        // Sync active subagent swarm tabs from storage with real live runtime status
        if let Ok(agents) = self.storage.list_subagents(&self.session_id) {
            let tabs: Vec<crate::bottom_pane::SubagentTab> = agents
                .into_iter()
                .map(|(child_id, role)| {
                    let status_str = self.engine_runtime.get_status_sync(&child_id);
                    let status = match status_str.as_deref() {
                        Some("thinking") | Some("streaming") => {
                            crate::bottom_pane::SubagentStatus::Thinking
                        }
                        Some("tool") | Some("executing_tool") => {
                            crate::bottom_pane::SubagentStatus::RunningTool
                        }
                        Some("waiting") | Some("approval") => {
                            crate::bottom_pane::SubagentStatus::AwaitingApproval
                        }
                        Some("completed") => crate::bottom_pane::SubagentStatus::Completed,
                        _ => crate::bottom_pane::SubagentStatus::Idle,
                    };
                    crate::bottom_pane::SubagentTab {
                        thread_id: child_id,
                        role,
                        status,
                    }
                })
                .collect();
            self.bottom_pane.subagent_tabs = tabs;
        }
    }

    /// Cycles through active subagents using Alt+Left / Alt+Right.
    pub fn switch_active_subagent(&mut self, next: bool) {
        let total = self.bottom_pane.subagent_tabs.len() + 1; // 0 is main agent
        if total <= 1 {
            return;
        }
        let current = self.bottom_pane.active_subagent_tab;
        let new_idx = if next {
            (current + 1) % total
        } else {
            (current + total - 1) % total
        };

        if new_idx == 0 {
            self.switch_subagent_view(None);
        } else if let Some(tab) = self.bottom_pane.subagent_tabs.get(new_idx - 1).cloned() {
            self.switch_subagent_view(Some(tab.thread_id));
        }
    }

    /// Switches view between the main conversation transcript and a subagent child thread.
    pub fn switch_subagent_view(&mut self, child_id: Option<String>) {
        if let Some(target_id) = child_id {
            self.active_thread_id = Some(target_id.clone());
            if let Some(pos) = self
                .bottom_pane
                .subagent_tabs
                .iter()
                .position(|t| t.thread_id == target_id)
            {
                self.bottom_pane.active_subagent_tab = pos + 1;
            }
            self.transcript.clear();
            self.transcript.push(TranscriptItem::notice(
                format!("Switched view context to subagent [{}]", target_id),
                NoticeLevel::Info,
            ));
            if let Ok(items) = self.storage.list_items(&target_id) {
                for (item_type, content, _id) in items {
                    match item_type.as_str() {
                        "user" => self.transcript.push(TranscriptItem::user(content)),
                        "assistant" => self.transcript.push(TranscriptItem::assistant(content)),
                        _ => {}
                    }
                }
            }
        } else {
            self.active_thread_id = None;
            self.bottom_pane.active_subagent_tab = 0;
            self.transcript.clear();
            self.transcript.push(TranscriptItem::notice(
                format!(
                    "Switched view context to [Main Session] ({})",
                    self.session_id
                ),
                NoticeLevel::Info,
            ));
            if let Ok(items) = self.storage.list_items(&self.session_id) {
                for (item_type, content, _id) in items {
                    match item_type.as_str() {
                        "user" => self.transcript.push(TranscriptItem::user(content)),
                        "assistant" => self.transcript.push(TranscriptItem::assistant(content)),
                        _ => {}
                    }
                }
            } else {
                for msg in &self.chat_history {
                    match &msg.role {
                        m31a_core::types::Role::User => {
                            if let Some(ref text) = msg.content {
                                self.transcript.push(TranscriptItem::user(text.clone()));
                            }
                        }
                        m31a_core::types::Role::Assistant => {
                            if let Some(ref text) = msg.content {
                                self.transcript
                                    .push(TranscriptItem::assistant(text.clone()));
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
        self.sync_virtualizer();
        self.scroll_to_bottom();
    }

    /// Terminates a running child subagent and cleans up resources.
    pub fn terminate_subagent(&mut self, child_id: &str, role: &str) -> Result<()> {
        let _ = self.storage.terminate_agent_tree(child_id);
        let _ = self.storage.delete_thread(child_id);

        let agent_rt = self.agent_runtime.clone();
        let cid = child_id.to_string();
        let proc_sup = self.proc_supervisor.clone();

        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            handle.spawn(async move {
                let _ = agent_rt.terminate_tree(&cid).await;
                let procs = proc_sup.list().await;
                for p in procs {
                    if p.id.contains(&cid) || p.label.contains(&cid) {
                        let _ = proc_sup.stop(&p.id, true).await;
                    }
                }
            });
        }

        self.bottom_pane
            .subagent_tabs
            .retain(|t| t.thread_id != child_id);
        if self.active_thread_id.as_deref() == Some(child_id) {
            self.switch_subagent_view(None);
        }

        self.transcript.push(TranscriptItem::notice(
            format!(
                "Terminated subagent [{}] ({}) and cleaned up resources.",
                role, child_id
            ),
            NoticeLevel::Warning,
        ));
        self.sync_virtualizer();
        Ok(())
    }

    /// Switches working directory and re-evaluates trust, hooks, git status, and watcher.
    pub fn switch_workspace(&mut self, new_cwd: PathBuf) -> Result<()> {
        if !new_cwd.exists() || !new_cwd.is_dir() {
            anyhow::bail!(
                "Target workspace path does not exist or is not a directory: {}",
                new_cwd.display()
            );
        }

        let new_git_status = Self::detect_git_status(&new_cwd);
        let new_hooks = Arc::new(m31a_hooks::HookRuntime::new(
            m31a_hooks::HookConfig::discover(&new_cwd),
        ));
        let new_tools = ToolRegistry::new_isolated(
            Some(self.storage.clone()),
            self.vault.clone(),
            self.mcp_manager.clone(),
            self.config.extra_dirs.clone(),
            &new_cwd,
        );
        let new_engine_runtime = Arc::new(
            m31a_agent_engine::agents::EngineAgentRuntime::new(
                self.nim_client.clone(),
                new_tools.clone(),
                self.storage.clone(),
                self.exec_policy.clone(),
                new_cwd.clone(),
                Some(new_hooks.clone()),
                self.config.model_context_window,
                self.config.agents.max_depth as u32,
            )
            .with_trust_store(self.trust_store.clone()),
        );
        let new_tools = new_tools;
        new_tools.set_agent_runtime(new_engine_runtime.clone());

        // Atomically commit updated workspace state
        self.cwd = new_cwd.clone();
        self.bottom_pane.git_status = new_git_status;
        self.hooks = new_hooks;
        self.tools = new_tools;
        self.engine_runtime = new_engine_runtime.clone();
        self.agent_runtime = new_engine_runtime;
        self.proc_supervisor = self.tools.proc_supervisor.clone();

        // Evaluate workspace trust
        let decision = self.trust_store.lock().unwrap().evaluate(&self.cwd);
        match decision {
            m31a_trust::TrustDecision::Trusted => {
                self.transcript.push(TranscriptItem::notice(
                    format!(
                        "Workspace verified: '{}' is a trusted directory.",
                        self.cwd.display()
                    ),
                    NoticeLevel::Success,
                ));
            }
            m31a_trust::TrustDecision::SessionOnly => {
                self.transcript.push(TranscriptItem::notice(
                    format!(
                        "Workspace trusted for this session only: '{}'.",
                        self.cwd.display()
                    ),
                    NoticeLevel::Info,
                ));
            }
            m31a_trust::TrustDecision::RequiresConfirmation {
                ref path,
                ref reason,
            } => {
                self.transcript.push(TranscriptItem::notice(
                    format!("Untrusted workspace: '{}' ({reason}).", path.display()),
                    NoticeLevel::Warning,
                ));
            }
            m31a_trust::TrustDecision::Denied { ref path } => {
                self.transcript.push(TranscriptItem::notice(
                    format!(
                        "Access denied: '{}' is marked Denied by workspace trust security policy.",
                        path.display()
                    ),
                    NoticeLevel::Error,
                ));
            }
        }

        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            let watcher = Arc::new(m31a_watcher::WorkspaceWatcher::new(self.cwd.clone(), None));
            let mut watcher_rx = watcher.subscribe();
            watcher.start();
            let event_tx = self.event_tx.clone();
            handle.spawn(async move {
                while let Ok(batch) = watcher_rx.recv().await {
                    let _ = event_tx.send(TuiEvent::WorkspaceChanged(batch)).await;
                }
            });
            self.watcher = Some(watcher);
        }

        self.refresh_system_prompt();
        self.sync_virtualizer();
        Ok(())
    }

    /// Activates an engineering persona into active prompt dynamic context and system prompt.
    pub fn activate_persona(&mut self, name: &str, desc: &str, guidelines: &str) {
        self.active_persona = Some(name.to_string());
        self.pinned_context
            .retain(|item| !item.path.starts_with("persona:"));
        let content = format!(
            "Active Engineering Persona: {}\nDescription: {}\nGuidelines:\n{}",
            name, desc, guidelines
        );
        let tokens = content.split_whitespace().count();
        self.pinned_context.push(m31a_core::DynamicContextItem {
            path: format!("persona:{}", name),
            content,
            tokens,
        });
        self.refresh_system_prompt();
        self.transcript.push(TranscriptItem::notice(
            format!("Activated engineering persona '{}': {}", name, desc),
            NoticeLevel::Success,
        ));
        self.sync_virtualizer();
    }

    /// Activates a skill into active prompt dynamic context and system prompt.
    pub fn activate_skill(&mut self, name: &str, instructions: &str) {
        self.pinned_context
            .retain(|item| item.path != format!("skill:{}", name));
        let content = format!("Active Skill: {}\nInstructions:\n{}", name, instructions);
        let tokens = content.split_whitespace().count();
        self.pinned_context.push(m31a_core::DynamicContextItem {
            path: format!("skill:{}", name),
            content,
            tokens,
        });
        self.refresh_system_prompt();
        self.transcript.push(TranscriptItem::notice(
            format!("Activated skill '{}' into prompt context.", name),
            NoticeLevel::Success,
        ));
        self.sync_virtualizer();
    }

    /// Asynchronously connects or disconnects an MCP server in the background and reports via event notice.
    pub fn toggle_mcp_server_sync(&mut self, name: &str) {
        let srv_name = name.to_string();
        let mcp = self.mcp_manager.clone();
        let tools_reg = self.tools.clone();
        let cwd = self.cwd.clone();
        let srv_config = self.config.mcp_servers.get(name).cloned();
        let event_tx = self.event_tx.clone();

        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            handle.spawn(async move {
                let connected = mcp.is_server_connected_sync(&srv_name)
                    || mcp.list_server_names().await.contains(&srv_name);
                if connected {
                    tools_reg.unregister_server_tools(&srv_name);
                    mcp.disconnect_server(&srv_name).await;
                    let _ = event_tx
                        .send(TuiEvent::Notice(
                            format!(
                                "MCP server '{}' disconnected and tools deregistered.",
                                srv_name
                            ),
                            NoticeLevel::Info,
                        ))
                        .await;
                } else if let Some(srv) = srv_config {
                    match mcp.connect_server_config(&srv_name, &srv, Some(&cwd)).await {
                        Ok(defs) => {
                            let count = defs.len();
                            tools_reg.register_dynamic_tools(defs);
                            let _ = event_tx
                                .send(TuiEvent::Notice(
                                    format!(
                                        "MCP server '{}' connected (loaded and registered {} dynamic tools).",
                                        srv_name, count
                                    ),
                                    NoticeLevel::Success,
                                ))
                                .await;
                        }
                        Err(e) => {
                            let _ = event_tx
                                .send(TuiEvent::Notice(
                                    format!("Failed to connect MCP server '{}': {}", srv_name, e),
                                    NoticeLevel::Error,
                                ))
                                .await;
                        }
                    }
                }
            });
        }
    }

    /// Cancels active turn execution cleanly and resets state machine.
    pub fn cancel_turn(&mut self) {
        if let Some(token) = self.cancel_token.take() {
            token.cancel();
        }
        let turn_id = self
            .current_turn_id
            .clone()
            .unwrap_or_else(|| "active".to_string());
        let _ = self.turn_state.transition_cancellation_requested(&turn_id);
        let _ = self.turn_state.transition_cancelling(&turn_id);

        // Do not immediately force Idle or synthesize synthetic completion.
        // The background engine observes cancel_token and terminates cleanly.
        // Only if no async runtime is active (synchronous unit tests) do we force idle immediately.
        if tokio::runtime::Handle::try_current().is_err() {
            self.turn_state.force_idle();
            self.current_turn_id = None;
            self.turn_start_time = None;
            self.turn_ttft_ms = None;
            self.chat_widget.turn_state = crate::chatwidget::TurnExecutionState::Idle;
        }

        self.steer_tx = None;
        self.transcript.push(TranscriptItem::notice(
            "Turn cancellation requested (Ctrl+C)... waiting for execution to halt.",
            NoticeLevel::Warning,
        ));
        self.sync_virtualizer();
    }

    /// Opens the Esc-backtracking turn scrubber modal.
    pub fn open_turn_scrubber(&mut self) -> bool {
        let mut turns = Vec::new();
        let mut turn_idx = 0;
        let checkpoints = self
            .storage
            .list_checkpoints(&self.session_id)
            .unwrap_or_default();

        let mut iter = self.chat_history.iter().peekable();
        while let Some(msg) = iter.next() {
            if msg.role == m31a_core::Role::User {
                turn_idx += 1;
                let user_prompt = msg.content.clone().unwrap_or_default();
                let mut assistant_preview = None;

                if let Some(next_msg) = iter.peek()
                    && next_msg.role == m31a_core::Role::Assistant
                {
                    assistant_preview = next_msg.content.clone();
                }

                let chk_id = checkpoints
                    .iter()
                    .find(|c| c.turn_index == turn_idx)
                    .map(|c| c.id.clone());

                turns.push(crate::modals::ScrubTurnItem {
                    turn_index: turn_idx,
                    user_prompt,
                    assistant_preview,
                    checkpoint_id: chk_id,
                });
            }
        }

        if turns.is_empty() {
            return false;
        }

        // Reverse turns so latest turn appears on top
        turns.reverse();
        self.push_modal(ModalState::TurnScrubber {
            turns,
            selected_index: 0,
        });
        true
    }

    /// Backtracks the conversation to the selected turn: restores checkpoint and populates composer.
    pub fn apply_backtrack_turn(
        &mut self,
        turn_index: usize,
        prompt: String,
        checkpoint_id: Option<String>,
    ) {
        if let Some(chk_id) = checkpoint_id {
            let _ = self.storage.restore_checkpoint(&chk_id, true);
        }

        // Truncate chat_history back to selected turn
        let mut user_turn_count = 0;
        let mut cutoff_idx = self.chat_history.len();
        for (idx, msg) in self.chat_history.iter().enumerate() {
            if msg.role == m31a_core::Role::User {
                user_turn_count += 1;
                if user_turn_count == turn_index {
                    cutoff_idx = idx;
                    break;
                }
            }
        }
        self.chat_history.truncate(cutoff_idx);

        // Populate composer buffer with the turn's prompt for immediate editing
        self.composer.set_text(&prompt);
        self.bottom_pane.mode = crate::bottom_pane::BottomPaneMode::InputNormal;

        self.transcript.push(TranscriptItem::notice(
            format!(
                "Backtracked to Turn #{}. Prompt restored for editing below.",
                turn_index
            ),
            NoticeLevel::Success,
        ));
        self.sync_virtualizer();
    }

    /// Opens the interactive V8 Code Mode scratchpad modal.
    pub fn open_code_mode_scratchpad(&mut self) {
        let specs: Vec<m31a_code_mode::AmbientToolSpec> = self
            .tools
            .definitions()
            .into_iter()
            .map(|t| {
                m31a_code_mode::AmbientToolSpec::new(
                    t.function.name,
                    t.function.description,
                    t.function.parameters,
                )
            })
            .collect();
        let ambient_dts =
            m31a_code_mode::AmbientDtsGenerator::default().generate_ambient_dts(&specs);
        self.push_modal(ModalState::code_mode_scratchpad(ambient_dts, None));
    }

    /// Evaluates the script in the active Code Mode scratchpad asynchronously.
    pub fn eval_code_mode_scratchpad(&mut self) {
        if let Some(ModalState::CodeModeScratchpad(sp)) = self.current_modal_mut() {
            sp.is_evaluating = true;
            let code = sp.code.clone();
            let tools = self.tools.clone();
            let cwd = self.cwd.clone();
            let policy = self.config.sandbox;
            let event_tx = self.event_tx.clone();

            tokio::spawn(async move {
                let runtime = m31a_code_mode::CodeModeRuntime::new();
                let start = std::time::Instant::now();
                let outcome = match runtime.run(&code, &tools, &cwd, policy, 10000).await {
                    Ok(res) => {
                        let dur = start.elapsed();
                        format!("// [Evaluation completed in {:.2?}]\n{}", dur, res.output)
                    }
                    Err(e) => {
                        format!("// [Evaluation Error]:\n{}", e)
                    }
                };
                let _ = event_tx.send(TuiEvent::CodeModeResult(outcome)).await;
            });
        }
    }

    /// Opens the append-only rollout journal and event timeline modal.
    pub fn open_rollout_journal(&mut self) {
        let mut events = Vec::new();
        let mut id = 1;

        // 1. Collect past chat history turns
        for msg in &self.chat_history {
            let (cat, title) = match msg.role {
                m31a_core::types::Role::User => ("PROMPT", "User Turn Input"),
                m31a_core::types::Role::Assistant => ("ASSISTANT", "Assistant Completion"),
                m31a_core::types::Role::Tool => ("TOOL_RES", "Tool Result Observation"),
                m31a_core::types::Role::System => ("NOTICE", "System Preamble"),
            };
            let content_str = msg.content.clone().unwrap_or_default();
            let raw_json =
                serde_json::to_string_pretty(msg).unwrap_or_else(|_| content_str.clone());
            events.push(crate::modals::RolloutEventEntry {
                id,
                timestamp: chrono::Local::now().format("%H:%M:%S").to_string(),
                category: cat.to_string(),
                title: title.to_string(),
                detail: raw_json,
                tokens: None,
            });
            id += 1;
        }

        // 2. Collect transcript tool calls and thinking
        for item in &self.transcript {
            match item {
                TranscriptItem::ToolCall(tc) => {
                    events.push(crate::modals::RolloutEventEntry {
                        id,
                        timestamp: chrono::Local::now().format("%H:%M:%S").to_string(),
                        category: "TOOL".to_string(),
                        title: format!("{}(...)", tc.name),
                        detail: format!(
                            "Arguments:\n{}\n\nOutput:\n{}",
                            tc.arguments,
                            tc.output.as_deref().unwrap_or("(pending/running)")
                        ),
                        tokens: None,
                    });
                    id += 1;
                }
                TranscriptItem::Thinking { text, .. } => {
                    events.push(crate::modals::RolloutEventEntry {
                        id,
                        timestamp: chrono::Local::now().format("%H:%M:%S").to_string(),
                        category: "METRIC".to_string(),
                        title: "Reasoning Step".to_string(),
                        detail: text.clone(),
                        tokens: None,
                    });
                    id += 1;
                }
                _ => {}
            }
        }

        // 3. Collect checkpoints
        if let Ok(checkpoints) = self.storage.list_checkpoints(&self.session_id) {
            for cp in checkpoints {
                let ts_str = chrono::DateTime::from_timestamp(cp.created_at, 0)
                    .map(|dt| dt.format("%H:%M:%S").to_string())
                    .unwrap_or_else(|| cp.created_at.to_string());
                events.push(crate::modals::RolloutEventEntry {
                    id,
                    timestamp: ts_str,
                    category: "CHECKPOINT".to_string(),
                    title: format!("Checkpoint #{}: {}", cp.turn_index, cp.label),
                    detail: format!(
                        "ID: {}\nCommit: {}\nFiles: {:?}",
                        cp.id,
                        cp.git_commit.as_deref().unwrap_or("none"),
                        cp.modified_files
                    ),
                    tokens: None,
                });
                id += 1;
            }
        }

        // If no events exist yet, add synthetic initialization event
        if events.is_empty() {
            events.push(crate::modals::RolloutEventEntry {
                id: 1,
                timestamp: chrono::Local::now().format("%H:%M:%S").to_string(),
                category: "NOTICE".to_string(),
                title: "Session Initialized".to_string(),
                detail: format!(
                    "Session ID: {}\nModel: {}\nCWD: {}",
                    self.session_id,
                    self.active_model,
                    self.cwd.display()
                ),
                tokens: Some(0),
            });
        }

        self.push_modal(ModalState::rollout_journal(events));
    }

    pub fn tick(&mut self) {
        self.chat_widget.tick_spinner();
        self.bottom_pane.queued_prompts_count = self.queued_prompts.len();
    }

    pub fn handle_submit(&mut self) {
        let input = self.composer.submit();
        if input.is_empty() {
            return;
        }

        if input.starts_with('/') {
            let cmd_name = input.split_whitespace().next().unwrap_or("");
            if let Some(cmd) = m31a_composer::slash::find_command(cmd_name) {
                let outcome = self
                    .trust_store
                    .lock()
                    .unwrap()
                    .authorize_operation(&self.cwd, cmd.capability);
                match outcome {
                    m31a_trust::AuthorizationOutcome::Denied { reason, .. } => {
                        self.transcript.push(TranscriptItem::notice(
                            format!(
                                "Slash command '{}' BLOCKED: Working directory '{}' is Denied ({}).",
                                cmd.name, self.cwd.display(), reason
                            ),
                            NoticeLevel::Error,
                        ));
                        self.sync_virtualizer();
                        return;
                    }
                    m31a_trust::AuthorizationOutcome::RequiresConfirmation { reason, .. } => {
                        self.transcript.push(TranscriptItem::notice(
                            format!(
                                "Slash command '{}' BLOCKED: Working directory '{}' is untrusted ({}).\nType `/trust session` to authorize for this session, or `/trust add` to trust permanently.",
                                cmd.name, self.cwd.display(), reason
                            ),
                            NoticeLevel::Error,
                        ));
                        self.sync_virtualizer();
                        return;
                    }
                    m31a_trust::AuthorizationOutcome::Allowed => {}
                }
            }
            self.execute_slash_command(&input);
            return;
        }

        if self.app_status() != AppStatus::Idle
            && let Some(tx) = &self.steer_tx
        {
            let _ = tx.send(input.clone());
            self.transcript.push(TranscriptItem::notice(
                format!("Dynamic steering injected: \"{}\"", input),
                NoticeLevel::Info,
            ));
            self.sync_virtualizer();
            return;
        }

        self.submit_prompt(input);
    }

    pub fn submit_prompt(&mut self, prompt: String) {
        // Enforce centralized trust boundary
        let decision = self.trust_store.lock().unwrap().evaluate(&self.cwd);
        match decision {
            m31a_trust::TrustDecision::Denied { path } => {
                self.transcript.push(TranscriptItem::notice(
                    format!("Execution BLOCKED: Working directory '{}' is marked as Denied by security trust policy.", path.display()),
                    NoticeLevel::Error,
                ));
                self.sync_virtualizer();
                return;
            }
            m31a_trust::TrustDecision::RequiresConfirmation { path, reason } => {
                self.transcript.push(TranscriptItem::notice(
                    format!(
                        "Execution BLOCKED: Working directory '{}' is untrusted ({}).\nType `/trust session` to authorize for this session, `/trust add` to trust permanently, or `/trust audit` to inspect.",
                        path.display(),
                        reason
                    ),
                    NoticeLevel::Error,
                ));
                self.sync_virtualizer();
                return;
            }
            m31a_trust::TrustDecision::Trusted | m31a_trust::TrustDecision::SessionOnly => {}
        }
        let trimmed = prompt.trim();
        if let Some(cmd) = trimmed.strip_prefix('!') {
            let cmd = cmd.trim();
            if !cmd.is_empty() {
                self.execute_instant_shell_command(cmd);
                return;
            }
        }

        if self.app_status() != AppStatus::Idle || self.turn_state.is_active() {
            if let Some(tx) = &self.steer_tx {
                let _ = tx.send(prompt.clone());
                self.transcript.push(TranscriptItem::notice(
                    format!("Dynamic steering injected: \"{}\"", prompt),
                    NoticeLevel::Info,
                ));
                self.sync_virtualizer();
                return;
            }
            self.transcript.push(TranscriptItem::notice(
                "A generation turn is currently active. Guidance sent or press Ctrl+C to cancel.",
                NoticeLevel::Warning,
            ));
            self.sync_virtualizer();
            return;
        }

        self.transcript.push(TranscriptItem::user(prompt.clone()));
        self.sync_virtualizer();
        self.scroll_to_bottom();

        self.chat_history.push(ChatMessage::user(prompt));
        self.spawn_turn();
    }

    /// Executes an instant local shell command directly without model roundtrip using '!' prefix syntax.
    pub fn execute_instant_shell_command(&mut self, cmd: &str) {
        // Enforce centralized trust boundary on instant shell commands
        let decision = self.trust_store.lock().unwrap().evaluate(&self.cwd);
        match decision {
            m31a_trust::TrustDecision::Denied { path } => {
                self.transcript.push(TranscriptItem::notice(
                    format!("Execution BLOCKED: Working directory '{}' is marked as Denied by security trust policy.", path.display()),
                    NoticeLevel::Error,
                ));
                self.sync_virtualizer();
                return;
            }
            m31a_trust::TrustDecision::RequiresConfirmation { path, reason } => {
                self.transcript.push(TranscriptItem::notice(
                    format!(
                        "Execution BLOCKED: Working directory '{}' is untrusted ({}).\nType `/trust session` to authorize for this session, `/trust add` to trust permanently, or `/trust audit` to inspect.",
                        path.display(),
                        reason
                    ),
                    NoticeLevel::Error,
                ));
                self.sync_virtualizer();
                return;
            }
            m31a_trust::TrustDecision::Trusted | m31a_trust::TrustDecision::SessionOnly => {}
        }

        self.transcript
            .push(TranscriptItem::user(format!("!{}", cmd)));
        let tool_id = format!("shell-{}", uuid::Uuid::new_v4());

        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            let tool_entry = crate::transcript::ToolCallEntry {
                id: tool_id.clone(),
                name: "sh".to_string(),
                arguments: format!("\"{}\"", cmd),
                output: None,
                success: None,
                in_progress: true,
                collapsed: false,
                duration_ms: None,
            };
            self.transcript.push(TranscriptItem::ToolCall(tool_entry));
            self.sync_virtualizer();
            self.scroll_to_bottom();

            let cmd_string = cmd.to_string();
            let cwd = self.cwd.clone();
            let tx = self.event_tx.clone();
            let id = tool_id.clone();

            handle.spawn(async move {
                let start = std::time::Instant::now();
                let res = tokio::time::timeout(
                    std::time::Duration::from_secs(60),
                    tokio::process::Command::new("sh")
                        .arg("-c")
                        .arg(&cmd_string)
                        .current_dir(&cwd)
                        .output(),
                )
                .await;
                let elapsed = start.elapsed().as_millis() as u64;

                let (output, success) = match res {
                    Ok(Ok(out)) => {
                        let stdout = String::from_utf8_lossy(&out.stdout).to_string();
                        let stderr = String::from_utf8_lossy(&out.stderr).to_string();
                        let combined = if stderr.is_empty() {
                            stdout
                        } else if stdout.is_empty() {
                            stderr
                        } else {
                            format!("{}\n{}", stdout, stderr)
                        };
                        (combined, out.status.success())
                    }
                    Ok(Err(e)) => (format!("Failed to execute command: {}", e), false),
                    Err(_) => ("Command timed out after 60 seconds".to_string(), false),
                };

                let _ = tx
                    .send(TuiEvent::ShellCommandFinished {
                        id,
                        output,
                        success,
                        duration_ms: elapsed,
                    })
                    .await;
            });
        } else {
            // Synchronous fallback when running outside tokio reactor (e.g. sync unit tests)
            let start = std::time::Instant::now();
            let output = std::process::Command::new("sh")
                .arg("-c")
                .arg(cmd)
                .current_dir(&self.cwd)
                .output();
            let elapsed = start.elapsed().as_millis() as u64;

            match output {
                Ok(out) => {
                    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
                    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
                    let combined = if stderr.is_empty() {
                        stdout
                    } else if stdout.is_empty() {
                        stderr
                    } else {
                        format!("{}\n{}", stdout, stderr)
                    };
                    let success = out.status.success();
                    let tool_entry = crate::transcript::ToolCallEntry {
                        id: tool_id,
                        name: "sh".to_string(),
                        arguments: format!("\"{}\"", cmd),
                        output: Some(combined.clone()),
                        success: Some(success),
                        in_progress: false,
                        collapsed: false,
                        duration_ms: Some(elapsed),
                    };
                    self.transcript.push(TranscriptItem::ToolCall(tool_entry));
                    self.chat_history.push(ChatMessage::user(format!(
                        "Executed shell command: `{}`\nExit status: {}\nOutput:\n```\n{}\n```",
                        cmd,
                        if success { "0 (success)" } else { "failed" },
                        combined
                    )));
                }
                Err(e) => {
                    self.transcript.push(TranscriptItem::notice(
                        format!("Failed to execute shell command '{}': {}", cmd, e),
                        NoticeLevel::Error,
                    ));
                }
            }
            self.sync_virtualizer();
            self.scroll_to_bottom();
        }
    }

    /// Opens universal fuzzy command palette.
    pub fn open_command_palette(&mut self) {
        let items = ModalState::default_command_palette_items();
        self.push_modal(ModalState::command_palette(items));
    }

    /// Toggles collaboration mode between Build (full modification) and Plan (read-only analysis).
    pub fn toggle_collaboration_mode(&mut self) {
        self.collaboration_mode = match self.collaboration_mode {
            CollaborationMode::Default => CollaborationMode::Plan,
            CollaborationMode::Plan => CollaborationMode::Default,
        };
        self.bottom_pane.collaboration_mode = self.collaboration_mode;
        self.refresh_system_prompt();
        let (name, desc, level) = match self.collaboration_mode {
            CollaborationMode::Default => (
                "BUILD",
                "Autonomous implementation & execution enabled (edits files, runs commands).",
                NoticeLevel::Success,
            ),
            CollaborationMode::Plan => (
                "PLAN",
                "Advisory & architecture exploration enabled (read-only checks, no mutations).",
                NoticeLevel::Info,
            ),
        };
        self.transcript.push(TranscriptItem::notice(
            format!("Agent Mode changed to [{}]: {}", name, desc),
            level,
        ));
        self.sync_virtualizer();
    }

    pub fn execute_slash_command(&mut self, cmd: &str) {
        let tokens = m31a_composer::slash::tokenize_slash_command(cmd);
        if tokens.is_empty() {
            return;
        }
        let parts: Vec<&str> = tokens.iter().map(|s| s.as_str()).collect();
        let command = parts[0].to_lowercase();

        match command.as_str() {
            "/help" | "/?" => {
                self.push_modal(ModalState::help());
            }
            "/settings" | "/config" => {
                self.push_modal(ModalState::settings());
            }
            "/setup" => {
                if parts.len() > 1 {
                    let sub = parts[1].to_lowercase();
                    match sub.as_str() {
                        "settings" | "gui" | "edit" => {
                            self.push_modal(ModalState::settings());
                            return;
                        }
                        "wizard" => {
                            self.push_modal(ModalState::setup_wizard(
                                crate::modals::SetupWizardState::new(
                                    &self.config,
                                    &self.available_models,
                                ),
                            ));
                            return;
                        }
                        "status" => {
                            // fall through to print status
                        }
                        _ => {
                            self.push_modal(ModalState::setup_wizard(
                                crate::modals::SetupWizardState::new(
                                    &self.config,
                                    &self.available_models,
                                ),
                            ));
                            return;
                        }
                    }
                } else {
                    self.push_modal(ModalState::setup_wizard(
                        crate::modals::SetupWizardState::new(&self.config, &self.available_models),
                    ));
                    return;
                }
                let config_file = m31a_core::Config::config_dir().join("config.toml");
                let has_api_key = self
                    .config
                    .api_key
                    .as_ref()
                    .map(|k| !k.trim().is_empty())
                    .unwrap_or(false);
                let text = format!(
                    "── M31A Configuration Status ─────────────────────────────────\n\
                     • Configuration File: {}\n\
                     • Active Model:       {}\n\
                     • Base API Endpoint:  {}\n\
                     • API Key Status:     {}\n\
                     • Sandbox Policy:     {:?}\n\
                     • Approval Mode:      {:?}\n\
                     • Collaboration Mode: {:?}\n\
                     • Dynamic Routing:    {}\n\n\
                     • To run interactive onboarding: `/setup wizard` or `/setup`\n\
                     • To configure settings interactively: `/settings` or `/setup settings`\n\
                     ──────────────────────────────────────────────────────────────",
                    config_file.display(),
                    self.active_model,
                    self.config.api_base,
                    if has_api_key {
                        "Configured"
                    } else {
                        "Missing - run /setup"
                    },
                    self.config.sandbox,
                    self.config.approval,
                    self.collaboration_mode,
                    if self.config.routing_enabled {
                        "Enabled"
                    } else {
                        "Disabled"
                    }
                );
                self.transcript
                    .push(TranscriptItem::notice(text, NoticeLevel::Info));
                self.sync_virtualizer();
            }
            "/model" | "/models" => {
                if parts.len() > 1 {
                    let arg = parts[1].to_lowercase();
                    if arg == "route" {
                        if parts.len() > 2 {
                            let action = parts[2].to_lowercase();
                            if action == "on" || action == "enable" || action == "true" {
                                self.config.routing_enabled = true;
                                let _ = self.config.save();
                                self.transcript.push(TranscriptItem::notice(
                                    "Dynamic hybrid model routing enabled (tasks dynamically mapped to Fast / Balanced / DeepReasoning tiers).",
                                    NoticeLevel::Success,
                                ));
                            } else if action == "off" || action == "disable" || action == "false" {
                                self.config.routing_enabled = false;
                                let _ = self.config.save();
                                self.transcript.push(TranscriptItem::notice(
                                    "Dynamic hybrid model routing disabled.",
                                    NoticeLevel::Success,
                                ));
                            } else {
                                self.transcript.push(TranscriptItem::notice(
                                    format!(
                                        "Routing status: {}",
                                        if self.config.routing_enabled {
                                            "Enabled"
                                        } else {
                                            "Disabled"
                                        }
                                    ),
                                    NoticeLevel::Info,
                                ));
                            }
                        } else {
                            self.transcript.push(TranscriptItem::notice(
                                format!(
                                    "Routing status: {} (Usage: `/model route [on|off]`)",
                                    if self.config.routing_enabled {
                                        "Enabled"
                                    } else {
                                        "Disabled"
                                    }
                                ),
                                NoticeLevel::Info,
                            ));
                        }
                        self.sync_virtualizer();
                        if self.auto_scroll {
                            self.scroll_to_bottom();
                        }
                        return;
                    }
                    if arg == "refresh" || arg == "reload" {
                        let api_base = self.config.api_base.clone();
                        let api_key = self.config.api_key.clone().unwrap_or_default();
                        let tx = self.event_tx.clone();
                        if let Ok(handle) = tokio::runtime::Handle::try_current() {
                            handle.spawn(async move {
                                if let Ok(models) =
                                    m31a_provider::fetch_remote_models(&api_base, &api_key).await
                                {
                                    let _ = tx.send(TuiEvent::ModelsDiscovered(models)).await;
                                }
                            });
                        }
                        self.transcript.push(TranscriptItem::notice(
                            "Refreshing models dynamically from provider endpoint...".to_string(),
                            NoticeLevel::Info,
                        ));
                        self.sync_virtualizer();
                        return;
                    }

                    // Check for preset aliases or accept custom slug
                    let target_slug = match arg.as_str() {
                        "fast" | "bolt" => "nvidia/nemotron-3.5-bolt-e4b",
                        "coder" | "code" => "mistralai/codestral-22b-instruct-v0.1",
                        "reasoning" | "r1" | "think" => "deepseek-ai/deepseek-v4.1-flash",
                        "flagship" | "default" => "nvidia/nemotron-3.5-lightning-30b-a3b",
                        _ => parts[1],
                    };
                    if let Err(e) = self.switch_model_async(target_slug) {
                        self.provider_status_message = Some(format!("Model switch failed: {e}"));
                    }
                } else {
                    self.push_modal(ModalState::model_picker(
                        self.available_models.clone(),
                        &self.active_model,
                    ));
                }
            }
            "/context" => {
                if parts.len() > 1 {
                    match parts[1].to_lowercase().as_str() {
                        "refresh" | "update" | "reload" => {
                            self.refresh_system_prompt();
                            self.transcript.push(TranscriptItem::notice(
                                "Dynamic context re-scanned and synchronized with workspace state.",
                                NoticeLevel::Success,
                            ));
                        }
                        "pin" | "add" => {
                            if parts.len() > 2 {
                                let target_path = parts[2];
                                let abs_path = self.cwd.join(target_path);
                                match std::fs::read_to_string(&abs_path) {
                                    Ok(content) => {
                                        let tokens = content.len() / 4;
                                        self.pinned_context.retain(|item| item.path != target_path);
                                        self.pinned_context.push(m31a_core::DynamicContextItem {
                                            path: target_path.to_string(),
                                            content,
                                            tokens,
                                        });
                                        self.refresh_system_prompt();
                                        self.transcript.push(TranscriptItem::notice(
                                            format!(
                                                "Pinned '{}' into dynamic context (~{} tokens).",
                                                target_path, tokens
                                            ),
                                            NoticeLevel::Success,
                                        ));
                                    }
                                    Err(e) => {
                                        self.transcript.push(TranscriptItem::notice(
                                            format!("Failed to read file '{}': {}", target_path, e),
                                            NoticeLevel::Error,
                                        ));
                                    }
                                }
                            } else {
                                self.transcript.push(TranscriptItem::notice(
                                    "Usage: `/context pin <file-path>`",
                                    NoticeLevel::Warning,
                                ));
                            }
                        }
                        "unpin" | "remove" => {
                            if parts.len() > 2 {
                                let target = parts[2];
                                let before = self.pinned_context.len();
                                self.pinned_context.retain(|item| item.path != target);
                                if self.pinned_context.len() < before {
                                    self.refresh_system_prompt();
                                    self.transcript.push(TranscriptItem::notice(
                                        format!("Unpinned '{}' from dynamic context.", target),
                                        NoticeLevel::Success,
                                    ));
                                } else {
                                    self.transcript.push(TranscriptItem::notice(
                                        format!("'{}' was not pinned in dynamic context.", target),
                                        NoticeLevel::Warning,
                                    ));
                                }
                            } else {
                                self.transcript.push(TranscriptItem::notice(
                                    "Usage: `/context unpin <file-path>`",
                                    NoticeLevel::Warning,
                                ));
                            }
                        }
                        "clear" => {
                            let count = self.pinned_context.len();
                            self.pinned_context.clear();
                            self.refresh_system_prompt();
                            self.transcript.push(TranscriptItem::notice(
                                format!("Cleared {} pinned item(s) from dynamic context.", count),
                                NoticeLevel::Success,
                            ));
                        }
                        "compact" => {
                            let before_count = self.chat_history.len();
                            if before_count > 6 {
                                let retained = self.chat_history.split_off(before_count - 4);
                                self.chat_history = retained;
                                self.refresh_system_prompt();
                                self.transcript.push(TranscriptItem::notice(
                                    "Compacted context history: pruned earlier turns, retained last 4 messages.",
                                    NoticeLevel::Success,
                                ));
                            } else {
                                self.transcript.push(TranscriptItem::notice(
                                    "Context history is already compact.",
                                    NoticeLevel::Info,
                                ));
                            }
                        }
                        other => {
                            self.transcript.push(TranscriptItem::notice(
                                format!("Unknown /context subcommand: `{other}`. Supported: status, refresh, pin, unpin, clear, compact"),
                                NoticeLevel::Warning,
                            ));
                        }
                    }
                } else {
                    let pinned = self.pinned_context.clone();
                    let world_sections = self
                        .world_state_digests
                        .iter()
                        .map(|(k, v)| (k.clone(), v.clone()))
                        .collect();
                    self.push_modal(ModalState::context_manager(pinned, world_sections));
                }
            }
            "/palette" => {
                self.open_command_palette();
            }
            "/build" => {
                self.collaboration_mode = CollaborationMode::Default;
                self.bottom_pane.collaboration_mode = CollaborationMode::Default;
                self.refresh_system_prompt();
                self.transcript.push(TranscriptItem::notice(
                    "Agent Mode set to [BUILD]: Autonomous implementation & execution enabled.",
                    NoticeLevel::Success,
                ));
                self.sync_virtualizer();
            }
            "/plan" => {
                self.toggle_collaboration_mode();
            }
            "/effort" | "/reasoning-effort" => {
                if parts.len() > 1 {
                    let val = parts[1].to_lowercase();
                    let res = match val.as_str() {
                        "none" | "default" | "reset" | "off" => self.apply_reasoning_effort(None),
                        "low" => self.apply_reasoning_effort(Some("low".to_string())),
                        "medium" | "med" => self.apply_reasoning_effort(Some("medium".to_string())),
                        "high" => self.apply_reasoning_effort(Some("high".to_string())),
                        other => {
                            self.transcript.push(TranscriptItem::notice(
                                format!(
                                    "Invalid effort `{other}`. Supported: low, medium, high, none"
                                ),
                                NoticeLevel::Warning,
                            ));
                            self.sync_virtualizer();
                            return;
                        }
                    };
                    if let Err(e) = res {
                        self.transcript.push(TranscriptItem::notice(
                            format!("Failed to apply reasoning effort: {e}"),
                            NoticeLevel::Error,
                        ));
                        self.sync_virtualizer();
                    }
                } else {
                    self.push_modal(ModalState::reasoning_effort(
                        self.config.reasoning_effort.clone(),
                    ));
                }
            }
            "/status" => {
                let context_limit = 131_072usize;
                let tokens_used = self.tokens_used as usize;
                let context_pct = ((tokens_used as f64 / context_limit as f64) * 100.0).min(100.0);
                let tokens_remaining = context_limit.saturating_sub(tokens_used);
                let remaining_pct = (100.0 - context_pct).max(0.0);
                let now_local = chrono::Local::now().format("%Y-%m-%d %H:%M:%S %Z");
                let account = std::env::var("USER").unwrap_or_else(|_| "anonymous".to_string());
                let has_api_key = std::env::var("NVIDIA_API_KEY").is_ok();
                let rate_limit_label = if has_api_key {
                    "Standard NIM Tier (active)"
                } else {
                    "Unauthenticated (preview limits)"
                };

                let status_card = format!(
                    "── Session Status Card ───────────────────────────────────────\n\
                     • Account: {} ({})\n\
                     • Model: {}\n\
                     • Reasoning Effort: {}\n\
                     • Permission Profile: Sandbox={:?}, Approval={:?}\n\
                     • Directory: {}\n\
                     • Rate Limits: {} [Local Time: {}]\n\
                     • Context Window: {:.1}% used ({} / {} tokens)\n\
                     • Remaining Capacity: {:.1}% ({} tokens remaining)\n\
                     • Collaboration Mode: {:?}\n\
                     ──────────────────────────────────────────────────────────────",
                    account,
                    if has_api_key {
                        "API Key Configured"
                    } else {
                        "No Key"
                    },
                    self.active_model,
                    self.nim_client.reasoning_effort().unwrap_or("default"),
                    self.config.sandbox,
                    self.config.approval,
                    self.cwd.display(),
                    rate_limit_label,
                    now_local,
                    context_pct,
                    tokens_used,
                    context_limit,
                    remaining_pct,
                    tokens_remaining,
                    self.collaboration_mode,
                );
                self.transcript
                    .push(TranscriptItem::notice(status_card, NoticeLevel::Info));
            }
            "/dial" | "/dials" | "/quick" => {
                if parts.len() > 1
                    && let Ok(idx) = parts[1].parse::<u8>()
                    && let Some(prompt) = self.quick_dials.get(&idx)
                {
                    self.composer.set_text(prompt);
                    self.transcript.push(TranscriptItem::notice(
                        format!("Quick Dial #{} loaded into composer.", idx),
                        NoticeLevel::Info,
                    ));
                    self.sync_virtualizer();
                    if self.auto_scroll {
                        self.scroll_to_bottom();
                    }
                    return;
                }
                // Show HUD of all quick dials
                let mut hud = String::from(
                    "── Quick Dials HUD (/dial <number>) ──────────────────────────\n",
                );
                for (num, prompt) in &self.quick_dials {
                    hud.push_str(&format!("  [#{}] {}\n", num, prompt));
                }
                hud.push_str("──────────────────────────────────────────────────────────────");
                self.transcript
                    .push(TranscriptItem::notice(hud, NoticeLevel::Info));
            }
            "/skills" => {
                let skills = SkillLoader::discover_skills(&self.cwd);
                if parts.len() > 1 {
                    let query = parts[1..].join(" ").to_lowercase();
                    let filtered: Vec<_> = skills
                        .into_iter()
                        .filter(|s| {
                            s.name.to_lowercase().contains(&query)
                                || s.description.to_lowercase().contains(&query)
                        })
                        .collect();
                    if filtered.is_empty() {
                        self.transcript.push(TranscriptItem::notice(
                            format!("No skills matching `{query}` found."),
                            NoticeLevel::Warning,
                        ));
                    } else {
                        let mut text = format!("Matching Skills ({}):\n", filtered.len());
                        for s in &filtered {
                            text.push_str(&format!(
                                "  - {} ({}): {}\n",
                                s.name,
                                s.path.display(),
                                s.description
                            ));
                        }
                        self.transcript
                            .push(TranscriptItem::notice(text, NoticeLevel::Info));
                    }
                } else {
                    self.push_modal(ModalState::skills_modal(skills));
                }
            }
            "/agents" | "/subagents" => {
                let agents = self
                    .storage
                    .list_subagents(&self.session_id)
                    .unwrap_or_default();
                let recent_posts = self
                    .storage
                    .list_posts("default", "dev", 10)
                    .unwrap_or_default();
                self.push_modal(ModalState::subagents_modal(agents, recent_posts));
            }
            "/goal" | "/objective" => {
                let sub = parts.get(1).map(|s| s.to_lowercase());
                match sub.as_deref() {
                    Some("pause") => {
                        match self.storage.update_goal_status(&self.session_id, "paused") {
                            Ok(_) => {
                                self.transcript.push(TranscriptItem::notice(
                                    "Goal execution paused. Use `/goal resume` to continue.",
                                    NoticeLevel::Warning,
                                ));
                                self.update_status_badges();
                            }
                            Err(e) => {
                                self.transcript.push(TranscriptItem::notice(
                                    format!("Failed to pause goal: {}", e),
                                    NoticeLevel::Error,
                                ));
                            }
                        }
                    }
                    Some("resume") => {
                        match self.storage.update_goal_status(&self.session_id, "active") {
                            Ok(_) => {
                                self.transcript.push(TranscriptItem::notice(
                                    "Goal execution resumed.",
                                    NoticeLevel::Success,
                                ));
                                self.update_status_badges();
                                if self.app_status() == AppStatus::Idle
                                    && let Ok(Some(g)) = self.storage.get_goal(&self.session_id)
                                {
                                    let obj = g.objective.clone();
                                    self.submit_prompt(format!(
                                        "Continue autonomous execution toward the goal: {}",
                                        obj
                                    ));
                                }
                            }
                            Err(e) => {
                                self.transcript.push(TranscriptItem::notice(
                                    format!("Failed to resume goal: {}", e),
                                    NoticeLevel::Error,
                                ));
                            }
                        }
                    }
                    Some("clear") | Some("stop") | Some("cancel") => {
                        match self.storage.clear_goal(&self.session_id) {
                            Ok(_) => {
                                self.transcript.push(TranscriptItem::notice(
                                    "Active goal cleared.",
                                    NoticeLevel::Info,
                                ));
                                self.update_status_badges();
                            }
                            Err(e) => {
                                self.transcript.push(TranscriptItem::notice(
                                    format!("Failed to clear goal: {}", e),
                                    NoticeLevel::Error,
                                ));
                            }
                        }
                    }
                    Some("status") => match self.storage.get_goal(&self.session_id) {
                        Ok(Some(g)) => {
                            self.transcript.push(TranscriptItem::notice(
                                    format!("Active Goal:\n- Objective: {}\n- Status: {}\n- Tokens: {}\n- Elapsed: {}s", g.objective, g.status, g.tokens_used, g.time_used_seconds),
                                    NoticeLevel::Info,
                                ));
                        }
                        Ok(None) => {
                            self.transcript.push(TranscriptItem::notice(
                                    "No active goal set for this session. Use `/goal <objective>` to start one.",
                                    NoticeLevel::Info,
                                ));
                        }
                        Err(e) => {
                            self.transcript.push(TranscriptItem::notice(
                                format!("Error fetching goal: {}", e),
                                NoticeLevel::Error,
                            ));
                        }
                    },
                    Some(_) => {
                        let objective = parts[1..].join(" ");
                        match self.storage.set_goal(&self.session_id, &objective, None) {
                            Ok(_) => {
                                self.transcript.push(TranscriptItem::notice(
                                    format!(
                                        "Goal set: \"{}\". Initiating autonomous execution loop...",
                                        objective
                                    ),
                                    NoticeLevel::Success,
                                ));
                                self.update_status_badges();
                                self.submit_prompt(format!(
                                    "Autonomous goal initialized: {}. Break this down into steps using todowrite, systematically implement, verify with test/checks, and update goal status when done.",
                                    objective
                                ));
                            }
                            Err(e) => {
                                self.transcript.push(TranscriptItem::notice(
                                    format!("Failed to set goal: {}", e),
                                    NoticeLevel::Error,
                                ));
                            }
                        }
                    }
                    None => {
                        let goal = self.storage.get_goal(&self.session_id).ok().flatten();
                        self.push_modal(ModalState::goal_dashboard(goal));
                    }
                }
            }
            "/mcp" | "/hub" => {
                let mcp = self.mcp_manager.clone();
                let servers = self
                    .config
                    .mcp_servers
                    .iter()
                    .map(|(name, cfg)| {
                        let is_conn = mcp.is_server_connected_sync(name);
                        let tools = mcp.get_server_tools_sync(name);
                        let tools_count = tools.len();
                        crate::modals::McpServerDisplay {
                            name: name.clone(),
                            command: cfg.command.clone(),
                            transport: cfg.transport.clone().unwrap_or_else(|| "stdio".to_string()),
                            tools_count,
                            tools,
                            enabled: is_conn,
                        }
                    })
                    .collect();
                self.push_modal(ModalState::mcp_hub(servers));
            }
            "/template" | "/templates" => {
                let custom_dir = m31a_core::Config::config_dir().join("templates");
                let library = m31a_collaboration_templates::TemplateLibrary::new(custom_dir);
                let templates = library.list().into_iter().cloned().collect();
                self.push_modal(ModalState::templates_modal(templates));
            }
            "/persona" | "/personas" => {
                self.push_modal(ModalState::persona_modal(
                    ModalState::default_personas(),
                    None,
                ));
            }
            "/bell" | "/sound" => {
                if parts.len() > 1 {
                    let arg = parts[1].to_lowercase();
                    if arg == "on" || arg == "true" || arg == "enable" {
                        self.sound_enabled = true;
                    } else if arg == "off" || arg == "false" || arg == "disable" {
                        self.sound_enabled = false;
                    }
                } else {
                    self.sound_enabled = !self.sound_enabled;
                }
                if self.sound_enabled {
                    self.ring_bell();
                }
                self.transcript.push(TranscriptItem::notice(
                    format!(
                        "Audio feedback / terminal bells: {}",
                        if self.sound_enabled {
                            "Enabled (chime on complete/approvals)"
                        } else {
                            "Disabled"
                        }
                    ),
                    NoticeLevel::Success,
                ));
            }
            "/analytics" | "/stats" => {
                self.push_modal(ModalState::AnalyticsDashboard);
            }
            "/stash" => {
                let subcmd = parts.get(1).map(|s| s.to_lowercase());
                match subcmd.as_deref() {
                    Some("pop") => match crate::stash::PromptStash::pop() {
                        Ok(Some(prompt)) => {
                            self.composer.set_text(&prompt);
                            self.transcript.push(TranscriptItem::notice(
                                "Restored latest prompt draft from stash.",
                                NoticeLevel::Success,
                            ));
                        }
                        Ok(None) => {
                            self.transcript.push(TranscriptItem::notice(
                                "Prompt stash is empty.",
                                NoticeLevel::Warning,
                            ));
                        }
                        Err(e) => {
                            self.transcript.push(TranscriptItem::notice(
                                format!("Failed to pop from stash: {e}"),
                                NoticeLevel::Error,
                            ));
                        }
                    },
                    Some("list") => match crate::stash::PromptStash::list() {
                        Ok(entries) => {
                            if entries.is_empty() {
                                self.transcript.push(TranscriptItem::notice(
                                    "Prompt stash is empty.",
                                    NoticeLevel::Info,
                                ));
                            } else {
                                let items: Vec<crate::modals::SelectPickerItem> = entries
                                    .into_iter()
                                    .enumerate()
                                    .map(|(idx, entry)| {
                                        crate::modals::SelectPickerItem::new(
                                            format!("stash_select_{}", entry.id),
                                            format!(
                                                "#{}: {}",
                                                idx + 1,
                                                entry.prompt.chars().take(35).collect::<String>()
                                            ),
                                            entry.prompt.clone(),
                                            "Stash",
                                        )
                                        .with_detail(
                                            format!(
                                                "Stashed at: {}\n\nFull Prompt:\n{}",
                                                entry.timestamp, entry.prompt
                                            ),
                                        )
                                    })
                                    .collect();
                                self.push_modal(ModalState::select_picker(
                                    "Prompt Draft Stash",
                                    items,
                                ));
                            }
                        }
                        Err(e) => {
                            self.transcript.push(TranscriptItem::notice(
                                format!("Failed to list stash: {e}"),
                                NoticeLevel::Error,
                            ));
                        }
                    },
                    Some("clear") => {
                        let _ = crate::stash::PromptStash::clear();
                        self.transcript.push(TranscriptItem::notice(
                            "Cleared prompt draft stash.",
                            NoticeLevel::Info,
                        ));
                    }
                    _ => {
                        let text = self.composer.buffer.trim().to_string();
                        if text.is_empty() {
                            self.transcript.push(TranscriptItem::notice(
                                "Composer buffer is empty. Type a draft first, or use `/stash pop` / `/stash list`.",
                                NoticeLevel::Warning,
                            ));
                        } else {
                            match crate::stash::PromptStash::save(&text) {
                                Ok(count) => {
                                    self.composer.clear();
                                    self.transcript.push(TranscriptItem::notice(
                                        format!("Prompt draft stashed ({count} in stash). Use `/stash pop` to restore."),
                                        NoticeLevel::Success,
                                    ));
                                }
                                Err(e) => {
                                    self.transcript.push(TranscriptItem::notice(
                                        format!("Failed to save prompt to stash: {e}"),
                                        NoticeLevel::Error,
                                    ));
                                }
                            }
                        }
                    }
                }
                self.sync_virtualizer();
            }
            "/diff" => {
                let output = std::process::Command::new("git")
                    .arg("diff")
                    .current_dir(&self.cwd)
                    .output();

                let diff_text = match output {
                    Ok(out) => {
                        let text = String::from_utf8_lossy(&out.stdout).to_string();
                        if text.trim().is_empty() {
                            "No uncommitted changes in git repository.".to_string()
                        } else {
                            text
                        }
                    }
                    Err(e) => format!("Failed to run git diff: {}", e),
                };
                self.push_modal(ModalState::InteractiveDiff(
                    m31a_diff::DiffViewerState::from_unified_patch("Working Tree Diff", &diff_text),
                ));
            }
            "/verify" | "/check" => {
                let cwd = self.cwd.clone();
                let tx = self.event_tx.clone();
                self.transcript.push(TranscriptItem::notice(
                    "Running autonomous verification (compiler/linter check)...".to_string(),
                    NoticeLevel::Info,
                ));
                if let Ok(handle) = tokio::runtime::Handle::try_current() {
                    handle.spawn(async move {
                        match m31a_verify::AutonomousVerifier::verify_check(&cwd, std::time::Duration::from_secs(60)).await {
                            Ok(report) => {
                                if report.success {
                                    let _ = tx.send(TuiEvent::Notice(
                                        format!("Verification PASSED for command `{}`. Workspace is clean.", report.command),
                                        NoticeLevel::Success,
                                    )).await;
                                } else {
                                    let prompt = report.self_healing_prompt.unwrap_or(report.raw_output);
                                    let _ = tx.send(TuiEvent::Notice(
                                        format!("Verification FAILED ({} errors, {} warnings):\n{}", report.error_count, report.warning_count, prompt),
                                        NoticeLevel::Error,
                                    )).await;
                                }
                            }
                            Err(e) => {
                                let _ = tx.send(TuiEvent::Notice(format!("Verification failed to run: {}", e), NoticeLevel::Error)).await;
                            }
                        }
                    });
                }
            }
            "/test" => {
                let cwd = self.cwd.clone();
                let tx = self.event_tx.clone();
                self.transcript.push(TranscriptItem::notice(
                    "Running workspace test suite...".to_string(),
                    NoticeLevel::Info,
                ));
                if let Ok(handle) = tokio::runtime::Handle::try_current() {
                    handle.spawn(async move {
                        match m31a_verify::AutonomousVerifier::verify_tests(
                            &cwd,
                            std::time::Duration::from_secs(120),
                        )
                        .await
                        {
                            Ok(report) => {
                                if report.success {
                                    let _ = tx.send(TuiEvent::Notice(
                                        format!(
                                            "Test suite PASSED for command `{}`. All tests green.",
                                            report.command
                                         ),
                                        NoticeLevel::Success,
                                    )).await;
                                } else {
                                    let prompt =
                                        report.self_healing_prompt.unwrap_or(report.raw_output);
                                    let _ = tx
                                        .send(TuiEvent::Notice(
                                            format!(
                                                "Test suite FAILED ({} errors, {} warnings):\n{}",
                                                report.error_count, report.warning_count, prompt
                                            ),
                                            NoticeLevel::Error,
                                        ))
                                        .await;
                                }
                            }
                            Err(e) => {
                                let _ = tx
                                    .send(TuiEvent::Notice(
                                        format!("Test suite failed to run: {}", e),
                                        NoticeLevel::Error,
                                    ))
                                    .await;
                            }
                        }
                    });
                }
            }
            "/vibe" => {
                let cwd = self.cwd.clone();
                let client = self.nim_client.clone();
                let tx = self.event_tx.clone();
                self.transcript.push(TranscriptItem::notice(
                    "Engaging continuous background Vibe Auto-Healing loop...".to_string(),
                    NoticeLevel::Info,
                ));
                self.sync_virtualizer();

                if let Ok(handle) = tokio::runtime::Handle::try_current() {
                    handle.spawn(async move {
                        let config = m31a_agent_engine::vibe_loop::VibeConfig::default();
                        let engine = m31a_agent_engine::vibe_loop::VibeEngine::new(cwd, client, config);
                        let (mut rx, _run) = engine.start();
                        while let Some(evt) = rx.recv().await {
                            match evt {
                                m31a_agent_engine::vibe_loop::VibeEvent::WorkspaceClean { duration_ms } => {
                                    let _ = tx.send(TuiEvent::Notice(
                                        format!("[VIBE CLEAN] Workspace builds cleanly ({}ms)", duration_ms),
                                        NoticeLevel::Success,
                                    )).await;
                                }
                                m31a_agent_engine::vibe_loop::VibeEvent::SelfHealingSuccess { duration_ms, files_healed } => {
                                    let names: Vec<String> = files_healed.iter().filter_map(|p| p.file_name().map(|n| n.to_string_lossy().to_string())).collect();
                                    let _ = tx.send(TuiEvent::Notice(
                                        format!("[VIBE HEALED] Auto-repaired compiler errors in {}ms! Patched: {}", duration_ms, names.join(", ")),
                                        NoticeLevel::Success,
                                    )).await;
                                }
                                m31a_agent_engine::vibe_loop::VibeEvent::ErrorsDetected { error_count, .. } => {
                                    let _ = tx.send(TuiEvent::Notice(
                                        format!("[VIBE DETECTED] {} build error(s). Background healing engaged...", error_count),
                                        NoticeLevel::Warning,
                                    )).await;
                                }
                                _ => {}
                            }
                        }
                    });
                }
            }
            "/visual" | "/visual-tdd" => {
                let url = if parts.len() > 1 {
                    parts[1].to_string()
                } else {
                    "http://localhost:3000".to_string()
                };
                let tx = self.event_tx.clone();
                self.transcript.push(TranscriptItem::notice(
                    format!("Running autonomous Visual TDD against {}...", url),
                    NoticeLevel::Info,
                ));
                self.sync_virtualizer();

                if let Ok(handle) = tokio::runtime::Handle::try_current() {
                    handle.spawn(async move {
                        let scenario = m31a_verify::VisualTddScenario {
                            name: "TUI Visual Verification".to_string(),
                            description: None,
                            initial_url: url.clone(),
                            viewport: Some((1280, 800)),
                            actions: vec![],
                            assertions: vec![
                                m31a_verify::UiAssertion::ZeroConsoleErrors,
                                m31a_verify::UiAssertion::ZeroNetworkFailures,
                            ],
                        };
                        match m31a_verify::AutonomousVerifier::verify_visual_state(
                            &scenario, None, None,
                        )
                        .await
                        {
                            Ok(rep) => {
                                if rep.passed {
                                    let _ = tx.send(TuiEvent::Notice(
                                        format!(
                                            "Visual TDD PASSED on {}. DOM and console are clean.",
                                            rep.initial_url
                                        ),
                                        NoticeLevel::Success,
                                    )).await;
                                } else {
                                    let prompt = rep
                                        .self_healing_guidance
                                        .unwrap_or_else(|| "Visual assertions failed".to_string());
                                    let _ = tx
                                        .send(TuiEvent::Notice(
                                            format!(
                                                "Visual TDD FAILED ({} failed assertions):\n{}",
                                                rep.failed_assertions, prompt
                                            ),
                                            NoticeLevel::Error,
                                        ))
                                        .await;
                                }
                            }
                            Err(e) => {
                                let _ = tx
                                    .send(TuiEvent::Notice(
                                        format!("Visual verification failed: {}", e),
                                        NoticeLevel::Error,
                                    ))
                                    .await;
                            }
                        }
                    });
                }
            }
            "/safety" => {
                let cwd = self.cwd.clone();
                match m31a_verify::CodebaseSafetyGate::audit_workspace_changes(&cwd, &[], &[]) {
                    Ok(audit) => {
                        self.transcript.push(TranscriptItem::notice(
                            format!(
                                "Codebase Safety Audit:\n- Verdict: {:?}\n- Blast-Radius Score: {:.1}/100 ({:?})\n- Affected Files: {}\n- Taints: {}\n- Dead Mutations: {}",
                                audit.verdict,
                                audit.blast_radius.blast_radius_score,
                                audit.blast_radius.risk_level,
                                audit.blast_radius.total_affected_files,
                                audit.total_taints_found,
                                audit.unconsumed_mutations_count
                            ),
                            NoticeLevel::Info,
                        ));
                    }
                    Err(e) => {
                        self.transcript.push(TranscriptItem::notice(
                            format!("Failed to run safety audit: {}", e),
                            NoticeLevel::Error,
                        ));
                    }
                }
                self.sync_virtualizer();
            }
            "/outline" | "/symbols" => {
                let target_file = if parts.len() > 1 {
                    self.cwd.join(parts[1])
                } else {
                    let main_rs = self.cwd.join("src").join("main.rs");
                    let lib_rs = self.cwd.join("src").join("lib.rs");
                    if main_rs.exists() {
                        main_rs
                    } else if lib_rs.exists() {
                        lib_rs
                    } else {
                        self.cwd.clone()
                    }
                };

                if target_file.is_file() {
                    match std::fs::read_to_string(&target_file) {
                        Ok(content) => {
                            let rel_path =
                                target_file.strip_prefix(&self.cwd).unwrap_or(&target_file);
                            let symbols = m31a_core::parse_symbols_from_source(rel_path, &content);
                            if symbols.is_empty() {
                                self.transcript.push(TranscriptItem::notice(
                                    format!("No symbols found in '{}'", rel_path.display()),
                                    NoticeLevel::Info,
                                ));
                            } else {
                                self.push_modal(ModalState::symbol_outline(
                                    rel_path.display().to_string(),
                                    symbols,
                                ));
                            }
                        }
                        Err(e) => {
                            self.transcript.push(TranscriptItem::notice(
                                format!("Failed to read file '{}': {}", target_file.display(), e),
                                NoticeLevel::Error,
                            ));
                        }
                    }
                } else {
                    self.transcript.push(TranscriptItem::notice(
                        format!(
                            "Target file '{}' not found. Usage: /outline <path>",
                            target_file.display()
                        ),
                        NoticeLevel::Warning,
                    ));
                }
            }
            "/timing" | "/critical-path" => {
                let durations = self.last_turn_critical_path.clone().unwrap_or_default();
                let intervals = self.direct_tool_timings.completed_work();
                self.push_modal(ModalState::critical_path_modal(durations, intervals));
            }
            "/locks" | "/active-locks" => {
                let locks = m31a_core::ExecutionLockManager::global().active_locks();
                self.push_modal(ModalState::active_locks(locks));
            }
            "/checkpoints" => {
                let list = self
                    .storage
                    .list_checkpoints(&self.session_id)
                    .unwrap_or_default();
                self.push_modal(ModalState::checkpoints_modal(list));
            }
            "/ps" | "/processes" | "/procs" => {
                let supervisor = self.proc_supervisor.clone();
                let tx = self.event_tx.clone();
                self.push_modal(ModalState::processes_modal(Vec::new()));
                if let Ok(handle) = tokio::runtime::Handle::try_current() {
                    handle.spawn(async move {
                        let procs = supervisor.list().await;
                        let _ = tx.send(TuiEvent::ProcessListUpdated(procs)).await;
                    });
                }
            }
            "/kill" => {
                if parts.len() < 2 {
                    self.transcript.push(TranscriptItem::notice(
                        "Usage: /kill <process_id>".to_string(),
                        NoticeLevel::Warning,
                    ));
                } else {
                    let proc_id = parts[1].to_string();
                    let supervisor = self.proc_supervisor.clone();
                    let tx = self.event_tx.clone();
                    if let Ok(handle) = tokio::runtime::Handle::try_current() {
                        handle.spawn(async move {
                            match supervisor.stop(&proc_id, false).await {
                                Ok(r) => {
                                    let _ = tx
                                        .send(TuiEvent::Notice(
                                            format!("Process {}: {}", r.id, r.message),
                                            if r.stopped {
                                                NoticeLevel::Success
                                            } else {
                                                NoticeLevel::Warning
                                            },
                                        ))
                                        .await;
                                    let procs = supervisor.list().await;
                                    let _ = tx.send(TuiEvent::ProcessListUpdated(procs)).await;
                                }
                                Err(e) => {
                                    let _ = tx
                                        .send(TuiEvent::Notice(
                                            format!("Kill error: {}", e),
                                            NoticeLevel::Error,
                                        ))
                                        .await;
                                }
                            }
                        });
                    }
                }
            }
            "/logs" => {
                if parts.len() < 2 {
                    self.transcript.push(TranscriptItem::notice(
                        "Usage: /logs <process_id>".to_string(),
                        NoticeLevel::Warning,
                    ));
                } else {
                    let proc_id = parts[1].to_string();
                    let supervisor = self.proc_supervisor.clone();
                    let tx = self.event_tx.clone();
                    if let Ok(handle) = tokio::runtime::Handle::try_current() {
                        handle.spawn(async move {
                            let p_info = supervisor.get(&proc_id).await;
                            let lbl = p_info.map(|p| p.label).unwrap_or_else(|| proc_id.clone());
                            let lines = supervisor
                                .tail_output(&proc_id, 200)
                                .await
                                .unwrap_or_default();
                            let _ = tx
                                .send(TuiEvent::ProcessOutputReady {
                                    id: proc_id,
                                    label: lbl,
                                    lines,
                                })
                                .await;
                        });
                    }
                }
            }
            "/apply" => {
                let outcome = self.trust_store.lock().unwrap().authorize_operation(
                    &self.cwd,
                    m31a_trust::OperationCapability::PatchApplication,
                );
                match outcome {
                    m31a_trust::AuthorizationOutcome::Denied { reason, .. } => {
                        self.transcript.push(TranscriptItem::notice(
                            format!(
                                "Patch application BLOCKED: Working directory is Denied ({reason})."
                            ),
                            NoticeLevel::Error,
                        ));
                        return;
                    }
                    m31a_trust::AuthorizationOutcome::RequiresConfirmation { reason, .. } => {
                        self.transcript.push(TranscriptItem::notice(
                            format!(
                                "Patch application BLOCKED: Working directory is untrusted ({reason}).\nRun `/trust session` or `/trust add` first."
                            ),
                            NoticeLevel::Error,
                        ));
                        return;
                    }
                    m31a_trust::AuthorizationOutcome::Allowed => {}
                }

                if parts.len() > 1 {
                    let patch_arg = parts[1];
                    let candidate_path = if std::path::Path::new(patch_arg).is_absolute() {
                        std::path::PathBuf::from(patch_arg)
                    } else {
                        self.cwd.join(patch_arg)
                    };

                    let canonical_cwd =
                        self.cwd.canonicalize().unwrap_or_else(|_| self.cwd.clone());
                    let canonical_patch = match candidate_path.canonicalize() {
                        Ok(p) => p,
                        Err(e) => {
                            self.transcript.push(TranscriptItem::notice(
                                format!("Could not read patch file '{}': {}", patch_arg, e),
                                NoticeLevel::Error,
                            ));
                            return;
                        }
                    };

                    if !canonical_patch.starts_with(&canonical_cwd) {
                        self.transcript.push(TranscriptItem::notice(
                            format!(
                                "Security error: Patch file '{}' escapes workspace root '{}'",
                                patch_arg,
                                self.cwd.display()
                            ),
                            NoticeLevel::Error,
                        ));
                        return;
                    }

                    match std::fs::read_to_string(&canonical_patch) {
                        Ok(patch_content) => {
                            match m31a_patch::PatchEngine::apply(&self.cwd, &patch_content) {
                                Ok(stats) => {
                                    self.transcript.push(TranscriptItem::notice(
                                        format!(
                                            "Applied patch '{}' successfully ({} files modified, {} hunks applied).",
                                            parts[1], stats.files_modified, stats.hunks_applied
                                        ),
                                        NoticeLevel::Success,
                                    ));
                                }
                                Err(e) => {
                                    self.transcript.push(TranscriptItem::notice(
                                        format!("Failed to apply patch '{}': {}", parts[1], e),
                                        NoticeLevel::Error,
                                    ));
                                }
                            }
                        }
                        Err(e) => {
                            self.transcript.push(TranscriptItem::notice(
                                format!("Could not read patch file '{}': {}", parts[1], e),
                                NoticeLevel::Error,
                            ));
                        }
                    }
                } else {
                    let mut found_patch = None;
                    for item in self.transcript.iter().rev() {
                        match item {
                            TranscriptItem::Assistant { text } => {
                                if let Some(patch) = extract_patch_block(text) {
                                    found_patch = Some(patch);
                                    break;
                                }
                            }
                            TranscriptItem::ToolCall(tc) => {
                                if let Some(ref out) = tc.output
                                    && let Some(patch) = extract_patch_block(out)
                                {
                                    found_patch = Some(patch);
                                    break;
                                }
                            }
                            _ => {}
                        }
                    }

                    if let Some(patch_str) = found_patch {
                        match m31a_patch::PatchEngine::apply(&self.cwd, &patch_str) {
                            Ok(stats) => {
                                self.transcript.push(TranscriptItem::notice(
                                    format!(
                                        "Extracted and applied latest patch from transcript ({} files modified, {} hunks applied).",
                                        stats.files_modified, stats.hunks_applied
                                    ),
                                    NoticeLevel::Success,
                                ));
                            }
                            Err(e) => {
                                self.transcript.push(TranscriptItem::notice(
                                    format!(
                                        "Failed applying extracted patch from transcript: {}",
                                        e
                                    ),
                                    NoticeLevel::Error,
                                ));
                            }
                        }
                    } else {
                        let output = std::process::Command::new("git")
                            .args(["status", "--short"])
                            .current_dir(&self.cwd)
                            .output();
                        match output {
                            Ok(out) => {
                                let status = String::from_utf8_lossy(&out.stdout).to_string();
                                self.transcript.push(TranscriptItem::notice(
                                    format!(
                                        "Git Working Tree Status:\n{}\nTip: Run `/apply <patch-file>` to apply an explicit patch.",
                                        if status.is_empty() { "Working tree clean." } else { &status }
                                    ),
                                    NoticeLevel::Info,
                                ));
                            }
                            Err(e) => {
                                self.transcript.push(TranscriptItem::notice(
                                    format!("Git apply error: {}", e),
                                    NoticeLevel::Error,
                                ));
                            }
                        }
                    }
                }
            }
            "/review" => {
                let base = if parts.len() > 1 {
                    Some(parts[1])
                } else {
                    None
                };
                match m31a_review::ReviewEngine::analyze_git(&self.cwd, base) {
                    Ok(report) => {
                        let md = m31a_review::ReviewFormatter::render_markdown(&report);
                        self.transcript.push(TranscriptItem::notice(
                            format!(
                                "Automated Code Review Completed (Risk: {:?}, Verdict: {:?}):\n{}",
                                report.risk_level, report.verdict, md
                            ),
                            if report.critical_count() > 0 {
                                NoticeLevel::Error
                            } else if report.high_count() > 0 {
                                NoticeLevel::Warning
                            } else {
                                NoticeLevel::Success
                            },
                        ));
                        if !report.findings.is_empty() {
                            let prompt = format!(
                                "Please review the automated code review findings below and propose specific remediation fixes for any Critical or High defects:\n\n{}",
                                md
                            );
                            self.submit_prompt(prompt);
                        }
                    }
                    Err(e) => {
                        self.transcript.push(TranscriptItem::notice(
                            format!("Code review error: {}", e),
                            NoticeLevel::Error,
                        ));
                    }
                }
            }
            "/def" => {
                if parts.len() < 2 {
                    self.transcript.push(TranscriptItem::notice(
                        "Usage: /def <symbol_name>".to_string(),
                        NoticeLevel::Warning,
                    ));
                } else {
                    let symbol = parts[1];
                    let index = m31a_ast::AstCodebaseIndex::build(&self.cwd, 1000);
                    let defs = index.find_definition(symbol);
                    if defs.is_empty() {
                        self.transcript.push(TranscriptItem::notice(
                            format!("No definition for `{}` found in workspace.", symbol),
                            NoticeLevel::Warning,
                        ));
                    } else {
                        let mut out =
                            format!("Found {} definition(s) for `{}`:\n", defs.len(), symbol);
                        for def in defs {
                            let rel_path = def
                                .file_path
                                .strip_prefix(&self.cwd)
                                .unwrap_or(&def.file_path);
                            let scope = def
                                .parent_scope
                                .map(|s| format!(" in {}", s))
                                .unwrap_or_default();
                            out.push_str(&format!(
                                "  {}:{}-{} -> [{}{}] {}\n",
                                rel_path.display(),
                                def.range.start_line,
                                def.range.end_line,
                                def.kind.as_str(),
                                scope,
                                def.signature
                            ));
                        }
                        self.transcript
                            .push(TranscriptItem::notice(out, NoticeLevel::Info));
                    }
                }
            }
            "/trust" => {
                let sub = if parts.len() > 1 { parts[1] } else { "status" };
                match sub {
                    "add" | "yes" | "y" => {
                        match self
                            .trust_store
                            .lock()
                            .unwrap()
                            .trust(&self.cwd, m31a_trust::TrustLevel::Trusted)
                        {
                            Ok(()) => {
                                self.transcript.push(TranscriptItem::notice(
                                    format!(
                                        "Marked '{}' as a permanently trusted directory.",
                                        self.cwd.display()
                                    ),
                                    NoticeLevel::Success,
                                ));
                            }
                            Err(e) => {
                                self.transcript.push(TranscriptItem::notice(
                                    format!(
                                        "Failed to record permanent trust for '{}': {}",
                                        self.cwd.display(),
                                        e
                                    ),
                                    NoticeLevel::Error,
                                ));
                            }
                        }
                    }
                    "session" | "once" => {
                        match self
                            .trust_store
                            .lock()
                            .unwrap()
                            .trust(&self.cwd, m31a_trust::TrustLevel::SessionOnly)
                        {
                            Ok(()) => {
                                self.transcript.push(TranscriptItem::notice(
                                    format!(
                                        "Granted session-only trust for '{}'.",
                                        self.cwd.display()
                                    ),
                                    NoticeLevel::Success,
                                ));
                            }
                            Err(e) => {
                                self.transcript.push(TranscriptItem::notice(
                                    format!("Failed to grant session trust: {}", e),
                                    NoticeLevel::Error,
                                ));
                            }
                        }
                    }
                    "revoke" => match self.trust_store.lock().unwrap().revoke(&self.cwd) {
                        Ok(revoked) => {
                            let msg = if revoked {
                                format!("Revoked trust for directory '{}'.", self.cwd.display())
                            } else {
                                format!(
                                    "Directory '{}' was not in trust store.",
                                    self.cwd.display()
                                )
                            };
                            self.transcript
                                .push(TranscriptItem::notice(msg, NoticeLevel::Warning));
                        }
                        Err(e) => {
                            self.transcript.push(TranscriptItem::notice(
                                format!(
                                    "Failed to revoke trust for '{}': {}",
                                    self.cwd.display(),
                                    e
                                ),
                                NoticeLevel::Error,
                            ));
                        }
                    },
                    "audit" => {
                        let report = m31a_trust::GitArmor::audit(&self.cwd);
                        if report.is_safe && report.risks.is_empty() {
                            self.transcript.push(TranscriptItem::notice(
                                "Git Armor audit clean: No suspicious executable hooks or command injection vectors detected.",
                                NoticeLevel::Success,
                            ));
                        } else {
                            let mut msg = format!(
                                "Git Armor Report ({} risk(s) identified):\n",
                                report.risks.len()
                            );
                            for r in &report.risks {
                                msg.push_str(&format!(
                                    "  - [{:?}] {}: {}\n",
                                    r.severity, r.title, r.description
                                ));
                            }
                            self.transcript
                                .push(TranscriptItem::notice(msg, NoticeLevel::Warning));
                        }
                    }
                    _ => {
                        let decision = self.trust_store.lock().unwrap().evaluate(&self.cwd);
                        self.transcript.push(TranscriptItem::notice(
                            format!("Workspace Trust Status for '{}': {:?}\nSubcommands: /trust add | /trust session | /trust audit | /trust revoke", self.cwd.display(), decision),
                            NoticeLevel::Info,
                        ));
                    }
                }
                self.sync_virtualizer();
            }
            "/fork" => {
                let prefix: String = self.session_id.chars().take(8).collect();
                let branch_title = format!("Fork of {}", prefix);
                match self.storage.fork_thread(&self.session_id, &branch_title) {
                    Ok(forked) => {
                        self.session_id = forked.id.clone();
                        self.transcript.push(TranscriptItem::notice(
                            format!(
                                "Forked current session into new branch: '{}' ({})",
                                forked.title, forked.id
                            ),
                            NoticeLevel::Success,
                        ));
                    }
                    Err(e) => {
                        self.transcript.push(TranscriptItem::notice(
                            format!("Failed to fork session: {}", e),
                            NoticeLevel::Error,
                        ));
                    }
                }
            }
            "/worktree" | "/workspace" => {
                if let Ok(mgr) = WorktreeManager::new() {
                    if parts.len() > 1 && parts[1] == "create" {
                        let base = parts.get(2).copied();
                        match mgr.create_worktree(&self.cwd, base) {
                            Ok(wt) => {
                                self.cwd = wt.path;
                                self.refresh_system_prompt();
                                self.transcript.push(TranscriptItem::notice(
                                    format!("Created and switched to worktree: {:?}", self.cwd),
                                    NoticeLevel::Success,
                                ));
                            }
                            Err(e) => {
                                self.transcript.push(TranscriptItem::notice(
                                    format!("Worktree error: {}", e),
                                    NoticeLevel::Error,
                                ));
                            }
                        }
                    } else if parts.len() > 1 && parts[1] == "list" {
                        match mgr.list_worktrees(&self.cwd) {
                            Ok(list) => {
                                let mut text = "Managed Git Worktrees:\n".to_string();
                                for l in list {
                                    text.push_str(&format!("  {}\n", l));
                                }
                                self.transcript
                                    .push(TranscriptItem::notice(text, NoticeLevel::Info));
                            }
                            Err(e) => {
                                self.transcript.push(TranscriptItem::notice(
                                    format!("Worktree list error: {}", e),
                                    NoticeLevel::Error,
                                ));
                            }
                        }
                    } else {
                        // Open interactive modal
                        match mgr.list_worktrees(&self.cwd) {
                            Ok(worktrees) => {
                                let current_idx = worktrees
                                    .iter()
                                    .position(|w| w.path == self.cwd)
                                    .unwrap_or(0);
                                self.push_modal(ModalState::WorktreeSelector {
                                    worktrees,
                                    selected_index: current_idx,
                                });
                            }
                            Err(e) => {
                                self.transcript.push(TranscriptItem::notice(
                                    format!("Worktree error: {}", e),
                                    NoticeLevel::Error,
                                ));
                            }
                        }
                    }
                } else {
                    self.transcript.push(TranscriptItem::notice(
                        "Failed to initialize worktree manager.",
                        NoticeLevel::Error,
                    ));
                }
            }
            "/hooks" => {
                let engine = HookEngine::load_from_dir(&self.cwd);
                let hooks = engine.list_hooks();
                if hooks.is_empty() {
                    self.transcript.push(TranscriptItem::notice(
                        "No lifecycle hooks configured.",
                        NoticeLevel::Info,
                    ));
                } else {
                    let mut text = "Configured Lifecycle Hooks:\n".to_string();
                    for h in hooks {
                        text.push_str(&format!(
                            "  - {} [{}] -> {}\n",
                            h.name,
                            h.event.as_str(),
                            h.command
                        ));
                    }
                    self.transcript
                        .push(TranscriptItem::notice(text, NoticeLevel::Info));
                }
            }
            "/recap" => {
                // §5.4: bounded catch-up — a recap fragment submitted as a
                // regular turn, parsed on completion (JSON {summary, next_action}).
                let history_text = self
                    .chat_history
                    .iter()
                    .filter_map(|m| m.content.as_deref())
                    .map(|c| format!("{c}\n"))
                    .collect::<String>();
                let recap = m31a_core::fragments::RecapPrompt::new(&history_text);
                let prompt =
                    m31a_core::fragments::ContextualUserFragment::render_fragment(&recap).content;
                self.transcript.push(TranscriptItem::notice(
                    "Generating bounded recap of this session...".to_string(),
                    NoticeLevel::Info,
                ));
                self.submit_prompt(prompt);
            }
            "/experimental" => {
                // §7.1: experimental features toggle menu
                let registry = self.features_registry.clone();
                let items = registry.list_experimental_features();
                self.push_modal(ModalState::ExperimentalFeatures {
                    items,
                    selected_index: 0,
                });
            }
            "/memories" => {
                if let Ok(mgr) = MemoryManager::new() {
                    let text = mgr.load_consolidated_memories();
                    if text.is_empty() {
                        self.transcript.push(TranscriptItem::notice(
                            "No consolidated memories found.",
                            NoticeLevel::Info,
                        ));
                    } else {
                        self.transcript
                            .push(TranscriptItem::notice(text, NoticeLevel::Info));
                    }
                }
            }
            "/undo" => {
                match self.storage.list_checkpoints(&self.session_id) {
                    Ok(cps) if !cps.is_empty() => {
                        let latest = &cps[0];
                        match self.storage.restore_checkpoint(&latest.id, true) {
                            Ok(_) => {
                                if let Some(commit) = &latest.git_commit {
                                    let _ = std::process::Command::new("git")
                                        .args(["checkout", commit, "--", "."])
                                        .current_dir(&self.cwd)
                                        .output();
                                }
                                self.transcript.push(TranscriptItem::notice(
                                    format!("Reverted session and workspace to checkpoint `{}` (Turn {})", latest.id, latest.turn_index),
                                    NoticeLevel::Success,
                                ));
                            }
                            Err(e) => {
                                self.transcript.push(TranscriptItem::notice(
                                    format!("Failed to restore checkpoint: {}", e),
                                    NoticeLevel::Error,
                                ));
                            }
                        }
                    }
                    Ok(_) => {
                        self.transcript.push(TranscriptItem::notice(
                            "No checkpoints found for this session to undo.",
                            NoticeLevel::Warning,
                        ));
                    }
                    Err(e) => {
                        self.transcript.push(TranscriptItem::notice(
                            format!("Failed to query checkpoints: {}", e),
                            NoticeLevel::Error,
                        ));
                    }
                }
            }
            "/revert" => {
                let target_id = if parts.len() > 1 {
                    if parts[1] == "--checkpoint" && parts.len() > 2 {
                        Some(parts[2])
                    } else {
                        Some(parts[1])
                    }
                } else {
                    None
                };

                if let Some(chk_id) = target_id {
                    match self.storage.restore_checkpoint(chk_id, true) {
                        Ok(_) => {
                            let cps = self
                                .storage
                                .list_checkpoints(&self.session_id)
                                .unwrap_or_default();
                            if let Some(target_chk) = cps.iter().find(|c| c.id == chk_id)
                                && let Some(commit) = &target_chk.git_commit
                            {
                                let _ = std::process::Command::new("git")
                                    .args(["checkout", commit, "--", "."])
                                    .current_dir(&self.cwd)
                                    .output();
                            }
                            self.transcript.push(TranscriptItem::notice(
                                format!(
                                    "Reverted session and workspace to checkpoint `{}`",
                                    chk_id
                                ),
                                NoticeLevel::Success,
                            ));
                        }
                        Err(e) => {
                            self.transcript.push(TranscriptItem::notice(
                                format!("Failed to revert to checkpoint `{}`: {}", chk_id, e),
                                NoticeLevel::Error,
                            ));
                        }
                    }
                } else {
                    let list = self
                        .storage
                        .list_checkpoints(&self.session_id)
                        .unwrap_or_default();
                    if list.is_empty() {
                        self.transcript.push(TranscriptItem::notice(
                            "No checkpoints found for this session to revert.",
                            NoticeLevel::Warning,
                        ));
                    } else {
                        self.push_modal(ModalState::checkpoints_modal(list));
                    }
                }
            }
            "/search" => {
                if parts.len() > 1 {
                    let query = parts[1..].join(" ");
                    let prompt = format!(
                        "Please use the web_search tool to find the latest documentation and information for: {}",
                        query
                    );
                    self.submit_prompt(prompt);
                } else {
                    self.transcript.push(TranscriptItem::notice(
                        "Usage: /search <query>",
                        NoticeLevel::Warning,
                    ));
                }
            }
            "/clear" => {
                self.transcript.clear();
                self.scroll_offset = 0;
                self.transcript.push(TranscriptItem::notice(
                    "Transcript view cleared.",
                    NoticeLevel::Info,
                ));
            }
            "/compact" => {
                let before_count = self.chat_history.len();
                if before_count > 4 {
                    let retained = self.chat_history.split_off(before_count - 4);
                    self.chat_history = retained;
                    self.transcript.push(TranscriptItem::notice(
                        "Compacted context history: pruned earlier turns, retained last 4 messages.".to_string(),
                        NoticeLevel::Success,
                    ));
                } else {
                    self.transcript.push(TranscriptItem::notice(
                        "Context history is already compact.",
                        NoticeLevel::Info,
                    ));
                }
            }
            "/export" => {
                let prefix: String = self.session_id.chars().take(8).collect();
                let out_file = self.cwd.join(format!("m31a-session-{}.md", prefix));
                let mut content = format!(
                    "# M31A Session Export: {}\n\n- Model: {}\n- Date: {}\n\n",
                    self.session_id,
                    self.active_model,
                    chrono::Utc::now().to_rfc3339()
                );
                for item in &self.transcript {
                    match item {
                        TranscriptItem::User { text, .. } => {
                            content.push_str(&format!("## User\n\n{}\n\n", text));
                        }
                        TranscriptItem::Assistant { text } => {
                            content.push_str(&format!("## M31A\n\n{}\n\n", text));
                        }
                        TranscriptItem::Thinking { text, .. } => {
                            content.push_str(&format!("> **Thinking**: {}\n\n", text));
                        }
                        TranscriptItem::ToolCall(tc) => {
                            content.push_str(&format!(
                                "### Tool: `{}`\n```json\n{}\n```\n",
                                tc.name, tc.arguments
                            ));
                            if let Some(ref out) = tc.output {
                                content.push_str(&format!("**Result:**\n```\n{}\n```\n\n", out));
                            }
                        }
                        TranscriptItem::Notice { text, .. } => {
                            content.push_str(&format!("*Notice*: {}\n\n", text));
                        }
                    }
                }
                match std::fs::write(&out_file, content) {
                    Ok(_) => self.transcript.push(TranscriptItem::notice(
                        format!("Exported transcript to {:?}", out_file),
                        NoticeLevel::Success,
                    )),
                    Err(e) => self.transcript.push(TranscriptItem::notice(
                        format!("Failed to export transcript: {}", e),
                        NoticeLevel::Error,
                    )),
                }
            }
            "/lock" => {
                if parts.len() < 2 {
                    self.transcript.push(TranscriptItem::notice(
                        "Usage: /lock <relative_file_path>",
                        NoticeLevel::Warning,
                    ));
                } else {
                    let path = parts[1];
                    let is_dir = self.cwd.join(path).is_dir();
                    match self
                        .storage
                        .lock_path(&self.session_id, path, is_dir, "user")
                    {
                        Ok(_) => {
                            self.transcript.push(TranscriptItem::notice(
                                format!("Locked path '{}' from agent modification.", path),
                                NoticeLevel::Success,
                            ));
                        }
                        Err(e) => {
                            self.transcript.push(TranscriptItem::notice(
                                format!("Failed to lock path '{}': {}", path, e),
                                NoticeLevel::Error,
                            ));
                        }
                    }
                }
                self.sync_virtualizer();
            }
            "/unlock" => {
                if parts.len() < 2 {
                    self.transcript.push(TranscriptItem::notice(
                        "Usage: /unlock <relative_file_path>",
                        NoticeLevel::Warning,
                    ));
                } else {
                    let path = parts[1];
                    match self.storage.unlock_path(&self.session_id, path) {
                        Ok(true) => {
                            self.transcript.push(TranscriptItem::notice(
                                format!("Unlocked path '{}'. Agent may now modify it.", path),
                                NoticeLevel::Success,
                            ));
                        }
                        Ok(false) => {
                            self.transcript.push(TranscriptItem::notice(
                                format!("Path '{}' was not locked.", path),
                                NoticeLevel::Info,
                            ));
                        }
                        Err(e) => {
                            self.transcript.push(TranscriptItem::notice(
                                format!("Failed to unlock path '{}': {}", path, e),
                                NoticeLevel::Error,
                            ));
                        }
                    }
                }
                self.sync_virtualizer();
            }
            "/file-locks" | "/path-locks" => {
                match self.storage.list_locks(&self.session_id) {
                    Ok(locks) => {
                        if locks.is_empty() {
                            self.transcript.push(TranscriptItem::notice(
                                "No paths are currently locked. All workspace files may be edited by the agent.",
                                NoticeLevel::Info,
                            ));
                        } else {
                            let mut card = format!(
                                "── Active File Locks ({}) ─────────────────────────────\n",
                                locks.len()
                            );
                            for l in locks {
                                let kind = if l.is_dir { "[DIR]" } else { "[FILE]" };
                                card.push_str(&format!(
                                    "  • {} {} (locked by: {})\n",
                                    kind, l.path, l.locked_by
                                ));
                            }
                            card.push_str(
                                "───────────────────────────────────────────────────────",
                            );
                            self.transcript
                                .push(TranscriptItem::notice(card, NoticeLevel::Info));
                        }
                    }
                    Err(e) => {
                        self.transcript.push(TranscriptItem::notice(
                            format!("Failed to list locks: {}", e),
                            NoticeLevel::Error,
                        ));
                    }
                }
                self.sync_virtualizer();
            }
            "/qr" => {
                let url = "exp://localhost:8081";
                self.transcript.push(TranscriptItem::notice(
                    format!("Expo / Live Dev Preview URL: {}\nOpen this in Expo Go or your mobile browser for live cross-device previewing.", url),
                    NoticeLevel::Info,
                ));
                self.sync_virtualizer();
            }
            "/cd" => {
                if parts.len() > 1 {
                    let target = PathBuf::from(parts[1]);
                    let new_cwd = if target.is_absolute() {
                        target
                    } else {
                        self.cwd.join(target)
                    };
                    if new_cwd.is_dir() {
                        self.cwd = new_cwd.canonicalize().unwrap_or(new_cwd);
                        self.refresh_system_prompt();
                        self.transcript.push(TranscriptItem::notice(
                            format!("Changed working directory to: {:?}", self.cwd),
                            NoticeLevel::Success,
                        ));
                    } else {
                        self.transcript.push(TranscriptItem::notice(
                            format!("Directory does not exist: {:?}", new_cwd),
                            NoticeLevel::Error,
                        ));
                    }
                } else {
                    self.transcript.push(TranscriptItem::notice(
                        "Usage: /cd <path>",
                        NoticeLevel::Warning,
                    ));
                }
            }
            "/pwd" | "/cwd" => {
                self.transcript.push(TranscriptItem::notice(
                    format!("Working directory: {:?}", self.cwd),
                    NoticeLevel::Info,
                ));
            }
            "/resume" | "/sessions" => match self.storage.list_threads(20) {
                Ok(threads) => {
                    self.push_modal(ModalState::resume_picker(threads));
                }
                Err(e) => {
                    self.transcript.push(TranscriptItem::notice(
                        format!("Failed to query past threads: {}", e),
                        NoticeLevel::Error,
                    ));
                }
            },
            "/sandbox" => {
                self.config.sandbox = match self.config.sandbox {
                    SandboxPolicy::ReadOnly => SandboxPolicy::WorkspaceWrite,
                    SandboxPolicy::WorkspaceWrite => SandboxPolicy::DangerFullAccess,
                    SandboxPolicy::DangerFullAccess => SandboxPolicy::ReadOnly,
                };
                if let Err(e) = self.config.save() {
                    tracing::warn!("Failed to persist sandbox config: {e}");
                }
                self.refresh_system_prompt();
                self.transcript.push(TranscriptItem::notice(
                    format!("Sandbox policy updated to: {:?}", self.config.sandbox),
                    NoticeLevel::Info,
                ));
            }
            "/approval" => {
                self.config.approval = match self.config.approval {
                    ApprovalPolicy::OnRequest => ApprovalPolicy::ApproveForMe,
                    ApprovalPolicy::ApproveForMe => ApprovalPolicy::Never,
                    ApprovalPolicy::Never => ApprovalPolicy::OnRequest,
                };
                if let Err(e) = self.config.save() {
                    tracing::warn!("Failed to persist approval config: {e}");
                }
                self.transcript.push(TranscriptItem::notice(
                    format!("Approval policy updated to: {:?}", self.config.approval),
                    NoticeLevel::Info,
                ));
            }
            "/doctor" => {
                let key_status = if std::env::var("NVIDIA_API_KEY").is_ok() {
                    "OK"
                } else {
                    "WARN (missing)"
                };
                let bwrap_status = if std::path::Path::new("/usr/bin/bwrap").exists() {
                    "OK"
                } else {
                    "WARN"
                };
                self.transcript.push(TranscriptItem::notice(
                    format!(
                        "Diagnostics:\n  NVIDIA_API_KEY: {}\n  Bubblewrap: {}\n  Model: {}\n  Working Dir: {:?}",
                        key_status, bwrap_status, self.active_model, self.cwd
                    ),
                    NoticeLevel::Info,
                ));
            }
            "/board" => match self.storage.list_posts("default", "dev", 5) {
                Ok(posts) => {
                    if posts.is_empty() {
                        self.transcript.push(TranscriptItem::notice(
                            "No posts found on default message board.",
                            NoticeLevel::Info,
                        ));
                    } else {
                        let mut msg = "Recent Agent Board Posts:\n".to_string();
                        for p in posts {
                            msg.push_str(&format!("  [#{}] @{}: {}\n", p.seq, p.author, p.body));
                        }
                        self.transcript
                            .push(TranscriptItem::notice(msg, NoticeLevel::Info));
                    }
                }
                Err(e) => {
                    self.transcript.push(TranscriptItem::notice(
                        format!("Error reading board: {}", e),
                        NoticeLevel::Error,
                    ));
                }
            },
            "/theme" => {
                if parts.len() > 1 {
                    let variant = match parts[1].to_lowercase().as_str() {
                        "everblush" => Some(ThemeVariant::Everblush),
                        "light" => Some(ThemeVariant::Light),
                        "dark" => Some(ThemeVariant::Dark),
                        "contrast" | "high-contrast" | "hc" => Some(ThemeVariant::HighContrast),
                        _ => None,
                    };
                    if let Some(v) = variant {
                        init_theme(v);
                        self.config.tui.theme = Some(parts[1].to_lowercase());
                        let _ = self.config.save();
                        self.transcript.push(TranscriptItem::notice(
                            format!("Theme switched to: {}", parts[1].to_lowercase()),
                            NoticeLevel::Success,
                        ));
                    } else {
                        self.transcript.push(TranscriptItem::notice(
                            "Unknown theme variant. Supported: everblush, dark, light, contrast",
                            NoticeLevel::Warning,
                        ));
                    }
                } else {
                    let current = self
                        .config
                        .tui
                        .theme
                        .as_deref()
                        .unwrap_or("everblush")
                        .to_lowercase();
                    let (next, name) = match current.as_str() {
                        "everblush" | "dark" => (ThemeVariant::Light, "light"),
                        "light" => (ThemeVariant::HighContrast, "contrast"),
                        _ => (ThemeVariant::Everblush, "everblush"),
                    };
                    init_theme(next);
                    self.config.tui.theme = Some(name.to_string());
                    let _ = self.config.save();
                    self.transcript.push(TranscriptItem::notice(
                        format!("Theme switched to: {}", name),
                        NoticeLevel::Success,
                    ));
                }
            }
            "/scratchpad" | "/codemode" => {
                self.open_code_mode_scratchpad();
            }
            "/rollout" | "/journal" => {
                self.open_rollout_journal();
            }
            "/exit" | "/quit" | "/q" => {
                self.should_quit = true;
            }
            other => {
                self.transcript.push(TranscriptItem::notice(
                    format!(
                        "Unknown slash command: {}. Type /help for all supported commands.",
                        other
                    ),
                    NoticeLevel::Warning,
                ));
            }
        }

        self.sync_virtualizer();
        if self.auto_scroll {
            self.scroll_to_bottom();
        }
    }

    pub fn refresh_system_prompt(&mut self) {
        let mut snapshot = m31a_core::prompts::PromptRuntimeSnapshot::new(
            &self.cwd,
            &self.active_model,
            &format!("{:?}", self.config.sandbox),
            self.collaboration_mode,
        )
        .with_user_custom_instructions(self.config.system_prompt.clone())
        .with_provider(
            self.config
                .model_provider
                .as_deref()
                .unwrap_or("NVIDIA NIM / Upstream Provider"),
        )
        .with_dynamic_context(self.pinned_context.clone());
        if let Some(ref goal) = self.active_goal {
            snapshot.active_goal = Some(goal.clone());
        }

        let state = SystemPromptBuilder::sections_canonical(&snapshot);

        // Calculate and maintain world-state section digests silently.
        let deltas = state.diff_since(&self.world_state_digests);
        if !deltas.is_empty() {
            self.world_state_digests = state.section_digests();
            // Dynamic context updates must remain silent to conversation transcript.
            // Never display raw internal identifiers such as workspace_status or digest values.
        }

        let sys_prompt = state.render();
        if let Some(first) = self.chat_history.first_mut()
            && first.role == m31a_core::types::Role::System
        {
            first.content = Some(sys_prompt);
            return;
        }
        self.chat_history.insert(0, ChatMessage::system(sys_prompt));
    }

    /// Emits terminal bell chime if sound is enabled.
    pub fn ring_bell(&self) {
        if self.sound_enabled {
            use std::io::Write;
            let mut out = std::io::stdout();
            let _ = out.write_all(b"\x07");
            let _ = out.flush();
        }
    }

    pub fn switch_model(&mut self, slug: &str) -> Result<String> {
        // 1. Reject activation if turn or agent execution is running
        if self.turn_state.is_active() || self.current_turn_id.is_some() {
            let err_msg = "Cannot switch model while a turn or agent execution is active.";
            self.transcript.push(TranscriptItem::notice(
                err_msg.to_string(),
                NoticeLevel::Error,
            ));
            self.sync_virtualizer();
            if self.auto_scroll {
                self.scroll_to_bottom();
            }
            anyhow::bail!("{err_msg}");
        }

        // 2. Pure model resolution (side-effect free, no mutation of available_models or active_model)
        let trimmed_slug = slug.trim();
        if trimmed_slug.is_empty() {
            let err_msg = "Model identifier cannot be empty.";
            self.transcript.push(TranscriptItem::notice(
                err_msg.to_string(),
                NoticeLevel::Error,
            ));
            self.sync_virtualizer();
            anyhow::bail!("{err_msg}");
        }

        // Match exact slug in available_models first
        let matched_slug = self
            .available_models
            .iter()
            .find(|m| m.slug == trimmed_slug)
            .map(|m| m.slug.clone())
            // Or case-insensitive match
            .or_else(|| {
                self.available_models
                    .iter()
                    .find(|m| m.slug.eq_ignore_ascii_case(trimmed_slug))
                    .map(|m| m.slug.clone())
            })
            // Or canonical catalog match
            .or_else(|| {
                m31a_provider::models::find_canonical_model(trimmed_slug).map(|c| c.id.clone())
            });

        let target_slug = match matched_slug {
            Some(s) => s,
            None => {
                // If not found in catalog or available_models, check if explicit provider resolution succeeds
                // (e.g. custom endpoint or local provider configured)
                match ProviderType::resolve(
                    self.config.model_provider.as_deref(),
                    trimmed_slug,
                    Some(&self.config.api_base),
                ) {
                    Ok(_) => trimmed_slug.to_string(),
                    Err(e) => {
                        let err_msg = format!("Unrecognized model '{}': {}", trimmed_slug, e);
                        self.transcript
                            .push(TranscriptItem::notice(err_msg.clone(), NoticeLevel::Error));
                        self.sync_virtualizer();
                        if self.auto_scroll {
                            self.scroll_to_bottom();
                        }
                        anyhow::bail!("{err_msg}");
                    }
                }
            }
        };

        // 3. Provider resolution & credential validation
        let provider_type = match ProviderType::resolve(
            self.config.model_provider.as_deref(),
            &target_slug,
            Some(&self.config.api_base),
        ) {
            Ok(pt) => pt,
            Err(e) => {
                let err_msg = format!(
                    "Failed to resolve provider for model '{}': {}",
                    target_slug, e
                );
                self.transcript
                    .push(TranscriptItem::notice(err_msg.clone(), NoticeLevel::Error));
                self.sync_virtualizer();
                if self.auto_scroll {
                    self.scroll_to_bottom();
                }
                anyhow::bail!("{err_msg}");
            }
        };

        let provider_config = ProviderConfig {
            provider_type,
            api_base: Some(self.config.api_base.clone()),
            api_key: self.config.api_key.clone().filter(|k| !k.trim().is_empty()),
            model: target_slug.clone(),
            extra: std::collections::HashMap::new(),
        };

        if let Err(e) = m31a_provider::validate_provider_config(&provider_config) {
            let err_msg = format!(
                "Configuration validation failed for model '{}': {}",
                target_slug, e
            );
            self.transcript
                .push(TranscriptItem::notice(err_msg.clone(), NoticeLevel::Error));
            self.sync_virtualizer();
            if self.auto_scroll {
                self.scroll_to_bottom();
            }
            anyhow::bail!("{err_msg}");
        }

        // 4. Construct candidate provider client
        let current_effort = self.nim_client.reasoning_effort().map(|s| s.to_string());
        let candidate_client = match ProviderBuilder::build_client(provider_config) {
            Ok(client) => client.with_reasoning_effort(current_effort),
            Err(e) => {
                let err_msg = format!(
                    "Failed to build provider client for '{}': {}",
                    target_slug, e
                );
                self.transcript
                    .push(TranscriptItem::notice(err_msg.clone(), NoticeLevel::Error));
                self.sync_virtualizer();
                if self.auto_scroll {
                    self.scroll_to_bottom();
                }
                anyhow::bail!("{err_msg}");
            }
        };

        // 5. Genuine connectivity validation (bounded by 5-second timeout)
        let candidate_for_probe = candidate_client.clone();
        let probe_res = std::thread::spawn(move || {
            let rt = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(rt) => rt,
                Err(e) => return Err(anyhow::anyhow!("Failed to build runtime for probe: {e}")),
            };
            rt.block_on(async {
                tokio::time::timeout(std::time::Duration::from_secs(5), async {
                    candidate_for_probe.test_model_availability().await?;
                    Ok::<(), anyhow::Error>(())
                })
                .await
                .map_err(|_| anyhow::anyhow!("Connectivity probe timed out after 5s"))?
            })
        })
        .join();

        match probe_res {
            Ok(Ok(())) => {}
            Ok(Err(e)) => {
                let err_msg =
                    format!("Connectivity validation failed for model '{target_slug}': {e}");
                self.transcript
                    .push(TranscriptItem::notice(err_msg.clone(), NoticeLevel::Error));
                self.sync_virtualizer();
                if self.auto_scroll {
                    self.scroll_to_bottom();
                }
                anyhow::bail!("{err_msg}");
            }
            Err(_) => {
                let err_msg =
                    format!("Connectivity probe thread panicked for model '{target_slug}'");
                self.transcript
                    .push(TranscriptItem::notice(err_msg.clone(), NoticeLevel::Error));
                self.sync_virtualizer();
                if self.auto_scroll {
                    self.scroll_to_bottom();
                }
                anyhow::bail!("{err_msg}");
            }
        }

        self.commit_model_switch(&target_slug, candidate_client)
    }

    /// Asynchronous, non-blocking model activation for interactive TUI operations.
    pub fn switch_model_async(&mut self, slug: &str) -> Result<()> {
        if self.turn_state.is_active() || self.current_turn_id.is_some() {
            let err_msg = "Cannot switch model while a turn or agent execution is active.";
            self.transcript.push(TranscriptItem::notice(
                err_msg.to_string(),
                NoticeLevel::Error,
            ));
            self.sync_virtualizer();
            if self.auto_scroll {
                self.scroll_to_bottom();
            }
            anyhow::bail!("{err_msg}");
        }

        let trimmed_slug = slug.trim();
        if trimmed_slug.is_empty() {
            let err_msg = "Model identifier cannot be empty.";
            self.transcript.push(TranscriptItem::notice(
                err_msg.to_string(),
                NoticeLevel::Error,
            ));
            self.sync_virtualizer();
            anyhow::bail!("{err_msg}");
        }

        let matched_slug = self
            .available_models
            .iter()
            .find(|m| m.slug == trimmed_slug)
            .map(|m| m.slug.clone())
            .or_else(|| {
                self.available_models
                    .iter()
                    .find(|m| m.slug.eq_ignore_ascii_case(trimmed_slug))
                    .map(|m| m.slug.clone())
            })
            .or_else(|| {
                m31a_provider::models::find_canonical_model(trimmed_slug).map(|c| c.id.clone())
            });

        let target_slug = match matched_slug {
            Some(s) => s,
            None => {
                match ProviderType::resolve(
                    self.config.model_provider.as_deref(),
                    trimmed_slug,
                    Some(&self.config.api_base),
                ) {
                    Ok(_) => trimmed_slug.to_string(),
                    Err(e) => {
                        let err_msg = format!("Unrecognized model '{}': {}", trimmed_slug, e);
                        self.transcript
                            .push(TranscriptItem::notice(err_msg.clone(), NoticeLevel::Error));
                        self.sync_virtualizer();
                        if self.auto_scroll {
                            self.scroll_to_bottom();
                        }
                        anyhow::bail!("{err_msg}");
                    }
                }
            }
        };

        let provider_type = match ProviderType::resolve(
            self.config.model_provider.as_deref(),
            &target_slug,
            Some(&self.config.api_base),
        ) {
            Ok(pt) => pt,
            Err(e) => {
                let err_msg = format!(
                    "Failed to resolve provider for model '{}': {}",
                    target_slug, e
                );
                self.transcript
                    .push(TranscriptItem::notice(err_msg.clone(), NoticeLevel::Error));
                self.sync_virtualizer();
                if self.auto_scroll {
                    self.scroll_to_bottom();
                }
                anyhow::bail!("{err_msg}");
            }
        };

        let provider_config = ProviderConfig {
            provider_type,
            api_base: Some(self.config.api_base.clone()),
            api_key: self.config.api_key.clone().filter(|k| !k.trim().is_empty()),
            model: target_slug.clone(),
            extra: std::collections::HashMap::new(),
        };

        if let Err(e) = m31a_provider::validate_provider_config(&provider_config) {
            let err_msg = format!(
                "Configuration validation failed for model '{}': {}",
                target_slug, e
            );
            self.transcript
                .push(TranscriptItem::notice(err_msg.clone(), NoticeLevel::Error));
            self.sync_virtualizer();
            if self.auto_scroll {
                self.scroll_to_bottom();
            }
            anyhow::bail!("{err_msg}");
        }

        let current_effort = self.nim_client.reasoning_effort().map(|s| s.to_string());
        let candidate_client = match ProviderBuilder::build_client(provider_config) {
            Ok(client) => client.with_reasoning_effort(current_effort),
            Err(e) => {
                let err_msg = format!(
                    "Failed to build provider client for '{}': {}",
                    target_slug, e
                );
                self.transcript
                    .push(TranscriptItem::notice(err_msg.clone(), NoticeLevel::Error));
                self.sync_virtualizer();
                if self.auto_scroll {
                    self.scroll_to_bottom();
                }
                anyhow::bail!("{err_msg}");
            }
        };

        self.provider_status_message = Some(format!("Connecting to {}...", target_slug));
        self.transcript.push(TranscriptItem::notice(
            format!("Validating connectivity for model '{target_slug}'..."),
            NoticeLevel::Info,
        ));
        self.sync_virtualizer();

        let candidate_for_probe = candidate_client.clone();
        let slug_for_probe = target_slug.clone();
        let event_tx = self.event_tx.clone();
        let task = async move {
            let probe_res = tokio::time::timeout(std::time::Duration::from_secs(5), async {
                candidate_for_probe.test_model_availability().await?;
                Ok::<(), anyhow::Error>(())
            })
            .await;

            let result = match probe_res {
                Ok(Ok(())) => Ok(()),
                Ok(Err(e)) => Err(e.to_string()),
                Err(_) => Err("Connectivity probe timed out after 5s".to_string()),
            };

            let _ = event_tx
                .send(TuiEvent::ModelActivationResult {
                    slug: slug_for_probe,
                    candidate_client: candidate_for_probe,
                    result,
                })
                .await;
        };

        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            handle.spawn(task);
        } else {
            std::thread::spawn(move || {
                if let Ok(rt) = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                {
                    rt.block_on(task);
                }
            });
        }

        Ok(())
    }

    /// Atomically commits a validated candidate model client to application state.
    pub fn commit_model_switch(
        &mut self,
        target_slug: &str,
        candidate_client: ProviderClient,
    ) -> Result<String> {
        let new_engine_runtime = Arc::new(
            m31a_agent_engine::agents::EngineAgentRuntime::new(
                candidate_client.clone(),
                self.tools.clone(),
                self.storage.clone(),
                self.exec_policy.clone(),
                self.cwd.clone(),
                Some(self.hooks.clone()),
                self.config.model_context_window,
                self.config.agents.max_depth as u32,
            )
            .with_trust_store(self.trust_store.clone()),
        );
        let new_guardian = Arc::new(GuardianReviewer::new(target_slug));

        let mut candidate_config = self.config.clone();
        candidate_config.model = target_slug.to_string();
        if let Some(info) = m31a_provider::find_model(target_slug) {
            candidate_config.model_context_window = info.context_window as usize;
        } else if let Some(cw) =
            m31a_provider::find_canonical_model(target_slug).and_then(|canon| canon.context_window)
        {
            candidate_config.model_context_window = cw as usize;
        }
        if let Err(e) = candidate_config.save() {
            let err_msg = format!("Failed to persist model configuration for '{target_slug}': {e}");
            self.transcript
                .push(TranscriptItem::notice(err_msg.clone(), NoticeLevel::Error));
            self.sync_virtualizer();
            if self.auto_scroll {
                self.scroll_to_bottom();
            }
            anyhow::bail!("{err_msg}");
        }

        self.config = candidate_config;
        self.active_model = target_slug.to_string();
        self.bottom_pane.active_model = target_slug.to_string();
        self.bottom_pane.token_meter = Some(crate::bottom_pane::TokenContextMeter::new(
            self.tokens_used as usize,
            self.config.model_context_window,
        ));
        self.nim_client = candidate_client;
        self.tools.set_agent_runtime(new_engine_runtime.clone());
        self.engine_runtime = new_engine_runtime.clone();
        self.agent_runtime = new_engine_runtime;
        self.guardian = new_guardian;
        if let Some(ref mut ms) = self.models_screen_state {
            ms.set_active_model(target_slug.to_string());
        }
        self.refresh_system_prompt();
        self.transcript.push(TranscriptItem::notice(
            format!("Active model switched to: {}", target_slug),
            NoticeLevel::Success,
        ));
        self.sync_virtualizer();
        if self.auto_scroll {
            self.scroll_to_bottom();
        }

        Ok(target_slug.to_string())
    }

    pub fn resume_session(&mut self, thread_id: &str) -> Result<()> {
        self.save_current_session_draft();
        if let Some(thread) = self.storage.get_thread(thread_id)? {
            let api_key = self.config.api_key.clone().filter(|k| !k.trim().is_empty());
            let provider_type = ProviderType::resolve(
                self.config.model_provider.as_deref(),
                &thread.model,
                Some(&self.config.api_base),
            )?;
            let provider_config = ProviderConfig {
                provider_type,
                api_base: Some(self.config.api_base.clone()),
                api_key,
                model: thread.model.clone(),
                extra: std::collections::HashMap::new(),
            };
            m31a_provider::validate_provider_config(&provider_config)?;
            let client = ProviderBuilder::build_client(provider_config)?
                .with_reasoning_effort(self.config.reasoning_effort.clone());

            let candidate_for_probe = client.clone();
            let probe_res = std::thread::spawn(move || {
                let rt = match tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                {
                    Ok(rt) => rt,
                    Err(e) => {
                        return Err(anyhow::anyhow!("Failed to build runtime for probe: {e}"));
                    }
                };
                rt.block_on(async {
                    tokio::time::timeout(std::time::Duration::from_secs(5), async {
                        candidate_for_probe.test_model_availability().await?;
                        Ok::<(), anyhow::Error>(())
                    })
                    .await
                    .map_err(|_| anyhow::anyhow!("Connectivity probe timed out after 5s"))?
                })
            })
            .join();

            match probe_res {
                Ok(Ok(())) => {}
                Ok(Err(e)) => {
                    anyhow::bail!(
                        "Cannot resume thread with model '{}': connectivity check failed: {e}",
                        thread.model
                    );
                }
                Err(_) => {
                    anyhow::bail!(
                        "Cannot resume thread with model '{}': connectivity probe panicked",
                        thread.model
                    );
                }
            }

            self.commit_session_resume(thread_id, client, thread)
        } else {
            anyhow::bail!("Session thread '{}' not found", thread_id);
        }
    }

    /// Asynchronous, non-blocking session resume for interactive TUI operations.
    pub fn resume_session_async(&mut self, thread_id: &str) -> Result<()> {
        self.save_current_session_draft();
        if let Some(thread) = self.storage.get_thread(thread_id)? {
            let api_key = self.config.api_key.clone().filter(|k| !k.trim().is_empty());
            let provider_type = ProviderType::resolve(
                self.config.model_provider.as_deref(),
                &thread.model,
                Some(&self.config.api_base),
            )?;
            let provider_config = ProviderConfig {
                provider_type,
                api_base: Some(self.config.api_base.clone()),
                api_key,
                model: thread.model.clone(),
                extra: std::collections::HashMap::new(),
            };
            m31a_provider::validate_provider_config(&provider_config)?;
            let client = ProviderBuilder::build_client(provider_config)?
                .with_reasoning_effort(self.config.reasoning_effort.clone());

            self.provider_status_message = Some(format!("Resuming session {}...", thread_id));
            self.transcript.push(TranscriptItem::notice(
                format!(
                    "Verifying connectivity to resume session '{thread_id}' with model '{}'...",
                    thread.model
                ),
                NoticeLevel::Info,
            ));
            self.sync_virtualizer();

            let candidate_for_probe = client.clone();
            let tid = thread_id.to_string();
            let thread_record = thread.clone();
            let event_tx = self.event_tx.clone();
            let task = async move {
                let probe_res = tokio::time::timeout(std::time::Duration::from_secs(5), async {
                    candidate_for_probe.test_model_availability().await?;
                    Ok::<(), anyhow::Error>(())
                })
                .await;

                let result = match probe_res {
                    Ok(Ok(())) => Ok(()),
                    Ok(Err(e)) => Err(e.to_string()),
                    Err(_) => Err("Connectivity probe timed out after 5s".to_string()),
                };

                let _ = event_tx
                    .send(TuiEvent::SessionResumeResult {
                        thread_id: tid,
                        client: candidate_for_probe,
                        thread: thread_record,
                        result,
                    })
                    .await;
            };

            if let Ok(handle) = tokio::runtime::Handle::try_current() {
                handle.spawn(task);
            } else {
                std::thread::spawn(move || {
                    if let Ok(rt) = tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()
                    {
                        rt.block_on(task);
                    }
                });
            }

            Ok(())
        } else {
            anyhow::bail!("Session thread '{}' not found", thread_id);
        }
    }

    /// Atomically commits a validated candidate session resume to application state.
    pub fn commit_session_resume(
        &mut self,
        thread_id: &str,
        client: ProviderClient,
        thread: m31a_storage::db::ThreadRecord,
    ) -> Result<()> {
        if let Some(token) = self.cancel_token.take() {
            token.cancel();
        }
        if let Some(ref turn_id) = self.current_turn_id.take() {
            let _ = self.turn_state.transition_cancellation_requested(turn_id);
            let _ = self.turn_state.transition_cancelling(turn_id);
        }
        self.turn_state.force_idle();
        self.session_epoch += 1;
        self.current_turn_id = None;
        self.turn_start_time = None;
        self.turn_ttft_ms = None;
        self.steer_tx = None;
        self.chat_widget.turn_state = crate::chatwidget::TurnExecutionState::Idle;

        self.session_id = thread.id.clone();
        self.active_model = thread.model.clone();
        self.bottom_pane.active_model = thread.model.clone();
        self.nim_client = client;

        let new_engine_runtime = Arc::new(
            m31a_agent_engine::agents::EngineAgentRuntime::new(
                self.nim_client.clone(),
                self.tools.clone(),
                self.storage.clone(),
                self.exec_policy.clone(),
                self.cwd.clone(),
                Some(self.hooks.clone()),
                self.config.model_context_window,
                self.config.agents.max_depth as u32,
            )
            .with_trust_store(self.trust_store.clone()),
        );
        self.tools.set_agent_runtime(new_engine_runtime.clone());
        self.engine_runtime = new_engine_runtime.clone();
        self.agent_runtime = new_engine_runtime;
        if let Some(ref mut ms) = self.models_screen_state {
            ms.set_active_model(thread.model.clone());
        }
        self.guardian = Arc::new(GuardianReviewer::new(&thread.model));

        // Re-evaluate workspace trust
        let _ = self.trust_store.lock().unwrap().evaluate(&self.cwd);

        self.transcript.clear();
        self.chat_history.clear();
        self.transcript.push(TranscriptItem::notice(
            format!("Resumed session '{}' ({})", thread.title, thread.id),
            NoticeLevel::Success,
        ));

        let items = self.storage.list_items(thread_id)?;
        for (item_type, content, _id) in items {
            match item_type.as_str() {
                "user" => {
                    self.chat_history.push(ChatMessage::user(content.clone()));
                    self.transcript.push(TranscriptItem::user(content));
                }
                "assistant" => {
                    self.chat_history
                        .push(ChatMessage::assistant(Some(content.clone()), None));
                    self.transcript.push(TranscriptItem::assistant(content));
                }
                _ => {}
            }
        }
        self.refresh_system_prompt();
        self.sync_virtualizer();
        self.restore_session_draft(thread_id);
        self.current_screen = crate::screens::Screen::Chat;
        if self.auto_scroll {
            self.scroll_to_bottom();
        }

        Ok(())
    }

    pub fn restore_checkpoint_action(
        &mut self,
        checkpoint: &m31a_storage::CheckpointRecord,
    ) -> Result<String> {
        // Revert session thread in database
        self.storage.restore_checkpoint(&checkpoint.id, true)?;

        // Revert git working tree if a commit sha is associated
        let mut git_msg = String::new();
        if let Some(ref sha) = checkpoint.git_commit
            && !sha.is_empty()
        {
            match m31a_tools::git_ops::safe_revert_to_commit(&self.cwd, sha, false, true) {
                Ok(_) => {
                    git_msg = format!(
                        " | Git working tree restored to {}",
                        &sha[..sha.len().min(8)]
                    );
                }
                Err(e) => {
                    git_msg = format!(" | Git revert warning: {}", e);
                }
            }
        }

        // Reload conversation items from storage up to restored turn
        self.transcript.clear();
        self.chat_history.clear();
        self.transcript.push(TranscriptItem::notice(
            format!(
                "Restored session to checkpoint '{}' (Turn {}){}\nLabel: {}",
                checkpoint.id, checkpoint.turn_index, git_msg, checkpoint.label
            ),
            NoticeLevel::Success,
        ));

        let items = self.storage.list_items(&checkpoint.thread_id)?;
        for (item_type, content, _id) in items {
            match item_type.as_str() {
                "user" => {
                    self.chat_history.push(ChatMessage::user(content.clone()));
                    self.transcript.push(TranscriptItem::user(content));
                }
                "assistant" => {
                    self.chat_history
                        .push(ChatMessage::assistant(Some(content.clone()), None));
                    self.transcript.push(TranscriptItem::assistant(content));
                }
                _ => {}
            }
        }

        self.refresh_system_prompt();
        self.sync_virtualizer();
        self.scroll_to_bottom();

        Ok(format!("Restored to checkpoint {}", checkpoint.id))
    }

    pub fn scroll_to_top(&mut self) {
        self.chat_widget.scroll_controller.scroll_to_top();
        self.auto_scroll = self.chat_widget.scroll_controller.is_pinned_to_bottom;
        self.scroll_offset = self.chat_widget.scroll_controller.offset();
    }

    pub fn scroll_to_bottom(&mut self) {
        self.chat_widget.scroll_controller.scroll_to_bottom();
        self.auto_scroll = self.chat_widget.scroll_controller.is_pinned_to_bottom;
        self.scroll_offset = self.chat_widget.scroll_controller.offset();
    }

    pub fn scroll_up(&mut self, amount: usize) {
        let delta = self
            .chat_widget
            .scroll_controller
            .accelerate_delta(amount, self.config.tui.scroll_acceleration);
        self.chat_widget.scroll_controller.scroll_up(delta);
        self.auto_scroll = self.chat_widget.scroll_controller.is_pinned_to_bottom;
        self.scroll_offset = self.chat_widget.scroll_controller.offset();
    }

    pub fn scroll_down(&mut self, amount: usize) {
        let delta = self
            .chat_widget
            .scroll_controller
            .accelerate_delta(amount, self.config.tui.scroll_acceleration);
        self.chat_widget.scroll_controller.scroll_down(delta);
        self.auto_scroll = self.chat_widget.scroll_controller.is_pinned_to_bottom;
        self.scroll_offset = self.chat_widget.scroll_controller.offset();
    }

    pub fn poll_events(&mut self) {
        let mut processed = 0;
        while processed < 128 {
            match self.event_rx.try_recv() {
                Ok(event) => {
                    self.handle_tui_event(event);
                    processed += 1;
                }
                Err(_) => break,
            }
        }
    }

    pub fn handle_tui_event(&mut self, event: TuiEvent) {
        match event {
            TuiEvent::Correlated { correlation, event } => {
                if correlation.session_epoch != self.session_epoch {
                    tracing::debug!(
                        expected = self.session_epoch,
                        received = correlation.session_epoch,
                        "Discarded event from stale session epoch"
                    );
                    return;
                }
                if let Some(ref active_turn) = self.current_turn_id {
                    if &correlation.turn_id != active_turn {
                        tracing::debug!(
                            expected = %active_turn,
                            received = %correlation.turn_id,
                            "Discarded event from superseded turn"
                        );
                        return;
                    }
                } else {
                    // Turn has finished or cancelled; ignore non-terminal events
                    match &*event {
                        TuiEvent::TurnFailed(_) | TuiEvent::TurnComplete { .. } => {}
                        _ => {
                            tracing::debug!(
                                turn_id = %correlation.turn_id,
                                "Discarded event for concluded turn"
                            );
                            return;
                        }
                    }
                }
                self.handle_tui_event(*event);
            }
            TuiEvent::ReasoningChunk(chunk) => {
                if matches!(
                    self.turn_state.state(),
                    TurnState::CancellationRequested { .. } | TurnState::Cancelling { .. }
                ) {
                    return;
                }
                if self.turn_ttft_ms.is_none()
                    && let Some(start) = self.turn_start_time
                {
                    self.turn_ttft_ms = Some(start.elapsed().as_millis() as u64);
                }
                let turn_id = self.current_turn_id.clone().unwrap_or_default();
                let _ = self.turn_state.transition_thinking(&turn_id, &chunk);
                self.chat_widget.streaming_buffer.append_chunk(&chunk);
                if let Some(TranscriptItem::Thinking { text, .. }) = self.transcript.last_mut() {
                    text.push_str(&chunk);
                } else {
                    self.transcript.push(TranscriptItem::thinking(chunk));
                }
                self.update_status_badges();
                self.sync_virtualizer();
                if self.auto_scroll {
                    self.scroll_to_bottom();
                }
            }
            TuiEvent::ContentChunk(chunk) => {
                if matches!(
                    self.turn_state.state(),
                    TurnState::CancellationRequested { .. } | TurnState::Cancelling { .. }
                ) {
                    return;
                }
                if self.turn_ttft_ms.is_none()
                    && let Some(start) = self.turn_start_time
                {
                    self.turn_ttft_ms = Some(start.elapsed().as_millis() as u64);
                }
                let turn_id = self.current_turn_id.clone().unwrap_or_default();
                let _ = self.turn_state.transition_streaming(&turn_id);
                self.chat_widget.streaming_buffer.append_chunk(&chunk);
                if let Some(TranscriptItem::Assistant { text }) = self.transcript.last_mut() {
                    text.push_str(&chunk);
                } else {
                    self.transcript.push(TranscriptItem::assistant(chunk));
                }
                self.update_status_badges();
                self.sync_virtualizer();
                if self.auto_scroll {
                    self.scroll_to_bottom();
                }
            }
            TuiEvent::ToolCallStarted { id, name, args } => {
                let turn_id = self.current_turn_id.clone().unwrap_or_default();
                let _ = self
                    .turn_state
                    .transition_executing_tool(&turn_id, &id, &name);
                self.chat_widget.start_tool(&id, &name);
                self.transcript
                    .push(TranscriptItem::tool_call(id, name, args));
                self.sync_virtualizer();
                if self.auto_scroll {
                    self.scroll_to_bottom();
                }
            }
            TuiEvent::ToolCallFinished {
                id,
                output,
                success,
            } => {
                for item in self.transcript.iter_mut().rev() {
                    if let TranscriptItem::ToolCall(entry) = item
                        && entry.id == id
                    {
                        entry.output = Some(output.clone());
                        entry.success = Some(success);
                        entry.in_progress = false;
                        break;
                    }
                }
                self.update_status_badges();
                self.sync_virtualizer();
                if self.auto_scroll {
                    self.scroll_to_bottom();
                }
            }
            TuiEvent::ShellCommandFinished {
                id,
                output,
                success,
                duration_ms,
            } => {
                for item in self.transcript.iter_mut().rev() {
                    if let TranscriptItem::ToolCall(entry) = item
                        && entry.id == id
                    {
                        entry.output = Some(output.clone());
                        entry.success = Some(success);
                        entry.in_progress = false;
                        entry.duration_ms = Some(duration_ms);
                        break;
                    }
                }
                self.chat_history.push(ChatMessage::user(format!(
                    "Executed instant shell command. Exit status: {}\nOutput:\n```\n{}\n```",
                    if success { "0 (success)" } else { "failed" },
                    output
                )));
                self.update_status_badges();
                self.sync_virtualizer();
                if self.auto_scroll {
                    self.scroll_to_bottom();
                }
            }
            TuiEvent::Notice(msg, level) => {
                if self.chat_widget.turn_state.is_active() {
                    self.deferred_interrupts
                        .set_turn_state(self.chat_widget.turn_state.clone());
                    self.deferred_interrupts
                        .enqueue(crate::interrupts::DeferredInterrupt::Toast {
                            message: msg,
                            level,
                            queued_at: std::time::Instant::now(),
                        });
                } else {
                    self.transcript.push(TranscriptItem::notice(msg, level));
                    self.sync_virtualizer();
                    if self.auto_scroll {
                        self.scroll_to_bottom();
                    }
                }
            }
            TuiEvent::ApprovalRequested {
                approval,
                response_tx,
            } => {
                if self.approval_tx.is_some() {
                    let _ = response_tx.send(m31a_elicitation::ApprovalDecision::Deny {
                        reason: "Superseded by an already pending approval request.".to_string(),
                    });
                    self.transcript.push(TranscriptItem::notice(
                        "An approval request arrived while another was pending and was denied without being shown.",
                        NoticeLevel::Warning,
                    ));
                    self.sync_virtualizer();
                    return;
                }
                self.ring_bell();
                for item in self.deferred_interrupts.drain_pending() {
                    if let crate::interrupts::DeferredInterrupt::Toast { message, level, .. } = item
                    {
                        self.transcript.push(TranscriptItem::notice(message, level));
                    }
                }
                let turn_id = self.current_turn_id.clone().unwrap_or_default();
                let summary = approval.command.clone().unwrap_or_else(|| {
                    format!("{}: {}", approval.tool_call.function.name, approval.reason)
                });
                let _ = self.turn_state.transition_awaiting_approval(
                    &turn_id,
                    &approval.tool_call.id,
                    &summary,
                );
                let req_id = approval.tool_call.id.clone();
                self.chat_widget.await_approval(&req_id, &summary);
                self.approval_tx = Some(response_tx);
                self.push_modal(ModalState::ApprovalDialog {
                    approval,
                    selected_option: 0,
                });
            }
            TuiEvent::QuestionRequested {
                question,
                response_tx,
            } => {
                let turn_id = self.current_turn_id.clone().unwrap_or_default();
                let _ = self.turn_state.transition_awaiting_question(
                    &turn_id,
                    &question.id,
                    &question.question,
                );
                self.question_tx = Some(response_tx);
                self.push_modal(ModalState::QuestionPrompt(question));
            }
            TuiEvent::Compacted {
                original_tokens,
                compacted_tokens,
            } => {
                let turn_id = self.current_turn_id.clone().unwrap_or_default();
                let _ = self.turn_state.transition_compacting(&turn_id);
                self.transcript.push(TranscriptItem::notice(
                    format!(
                        "Inline context auto-compacted: {} -> {} tokens",
                        original_tokens, compacted_tokens
                    ),
                    NoticeLevel::Info,
                ));
                self.sync_virtualizer();
            }
            TuiEvent::TurnComplete { usage, messages } => {
                self.ring_bell();
                for item in self.deferred_interrupts.drain_pending() {
                    if let crate::interrupts::DeferredInterrupt::Toast { message, level, .. } = item
                    {
                        self.transcript.push(TranscriptItem::notice(message, level));
                    }
                }
                let turn_id = self.current_turn_id.clone().unwrap_or_default();
                let now = std::time::Instant::now();
                let duration_ms = self
                    .turn_start_time
                    .take()
                    .map(|t| (now - t).as_millis() as u64)
                    .unwrap_or(0);
                let ttft_ms = self.turn_ttft_ms.take().unwrap_or(duration_ms);

                let _ = self.turn_state.transition_completed(
                    &turn_id,
                    usage.total_tokens as usize,
                    duration_ms,
                );
                let _ = self.turn_state.transition_idle();
                self.current_turn_id = None;
                self.tokens_used += usage.total_tokens;
                self.session_metrics
                    .record_turn_metrics(m31a_analytics::TurnMetrics {
                        model: self.active_model.clone(),
                        prompt_tokens: usage.prompt_tokens,
                        completion_tokens: usage.completion_tokens,
                        reasoning_tokens: usage.reasoning_tokens,
                        ttft_ms,
                        duration_ms,
                    });
                self.steer_tx = None;
                self.cancel_token = None;
                let max_ctx = self.config.model_context_window;
                self.bottom_pane.token_meter = Some(crate::bottom_pane::TokenContextMeter::new(
                    self.tokens_used as usize,
                    max_ctx,
                ));
                self.chat_widget
                    .complete_turn(0, usage.total_tokens as usize);
                self.chat_history = messages;
                self.bottom_pane.git_status = Self::detect_git_status(&self.cwd);
                self.update_status_badges();

                // If an active goal is present, update token metrics
                if let Ok(Some(g)) = self.storage.get_goal(&self.session_id)
                    && g.status == "active"
                {
                    let _ =
                        self.storage
                            .update_goal_tokens(&self.session_id, usage.total_tokens, 0);
                }

                // Transactionally persist session state without duplicate records
                let mut items_to_save = Vec::new();
                for msg in &self.chat_history {
                    match &msg.role {
                        m31a_core::types::Role::User => {
                            if let Some(ref text) = msg.content {
                                items_to_save.push(("user".to_string(), text.clone()));
                            }
                        }
                        m31a_core::types::Role::Assistant => {
                            if let Some(ref text) = msg.content {
                                items_to_save.push(("assistant".to_string(), text.clone()));
                            }
                        }
                        _ => {}
                    }
                }
                let _ = self
                    .storage
                    .replace_thread_items(&self.session_id, &items_to_save);

                // If blurred, emit terminal OSC 9 desktop notification
                if !self.is_focused {
                    use std::io::Write;
                    print!("\x1b]9;M31A: Turn completed\x1b\\");
                    let _ = std::io::stdout().flush();
                }

                // Check queued prompts
                if let Some(next_prompt) = self.queued_prompts.pop_front() {
                    let remaining = self.queued_prompts.len();
                    self.transcript.push(TranscriptItem::notice(
                        if remaining > 0 {
                            format!("Auto-running next queued prompt ({remaining} remaining in queue)...")
                        } else {
                            "Auto-running queued prompt...".to_string()
                        },
                        NoticeLevel::Info,
                    ));
                    self.sync_virtualizer();
                    self.submit_prompt(next_prompt);
                }
            }
            TuiEvent::TurnFailed(err) => {
                self.ring_bell();
                for item in self.deferred_interrupts.drain_pending() {
                    if let crate::interrupts::DeferredInterrupt::Toast { message, level, .. } = item
                    {
                        self.transcript.push(TranscriptItem::notice(message, level));
                    }
                }
                let turn_id = self.current_turn_id.clone().unwrap_or_default();
                let _ = self.turn_state.transition_failed(&turn_id, &err);
                let _ = self.turn_state.transition_idle();
                self.current_turn_id = None;
                self.turn_start_time = None;
                self.turn_ttft_ms = None;
                self.chat_widget.fail_turn(&err);
                self.transcript.push(TranscriptItem::notice(
                    format!("Turn failed: {}", err),
                    NoticeLevel::Error,
                ));
                if !self.is_focused {
                    use std::io::Write;
                    print!("\x1b]9;M31A: Turn failed\x1b\\");
                    let _ = std::io::stdout().flush();
                }
                if !self.queued_prompts.is_empty() {
                    self.transcript.push(TranscriptItem::notice(
                        format!(
                            "Paused {} queued prompt(s) due to previous turn failure.",
                            self.queued_prompts.len()
                        ),
                        NoticeLevel::Warning,
                    ));
                }
                self.sync_virtualizer();
            }
            TuiEvent::WorkspaceChanged(batch) => {
                self.bottom_pane.git_status = Self::detect_git_status(&self.cwd);
                self.update_status_badges();
                if batch.git_head_changed {
                    self.transcript.push(TranscriptItem::notice(
                        "Git branch/HEAD updated externally — working tree status synchronized."
                            .to_string(),
                        NoticeLevel::Info,
                    ));
                }
                self.refresh_system_prompt();
                self.sync_virtualizer();
            }
            TuiEvent::ModelsDiscovered(remote_models) => {
                let mut added = 0;
                for rm in remote_models {
                    if !self.available_models.iter().any(|m| m.slug == rm.slug) {
                        self.available_models.push(rm);
                        added += 1;
                    }
                }
                if added > 0 {
                    m31a_provider::save_cached_models(&self.available_models);
                    self.transcript.push(TranscriptItem::notice(
                        format!("Discovered {} models dynamically from provider endpoint ({} total available).", added, self.available_models.len()),
                        NoticeLevel::Info,
                    ));
                    self.sync_virtualizer();
                }
            }
            TuiEvent::CodeModeResult(output) => {
                if let Some(ModalState::CodeModeScratchpad(sp)) = self.current_modal_mut() {
                    sp.is_evaluating = false;
                    sp.output_lines = output.lines().map(|s| s.to_string()).collect();
                    sp.scroll_output = 0;
                    sp.active_tab = 0;
                }
            }
            TuiEvent::ProcessListUpdated(procs) => {
                if let Some(ModalState::ProcessesModal {
                    processes,
                    selected_index,
                }) = self.current_modal_mut()
                {
                    *processes = procs;
                    *selected_index = (*selected_index).min(processes.len().saturating_sub(1));
                }
            }
            TuiEvent::ProcessOutputReady { id, label, lines } => {
                self.push_modal(ModalState::process_viewer(id, label, lines));
            }
            TuiEvent::ProviderStatus(msg) => {
                self.provider_status_message = Some(msg);
            }
            TuiEvent::SetupConnectivityResult {
                generation,
                status,
                details,
                error,
            } => {
                if let Some(ModalState::SetupWizard { state }) = self.current_modal_mut() {
                    if state.connectivity_generation == generation {
                        state.connectivity_status = status;
                        state.connectivity_details = details;
                        state.connectivity_error = error;
                    } else {
                        tracing::debug!(
                            "Ignoring stale setup connectivity result for generation {} (current: {})",
                            generation,
                            state.connectivity_generation
                        );
                    }
                }
            }
            TuiEvent::ModelHealthResult { model, state } => {
                if let Some(ModalState::ModelPicker {
                    state: picker_state,
                }) = self.current_modal_mut()
                {
                    picker_state.set_health(model, state);
                }
            }
            TuiEvent::ModelActivationResult {
                slug,
                candidate_client,
                result,
            } => {
                self.provider_status_message = None;
                match result {
                    Ok(()) => {
                        if let Err(e) = self.commit_model_switch(&slug, candidate_client) {
                            self.transcript.push(TranscriptItem::notice(
                                format!("Failed to commit model switch to '{slug}': {e}"),
                                NoticeLevel::Error,
                            ));
                        }
                    }
                    Err(e) => {
                        self.transcript.push(TranscriptItem::notice(
                            format!("Connectivity validation failed for model '{slug}': {e}"),
                            NoticeLevel::Error,
                        ));
                    }
                }
                self.sync_virtualizer();
                if self.auto_scroll {
                    self.scroll_to_bottom();
                }
            }
            TuiEvent::SessionResumeResult {
                thread_id,
                client,
                thread,
                result,
            } => {
                self.provider_status_message = None;
                match result {
                    Ok(()) => {
                        if let Err(e) = self.commit_session_resume(&thread_id, client, thread) {
                            self.transcript.push(TranscriptItem::notice(
                                format!("Failed to commit resumed session '{thread_id}': {e}"),
                                NoticeLevel::Error,
                            ));
                        }
                    }
                    Err(e) => {
                        self.transcript.push(TranscriptItem::notice(
                            format!(
                                "Cannot resume thread '{thread_id}': connectivity check failed: {e}"
                            ),
                            NoticeLevel::Error,
                        ));
                    }
                }
                self.sync_virtualizer();
                if self.auto_scroll {
                    self.scroll_to_bottom();
                }
            }
        }
    }

    fn spawn_turn(&mut self) {
        let mut engine = m31a_agent_engine::AgentEngine::new(
            self.nim_client.clone(),
            self.tools.clone(),
            self.storage.clone(),
            self.config.approval,
            self.exec_policy.clone(),
            self.config.sandbox,
            self.cwd.clone(),
        );
        // Share the app-level Guardian so the circuit breaker persists across turns.
        engine.guardian = Some(self.guardian.clone());
        engine.routing_enabled = self.config.routing_enabled;
        engine.tool_output_token_limit = self.config.tool_output_token_limit;
        engine.code_mode = self.code_mode.clone();
        engine.hooks = Some(self.hooks.clone());
        engine.collaboration_mode = self.collaboration_mode;
        engine.dangerously_bypass = self.config.dangerously_bypass;

        let messages = self.chat_history.clone();
        let event_tx = self.event_tx.clone();
        let session_id = self.session_id.clone();
        let turn_id = uuid::Uuid::new_v4().to_string();
        self.current_turn_id = Some(turn_id.clone());
        self.turn_start_time = Some(std::time::Instant::now());
        self.turn_ttft_ms = None;

        if let Err(e) = self.turn_state.transition_submitted(&turn_id) {
            tracing::warn!("Failed to transition to Submitted: {e}");
        }

        let correlation = EventCorrelation::new(&session_id, self.session_epoch, &turn_id);

        let (steer_tx, steer_rx) = mpsc::unbounded_channel();
        self.steer_tx = Some(steer_tx);
        let cancel_token = tokio_util::sync::CancellationToken::new();
        self.cancel_token = Some(cancel_token.clone());

        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            let handle_fwd = handle.clone();
            let corr = correlation.clone();
            handle.spawn(async move {
                let (eng_tx, mut eng_rx) = mpsc::unbounded_channel();
                let fwd_tx = event_tx.clone();
                let corr_fwd = corr.clone();

                let forward_handle = handle_fwd.spawn(async move {
                    let mut batcher = m31a_agent_engine::DeltaBatcher::new(50);
                    let mut interval =
                        tokio::time::interval(std::time::Duration::from_millis(50));
                    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

                    loop {
                        tokio::select! {
                            _ = interval.tick() => {
                                let (content, reasoning) = batcher.flush_all();
                                if let Some(r) = reasoning {
                                    let ev = TuiEvent::Correlated {
                                        correlation: corr_fwd.clone(),
                                        event: Box::new(TuiEvent::ReasoningChunk(r)),
                                    };
                                    if fwd_tx.send(ev).await.is_err() {
                                        break;
                                    }
                                }
                                if let Some(c) = content {
                                    let ev = TuiEvent::Correlated {
                                        correlation: corr_fwd.clone(),
                                        event: Box::new(TuiEvent::ContentChunk(c)),
                                    };
                                    if fwd_tx.send(ev).await.is_err() {
                                        break;
                                    }
                                }
                            }
                            ev_opt = eng_rx.recv() => {
                                match ev_opt {
                                    Some(ev) => {
                                        match ev {
                                            m31a_agent_engine::EngineEvent::ContentChunk(c) => {
                                                batcher.push_content(&c);
                                            }
                                            m31a_agent_engine::EngineEvent::ReasoningChunk(r) => {
                                                batcher.push_reasoning(&r);
                                            }
                                            other => {
                                                // Flush pending text/reasoning first to preserve strict content ordering
                                                let (content, reasoning) = batcher.flush_all();
                                                if let Some(r) = reasoning {
                                                    let ev = TuiEvent::Correlated {
                                                        correlation: corr_fwd.clone(),
                                                        event: Box::new(TuiEvent::ReasoningChunk(r)),
                                                    };
                                                    if fwd_tx.send(ev).await.is_err() {
                                                        break;
                                                    }
                                                }
                                                if let Some(c) = content {
                                                    let ev = TuiEvent::Correlated {
                                                        correlation: corr_fwd.clone(),
                                                        event: Box::new(TuiEvent::ContentChunk(c)),
                                                    };
                                                    if fwd_tx.send(ev).await.is_err() {
                                                        break;
                                                    }
                                                }

                                                let tui_ev = match other {
                                                    m31a_agent_engine::EngineEvent::ContentChunk(c) => {
                                                        TuiEvent::ContentChunk(c)
                                                    }
                                                    m31a_agent_engine::EngineEvent::ReasoningChunk(r) => {
                                                        TuiEvent::ReasoningChunk(r)
                                                    }
                                                    m31a_agent_engine::EngineEvent::ToolCallStarted { id, name, args } => {
                                                        TuiEvent::ToolCallStarted { id, name, args }
                                                    }
                                                    m31a_agent_engine::EngineEvent::ToolCallFinished {
                                                        id,
                                                        output,
                                                        success,
                                                    } => TuiEvent::ToolCallFinished {
                                                        id,
                                                        output,
                                                        success,
                                                    },
                                                    m31a_agent_engine::EngineEvent::GuardianDenied { rationale } => {
                                                        TuiEvent::Notice(
                                                            format!("Guardian denied tool call: {}", rationale),
                                                            NoticeLevel::Warning,
                                                        )
                                                    }
                                                    m31a_agent_engine::EngineEvent::Notice(msg, is_warn) => {
                                                        TuiEvent::Notice(
                                                            msg,
                                                            if is_warn {
                                                                NoticeLevel::Warning
                                                            } else {
                                                                NoticeLevel::Info
                                                            },
                                                        )
                                                    }
                                                    m31a_agent_engine::EngineEvent::ApprovalRequested {
                                                        approval,
                                                        response_tx,
                                                    } => TuiEvent::ApprovalRequested {
                                                        approval: PendingApproval::new(
                                                            approval.tool_call,
                                                            approval.reason,
                                                            approval.command,
                                                        ),
                                                        response_tx,
                                                    },
                                                    m31a_agent_engine::EngineEvent::QuestionRequested {
                                                        question,
                                                        response_tx,
                                                    } => TuiEvent::QuestionRequested {
                                                        question,
                                                        response_tx,
                                                    },
                                                    m31a_agent_engine::EngineEvent::Compacted {
                                                        window_id: _,
                                                        original_tokens,
                                                        compacted_tokens,
                                                    } => TuiEvent::Compacted {
                                                        original_tokens,
                                                        compacted_tokens,
                                                    },
                                                    m31a_agent_engine::EngineEvent::TurnComplete { usage, messages } => {
                                                        TuiEvent::TurnComplete { usage, messages }
                                                    }
                                                    m31a_agent_engine::EngineEvent::TurnFailed(err) => {
                                                        TuiEvent::TurnFailed(err)
                                                    }
                                                };

                                                let correlated_ev = TuiEvent::Correlated {
                                                    correlation: corr_fwd.clone(),
                                                    event: Box::new(tui_ev),
                                                };

                                                if fwd_tx.send(correlated_ev).await.is_err() {
                                                    break;
                                                }
                                            }
                                        }
                                    }
                                    None => {
                                        // Channel closed: flush any residual buffered content
                                        let (content, reasoning) = batcher.flush_all();
                                        if let Some(r) = reasoning {
                                            let _ = fwd_tx.send(TuiEvent::Correlated {
                                                correlation: corr_fwd.clone(),
                                                event: Box::new(TuiEvent::ReasoningChunk(r)),
                                            }).await;
                                        }
                                        if let Some(c) = content {
                                            let _ = fwd_tx.send(TuiEvent::Correlated {
                                                correlation: corr_fwd.clone(),
                                                event: Box::new(TuiEvent::ContentChunk(c)),
                                            }).await;
                                        }
                                        break;
                                    }
                                }
                            }
                        }
                    }
                });

                if let Err(e) = engine
                    .run_turn(&session_id, messages, eng_tx, steer_rx, cancel_token, 20)
                    .await
                {
                    let _ = event_tx
                        .send(TuiEvent::Correlated {
                            correlation: corr,
                            event: Box::new(TuiEvent::TurnFailed(e.to_string())),
                        })
                        .await;
                }

                let _ = forward_handle.await;
            });
        }
    }
}

fn extract_patch_block(text: &str) -> Option<String> {
    if let Some(start) = text.find("```diff\n") {
        let after = &text[start + 8..];
        if let Some(end) = after.find("```") {
            return Some(after[..end].to_string());
        }
    }
    if let Some(start) = text.find("--- a/") {
        return Some(text[start..].to_string());
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_restore_checkpoint_action() {
        let config = Config {
            api_key: Some("test_key".to_string()),
            ..Default::default()
        };
        let mut app = App::new(config).unwrap();

        // Create initial thread record in DB
        let session_meta = m31a_core::SessionMetadata {
            id: app.session_id.clone(),
            title: "Test Session".to_string(),
            model: "test-model".to_string(),
            cwd: ".".to_string(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            extra: std::collections::HashMap::new(),
        };
        app.storage
            .save_thread(&session_meta, "", "workspace-write", "on-request")
            .unwrap();

        // Create checkpoint
        let files = vec!["src/main.rs".to_string()];
        let cp = app
            .storage
            .create_checkpoint(&app.session_id, "Initial Milestone", None, 1, &files, None)
            .unwrap();

        assert_eq!(cp.label, "Initial Milestone");

        // Execute restore action
        let res = app.restore_checkpoint_action(&cp);
        assert!(res.is_ok());

        // Verify transcript notice was added
        let has_notice = app.transcript.iter().any(|item| {
            if let TranscriptItem::Notice { text, .. } = item {
                text.contains("Restored session to checkpoint")
            } else {
                false
            }
        });
        assert!(has_notice);
    }

    #[test]
    fn test_app_deep_runtime_wiring() {
        let config = Config {
            api_key: Some("test_key".to_string()),
            ..Default::default()
        };
        let mut app = App::new(config).unwrap();

        // 1. Check initial virtualization & bottom pane state
        assert!(
            !app.transcript_virtualizer.cells().is_empty(),
            "Transcript virtualizer should contain welcome notice"
        );
        assert_eq!(app.bottom_pane.active_model, app.active_model);

        // 2. Advance spinner tick
        let tick_before = app.chat_widget.spinner_tick;
        app.tick();
        assert_eq!(app.chat_widget.spinner_tick, (tick_before + 1) % 10);

        // 3. Receive streaming events
        app.handle_tui_event(TuiEvent::ContentChunk(
            "Hello world from M31A\n```rust\nfn main() {}\n```".to_string(),
        ));
        assert_eq!(
            app.chat_widget.streaming_buffer.content(),
            "Hello world from M31A\n```rust\nfn main() {}\n```"
        );

        // 4. Verify transcript virtualizer parsed assistant message and extracted code block
        let code_block = app.transcript_virtualizer.find_last_code_block();
        assert!(
            code_block.is_some(),
            "Transcript virtualizer should extract fenced code block"
        );
        let cb = code_block.unwrap();
        assert_eq!(cb.language.as_deref(), Some("rust"));
        assert!(cb.code.contains("fn main() {}"));

        // 5. Tool call lifecycle
        app.handle_tui_event(TuiEvent::ToolCallStarted {
            id: "call-1".to_string(),
            name: "read_file".to_string(),
            args: r#"{"path":"src/main.rs"}"#.to_string(),
        });
        assert_eq!(
            app.chat_widget.turn_state,
            crate::chatwidget::TurnExecutionState::ToolExecuting {
                tool_id: "call-1".to_string(),
                tool_name: "read_file".to_string(),
                started_at: match app.chat_widget.turn_state {
                    crate::chatwidget::TurnExecutionState::ToolExecuting { started_at, .. } =>
                        started_at,
                    _ => unreachable!(),
                },
            }
        );

        app.handle_tui_event(TuiEvent::ToolCallFinished {
            id: "call-1".to_string(),
            output: "fn main() {}".to_string(),
            success: true,
        });

        // 6. Turn completion
        app.handle_tui_event(TuiEvent::TurnComplete {
            usage: TokenUsage {
                prompt_tokens: 100,
                completion_tokens: 50,
                total_tokens: 150,
                reasoning_tokens: 0,
            },
            messages: vec![],
        });
        assert_eq!(app.tokens_used, 150);
        assert!(app.bottom_pane.token_meter.is_some());
        assert_eq!(
            app.bottom_pane.token_meter.as_ref().unwrap().used_tokens,
            150
        );
    }

    #[test]
    fn test_toggle_collaboration_mode() {
        let config = Config {
            api_key: Some("test_key".to_string()),
            ..Default::default()
        };
        let mut app = App::new(config).unwrap();
        assert_eq!(app.collaboration_mode, CollaborationMode::Default);
        assert_eq!(
            app.bottom_pane.collaboration_mode,
            CollaborationMode::Default
        );

        // Toggle to Plan mode
        app.toggle_collaboration_mode();
        assert_eq!(app.collaboration_mode, CollaborationMode::Plan);
        assert_eq!(app.bottom_pane.collaboration_mode, CollaborationMode::Plan);

        // Toggle back to Build mode
        app.toggle_collaboration_mode();
        assert_eq!(app.collaboration_mode, CollaborationMode::Default);
        assert_eq!(
            app.bottom_pane.collaboration_mode,
            CollaborationMode::Default
        );
    }

    #[test]
    fn test_execute_instant_shell_command() {
        let config = Config {
            api_key: Some("test_key".to_string()),
            ..Default::default()
        };
        let mut app = App::new(config).unwrap();
        app.trust_store
            .lock()
            .unwrap()
            .trust(&app.cwd, m31a_trust::TrustLevel::SessionOnly)
            .unwrap();

        // Submit prompt with '!' prefix
        app.submit_prompt("!echo 'm31a instant shell'".to_string());

        let has_tool_call = app.transcript.iter().any(|item| {
            if let TranscriptItem::ToolCall(entry) = item {
                entry.name == "sh"
                    && entry
                        .output
                        .as_deref()
                        .unwrap_or("")
                        .contains("m31a instant shell")
            } else {
                false
            }
        });
        assert!(
            has_tool_call,
            "Instant shell command should produce a tool call in transcript"
        );
    }

    #[test]
    fn test_tui_trust_enforcement_blocks_untrusted_and_allows_session_trust() {
        let temp = tempfile::tempdir().unwrap();
        let config = Config {
            api_key: Some("test_key".to_string()),
            ..Default::default()
        };
        let mut app = App::new(config).unwrap();
        app.cwd = temp.path().to_path_buf();
        let initial_history_len = app.chat_history.len();

        // 1. In untrusted directory, submit_prompt must be blocked with Error notice
        app.submit_prompt("hello agent".to_string());
        assert_eq!(
            app.chat_history.len(),
            initial_history_len,
            "Prompt must not be added to chat history in untrusted workspace"
        );
        let last_notice = app.transcript.last().unwrap();
        assert!(
            matches!(last_notice, TranscriptItem::Notice { level: NoticeLevel::Error, text, .. } if text.contains("Execution BLOCKED: Working directory"))
        );

        // 2. Instant shell command must also be blocked in untrusted workspace
        app.execute_instant_shell_command("ls");
        let shell_blocked = app.transcript.last().unwrap();
        assert!(
            matches!(shell_blocked, TranscriptItem::Notice { level: NoticeLevel::Error, text, .. } if text.contains("Execution BLOCKED: Working directory"))
        );

        // 3. Grant session-scoped trust via /trust session
        app.execute_slash_command("/trust session");
        assert_eq!(
            app.trust_store.lock().unwrap().evaluate(&app.cwd),
            m31a_trust::TrustDecision::SessionOnly
        );

        // 4. Submission now succeeds past trust gate
        app.submit_prompt("hello agent".to_string());
        assert_eq!(
            app.chat_history.len(),
            initial_history_len + 1,
            "Prompt should proceed after session authorization"
        );

        // 5. Denied workspace must ALWAYS be blocked even if session command was attempted
        let _ = app
            .trust_store
            .lock()
            .unwrap()
            .trust(&app.cwd, m31a_trust::TrustLevel::Denied);
        app.submit_prompt("blocked attempt".to_string());
        let denied_notice = app.transcript.last().unwrap();
        assert!(
            matches!(denied_notice, TranscriptItem::Notice { level: NoticeLevel::Error, text, .. } if text.contains("marked as Denied"))
        );
    }

    #[test]
    fn test_command_palette_filtering() {
        let items = ModalState::default_command_palette_items();
        let modal = ModalState::command_palette(items);
        if let ModalState::CommandPalette {
            query,
            items,
            filtered_indices,
            ..
        } = modal
        {
            assert!(query.is_empty());
            assert_eq!(filtered_indices.len(), items.len());
        } else {
            panic!("Expected CommandPalette variant");
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_slash_commands_execution() {
        let config = Config {
            api_key: Some("test_key".to_string()),
            ..Default::default()
        };
        let mut app = App::new(config).unwrap();
        app.trust_store
            .lock()
            .unwrap()
            .trust(&app.cwd, m31a_trust::TrustLevel::SessionOnly)
            .unwrap();

        // 1. /theme command switches theme
        app.execute_slash_command("/theme light");
        assert_eq!(app.config.tui.theme.as_deref(), Some("light"));
        app.execute_slash_command("/theme dark");
        assert_eq!(app.config.tui.theme.as_deref(), Some("dark"));

        // 2. /procs opens processes modal
        app.execute_slash_command("/procs");
        assert!(matches!(
            app.current_modal(),
            Some(ModalState::ProcessesModal { .. })
        ));
        app.pop_modal();

        // 3. /revert opens checkpoints modal or restores
        app.execute_slash_command("/revert");
        // No checkpoints yet, notice is emitted
        let has_revert_notice = app.transcript.iter().any(|item| {
            if let TranscriptItem::Notice { text, .. } = item {
                text.contains("checkpoints")
            } else {
                false
            }
        });
        assert!(has_revert_notice);

        // 4. /apply without patch shows status or usage
        app.execute_slash_command("/apply");
        let has_apply_notice = app.transcript.iter().any(|item| {
            if let TranscriptItem::Notice { text, .. } = item {
                text.contains("Working Tree Status") || text.contains("patch")
            } else {
                false
            }
        });
        assert!(has_apply_notice);

        // 5. /effort sets reasoning effort properly or opens modal
        app.execute_slash_command("/effort high");
        assert_eq!(app.config.reasoning_effort.as_deref(), Some("high"));
        assert_eq!(app.nim_client.reasoning_effort(), Some("high"));
        assert_eq!(app.bottom_pane.reasoning_effort.as_deref(), Some("high"));

        app.execute_slash_command("/effort"); // without args opens modal!
        assert!(matches!(
            app.current_modal(),
            Some(ModalState::ReasoningEffort { .. })
        ));
        app.pop_modal();

        app.execute_slash_command("/effort none"); // explicit none sets to None
        assert_eq!(app.config.reasoning_effort, None);
        assert_eq!(app.nim_client.reasoning_effort(), None);

        app.execute_slash_command("/effort low");
        assert_eq!(app.config.reasoning_effort.as_deref(), Some("low"));

        // 6. /model rejects unknown model without explicit provider and preserves active model
        let prev_model = app.active_model.clone();
        app.execute_slash_command("/model custom-org/my-new-model");
        assert_eq!(app.active_model, prev_model);
        assert!(
            !app.available_models
                .iter()
                .any(|m| m.slug == "custom-org/my-new-model")
        );
        assert!(app.provider_status_message.is_some());
    }

    #[test]
    fn test_slash_commands_trust_enforcement_and_path_traversal() {
        let temp = tempfile::tempdir().unwrap();
        let config = Config {
            api_key: Some("test_key".to_string()),
            ..Default::default()
        };
        let mut app = App::new(config).unwrap();
        app.cwd = temp.path().to_path_buf();

        // 1. Mutating slash command via handle_submit in untrusted workspace is BLOCKED
        app.composer.buffer = "/apply some.patch".to_string();
        app.handle_submit();
        let last_notice = app.transcript.last().unwrap();
        assert!(
            matches!(last_notice, TranscriptItem::Notice { level: NoticeLevel::Error, text, .. } if text.contains("Slash command '/apply' BLOCKED"))
        );

        // 2. Harmless UI slash command (/help) is ALLOWED without trust
        app.composer.buffer = "/help".to_string();
        app.handle_submit();
        assert!(app.current_modal().is_some());
        app.pop_modal();

        // 3. Grant session trust
        app.composer.buffer = "/trust session".to_string();
        app.handle_submit();
        assert_eq!(
            app.trust_store.lock().unwrap().evaluate(&app.cwd),
            m31a_trust::TrustDecision::SessionOnly
        );

        // 4. Path traversal attempt via /apply is rejected with security error
        let escape_patch = temp.path().join("../escaped.patch");
        std::fs::write(&escape_patch, "diff --git a/foo b/foo").unwrap();
        app.execute_slash_command(&format!("/apply {}", escape_patch.display()));
        let traversal_notice = app.transcript.last().unwrap();
        assert!(
            matches!(traversal_notice, TranscriptItem::Notice { level: NoticeLevel::Error, text, .. } if text.contains("escapes workspace root")),
            "Path traversal must be rejected: {:?}",
            traversal_notice
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_settings_modal_workflow() {
        let config = Config {
            api_key: Some("test_key".to_string()),
            ..Default::default()
        };
        let mut app = App::new(config).unwrap();

        // 1. Verify command palette includes settings entry
        let palette_items = ModalState::default_command_palette_items();
        let settings_item = palette_items.iter().find(|item| item.id == "settings");
        assert!(settings_item.is_some());
        assert_eq!(settings_item.unwrap().category, "Settings");

        // 2. /settings command opens SettingsModal
        app.execute_slash_command("/settings");
        assert!(matches!(
            app.current_modal(),
            Some(ModalState::SettingsModal {
                active_tab: 0,
                selected_index: 0
            })
        ));

        // 3. /config alias opens SettingsModal as well
        app.pop_modal();
        app.execute_slash_command("/config");
        assert!(matches!(
            app.current_modal(),
            Some(ModalState::SettingsModal {
                active_tab: 0,
                selected_index: 0
            })
        ));

        // 4. Tab navigates between tabs
        use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
        crate::terminal::handle_key_event(
            &mut app,
            KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE),
        );
        assert!(matches!(
            app.current_modal(),
            Some(ModalState::SettingsModal { active_tab: 1, .. })
        ));

        // 5. Jump to TUI tab via '3'
        crate::terminal::handle_key_event(
            &mut app,
            KeyEvent::new(KeyCode::Char('3'), KeyModifiers::NONE),
        );
        assert!(matches!(
            app.current_modal(),
            Some(ModalState::SettingsModal {
                active_tab: 2,
                selected_index: 0
            })
        ));

        // 6. Toggle theme via Space (first item in TUI tab is Color Theme)
        let initial_theme = app.config.tui.theme.clone();
        crate::terminal::handle_key_event(
            &mut app,
            KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE),
        );
        assert_ne!(app.config.tui.theme, initial_theme);

        // 7. Close modal with Esc
        crate::terminal::handle_key_event(
            &mut app,
            KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE),
        );
        assert!(app.current_modal().is_none());
        let has_save_notice = app.transcript.iter().any(|item| {
            if let TranscriptItem::Notice { text, .. } = item {
                text.contains("Settings saved")
            } else {
                false
            }
        });
        assert!(has_save_notice);
    }

    #[test]
    fn test_worktree_selector_modal_flow() {
        let config = Config {
            api_key: Some("test_key".to_string()),
            ..Default::default()
        };
        let mut app = App::new(config).unwrap();

        // 1. /worktree opens WorktreeSelector modal
        app.execute_slash_command("/worktree");
        assert!(matches!(
            app.current_modal(),
            Some(ModalState::WorktreeSelector { .. })
        ));

        // 2. Esc closes modal
        use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
        crate::terminal::handle_key_event(
            &mut app,
            KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE),
        );
        assert!(app.current_modal().is_none());

        // 3. Ctrl+W opens WorktreeSelector modal
        crate::terminal::handle_key_event(
            &mut app,
            KeyEvent::new(KeyCode::Char('w'), KeyModifiers::CONTROL),
        );
        assert!(matches!(
            app.current_modal(),
            Some(ModalState::WorktreeSelector { .. })
        ));
    }

    #[test]
    fn test_extended_slash_commands_and_modals() {
        let config = Config {
            api_key: Some("test_key".to_string()),
            ..Default::default()
        };
        let mut app = App::new(config).unwrap();

        // 1. /mcp opens McpHubModal
        app.execute_slash_command("/mcp");
        assert!(matches!(
            app.current_modal(),
            Some(ModalState::McpHubModal { .. })
        ));
        app.clear_modals();

        // 2. /templates opens TemplatesModal
        app.execute_slash_command("/templates");
        assert!(matches!(
            app.current_modal(),
            Some(ModalState::TemplatesModal { .. })
        ));
        app.clear_modals();

        // 3. /persona opens PersonaModal
        app.execute_slash_command("/persona");
        assert!(matches!(
            app.current_modal(),
            Some(ModalState::PersonaModal { .. })
        ));
        app.clear_modals();

        // 4. /bell toggles sound_enabled
        assert!(app.sound_enabled);
        app.execute_slash_command("/bell off");
        assert!(!app.sound_enabled);
        app.execute_slash_command("/bell on");
        assert!(app.sound_enabled);
        app.execute_slash_command("/bell");
        assert!(!app.sound_enabled);
        app.execute_slash_command("/bell");
        assert!(app.sound_enabled);

        // 5. /context without args opens ContextManagerModal
        app.execute_slash_command("/context");
        assert!(matches!(
            app.current_modal(),
            Some(ModalState::ContextManagerModal { .. })
        ));
        app.clear_modals();

        // 6. /skills opens SkillsModal
        app.execute_slash_command("/skills");
        assert!(matches!(
            app.current_modal(),
            Some(ModalState::SkillsModal { .. })
        ));
        app.clear_modals();

        // 7. /goal without args opens GoalDashboardModal
        app.execute_slash_command("/goal");
        assert!(matches!(
            app.current_modal(),
            Some(ModalState::GoalDashboardModal { .. })
        ));
        app.clear_modals();

        // 8. /agents opens SubagentsModal
        app.execute_slash_command("/agents");
        assert!(matches!(
            app.current_modal(),
            Some(ModalState::SubagentsModal { .. })
        ));
        app.clear_modals();

        // 9. Verify command palette actions open each modal
        crate::terminal::execute_palette_action(&mut app, "mcp_hub");
        assert!(matches!(
            app.current_modal(),
            Some(ModalState::McpHubModal { .. })
        ));
        app.clear_modals();

        crate::terminal::execute_palette_action(&mut app, "skills_library");
        assert!(matches!(
            app.current_modal(),
            Some(ModalState::SkillsModal { .. })
        ));
        app.clear_modals();

        crate::terminal::execute_palette_action(&mut app, "templates");
        assert!(matches!(
            app.current_modal(),
            Some(ModalState::TemplatesModal { .. })
        ));
        app.clear_modals();

        crate::terminal::execute_palette_action(&mut app, "goal_dashboard");
        assert!(matches!(
            app.current_modal(),
            Some(ModalState::GoalDashboardModal { .. })
        ));
        app.clear_modals();

        crate::terminal::execute_palette_action(&mut app, "subagents");
        assert!(matches!(
            app.current_modal(),
            Some(ModalState::SubagentsModal { .. })
        ));
        app.clear_modals();

        crate::terminal::execute_palette_action(&mut app, "context_mgr");
        assert!(matches!(
            app.current_modal(),
            Some(ModalState::ContextManagerModal { .. })
        ));
        app.clear_modals();

        crate::terminal::execute_palette_action(&mut app, "persona_picker");
        assert!(matches!(
            app.current_modal(),
            Some(ModalState::PersonaModal { .. })
        ));
        app.clear_modals();

        // 10. /scratchpad opens CodeModeScratchpad
        app.execute_slash_command("/scratchpad");
        assert!(matches!(
            app.current_modal(),
            Some(ModalState::CodeModeScratchpad(_))
        ));
        app.clear_modals();

        // 11. /rollout opens RolloutJournal
        app.execute_slash_command("/rollout");
        assert!(matches!(
            app.current_modal(),
            Some(ModalState::RolloutJournal(_))
        ));
        app.clear_modals();

        // 12. Command palette actions for scratchpad and rollout
        crate::terminal::execute_palette_action(&mut app, "scratchpad");
        assert!(matches!(
            app.current_modal(),
            Some(ModalState::CodeModeScratchpad(_))
        ));
        app.clear_modals();

        crate::terminal::execute_palette_action(&mut app, "rollout");
        assert!(matches!(
            app.current_modal(),
            Some(ModalState::RolloutJournal(_))
        ));
        app.clear_modals();
    }

    #[test]
    fn test_deferred_interrupts_buffering_and_drain() {
        let temp = tempfile::tempdir().unwrap();
        let config = Config {
            api_key: Some("test_key".to_string()),
            ..Default::default()
        };
        let mut app = App::new_test(config, temp.path()).unwrap();

        // 1. Ring bell should succeed with sound_enabled
        app.ring_bell();

        // 2. When chat widget is in active streaming state, Notice events are buffered
        app.chat_widget.turn_state = crate::chatwidget::TurnExecutionState::StreamingContent {
            started_at: std::time::Instant::now(),
            tokens: 42,
        };

        app.handle_tui_event(TuiEvent::Notice(
            "Streaming notice 1".to_string(),
            NoticeLevel::Info,
        ));
        app.handle_tui_event(TuiEvent::Notice(
            "Streaming notice 2".to_string(),
            NoticeLevel::Warning,
        ));

        assert_eq!(app.deferred_interrupts.pending_count(), 2);

        // 3. Turn complete drains deferred interrupts into transcript
        let prev_transcript_len = app.transcript.len();
        app.handle_tui_event(TuiEvent::TurnComplete {
            usage: m31a_core::types::TokenUsage {
                prompt_tokens: 10,
                completion_tokens: 20,
                reasoning_tokens: 0,
                total_tokens: 30,
            },
            messages: Vec::new(),
        });

        assert_eq!(app.deferred_interrupts.pending_count(), 0);
        assert!(app.transcript.len() >= prev_transcript_len + 2);

        let notices: Vec<_> = app
            .transcript
            .iter()
            .filter_map(|item| {
                if let TranscriptItem::Notice { text, .. } = item {
                    Some(text.as_str())
                } else {
                    None
                }
            })
            .collect();

        assert!(notices.contains(&"Streaming notice 1"));
        assert!(notices.contains(&"Streaming notice 2"));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_setup_slash_command_and_virtualizer_sync() {
        let config = Config {
            api_key: Some("test_key".to_string()),
            ..Default::default()
        };
        let mut app = App::new(config).unwrap();

        // 1. /setup without args or /setup wizard should launch SetupWizard modal
        app.execute_slash_command("/setup");
        assert!(matches!(
            app.current_modal(),
            Some(ModalState::SetupWizard { .. })
        ));
        app.pop_modal();

        app.execute_slash_command("/setup wizard");
        assert!(matches!(
            app.current_modal(),
            Some(ModalState::SetupWizard { .. })
        ));
        app.pop_modal();

        // 1b. /setup status should push configuration notice AND sync virtualizer
        let prev_lines = app.transcript_virtualizer.total_line_count(80);
        app.execute_slash_command("/setup status");
        let new_lines = app.transcript_virtualizer.total_line_count(80);
        assert!(
            new_lines > prev_lines,
            "Virtualizer line count should increase after /setup status"
        );

        let has_setup_notice = app.transcript.iter().any(|item| {
            if let TranscriptItem::Notice { text, .. } = item {
                text.contains("M31A Configuration Status") && text.contains("Active Model")
            } else {
                false
            }
        });
        assert!(
            has_setup_notice,
            "Transcript must contain configuration notice"
        );

        // 2. /setup settings should open the Settings modal
        app.execute_slash_command("/setup settings");
        assert!(matches!(
            app.current_modal(),
            Some(ModalState::SettingsModal { .. })
        ));
        app.clear_modals();

        // 3. /doctor and /status also sync virtualizer immediately
        let lines_before_doctor = app.transcript_virtualizer.total_line_count(80);
        app.execute_slash_command("/doctor");
        let lines_after_doctor = app.transcript_virtualizer.total_line_count(80);
        assert!(
            lines_after_doctor > lines_before_doctor,
            "Virtualizer must update for /doctor"
        );

        let lines_before_status = app.transcript_virtualizer.total_line_count(80);
        app.execute_slash_command("/status");
        let lines_after_status = app.transcript_virtualizer.total_line_count(80);
        assert!(
            lines_after_status > lines_before_status,
            "Virtualizer must update for /status"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_model_picker_keybindings() {
        use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

        let config = Config {
            api_key: Some("test_key".to_string()),
            ..Default::default()
        };
        let mut app = App::new(config).unwrap();

        // 1. Ctrl+M opens model picker modal
        crate::terminal::handle_key_event(
            &mut app,
            KeyEvent::new(KeyCode::Char('m'), KeyModifiers::CONTROL),
        );
        assert!(matches!(
            app.current_modal(),
            Some(ModalState::ModelPicker { .. })
        ));

        // Esc closes modal
        crate::terminal::handle_key_event(
            &mut app,
            KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE),
        );
        assert!(app.current_modal().is_none());

        // 2. Alt+M opens model picker modal (legacy terminal fallback)
        crate::terminal::handle_key_event(
            &mut app,
            KeyEvent::new(KeyCode::Char('m'), KeyModifiers::ALT),
        );
        assert!(matches!(
            app.current_modal(),
            Some(ModalState::ModelPicker { .. })
        ));

        // F2 closes/toggles modal
        crate::terminal::handle_key_event(
            &mut app,
            KeyEvent::new(KeyCode::F(2), KeyModifiers::NONE),
        );
        assert!(app.current_modal().is_none());

        // 3. F2 opens model picker modal
        crate::terminal::handle_key_event(
            &mut app,
            KeyEvent::new(KeyCode::F(2), KeyModifiers::NONE),
        );
        assert!(matches!(
            app.current_modal(),
            Some(ModalState::ModelPicker { .. })
        ));

        // Ctrl+M closes/toggles modal
        crate::terminal::handle_key_event(
            &mut app,
            KeyEvent::new(KeyCode::Char('m'), KeyModifiers::CONTROL),
        );
        assert!(app.current_modal().is_none());

        // 4. Ctrl+Enter does NOT open model picker (reserved strictly for prompt submission/queuing)
        crate::terminal::handle_key_event(
            &mut app,
            KeyEvent::new(KeyCode::Enter, KeyModifiers::CONTROL),
        );
        assert!(app.current_modal().is_none());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_esc_turn_scrubber_and_backtrack() {
        use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

        let config = Config {
            api_key: Some("test_key".to_string()),
            ..Default::default()
        };
        let mut app = App::new(config).unwrap();

        // Simulate 2 previous conversation turns
        app.chat_history
            .push(ChatMessage::user("Turn 1 user request"));
        app.chat_history.push(ChatMessage::assistant(
            Some("Turn 1 assistant answer".to_string()),
            None,
        ));
        app.chat_history
            .push(ChatMessage::user("Turn 2 user request"));
        app.chat_history.push(ChatMessage::assistant(
            Some("Turn 2 assistant answer".to_string()),
            None,
        ));

        // Pressing Esc when composer buffer is empty opens TurnScrubber
        assert!(app.composer.is_empty());
        crate::terminal::handle_key_event(
            &mut app,
            KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE),
        );
        assert!(matches!(
            app.current_modal(),
            Some(ModalState::TurnScrubber { .. })
        ));

        // Pressing Enter on the first turn in scrubber (most recent, Turn 2) backtracks
        crate::terminal::handle_key_event(
            &mut app,
            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
        );
        assert!(app.current_modal().is_none());
        assert_eq!(app.composer.buffer, "Turn 2 user request");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_subagent_tabs_and_alt_arrows() {
        use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

        let config = Config {
            api_key: Some("test_key".to_string()),
            ..Default::default()
        };
        let mut app = App::new(config).unwrap();

        // Add subagents to tabs
        app.bottom_pane.subagent_tabs = vec![
            crate::bottom_pane::SubagentTab {
                thread_id: "agent-1".to_string(),
                role: "Core Engine Analyst".to_string(),
                status: crate::bottom_pane::SubagentStatus::Thinking,
            },
            crate::bottom_pane::SubagentTab {
                thread_id: "agent-2".to_string(),
                role: "Tools Analyst".to_string(),
                status: crate::bottom_pane::SubagentStatus::RunningTool,
            },
        ];

        // Initially on main session
        assert_eq!(app.bottom_pane.active_subagent_tab, 0);
        assert!(app.active_thread_id.is_none());

        // Alt+Right switches to agent-1
        crate::terminal::handle_key_event(
            &mut app,
            KeyEvent::new(KeyCode::Right, KeyModifiers::ALT),
        );
        assert_eq!(app.bottom_pane.active_subagent_tab, 1);
        assert_eq!(app.active_thread_id.as_deref(), Some("agent-1"));

        // Alt+Right switches to agent-2
        crate::terminal::handle_key_event(
            &mut app,
            KeyEvent::new(KeyCode::Right, KeyModifiers::ALT),
        );
        assert_eq!(app.bottom_pane.active_subagent_tab, 2);
        assert_eq!(app.active_thread_id.as_deref(), Some("agent-2"));

        // Alt+Right cycles back to main
        crate::terminal::handle_key_event(
            &mut app,
            KeyEvent::new(KeyCode::Right, KeyModifiers::ALT),
        );
        assert_eq!(app.bottom_pane.active_subagent_tab, 0);
        assert!(app.active_thread_id.is_none());

        // Alt+Left cycles backwards to agent-2
        crate::terminal::handle_key_event(
            &mut app,
            KeyEvent::new(KeyCode::Left, KeyModifiers::ALT),
        );
        assert_eq!(app.bottom_pane.active_subagent_tab, 2);
        assert_eq!(app.active_thread_id.as_deref(), Some("agent-2"));
    }

    #[test]
    fn test_streaming_metrics_tracking() {
        let mut buf = crate::chatwidget::StreamingTokenBuffer::new();
        buf.mark_turn_start();
        assert!(buf.is_empty());
        assert_eq!(buf.token_count(), 0);

        buf.append_chunk("Hello world, this is a streaming test.");
        assert!(!buf.is_empty());
        assert!(buf.token_count() > 0);
        assert!(buf.ttft_ms().is_some());

        let snap = buf.snapshot();
        assert!(snap.total_tokens > 0);
    }
}
