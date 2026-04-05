# M31A Architecture

## Overview

M31A is a modular AI agent framework built in Go with a Bubble Tea TUI. It routes user prompts through a **six-phase workflow** (`session → auto → dream → bisect → arbitrage → rollback`), selecting models by cost/quality, streaming responses, and maintaining full session history.

---

## Directory Layout

```
.
├── cmd/m31a/              # Main entry point (flag parsing, config init)
├── docs/                  # User-facing documentation
├── internal/
│   ├── app/               # App lifecycle, Bubble Tea TUI, views
│   ├── commands/          # Slash command registry + built-in commands
│   ├── config/            # YAML config loading, validation, defaults
│   ├── errors/            # Sentinel errors for the entire app
│   ├── ghost/             # Ghost-written append-only file generation
│   ├── provider/          # LLM provider abstraction (OpenRouter, Zen)
│   ├── style/             # Lipgloss TUI styling
│   ├── tools/             # Tool registry, MCP client, Toolhouse, Brave Search
│   ├── types/             # Core type definitions
│   ├── ui/                # TUI components (spinner, text input, etc.)
│   ├── verify/            # Verification — attestation, validation, MCP mocks
│   └── version/           # Version info from ldflags
├── pkg/
│   ├── arbitrage/         # Model scoring, cost estimation, recommendation
│   ├── autodream/         # Prompt enhancement (DREAM → enhanced prompt)
│   ├── bisect/            # Response comparison / diffing
│   ├── keychain/          # Encrypted API key storage via OS keychain
│   ├── ledger/            # Auditable prompt/response log
│   ├── rollback/          # Session state snapshot & restore
│   └── session/           # Session lifecycle, persistence, checkpointing
└── m31a.yaml              # Default user config
```

---

## Six-Phase Workflow

| Phase | Package | Purpose |
|-------|---------|---------|
| **Session** | `pkg/session` | Create, persist, checkpoint, archive sessions under `~/.m31a/sessions/` |
| **Auto** | `pkg/autodream` | Prompt enhancement — DEEP/FAST/SKIP; enhances user prompts for better results |
| **Dream** | `internal/ghost` | Ghost writes append-only files with prompt-driven content generation |
| **Bisect** | `pkg/bisect` | Compare multiple model responses, compute diff scores |
| **Arbitrage** | `pkg/arbitrage` | Score task complexity, estimate costs, recommend optimal model |
| **Rollback** | `pkg/rollback` | Snapshot session state, restore from checkpoint |

---

## Provider Layer

### Interface (`internal/provider/provider.go`)
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

### Implementations

- **OpenRouter** (`internal/provider/openrouter/`) — Aggregates 300+ models; configurable referer/title
- **Zen** (`internal/provider/zen/`) — Barret AI / Zen API provider with default context length support

### BaseClient (`internal/provider/base_client.go`)
Shared HTTP transport with connection pooling (100 max idle conns, 10/host, 90s idle timeout). Provides `NewBaseClient()`, `APIKey()` (masked), `EstimateCost()`, `GetModel()`, `CachedModels()`, `MakeIterator()`, and caching.

### Model Cache (`internal/provider/cache.go`)
TTL-based in-memory cache with stale-while-revalidate support. Default TTL: 5 min, stale TTL: 1 hour.

### Capability Detection (`internal/provider/capabilities.go`)
Heuristic inference of model capabilities from model ID:
- **Tool use**: claude, gpt, gemini, deepseek, qwen, llama, mistral, command-r, command-a
- **Reasoning**: /o1, /o3, /o4 patterns + "reason" / "thinking" in ID
- **Vision**: "vision" or "multimodal" in ID

---

## TUI Layer

Built with **Bubble Tea** (`tea.Program`). Architecture:

```
cmd/m31a/main.go → NewApp() → tea.NewProgram(model)
```

### App Model (`internal/app/`)
- **states.go** — AppState enum (Init, Ready, Processing, Streaming, Paused, Error, ConfirmQuit, SessionPicker, GhostPicker, GhostOutput, BisectOutput)
- **init.go** — Tea init command (loads config, checks version)
- **update.go** — Tea update loop (handles messages, keybinds, state transitions)
- **view.go** — Tea render function (routes to active view)
- **views.go** — Individual view renderers (init, ready, processing, streaming, error, confirmquit, session picker, ghost picker, ghost output, bisect output)
- **keybinds.go** — Key mapping table
- **messages.go** — Custom tea.Msg types
- **startup.go** — Startup routine (checks keychain, provider health, version)
- **session.go** — Session management (create, save checkpoint, restore, list)
- **commands.go** — Slash command execution dispatch
- **ghost.go** — Ghost write command execution
- **bisect.go** — Bisect command execution
- **autodream.go** — Auto dream prompt enhancement execution

### Keyboard Shortcuts
| Key | Action |
|-----|--------|
| `Ctrl+C` / `q` | Quit (with confirmation in Processing/Streaming states) |
| `Enter` | Submit prompt |
| `Tab` / `Shift+Tab` | Cycle through autocomplete suggestions |
| `Up` / `Down` | Navigate suggestions |
| `Ctrl+S` | Save session checkpoint (when in Ready state, session active) |
| `Esc` | Abort / go back |

### Slash Commands
Registered in `internal/commands/registry.go`. Executed via `internal/app/commands.go`.

| Command | Description |
|---------|-------------|
| `/session` | Session management (list, create, delete, switch, save, checkpoint) |
| `/model` | List and select models |
| `/tools` | List and toggle available tools |
| `/config` | View/alter config at runtime |
| `/history` | View/prompt session history |
| `/export` | Export session data |
| `/agent` | Agent configuration |
| `/ghost` | Ghost write files |
| `/bisect` | Compare model responses |
| `/tui` | TUI mode toggle |
| `/help` | General help |
| `/keychain` | API key management |
| `/dream` | Toggle dream/prompt enhancement |
| `/flush` | Clear screen and reset |
| `/quit` | Quit application |
| `/exit` | Alias for quit |
| `//` | Literal slash passthrough |
| `!` | Bash command passthrough |

### Autocomplete
`internal/commands/autocomplete.go` — Tab-based suggestions; cycle through with Tab/Shift+Tab; matches by prefix.

---

## Configuration (`internal/config/`)

Config loaded from `~/.m31a.yaml` or `$XDG_CONFIG_HOME/m31a/m31a.yaml`. Uses `gopkg.in/yaml.v3`. Schema defined in `config.go` with validation.

### Sections
| Section | Description |
|---------|-------------|
| `api_key` | OpenRouter API key |
| `providers` | Provider-specific settings (Zen API URL, key, default context) |
| `model` | Default model ID, fallback model, token limits |
| `ui` | TUI theme, viewport history, edit mode, suggestions |
| `keys` | Custom keybindings |
| `agents` | Agent definitions (name, model, system prompt, tools, parameters) |
| `tools` | Tool configurations (MCP server commands, Toolhouse, Brave Search API key) |
| `git` | Git integration settings (auto-commit, author, GPG signing) |
| `verify` | Verification settings (provider, attestation, logging) |
| `ghost` | Ghost write settings (default directory, max retries, file patterns) |
| `dev` | Dev mode settings |
| `advanced` | Advanced options (cache TTL, health check thresholds, SSE parsers) |

---

## Tools Layer (`internal/tools/`)

| Package | Purpose |
|---------|---------|
| `registry.go` | ToolRegistry — register, list, lookup tools by name |
| `execute.go` | ToolExecutor — execute tools with timeout, collect output |
| `mcp_client.go` | MCP (Model Context Protocol) client — connects to MCP servers |
| `mcp_transport.go` | MCP transport — stdio-based communication with MCP servers |
| `toolhouse.go` | Toolhouse integration — cloud-based tool execution platform |
| `brave.go` | Braze Search API integration — web search via Brave |

### Tool Registry
Tools are registered with name, description, input/output schemas, and a handler function. The registry allows listing available tools and dispatching execution by name.

### MCP Client
Connects to MCP servers over stdio transport. Handles JSON-RPC message exchange for tool discovery and execution.

---

## Verification Layer (`internal/verify/`)

| Package | Purpose |
|---------|---------|
| `attestation.go` | Attestation — verifies binary integrity and provenance |
| `validate.go` | Validation — validates prompts, configs, tool outputs |
| `mock_mcp.go` | Mock MCP server for testing tool execution |

---

## Error Handling (`internal/errors/`)

Defines sentinel errors used across the app:
- `ErrInvalidKey` — Missing or empty API key
- `ErrNoModels` — No models available from provider
- `ErrHTTPRequest` — HTTP request failure
- `ErrStream` — Stream read failure
- `ErrConfigParse` — Config file parse failure
- `ErrSessionNotFound`, `ErrSessionLoad`, `ErrSessionSave` — Session errors
- `ErrProviderNotFound` — Unknown provider
- `ErrToolNotFound`, `ErrToolExecution` — Tool errors
- `ErrKeychainLocked`, `ErrKeychainSet`, `ErrKeychainGet` — Keychain errors
- `ErrGhostWrite`, `ErrGhostRead` — Ghost write errors
- `ErrBisectNoResponses`, `ErrBisectNoDiff` — Bisect errors

---

## State Machine

```
Init → Ready (healthy) or Error (startup failure)
Ready → Processing (Enter / submit)
Processing → Streaming (response received)
Streaming → Ready (stream complete)
Streaming → Paused (user interrupt) → Ready
Any state → Error → Ready
Any state → ConfirmQuit → Quit / resume
Ready → SessionPicker (list sessions)
Ready → GhostPicker / GhostOutput
Ready → BisectOutput
```

---

## Version

`internal/version/version.go` — exported via ldflags at build time:
```go
var (
    Version   = "dev"
    Commit    = "none"
    Date      = "unknown"
)
```
