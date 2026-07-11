# Coding Conventions

**Analysis Date:** 2026-07-11

## Language & Runtime

**Primary:**
- Go 1.25.0 — entire codebase (`go.mod`)

**Build Constraints:**
- `CGO_ENABLED=0` hard requirement (static binary, no CGO)
- Cross-compilation targets: linux/{amd64,arm64}, darwin/{amd64,arm64}, windows/amd64

## Formatting & Imports

**Formatter:** `gofmt` (enforced via `make fmt` which runs `go fmt ./... && goimports -w ...`)

**Import Organization (goimports groups):**
1. Standard library
2. Third-party (external modules)
3. Project internal (`github.com/eshanized/M31A/...`)

```go
import (
	"context"
	"fmt"
	"time"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/eshanized/M31A/internal/config"
	"github.com/eshanized/M31A/internal/errors"
	"github.com/eshanized/M31A/internal/types"
)
```

**Line Length:** No explicit limit; follows `gofmt` defaults.

## Naming Conventions

| Element | Convention | Example |
|---------|------------|---------|
| Files | `snake_case.go` | `runner.go`, `bisect_test.go` |
| Test Files | `<name>_test.go` | `runner_test.go`, `errors_test.go` |
| Packages | Lowercase, single word | `taskrunner`, `bisect`, `errors` |
| Exported Types | PascalCase | `Runner`, `TaskResult`, `ProviderError` |
| Exported Functions | PascalCase | `NewRunner`, `Schedule`, `UserMessage` |
| Unexported Functions | camelCase | `parseBisectLog`, `buildToolDefinitions` |
| Constants | PascalCase (or UPPER_SNAKE for package-level) | `StatusPending`, `MaxHealAttempts` |
| Variables (local) | camelCase | `taskCtx`, `readyTask` |
| Receiver Names | 1-2 letters, lower | `r *Runner`, `e *Engine` |
| Interface Suffix | None (e.g., `Tool`, not `ToolInterface`) | `SchemaProvider` |

**Special:**
- No emojis in code or docs (explicit project rule)
- Doc comments on all exported types/functions (godoc style: `// Package...`, `// Type...`)

## Error Handling

**Core Patterns:**

1. **Sentinel Errors** — defined in `internal/errors/errors.go`
   ```go
   var ErrCircularDependency = errors.New("circular dependency in task graph")
   var ErrProviderUnreachable = errors.New("provider unreachable")
   ```

2. **Wrapped Errors** — always use `%w` verb for error chaining
   ```go
   return fmt.Errorf("task %d: %w", t.ID, m31errors.ErrCircularDependency)
   return fmt.Errorf("auth failed: %w", ErrInvalidKey)
   ```

3. **Structured Error Types** — implement `Unwrap()` for `errors.Is`/`errors.As`:
   ```go
   type ToolError struct { Tool, Op string; Err error }
   func (e *ToolError) Error() string { ... }
   func (e *ToolError) Unwrap() error { return e.Err }
   ```

4. **User-Facing Messages** — `internal/errors.UserMessage(err error) string` maps sentinel/structured errors to friendly strings.

**Anti-Patterns (enforced by linter):**
- No `panic()` in production code
- No bare `return err` without context (use `fmt.Errorf("%w", err)`)
- No `errors.New` in hot paths for control flow — use sentinels

## Documentation

**Package Docs:** Every package has a `doc.go` with a one-sentence summary.
```go
// Package rollback provides a commit chain browser for M31A sessions.
// It lists session commits, supports soft and hard resets...
package rollback
```

**Exported Symbols:** Doc comment required (enforced by `golangci-lint` via `govet`).
```go
// Runner schedules and executes tasks with dependency resolution.
type Runner struct { ... }

// Schedule performs topological sort and returns execution groups.
func (r *Runner) Schedule() ([][]int, error) { ... }
```

**Internal Comments:** Minimal; prefer self-documenting code. Use `//` for logic explanation, `/* */` for disabled code blocks.

## Code Structure Patterns

**Dependency Rule:** `pkg/` MUST NOT import `internal/` (enforced by Go module system).
- `internal/` = shared private code (types, errors, tools, config, workflow)
- `pkg/` = reusable libraries (taskrunner, bisect, rollback, etc.)

**Interface Definitions:** Defined in consumer package, not provider.
```go
// In internal/types/types.go (consumer-facing)
type Tool interface {
    Name() string
    Description() string
    RiskLevel() RiskLevel
    Execute(ctx context.Context, input ToolInput) (ToolResult, error)
}
```

**Concurrency:** 
- Bubble Tea (TUI) is single-threaded — all state mutations via `Update()` + `tea.Cmd`/`tea.Msg`
- Background goroutines use channels, never shared mutable state
- `sync.RWMutex` for protecting maps/slices accessed from multiple goroutines (e.g., `Runner.status`, `Runner.results`)

**Generics:** Used in `pkg/coordinator/coordinator.go` for type-safe keys:
```go
type Coordinator[Key comparable] struct { ... }
```

## Linting Configuration

**Tool:** `golangci-lint` v2 (config: `.golangci.yml`)

**Enabled Linters:**
- `govet` (with `shadow` check)
- `staticcheck`
- `errcheck`
- `ineffassign`
- `unused`

**Exclusions:**
```yaml
exclusions:
  rules:
    - path: _test\.go
      linters:
        - errcheck
        - unused
```

**Run Command:** `make lint` (5m timeout)

## Commit Convention

**Conventional Commits:**
```
feat: add website template extraction
fix: handle nil logger in bisect
docs: update ARCHITECTURE.md
test: add coverage for rollback edge cases
refactor: extract phase coordinator
chore: bump go version to 1.25
```

## Key Files for Convention Reference

| File | Purpose |
|------|---------|
| `.golangci.yml` | Lint config |
| `Makefile` | Build/test/lint commands |
| `internal/errors/errors.go` | Error sentinels, wrapping patterns, user messages |
| `internal/types/types.go` | Core types, interfaces, constants |
| `pkg/taskrunner/runner.go` | Concurrency, error handling, callbacks |
| `internal/workflow/engine.go` | Large struct, dependency injection, context handling |
| `pkg/rollback/rollback.go` | Git operations, error wrapping, result types |

---

*Convention analysis: 2026-07-11*