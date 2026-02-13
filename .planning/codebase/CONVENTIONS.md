# Coding Conventions

**Analysis Date:** 2026-06-11

## Language & Toolchain

**Language:** Go 1.24+
**Module:** `github.com/eshanized/M31A`
**Build:** `CGO_ENABLED=0` static binary (mandatory, no CGO)

## Naming Patterns

**Files:**
- Lowercase with underscores: `bash_test.go`, `filewrite.go`, `engine_test.go`
- Test files: `*_test.go` suffix (standard Go convention)
- Interface files: `interface.go` (e.g., `internal/tools/interface.go`, `internal/provider/interface.go`)
- Type definition files: `types.go` (e.g., `internal/types/types.go`, `internal/config/types.go`)
- Constants files: `constants.go` (e.g., `internal/types/constants.go`)

**Packages:**
- Lowercase single words: `tools`, `provider`, `types`, `config`, `session`
- Avoid abbreviations unless universally understood: use `session` not `sess`
- Sub-packages for providers: `internal/provider/openrouter/`, `internal/provider/zen/`

**Structs:**
- PascalCase: `Bash`, `FileRead`, `Client`, `Dispatcher`, `Engine`
- Interface names: noun or `-er` suffix: `LLMProvider`, `Tool`, `PermissionGate`, `SchemaProvider`
- Mock types: `mock` prefix: `mockProvider`, `mockTool`, `multiTurnMockProvider`

**Functions:**
- PascalCase for exported: `NewBash()`, `NewEngine()`, `Execute()`, `ChatCompletionStream()`
- camelCase for unexported: `maskAPIKeys()`, `rotateLogFiles()`, `readFileLimited()`
- Constructor pattern: `NewTypeName(params) *TypeName` (e.g., `NewBash(workDir string) *Bash`)
- Boolean getters: `CanConsolidate()`, `IsHealthy()`

**Methods on structs:**
- Receiver names: single letter matching first letter of type: `(t *Bash)`, `(c *Client)`, `(m *Manager)`, `(e *Engine)`
- Receiver names are consistent within a file

**Constants:**
- PascalCase for exported: `RiskSafe`, `PhasePlan`, `StatusPending`, `MaxFileSize`
- camelCase for unexported: `dirPermission`, `filePermission`, `dateFormat`
- String enums use typed constants: `type RiskLevel string` with `const` block
- Block grouping by domain: `RiskLevel`, `WorkflowPhase`, `TaskStatus` each in their own `const` block

**Variables:**
- camelCase for unexported: `defaultLogger`, `skipDirsCache`, `permissionRequestID`
- PascalCase for exported: `Version`, `SkipDirs`, `NormalBorder`
- Package-level `var` blocks for grouped state

## Code Style

**Formatting:**
- Tool: `gofmt` (enforced in CI via `gofmt -l .`)
- Import sorting: `goimports` (run via `make fmt`)
- No tabs in comments; tabs for code indentation (standard Go)

**Linting:**
- Tool: `golangci-lint` (`.golangci.yml`)
- Enabled linters: `govet`, `staticcheck`, `errcheck`, `ineffassign`, `unused`, `gosimple`
- Shadow checking enabled: `govet.check-shadowing: true`
- Test files exempt from `errcheck` and `unused` linters
- Lint timeout: 5 minutes

**Key lint rules:**
- No unused variables or imports
- All errors must be checked (except in test files)
- No shadowed variables (caught by govet)

## Import Organization

**Order (with blank line separators):**
1. Standard library
2. Third-party packages
3. Internal project packages (`github.com/eshanized/M31A/...`)

**Aliasing convention:**
- Error package: `m31errors "github.com/eshanized/M31A/internal/errors"`
- Types package: `m31types "github.com/eshanized/M31A/internal/types"`
- Bubble Tea: `tea "github.com/charmbracelet/bubbletea"`
- Provider package: used as-is (no alias needed when not conflicting)

**Example from `internal/workflow/engine.go`:**
```go
import (
    "context"
    "embed"
    "fmt"
    "io"
    "log/slog"
    "os/exec"
    "path/filepath"
    "strings"
    "time"

    "github.com/eshanized/M31A/internal/config"
    m31errors "github.com/eshanized/M31A/internal/errors"
    "github.com/eshanized/M31A/internal/git"
    "github.com/eshanized/M31A/internal/provider"
    "github.com/eshanized/M31A/internal/tokens"
    "github.com/eshanized/M31A/internal/tools"
    m31types "github.com/eshanized/M31A/internal/types"
    "github.com/eshanized/M31A/pkg/session"

    tea "github.com/charmbracelet/bubbletea"
)
```

## Error Handling

**Strategy:** Return errors, never panic. Use sentinel errors and wrapping.

**Sentinel errors:**
- Defined in `internal/errors/errors.go` as package-level `var` with `errors.New()`
- Naming: `Err` prefix + PascalCase: `ErrInvalidKey`, `ErrProviderUnreachable`, `ErrToolExecution`
- Use `errors.Is()` for comparison, never type assertions

**Error wrapping:**
- Use `fmt.Errorf` with `%w` verb: `fmt.Errorf("missing parameter: command: %w", m31errors.ErrToolExecution)`
- Always wrap with the sentinel error as the target
- Provide context in the message: what failed and why

**User-facing errors:**
- `internal/errors/errors.go` provides `UserMessage(e error) string` function
- Maps sentinel errors to human-readable messages
- Falls back to pattern matching for unwrapped errors
- Never expose raw error messages to TUI

**Error return pattern:**
```go
// Correct
command, ok := input.Params["command"]
if !ok {
    return types.ToolResult{}, fmt.Errorf("missing parameter: command: %w", m31errors.ErrToolExecution)
}

// Incorrect - don't return errors without sentinel wrapping
if !ok {
    return types.ToolResult{}, fmt.Errorf("missing parameter")
}
```

## Type Usage Patterns

**Typed string enums:**
```go
type RiskLevel string
const (
    RiskSafe        RiskLevel = "safe"
    RiskMedium      RiskLevel = "medium"
    RiskDangerous   RiskLevel = "dangerous"
    RiskDestructive RiskLevel = "destructive"
)
```

**Struct tags:**
- JSON: `json:"field_name"` with `omitempty` for optional fields
- TOML: `toml:"field_name"` for config structs
- Always use lowercase snake_case in tags

**Interface compliance check:**
```go
// Compile-time interface check
var _ provider.LLMProvider = (*Client)(nil)
```

**Optional interfaces:**
```go
// SchemaProvider is an optional interface that tools can implement
type SchemaProvider interface {
    ParameterSchema() string
}
```

## Documentation Style

**Package comments:**
- Each package has a `package` comment (implicit via doc.go or file comment)

**Exported functions/types:**
- Must have doc comments starting with the name: `// Manager provides CRUD operations for sessions stored on disk.`
- Comments explain what and why, not how

**Inline comments:**
- Explain why, not what
- Use for non-obvious decisions: `// log rotation failure is non-fatal — warn and continue append-only`
- Use for TODO tracking: `// DEP-3: BurntSushi/toml v1 — in maintenance mode; v2 has different API; migrate when ready`

**Test comments:**
- Test function names describe the scenario: `TestBash_SimpleCommand`, `TestDispatcher_DangerousToolPermissionGranted`
- Helper comments at section breaks: `// ---------------------------------------------------------------------------`
- Test helper functions documented: `// makeMessages creates n alternating user/assistant messages with sample content.`

## Common Patterns

**Constructor pattern:**
```go
func NewTypeName(workDir string) *TypeName {
    return &TypeName{
        workDir: workDir,
    }
}
```

**Options struct pattern:**
```go
type Options struct {
    BaseURL           string
    CacheTTL          time.Duration
    HealthCheckLiveMs int64
}

func New(apiKey string, opts Options) (*Client, error) {
    if opts.BaseURL == "" {
        opts.BaseURL = types.DefaultOpenRouterBaseURL
    }
    // ...
}
```

**Atomic file writes:**
```go
func atomicWrite(path string, data []byte) error {
    return fileutil.AtomicWrite(path, data)
}
```

**Context propagation:**
```go
func (t *Bash) Execute(ctx context.Context, input types.ToolInput) (types.ToolResult, error) {
    if err := ctx.Err(); err != nil {
        return types.ToolResult{}, fmt.Errorf("%w: %v", m31errors.ErrToolExecution, err)
    }
    // ...
}
```

**Temporary directory for tests:**
```go
func TestSomething(t *testing.T) {
    t.Parallel()
    dir := t.TempDir()
    // ...
}
```

## Logging

**Framework:** `log/slog` (structured logging)

**Destination:** File only (`~/.m31a/m31a.log`), never stdout/stderr during TUI operation

**Log rotation:** 7-day retention, daily rotation

**Levels:** Configurable via `M31A_LOG_LEVEL` env var (default: `info`)

**Format:** Configurable via `M31A_LOG_FORMAT` env var (default: `json`)

**Usage:**
```go
logger.Info("session created", "session_id", s.ID, "model", model)
logger.Error("provider health check failed", "error", err, "provider", name)
```

## Configuration

**Primary config:** `~/.m31a/config.toml` (TOML format)

**Resolution order:** Environment variable → OS keychain → Config file

**Config struct:** `internal/config/types.go` with nested typed structs

**Defaults:** Set in code, not in config file. Missing config fields use zero values which trigger defaults.

## Git Commit Conventions

**Format:** Conventional commits

**Prefixes:**
- `feat:` — new feature
- `fix:` — bug fix
- `docs:` — documentation only
- `test:` — adding or updating tests
- `refactor:` — code change that neither fixes a bug nor adds a feature
- `chore:` — maintenance tasks
- `perf:` — performance improvement

**Scope (optional):** Package or area in parentheses: `feat(tui):`, `fix(tools):`, `workflow):`

**Examples from git log:**
```
feat(tui): add phase model picker for planning vs coding model selection
fix(taskrunner): use parent context for retry backoff
refactor(tui): simplify stream handlers and update views
chore: add image asset
```

**Rules:**
- One concern per commit
- No breaking changes without discussion
- Commit messages are imperative mood: "add feature" not "added feature"

## Architecture Constraints

**Package dependency rules:**
- `internal/types/` must have zero internal imports
- `internal/errors/` must have zero internal imports
- `internal/log/` must import only stdlib packages
- No circular dependencies permitted

**Threading model:**
- Bubble Tea is single-threaded
- All state mutations through `Update()` only
- Never mutate `AppState` from goroutines
- Use `tea.Cmd` and `tea.Msg` for cross-goroutine communication

**Prohibitions:**
- No direct Anthropic/OpenAI connections (only OpenRouter and Zen)
- No CSS-style animations (Bubble Tea uses Unicode spinners)
- No hardcoded model lists (dynamic discovery)
- No telemetry, analytics, or external calls except LLM APIs
- No plaintext API key storage

---

*Convention analysis: 2026-06-11*
