# Coding Conventions

**Analysis Date:** [YYYY-MM-DD]

## Naming Patterns

**Files:**
- Pattern observed: `snake_case.go` for all Go source files (e.g., `bash_unix.go`, `loader.go`). Test files are suffixed with `_test.go` (e.g., `loader_test.go`).

**Functions:**
- Pattern observed: `PascalCase` for exported functions (`DefaultConfig()`, `UserMessage()`) and `camelCase` for unexported functions (`newMockKeychain()`). Test methods follow `TestComponent_Method` or `TestFunction` patterns (e.g., `TestBash_SimpleCommand`).

**Variables:**
- Pattern observed: `PascalCase` for exported variables (e.g., `ErrProviderUnreachable`). Local variables use concise `camelCase` (e.g., `todoPath`, `cfg`, `depStatus`).

**Types:**
- Pattern observed: `PascalCase` for exported types and interfaces (`Runner`, `TaskResult`, `LLMProvider`). Interface naming occasionally utilizes the `-er` suffix standard where applicable (`MsgEmitter`), but descriptive names are preferred for domain interfaces (`PermissionGate`, `LLMProvider`).

## Code Style

**Formatting:**
- Tool used: Standard Go formatter (`go fmt`).
- Key settings: Automated and unified through `make fmt`.

**Linting:**
- Tool used: `golangci-lint` invoked via `make lint`.
- Key rules: `govet`, `staticcheck`, `errcheck`, `ineffassign`, `unused`, and `gosimple`. `errcheck` and `unused` are excluded for `_test.go` files.

## Import Organization

**Order:**
1. Standard library packages
2. Third-party packages
3. Internal application packages (`github.com/eshanized/M31A/...`)

**Path Aliases:**
- Aliases used: Occasional path aliases are used to prevent namespace collisions or add clarity (e.g., `m31errors "github.com/eshanized/M31A/internal/errors"`).

## Error Handling

**Patterns:**
- Widespread usage of Sentinel Errors created with `errors.New` (e.g., `ErrProviderUnreachable`, `ErrInvalidKey`).
- Errors are wrapped with context using `fmt.Errorf("...: %w", err)` to maintain underlying traces.
- A centralized user-facing error dictionary is maintained in `internal/errors/errors.go` via a `UserMessage(error) string` function, translating programmatic errors to actionable UI feedback.

## Logging

**Framework:** Custom application logging/UI mechanisms. Standard logging is generally suppressed in favor of bubbletea TUI outputs.

**Patterns:**
- Errors are returned back up the stack (`return nil, err`) rather than being printed directly to standard output mid-execution.
- The workflow engine triggers message events (`MsgEmitter`) indicating state updates, maintaining UI separation from core logic.

## Comments

**When to Comment:**
- Godoc comments are strictly added on exported structs, variables, and functions.
- Inline explanations are used for complex algorithmic code blocks (e.g., Kahn's topological sort implementation in task runner logic).

**JSDoc/TSDoc:**
- Usage pattern: Godoc format is universally employed (e.g., `// TaskResult holds the outcome of executing a single task.`).

## Function Design

**Size:** Functions are predominantly short, scoped to single responsibilities.

**Parameters:** Standard Go context propagation (`ctx context.Context` as the primary parameter in executing tasks or tools). Context is consistently utilized for timeout handling and cancellations.

**Return Values:** Typically follows the standard `(ResultType, error)` signature pattern.

## Module Design

**Exports:** Strict separation of internal modules. High-level architecture hides internal complexities under `internal/...` while `pkg/...` holds agnostic components (e.g., `pkg/taskrunner`, `pkg/keychain`).

**Barrel Files:** Not applicable in Go. Standard Go package export mechanisms are utilized natively.

---

*Convention analysis: [YYYY-MM-DD]*