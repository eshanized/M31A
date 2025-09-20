# M31A — Interfaces Reference

> This document mirrors all Go interface and type definitions from `internal/` files
> for LLM context in future OpenCode sessions. The source of truth is always the
> `.go` files. This document is a convenience mirror for quick reference.

> **Last Updated:** 2026-06-06

---

## Package: `internal/types/types.go`

### RiskLevel

```go
type RiskLevel string

const (
    RiskSafe        RiskLevel = "safe"
    RiskMedium      RiskLevel = "medium"
    RiskDangerous   RiskLevel = "dangerous"
    RiskDestructive RiskLevel = "destructive"
)
```

### WorkflowPhase

```go
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
```

### TaskStatus

```go
type TaskStatus string

const (
    StatusPending       TaskStatus = "pending"
    StatusRunning       TaskStatus = "running"
    StatusDone          TaskStatus = "done"
    StatusFailed        TaskStatus = "failed"
    StatusSkipped       TaskStatus = "skipped"
    StatusUnrecoverable TaskStatus = "unrecoverable"
)
```

### Usage

```go
type Usage struct {
    PromptTokens     int `json:"prompt_tokens"`
    CompletionTokens int `json:"completion_tokens"`
    TotalTokens      int `json:"total_tokens"`
}
```

### CapFlags

```go
type CapFlags struct {
    Tools     bool `json:"tools"`
    Reasoning bool `json:"reasoning"`
    Vision    bool `json:"vision"`
}
```

### Pricing

```go
type Pricing struct {
    InputPerMToken  float64 `json:"input_per_m_token"`
    OutputPerMToken float64 `json:"output_per_m_token"`
}
```

### ArchInfo

```go
type ArchInfo struct {
    TokenizerFamily string `json:"tokenizer_family"`
}
```

### ModelInfo

```go
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
	Variant       *string  `json:"variant,omitempty"` // nil by default; "thinking", "fast", "extended", "vision"
}
```

### MessageSegment

```go
type MessageSegment struct {
	Type       string    `json:"type"`
	Content    string    `json:"content"`
	DurationMs int64     `json:"duration_ms"`
	Visible    bool      `json:"visible"`
	StartedAt  time.Time `json:"started_at,omitempty"`
}
```

### ToolCall

```go
type ToolCall struct {
    ID    string          `json:"id"`
    Name  string          `json:"name"`
    Input json.RawMessage `json:"input"`
}
```

### Message

```go
type Message struct {
	Role       string           `json:"role"`
	Content    string           `json:"content"`
	Segments   []MessageSegment `json:"segments"`
	ToolCalls  []ToolCall       `json:"tool_calls,omitempty"`
	Usage      *Usage           `json:"usage,omitempty"`
	CreatedAt  time.Time        `json:"created_at"`
	SkipForLLM bool             `json:"skip_for_llm,omitempty"`
}
```

### ToolInput

```go
type ToolInput struct {
    Name   string         `json:"name"`
    Params map[string]any `json:"params"`
}
```

### ToolResult

```go
type ToolResult struct {
    ToolCallID string `json:"tool_call_id"`
    Output     string `json:"output"`
    Error      string `json:"error,omitempty"`
    DurationMs int64  `json:"duration_ms"`
    Truncated  bool   `json:"truncated"`
}
```

### Tool Interface

```go
type Tool interface {
    Name() string
    Description() string
    RiskLevel() RiskLevel
    Execute(ctx context.Context, input ToolInput) (ToolResult, error)
}
```

### SchemaProvider

```go
// SchemaProvider is an optional interface that tools can implement to
// provide JSON Schema parameter definitions to the LLM.
type SchemaProvider interface {
    ParameterSchema() string
}
```

### FilePrediction

```go
type FilePrediction struct {
	Path           string `json:"path"`
	Action         string `json:"action"`
	EstimatedLines int    `json:"estimated_lines"`
}

// FilePrediction action constants
const (
	FileActionCreate = "create"
	FileActionModify = "modify"
	FileActionDelete = "delete"
)
```

### Task

```go
type Task struct {
    ID                 int              `json:"id"`
    Description        string           `json:"description"`
    Action             string           `json:"action"`
    Dependencies       []int            `json:"dependencies"`
    Files              []string         `json:"files"`
    AcceptanceCriteria []string         `json:"acceptance_criteria"`
    Status             TaskStatus       `json:"status"`
    HealsAttempted     int              `json:"heals_attempted"`
    PredictedFiles     []FilePrediction `json:"predicted_files,omitempty"`
    CommitHash         string           `json:"commit_hash,omitempty"`
}
```

### ProjectState

```go
type ProjectState struct {
    Goal        string            `json:"goal"`
    ProjectType string            `json:"project_type"`
    Framework   string            `json:"framework"`
    Answers     map[string]string `json:"answers"`
    CreatedAt   time.Time         `json:"created_at"`
}
```

### Session

```go
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
```

### StreamChunk

```go
type StreamChunk struct {
	Type             string `json:"type"`
	Delta            string `json:"delta"`
	ThinkingDuration int64  `json:"thinking_duration"`
	Usage            *Usage `json:"usage,omitempty"`
}
```

### StreamIterator

```go
type StreamIterator struct {
    Next  func() (*StreamChunk, error)
    Close func() error
}
```

### StreamChunkMsg

```go
// StreamChunkMsg is emitted by workflow phases that stream LLM responses.
type StreamChunkMsg struct {
    Chunk  *StreamChunk
    Source string // "discuss", "plan", "execute", etc.
}
```

### HealthStatus

```go
type HealthStatus struct {
    Status    string `json:"status"`
    LatencyMs int64  `json:"latency_ms"`
    Error     string `json:"error,omitempty"`
}
```

---

## Package: `internal/provider/interface.go`

### LLMProvider

```go
type LLMProvider interface {
    Name() string
    FetchModels(ctx context.Context) ([]types.ModelInfo, error)
    ChatCompletionStream(ctx context.Context, req ChatRequest) (*types.StreamIterator, error)
    EstimateCost(usage types.Usage) float64
    HealthCheck(ctx context.Context) types.HealthStatus
    GetModel(id string) (*types.ModelInfo, error)
}
```

### ChatRequest

```go
type ChatRequest struct {
    Model            string           `json:"model"`
    Messages         []types.Message  `json:"messages"`
    MaxTokens        int              `json:"max_tokens,omitempty"`
    Stream           bool             `json:"stream"`
    Tools            []ToolDefinition `json:"tools,omitempty"`
    ReasoningEnabled bool             `json:"reasoning_enabled,omitempty"`
}
```

### ToolDefinition

```go
type ToolDefinition struct {
    Name        string `json:"name"`
    Description string `json:"description"`
    Parameters  string `json:"parameters"`
}
```

### ProviderRegistry

```go
type ProviderRegistry struct {
    Active    func() string
    SetActive func(name string) error
    Get       func(name string) (LLMProvider, error)
    List      func() []string
}
```

---

## Package: `internal/tools/interface.go`

### PermissionRequest

```go
type PermissionRequest struct {
    ToolName    string          `json:"tool_name"`
    Command     string          `json:"command"`
    RiskLevel   types.RiskLevel `json:"risk_level"`
    TimeoutSecs int             `json:"timeout_secs"`
}
```

### PermissionResponse

```go
type PermissionResponse struct {
    Allowed  bool `json:"allowed"`
    Remember bool `json:"remember"`
}
```

### Dispatcher

```go
type Dispatcher struct {
    Register func(name string, tool types.Tool)
    Execute  func(ctx context.Context, call types.ToolCall) (types.ToolResult, error)
    List     func() []string
}
```

---

## Package: `internal/config/types.go`

### Config

```go
type Config struct {
    Provider    ProviderConfig    `toml:"provider"`
    Model       ModelConfig       `toml:"model"`
    UI          UIConfig          `toml:"ui"`
    Permissions PermissionsConfig `toml:"permissions"`
    Features    FeaturesConfig    `toml:"features"`
    Ledger      LedgerConfig      `toml:"ledger"`
    Ghost       GhostConfig       `toml:"ghost"`
}
```

### ProviderConfig

```go
type ProviderConfig struct {
    Default      string                   `toml:"default"`
    AutoFallback bool                     `toml:"auto_fallback"`
    OpenRouter   ProviderCredentialConfig `toml:"openrouter"`
    Zen          ProviderCredentialConfig `toml:"zen"`
}
```

### ProviderCredentialConfig

```go
type ProviderCredentialConfig struct {
    APIKey string `toml:"api_key"`
}
```

### ModelConfig

```go
type ModelConfig struct {
    Default                 string  `toml:"default"`
    ContextWarningThreshold float64 `toml:"context_warning_threshold"`
    ShowThinkingByDefault   bool    `toml:"show_thinking_by_default"`
    AutoCollapseTools       bool    `toml:"auto_collapse_tools"`
    AutoArbitrage           bool    `toml:"auto_arbitrage"`
    ArbitrageThreshold      float64 `toml:"arbitrage_threshold"`
}
```

### UIConfig

```go
type UIConfig struct {
    Theme            string `toml:"theme"`
    CompactMode      bool   `toml:"compact_mode"`
    ShowTokenUsage   bool   `toml:"show_token_usage"`
    ShowCostEstimate bool   `toml:"show_cost_estimate"`
    MaxIterations    int    `toml:"max_iterations"`
}
```

### PermissionsConfig

```go
type PermissionsConfig struct {
    DefaultMode    string           `toml:"default_mode"`
    TimeoutSeconds int              `toml:"timeout_seconds"`
    Rules          []PermissionRule `toml:"rules"`
}
```

### PermissionRule

```go
type PermissionRule struct {
    Tool      string          `toml:"tool"`
    Pattern   string          `toml:"pattern"`
    RiskLevel types.RiskLevel `toml:"risk_level"`
    Action    string          `toml:"action"`
}
```

### FeaturesConfig

```go
type FeaturesConfig struct {
    AutodreamEnabled bool `toml:"autodream_enabled"`
    SubagentEnabled  bool `toml:"subagent_enabled"`
    AutoBackup       bool `toml:"auto_backup"`
    ResumeOnStartup  bool `toml:"resume_on_startup"`
}
```

### LedgerConfig

```go
type LedgerConfig struct {
    Enabled    bool `toml:"enabled"`
    MaxEntries int  `toml:"max_entries"`
}
```

### GhostConfig

```go
type GhostConfig struct {
    Enabled bool `toml:"enabled"`
}
```

---

## Package: `internal/types/constants.go`

```go
const (
    ModelCacheTTL             = 5 * time.Minute
    HealthCheckInterval       = 60 * time.Second
    MaxFileSize               = 5 * 1024 * 1024
    MaxToolOutputChars        = 10_000
    MaxHealAttempts           = 2
    MaxPlanRetries            = 3
    SessionIDLength           = 8
    AutoDreamThreshold        = 0.60
    ContextWarningThreshold   = 0.80
    HTTPDialTimeout           = 30 * time.Second
    BashTimeout               = 30 * time.Minute
    BashOutputLimit           = 50_000
    DefaultContextLength      = 128_000
)
```

---

## Package: `internal/errors/errors.go`

```go
var (
	ErrProviderUnreachable  = errors.New("provider unreachable")
	ErrProviderNotFound     = errors.New("provider not found")
	ErrInvalidProvider      = errors.New("invalid provider name")
	ErrRateLimited          = errors.New("rate limited")
	ErrInvalidKey           = errors.New("invalid API key")
	ErrNoCredits            = errors.New("no credits available")
	ErrContextExceeded      = errors.New("context window exceeded")
	ErrModelNotFound        = errors.New("model not found")
	ErrSessionCorrupted     = errors.New("session data corrupted")
	ErrNoBinaryContent      = errors.New("binary content not displayable")
	ErrFileTooLarge         = errors.New("file exceeds 5MB limit")
	ErrCircularDependency   = errors.New("circular dependency in task graph")
	ErrPermissionDenied     = errors.New("permission denied")
	ErrToolExecution        = errors.New("tool execution failed")
	ErrTaskFailed           = errors.New("task failed")
	ErrPhaseTransition      = errors.New("invalid phase transition")
	ErrCheckpointNotFound   = errors.New("checkpoint not found")
	ErrToolInputTooLarge    = errors.New("tool input exceeds size limit")
	ErrInvalidTimeout       = errors.New("invalid timeout: must be > 0 and <= 30m")
	ErrPrivateIPBlocked     = errors.New("access to private IP is blocked (SSRF protection)")
	ErrStreamTruncated      = errors.New("stream truncated before completion")
	ErrBisectResetFailed    = errors.New("bisect reset failed")
	ErrBisectFailed         = errors.New("bisect failed")
	ErrSessionNotFound      = errors.New("session not found")
	ErrSessionPermission    = errors.New("session access denied")
)
```
