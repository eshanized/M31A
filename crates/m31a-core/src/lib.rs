//! `m31a-core`: Foundational abstractions, configuration models, and runtime primitives for M31A.
//!
//! Defines core session structures, chat message types, configuration loading, cryptographic vaults,
//! prompt engineering builders, context compactors, and identity management.

pub mod agent_identity;
pub mod agent_roles;
pub mod agents_md;
pub mod analytics;
pub mod arg0;
pub mod async_utils;
pub mod attachments;
pub mod aws_auth;
pub mod baseline;
pub mod capability_discovery;
pub mod cloud_config;
pub mod cloud_task;
pub mod collaboration_templates;
pub mod config;
pub mod config_schema;
pub mod connector_merge;
pub mod connectors;
pub mod context_compactor;
pub mod continuous_learning;
pub mod cost_accounting;
pub mod critical_path;
pub mod diagnostics;
pub mod elicitation;
pub mod environment_selection;
pub mod execution_lock;
pub mod external_migration;
pub mod features;
pub mod feedback;
pub mod file_system_bounds;
pub mod file_watcher;
pub mod fragments;
pub mod git_attribution;
pub mod goal_steering;
pub mod head_tail_buffer;
pub mod hooks;
pub mod install_context;
pub mod instructions;
pub mod json_to_toml;
pub mod keyring_store;
pub mod lru_cache;
pub mod memories;
pub mod message_board_client;
pub mod message_history;
pub mod otel_trace;
pub mod otel_trace_websocket;
pub mod path_utils;
pub mod plugin_bundle;
pub mod plugins;
pub mod process_hardening;
pub mod prompts;
pub mod queue_service;
pub mod realtime_conversation;
pub mod realtime_voice;
pub mod redacted_string;
pub mod response_debug;
pub mod response_history;
pub mod responses_proxy;
pub mod rollout_budget;
pub mod rollout_trace;
pub mod runtime_agent_instance;
pub mod sanitizer;
pub mod secrets;
pub mod self_healing;
pub mod session;
pub mod shell_command;
pub mod shell_snapshot;
pub mod skills;
pub mod sleep_inhibitor;
pub mod stream_parser;
pub mod swarm;
pub mod symbol_index;
pub mod template;
pub mod terminal_detection;
pub mod thread_manager;
pub mod tool_timing;
pub mod turn_diff;
pub mod turn_metadata;
pub mod types;
pub mod uds;
pub mod user_verification;
pub mod watcher;
pub mod websocket_auth;
pub mod workflow;
pub mod worktree;
pub mod world_state;

pub use agent_identity::AgentIdentity;
pub use agent_roles::{AgentRole, AgentRoleRegistry};
pub use agents_md::{
    AgentsMdEntry, AgentsMdManager, DEFAULT_AGENTS_MD_FILENAME, LOCAL_AGENTS_MD_FILENAME,
    LoadedAgentsMd,
};
pub use analytics::{
    AcceptedLineCounts, AnalyticsReducer, CommandExecutionFact, ToolCallFact, ToolExecutionStatus,
    TurnMetricsFact, accepted_line_counts_from_unified_diff, canonicalize_git_remote,
    fingerprint_hash, repo_hash_from_remote,
};
pub use arg0::{
    APPLY_PATCH_ARG0, Arg0DispatchPaths, Arg0PathEntryGuard, LINUX_SANDBOX_ARG0,
    is_apply_patch_invocation, is_linux_sandbox_invocation, setup_arg0_dispatch,
};
pub use async_utils::{
    CancelErr, CancellationToken, OrCancelExt, THREAD_STACK_SIZE_BYTES, backoff,
    spawn_bounded_thread,
};
pub use attachments::{Attachment, AttachmentStore, MAX_ATTACHMENT_SIZE_BYTES};
pub use aws_auth::{
    AwsAccessKeys, AwsAuthConfig, AwsProfile, compute_sigv4_signature, derive_sigv4_signing_key,
    discover_aws_profiles,
};
pub use baseline::{
    BaselineDiff, BaselineSnapshot, FileStatusKind, SessionReconciliationReport,
    WorkingTreeBaseline,
};
pub use capability_discovery::{
    DiscoveredCapabilityFiles, DiscoveredTool, HostCapabilityReport, WorkspaceProjectSignatures,
    discover_host_capabilities,
};
pub use cloud_config::{
    CLOUD_CONFIG_BUNDLE_TIMEOUT, CloudConfigBundle, CloudConfigCacheEntry, CloudConfigFragment,
    CloudConfigService, CloudConfigTomlBundle, CloudRequirementsFragment,
    CloudRequirementsTomlBundle, DEFAULT_CACHE_TTL as DEFAULT_CLOUD_CONFIG_CACHE_TTL,
};
pub use cloud_task::{
    CloudTaskManager, CloudTaskOutput, CloudTaskStatus, ExecutionBackendKind, PersistedTaskRecord,
};
pub use collaboration_templates::{
    CollaborationContext, CollaborationModeKind, CollaborationTemplateEngine,
};
pub use config::{
    AgentsConfig, ApprovalPolicy, Config, McpServerConfig, ResourceGovernance, SandboxPolicy,
    resolve_stored_api_key,
};
pub use config_schema::{generate_config_schema, validate_config_structure, write_config_schema};
pub use connector_merge::{
    AppInfo, connector_install_url, filter_connectors, merge_connectors, merge_plugin_connectors,
    sort_connectors_by_accessibility_and_name,
};
pub use connectors::{
    AppBranding, AppMetadata, AppReview, AppScreenshot, AppToolPolicy, CONNECTORS_CACHE_TTL,
    ConnectorDirectoryCache, ConnectorMetadata, ConnectorToolSummary,
};
pub use context_compactor::{
    CompactTokenBudget, CompactedHistory, ContextCompactor, digest_tool_output,
};
pub use continuous_learning::{
    ContinuousLearningEngine, DpoPreferencePair, FewShotExemplar, VerificationRecord,
};
pub use cost_accounting::{
    BudgetEnforcementAction, CostBudgetPolicy, ModelPricing, ModelUsageBreakdown,
    ThreadCostTracker, TurnCostRecord,
};
pub use critical_path::{
    CausalCompletedWork, CausalCriticalPath, CausalPathDurations, CausalToolBatchTiming,
    CausalWorkKind,
};
pub use diagnostics::{
    ACTIVE_BACKGROUND_PROCESSES, ACTIVE_MCP_SERVERS, ACTIVE_SUBAGENTS, Gauge, GaugeGuard,
    GaugeSnapshot, STORAGE_READS, STORAGE_WRITES, TOOL_CALLS_EXECUTED, TOTAL_COMPLETION_TOKENS,
    TOTAL_PROMPT_TOKENS, render_diagnostics_report, snapshot_all,
};
pub use elicitation::{
    ElicitationKind, ElicitationPayload, ElicitationRequest, ElicitationResponse,
    ElicitationValidationError, validate_elicitation_response,
};
pub use environment_selection::{
    NetworkDomainPermissionEntry, NetworkDomainPermissions, NetworkUnixSocketPermissions,
    SelectedEnvironment, ToolchainInfo, ToolchainKind, detect_workspace_toolchains,
    select_environment,
};
pub use execution_lock::{
    ExecutionLockError, ExecutionLockManager, LockEntry, LockMode, PathLockGuard,
};
pub use external_migration::{
    ExternalAgentMigrator, ExternalAgentSource, MigrationCandidate, MigrationItemKind,
    MigrationSummary,
};
pub use features::{FeatureKey, FeatureRegistry, FeatureStage, unstable_features_warning_event};
pub use feedback::{FeedbackAttachment, FeedbackBundleManager, FeedbackLimits, FeedbackReport};
pub use file_system_bounds::{
    COMMON_PROJECT_MARKERS, FILE_READ_CHUNK_SIZE, FILE_WRITE_CHUNK_SIZE, FindUpErrorPolicy,
    LocalLogsDir, MAX_WALK_DEPTH, MAX_WALK_DIRECTORIES, MAX_WALK_ENTRIES, MAX_WALK_RESPONSE_BYTES,
    find_nearest_ancestor_with_markers, is_nfs, read_file_stream_bounded,
};
pub use file_watcher::{
    FileChangeKind, FileWatcherBatch, WatcherIgnoreFilter, WorkspaceSnapshotTracker,
};
pub use fragments::{
    AnsweredQuestionFragment, ContextFragment, ContextFragmentManager, ContextualUserFragment,
    RECAP_PROMPT_PREFIX, RecapPrompt, RecapResponse, RenderedFragment,
};
pub use git_attribution::{
    DEFAULT_PR_MARKER, DEFAULT_TRAILER, DISABLED_ATTRIBUTION_PROMPT, ENABLED_ATTRIBUTION_PROMPT,
    GitAttributionPolicy, GitAttributionState,
    POLICY_RETRY_DELAY as GIT_ATTRIBUTION_POLICY_RETRY_DELAY,
    format_commit_message_with_attribution, format_pr_body_with_attribution,
    render_git_attribution_fragment,
};
pub use goal_steering::{
    GoalBudgetStatus, evaluate_goal_budget, render_goal_budget_warning, render_goal_continuation,
    render_goal_objective_updated,
};
pub use head_tail_buffer::{
    DynamicHeadTailBuffer, HeadTailBuffer, TruncationPolicy, approx_token_count,
    format_output_omission_marker, truncate_text_with_policy, truncate_tool_output,
};
pub use hooks::{HookDefinition, HookEngine, HookEvent, HookOutcome};
pub use install_context::{BuildInfo, CURRENT_BUILD, InstallContext, InstallMethod, PackageLayout};
pub use instructions::{HierarchicalInstructions, InstructionEntry, InstructionSource};
pub use json_to_toml::{json_str_to_toml_str, json_to_toml};
pub use keyring_store::{
    CredentialStoreError, FileFallbackKeyringStore, KeyringStore, MemoryKeyringStore,
};
pub use lru_cache::{AsyncLruCache, LruStats};
pub use memories::{
    EmbeddingGenerator, HttpEmbeddingGenerator, MemoryItem, MemoryManager, MemoryRecord,
    MockEmbeddingGenerator, RetrievalMethod, ScoredMemoryItem, calculate_cosine_similarity,
    normalize_vector,
};
pub use message_board_client::{
    BoardNotification, BoardPostInfo, BoardThreadSummary, ChannelInfo, RemoteAgentMessageBoard,
};
pub use message_history::{
    DEFAULT_HISTORY_FILENAME, DEFAULT_HISTORY_MAX_BYTES, HISTORY_SOFT_CAP_RATIO, HistoryBatch,
    HistoryBatchEntry, HistoryEntry, append_history, lookup_history_batch, trim_history_if_needed,
};
pub use otel_trace::{SpanTimer, TraceContext, W3C_TRACEPARENT_HEADER};
pub use otel_trace_websocket::{
    MAX_TRACE_WEBSOCKET_FRAME_BYTES, TRACE_CHANNEL_CAPACITY, TraceWebSocket,
};
pub use path_utils::{
    SymlinkWritePaths, atomic_write_file, paths_match_after_normalization,
    replace_path_and_deduplicate, resolve_symlink_write_paths, symlink_metadata_if_exists,
};
pub use plugin_bundle::PluginBundle;
pub use plugins::{InstalledPlugin, PluginManager, PluginManifest};
pub use process_hardening::{
    DANGEROUS_ENV_PREFIXES, apply_process_hardening, scrub_dangerous_env_vars,
};
pub use prompts::{
    CanonicalPromptAssembler, CollaborationMode, DynamicContextItem, InstructionResolver,
    ModelCapabilitiesSnapshot, ModelFamily, PromptLifecycleManager, PromptRuntimeSnapshot,
    SystemPromptBuilder,
};
pub use queue_service::{
    MAX_QUEUE_ITEMS, MAX_USER_INPUT_TEXT_CHARS, QueueService, QueueServiceError, QueuedItem,
};
pub use realtime_conversation::{
    ConversationAction, ConversationState, RealtimeConversationCoordinator, RealtimeEvent,
};
pub use realtime_voice::{
    AudioControls, AudioDevice, AudioDeviceKind, AudioDeviceManager, AudioJitterBuffer, AudioState,
    AudioStreamHandle, MAX_SDP_SIZE_BYTES, SessionDescription, VoiceHostControlMessage,
    VoiceSessionCoordinator, VoiceSessionStatus,
};
pub use redacted_string::RedactedString;
pub use response_debug::{
    ResponseDebugContext, TransportErrorCategory, classify_http_response,
    extract_response_debug_context, remediation_guidance, telemetry_message,
};
pub use response_history::{
    FunctionCallError, ResponseHistoryItem, retain_tail_from_last_n_user_messages,
    truncate_assistant_output_text_to_token_budget,
};
pub use responses_proxy::{
    LockedCredential, ResponsesProxyConfig, ResponsesProxyHandle, ResponsesProxyServer, ServerInfo,
};
pub use rollout_budget::{RateLimitDecision, RolloutBudgetPolicy, SlidingWindowRateLimiter};
pub use rollout_trace::{
    RawTraceEvent, RolloutSummary, TraceEventPayload, TraceWriter, replay_trace,
};
pub use runtime_agent_instance::{
    DEFAULT_REGISTRATION_URL, MAX_BODY_BYTES as MAX_REGISTRATION_BODY_BYTES,
    RuntimeRegistrationClient, RuntimeRegistrationRequest, RuntimeRegistrationResponse,
};
pub use sanitizer::redact_secrets;
pub use secrets::{
    LocalEncryptedVault, LocalSecretsNamespace, SecretEntry, SecretScope, SecretsBackend,
};
pub use self_healing::{
    Diagnostic, DiagnosticSeverity, SelfHealingReport, build_self_healing_prompt,
    generate_self_healing_report, parse_compiler_diagnostics,
};
pub use session::Session;
pub use shell_command::{
    DangerousCommandMatch, DetectedShell, ShellType, dangerous_command_match,
    detect_current_user_shell, detect_shell_type, get_user_home_path, get_user_shell_path,
};
pub use shell_snapshot::{
    ESSENTIAL_TOOLCHAIN_VARS, HermeticEnvironmentExporter, ShellSnapshot, ShellSnapshotCache,
};
pub use skills::{
    SkillDependencies, SkillInfo, SkillInterface, SkillLoader, SkillPolicy, SkillScope,
    ToolMentions, extract_tool_mentions,
};
pub use sleep_inhibitor::{SleepInhibitionGuard, SleepInhibitor, SleepInhibitorBackend};
pub use stream_parser::{
    ExtractedInlineTag, InlineHiddenTagParser, InlineTagSpec, PlainTextStreamParser,
    StreamTextChunk, StreamTextParser, Utf8StreamParser, Utf8StreamParserError,
};
pub use swarm::{
    AdversarialChallenge, AdversarialGauntletReport, SwarmBlackboard, SwarmCoordinator,
    SwarmMessage, SwarmPhase, SwarmRoleKind, SwarmTaskStep,
};
pub use symbol_index::{
    FileOutline, SymbolIndex, SymbolItem, SymbolKind, SymbolReference, SymbolVisibility,
    parse_symbols_from_source,
};
pub use template::{Template, TemplateError, TemplateParseError, TemplateRenderError};
pub use terminal_detection::{ColorDepth, Multiplexer, TerminalInfo, TerminalName};
pub use thread_manager::{
    AgentTreeShutdownReport, ThreadId, ThreadLifecycleStatus, ThreadManager, ThreadShutdownReport,
    ThreadSpawnOptions, TurnId, TurnInput, TurnSubmissionOutcome,
};
pub use tool_timing::{
    CompletedWork, CriticalPathReport, DirectToolTimings, ToolTimingGuard, WorkInterval, WorkKind,
};
pub use turn_diff::{DiffStats, TrackedPath, TurnDiffTracker};
pub use turn_metadata::{ModelRoutingDecision, TurnCacheStatus, TurnExecutionTiming, TurnMetadata};
pub use types::{
    ChatMessage, FunctionCall, FunctionDefinition, Role, SessionMetadata, TokenUsage, ToolCall,
    ToolDefinition,
};
pub use uds::{
    SOCKET_DIR_MODE, bind_unix_listener, default_daemon_socket_path, is_stale_socket_path,
    prepare_private_socket_directory, shared_daemon_socket_directory,
    unlink_stale_socket_if_needed,
};
pub use user_verification::{
    LocalUserVerificationProvider, UserVerificationKeyCreation, UserVerificationKeyInfo,
    UserVerificationProof, UserVerificationRequest, UserVerificationStatus,
};
pub use watcher::{FileWatcherEvent, WatchPath, WorkspaceWatcher};
pub use websocket_auth::{WebsocketAuthCliMode, WebsocketAuthPolicy, constant_time_eq_32};
pub use workflow::{
    WorkflowApprovalRequest, WorkflowApprovalStatus, WorkflowArtifact, WorkflowEvent,
    WorkflowEventType, WorkflowPlan, WorkflowRun, WorkflowState, WorkflowTask, WorkflowTaskStatus,
    WorkspaceIsolationStrategy,
};
pub use worktree::{ManagedWorktree, WorktreeManager};
