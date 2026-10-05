//! Strongly typed UI and Application Actions for developer interaction (PRD §01, CLI-01, CLI-04).
//!
//! "Input -> Interaction Parser -> Typed Application Action -> Application Command -> Runtime -> Events / State -> CLI/TUI projection."

use serde::{Deserialize, Serialize};

use crate::interaction::mentions::{MentionReference, ParsedUserMessage};
use crate::tui::approval::ApprovalDecision;

/// High-level interaction intent classified deterministically from user input.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum InteractionIntent {
    /// Slash command invocation (e.g. /help, /status, /diff, /commit).
    SlashCommand { name: String, args: Vec<String> },
    /// Direct operator response to an outstanding approval request (e.g. y, n, cancel).
    Approval {
        approved: bool,
        reason: Option<String>,
    },
    /// Inspection of runtime or repository state (e.g. diff, status, log).
    Inspection { target: String },
    /// Request to commit verified changes.
    Commit { message: Option<String> },
    /// Follow-up turn continuing an active mission or session.
    FollowUp { text: String },
    /// New autonomous mission request.
    NewMission { prompt: String },
    /// Conversational inquiry, greeting, or question without autonomous mission execution.
    Conversation { text: String },
    /// Request to cancel currently active in-flight operation.
    Cancellation,
    /// Session management action (clear, resume, exit).
    SessionControl { action: String },
}

/// Typed application action emitted by the interaction parser for consumption by CLI or TUI.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ApplicationAction {
    /// User submitted a natural-language message containing resolved mentions and text segments.
    UserTextSubmitted(ParsedUserMessage),

    /// User invoked a slash command with parsed arguments.
    SlashCommandSubmitted { command: String, args: Vec<String> },

    /// A file or directory mention reference was parsed and resolved.
    MentionResolved(MentionReference),

    /// Operator submitted a resolution decision for an active approval modal or prompt.
    ApprovalDecision {
        request_id: String,
        decision: ApprovalDecision,
    },

    /// Operator requested cooperative cancellation of in-flight work.
    CancelRequested,

    /// Operator requested resuming an existing durable session by ID.
    SessionResumeRequested { session_id: String },

    /// Operator requested visual inspection of git/worktree diff.
    DiffRequested,

    /// Operator requested committing verified changes with trailers.
    CommitRequested { message: Option<String> },

    /// Operator requested comprehensive session and runtime status.
    StatusRequested,

    /// Operator requested clearing active input/composer state.
    ClearRequested,

    /// Operator requested clearing durable session history.
    ClearSessionRequested,

    /// Operator requested switching the active LLM model for this session.
    ModelChangeRequested { model: String },

    /// Operator requested switching the active model provider for this session.
    ProviderChangeRequested { provider: String },

    /// Operator requested switching the active autonomy profile for this session.
    ProfileChangeRequested { profile: String },

    /// Operator requested pausing the active mission.
    MissionPauseRequested { mission_id: String },

    /// Operator requested resuming a paused mission.
    MissionResumeRequested { mission_id: String },

    /// Operator requested applying a session-scoped configuration override.
    ConfigOverrideRequested { key: String, value: String },

    /// Operator requested launching a Project Genesis intake and planning workflow.
    GenesisRequested {
        prompt: String,
        mode: Option<String>,
    },

    /// Operator requested inspecting current Project Genesis planning state.
    GenesisStateRequested,

    /// Operator requested resuming an existing Genesis workflow run.
    WorkflowResumeRequested { run_id: String },

    /// Operator submitted a step approval decision for an active workflow run.
    WorkflowApprovalSubmitted {
        run_id: String,
        step_key: String,
        approved: bool,
        reason: Option<String>,
    },

    /// Operator requested pausing a running workflow (Running -> Blocked).
    WorkflowPauseRequested { run_id: String, reason: String },

    /// Operator requested cancelling a workflow run (non-terminal -> Cancelled).
    WorkflowCancelRequested { run_id: String, reason: String },

    /// Operator requested a read-only inspection snapshot of a workflow run.
    WorkflowInspectRequested { run_id: String },

    /// Operator requested terminating the interactive session.
    ExitRequested,

    // Human-governed lifecycle and pre-execution actions.
    /// Operator requested initializing intent discovery with a prompt.
    IntentInitRequested {
        session_id: Option<String>,
        raw_prompt: String,
    },

    /// Operator submitted an answer to a discovery question.
    QuestionAnswerSubmitted {
        session_id: Option<String>,
        question_id: String,
        answer: String,
    },

    /// Operator submitted a manual edit to the candidate plan (plan.json).
    PlanEditRequested {
        session_id: Option<String>,
        plan_json: String,
    },

    /// Operator requested a model revision of the plan with feedback.
    PlanRevisionRequested {
        session_id: Option<String>,
        feedback: String,
    },

    /// Operator requested explicit regeneration of the candidate plan.
    PlanRegenerateRequested { session_id: Option<String> },

    /// Operator explicitly accepted the candidate plan.
    PlanAcceptRequested { session_id: Option<String> },

    /// Operator explicitly rejected the candidate plan.
    PlanRejectRequested {
        session_id: Option<String>,
        reason: String,
    },

    /// Operator submitted an edit for a specific candidate task.
    TaskEditRequested {
        session_id: Option<String>,
        task_json: String,
    },

    /// Operator submitted a new candidate task to add to the task set.
    TaskAddRequested {
        session_id: Option<String>,
        task_json: String,
    },

    /// Operator requested removing a candidate task by ID.
    TaskRemoveRequested {
        session_id: Option<String>,
        task_id: String,
    },

    /// Operator requested regenerating the candidate task set.
    TaskRegenerateRequested {
        session_id: Option<String>,
        feedback: Option<String>,
    },

    /// Operator explicitly accepted the candidate task set.
    TasksAcceptRequested { session_id: Option<String> },

    /// Operator submitted an execution launch authorization decision.
    ExecutionAuthorizationSubmitted {
        session_id: Option<String>,
        decision: bool,
        reason: Option<String>,
    },

    /// Operator invoked a global user-defined slash command.
    ///
    /// The command is a typed contract loaded from
    /// `<global_config_dir>/prompts/commands/*.toml`. Execution routes
    /// through the canonical runtime path: PromptCatalog → PromptCompiler
    /// → ModelCaller (ModelInvocationKind::UserCommand) → PolicyGate →
    /// ApprovalCoordinator → ToolPipeline → Verification.
    UserCommandRequested { command: String, args: Vec<String> },
}
