# ROLE

You are the **Technical Architect and Lead Go Engineer** for M31A — a terminal-based
AI coding assistant written in Go. You are executing Phase 0 of a multi-phase build
plan. Your job in this phase is to produce project scaffolding, real Go interface/type
files, CI/CD configuration, and documentation.

Do not invent features. Do not add packages not listed. Do not write code that is
"nice to have." You are building a precise foundation that every future phase depends
on. Deviation = breakage downstream.

---

# PROJECT IDENTITY (READ BEFORE EVERYTHING ELSE)

M31A is a terminal-based AI coding assistant written in Go 1.22+.
- Module path: github.com/eshanized/M31A
- Binary: single static binary (CGO_ENABLED=0)
- UI: Bubble Tea + Lipgloss + Bubbles + Glamour (Charm stack)
- License: MIT — NO telemetry, NO vendor lock-in, NO paid tiers
- LLM providers: ONLY OpenRouter (https://openrouter.ai/api/v1) and OpenCode Zen
  (https://opencode.ai/zen/v1). NEVER direct Anthropic or OpenAI connections.
- Target OS: Linux, macOS, Windows

The six-phase workflow engine (Initialize → Discuss → Plan → Execute → Verify → Ship)
is the core of M31A. Each phase runs in a fresh, pruned context. State persists via
human-readable Markdown files in ~/.m31a/sessions/<id>/planning/.

---

# PHASE 0 MISSION

Create every foundation file that future phases will reference. This includes:
1. OpenCode project configuration (AGENTS.md + opencode.json)
2. GSD planning artifacts (PROJECT.md, REQUIREMENTS.md, STATE.md, CONTEXT.md)
3. Go project structure (go.mod, Makefile, .gitignore, directory skeleton)
4. Real Go interface and type files (NOT documentation — actual compilable Go code)
5. Structured logging implementation (internal/log/log.go)
6. CI/CD configuration (.github/workflows/ci.yml, .goreleaser.yaml, .golangci.yml)
7. Technical documentation in docs/ (ARCHITECTURE.md, INTERFACES.md, TYPES.md)
8. The walkthrough document (walkthrough_0.md)

---

# DELIVERABLES

Create EXACTLY these files. No more, no less.

## 1. AGENTS.md (project root)

OpenCode reads this on every session. It must contain:

### Section: Project Overview
- M31A is a Go 1.22+ terminal AI coding agent
- Module: github.com/eshanized/M31A
- Binary: CGO_ENABLED=0 static binary
- UI: charmbracelet/bubbletea + charmbracelet/lipgloss + charmbracelet/bubbles +
  charmbracelet/glamour

### Section: Build & Test Commands

```
Build: CGO_ENABLED=0 go build -o m31a ./cmd/m31a
Test:  go test -race -cover ./...
Lint:  golangci-lint run ./...
Vet:   go vet ./...
Clean: rm -f m31a
```

### Section: Architecture Rules (CRITICAL — agent must follow these)
- Bubble Tea is single-threaded. ALL state mutations go through Update() only.
  Never mutate AppState from a goroutine. Use tea.Cmd and tea.Msg.
- HTTP client: 30s dial timeout via DialContext. NO body read timeout (streaming).
- Context pruning: each workflow phase discards prior conversation; reads state from
  planning/ files only.
- No direct Anthropic/OpenAI connections. Only OpenRouter and Zen gateways.
- No CGO. Binary must be static (CGO_ENABLED=0).
- No telemetry. No analytics. No external calls except to OpenRouter/Zen APIs.
- API keys: resolved in order: env var → OS keychain → config file. Never plaintext.
- V1 tools: Bash, FileRead, FileWrite, Glob, Grep ONLY.
  Do NOT implement FileEdit, WebFetch, WebSearch, AgentTool, TaskTool,
  AskUserQuestion, or GitTool in V1.
- V1 task execution is SEQUENTIAL. Do not implement concurrency in V1.
- No hardcoded model lists. Models discovered dynamically from provider APIs.

### Section: Package Layout

```
cmd/m31a/          — binary entry point only, no logic
internal/config/   — config parsing, env vars, keychain resolution
internal/provider/ — LLMProvider interface + OpenRouter + Zen clients
internal/tui/      — Bubble Tea app, all screens
internal/workflow/ — six workflow phases
internal/tools/    — Bash, FileRead, FileWrite, Glob, Grep
internal/log/      — structured logger (slog) with rotation
internal/git/      — git operations wrapper
internal/types/    — shared core types (Message, Task, ToolCall, etc.)
pkg/taskrunner/    — dependency graph, topological sort, task lifecycle
pkg/arbitrage/     — complexity scoring, model cost comparison
pkg/bisect/        — git bisect wrapper
pkg/ledger/        — cross-session learning ledger
pkg/rollback/      — commit chain browser
pkg/autodream/     — context consolidation
pkg/session/       — session lifecycle, file persistence
pkg/keychain/      — OS-specific keychain (linux/darwin/windows)
```

### Section: Absolute Prohibitions
- DO NOT add direct Anthropic or OpenAI provider support
- DO NOT use CSS-style animations (Bubble Tea uses Unicode spinners + frame redraws)
- DO NOT implement V1.1 features (ghost mode, PiP, subagents, deferred tools)
- DO NOT store API keys in plaintext
- DO NOT assume concurrent task execution in V1
- DO NOT add telemetry or analytics
- DO NOT hardcode model lists
- DO NOT use the AskUserQuestion tool in V1

### Section: File Paths That Always Matter
- Config: ~/.m31a/config.toml
- Sessions: ~/.m31a/sessions/<id>/
- Ledger: ~/.m31a/LEDGER.md
- Global AGENTS: ~/.config/opencode/AGENTS.md

### Section: Key Go Dependencies (use only these)

```
charmbracelet/bubbletea    — TUI framework
charmbracelet/lipgloss     — terminal styling
charmbracelet/bubbles      — TUI components (textarea, viewport, spinner, list)
charmbracelet/glamour      — Markdown rendering
BurntSushi/toml            — config parsing
tiktoken-go                — token estimation for GPT/Claude families
doublestar                 — glob pattern matching with ** support
creack/pty                 — PTY for Bash tool on Linux/macOS
```

---

## 2. opencode.json (project root)

```json
{
  "$schema": "https://opencode.ai/config.json",
  "instructions": [
    "AGENTS.md",
    "docs/ARCHITECTURE.md",
    "docs/INTERFACES.md",
    "docs/TYPES.md",
    "Adrenaline/ROADMAP.md"
  ]
}
```

---

## 3. .planning/PROJECT.md

GSD artifact. Content:

```markdown
# M31A — Project

## Goal
Build a terminal-based AI coding assistant in Go that runs a six-phase
spec-driven workflow engine (Initialize → Discuss → Plan → Execute →
Verify → Ship), communicates exclusively through OpenRouter and OpenCode
Zen gateways, renders a Gemini-caliber TUI via the Charm library stack,
and ships as a single static binary under MIT license with no telemetry.

## Project Type
Go · CLI · TUI · Systems

## Module
github.com/eshanized/M31A

## Framework
Bubble Tea (charmbracelet/bubbletea) + Lipgloss + Bubbles + Glamour

## Build Constraint
CGO_ENABLED=0 — static binary only

## License
MIT — no vendor lock-in, no telemetry, no paid tiers
```

---

## 4. .planning/REQUIREMENTS.md

GSD artifact. List every V1 acceptance criterion from the spec, verbatim, as a numbered checklist. Include all 26 criteria from the original spec plus the 6 differentiator criteria.

Write the FULL content — do NOT use "[continue for all 26]" as a shortcut. Expand every single criterion with its full description from `Adrenaline/idea.md` sections 15 and 17.

---

## 5. .planning/STATE.md

GSD artifact.

```markdown
# M31A — Current State

## Active Phase
Phase 0 — Foundation & Documentation

## Status
Complete

## Completed Phases
- Phase 0 — Foundation & Documentation

## Next Phase
Phase 1 — Provider Abstraction Layer

## Key Decisions Made
- Sequential task execution in V1 (no concurrency)
- Only OpenRouter and Zen as LLM gateways
- CGO_ENABLED=0 static binary
- AGENTS.md established as project memory for OpenCode
- Real Go interface files created in internal/ (source of truth)
- docs/INTERFACES.md mirrors Go interfaces for documentation purposes

## Blockers
None
```

---

## 6. .planning/CONTEXT.md

GSD artifact. Captures Phase 0 implementation decisions.

```markdown
# Phase 0 — Context & Decisions

## What Phase 0 Establishes
- AGENTS.md: OpenCode project memory with all architectural constraints
- opencode.json: Instruction file chain for all future sessions
- Real Go interface files: internal/errors/, internal/types/, internal/provider/interface.go, internal/tools/interface.go, internal/config/types.go
- docs/INTERFACES.md: Mirrors Go interfaces for LLM context in future sessions
- docs/TYPES.md: Error sentinels, env vars, constants reference
- docs/ARCHITECTURE.md: Package dependency and data flow
- Go module structure: directory skeleton, go.mod, Makefile
- CI/CD: GitHub Actions matrix, goreleaser, golangci-lint
- Logging: structured slog with rotation to ~/.m31a/m31a.log

## Interface Decisions
See internal/types/ and internal/provider/interface.go for canonical definitions.
docs/INTERFACES.md mirrors these for human/LLM readability.

## State Persistence Decision
All workflow state stored as human-readable Markdown + JSON in
~/.m31a/sessions/<id>/planning/. Never binary formats. Always resumable.

## Token Estimation Decision
Two-phase: client-side tiktoken-go during streaming, server calibration
from final SSE chunk usage field. EMA alpha=0.3 correction factor per
model family.

## Context Pruning Decision
Each workflow phase discards prior conversation. Reads only structured
state files (PROJECT.md, TASKS.md, STATE.md) plus system prompt. No
conversation history carried between phases.
```

---

## 7. go.mod (project root)

```
module github.com/eshanized/M31A

go 1.22
```

Do NOT pre-declare dependencies. `go mod tidy` will add them as code imports them.
Phase 0 code only imports: `fmt`, `log/slog`, `os`, `context`, `errors`, `time`,
`path/filepath`, `io`, `strings`, `encoding/json`, `sync`, `bufio`.

After creating all Go files, run: `go mod tidy`

---

## 8. Makefile (project root)

```makefile
BINARY  := m31a
MODULE  := github.com/eshanized/M31A
VERSION := $(shell git describe --tags --always --dirty 2>/dev/null || echo "dev")
LDFLAGS := -ldflags "-X main.Version=$(VERSION) -s -w"

.PHONY: build test lint vet clean release

build:
	CGO_ENABLED=0 go build $(LDFLAGS) -o $(BINARY) ./cmd/m31a

test:
	go test -race -cover -coverprofile=coverage.out ./...

lint:
	golangci-lint run ./...

vet:
	go vet ./...

clean:
	rm -f $(BINARY) coverage.out

release:
	goreleaser release --snapshot --clean

.DEFAULT_GOAL := build
```

---

## 9. .gitignore (project root)

```
# Binaries
m31a
*.exe
*.test

# Build artifacts
coverage.out
/dist/
/.goreleaser-dist/

# Session data (runtime, not development)
/.m31a/

# Editor files
*.swp
*.swo
*~
.DS_Store

# Vendor
/vendor/
```

---

## 10. Directory skeleton

Create these empty directories with a .gitkeep file each:

```
cmd/m31a/
internal/config/
internal/errors/
internal/git/
internal/log/
internal/provider/
internal/tools/
internal/tui/
internal/types/
internal/workflow/
pkg/taskrunner/
pkg/arbitrage/
pkg/bisect/
pkg/ledger/
pkg/rollback/
pkg/autodream/
pkg/session/
pkg/keychain/
docs/
.github/workflows/
.planning/
```

---

## 11. cmd/m31a/main.go

This is the main entry point. It initializes the structured logger and prints version.

```go
package main

import (
    "fmt"
    "os"
    "runtime"

    "github.com/eshanized/M31A/internal/log"
)

// Version is set at build time via ldflags.
var Version = "dev"

func main() {
    // Initialize structured logger before any other code.
    logger, cleanup, err := log.NewLogger(Version)
    if err != nil {
        fmt.Fprintf(os.Stderr, "failed to initialize logger: %v\n", err)
        os.Exit(1)
    }
    defer cleanup()

    logger.Info("M31A starting",
        "version", Version,
        "go_version", runtime.Version(),
        "os", runtime.GOOS,
        "arch", runtime.GOARCH,
    )

    fmt.Printf("M31A %s — AI coding assistant\n", Version)
    fmt.Println("Run 'make build' to build. Implementation coming in Phase 1+.")
}
```

---

## 12. internal/log/log.go

Structured logger using log/slog. Writes to ~/.m31a/m31a.log only.
Implements daily rotation with 7-day retention. Configurable via env vars.

Write a complete, compilable implementation with these exported functions:

```go
// NewLogger creates a structured slog.Logger that writes to ~/.m31a/m31a.log.
// Returns the logger, a cleanup function (to close the file handle), and any error.
// The cleanup function must be called via defer.
func NewLogger(version string) (*slog.Logger, func(), error)

// DefaultLogger returns the package-level default logger.
// Must only be called after NewLogger has been called once.
func DefaultLogger() *slog.Logger
```

Implementation requirements:
- Log file path: ~/.m31a/m31a.log (create directory if missing)
- Rotation: on each NewLogger call, check if a log file from a previous day exists.
  If so, rename it to m31a.log.YYYY-MM-DD. Delete any rotated files older than 7 days.
- Format: JSON by default. If M31A_LOG_FORMAT=text env var is set, use text format.
- Level: INFO by default. If M31A_LOG_LEVEL=debug env var is set, use DEBUG level.
- NO stdout/stderr output during TUI operation — logger writes to file only.
- NO telemetry, NO analytics, NO external HTTP calls.
- Use only Go stdlib: log/slog, os, path/filepath, time, fmt, io, strings.

---

## 13. internal/errors/errors.go

Define all sentinel errors. Each must be a var with errors.New():

```go
package errors

import "errors"

var (
    ErrProviderUnreachable = errors.New("provider unreachable")
    ErrRateLimited         = errors.New("rate limited")
    ErrInvalidKey          = errors.New("invalid API key")
    ErrContextExceeded     = errors.New("context window exceeded")
    ErrModelNotFound       = errors.New("model not found")
    ErrSessionCorrupted    = errors.New("session data corrupted")
    ErrNoBinaryContent     = errors.New("binary content not displayable")
    ErrFileTooLarge        = errors.New("file exceeds 5MB limit")
    ErrCircularDependency  = errors.New("circular dependency in task graph")
    ErrBisectFailed        = errors.New("git bisect failed to identify regression")
    ErrPermissionDenied    = errors.New("permission denied")
    ErrToolExecution       = errors.New("tool execution failed")
    ErrTaskFailed          = errors.New("task failed")
    ErrPhaseTransition     = errors.New("invalid phase transition")
    ErrCheckpointNotFound  = errors.New("checkpoint not found")
)
```

---

## 14. internal/types/types.go

All shared core types in one file. Must compile. Import: context, encoding/json, time.

```go
package types

import (
    "context"
    "encoding/json"
    "time"
)
```

### RiskLevel

```go
type RiskLevel string

const (
    RiskSafe        RiskLevel = "safe"
    RiskMedium      RiskLevel = "medium"
    RiskDangerous   RiskLevel = "dangerous"
    RiskDestructive RiskLevel = "destructive"
)
```

### WorkflowPhase

```go
type WorkflowPhase string

const (
    PhaseIdle       WorkflowPhase = "idle"
    PhaseInitialize WorkflowPhase = "initialize"
    PhaseDiscuss    WorkflowPhase = "discuss"
    PhasePlan       WorkflowPhase = "plan"
    PhaseExecute    WorkflowPhase = "execute"
    PhaseVerify     WorkflowPhase = "verify"
    PhaseShip       WorkflowPhase = "ship"
)
```

### TaskStatus

```go
type TaskStatus string

const (
    StatusPending      TaskStatus = "pending"
    StatusRunning      TaskStatus = "running"
    StatusDone         TaskStatus = "done"
    StatusFailed       TaskStatus = "failed"
    StatusSkipped      TaskStatus = "skipped"
    StatusUnrecoverable TaskStatus = "unrecoverable"
)
```

### Usage

```go
// Usage tracks token consumption from an LLM response.
type Usage struct {
    PromptTokens     int `json:"prompt_tokens"`
    CompletionTokens int `json:"completion_tokens"`
    TotalTokens      int `json:"total_tokens"`
}
```

### CapFlags

```go
// CapFlags describes model capabilities.
type CapFlags struct {
    Tools     bool `json:"tools"`      // Supports function calling / tool use
    Reasoning bool `json:"reasoning"`  // Supports explicit thinking/reasoning tokens
    Vision    bool `json:"vision"`     // Supports image input (reserved for V1.2)
}
```

### Pricing

```go
// Pricing holds per-model pricing in USD per million tokens.
type Pricing struct {
    InputPerMToken  float64 `json:"input_per_m_token"`
    OutputPerMToken float64 `json:"output_per_m_token"`
}
```

### ArchInfo

```go
// ArchInfo describes the model's architecture.
type ArchInfo struct {
    TokenizerFamily string `json:"tokenizer_family"`
}
```

### ModelInfo

```go
// ModelInfo describes an LLM model available through a provider.
type ModelInfo struct {
    ID            string   `json:"id"`             // e.g. "anthropic/claude-3.5-sonnet"
    Provider      string   `json:"provider"`       // "openrouter" | "zen"
    Name          string   `json:"name"`           // Human-readable name
    Description   string   `json:"description"`    // Model description
    ContextLength int64    `json:"context_length"` // Maximum context window
    Pricing       Pricing  `json:"pricing"`
    Architecture  ArchInfo `json:"architecture"`
    TopProvider   string   `json:"top_provider"`   // Latency-ranked best provider (OpenRouter only)
    Capabilities  CapFlags `json:"capabilities"`
}
```

### MessageSegment

```go
// MessageSegment is a single segment within a message, either content or thinking.
type MessageSegment struct {
    Type       string `json:"type"`        // "content" | "thinking" | "memory"
    Content    string `json:"content"`     // Segment text
    DurationMs int64  `json:"duration_ms"` // For thinking segments only
    Visible    bool   `json:"visible"`     // User toggle state (default true)
}
```

### ToolCall

```go
// ToolCall represents a tool invocation requested by the LLM.
type ToolCall struct {
    ID    string          `json:"id"`
    Name  string          `json:"name"`
    Input json.RawMessage `json:"input"`
}
```

### Message

```go
// Message represents a single message in the conversation history.
type Message struct {
    Role      string           `json:"role"`       // "user" | "assistant" | "tool"
    Content   string           `json:"content"`    // Final assembled text (thinking stripped)
    Segments  []MessageSegment `json:"segments"`   // Ordered: content → thinking → content...
    ToolCalls []ToolCall       `json:"tool_calls,omitempty"`
    Usage     *Usage           `json:"usage,omitempty"` // Only set on assistant messages
    CreatedAt time.Time        `json:"created_at"`
}
```

### ToolInput

```go
// ToolInput carries the parameters for a tool execution.
type ToolInput struct {
    Name   string         `json:"name"`
    Params map[string]any `json:"params"`
}
```

### ToolResult

```go
// ToolResult is the output of a tool execution.
type ToolResult struct {
    ToolCallID string `json:"tool_call_id"`
    Output     string `json:"output"`
    Error      string `json:"error,omitempty"`
    DurationMs int64  `json:"duration_ms"`
    Truncated  bool   `json:"truncated"`
}
```

### Tool interface

```go
// Tool is the interface that all tools must implement.
type Tool interface {
    Name() string
    Description() string
    RiskLevel() RiskLevel
    Execute(ctx context.Context, input ToolInput) (ToolResult, error)
}
```

### FilePrediction

```go
// FilePrediction is the LLM's predicted file change for a task.
type FilePrediction struct {
    Path           string `json:"path"`
    Action         string `json:"action"`          // "create" | "modify" | "delete"
    EstimatedLines int    `json:"estimated_lines"` // Approximate line count
}
```

### Task

```go
// Task represents a single unit of work in the plan.
type Task struct {
    ID                 int              `json:"id"`
    Description        string           `json:"description"`
    Action             string           `json:"action"`              // "NEW" | "EDIT" | "DELETE"
    Dependencies       []int            `json:"dependencies"`        // Task IDs that must complete first
    Files              []string         `json:"files"`               // Files this task touches
    AcceptanceCriteria []string         `json:"acceptance_criteria"`
    Status             TaskStatus       `json:"status"`
    HealsAttempted     int              `json:"heals_attempted"`
    PredictedFiles     []FilePrediction `json:"predicted_files,omitempty"`
    CommitHash         string           `json:"commit_hash,omitempty"`
}
```

### ProjectState

```go
// ProjectState captures the project context from the Discuss phase.
type ProjectState struct {
    Goal        string            `json:"goal"`
    ProjectType string            `json:"project_type"` // "web" | "go" | "python" | etc.
    Framework   string            `json:"framework"`
    Answers     map[string]string `json:"answers"`      // Discuss phase Q&A
    CreatedAt   time.Time         `json:"created_at"`
}
```

### Session

```go
// Session metadata for persistence.
type Session struct {
    ID            string        `json:"id"`
    Model         string        `json:"model"`
    Provider      string        `json:"provider"`
    StartedAt     time.Time     `json:"started_at"`
    MessageCount  int           `json:"message_count"`
    WorkflowPhase WorkflowPhase `json:"workflow_phase"`
    Project       *ProjectState `json:"project,omitempty"`
}
```

### StreamChunk

```go
// StreamChunk is a single chunk from an SSE stream.
type StreamChunk struct {
    Type            string `json:"type"`              // "content" | "thinking" | "done"
    Delta           string `json:"delta"`             // Content or thinking token
    ThinkingDuration int64 `json:"thinking_duration"` // ms, only for thinking segments
}
```

### StreamIterator

```go
// StreamIterator wraps an SSE response into a chunk-by-chunk iterator.
type StreamIterator struct {
    // Next returns the next chunk from the stream.
    // Returns io.EOF when the stream ends.
    Next func() (*StreamChunk, error)
    // Close releases underlying response resources.
    Close func() error
}
```

### HealthStatus

```go
// HealthStatus describes the health of an LLM provider.
type HealthStatus struct {
    Status    string `json:"status"`     // "live" | "slow" | "offline"
    LatencyMs int64  `json:"latency_ms"`
    Error     string `json:"error,omitempty"`
}
```

---

## 15. internal/provider/interface.go

The LLMProvider interface. This file imports internal/types and internal/errors.

```go
package provider

import (
    "context"

    "github.com/eshanized/M31A/internal/errors"
    "github.com/eshanized/M31A/internal/types"
)

// LLMProvider is the interface all LLM gateway clients must implement.
type LLMProvider interface {
    // Name returns the provider identifier (e.g. "openrouter" or "zen").
    Name() string

    // FetchModels retrieves the model catalog from the provider API.
    // Results should be cached with a 5-minute TTL.
    FetchModels(ctx context.Context) ([]types.ModelInfo, error)

    // ChatCompletionStream starts a streaming chat completion request.
    // Returns a StreamIterator that yields chunks one at a time.
    ChatCompletionStream(ctx context.Context, req ChatRequest) (*types.StreamIterator, error)

    // EstimateCost calculates the cost in USD for the given token usage.
    EstimateCost(usage types.Usage) float64

    // HealthCheck probes the provider's health endpoint.
    HealthCheck(ctx context.Context) types.HealthStatus

    // GetModel returns metadata for a specific model from the cache.
    // Returns errors.ErrModelNotFound if the model is not in cache.
    GetModel(id string) (*types.ModelInfo, error)
}

// ChatRequest is the request body for a chat completion call.
type ChatRequest struct {
    Model             string           `json:"model"`
    Messages          []types.Message  `json:"messages"`
    MaxTokens         int              `json:"max_tokens,omitempty"`
    Stream            bool             `json:"stream"`
    Tools             []ToolDefinition `json:"tools,omitempty"`
    ReasoningEnabled  bool             `json:"reasoning_enabled,omitempty"`
}

// ToolDefinition describes a tool available for function calling.
type ToolDefinition struct {
    Name        string `json:"name"`
    Description string `json:"description"`
    Parameters  string `json:"parameters"` // JSON schema as string
}

// ProviderRegistry manages multiple LLM providers.
type ProviderRegistry struct {
    // Active returns the currently selected provider name.
    Active func() string
    // SetActive switches the active provider.
    SetActive func(name string) error
    // Get returns a provider by name.
    Get func(name string) (LLMProvider, error)
    // List returns all registered provider names.
    List func() []string
}
```

---

## 16. internal/tools/interface.go

Tool interface (already defined in internal/types/types.go). This file adds the
dispatcher and permission request types.

```go
package tools

import (
    "context"

    "github.com/eshanized/M31A/internal/types"
)

// PermissionRequest is emitted when a dangerous tool requires user approval.
type PermissionRequest struct {
    ToolName    string            `json:"tool_name"`
    Command     string            `json:"command"`
    RiskLevel   types.RiskLevel   `json:"risk_level"`
    TimeoutSecs int               `json:"timeout_secs"`
}

// PermissionResponse is the user's response to a permission request.
type PermissionResponse struct {
    Allowed   bool `json:"allowed"`
    Remember  bool `json:"remember"` // If true, remember this choice for future requests
}

// Dispatcher routes ToolCalls to the appropriate tool implementation.
type Dispatcher struct {
    // Register adds a tool implementation to the dispatcher.
    Register func(name string, tool types.Tool)
    // Execute dispatches a ToolCall to the registered tool.
    // Returns ErrPermissionDenied if the tool requires approval and hasn't been granted.
    Execute func(ctx context.Context, call types.ToolCall) (types.ToolResult, error)
    // List returns all registered tool names.
    List func() []string
}
```

---

## 17. internal/config/types.go

Full config struct matching the config.toml schema from the spec.

```go
package config

import "github.com/eshanized/M31A/internal/types"

// Config is the top-level configuration for M31A.
type Config struct {
    Provider    ProviderConfig    `toml:"provider"`
    Model       ModelConfig       `toml:"model"`
    UI          UIConfig          `toml:"ui"`
    Permissions PermissionsConfig `toml:"permissions"`
    Features    FeaturesConfig    `toml:"features"`
    Ledger      LedgerConfig      `toml:"ledger"`
    Ghost       GhostConfig       `toml:"ghost"`
}

// ProviderConfig holds per-provider API key and settings.
type ProviderConfig struct {
    Default      string                    `toml:"default"`       // "openrouter" | "zen"
    AutoFallback bool                      `toml:"auto_fallback"` // Auto-switch on 503/429
    OpenRouter   ProviderCredentialConfig  `toml:"openrouter"`
    Zen          ProviderCredentialConfig  `toml:"zen"`
}

// ProviderCredentialConfig holds API key storage location for a provider.
type ProviderCredentialConfig struct {
    APIKey string `toml:"api_key"` // Resolved: env > keychain > this file
}

// ModelConfig holds model selection and behavior settings.
type ModelConfig struct {
    Default                 string  `toml:"default"`                   // provider/model-id format
    ContextWarningThreshold float64 `toml:"context_warning_threshold"` // Percentage (0-100)
    ShowThinkingByDefault   bool    `toml:"show_thinking_by_default"`
    AutoCollapseTools       bool    `toml:"auto_collapse_tools"`
    AutoArbitrage           bool    `toml:"auto_arbitrage"`
    ArbitrageThreshold      float64 `toml:"arbitrage_threshold"` // Complexity score 0-1
}

// UIConfig holds appearance settings.
type UIConfig struct {
    Theme         string `toml:"theme"`          // "dark" | "light" | "auto"
    CompactMode   bool   `toml:"compact_mode"`
    ShowTokenUsage bool  `toml:"show_token_usage"`
    ShowCostEstimate bool `toml:"show_cost_estimate"`
    MaxIterations  int   `toml:"max_iterations"`
}

// PermissionsConfig holds tool permission settings.
type PermissionsConfig struct {
    DefaultMode    string            `toml:"default_mode"` // "default" | "auto" | "bypass"
    TimeoutSeconds int               `toml:"timeout_seconds"`
    Rules          []PermissionRule  `toml:"rules"`
}

// PermissionRule matches a tool/pattern to a risk level and action.
type PermissionRule struct {
    Tool      string            `toml:"tool"`
    Pattern   string            `toml:"pattern"`
    RiskLevel types.RiskLevel   `toml:"risk_level"`
    Action    string            `toml:"action"` // "ask" | "allow" | "deny"
}

// FeaturesConfig holds feature toggle settings.
type FeaturesConfig struct {
    AutodreamEnabled bool `toml:"autodream_enabled"`
    SubagentEnabled  bool `toml:"subagent_enabled"`
    AutoBackup       bool `toml:"auto_backup"`
    ResumeOnStartup   bool `toml:"resume_on_startup"`
}

// LedgerConfig holds cross-session learning ledger settings.
type LedgerConfig struct {
    Enabled    bool `toml:"enabled"`
    MaxEntries int  `toml:"max_entries"`
}

// GhostConfig holds ghost mode settings.
type GhostConfig struct {
    Enabled bool `toml:"enabled"`
}
```

---

## 18. internal/types/constants.go

All package-level constants.

```go
package types

import "time"

const (
    // ModelCacheTTL is how long model catalogs are cached before refresh.
    ModelCacheTTL = 5 * time.Minute

    // HealthCheckInterval is the default polling interval for provider health checks.
    HealthCheckInterval = 60 * time.Second

    // MaxFileSize is the maximum file size allowed for FileRead (5MB).
    MaxFileSize = 5 * 1024 * 1024

    // MaxToolOutputChars caps tool output in the TUI (longer outputs are truncated).
    MaxToolOutputChars = 10_000

    // MaxHealAttempts is the maximum number of self-heal retries for a failed task.
    MaxHealAttempts = 2

    // MaxPlanRetries is the maximum number of plan regeneration attempts on validation failure.
    MaxPlanRetries = 3

    // SessionIDLength is the number of characters in a session ID (lowercase alphanumeric).
    SessionIDLength = 8

    // AutoDreamThreshold is the context usage percentage that triggers AutoDream consolidation.
    AutoDreamThreshold = 0.60

    // ContextWarningThreshold is the context usage percentage that shows a warning banner.
    ContextWarningThreshold = 0.80

    // HTTPDialTimeout is the dial timeout for provider HTTP connections (30s).
    HTTPDialTimeout = 30 * time.Second

    // BashTimeout is the absolute timeout for Bash tool execution (30 minutes).
    BashTimeout = 30 * time.Minute

    // BashOutputLimit is the max characters to stream from Bash output to the TUI.
    BashOutputLimit = 50_000

    // DefaultContextLength is the fallback context length if model metadata is unavailable.
    DefaultContextLength = 128_000
)
```

---

## 19. .github/workflows/ci.yml

```yaml
name: CI

on:
  push:
    branches: [main, dev]
  pull_request:
    branches: [main]
  workflow_dispatch:

jobs:
  lint:
    runs-on: ubuntu-latest
    timeout-minutes: 10
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-go@v5
        with:
          go-version: "1.22"
      - name: golangci-lint
        uses: golangci/golangci-lint-action@v4
        with:
          version: latest
          args: --timeout=5m

  test:
    runs-on: ubuntu-latest
    timeout-minutes: 10
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-go@v5
        with:
          go-version: "1.22"
      - name: Run tests
        run: go test -race -coverprofile=coverage.out -covermode=atomic ./...
      - name: Upload coverage
        uses: actions/upload-artifact@v4
        with:
          name: coverage
          path: coverage.out

  build:
    strategy:
      matrix:
        os: [ubuntu-latest, macos-latest, windows-latest]
        arch: [amd64, arm64]
        go: ["1.22"]
        exclude:
          # Skip arm64 on windows (not a standard target)
          - os: windows-latest
            arch: arm64
    runs-on: ${{ matrix.os }}
    timeout-minutes: 10
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-go@v5
        with:
          go-version: ${{ matrix.go }}
      - name: Build
        run: CGO_ENABLED=0 GOARCH=${{ matrix.arch }} go build -o m31a ./cmd/m31a
        env:
          CGO_ENABLED: "0"
          GOARCH: ${{ matrix.arch }}
      - name: Vet
        run: GOARCH=${{ matrix.arch }} go vet ./cmd/m31a
        env:
          GOARCH: ${{ matrix.arch }}

  release:
    needs: [lint, test, build]
    if: startsWith(github.ref, 'refs/tags/v')
    runs-on: ubuntu-latest
    timeout-minutes: 15
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0
      - uses: actions/setup-go@v5
        with:
          go-version: "1.22"
      - name: Run GoReleaser
        uses: goreleaser/goreleaser-action@v5
        with:
          distribution: goreleaser
          version: latest
          args: release --clean
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
```

---

## 20. .goreleaser.yaml

```yaml
version: 2

project_name: m31a

builds:
  - env:
      - CGO_ENABLED=0
    goos:
      - linux
      - darwin
      - windows
    goarch:
      - amd64
      - arm64
    binary: m31a
    main: ./cmd/m31a
    ldflags:
      - -s -w
      - -X main.Version={{.Version}}
    ignore:
      - goos: windows
        goarch: arm64

archives:
  - format: tar.gz
    name_template: >-
      {{ .ProjectName }}_{{ .Version }}_{{ .Os }}_{{ .Arch }}
    format_overrides:
      - goos: windows
        format: zip

checksum:
  name_template: 'checksums.txt'
  algorithm: sha256

release:
  draft: true
  prerelease: auto
```

---

## 21. .golangci.yml

```yaml
run:
  timeout: 5m

linters:
  enable:
    - govet
    - staticcheck
    - errcheck
    - ineffassign
    - unused
    - gosimple

linters-settings:
  govet:
    check-shadowing: true
  errcheck:
    check-type-assertions: false
    check-blank: false

issues:
  exclude-rules:
    - path: _test\.go
      linters:
        - errcheck
        - unused
```

---

## 22. docs/ARCHITECTURE.md

Write a comprehensive architecture document covering:

### Package Dependency Graph

Full ASCII dependency tree matching the package layout in AGENTS.md. No circular dependencies.

### Data Flow: Provider Layer

Document the complete streaming path: user input → TUI → ChatCompletionStream → HTTP POST → SSE parser → StreamIterator.Next() → StreamChunk → TUI message renderer. Include reasoning/thinking segment detection.

### Data Flow: Workflow Engine

Document all six phases with their context injection table (what each phase receives, what it discards, what state files it reads/writes). Reference idea.md §7 for phase lifecycle details.

### State Persistence Model

Document the full session directory structure with descriptions for each file.

### Threading Model

Document: single-threaded Bubble Tea update loop, tea.Cmd/tea.Msg for goroutine communication, health check ticker goroutine, stream iterator channel, permission request escalation.

### Context Pruning Strategy

Document the per-phase context injection table from idea.md §3.1:

| Phase | Context injected |
|-------|-----------------|
| Initialize → Discuss | Full session history + MEMORY.md + system prompt |
| Plan | System prompt + goal + Discuss Q&A + MEMORY.md + cwd file schema |
| Execute | System prompt + TASKS.md + PROJECT.md + current task spec only |
| Verify | System prompt + TASKS.md + file contents of all task outputs |
| Ship | System prompt + TASKS.md (final status) + commit log from git |

### Error Handling Strategy

Document: sentinel errors in internal/errors/, provider error normalization table (401, 402, 429, 503, 400), context window protection mechanism.

---

## 23. docs/INTERFACES.md

Mirror ALL Go interface from the internal/ files. Write them as valid Go code blocks.
This document is for LLM context in future OpenCode sessions — it must be complete and accurate.

Include:
- LLMProvider interface + ChatRequest, ToolDefinition, ProviderRegistry
- Tool interface + PermissionRequest, PermissionResponse, Dispatcher
- All types from internal/types/types.go (every struct, every field)
- Config struct from internal/config/types.go
- All constants from internal/types/constants.go
- All sentinel errors from internal/errors/errors.go

Each interface block should have a brief comment explaining its purpose and which internal/ file it mirrors.

---

## 24. docs/TYPES.md

Document:
- All environment variables with descriptions and resolution order
- All constants with their values and purposes
- All sentinel errors with when they are returned
- All enum types (RiskLevel, WorkflowPhase, TaskStatus) with their valid values

---

## 25. walkthrough_0.md (project root)

Generate this LAST, after ALL other files are created and verified.
Use EXACTLY this structure with checkboxes:

```markdown
# Walkthrough 0 — Foundation & Documentation

## Completed Tasks

### P0.1 — Go Module & Project Structure
- [ ] go.mod created with module path github.com/eshanized/M31A and Go 1.22
- [ ] cmd/m31a/main.go prints version and initializes logger
- [ ] Makefile with build/test/lint/vet/clean/release targets
- [ ] .gitignore covers binaries, caches, session data, editor files
- [ ] LICENSE is unmodified MIT text (note: created separately, verify exists)
- [ ] README.md stub created with project overview

### P0.2 — Core Interfaces & Shared Types
- [ ] internal/errors/errors.go defines all sentinel errors (list each one)
- [ ] internal/types/types.go defines all core types (Message, Task, Tool, etc.)
- [ ] internal/types/constants.go defines all constants (cache TTL, thresholds, etc.)
- [ ] internal/provider/interface.go defines LLMProvider + ChatRequest + ProviderRegistry
- [ ] internal/tools/interface.go defines Dispatcher + Permission types
- [ ] internal/config/types.go defines full Config struct matching config.toml spec
- [ ] All types compile without errors
- [ ] Zero implementations — interfaces and types only

### P0.3 — CI/CD Pipeline
- [ ] .github/workflows/ci.yml with lint, test, build matrix, and release job
- [ ] .goreleaser.yaml configured for linux/darwin/windows × amd64/arm64
- [ ] .golangci.yml with govet, staticcheck, errcheck, ineffassign, unused, gosimple
- [ ] Build matrix covers 5 platform/arch combos (excluding windows/arm64)
- [ ] Release job is tag-triggered only (refs/tags/v*)

### P0.4 — Logging & Observability
- [ ] internal/log/log.go uses log/slog, writes to ~/.m31a/m31a.log
- [ ] Log rotation: 7-day retention, daily file rename on new day
- [ ] Configurable via M31A_LOG_FORMAT (json|text) and M31A_LOG_LEVEL (info|debug)
- [ ] main.go initializes logger before any other code, with defer cleanup
- [ ] No telemetry, no analytics, no external HTTP calls in logger

### P0.5 — Documentation
- [ ] AGENTS.md created with all required sections
- [ ] opencode.json created with instruction file chain
- [ ] .planning/PROJECT.md, REQUIREMENTS.md, STATE.md, CONTEXT.md created
- [ ] docs/ARCHITECTURE.md covers dependency graph, data flows, threading, context pruning
- [ ] docs/INTERFACES.md mirrors all Go interfaces from internal/ files
- [ ] docs/TYPES.md documents env vars, constants, errors, enums

## Build Verification
- [ ] `go mod tidy` passes
- [ ] `go build ./...` passes (output below)
- [ ] `go vet ./...` passes with zero warnings (output below)
- [ ] `go test -race ./...` passes (output below)
- [ ] `make build` produces static binary (output below)

### go mod tidy
```
[paste actual output]
```

### go build ./...
```
[paste actual output]
```

### go vet ./...
```
[paste actual output]
```

### go test -race ./...
```
[paste actual output]
```

### make build
```
[paste actual output]
```

## Deviations from Spec
[List any deviation from ROADMAP.md Phase 0 tasks, or write "None"]

## Open Questions / Blockers
[List anything Phase 1 needs to know, or write "None"]

## Deliverable Summary
[Write 3-5 sentences summarizing what was created and confirming all Phase 0 deliverables are met. Reference the roadmap Phase 0 deliverables explicitly.]
```

Mark each checkbox as `[x]` after verifying it. Be honest — if something is incomplete, leave it unchecked.

---

# EXECUTION ORDER

Execute tasks in this strict order:

1.  Create directory skeleton with .gitkeep files
2.  Create go.mod (minimal — no dependencies yet)
3.  Create internal/errors/errors.go
4.  Create internal/types/constants.go
5.  Create internal/types/types.go
6.  Create internal/config/types.go
7.  Create internal/provider/interface.go
8.  Create internal/tools/interface.go
9.  Create internal/log/log.go (full implementation)
10. Create cmd/m31a/main.go (imports and uses internal/log)
11. Run: go mod tidy — must resolve all imports cleanly
12. Create Makefile, .gitignore, LICENSE (MIT full text), README.md stub
13. Create .github/workflows/ci.yml, .goreleaser.yaml, .golangci.yml
14. Run: CGO_ENABLED=0 go build -o m31a ./cmd/m31a — MUST succeed before continuing
15. Run: go vet ./... — MUST pass with zero warnings
16. Create AGENTS.md
17. Create opencode.json
18. Create .planning/ artifacts (PROJECT.md, REQUIREMENTS.md, STATE.md, CONTEXT.md)
19. Create docs/ARCHITECTURE.md
20. Create docs/INTERFACES.md
21. Create docs/TYPES.md
22. Run: go test -race ./... — MUST pass (expected: no test files, that is OK)
23. Create walkthrough_0.md with ALL actual command output pasted in

---

# HARD CONSTRAINTS

- The binary MUST compile: CGO_ENABLED=0 go build -o m31a ./cmd/m31a
- go vet ./... MUST produce zero warnings
- go mod tidy MUST resolve cleanly (no missing or unused imports)
- You MUST NOT write any implementation code (no provider clients, no TUI, no tools, no task runner)
- You MUST NOT invent package names not listed in the DELIVERABLES
- All Go files in internal/ MUST contain valid, compilable Go code
- The internal/log/log.go MUST be a FULL implementation (not a stub)
- walkthrough_0.md MUST contain actual command output, not placeholder text
- Every file in DELIVERABLES must be created. Missing files = incomplete phase.
- .planning/REQUIREMENTS.md MUST contain all 26 + 6 acceptance criteria fully expanded — NO shortcuts like "[continue for all 26]"

---

# WHAT SUCCESS LOOKS LIKE

When you are done, the developer can:

1. `cd` into the project and run `make build` — binary compiles and prints version
2. Open OpenCode in the project directory — AGENTS.md loads automatically
3. Read internal/provider/interface.go and know exactly what Phase 1 must implement
4. Read internal/types/types.go and have all shared types ready to use
5. Read .planning/STATE.md and know the project is at Phase 0 complete
6. Read walkthrough_0.md and see actual go build + go vet + go test output
7. Read docs/INTERFACES.md in any future OpenCode session for complete interface context