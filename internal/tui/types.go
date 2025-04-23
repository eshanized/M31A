package tui

import (
	"time"

	"github.com/eshanized/M31A/internal/tools"
	"github.com/eshanized/M31A/internal/types"
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
)

type AppMsg struct {
	Screen         Screen
	SessionID      string   // populated by resume screen on selection
	SaveKeychain   bool     // save API key to system keychain
	FallbackEvent  *FallbackEventMsg    // provider fallback notification
	ThinkingToggle *ThinkingToggleMsg   // toggle thinking block visibility
	RefreshCache   *RefreshCacheMsg     // trigger model cache refresh
	ModelSelected  *ModelSelectedMsg    // model selection result
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

// FallbackEventMsg carries provider fallback information to the TUI.
type FallbackEventMsg struct {
	From   string `json:"from"`
	To     string `json:"to"`
	Reason string `json:"reason"`
}

// ThinkingToggleMsg signals that the user wants to toggle thinking block visibility.
type ThinkingToggleMsg struct {
	// empty — the handler toggles all blocks
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
	Phase    types.WorkflowPhase
	Tasks    []types.Task
	Messages []types.Message
	Success  bool
	Error    string
}

// PlanReadyMsg is emitted from RunPhaseCmd when the plan phase completes
// successfully with valid tasks.
type PlanReadyMsg struct {
	Tasks        []types.Task
	CostEstimate string
	TimeEstimate string
}
