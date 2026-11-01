//! `m31a-tools`: Tool registry, execution dispatchers, and external integrations.
//!
//! Houses built-in tools for filesystem operations, git repository management, code search,
//! patch application, and provides bridges to external Model Context Protocol (MCP) servers.

pub mod agent_runtime;
pub mod browser;
pub mod catalog_selector;
pub mod debugger;
pub mod dispatch_trace;
pub mod edit;
pub mod git_merge;
pub mod git_ops;
pub mod harness;
pub mod image_detail;
pub mod image_gen;
pub mod input_repair;
pub mod mcp;
pub mod multi_env_copy;
pub mod namespace;
pub mod output_schema;
pub mod parallel_dispatcher;
pub mod patch;
pub mod plugin_install;
pub mod pr;
pub mod question;
pub mod registry;
pub mod schedule;
pub mod schema_compaction;
pub mod search;
pub mod todo;
pub mod vibe_tools;
pub mod web_search;

pub use agent_runtime::{
    AgentRuntime, AgentSpawnRequest, AgentWaitOutcome, LiveAgentInfo, SpawnedAgent,
};
pub use catalog_selector::{
    CatalogSelector, CatalogSelectorConfig, CatalogToolEntry, ToolDomainCategory,
};
pub use dispatch_trace::{
    ToolDispatchRecord, ToolDispatchStatus, ToolDispatchTracer, ToolTurnSummary,
};
pub use harness::{ToolExecutionHarness, ToolHarnessConfig};
pub use parallel_dispatcher::{
    BoxedToolFuture, ParallelDispatcher, ParallelDispatcherConfig, ToolCallRequest, ToolCallResult,
    ToolConcurrencyCategory, ToolExecutionHandler,
};

pub use image_detail::{
    ImageDetail, ModelVisionCapabilities, normalize_output_image_detail,
    sanitize_image_detail_in_json,
};
pub use input_repair::{clean_raw_string, repair_json_value, repair_tool_args};
pub use namespace::{
    IndirectNamespacePrefixConflict, IndirectNamespacePrefixes, ToolName, ToolNamespaceDescription,
};
pub use output_schema::{OutputValidationError, ToolOutputSchema};

pub use browser::{
    ClickArgs, EvaluateArgs, NavigateArgs, ScreenshotArgs, TypeArgs, execute_browser_click,
    execute_browser_console, execute_browser_evaluate, execute_browser_navigate,
    execute_browser_network, execute_browser_screenshot, execute_browser_snapshot,
    execute_browser_type,
};
pub use debugger::{
    Breakpoint, DapClient, DebugBreakpointArgs, DebugBreakpointResult, DebugInspectArgs,
    DebugInspectResult, DebugLaunchArgs, DebugLaunchResult, DebugSessionStatus, DebugStepArgs,
    DebugStepResult, DebugVariable, StackFrame, execute_debug_breakpoint, execute_debug_inspect,
    execute_debug_launch, execute_debug_step,
};
pub use edit::{EditArgs, EditResult, execute_edit};
pub use git_merge::{ConflictStrategy, GitConflictReconciler, MergeConflictResult};
pub use git_ops::{
    GitFileChange, GitFileStatusKind, GitRepoStatus, GitToolingError, branch_ahead_behind,
    ensure_git_repository, find_git_repo_root, get_git_diff, get_repository_status,
    resolve_current_branch, resolve_head, safe_revert_to_commit, verify_repo_trust,
};
pub use image_gen::{
    GENERATED_IMAGE_ARTIFACTS_DIR, ImageGenerationRequest, ImageGenerationResult,
    execute_image_generation, image_generation_artifact_path, image_generation_output_hint,
};
pub use mcp::{MCP_PREFIX, McpClient, McpManager};
pub use multi_env_copy::{
    MULTI_ENV_COPY_DESCRIPTION, MULTI_ENV_COPY_TOOL_NAME, MultiEnvCopyArgs, MultiEnvCopyOutput,
    execute_multi_env_copy,
};
pub use patch::{ApplyPatchArgs, PatchResult, apply_patch_to_workspace, parse_patch};
pub use plugin_install::{
    DiscoverableTool, DiscoverableToolAction, DiscoverableToolType, RequestPluginInstallArgs,
    RequestPluginInstallResult, handle_request_plugin_install, list_discoverable_tools,
};
pub use pr::{IssueDetails, PullRequestManager, PullRequestResult};
pub use question::{
    QuestionAnswer, QuestionArgs, QuestionItem, QuestionModalHandler, QuestionResult,
    execute_question, register_question_modal_handler,
};
pub use registry::ToolRegistry;
pub use schedule::{
    CronExpression, PromptDeliveryCallback, ScheduleArgs, ScheduleResult, ScheduledTaskInfo,
    cancel_scheduled_task, execute_schedule, list_scheduled_tasks,
    register_prompt_delivery_callback,
};
pub use schema_compaction::{
    DEFAULT_COMPACT_TOOL_SCHEMA_BYTES, MAX_COMPACT_TOOL_SCHEMA_DEPTH, compact_large_tool_schema,
    schema_byte_len,
};
pub use search::{FileSearchEngine, SearchEntryType, SearchResult};
pub use todo::{
    TodoArgs, TodoItem, TodoResult, TodoStatus, execute_todowrite, execute_todowrite_scoped,
    get_current_todos, get_session_todos, get_todo_badge, get_todo_badge_for_session,
    summarize_todos,
};
pub use web_search::{
    WebSearchMode, WebSearchQuery, WebSearchResponse, WebSearchResultItem, build_proxied_client,
    execute_web_search, execute_web_search_with_mode,
};
