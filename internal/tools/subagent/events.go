// Package subagent implements parallel child-agent execution.
//
// A subagent is a lightweight agentic loop that runs in its own goroutine
// with its own working directory (typically a git worktree), its own tool
// dispatcher, and its own message history. It communicates with the parent
// through a typed event channel; no shared Bubble Tea state is touched.
package subagent

import (
	"context"
	"encoding/json"
	"time"

	"github.com/eshanized/M31A/pkg/types"
)

// EventType discriminates the SubagentEvent variants below.
type EventType string

const (
	EventSpawned     EventType = "spawned"
	EventSpawnFailed EventType = "spawn_failed"
	EventToolStart   EventType = "tool_start"
	EventToolDone    EventType = "tool_done"
	EventTextDelta   EventType = "text_delta"
	EventThinking    EventType = "thinking"
	EventDone        EventType = "done"
	EventError       EventType = "error"
	EventCancelled   EventType = "cancelled"
)

// Status describes a subagent's lifecycle state.
type Status string

const (
	StatusPending Status = "pending"
	StatusRunning Status = "running"
	StatusDone    Status = "done"
	StatusError   Status = "error"
	StatusCancel  Status = "cancelled"
)

// Isolation controls workspace isolation for a subagent.
type Isolation string

const (
	// IsolationWorktree creates a git worktree + branch for the subagent.
	IsolationWorktree Isolation = "worktree"
	// IsolationDefault shares the parent working directory.
	IsolationDefault Isolation = "default"
)

// SpawnRequest is the input to Manager.Spawn.
type SpawnRequest struct {
	Description  string    // short 3-5 word label
	Prompt       string    // full task description
	Name         string    // optional stable name
	SubagentType string    // agent profile name ("explore", "general", etc.); empty = "general"
	Isolation    Isolation // worktree or default
	Background   bool      // if true, return immediately; if false, block until done
	ModelID      string    // empty = inherit parent
	MaxTools     int       // 0 = default (50)
	MaxTokens    int       // 0 = default (50_000)
}

// SubagentEvent is emitted by a running subagent. Consumers (the TUI, the
// parent tool result collector) read these from Manager.Events().
//
// Tagged union — interpret fields based on Type.
type SubagentEvent struct {
	Type         EventType `json:"type"`
	AgentID      string    `json:"agent_id"`
	Name         string    `json:"name,omitempty"`
	SubagentType string    `json:"subagent_type,omitempty"`
	Timestamp    time.Time `json:"ts"`

	// Spawned
	Worktree string `json:"worktree,omitempty"`

	// ToolStart / ToolDone
	ToolCallID string `json:"tool_call_id,omitempty"`
	ToolName   string `json:"tool_name,omitempty"`
	ToolInput  string `json:"tool_input,omitempty"` // abbreviated for display
	ToolOutput string `json:"tool_output,omitempty"`
	ToolError  string `json:"tool_error,omitempty"`
	DurationMs int64  `json:"duration_ms,omitempty"`

	// TextDelta / Thinking
	Delta string `json:"delta,omitempty"`

	// Done
	Summary    string       `json:"summary,omitempty"`
	ToolCalls  int          `json:"tool_calls,omitempty"`
	InputToks  int          `json:"input_tokens,omitempty"`
	OutputToks int          `json:"output_tokens,omitempty"`
	Usage      *types.Usage `json:"usage,omitempty"`

	// Error
	Error string `json:"error,omitempty"`
}

// SubagentInfo is a snapshot for TUI rendering and /agent listing.
type SubagentInfo struct {
	ID           string    `json:"id"`
	Name         string    `json:"name,omitempty"`
	SubagentType string    `json:"subagent_type,omitempty"`
	Description  string    `json:"description"`
	Status       Status    `json:"status"`
	Isolation    Isolation `json:"isolation"`
	Worktree     string    `json:"worktree,omitempty"`
	ModelID      string    `json:"model_id"`
	StartedAt    time.Time `json:"started_at"`
	FinishedAt   time.Time `json:"finished_at,omitempty"`

	// Counters
	ToolCalls  int `json:"tool_calls"`
	InputToks  int `json:"input_tokens"`
	OutputToks int `json:"output_tokens"`

	// Last activity for the expanded card
	LastToolName   string `json:"last_tool_name,omitempty"`
	LastToolStatus string `json:"last_tool_status,omitempty"` // "running" | "done"
	LastSummary    string `json:"last_summary,omitempty"`
	LastError      string `json:"last_error,omitempty"`
}

// MarshalJSON guards against nil slices and ensures stable output.
func (e SubagentEvent) MarshalJSON() ([]byte, error) {
	type alias SubagentEvent
	return json.Marshal((alias)(e))
}

// ToolCallInput is the minimum shape the subagent loop needs from a tool
// dispatcher. It mirrors types.ToolCall/types.ToolResult so the subagent
// package does not import internal/tools (which would create a cycle).
type ToolCallInput struct {
	ID    string          `json:"id"`
	Name  string          `json:"name"`
	Input json.RawMessage `json:"input"`
}

type ToolCallOutput struct {
	ToolCallID string `json:"tool_call_id"`
	Output     string `json:"output"`
	Error      string `json:"error,omitempty"`
	DurationMs int64  `json:"duration_ms"`
}

// ToolDispatcher abstracts the subset of tools.Dispatcher that the subagent
// loop uses. The concrete Dispatcher satisfies this interface without
// modification.
type ToolDispatcher interface {
	Execute(ctx context.Context, call ToolCallInput) (ToolCallOutput, error)
	ListTools() []ToolDescriptor
	UnregisterTool(name string)
	Stop()
}

// ToolDescriptor describes a single tool for the provider tool-definition
// payload. The loop uses these to advertise available tools to the LLM.
type ToolDescriptor struct {
	Name        string
	Description string
	// ParameterSchema is the JSON Schema string; empty means "{}".
	ParameterSchema string
}

// DispatcherFactory builds a fresh ToolDispatcher scoped to a subagent's
// workspace. This lets the parent wire a real tools.Dispatcher without the
// subagent package importing internal/tools.
type DispatcherFactory func(workDir string) (ToolDispatcher, error)
