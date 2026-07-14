package types

import (
	pkgTypes "github.com/eshanized/M31A/pkg/types"
)

// Type aliases — these preserve type identity so internal/ code continues
// to work unchanged while the canonical definitions live in pkg/types/.

type RiskLevel = pkgTypes.RiskLevel

const (
	RiskSafe        = pkgTypes.RiskSafe
	RiskMedium      = pkgTypes.RiskMedium
	RiskDangerous   = pkgTypes.RiskDangerous
	RiskDestructive = pkgTypes.RiskDestructive
)

type WorkflowPhase = pkgTypes.WorkflowPhase

const (
	PhaseIdle       = pkgTypes.PhaseIdle
	PhaseInitialize = pkgTypes.PhaseInitialize
	PhaseDiscuss    = pkgTypes.PhaseDiscuss
	PhasePlan       = pkgTypes.PhasePlan
	PhaseExecute    = pkgTypes.PhaseExecute
	PhaseVerify     = pkgTypes.PhaseVerify
	PhaseRuntime    = pkgTypes.PhaseRuntime
	PhaseShip       = pkgTypes.PhaseShip
)

type ComplexityLevel = pkgTypes.ComplexityLevel

const (
	ComplexityTrivial  = pkgTypes.ComplexityTrivial
	ComplexitySimple   = pkgTypes.ComplexitySimple
	ComplexityModerate = pkgTypes.ComplexityModerate
	ComplexityComplex  = pkgTypes.ComplexityComplex
)

type WorkflowMode = pkgTypes.WorkflowMode

const (
	ModeAuto   = pkgTypes.ModeAuto
	ModeFull   = pkgTypes.ModeFull
	ModeFast   = pkgTypes.ModeFast
	ModeDirect = pkgTypes.ModeDirect
)

type IntentType = pkgTypes.IntentType

const (
	IntentFeature     = pkgTypes.IntentFeature
	IntentBugfix      = pkgTypes.IntentBugfix
	IntentRefactor    = pkgTypes.IntentRefactor
	IntentQuestion    = pkgTypes.IntentQuestion
	IntentExplanation = pkgTypes.IntentExplanation
	IntentExploration = pkgTypes.IntentExploration
	IntentChore       = pkgTypes.IntentChore
)

type IntentResult = pkgTypes.IntentResult

func WorkflowModeForIntent(ir IntentResult) WorkflowMode {
	return pkgTypes.WorkflowModeForIntent(ir)
}

func WorkflowModeForIntentComplexity(c ComplexityLevel) WorkflowMode {
	return pkgTypes.WorkflowModeForIntentComplexity(c)
}

// IsWorkflowWorthy is a method on IntentResult. Since IntentResult is an alias,
// the method is automatically available.

type TaskStatus = pkgTypes.TaskStatus

const (
	StatusPending       = pkgTypes.StatusPending
	StatusRunning       = pkgTypes.StatusRunning
	StatusDone          = pkgTypes.StatusDone
	StatusFailed        = pkgTypes.StatusFailed
	StatusSkipped       = pkgTypes.StatusSkipped
	StatusUnrecoverable = pkgTypes.StatusUnrecoverable
)

type Usage = pkgTypes.Usage

type CapFlags = pkgTypes.CapFlags

type Pricing = pkgTypes.Pricing

type ArchInfo = pkgTypes.ArchInfo

type ModelInfo = pkgTypes.ModelInfo

type MessageSegment = pkgTypes.MessageSegment

const MessageCompaction = pkgTypes.MessageCompaction

type ToolCall = pkgTypes.ToolCall

type Message = pkgTypes.Message

type ToolInput = pkgTypes.ToolInput

type ToolResult = pkgTypes.ToolResult

type ToolError = pkgTypes.ToolError

func NewToolError(err error, hint string) *ToolError {
	return pkgTypes.NewToolError(err, hint)
}

type HealReport = pkgTypes.HealReport

type Tool = pkgTypes.Tool

type SchemaProvider = pkgTypes.SchemaProvider

type Task = pkgTypes.Task

type ProjectState = pkgTypes.ProjectState

type Session = pkgTypes.Session

type StreamChunk = pkgTypes.StreamChunk

type StreamIterator = pkgTypes.StreamIterator

type StreamChunkMsg = pkgTypes.StreamChunkMsg

type HealthStatus = pkgTypes.HealthStatus

type DiffSummary = pkgTypes.DiffSummary

type FileDiff = pkgTypes.FileDiff

type ChatRequest = pkgTypes.ChatRequest

type ToolDefinition = pkgTypes.ToolDefinition

// FilePrediction action constants for consistent action values.
const (
	FileActionCreate = pkgTypes.FileActionCreate
	FileActionModify = pkgTypes.FileActionModify
	FileActionDelete = pkgTypes.FileActionDelete
)
