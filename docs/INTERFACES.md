# Interface Reference

Key interfaces and their implementations across M31 Autonomous.

---

## LLMProvider

Defined in `internal/provider/provider.go`.

```go
type LLMProvider interface {
    Name() string
    ListModels(ctx context.Context) ([]types.ModelInfo, error)
    ChatCompletion(ctx context.Context, req types.ChatRequest) (*types.ChatResponse, error)
    ChatCompletionStream(ctx context.Context, req types.ChatRequest) (*types.StreamIterator, error)
    HealthCheck(ctx context.Context) (*types.HealthReport, error)
    EstimateCost(modelID string, usage types.Usage) float64
    GetModel(id string) (*types.ModelInfo, error)
    CachedModels() []types.ModelInfo
    APIKey() string
    FetchModels(ctx context.Context) ([]types.ModelInfo, error)
}
```

**Implementations:**
| Implementation | Package | Provider |
|---|---|---|
| `Client` | `internal/provider/openrouter` | OpenRouter API |
| `Client` | `internal/provider/zen` | Barret AI / Zen API |

---

## BaseClient

Embedded by all provider clients. Defined in `internal/provider/provider.go`/`base_client.go`.

```go
type BaseClient struct {
    APIKeyField  string
    BaseURLField string
    HTTPClient   *http.Client
    Cache        *ModelCache
    HealthLiveMs int64
    HealthSlowMs int64
    Version      string
}

func NewBaseClient(apiKey, baseURL, version string, cacheTTL, cacheStaleTTL time.Duration, healthLiveMs, healthSlowMs int64) BaseClient
func (b *BaseClient) APIKey() string                          // Masked key display
func (b *BaseClient) EstimateCost(modelID string, usage types.Usage) float64
func (b *BaseClient) GetModel(id string) (*types.ModelInfo, error)
func (b *BaseClient) CachedModels() []types.ModelInfo
func (b *BaseClient) MakeIterator(sse *SSEParser, modelID string) *types.StreamIterator
```

---

## ModelCache

Provider-agnostic in-memory cache. Defined in `internal/provider/cache.go`.

```go
type ModelCache struct {
    mu       sync.RWMutex
    models   map[string]cacheEntry
    ttl      time.Duration
    staleTTL time.Duration
}

func NewModelCache() *ModelCache
func NewModelCacheWithStale(ttl, staleTTL time.Duration) *ModelCache
func (c *ModelCache) Get(id string) (*types.ModelInfo, error)
func (c *ModelCache) Set(id string, model types.ModelInfo)
func (c *ModelCache) All() []types.ModelInfo
func (c *ModelCache) Refresh() error
```

---

## StreamIterator

Chunk-by-chunk streaming wrapper. Defined in `internal/types/types.go`.

```go
type StreamIterator struct {
    Next func() (*types.StreamChunk, error)
}
```

---

## SSEParser

Server-Sent Events parser for streaming responses. Defined in `internal/provider/sse.go`.

```go
type SSEParser struct {
    scanner *bufio.Scanner
    event   string
    data    strings.Builder
    done    bool
}

func NewSSEParser(r io.Reader) *SSEParser
func (s *SSEParser) Next() (event string, data []byte, err error)
```

---

## Tool Interfaces

Defined in `internal/tools/`.

```go
// ToolExecutor
func ExecuteTool(ctx context.Context, name string, args map[string]interface{}) (string, error)

// ToolRegistry
type ToolRegistry struct{}

func NewToolRegistry() *ToolRegistry
func (r *ToolRegistry) Register(tool types.ToolDef, handler func(context.Context, map[string]interface{}) (string, error))
func (r *ToolRegistry) List() []types.ToolDef
func (r *ToolRegistry) Lookup(name string) (types.ToolDef, func(context.Context, map[string]interface{}) (string, error), bool)
func (r *ToolRegistry) Execute(ctx context.Context, name string, args map[string]interface{}) (string, error)
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
    Provider    ProviderConfig
    Model       ModelConfig
    UI          UIConfig
    Permissions PermissionsConfig
    Features    FeaturesConfig
    Ledger      LedgerConfig
    Tools       ToolsConfig
    Agents      AgentsConfig
    Git         GitConfig
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
func (c *Config) SaveProject(path string) error
```

---

## App (Bubble Tea Model)

Defined in `internal/app/`.

```go
type AppModel struct {
    state       AppState    // Current UI state
    config      *config.Config
    provider    *provider.Manager
    session     *types.Session
    viewport    tea.Model
    input       tea.Model
    messages    []types.Message
    suggestions []string    // Autocomplete suggestions
    ready       bool
    width, height int
    err         error
}
```

Implements `tea.Model`:
```go
func (m AppModel) Init() tea.Cmd
func (m AppModel) Update(msg tea.Msg) (tea.Model, tea.Cmd)
func (m AppModel) View() string
```

State machine:
```
Init → Ready (healthy) | Error (startup failure)
Ready → Processing (submit)
Processing → Streaming (response started)
Streaming → Ready (stream complete)
Streaming → Paused (interrupt) → Ready
Any → ConfirmQuit → Quit / resume
Ready → SessionPicker | GhostPicker | GhostOutput | BisectOutput
```

---

## AppState

Defined in `internal/app/states.go`.

```go
type AppState int

const (
    AppStateInit AppState = iota
    AppStateReady
    AppStateProcessing
    AppStateStreaming
    AppStatePaused
    AppStateError
    AppStateConfirmQuit
    AppStateSessionPicker
    AppStateGhostPicker
    AppStateGhostOutput
    AppStateBisectOutput
)
```

---

## Cmd / Handler

```go
type Cmd func() tea.Msg

type Handler func(ctx context.Context, msg tea.Msg) (tea.Msg, error)
```

---

## ConfigReloadMsg

Emitted when config file changes on disk.

```go
type ConfigReloadMsg struct {
    Config *Config
    Error  error
}
```

---

## Permission Interfaces

```go
type PermissionChecker interface {
    Check(toolName string, args map[string]interface{}) (PermissionResult, error)
}

type PermissionResult struct {
    Allowed bool
    Reason  string
    Risk    types.RiskLevel
}
```
