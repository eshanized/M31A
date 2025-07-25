# Coding Conventions

**Analysis Date:** 2026-06-04

## Naming Patterns

**Files:**
- snake_case for Go files: `bash_unix.go`, `filewrite_test.go`, `cache_refresh_test.go`
- Build-tag files use platform suffix: `bash_unix.go` (with `//go:build !windows`)
- Test files use `_test.go` suffix, co-located with source files

**Functions:**
- Exported: PascalCase — `NewRegistry()`, `FindFallbackProvider()`, `ChatCompletionStream()`
- Unexported: camelCase — `sanitizeProviderError()`, `isContextExceeded()`, `getNestedField()`
- Constructor pattern: `New<Type>(<deps>)` — `NewBash(dir)`, `NewGlob(dir)`, `NewRegistry()`
- Boolean helpers: `Is<Condition>` or `has<Something>` — `IsRateLimited()`, `IsRepo()`, `hasRg`

**Variables:**
- Local variables: camelCase — `callCount`, `requestBody`, `retryAfterHeader`
- Constants: camelCase for unexported (`sigInt`, `maxRetryAfter`), PascalCase for exported (`BashOutputLimit`)
- JSON struct tags: snake_case — `json:"prompt_tokens"`, `json:"context_length,omitempty"`
- TOML struct tags: snake_case — `toml:"api_key"`, `toml:"default_mode"`

**Types:**
- Exported types: PascalCase — `Registry`, `ModelCache`, `FallbackEvent`
- Interface names: noun or noun phrase — `LLMProvider`, `Tool`, `PermissionGate`
- Error types: `Err` prefix — `ErrValidation`, `ErrProviderUnreachable`
- Config types: `<Section>Config` — `ProviderConfig`, `ModelConfig`, `UIConfig`

**Constants:**
- Typed string constants for enums — `RiskSafe`, `PhaseInitialize`, `StatusPending`
- Constants grouped in `const` blocks with typed values — `RiskLevel`, `WorkflowPhase`, `TaskStatus`

## Code Style

**Formatting:**
- Tool: `gofmt` (enforced in CI), `goimports` run via Makefile `fmt` target
- No `.prettierrc` or formatter config — standard Go formatting only

**Linting:**
- Tool: `golangci-lint` v4 with `.golangci.yml`
- Enabled linters: `govet`, `staticcheck`, `errcheck`, `ineffassign`, `unused`, `gosimple`
- Shadow checking enabled: `govet.check-shadowing: true`
- Test file exclusions: `errcheck` and `unused` disabled in `_test.go` files
- Timeout: 5 minutes

**Import Organization:**
- Group 1: Standard library (`context`, `errors`, `fmt`, `net/http`)
- Group 2: External packages (`github.com/BurntSushi/toml`, `github.com/charmbracelet/bubbletea`)
- Group 3: Internal packages (`github.com/eshanized/M31A/internal/errors`, `github.com/eshanized/M31A/internal/types`)
- Import alias convention: `m31errors "github.com/eshanized/M31A/internal/errors"` — always alias the errors package

## Error Handling

**Sentinel Errors:**
- All sentinel errors defined in `internal/errors/errors.go` as package-level `var` with `errors.New()`
- Naming: `Err` prefix + PascalCase — `ErrProviderUnreachable`, `ErrInvalidKey`, `ErrPermissionDenied`
- Comparison: always use `errors.Is(err, sentinel)` — never `err == sentinel`
- Wrapping: `fmt.Errorf("context: %w", err)` — always wrap with context

**Error Patterns:**
```go
// Wrapping with context
return nil, fmt.Errorf("decode global config %s: %w", path, err)

// Sentinel error comparison
if errors.Is(err, m31errors.ErrInvalidKey) {
    return "Invalid API key"
}

// User-facing messages via errors.UserMessage(err)
msg := errors.UserMessage(err)
```

**Error Types:**
- `ValidationError` struct with `Field`, `ExpectedType`, `ActualValue` fields for config validation
- Implements `error` interface via `Error() string`
- Multiple validation errors joined with newlines in a single error return

## Provider Interface Pattern

**Location:** `internal/provider/interface.go`

```go
type LLMProvider interface {
    Name() string
    APIKey() string
    FetchModels(ctx context.Context) ([]types.ModelInfo, error)
    ChatCompletionStream(ctx context.Context, req ChatRequest) (*types.StreamIterator, error)
    EstimateCost(modelID string, usage types.Usage) float64
    HealthCheck(ctx context.Context) types.HealthStatus
    GetModel(id string) (*types.ModelInfo, error)
}
```

**Implementation Pattern:**
- Each provider in its own subpackage: `internal/provider/openrouter/`, `internal/provider/zen/`
- Constructor: `New(apiKey string, Options{}) (*Client, error)` — validates API key upfront
- Base URL stored as struct field, overridden in tests via `c.baseURL = ts.URL`
- Model cache: `sync.RWMutex`-protected `ModelCache` with TTL and stale fallback
- SSE parsing: `SSEParser` reads `io.ReadCloser`, yields events via `Next() (eventType, data, err)`
- Reasoning normalization: `ParseSSEChunk()` maps provider-specific SSE fields to unified `StreamChunk{Type: "thinking"|"content"}`
- HTTP client: 30s dial timeout via `http.Client{Timeout: ...}`; no body read timeout for streaming

## Tool Interface Pattern

**Location:** `internal/types/types.go`

```go
type Tool interface {
    Name() string
    Description() string
    RiskLevel() RiskLevel
    Execute(ctx context.Context, input ToolInput) (ToolResult, error)
}
```

**Implementation Pattern:**
- Each tool in its own file: `internal/tools/bash.go`, `internal/tools/fileread.go`
- Constructor: `New<Type>(workDir string, ...deps)` — stores working directory
- Platform-specific code: build tags (`bash_unix.go` with `//go:build !windows`)
- Parameter extraction: type-assert from `map[string]any` with explicit type checks
- Missing params: return `fmt.Errorf("missing parameter: %s", name)`
- Invalid types: return `fmt.Errorf("parameter %s must be a string", name)`
- Path safety: resolve symlinks, reject paths outside `workDir`
- Atomic writes: temp file + `os.Rename` pattern in `FileWrite`
- Output limits: `limitWriter` caps output at `BashOutputLimit` (50K chars)

## Module Design

**Package Layout:**
- `cmd/m31a/` — binary entry point only, no logic
- `internal/` — private packages, cannot be imported externally
- `pkg/` — public packages, potentially reusable

**Export Pattern:**
- Minimal exported API per package
- Unexported implementations with exported interfaces
- `New()` constructors for all major types
- Interface-based abstractions (`LLMProvider`, `Tool`, `PermissionGate`)

**Barrel Files:** Not used. Each type file exports specific types.

## Structured Logging

**Framework:** `log/slog` (stdlib)

**Pattern:**
```go
import "log/slog"

// Structured fields
slog.Warn("keychain error", "provider", "openrouter", "error", err)

// Config-driven format
if os.Getenv("M31A_LOG_FORMAT") == "text" {
    handler = slog.NewTextHandler(f, opts)
} else {
    handler = slog.NewJSONHandler(f, opts)
}
```

**Rules:**
- Log to file only (`~/.m31a/m31a.log`) — never stdout/stderr during TUI operation
- Level controlled by `M31A_LOG_LEVEL` env var (debug/info/warn/error)
- Log rotation: daily, 7-day retention
- No telemetry, no analytics, no external calls

## Atomic File Operations

**Pattern:** Write to temp file, then rename:
```go
func atomicWrite(path string, data []byte) error {
    tmpPath := filepath.Join(dir, ".m31a_tmp_"+hex.EncodeToString(randBytes))
    // write to tmpPath
    return os.Rename(tmpPath, path)
}
```

**Used in:**
- `internal/config/loader.go` — config save
- `internal/tools/filewrite.go` — file writes
- `pkg/session/` — session persistence

## Threading Rules

**Bubble Tea is single-threaded.** All state mutations go through `Update()` only.
- Goroutines emit `tea.Cmd` functions that return `tea.Msg`
- `View()` never reads mutable state without synchronization
- Provider registry uses `sync.RWMutex` for concurrent access
- Model cache uses `sync.RWMutex` + `atomic.Bool` for refresh flag
- Permission dispatcher uses buffered channels (`chan PermissionRequest`) for permission gating

## Platform-Specific Code

**Build tags:** Used for platform-specific implementations
```go
//go:build !windows
// bash_unix.go — Linux/macOS PTY and process group handling

//go:build windows
// bash_windows.go — Windows pipe-based fallback
```

**Keychain implementations:**
- `pkg/keychain/keychain_linux.go` — freedesktop Secret Service
- `pkg/keychain/keychain_darwin.go` — macOS Keychain Services
- `pkg/keychain/keychain_windows.go` — Windows Credential Manager

## Comments

**When to Comment:**
- Doc comments for all exported functions and types
- Inline comments for non-obvious logic (SSE parsing, reasoning normalization)
- Fix references: `// Fix C-4:`, `// Fix M-13:` — link to issue/fix tracker
- Build tags: always include explanation above `//go:build` directive

**JSDoc/TSDoc:** Not applicable (Go project). Use Go doc comment conventions.

## Configuration

**Format:** TOML via `BurntSushi/toml`
**Location:** `~/.m31a/config.toml` (global), `m31a.toml` (project-level, walk-up discovery)
**Override order:** Defaults → Global TOML → Env vars (`M31A_*`) → Project TOML → `${VAR}` substitution
**Validation:** `validateConfig()` collects all errors, returns joined error with `ErrValidation` sentinel

---

*Convention analysis: 2026-06-04*
