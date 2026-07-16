// Package tuitypes defines shared types for the M31A TUI.
//
// Screen, message types, and workflow engine interface are extracted here
// to break circular dependencies between the core tui package and its
// sub-packages (commands, streaming).
package tuitypes

import (
	"context"
	"time"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/eshanized/M31A/internal/decision"
	"github.com/eshanized/M31A/internal/git"
	"github.com/eshanized/M31A/internal/provider"
	"github.com/eshanized/M31A/internal/tools"
	"github.com/eshanized/M31A/internal/workflow"
	"github.com/eshanized/M31A/pkg/arbitrage"
	"github.com/eshanized/M31A/pkg/session"
	"github.com/eshanized/M31A/pkg/types"
)

// Screen identifies which full-screen view is active.
type Screen int

const (
	ScreenFirstRun         Screen = iota // 0 — API key setup wizard
	ScreenREPL                           // 1 — main chat REPL
	ScreenModelSelector                  // 2 — model/provider picker
	ScreenSettings                       // 3 — settings editor (6 tabs)
	ScreenResume                         // 4 — session browser
	ScreenPermission                     // 5 — tool permission modal
	ScreenPlan                           // 6 — plan review
	ScreenExecute                        // 7 — task execution progress
	ScreenVerify                         // 8 — verification results
	ScreenShip                           // 9 — ship summary
	ScreenDiff             Screen = 10   // diff viewer
	ScreenLedger           Screen = 11   // learning ledger browser
	ScreenRollback         Screen = 12   // commit time machine
	ScreenGoalInput        Screen = 13   // full-screen goal entry
	ScreenDiscuss          Screen = 14   // discuss Q&A
	ScreenMetrics          Screen = 15   // session analytics
	ScreenConfig           Screen = 16   // full config viewer
	ScreenHelp             Screen = 17   // keybinding help overlay
	ScreenBisect           Screen = 18   // git bisect interactive
	ScreenNotifications    Screen = 20   // notification history
	ScreenDashboard        Screen = 21   // workflow pipeline overview
	ScreenSessionDetail    Screen = 22   // session detail preview
	ScreenFileExplorer     Screen = 23   // file tree browser
	ScreenToolDetail       Screen = 24   // expandable tool output
	ScreenPhaseModelPicker Screen = 25   // dual-model picker (planning vs coding)
	ScreenGhostPicker      Screen = 26   // ghost write file selector
	ScreenGhostOutput      Screen = 27   // ghost write results
	ScreenConfirmQuit      Screen = 28   // confirm quit dialog
	ScreenChatHistory      Screen = 29   // chat history table browser
	ScreenCommandPalette   Screen = 30   // dedicated command palette with detail panel
	ScreenRuntimeCheck     Screen = 31   // runtime verification (dev server + smoke tests)
	ScreenHome             Screen = 32   // landing screen with logo, prompt, and tips
	ScreenDecisions        Screen = 33   // decision log browser
	ScreenPhaseTransition  Screen = 34   // phase transition confirmation
)

// Label returns a human-readable name for the screen.
func (s Screen) Label() string {
	switch s {
	case ScreenFirstRun:
		return "Setup"
	case ScreenREPL:
		return "Chat"
	case ScreenModelSelector:
		return "Models"
	case ScreenSettings:
		return "Settings"
	case ScreenResume:
		return "Sessions"
	case ScreenPermission:
		return "Permission"
	case ScreenPlan:
		return "Plan"
	case ScreenExecute:
		return "Execute"
	case ScreenVerify:
		return "Verify"
	case ScreenShip:
		return "Ship"
	case ScreenDiff:
		return "Diff"
	case ScreenLedger:
		return "Ledger"
	case ScreenRollback:
		return "Rollback"
	case ScreenGoalInput:
		return "Goal"
	case ScreenDiscuss:
		return "Discuss"
	case ScreenMetrics:
		return "Metrics"
	case ScreenConfig:
		return "Config"
	case ScreenHelp:
		return "Help"
	case ScreenBisect:
		return "Bisect"
	case ScreenNotifications:
		return "Notifications"
	case ScreenDashboard:
		return "Dashboard"
	case ScreenSessionDetail:
		return "Session"
	case ScreenFileExplorer:
		return "Files"
	case ScreenToolDetail:
		return "Tool Output"
	case ScreenPhaseModelPicker:
		return "Model Setup"
	case ScreenGhostPicker:
		return "Ghost Picker"
	case ScreenGhostOutput:
		return "Ghost Output"
	case ScreenConfirmQuit:
		return "Confirm Quit"
	case ScreenChatHistory:
		return "Chat History"
	case ScreenCommandPalette:
		return "Commands"
	case ScreenRuntimeCheck:
		return "Runtime Check"
	case ScreenHome:
		return "Home"
	case ScreenDecisions:
		return "Decisions"
	default:
		return "Unknown"
	}
}

// Name returns a lowercase slug for the screen, used for sidebar hints
// and programmatic lookups.
func (s Screen) Name() string {
	switch s {
	case ScreenREPL:
		return "repl"
	case ScreenExecute:
		return "execute"
	case ScreenPlan:
		return "plan"
	case ScreenVerify:
		return "verify"
	case ScreenRuntimeCheck:
		return "runtime"
	case ScreenShip:
		return "ship"
	case ScreenDiscuss:
		return "discuss"
	case ScreenSettings:
		return "settings"
	case ScreenHelp:
		return "help"
	case ScreenChatHistory:
		return "chathistory"
	case ScreenConfig:
		return "config"
	case ScreenResume:
		return "resume"
	case ScreenRollback:
		return "rollback"
	case ScreenDiff:
		return "diff"
	case ScreenModelSelector:
		return "modelselector"
	case ScreenCommandPalette:
		return "cmdpalette"
	case ScreenPhaseModelPicker:
		return "phasempicker"
	case ScreenSessionDetail:
		return "session"
	case ScreenFileExplorer:
		return "fileexplorer"
	case ScreenConfirmQuit:
		return "confirmquit"
	case ScreenDashboard:
		return "dashboard"
	case ScreenMetrics:
		return "metrics"
	case ScreenLedger:
		return "ledger"
	case ScreenHome:
		return "home"
	case ScreenFirstRun:
		return "firstrun"
	case ScreenGoalInput:
		return "goalinput"
	case ScreenGhostPicker:
		return "ghostpicker"
	case ScreenGhostOutput:
		return "ghostoutput"
	case ScreenToolDetail:
		return "tooldetail"
	case ScreenNotifications:
		return "notifications"
	case ScreenBisect:
		return "bisect"
	case ScreenPermission:
		return "permission"
	case ScreenDecisions:
		return "decisions"
	default:
		return ""
	}
}

// ─── App-level messages ──────────────────────────────────────────────────────

// AppMsg is the general routing message from sub-models to AppState.
type AppMsg struct {
	Screen        Screen
	Action        string // optional action identifier (e.g., "new_session")
	SessionID     string // populated by resume screen on selection
	SaveKeychain  bool   // save API key to system keychain
	ModelSelected *ModelSelectedMsg
}

// ModelSelectedMsg carries the result of model selection back to AppState.
type ModelSelectedMsg struct {
	Model    types.ModelInfo
	Provider string
}

// ProviderEntry pairs a provider ID with its collected API key.
type ProviderEntry struct {
	ID     string
	APIKey string
}

// FirstRunCompleteMsg carries the multi-provider wizard results to AppState.
type FirstRunCompleteMsg struct {
	Providers       []ProviderEntry
	ModelID         string
	SaveKeychain    bool
	DefaultProvider string
}

// ─── Infrastructure messages ─────────────────────────────────────────────────

// HealthCheckTickMsg is emitted by the health check ticker.
type HealthCheckTickMsg struct {
	Time time.Time
}

// HealthCheckResultMsg carries an async health check result.
type HealthCheckResultMsg struct {
	Result types.HealthStatus
}

// RefreshCacheMsg triggers a model cache refresh.
type RefreshCacheMsg struct {
	ProviderName string
}

// CacheRefreshResultMsg carries an async cache refresh result.
type CacheRefreshResultMsg struct {
	ErrMsg  string
	NextCmd tea.Cmd
}

// ErrorMsg carries a generic error to the TUI update loop.
type ErrorMsg struct {
	Err error
}

// EmitterDropLogTickMsg is emitted periodically to log the emitter drop counter
// if any messages have been dropped.
type EmitterDropLogTickMsg struct {
	Time time.Time
}

// ─── Permission messages ──────────────────────────────────────────────────────

// PermissionRequestMsg is sent when a tool needs user approval.
type PermissionRequestMsg struct {
	Request tools.PermissionRequest
}

// PermissionResponseMsg carries the user's permission decision.
type PermissionResponseMsg struct {
	Response tools.PermissionResponse
}

// PermissionTickMsg drives the countdown timer on the permission modal.
type PermissionTickMsg struct{}

// ─── Question messages ────────────────────────────────────────────────────────

// QuestionRequestMsg is sent by the AskUserQuestion tool.
type QuestionRequestMsg struct {
	ID          int64
	Question    string
	Header      string
	Options     []string
	AllowCustom bool
	TimeoutSecs int
}

// QuestionResponseMsg carries the user's answer back to the question tool.
type QuestionResponseMsg struct {
	Answer string
}

// DiscussAnswerTimeoutMsg is emitted when the discuss answer timer expires.
type DiscussAnswerTimeoutMsg struct {
	QuestionIndex int
}

// DiscussAnswerMsg carries a single discuss answer to the AppState for submission to the workflow engine.
type DiscussAnswerMsg struct {
	Index  int
	Answer string
}

// DiscussCompleteMsg signals that all discuss questions have been answered and the engine should finalize.
type DiscussCompleteMsg struct{}

// ─── Workflow messages ────────────────────────────────────────────────────────

// PhaseResultMsg carries the result of a completed workflow phase.
type PhaseResultMsg struct {
	Phase                   types.WorkflowPhase
	Tasks                   []types.Task
	Messages                []types.Message
	Success                 bool
	Error                   string
	NeedsAnswers            bool
	RequiresManualInput     bool
	DurationMs              int64
	Usage                   *types.Usage
	Cost                    float64
	ToolCalls               int
	Commits                 []git.CommitInfo
	DiffStats               workflow.DiffStats
	Demonstration           string
	ManualVerificationSteps []string
	WorkflowMode            types.WorkflowMode
	RuntimeSummary          *workflow.RuntimeSummary
}

// PlanReadyMsg is emitted when the plan phase completes with valid tasks.
type PlanReadyMsg struct {
	Tasks        []types.Task
	CostEstimate string
	TimeEstimate string
}

// PlanApproveMsg is emitted when the user approves the plan for execution.
type PlanApproveMsg struct{}

// PlanRefineMsg is emitted when the user submits refinement feedback for the plan.
type PlanRefineMsg struct {
	Feedback string
}

// ExecutePauseMsg is emitted when the user toggles pause/resume on execute screen.
type ExecutePauseMsg struct {
	Paused bool
}

// HealResultMsg is emitted after a self-heal attempt.
type HealResultMsg struct {
	TaskID  int
	Success bool
}

// GoalSubmittedMsg is emitted by GoalInputModel when the user confirms a goal.
type GoalSubmittedMsg struct {
	Goal string
}

// PhaseModelPickedMsg is emitted by PhaseModelPickerModel when the user confirms
// their model selections (or skips). Empty model IDs mean "use default".
type PhaseModelPickedMsg struct {
	PlanningModelID  string // model for Discuss, Plan, Verify phases
	PlanningProvider string
	CodingModelID    string // model for Execute, Ship phases
	CodingProvider   string
}

// ─── Stream messages ──────────────────────────────────────────────────────────

// StreamChunkMsg carries a streaming token chunk from a workflow phase.
// It is an alias for types.StreamChunkMsg for compatibility.
type StreamChunkMsg = types.StreamChunkMsg

// ─── UI messages ─────────────────────────────────────────────────────────────

// SlashCommandMsg is emitted when the REPL user enters a slash command.
type SlashCommandMsg struct {
	Command       string
	AttachedFiles int // number of files attached via @-mention (0 if none)
}

// HomeSubmitMsg is emitted when the user submits text from the Home screen prompt.
type HomeSubmitMsg struct {
	Text string
}

// ToastMsg displays a transient notification.
type ToastMsg struct {
	Text     string
	Duration time.Duration
	Type     string // "info", "success", "warning", "error"
}

// ToastExpiryMsg clears an expired toast (H-2 fix: handled in Update, not View).
type ToastExpiryMsg struct {
	ToastID int
}

// Toast represents a transient notification overlay.
type Toast struct {
	ID          int
	Text        string
	Type        string // "success", "error", "warning", "info"
	CreatedAt   time.Time
	Frame       int            // animation frame (0, 1, 2)
	Duration    time.Duration  // auto-dismiss duration (0 = default 5s)
	Action      func() tea.Msg // optional action triggered by Enter key
	ActionLabel string         // label for action button (e.g., "Undo", "Retry")
}

// DismissToastMsg is emitted when a toast should be dismissed manually.
type DismissToastMsg struct {
	ToastID int
}

// FallbackEventMsg carries provider fallback information.
type FallbackEventMsg struct {
	From   string
	To     string
	Reason string
}

// SettingsSavedMsg is emitted when settings are saved.
type SettingsSavedMsg struct{}

// ResetCompleteMsg is emitted when /reset finishes cleaning up persistent state.
// AppState handles this by resetting in-memory state and navigating to the first-run screen.
type ResetCompleteMsg struct{}

// OptimizedMsg carries arbitrage optimization recommendations.
type OptimizedMsg struct {
	Recommendations []arbitrage.ArbitrageRecommendation
	TaskID          int
}

// BisectStartMsg carries commit data to initialize the bisect screen.
type BisectStartMsg struct {
	GoodCommit string
	BadCommit  string
}

// SessionDetailRequestMsg navigates to the session detail screen with session data.
type SessionDetailRequestMsg struct {
	Session *session.Session
}

// DiffScreenMsg triggers the diff viewer screen.
type DiffScreenMsg struct {
	Diff  string
	Title string
	Lines []string
}

// DiffCloseMsg closes the diff viewer.
type DiffCloseMsg struct{}

// SidebarRefreshMsg triggers a sidebar git status refresh.
type SidebarRefreshMsg struct {
	Files  []SidebarFile
	Branch string
	Remote string
}

// SidebarFile represents a file in the sidebar git status.
type SidebarFile struct {
	Path   string
	Status string
}

// SidebarRefreshTickMsg is emitted periodically to trigger sidebar git status refresh.
type SidebarRefreshTickMsg struct{}

// SidebarTodoUpdateMsg carries updated TODO items from the TodoWrite tool to the sidebar.
type SidebarTodoUpdateMsg struct {
	Items []SidebarTodoItem
}

// SidebarTodoItem represents a single item in the sidebar todo list.
type SidebarTodoItem struct {
	Content  string
	Status   string // "pending", "in_progress", "completed", "cancelled"
	Priority string // "high", "medium", "low"
	Source   string // "task" or "llm"
	TaskID   int    // only for Source=="task"
}

// SidebarRevertMsg triggers the sidebar to revert from todo mode back to file tree.
type SidebarRevertMsg struct{}

// SessionRenameMsg is emitted when the user triggers a rename on a session in the browser.
type SessionRenameMsg struct {
	SessionID string
}

// SessionExportMsg is emitted when the user triggers an export on a session in the browser.
type SessionExportMsg struct {
	SessionID string
}

// PopScreenMsg navigates back to the previous screen in the back-stack.
type PopScreenMsg struct{}

// ChatHistoryContinueMsg is emitted when the user selects a message in chat history to continue from.
type ChatHistoryContinueMsg struct {
	MessageIndex int // index of the message to continue from (truncate everything after)
}

// GhostWriteRequestMsg is emitted when the user selects files for ghost write.
type GhostWriteRequestMsg struct {
	Files []string
}

// GhostWriteResultMsg carries the result of a ghost write operation.
type GhostWriteResultMsg struct {
	Result *GhostResult
}

// GhostResult holds the output of a ghost write operation.
type GhostResult struct {
	Files    []GhostFile
	Warnings []string
}

// GhostFile represents a file generated by ghost write.
type GhostFile struct {
	Path    string
	Content string
	Prompt  string
}

// ─── Workflow engine interface ────────────────────────────────────────────────

// WorkflowEngine is the interface that AppState uses to invoke workflow phases.
// It is defined separately from the concrete workflow.Engine to allow testing.
type WorkflowEngine interface {
	RunPhase(ctx context.Context, phase types.WorkflowPhase, goal string) (*workflow.PhaseResult, error)
	Transition(ctx context.Context, from, to types.WorkflowPhase) error
	SetModel(modelID string, p provider.LLMProvider)
	SetPhaseModel(phase types.WorkflowPhase, modelID string)
	SetMsgEmitter(em workflow.MsgEmitter)
	SetSessionID(id string)
	SetGit(g *git.Git)
	SessionID() string
	HealTask(ctx context.Context, taskID int) (bool, error)
	SubmitDiscussAnswer(index int, answer string) error
	FinalizeDiscuss() error
	SkipDiscuss() error
	DiscussState() workflow.DiscussState
	PlanContent() string
	PlanVersion() int
	SetRefinementFeedback(feedback string)
	SetWorkflowMode(mode types.WorkflowMode)
	WorkflowMode() types.WorkflowMode
	SnapshotDecisions() []decision.DecisionReceipt
	Close()
	LoadCheckpointData(data *workflow.CheckpointData)
	GetCheckpointData() *workflow.CheckpointData

	// Pause/Resume support for execute phase
	PauseExecution() bool
	ResumeExecution() bool
	IsPaused() bool
	SkipCurrentTask(taskID int)
	CancelCurrentTask(taskID int)
	CancelGroup()

	// Cost tracking
	GetCostInfo() (totalCost float64, budgetLimit float64, budgetRemaining float64)
}

// ─── Intent classification ─────────────────────────────────────────────────────

// IntentClassifiedMsg carries the result of an async LLM intent classification.
type IntentClassifiedMsg struct {
	Result types.IntentResult
	Input  string
	Err    error
}
