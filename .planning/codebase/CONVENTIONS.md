# Code Conventions

**Last mapped:** 2026-08-02
**Project:** M31 Autonomous (Terminal AI Coding Agent)

## Go Code Style

### Formatting

- **Tool:** `go fmt` (standard)
- **Imports:** `goimports` for import grouping
- **Line length:** No hard limit, but prefer readability

### Import Organization

Standard Go import grouping:

```go
import (
    // Standard library
    "context"
    "fmt"
    
    // Third-party
    tea "github.com/charmbracelet/bubbletea"
    
    // Internal
    "github.com/eshanized/M31A/internal/core/config"
)
```

### Naming Conventions

#### Packages

- **Lowercase, single word:** `workflow`, `session`, `provider`
- **Avoid abbreviations:** `fileoperations` not `fileops` (except where established)
- **No underscores:** `codeanalysis` not `code_analysis`

#### Files

- **Snake case:** `file_read.go`, `bash_sandbox_linux.go`
- **Test files:** `*_test.go` suffix
- **Platform-specific:** `*_unix.go`, `*_windows.go`, `*_linux.go`, `*_darwin.go`

#### Types

- **PascalCase:** `WorkflowEngine`, `SessionManager`, `ProviderRegistry`
- **Interfaces:** Verb-noun pattern: `Provider`, `Tool`, `Manager`
- **Structs:** Noun pattern: `Config`, `Message`, `ToolCall`

#### Functions

- **PascalCase (exported):** `NewProvider()`, `LoadConfig()`, `ExecuteWorkflow()`
- **camelCase (unexported):** `parseFlags()`, `validateInput()`
- **Getters:** `BaseURL()`, `Version()`, `ActiveProvider()`
- **Setters:** `SetTimeout()`, `UpdateConfig()`

#### Constants

- **PascalCase (exported):** `DefaultTimeout`, `MaxRetries`
- **camelCase (unexported):** `maxBufferSize`, `defaultRetryDelay`
- **SCREAMING_SNAKE:** Only for true constants: `CGO_ENABLED=0`

### Error Handling

- **Pattern:** Return errors, never panic
- **Wrapping:** `fmt.Errorf("%w", err)` for error chains
- **Types:** Custom error types in `internal/core/errors/`
- **Sentinel errors:** `var ErrNotFound = errors.New("not found")`

Example:

```go
func readFile(path string) ([]byte, error) {
    data, err := os.ReadFile(path)
    if err != nil {
        return nil, fmt.Errorf("reading file %s: %w", path, err)
    }
    return data, nil
}
```

### Comments

- **Exported functions:** Always have doc comments
- **Package comments:** `// Package X provides...`
- **No obvious comments:** Don't comment what code does, explain why
- **TODO format:** `// TODO(username): description`

Example:

```go
// ExecuteWorkflow runs the seven-phase workflow for the given goal.
// It returns an error if any phase fails or if the context is cancelled.
func ExecuteWorkflow(ctx context.Context, goal string) error {
    // ...
}
```

## Project-Specific Conventions

### Bubble Tea (TUI)

- **Single-threaded:** Never mutate state from goroutines
- **Message-based:** All state changes via `Update()` method
- **Commands:** Return `tea.Cmd` for side effects
- **Models:** Implement `tea.Model` interface

### Workflow Engine

- **Phase transitions:** Use `transitionMu` mutex
- **State mutations:** Only through `WorkflowState` methods
- **LLM calls:** Always with timeout and retry

### Provider Layer

- **Interface-based:** All providers implement `Provider` interface
- **Registry pattern:** Dynamic registration via `provider.Register()`
- **Failover:** Automatic fallback between providers

### Tool System

- **Dispatcher pattern:** All tools go through `dispatcher.go`
- **Permission system:** Tools require user approval
- **Sandboxing:** Bash commands are security-sandboxed

## Testing Conventions

### Test Organization

- **Parallel tests:** Use `t.Parallel()` for independent tests
- **Table-driven:** Use table-driven tests for multiple cases
- **Test files:** `*_test.go` in same package
- **Test helpers:** In `tests/testutil/`

### Test Naming

- **Function:** `TestFunctionName_Scenario`
- **Table-driven:** `TestFunctionName` with subtests
- **Benchmarks:** `BenchmarkFunctionName`

### Test Patterns

```go
func TestParseInput(t *testing.T) {
    t.Parallel()
    
    tests := []struct {
        name    string
        input   string
        want    Result
        wantErr bool
    }{
        {
            name:    "valid input",
            input:   "test",
            want:    Result{Value: "test"},
            wantErr: false,
        },
        {
            name:    "empty input",
            input:   "",
            wantErr: true,
        },
    }
    
    for _, tt := range tests {
        t.Run(tt.name, func(t *testing.T) {
            t.Parallel()
            got, err := ParseInput(tt.input)
            if (err != nil) != tt.wantErr {
                t.Errorf("ParseInput() error = %v, wantErr %v", err, tt.wantErr)
                return
            }
            if got != tt.want {
                t.Errorf("ParseInput() = %v, want %v", got, tt.want)
            }
        })
    }
}
```

### Mocking

- **Interface-based:** Mock by implementing interfaces
- **Test doubles:** In `tests/testutil/`
- **HTTP mocking:** Use `httptest.NewServer`

## Configuration Conventions

### TOML Configuration

- **File:** `m31a.toml`
- **Sections:** `[provider]`, `[workflow]`, `[tools]`
- **Defaults:** Always provide sensible defaults
- **Validation:** Schema-based validation

### Environment Variables

- **Prefix:** `M31A_` for all env vars
- **Examples:** `M31A_API_KEY`, `M31A_LOG_LEVEL`
- **Priority:** Env vars override config file

## Build Conventions

### Makefile

- **Targets:** Lowercase, hyphen-separated
- **Comments:** `## target — description`
- **Phony targets:** Explicit `.PHONY` declarations

### Cross-Compilation

- **Platforms:** linux, darwin, windows
- **Architectures:** amd64, arm64
- **Static binary:** `CGO_ENABLED=0`

## Documentation Conventions

### README Files

- **Location:** Root and key directories
- **Format:** Markdown with badges
- **Content:** Overview, install, usage, examples

### Code Comments

- **Doc comments:** Always for exported symbols
- **TODO format:** `// TODO(username): description`
- **No obvious comments:** Don't comment what code does

## Git Conventions

### Commit Messages

- **Format:** `<type>: <description>`
- **Types:** feat, fix, docs, test, refactor, chore
- **Examples:**
  - `feat: add new tool for code analysis`
  - `fix: handle nil pointer in parser`
  - `docs: update README with examples`

### Branch Naming

- **Feature:** `feature/description`
- **Fix:** `fix/description`
- **Release:** `release/v1.2.0`

## Security Conventions

### API Keys

- **Storage:** OS keychain only
- **Never:** In config files, environment variables, or logs
- **Retrieval:** `pkg/keychain/` package

### Command Execution

- **Sandboxing:** All bash commands are sandboxed
- **Dangerous commands:** Blocked by security policy
- **User approval:** Required for all tool execution

## Performance Conventions

### Caching

- **Pattern:** `sync.Once` for lazy initialization
- **Invalidation:** TTL-based or event-driven
- **Storage:** In-memory with optional persistence

### Concurrency

- **Mutexes:** Use `sync.RWMutex` for read-heavy workloads
- **Channels:** For inter-goroutine communication
- **Context:** Always pass context for cancellation

## Logging Conventions

### Structured Logging

- **Framework:** `log/slog`
- **Levels:** Debug, Info, Warn, Error
- **Fields:** Key-value pairs for context

Example:

```go
slog.Info("workflow started",
    "goal", goal,
    "provider", provider,
    "mode", mode,
)
```