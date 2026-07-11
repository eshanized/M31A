// Package tui implements the Bubble Tea TUI for M31A.
//
// This file re-exports types from the tuitypes sub-package so that
// existing code within the tui package can reference them without
// explicit imports. New code should import tuitypes directly.
package tui

import "github.com/eshanized/M31A/internal/tui/tuitypes"

// Screen re-exports
type Screen = tuitypes.Screen

const (
	ScreenFirstRun         = tuitypes.ScreenFirstRun
	ScreenREPL             = tuitypes.ScreenREPL
	ScreenModelSelector    = tuitypes.ScreenModelSelector
	ScreenSettings         = tuitypes.ScreenSettings
	ScreenResume           = tuitypes.ScreenResume
	ScreenPermission       = tuitypes.ScreenPermission
	ScreenPlan             = tuitypes.ScreenPlan
	ScreenExecute          = tuitypes.ScreenExecute
	ScreenVerify           = tuitypes.ScreenVerify
	ScreenShip             = tuitypes.ScreenShip
	ScreenDiff             = tuitypes.ScreenDiff
	ScreenLedger           = tuitypes.ScreenLedger
	ScreenRollback         = tuitypes.ScreenRollback
	ScreenGoalInput        = tuitypes.ScreenGoalInput
	ScreenDiscuss          = tuitypes.ScreenDiscuss
	ScreenMetrics          = tuitypes.ScreenMetrics
	ScreenConfig           = tuitypes.ScreenConfig
	ScreenHelp             = tuitypes.ScreenHelp
	ScreenBisect           = tuitypes.ScreenBisect
	ScreenNotifications    = tuitypes.ScreenNotifications
	ScreenDashboard        = tuitypes.ScreenDashboard
	ScreenSessionDetail    = tuitypes.ScreenSessionDetail
	ScreenFileExplorer     = tuitypes.ScreenFileExplorer
	ScreenToolDetail       = tuitypes.ScreenToolDetail
	ScreenPhaseModelPicker = tuitypes.ScreenPhaseModelPicker
	ScreenGhostPicker      = tuitypes.ScreenGhostPicker
	ScreenGhostOutput      = tuitypes.ScreenGhostOutput
	ScreenConfirmQuit      = tuitypes.ScreenConfirmQuit
	ScreenChatHistory      = tuitypes.ScreenChatHistory
	ScreenCommandPalette   = tuitypes.ScreenCommandPalette
	ScreenRuntimeCheck     = tuitypes.ScreenRuntimeCheck
	ScreenHome             = tuitypes.ScreenHome
	ScreenDecisions        = tuitypes.ScreenDecisions
)

// Message type re-exports
type (
	AppMsg                  = tuitypes.AppMsg
	ModelSelectedMsg        = tuitypes.ModelSelectedMsg
	ProviderEntry           = tuitypes.ProviderEntry
	FirstRunCompleteMsg     = tuitypes.FirstRunCompleteMsg
	HealthCheckTickMsg      = tuitypes.HealthCheckTickMsg
	HealthCheckResultMsg    = tuitypes.HealthCheckResultMsg
	RefreshCacheMsg         = tuitypes.RefreshCacheMsg
	CacheRefreshResultMsg   = tuitypes.CacheRefreshResultMsg
	ErrorMsg                = tuitypes.ErrorMsg
	PermissionRequestMsg    = tuitypes.PermissionRequestMsg
	PermissionResponseMsg   = tuitypes.PermissionResponseMsg
	PermissionTickMsg       = tuitypes.PermissionTickMsg
	QuestionRequestMsg      = tuitypes.QuestionRequestMsg
	QuestionResponseMsg     = tuitypes.QuestionResponseMsg
	DiscussAnswerTimeoutMsg = tuitypes.DiscussAnswerTimeoutMsg
	DiscussAnswerMsg        = tuitypes.DiscussAnswerMsg
	DiscussCompleteMsg      = tuitypes.DiscussCompleteMsg
	PhaseResultMsg          = tuitypes.PhaseResultMsg
	PlanReadyMsg            = tuitypes.PlanReadyMsg
	PlanApproveMsg          = tuitypes.PlanApproveMsg
	PlanRefineMsg           = tuitypes.PlanRefineMsg
	ExecutePauseMsg         = tuitypes.ExecutePauseMsg
	HealResultMsg           = tuitypes.HealResultMsg
	GoalSubmittedMsg        = tuitypes.GoalSubmittedMsg
	PhaseModelPickedMsg     = tuitypes.PhaseModelPickedMsg
	StreamChunkMsg          = tuitypes.StreamChunkMsg
	SlashCommandMsg         = tuitypes.SlashCommandMsg
	HomeSubmitMsg           = tuitypes.HomeSubmitMsg
	ToastMsg                = tuitypes.ToastMsg
	ToastExpiryMsg          = tuitypes.ToastExpiryMsg
	Toast                   = tuitypes.Toast
	DismissToastMsg         = tuitypes.DismissToastMsg
	FallbackEventMsg        = tuitypes.FallbackEventMsg
	SettingsSavedMsg        = tuitypes.SettingsSavedMsg
	ResetCompleteMsg        = tuitypes.ResetCompleteMsg
	OptimizedMsg            = tuitypes.OptimizedMsg
	BisectStartMsg          = tuitypes.BisectStartMsg
	SessionDetailRequestMsg = tuitypes.SessionDetailRequestMsg
	DiffScreenMsg           = tuitypes.DiffScreenMsg
	DiffCloseMsg            = tuitypes.DiffCloseMsg
	SidebarRefreshMsg       = tuitypes.SidebarRefreshMsg
	SidebarFile             = tuitypes.SidebarFile
	SidebarRefreshTickMsg   = tuitypes.SidebarRefreshTickMsg
	SidebarTodoUpdateMsg    = tuitypes.SidebarTodoUpdateMsg
	SidebarTodoItem         = tuitypes.SidebarTodoItem
	SidebarRevertMsg        = tuitypes.SidebarRevertMsg
	SessionRenameMsg        = tuitypes.SessionRenameMsg
	SessionExportMsg        = tuitypes.SessionExportMsg
	PopScreenMsg            = tuitypes.PopScreenMsg
	GhostWriteRequestMsg    = tuitypes.GhostWriteRequestMsg
	GhostWriteResultMsg     = tuitypes.GhostWriteResultMsg
	ChatHistoryContinueMsg  = tuitypes.ChatHistoryContinueMsg
	IntentClassifiedMsg     = tuitypes.IntentClassifiedMsg
	EmitterDropLogTickMsg   = tuitypes.EmitterDropLogTickMsg
)

// WorkflowEngine re-exports the workflow engine interface from tuitypes.
type WorkflowEngine = tuitypes.WorkflowEngine
