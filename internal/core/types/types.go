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
	PhaseRuntime    WorkflowPhase = "runtime"
	PhaseShip       WorkflowPhase = "ship"
)

// WorkflowMode controls how aggressively the workflow skips phases.
type WorkflowMode string

const (
	ModeAuto   WorkflowMode = "auto"   // classify and choose automatically (default)
	ModeFull   WorkflowMode = "full"   // all 7 active phases: Init->Discuss->Plan->Exec->Verify->Runtime->Ship
	ModeFast   WorkflowMode = "fast"   // skip Plan: Init->Discuss->Exec->Verify->Ship
	ModeDirect WorkflowMode = "direct" // skip Discuss, Plan, Verify: Init->Exec->Ship
)

// IntentResult holds the outcome of LLM-based intent classification.
type IntentResult struct {
	Intent     IntentType      `json:"intent"`
	Complexity ComplexityLevel `json:"complexity"`
	Confidence float64         `json:"confidence"`
	Scope      []string        `json:"scope"`
	Summary    string          `json:"summary"`
}

// WorkflowModeForIntent returns the recommended workflow mode for an intent result.
func WorkflowModeForIntent(ir IntentResult) WorkflowMode {
	switch ir.Intent {
	case IntentChore:
		return ModeDirect
	case IntentFeature, IntentBugfix, IntentRefactor:
		return WorkflowModeForIntentComplexity(ir.Complexity)
	default:
		return ModeFull
	}
}

// WorkflowModeForIntentComplexity maps complexity to workflow mode (intent-aware).
func WorkflowModeForIntentComplexity(c ComplexityLevel) WorkflowMode {
	switch c {
	case ComplexityTrivial:
		return ModeDirect
	case ComplexitySimple:
		return ModeFast
	default:
		return ModeFull
	}
}

// IsWorkflowWorthy returns true if the intent should trigger a structured workflow.
func (ir IntentResult) IsWorkflowWorthy() bool {
	switch ir.Intent {
	case IntentFeature, IntentBugfix, IntentRefactor, IntentChore:
		return true
	default:
		return false
	}
}

type Usage struct {
	PromptTokens     int `json:"prompt_tokens"`
	CompletionTokens int `json:"completion_tokens"`
	TotalTokens      int `json:"total_tokens"`
}

type CapFlags struct {
	Tools     bool `json:"tools"`
	Reasoning bool `json:"reasoning"`
	Vision    bool `json:"vision"`
	Chat      bool `json:"chat"`
}

type Pricing struct {
	InputPerMToken  float64 `json:"input_per_m_token"`
	OutputPerMToken float64 `json:"output_per_m_token"`
}

type ArchInfo struct {
	TokenizerFamily string `json:"tokenizer_family"`
}

type ModelInfo struct {
	ID                string   `json:"id"`
	Provider          string   `json:"provider"`
	Name              string   `json:"name"`
	Description       string   `json:"description"`
	ContextLength     int64    `json:"context_length"`
	MaxOutputTokens   int64    `json:"max_output_tokens"`
	Pricing           Pricing  `json:"pricing"`
	Architecture      ArchInfo `json:"architecture"`
	TopProvider       string   `json:"top_provider"`
	Capabilities      CapFlags `json:"capabilities"`
	SupportedParameters []string `json:"supported_parameters"`
	InputModalities   []string `json:"input_modalities"`
	OutputModalities  []string `json:"output_modalities"`
	Variant           *string  `json:"variant,omitempty"` // nil by default; "thinking", "fast", "extended", "vision"
}

type MessageSegment struct {
	Type       string    `json:"type"`
	Content    string    `json:"content"`
	DurationMs int64     `json:"duration_ms"`
	Visible    bool      `json:"visible"`
	StartedAt  time.Time `json:"started_at,omitempty"`
}

// MessageCompaction is the segment type for auto-compaction summaries.
const MessageCompaction = "compaction"

type ToolCall struct {
	ID    string          `json:"id"`
	Name  string          `json:"name"`
	Input json.RawMessage `json:"input"`
}

type Message struct {
	Role       string           `json:"role"`
	Content    string           `json:"content"`
	Segments   []MessageSegment `json:"segments"`
	ToolCalls  []ToolCall       `json:"tool_calls,omitempty"`
	ToolCallID string           `json:"tool_call_id,omitempty"`
	Usage      *Usage           `json:"usage,omitempty"`
	CreatedAt  time.Time        `json:"created_at"`
	SkipForLLM bool             `json:"skip_for_llm,omitempty"`
}

// MarshalJSON ensures Segments is never serialized as null by initializing
// a nil slice to an empty slice before default marshaling.
func (m Message) MarshalJSON() ([]byte, error) {
	if m.Segments == nil {
		m.Segments = []MessageSegment{}
	}
	type msgAlias Message
	return json.Marshal(msgAlias(m))
}

// ToolError is a structured error type that carries both an error message
// and a hint for LLM self-recovery. Tools should return this when they
// want to provide actionable guidance alongside the error.
type ToolError struct {
	Err  error  // The underlying error message
	Hint string // Actionable hint for the LLM to recover
}

func (e *ToolError) Error() string {
	if e.Hint != "" {
		return e.Err.Error() + "\nHint: " + e.Hint
	}
	return e.Err.Error()
}

func (e *ToolError) Unwrap() error {
	return e.Err
}

// NewToolError creates a new ToolError with an error and hint.
func NewToolError(err error, hint string) *ToolError {
	return &ToolError{Err: err, Hint: hint}
}

// ToolInput represents the input to a tool execution.
type ToolInput struct {
	Name   string         `json:"name"`
	Params map[string]any `json:"params"`
}

// ToolResult represents the result of a tool execution.
type ToolResult struct {
	ToolCallID string `json:"tool_call_id"`
	Output     string `json:"output"`
	Error      string `json:"error,omitempty"`
	DurationMs int64  `json:"duration_ms"`
	Truncated  bool   `json:"truncated"`
}

// HealReport records the outcome of a self-healing attempt.
type HealReport struct {
	TaskID     int       `json:"task_id"`
	Attempt    int       `json:"attempt"`
	Success    bool      `json:"success"`
	ErrorType  string    `json:"error_type"`
	ErrorMsg   string    `json:"error_msg"`
	Strategy   string    `json:"strategy"`
	FilesUsed  []string  `json:"files_used,omitempty"`
	DurationMs int64     `json:"duration_ms"`
	Timestamp  time.Time `json:"timestamp"`
}

type Tool interface {
	Name() string
	Description() string
	RiskLevel() RiskLevel
	Execute(ctx context.Context, input ToolInput) (ToolResult, error)
}

// SchemaProvider is an optional interface that tools can implement to
// provide JSON Schema parameter definitions to the LLM. Tools that
// implement this interface will have their schemas included in tool
// definitions sent to the provider.
type SchemaProvider interface {
	ParameterSchema() string
}

type StreamChunk struct {
	Type             string `json:"type"`
	Delta            string `json:"delta"`
	ThinkingDuration int64  `json:"thinking_duration"`
	Usage            *Usage `json:"usage,omitempty"`
	ToolCallID       string `json:"tool_call_id,omitempty"`
	ToolName         string `json:"tool_name,omitempty"`
	ToolInput        string `json:"tool_input,omitempty"`
	Index            int    `json:"index,omitempty"`
}

type StreamIterator struct {
	Next  func() (*StreamChunk, error)
	Close func() error
}

// StreamChunkMsg is emitted by workflow phases that stream LLM responses
// (currently only the Discuss phase). The TUI renders each chunk in the
// active screen via the REPL streaming infrastructure.
type StreamChunkMsg struct {
	Chunk  *StreamChunk
	Source string // "discuss", "plan", "execute", etc.
}

type HealthStatus struct {
	Status    string `json:"status"`
	LatencyMs int64  `json:"latency_ms"`
	Error     string `json:"error,omitempty"`
}

// DiffSummary holds the diff footprint of a session turn.
type DiffSummary struct {
	Files     []FileDiff `json:"files"`
	Additions int        `json:"additions"`
	Deletions int        `json:"deletions"`
}

// FileDiff holds per-file diff metadata.
type FileDiff struct {
	File      string `json:"file"`
	Status    string `json:"status"` // added, deleted, modified
	Additions int    `json:"additions"`
	Deletions int    `json:"deletions"`
}

// ModelProfile holds per-model parameter overrides.
// Pointer types for numeric/bool fields allow distinguishing "not set" from
// "explicitly set to zero/false" during merging.
type ModelProfile struct {
	ModelID             string   `json:"model_id"`
	Temperature         *float64 `json:"temperature,omitempty"`
	TopP                *float64 `json:"top_p,omitempty"`
	MaxTokens           *int     `json:"max_tokens,omitempty"`
	ReasoningEnabled    *bool    `json:"reasoning_enabled,omitempty"`
	ReasoningBudget     *int     `json:"reasoning_budget,omitempty"`
	ReasoningConfigRef  string   `json:"reasoning_config_ref,omitempty"` // optional reference to named reasoning config in reasoningParamMap
}

// ChatRequest is a chat completion request. Moved here from internal/provider
// to allow pkg/ packages to reference it without importing internal/.
type ChatRequest struct {
	Model            string           `json:"model"`
	Messages         []Message        `json:"messages"`
	MaxTokens        int              `json:"max_tokens,omitempty"`
	Tools            []ToolDefinition `json:"tools,omitempty"`
	ReasoningEnabled bool             `json:"reasoning_enabled,omitempty"`
	Provider         string           `json:"provider,omitempty"` // per-request provider override (D-25)
	Temperature      *float64         `json:"temperature,omitempty"`
	TopP             *float64         `json:"top_p,omitempty"`
}

// HasTemperature returns true if Temperature was explicitly set in the request.
func (r ChatRequest) HasTemperature() bool {
	return r.Temperature != nil
}

// HasTopP returns true if TopP was explicitly set in the request.
func (r ChatRequest) HasTopP() bool {
	return r.TopP != nil
}

// HasMaxTokens returns true if MaxTokens was explicitly set in the request.
func (r ChatRequest) HasMaxTokens() bool {
	return r.MaxTokens != 0
}

// HasReasoningEnabled returns true if ReasoningEnabled was explicitly set in the request.
func (r ChatRequest) HasReasoningEnabled() bool {
	return r.ReasoningEnabled
}

// HasProvider returns true if Provider was explicitly set in the request.
func (r ChatRequest) HasProvider() bool {
	return r.Provider != ""
}

// ChatResponse is a non-streaming chat completion response.
// Returned by LLMProvider.ChatCompletion (which collects stream chunks internally).
type ChatResponse struct {
	Content      string  `json:"content"`
	Usage        *Usage  `json:"usage,omitempty"`
	Model        string  `json:"model"`
	FinishReason string  `json:"finish_reason"`
}

// ToolDefinition describes a tool available to the LLM.
type ToolDefinition struct {
	Name             string `json:"name"`
	Description      string `json:"description"`
	Parameters       string `json:"parameters"`
	ParametersParsed any    `json:"-"` // cached json.Unmarshal result, populated by buildToolDefinitions
}

// QuestionRequest represents a question sent from the tool to the TUI.
type QuestionRequest struct {
	ID          int64
	Question    string
	Header      string
	Options     []string
	AllowCustom bool
	TimeoutSecs int
}

// QuestionResponse represents the user's answer from the TUI.
type QuestionResponse struct {
	Answer string
}