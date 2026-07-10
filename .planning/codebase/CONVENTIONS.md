# M31A — Code Conventions

This document captures all code conventions enforced in the M31A project.
Sources: `AGENTS.md`, `CONTRIBUTING.md`, `.golangci.yml`, `Makefile`, and observed patterns in source files.

---

## Go Code Style

### Formatting

All Go source files **must be `gofmt`-clean** before committing.
The `make fmt` target runs both `go fmt` and `goimports`:

```makefile
fmt:
    go fmt ./...
    goimports -w $(find . -name '*.go' -not -path './vendor/*')
```

Run `make fmt` before every commit. CI enforcement via `make check` (which sequences `fmt → tidy → vet → lint → test`).

### Import Grouping

Imports must be grouped in three blocks, separated by blank lines:

```go
import (
    // 1. Standard library
    "context"
    "fmt"
    "os"

    // 2. Third-party
    "github.com/charmbracelet/bubbletea"
    "github.com/BurntSushi/toml"

    // 3. Project-internal
    m31errors "github.com/eshanized/M31A/internal/errors"
    "github.com/eshanized/M31A/internal/types"
)
```

Use `goimports` (not `gofmt` alone) for sorting within each group. Aliased imports (e.g., `m31errors`) are allowed to disambiguate conflicting package names — this pattern is seen throughout the codebase (e.g., `internal/tools/bash_security_test.go`, `pkg/taskrunner/runner_test.go`).

---

## Naming Conventions

### Packages

- **Lowercase, single words**: `provider`, `taskrunner`, `bisect`, `rollback`, `keychain`.
- No underscores or camelCase in package names.
- Test packages may use the `_test` external-test suffix (e.g., `package m31a_test` in `e2e_test.go`), or be in-package (e.g., `package provider` in `internal/provider/common_test.go`).

### Types and Interfaces

- **Exported types** use `MixedCaps` (UpperCamelCase): `TaskResult`, `RollbackResult`, `ProviderError`, `ToolError`, `ConfigError`.
- Interfaces are named by behaviour, typically without an `-er` suffix when they describe a component: `keychain.Keychain`, `types.Tool`.
- Pointer receivers are preferred for structs with mutable state.

### Functions and Methods

- **Exported functions** use `MixedCaps`: `UserMessage`, `RequireAPIKey`, `BuildChatBody`, `SanitizeProviderError`.
- **Unexported (private) functions** use `lowerCamelCase`: `parseBisectLog`, `maskAPIKeys`, `scrubEnvironment`, `applyVarSubstitution`, `findProjectConfig`.
- Constructor functions follow the `New…` convention: `New(tasks)`, `NewBash(...)`, `NewModelCache(...)`.

### Variables

- Local variables use `lowerCamelCase`: `execFn`, `apiKey`, `headHash`, `checkFn`.
- Sentinel errors are `var Err… = errors.New(…)` at package level: `ErrCircularDependency`, `ErrBisectResetFailed`, `ErrInvalidTimeout`.
- Boolean fields / variables are named affirmatively: `IsCurrent`, `Success`, `ChangesStashed`, `Truncated`.

---

## Error Handling

### Core Rule: Return Errors, Never Panic

Panics are **forbidden** in production code paths. The only observed `panic` usage is in test helper functions (e.g., `createCommits` in `pkg/rollback/rollback_test.go`), which is acceptable.

### Wrapping

Use `fmt.Errorf` with `%w` for all error wrapping. This preserves the `errors.Is` / `errors.As` chain:

```go
// Correct
return fmt.Errorf("soft reset to %s: %w", hash, err)

// Wrong — breaks errors.Is
return fmt.Errorf("soft reset failed: %s", err)
```

Verified in `pkg/rollback/rollback_test.go`: error messages from `SoftReset`, `HardReset`, `SafeReset` all contain the operation name and the underlying error wraps through.

### Sentinel Errors

Project-level sentinel errors live in `internal/errors/`. The full set is tested in `TestSentinelsAreUnique`. Key sentinels:

| Sentinel | Use Case |
|---|---|
| `ErrCircularDependency` | Kahn's algorithm detects a cycle |
| `ErrInvalidTimeout` | Bash tool receives out-of-range timeout |
| `ErrModelNotFound` | Model ID not in provider cache |
| `ErrBisectResetFailed` | `git bisect reset` call fails |
| `ErrPrivateIPBlocked` | SSRF protection triggered |
| `ErrPermissionDenied` | Tool permission check fails |

### Structured Error Types

Three rich error types are defined in `internal/errors/`:

- `ToolError{Tool, Op, Err}` — wraps tool execution failures.
- `ProviderError{Provider, Model, StatusCode, Err}` — wraps HTTP provider failures.
- `ConfigError{Key, Err}` — wraps configuration validation failures.

All implement `Unwrap() error` for chain traversal.

### Error Check Exclusions

The `.golangci.yml` excludes `errcheck` and `unused` from `*_test.go` files, so test files may ignore error returns in setup / teardown code without lint warnings. However, in production code, all returned errors must be handled.

---

## Comment and Documentation Standards

### Exported Items Must Have Doc Comments

Every exported function, type, method, and package must have a Go doc comment. The comment must start with the name of the item:

```go
// RequireAPIKey skips the test if the named environment variable is not set
// or is empty. Use this for tests that need real API keys from .env.
func RequireAPIKey(t *testing.T, envVar string) string {
```

### Inline Comments

Inline comments explain *why*, not *what*. The code should be clear enough to explain what it does; comments are reserved for non-obvious decisions:

```go
// Existing env vars are never overridden.
func LoadTestDotEnv(t *testing.T) {
```

### Special Markers

`//nolint:<linter>` directives are used sparingly and only when necessary:

```go
_ = os.Setenv(key, val) //nolint:errcheck
```

### No Emojis

**No emojis are permitted** in code comments, documentation, commit messages, or any text files. This is a hard project policy stated in both `AGENTS.md` and `CONTRIBUTING.md`.

---

## Commit Convention

All commits follow the **Conventional Commits** specification:

```
<type>: <short description>
```

Allowed types:

| Type | Use |
|---|---|
| `feat:` | New feature or capability |
| `fix:` | Bug fix |
| `docs:` | Documentation changes only |
| `test:` | Adding or updating tests |
| `refactor:` | Code change that is neither bug fix nor feature |
| `chore:` | Tooling, config, dependency updates |

**No emojis** in commit messages. Keep the subject line under 72 characters.
Breaking changes require discussion in an issue before a PR.

---

## Lint Rules

Configured in `.golangci.yml`. Timeout: **5 minutes**.

### Enabled Linters

| Linter | Purpose |
|---|---|
| `govet` | Standard Go vet checks |
| `govet/shadow` | Detects variable shadowing (enabled as a sub-check) |
| `staticcheck` | Advanced static analysis (SA*, S1*, etc.) |
| `errcheck` | Ensures returned errors are not silently discarded |
| `ineffassign` | Detects assignments whose results are never used |
| `unused` | Detects unexported identifiers never referenced |

### Linter Settings

```yaml
govet:
  enable:
    - shadow          # variable shadowing detection

errcheck:
  check-type-assertions: false   # comma-ok pattern is exempt
  check-blank: false             # _ = expr is exempt
```

### Exclusions

```yaml
exclusions:
  rules:
    - path: _test\.go
      linters:
        - errcheck
        - unused
```

Test files are excluded from `errcheck` and `unused`. This allows test setup code to discard errors without lint failures.

### Running Lint

```bash
make lint          # golangci-lint run ./... --timeout=5m
make lint-fix      # with --fix flag
make vet           # go vet ./... standalone
```

---

## File Organization

### Directory Boundaries

```
cmd/m31a/          Entry point only (flag parsing, TUI launch)
internal/          All private implementation packages
pkg/               Public packages — importable by external consumers
```

**Hard constraint**: `pkg/` packages **must NOT import** anything from `internal/`. This is enforced by the Go module system. Observed: `pkg/taskrunner` imports `internal/errors` and `internal/types` — these are the only allowed cross-boundary imports because `internal/types` is the project's shared type vocabulary.

### Test File Placement

Test files are **co-located** with the source they test in the **same directory**:

```
pkg/taskrunner/runner.go
pkg/taskrunner/runner_test.go    # same package (white-box)
pkg/taskrunner/doc_test.go       # documentation examples
```

External (black-box) tests use the `_test` package suffix:
```go
// e2e_test.go
package m31a_test
```

### File Naming

- Source files: `lowercase_with_underscores.go` (e.g., `bash_security.go`, `cache_refresh.go`).
- Test files: same base name with `_test.go` suffix.
- Extra / supplement tests: `extra_test.go` or `<feature>_extra_test.go`.
- Integration tests: `integration_test.go`.
- Benchmark files: `bench_test.go`.

### Package-Level Globals

Sentinel errors and shared constants are declared at the package level in dedicated files:
- `internal/errors/` — sentinel errors and structured error types.
- `internal/types/types.go` — shared type vocabulary (`Task`, `TaskStatus`, `Message`, `BashOutputLimit`, workflow phases).

---

## Project-Specific Conventions

### No CGO

`CGO_ENABLED=0` is a hard constraint. The binary must be fully static. Any dependency that requires CGO will break the build.

### Bubble Tea Threading

The TUI uses the Bubble Tea Elm architecture. All state mutations go through `Update()` only. **Never mutate `AppState` from a goroutine.** Use `tea.Cmd` and `tea.Msg` to communicate results back to the update loop.

### API Key Handling

API keys are resolved in order: environment variable → OS keychain → config file. They are **never written to disk in plaintext**. The `internal/testutil` package provides `RequireAPIKey(t, "ENV_VAR")` to guard tests that need real credentials.

### No Hardcoded Model Names

Provider model lists are discovered dynamically from provider APIs at runtime. Model names must never be hardcoded in production code; tests may use specific model names only in integration test guards (e.g., `"meta/llama-3.1-8b-instruct"` in `e2e_test.go`).

### Bounded Parallelism

The task runner uses Kahn's topological sort with a maximum of **4 concurrent tasks** per execution group. This is enforced in `pkg/taskrunner/`.

### Dynamic Context System

`internal/workflow/engine.go` orchestrates seven fixed phases: Initialize → Discuss → Plan → Execute → Verify → Runtime → Ship. Phase transitions are validated against `validPhaseTransitions` map; invalid transitions return `ErrPhaseTransition`.

### `.env` File Policy

`.env` files are gitignored. Only `.env.example` (with placeholder values) is committed. `.env.test` is used for integration tests and is also gitignored; its structure is documented for contributors.
