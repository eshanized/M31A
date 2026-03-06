package tui

import (
	"time"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/eshanized/M31A/internal/git"
	"github.com/eshanized/M31A/internal/tools"
	"github.com/eshanized/M31A/internal/types"
	"github.com/eshanized/M31A/internal/workflow"
	"github.com/eshanized/M31A/pkg/arbitrage"
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
	ScreenThemePicker      Screen = 19   // theme browser/preview
	ScreenNotifications    Screen = 20   // notification history
	ScreenDashboard        Screen = 21   // workflow pipeline overview
	ScreenSessionDetail    Screen = 22   // session detail preview
	ScreenFileExplorer     Screen = 23   // file tree browser
	ScreenToolDetail       Screen = 24   // expandable tool output
	ScreenPhaseModelPicker Screen = 25   // dual-model picker (planning vs coding)
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
	case ScreenThemePicker:
		return "Themes"
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
	default:
		return "Unknown"
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

// ThemeChangedMsg is emitted when the theme is switched.
type ThemeChangedMsg struct {
	Theme string // "dark", "light", or "auto"
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

// FallbackEventMsg carries provider fallback information.
type FallbackEventMsg struct {
	From   string
	To     string
	Reason string
}

// SettingsSavedMsg is emitted when settings are saved.
type SettingsSavedMsg struct{}

// OptimizedMsg carries arbitrage optimization recommendations.
type OptimizedMsg struct {
	Recommendations []arbitrage.ArbitrageRecommendation
	TaskID          int
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
