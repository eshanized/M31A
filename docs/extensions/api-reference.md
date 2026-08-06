# pkg/extensions API Reference

**Package:** `github.com/eshanized/M31A/pkg/extensions`

Public interfaces and types for building M31A extensions.

## Interfaces

### ExternalTool

Tool extensions implement this interface. Registered via config and invoked through the Dispatcher.

```go
type ExternalTool interface {
    Name() string
    Description() string
    RiskLevel() types.RiskLevel
    ParameterSchema() string
    Execute(ctx context.Context, input types.ToolInput) (types.ToolResult, error)
}
```

| Method | Description |
|--------|-------------|
| `Name()` | Unique tool name (e.g., "custom-linter") |
| `Description()` | Human-readable description |
| `RiskLevel()` | Permission gating: `safe`, `caution`, or `dangerous` |
| `ParameterSchema()` | JSON Schema string for tool parameters |
| `Execute()` | Runs the tool with given input |

**Implementation:** Use `NewExternalToolAdapter(name, procManager)` which implements this interface by wrapping a `SubprocessManager`.

### ExternalProvider

Provider extensions implement this interface. Registered via config and managed by the Provider Registry.

```go
type ExternalProvider interface {
    Name() string
    APIKey() string
    FetchModels(ctx context.Context) ([]types.ModelInfo, error)
    CachedModels() []types.ModelInfo
    ChatCompletionStream(ctx context.Context, req types.ChatRequest) (*types.StreamIterator, error)
    EstimateCost(modelID string, usage types.Usage) float64
    HealthCheck(ctx context.Context) types.HealthStatus
    GetModel(id string) (*types.ModelInfo, error)
}
```

| Method | Description |
|--------|-------------|
| `Name()` | Unique provider name (e.g., "ollama") |
| `APIKey()` | Returns empty string for external providers |
| `FetchModels(ctx)` | Fetches available models from provider |
| `CachedModels()` | Returns cached models without fetching |
| `ChatCompletionStream(ctx, req)` | Streaming chat completion via JSON-RPC notifications |
| `EstimateCost(modelID, usage)` | Estimates cost in USD |
| `HealthCheck(ctx)` | Returns `types.HealthStatus` |
| `GetModel(id)` | Returns specific model by ID |

**Implementation:** Use `NewExternalProviderAdapter(name, procManager)`.

### PhaseHookHandler

Workflow phase hook extensions implement this interface. Invoked at pre/post phase transitions.

```go
type PhaseHookHandler interface {
    PrePhase(ctx context.Context, payload PhaseHookPayload) error
    PostPhase(ctx context.Context, payload PhaseHookPayload, result *PhaseResult) error
}
```

| Method | Description |
|--------|-------------|
| `PrePhase(ctx, payload)` | Called before a workflow phase begins |
| `PostPhase(ctx, payload, result)` | Called after a workflow phase completes |

**Implementation:** Use `NewPhaseHookAdapter(name, procManager, emitter)`.

## Data Types

### PhaseHookPayload

Carries context to phase hook handlers.

```go
type PhaseHookPayload struct {
    PhaseName       types.WorkflowPhase      `json:"phase_name"`
    WorkflowState   WorkflowStateSnapshot    `json:"workflow_state"`
    Context         context.Context          `json:"-"`
    ExtensionConfig []byte                   `json:"extension_config,omitempty"`
}
```

### WorkflowStateSnapshot

Captures workflow state for hooks.

```go
type WorkflowStateSnapshot struct {
    CurrentPhase    types.WorkflowPhase `json:"current_phase"`
    Goal            string              `json:"goal"`
    Tasks           []types.Task        `json:"tasks"`
    Messages        []types.Message     `json:"messages,omitempty"`
    SessionID       string              `json:"session_id"`
    BudgetSpentUSD  float64             `json:"budget_spent_usd"`
}
```

### PhaseResult

Represents the result of a workflow phase execution.

```go
type PhaseResult struct {
    Phase       types.WorkflowPhase `json:"phase"`
    Success     bool                `json:"success"`
    Error       string              `json:"error,omitempty"`
    Output      any                 `json:"output,omitempty"`
}
```

## Config Structures

### ExtensionsConfig

Top-level extension configuration.

```go
type ExtensionsConfig struct {
    Tools     map[string]ExternalToolConfig     `toml:"tools" json:"tools"`
    Providers map[string]ExternalProviderConfig `toml:"providers" json:"providers"`
    Hooks     map[string]PhaseHookConfig        `toml:"hooks" json:"hooks"`
}
```

### ExternalToolConfig

```go
type ExternalToolConfig struct {
    Command string            `toml:"command" json:"command"`
    Args    []string          `toml:"args" json:"args"`
    Env     map[string]string `toml:"env" json:"env"`
    Timeout string            `toml:"timeout" json:"timeout"`
}
```

| Field | Description |
|-------|-------------|
| `Command` | Path to extension executable (absolute or in PATH) |
| `Args` | Command-line arguments |
| `Env` | Environment variables for subprocess |
| `Timeout` | Max execution duration (e.g., "30s") |

### ExternalProviderConfig

```go
type ExternalProviderConfig struct {
    Command string            `toml:"command" json:"command"`
    Args    []string          `toml:"args" json:"args"`
    Env     map[string]string `toml:"env" json:"env"`
    Timeout string            `toml:"timeout" json:"timeout"`
}
```

### PhaseHookConfig

```go
type PhaseHookConfig struct {
    Command     string            `toml:"command" json:"command"`
    Args        []string          `toml:"args" json:"args"`
    Env         map[string]string `toml:"env" json:"env"`
    Phases      []string          `toml:"phases" json:"phases"`
    HookTypes   []string          `toml:"hook_types" json:"hook_types"`
    Timeout     string            `toml:"timeout" json:"timeout"`
}
```

| Field | Description |
|-------|-------------|
| `Command` | Path to extension executable |
| `Args` | Command-line arguments |
| `Env` | Environment variables for subprocess |
| `Phases` | Workflow phases: `initialize`, `discuss`, `plan`, `execute`, `verify`, `runtime`, `ship` |
| `HookTypes` | `pre`, `post`, or both |
| `Timeout` | Max hook execution duration (e.g., "30s") |

## JSON-RPC 2.0 Protocol

All communication uses JSON-RPC 2.0 over stdin/stdout.

### Request

```go
type JSONRPCRequest struct {
    JSONRPC string          `json:"jsonrpc"` // always "2.0"
    ID      json.RawMessage `json:"id"`
    Method  string          `json:"method"`
    Params  json.RawMessage `json:"params,omitempty"`
}
```

### Response

```go
type JSONRPCResponse struct {
    JSONRPC string          `json:"jsonrpc"`
    ID      json.RawMessage `json:"id"`
    Result  json.RawMessage `json:"result,omitempty"`
    Error   *JSONRPCError   `json:"error,omitempty"`
}
```

### Error Codes

| Code | Constant | Description |
|------|----------|-------------|
| -32700 | `ParseError` | Invalid JSON received |
| -32600 | `InvalidRequest` | Not a valid Request object |
| -32601 | `MethodNotFound` | Method does not exist |
| -32602 | `InvalidParams` | Invalid method parameters |
| -32603 | `InternalError` | Internal JSON-RPC error |

### Standard Methods

#### Tool Methods

| Constant | Method | Description |
|----------|--------|-------------|
| `MethodToolName` | `tool.name` | Returns tool name |
| `MethodToolDescription` | `tool.description` | Returns tool description |
| `MethodToolRiskLevel` | `tool.risk_level` | Returns risk level |
| `MethodToolSchema` | `tool.schema` | Returns parameter schema |
| `MethodToolExecute` | `tool.execute` | Executes tool with input |

#### Provider Methods

| Constant | Method | Description |
|----------|--------|-------------|
| `MethodProviderName` | `provider.name` | Returns provider name |
| `MethodProviderFetchModels` | `provider.fetch_models` | Returns available models |
| `MethodProviderChatCompletion` | `provider.chat_completion_stream` | Streaming chat completion |
| `MethodProviderEstimateCost` | `provider.estimate_cost` | Estimates request cost |
| `MethodProviderHealthCheck` | `provider.health_check` | Checks provider health |
| `MethodProviderGetModel` | `provider.get_model` | Returns specific model |

#### Hook Methods

| Constant | Method | Description |
|----------|--------|-------------|
| `MethodHookPrePhase` | `hook.pre_phase` | Called before phase |
| `MethodHookPostPhase` | `hook.post_phase` | Called after phase |

#### Lifecycle Methods

| Constant | Method | Description |
|----------|--------|-------------|
| `MethodHandshake` | `handshake` | Protocol version negotiation |
| `MethodShutdown` | `shutdown` | Graceful shutdown notification |

### Handshake

On startup, M31A sends a handshake request:

```json
{"jsonrpc":"2.0","id":1,"method":"handshake"}
```

Expected response:

```json
{"jsonrpc":"2.0","id":1,"result":{"protocol_version":"1.0","supported_methods":["tool.name","tool.execute",...]}}
```

Protocol version `1.0` is required. Extensions must declare supported methods.

## SubprocessManager

Manages extension subprocess lifecycle.

```go
func NewSubprocessManager(cmd string, args []string, env map[string]string, timeout time.Duration) *SubprocessManager
func (m *SubprocessManager) Start(ctx context.Context) error
func (m *SubprocessManager) Call(ctx context.Context, req JSONRPCRequest, timeout time.Duration) (JSONRPCResponse, error)
func (m *SubprocessManager) Stop() error
func (m *SubprocessManager) IsRunning() bool
func (m *SubprocessManager) ProtocolVersion() string
func (m *SubprocessManager) SupportedMethods() []string
func (m *SubprocessManager) SetWorkDir(dir string)
```

- `Start()` spawns process, opens pipes, starts readers, performs handshake
- `Call()` sends JSON-RPC request, waits for response with timeout
- `Stop()` sends shutdown notification, waits with timeout, force kills if needed
- Uses `sync.Once` for idempotent `Stop()`

## ExtensionRegistry

Central registry managing all extensions.

```go
func NewExtensionRegistry(cfg *ExtensionsConfig, emitter MsgEmitter) *ExtensionRegistry
func (r *ExtensionRegistry) Start(ctx context.Context) error
func (r *ExtensionRegistry) Stop() error
func (r *ExtensionRegistry) GetTool(name string) (ExternalTool, bool)
func (r *ExtensionRegistry) GetProvider(name string) (ExternalProvider, bool)
func (r *ExtensionRegistry) GetHooks(phase types.WorkflowPhase) []PhaseHookHandler
func (r *ExtensionRegistry) GetPreHooks(phase types.WorkflowPhase) []PhaseHookHandler
func (r *ExtensionRegistry) GetPostHooks(phase types.WorkflowPhase) []PhaseHookHandler
func (r *ExtensionRegistry) GetToolConfigs() map[string]ExternalToolConfig
func (r *ExtensionRegistry) GetProviderConfigs() map[string]ExternalProviderConfig
func (r *ExtensionRegistry) GetHookConfigs() map[string]PhaseHookConfig
func (r *ExtensionRegistry) IsStarted() bool
```

## Adapter Constructors

```go
func NewExternalToolAdapter(name string, proc *SubprocessManager) *ExternalToolAdapter
func NewExternalProviderAdapter(name string, proc *SubprocessManager) *ExternalProviderAdapter
func NewPhaseHookAdapter(name string, proc *SubprocessManager, emitter MsgEmitter) *PhaseHookAdapter
```

## Config Integration

Add to `m31a.json` (project) or `~/.m31a/config.toml` (global):

```json
{
  "extensions": {
    "tools": {
      "my-tool": {
        "command": "/path/to/my-tool",
        "args": ["--flag"],
        "env": {"MY_VAR": "value"},
        "timeout": "30s"
      }
    },
    "providers": {
      "my-provider": {
        "command": "/path/to/my-provider",
        "args": ["--model", "my-model"],
        "env": {"API_HOST": "http://localhost:8080"},
        "timeout": "120s"
      }
    },
    "hooks": {
      "my-hook": {
        "command": "/path/to/my-hook",
        "args": ["--check"],
        "phases": ["execute", "verify"],
        "hook_types": ["pre"],
        "timeout": "30s"
      }
    }
  }
}
```

## Version Compatibility

- Protocol version negotiated via `handshake` method
- Major version = breaking changes (method signatures, protocol)
- Minor version = new methods, backward compatible
- Extensions must declare `supported_methods` in handshake
- M31A rejects extensions with incompatible `protocol_version`