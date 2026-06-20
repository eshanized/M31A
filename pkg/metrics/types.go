// Package metrics provides a centralized observability pipeline for M31A
// sessions. It tracks tool execution metrics, LLM interaction metrics,
// and workflow phase metrics with thread-safe collection and JSON persistence.
package metrics

import (
	"time"

	m31types "github.com/eshanized/M31A/internal/types"
)

// ToolMetric captures the execution metrics for a single tool type.
type ToolMetric struct {
	Name        string  `json:"name"`
	CallCount   int64   `json:"call_count"`
	SuccessCount int64  `json:"success_count"`
	FailCount   int64   `json:"fail_count"`
	TotalDurMs  int64   `json:"total_duration_ms"`
	AvgDurMs    float64 `json:"avg_duration_ms"`
}

// LLMMetric captures token usage and cost for an LLM interaction in a phase.
type LLMMetric struct {
	Phase           m31types.WorkflowPhase `json:"phase"`
	PromptTokens    int64                  `json:"prompt_tokens"`
	CompletionTokens int64                 `json:"completion_tokens"`
	TotalTokens     int64                  `json:"total_tokens"`
	Cost            float64                `json:"cost"`
	InteractionCount int64                 `json:"interaction_count"`
}

// PhaseMetric captures workflow phase execution metrics.
type PhaseMetric struct {
	Phase           m31types.WorkflowPhase `json:"phase"`
	DurationMs      int64                  `json:"duration_ms"`
	TransitionCount int64                  `json:"transition_count"`
	HealTriggerCount int64                 `json:"heal_trigger_count"`
	BisectTriggerCount int64               `json:"bisect_trigger_count"`
	Success         bool                   `json:"success"`
}

// SessionMetrics is the top-level metrics container persisted per session.
type SessionMetrics struct {
	SessionID string        `json:"session_id"`
	StartedAt time.Time     `json:"started_at"`
	UpdatedAt time.Time     `json:"updated_at"`
	Tools     []ToolMetric  `json:"tools"`
	LLMs      []LLMMetric   `json:"llms"`
	Phases    []PhaseMetric `json:"phases"`
}
