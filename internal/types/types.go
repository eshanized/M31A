package types

import (
	"context"
	"encoding/json"
	"time"
)

type RiskLevel string

const (
	RiskSafe        RiskLevel = "safe"
	RiskMedium      RiskLevel = "medium"
	RiskDangerous   RiskLevel = "dangerous"
	RiskDestructive RiskLevel = "destructive"
)

type WorkflowPhase string

const (
	PhaseIdle       WorkflowPhase = "idle"
	PhaseInitialize WorkflowPhase = "initialize"
	PhaseDiscuss    WorkflowPhase = "discuss"
	PhasePlan       WorkflowPhase = "plan"
	PhaseExecute    WorkflowPhase = "execute"
	PhaseVerify     WorkflowPhase = "verify"
	PhaseShip       WorkflowPhase = "ship"
)

type TaskStatus string

const (
	StatusPending       TaskStatus = "pending"
	StatusRunning       TaskStatus = "running"
	StatusDone          TaskStatus = "done"
	StatusFailed        TaskStatus = "failed"
	StatusSkipped       TaskStatus = "skipped"
	StatusUnrecoverable TaskStatus = "unrecoverable"
)

type Usage struct {
	PromptTokens     int `json:"prompt_tokens"`
	CompletionTokens int `json:"completion_tokens"`
	TotalTokens      int `json:"total_tokens"`
}

type CapFlags struct {
	Tools     bool `json:"tools"`
	Reasoning bool `json:"reasoning"`
	Vision    bool `json:"vision"`
}

type Pricing struct {
	InputPerMToken  float64 `json:"input_per_m_token"`
	OutputPerMToken float64 `json:"output_per_m_token"`
}

type ArchInfo struct {
	TokenizerFamily string `json:"tokenizer_family"`
}

type ModelInfo struct {
	ID            string   `json:"id"`
	Provider      string   `json:"provider"`
	Name          string   `json:"name"`
	Description   string   `json:"description"`
	ContextLength int64    `json:"context_length"`
	Pricing       Pricing  `json:"pricing"`
	Architecture  ArchInfo `json:"architecture"`
	TopProvider   string   `json:"top_provider"`
	Capabilities  CapFlags `json:"capabilities"`
}

type MessageSegment struct {
	Type       string    `json:"type"`
	Content    string    `json:"content"`
	DurationMs int64     `json:"duration_ms"`
	Visible    bool      `json:"visible"`
	StartedAt  time.Time `json:"started_at,omitempty"`
}

type ToolCall struct {
	ID    string          `json:"id"`
	Name  string          `json:"name"`
	Input json.RawMessage `json:"input"`
}

type Message struct {
	Role      string           `json:"role"`
	Content   string           `json:"content"`
	Segments  []MessageSegment `json:"segments"`
	ToolCalls []ToolCall       `json:"tool_calls,omitempty"`
	Usage     *Usage           `json:"usage,omitempty"`
	CreatedAt time.Time        `json:"created_at"`
}

type ToolInput struct {
	Name   string         `json:"name"`
	Params map[string]any `json:"params"`
}

type ToolResult struct {
	ToolCallID string `json:"tool_call_id"`
	Output     string `json:"output"`
	Error      string `json:"error,omitempty"`
	DurationMs int64  `json:"duration_ms"`
	Truncated  bool   `json:"truncated"`
}

type Tool interface {
	Name() string
	Description() string
	RiskLevel() RiskLevel
	Execute(ctx context.Context, input ToolInput) (ToolResult, error)
}

type Task struct {
	ID                 int              `json:"id"`
	Description        string           `json:"description"`
	Action             string           `json:"action"`
	Dependencies       []int            `json:"dependencies"`
	Files              []string         `json:"files"`
	AcceptanceCriteria []string         `json:"acceptance_criteria"`
	Status             TaskStatus       `json:"status"`
	HealsAttempted     int              `json:"heals_attempted"`
	CommitHash         string           `json:"commit_hash,omitempty"`
}

type ProjectState struct {
	Goal        string            `json:"goal"`
	ProjectType string            `json:"project_type"`
	Framework   string            `json:"framework"`
	Answers     map[string]string `json:"answers"`
	CreatedAt   time.Time         `json:"created_at"`
}

type Session struct {
	ID            string        `json:"id"`
	ParentID      string        `json:"parent_id,omitempty"`
	ChildrenIDs   []string      `json:"children_ids,omitempty"`
	Model         string        `json:"model"`
	Provider      string        `json:"provider"`
	StartedAt     time.Time     `json:"started_at"`
	MessageCount  int           `json:"message_count"`
	WorkflowPhase WorkflowPhase `json:"workflow_phase"`
	Project       *ProjectState `json:"project,omitempty"`
}

type StreamChunk struct {
	Type             string `json:"type"`
	Delta            string `json:"delta"`
	ThinkingDuration int64  `json:"thinking_duration"`
}

type StreamIterator struct {
	Next  func() (*StreamChunk, error)
	Close func() error
}

type HealthStatus struct {
	Status    string `json:"status"`
	LatencyMs int64  `json:"latency_ms"`
	Error     string `json:"error,omitempty"`
}
