package tui

import (
	"time"

	"github.com/eshanized/M31A/internal/git"
	"github.com/eshanized/M31A/internal/tools"
	"github.com/eshanized/M31A/internal/types"
	"github.com/eshanized/M31A/internal/workflow"
	"github.com/eshanized/M31A/pkg/arbitrage"
)

type Screen int

const (
	ScreenFirstRun Screen = iota
	ScreenREPL
	ScreenModelSelector
	ScreenSettings
	ScreenResume
	ScreenPermission
	ScreenPlan
	ScreenExecute
	ScreenVerify
	ScreenShip
	ScreenDiff Screen = 10
)

type AppMsg struct {
	Screen        Screen
	SessionID     string         // populated by resume screen on selection
	SaveKeychain  bool           // save API key to system keychain
	ModelSelected *ModelSelectedMsg // model selection result
}

type HealthCheckTickMsg struct {
	Time time.Time
}

type ErrorMsg struct {
	Err error
}

type PermissionRequestMsg struct {
	Request tools.PermissionRequest
}

type PermissionResponseMsg struct {
	Response tools.PermissionResponse
}

// PermissionTickMsg is emitted every 100ms while the permission modal is visible,
// driving the countdown timer and triggering auto-deny on timeout.
type PermissionTickMsg struct{}

// FallbackEventMsg carries provider fallback information to the TUI.
type FallbackEventMsg struct {
	From   string `json:"from"`
	To     string `json:"to"`
	Reason string `json:"reason"`
}

// RefreshCacheMsg triggers a model cache refresh for the given provider.
type RefreshCacheMsg struct {
	ProviderName string `json:"provider_name"`
}

// ModelSelectedMsg carries the model selection result back to AppState.
type ModelSelectedMsg struct {
	Model    types.ModelInfo
	Provider string
}

// SettingsSavedMsg is emitted when the settings screen saves the config successfully.
type SettingsSavedMsg struct{}

// PhaseResultMsg carries the result of a workflow phase execution from the
// engine goroutine to the TUI update loop.
type PhaseResultMsg struct {
	Phase               types.WorkflowPhase
	Tasks               []types.Task
	Messages            []types.Message
	Success             bool
	Error               string
	NeedsAnswers        bool
	RequiresManualInput bool
	DurationMs          int64

	// Execution metrics (from workflow.PhaseResult)
	Usage     *types.Usage
	Cost      float64
	ToolCalls int
	Commits   []git.CommitInfo
	DiffStats workflow.DiffStats
}

// PlanReadyMsg is emitted from RunPhaseCmd when the plan phase completes
// successfully with valid tasks.
type PlanReadyMsg struct {
	Tasks        []types.Task
	CostEstimate string
	TimeEstimate string
}

// QuestionRequestMsg is sent by the AskUserQuestion tool to request user input.
type QuestionRequestMsg struct {
	Question    string
	Header      string
	Options     []string
	AllowCustom bool
	ResponseCh  chan tools.QuestionResponse
}

// QuestionResponseMsg carries the user's answer back to the question tool.
type QuestionResponseMsg struct {
	Answer string
}

// DiscussAnswerTimeoutMsg is emitted by the discuss answer timer when
// 5 minutes elapse without the user answering the current question.
// The TUI handler calls engine.SkipDiscuss() and advances to Plan.
type DiscussAnswerTimeoutMsg struct {
	QuestionIndex int
}

// StreamChunkMsg is emitted by workflow phases that stream LLM responses
// (currently only the Discuss phase). The TUI renders each chunk in the
// active screen via the REPL streaming infrastructure.
type StreamChunkMsg = types.StreamChunkMsg

// ThemeChangedMsg is emitted when the theme is switched at runtime.
type ThemeChangedMsg struct {
	Theme string // "dark" or "light"
}

// SlashCommandMsg is emitted by the REPL when the user enters a slash command.
// It carries the raw command string for app-level processing.
type SlashCommandMsg struct {
	Command string
}

// ToastMsg is a transient notification message.
type ToastMsg struct {
	Text     string
	Duration time.Duration
	Type     string // "info", "success", "warning", "error"
}

// OptimizedMsg carries arbitrage recommendations back to the TUI.
// H-19: wired from /optimize command and Plan screen "O" key.
type OptimizedMsg struct {
	Recommendations []arbitrage.ArbitrageRecommendation
	TaskID          int
}
