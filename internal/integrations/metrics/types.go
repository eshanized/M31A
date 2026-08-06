// Package metrics provides a centralized observability pipeline for M31A
// sessions. It tracks tool execution metrics, LLM interaction metrics,
// and workflow phase metrics with thread-safe collection and JSON persistence.
package metrics

import (
	"time"

	m31types "github.com/eshanized/M31A/internal/core/types"
)

// ToolMetric captures the execution metrics for a single tool type.
type ToolMetric struct {
	Name         string  `json:"name"`
	CallCount    int64   `json:"call_count"`
	SuccessCount int64   `json:"success_count"`
	FailCount    int64   `json:"fail_count"`
	TotalDurMs   int64   `json:"total_duration_ms"`
	AvgDurMs     float64 `json:"avg_duration_ms"`
}

// EditStrategyMetric tracks which edit replacement strategy was used.
type EditStrategyMetric struct {
	Strategy string `json:"strategy"`
	Count    int64  `json:"count"`
}

// LLMMetric captures token usage and cost for an LLM interaction in a phase.
type LLMMetric struct {
	Phase            m31types.WorkflowPhase `json:"phase"`
	PromptTokens     int64                  `json:"prompt_tokens"`
	CompletionTokens int64                  `json:"completion_tokens"`
	TotalTokens      int64                  `json:"total_tokens"`
	Cost             float64                `json:"cost"`
	InteractionCount int64                  `json:"interaction_count"`
	PromptHash       string                 `json:"prompt_hash,omitempty"`
	TruncatedInput   bool                   `json:"truncated_input,omitempty"`
}

// PhaseMetric captures workflow phase execution metrics.
type PhaseMetric struct {
	Phase              m31types.WorkflowPhase `json:"phase"`
	DurationMs         int64                  `json:"duration_ms"`
	TransitionCount    int64                  `json:"transition_count"`
	HealTriggerCount   int64                  `json:"heal_trigger_count"`
	HealSuccessCount   int64                  `json:"heal_success_count"`
	HealFailCount      int64                  `json:"heal_fail_count"`
	HealDurationMs     int64                  `json:"heal_duration_ms,omitempty"`
	HealLoopCount      int64                  `json:"heal_loop_count,omitempty"`
	BisectTriggerCount int64                  `json:"bisect_trigger_count"`
	BisectSuccessCount int64                  `json:"bisect_success_count"`
	BisectFailCount    int64                  `json:"bisect_fail_count"`
	Success            bool                   `json:"success"`
}

// PlanOutcome captures the result of executing a single task within a plan.
type PlanOutcome struct {
	TaskID      int       `json:"task_id"`
	Action      string    `json:"action"`
	Description string    `json:"description"`
	Files       []string  `json:"files,omitempty"`
	Success     bool      `json:"success"`
	DurationMs  int64     `json:"duration_ms"`
	HealsUsed   int       `json:"heals_used"`
	ToolCalls   int       `json:"tool_calls"`
	ErrorType   string    `json:"error_type,omitempty"`
	Timestamp   time.Time `json:"timestamp"`
}

// SessionMetrics is the top-level metrics container persisted per session.
type SessionMetrics struct {
	SessionID      string               `json:"session_id"`
	StartedAt      time.Time            `json:"started_at"`
	UpdatedAt      time.Time            `json:"updated_at"`
	Tools          []ToolMetric         `json:"tools"`
	EditStrategies []EditStrategyMetric `json:"edit_strategies,omitempty"`
	LLMs           []LLMMetric          `json:"llms"`
	Phases         []PhaseMetric        `json:"phases"`
	PlanOutcomes   []PlanOutcome        `json:"plan_outcomes,omitempty"`
	Startup        *StartupMetric       `json:"startup,omitempty"`
	Completions    []CompletionMetric   `json:"completions,omitempty"`
	Cancellations  []CancellationMetric `json:"cancellations,omitempty"`
}

// StartupMetric captures startup timing breakdown.
type StartupMetric struct {
	DurationMs   int64 `json:"duration_ms"`
	ConfigLoadMs int64 `json:"config_load_ms"`
	ProviderMs   int64 `json:"provider_ms"`
	TUIMs        int64 `json:"tui_ms"`
}

// CompletionMetric captures phase completion metrics.
type CompletionMetric struct {
	Phase     m31types.WorkflowPhase `json:"phase"`
	Success   bool                   `json:"success"`
	Failure   bool                   `json:"failure"`
	Timeout   bool                   `json:"timeout"`
	Timestamp time.Time              `json:"timestamp"`
}

// CancellationMetric captures workflow cancellation events.
type CancellationMetric struct {
	Phase     m31types.WorkflowPhase `json:"phase"`
	Reason    string                 `json:"reason"`
	Timestamp time.Time              `json:"timestamp"`
}
