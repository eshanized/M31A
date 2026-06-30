# Interface Reference

Key interfaces and their implementations across M31 Autonomous.

---

## LLMProvider

Defined in `internal/provider/interface.go`.

```go
type LLMProvider interface {
    Name() string
    APIKey() string
    FetchModels(ctx context.Context) ([]types.ModelInfo, error)
    CachedModels() []types.ModelInfo
    ChatCompletionStream(ctx context.Context, req ChatRequest) (*types.StreamIterator, error)
    EstimateCost(modelID string, usage types.Usage) float64
    HealthCheck(ctx context.Context) types.HealthStatus
    GetModel(id string) (*types.ModelInfo, error)
}
```

**Implementations:**

| Implementation | Package | Provider |
|---|---|---|
| `Client` | `internal/provider/openrouter` | OpenRouter API |
| `Client` | `internal/provider/zen` | Zen/OpenCode API |
| `Client` | `internal/provider/nvidia` | Nvidia NIM API |

---

## ChatRequest

Defined in `internal/provider/interface.go`.

```go
type ChatRequest struct {
    Model            string           `json:"model"`
    Messages         []types.Message  `json:"messages"`
    MaxTokens        int              `json:"max_tokens,omitempty"`
    Tools            []ToolDefinition `json:"tools,omitempty"`
    ReasoningEnabled bool             `json:"reasoning_enabled,omitempty"`
}
```

---

## ToolDefinition

Defined in `internal/provider/interface.go`.

```go
type ToolDefinition struct {
    Name             string `json:"name"`
    Description      string `json:"description"`
    Parameters       string `json:"parameters"`
    ParametersParsed any    `json:"-"`
}
```

---

## BaseClient

Embedded by all provider clients. Defined in `internal/provider/base_client.go`.

Provides shared HTTP transport, model lookup, cost estimation, and stream iterator creation.

---

## ModelCache

Defined in `internal/provider/cache.go`.

Thread-safe in-memory cache with TTL (5 min fresh, 24h stale) and singleflight deduplication.

---

## SSEParser

Defined in `internal/provider/sse.go`.

Server-Sent Events parser with watchdog timer for streaming responses.

---

## Tool Interface

Defined in `internal/types/types.go`.

```go
type Tool interface {
    Name() string
    Description() string
    Execute(ctx context.Context, input map[string]any) (string, error)
    ParameterSchema() string
}

type SchemaProvider interface {
    ParameterSchema() string
}
```

---

## Keychain

Defined in `pkg/keychain/keychain.go`.

```go
type Keychain interface {
    Get(service string) (string, error)
    Set(service, key string) error
    Delete(service string) error
}
```

**Implementations:** Linux (D-Bus Secret Service, `pass`), macOS (`/usr/bin/security`), Windows (Credential Manager)

---

## Config

Defined in `internal/config/types.go`.

```go
type Config struct {
    Provider     ProviderConfig
    Model        ModelConfig
    UI           UIConfig
    Permissions  PermissionsConfig
    Features     FeaturesConfig
    Ledger       LedgerConfig
    Tools        ToolsConfig
    Agents       AgentsConfig
    Git          GitConfig
    Verify       VerifyConfig
    Compaction   CompactionConfig
    Instructions InstructionsConfig
    Skills       SkillsConfig
}
```

Functions:
```go
func Load(path string) (*Config, error)
func DefaultConfig() *Config
func WatchConfig(ctx context.Context, path string, ch chan<- ConfigReloadMsg)
func LoadDotEnv()
func (c *Config) Save(path string) error
func (c *Config) SaveWithKeychain(path string, kc keychain.Keychain) error
func (c *Config) ResolveAPIKeys(kc keychain.Keychain) error
```

---

## WorkflowEngine

Defined in `internal/tui/tuitypes/tuitypes.go`.

```go
type WorkflowEngine interface {
    RunPhase(ctx context.Context, phase types.WorkflowPhase, goal string) (*workflow.PhaseResult, error)
    Transition(ctx context.Context, from, to types.WorkflowPhase) error
    SetModel(modelID string, p provider.LLMProvider)
    SetPhaseModel(phase types.WorkflowPhase, modelID string)
    SetMsgEmitter(em workflow.MsgEmitter)
    SetSessionID(id string)
    SetGit(g *git.Git)
    SessionID() string
    HealTask(ctx context.Context, taskID int) (bool, error)
    SubmitDiscussAnswer(index int, answer string) error
    FinalizeDiscuss() error
    SkipDiscuss() error
    DiscussState() workflow.DiscussState
    PlanContent() string
    PlanVersion() int
    SetRefinementFeedback(feedback string)
    SetWorkflowMode(mode types.WorkflowMode)
    WorkflowMode() types.WorkflowMode
    SnapshotDecisions() []decision.DecisionReceipt
    Close()
    LoadCheckpointData(data *workflow.CheckpointData)
    GetCheckpointData() *workflow.CheckpointData
}
```

---

## AppState

Defined in `internal/tui/app.go`.

Top-level Bubble Tea model containing all TUI state. Implements `tea.Model`:

```go
func (m AppState) Init() tea.Cmd
func (m AppState) Update(msg tea.Msg) (tea.Model, tea.Cmd)
func (m AppState) View() string
```

---

## Workflow Phases

Defined in `internal/types/types.go`.

```go
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
```

---

## Workflow Modes

```go
type WorkflowMode string

const (
    ModeAuto   WorkflowMode = "auto"
    ModeFull   WorkflowMode = "full"
    ModeFast   WorkflowMode = "fast"
    ModeDirect WorkflowMode = "direct"
)
```

---

## Permission Interfaces

```go
type PermissionResult struct {
    Allowed bool
    Reason  string
    Risk    types.RiskLevel
}
```
