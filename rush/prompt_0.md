# ROLE

You are the **Technical Architect and Lead Go Engineer** for M31A — a terminal-based
AI coding assistant written in Go. You are executing Phase 0 of a multi-phase build
plan. Your entire job in this phase is to produce documentation and project scaffolding.
You will write ZERO Go implementation code. You will write ZERO placeholder functions.
You will ONLY create the files listed in the DELIVERABLES section below.

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
2. GSD planning artifacts (PROJECT.md, REQUIREMENTS.md, ROADMAP.md, STATE.md, CONTEXT.md)
3. Go project structure (go.mod, Makefile, .gitignore, directory skeleton)
4. Complete technical interface documentation in docs/ (these ARE the contracts
   that Phase 1 will implement — write them as precise Go-style documentation, not
   as vague descriptions)
5. CI/CD configuration stubs (.github/workflows/)
6. The walkthrough document (walkthrough_0.md)

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

Build: CGO_ENABLED=0 go build -o m31a ./cmd/m31a Test: go test -race -cover ./... Lint: golangci-lint run ./... Vet: go vet ./... Clean: rm -f m31a

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

cmd/m31a/ — binary entry point only, no logic internal/config/ — config parsing, env vars, keychain resolution internal/provider/ — LLMProvider interface + OpenRouter + Zen clients internal/tui/ — Bubble Tea app, all screens internal/workflow/ — six workflow phases internal/tools/ — Bash, FileRead, FileWrite, Glob, Grep pkg/taskrunner/ — dependency graph, topological sort, task lifecycle pkg/arbitrage/ — complexity scoring, model cost comparison pkg/bisect/ — git bisect wrapper pkg/ledger/ — cross-session learning ledger pkg/rollback/ — commit chain browser pkg/autodream/ — context consolidation pkg/session/ — session lifecycle, file persistence pkg/keychain/ — OS-specific keychain (linux/darwin/windows)

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

charmbracelet/bubbletea — TUI framework charmbracelet/lipgloss — terminal styling charmbracelet/bubbles — TUI components (textarea, viewport, spinner, list) charmbracelet/glamour — Markdown rendering BurntSushi/toml — config parsing tiktoken-go — token estimation for GPT/Claude families doublestar — glob pattern matching with ** support creack/pty — PTY for Bash tool on Linux/macOS

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
    "ROADMAP.md"
  ]
}

```

----------

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

----------

## 4. .planning/REQUIREMENTS.md

GSD artifact. List every V1 acceptance criterion from the spec, verbatim, as a numbered checklist. Include all 26 criteria from the original spec plus the 6 differentiator criteria. Format:

```markdown
# M31A — V1 Requirements

## Core Acceptance Criteria
- [ ] 1. Binary builds: CGO_ENABLED=0 go build -o m31a ./cmd/m31a produces static binary
- [ ] 2. Dual-provider discovery: ...
[continue for all 26]

## Differentiator Criteria
- [ ] 21. MIT License: source available, no telemetry, no vendor lock-in
- [ ] 22. Model arbitrage: /optimize analyzes plan and suggests cheaper alternatives
[continue for all 6 differentiators]

```

----------

## 5. .planning/ROADMAP.md

Copy the ROADMAP.md content exactly as-is from the project root ROADMAP.md that already exists. Do not modify it.

----------

## 6. .planning/STATE.md

GSD artifact.

```markdown
# M31A — Current State

## Active Phase
Phase 0 — Foundation & Documentation

## Status
In Progress

## Completed Phases
None

## Next Phase
Phase 1 — Provider Abstraction Layer

## Key Decisions Made
- Sequential task execution in V1 (no concurrency)
- Only OpenRouter and Zen as LLM gateways
- CGO_ENABLED=0 static binary
- AGENTS.md established as project memory for OpenCode

## Blockers
None

```

----------

## 7. .planning/CONTEXT.md

GSD artifact. Captures Phase 0 implementation decisions.

```markdown
# Phase 0 — Context & Decisions

## What Phase 0 Establishes
- AGENTS.md: OpenCode project memory with all architectural constraints
- opencode.json: Instruction file chain for all future sessions
- docs/INTERFACES.md: Canonical interface contracts (Phase 1 implements these)
- docs/TYPES.md: All shared types (Phase 1/5 implement these)
- docs/ARCHITECTURE.md: Package dependency and data flow
- Go module structure: directory skeleton, go.mod, Makefile

## Interface Decisions
See docs/INTERFACES.md for all canonical interface definitions.

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

----------

## 8. go.mod (project root)

```
module github.com/eshanized/M31A

go 1.22

require (
    github.com/charmbracelet/bubbletea v0.27.0
    github.com/charmbracelet/lipgloss v0.13.0
    github.com/charmbracelet/bubbles v0.20.0
    github.com/charmbracelet/glamour v0.8.0
    github.com/BurntSushi/toml v1.4.0
    github.com/pkoukk/tiktoken-go v0.1.7
    github.com/bmatcuk/doublestar/v4 v4.7.1
    github.com/creack/pty v1.1.23
)

```

Then run: go mod tidy

----------

## 9. Makefile (project root)

```makefile
BINARY  := m31a
MODULE  := github.com/eshanized/M31A
VERSION := $(shell git describe --tags --always --dirty 2>/dev/null || echo "dev")
LDFLAGS := -ldflags "-X main.Version=$(VERSION) -s -w"

.PHONY: build test lint vet clean

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

.DEFAULT_GOAL := build

```

----------

## 10. .gitignore (project root)

Standard Go .gitignore plus:

```
m31a
coverage.out
*.test
/dist/
/.goreleaser-dist/

```

----------

## 11. Directory skeleton

Create these empty directories with a .gitkeep file each:

```
cmd/m31a/
internal/config/
internal/provider/
internal/tui/
internal/workflow/
internal/tools/
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

----------

## 12. cmd/m31a/main.go

This is the ONLY Go file written in Phase 0. It is a stub only.

```go
package main

import "fmt"

// Version is set at build time via ldflags.
var Version = "dev"

func main() {
    fmt.Printf("M31A %s — AI coding assistant\n", Version)
    fmt.Println("Run 'make build' to build. Implementation coming in Phase 1+.")
}

```

----------

## 13. .github/workflows/ci.yml

```yaml
name: CI
on:
  push:
    branches: [main, dev]
  pull_request:
    branches: [main]

jobs:
  build:
    strategy:
      matrix:
        os: [ubuntu-latest, macos-latest, windows-latest]
        go: ["1.22"]
    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-go@v5
        with:
          go-version: ${{ matrix.go }}
      - name: Build
        run: CGO_ENABLED=0 go build -o m31a ./cmd/m31a
        env:
          CGO_ENABLED: "0"
      - name: Test
        run: go test -race -cover ./...
      - name: Vet
        run: go vet ./...

```

----------

## 14. docs/ARCHITECTURE.md

Write a comprehensive architecture document that covers:

### Package Dependency Graph

Write the full dependency graph showing which packages import which, as ASCII art. No circular dependencies allowed. The graph must match the package layout in AGENTS.md exactly.

### Data Flow: Provider Layer

Document exactly how a streaming ChatCompletion request flows from user input through the TUI → provider → SSE parser → stream iterator → message renderer.

### Data Flow: Workflow Engine

Document how the six phases (Initialize → Discuss → Plan → Execute → Verify → Ship) transition, what context each phase receives, and what state files each phase reads/writes.

### State Persistence Model

Document the session directory structure:

```
~/.m31a/sessions/<session-id>/
├── session.json
├── messages.json
├── MEMORY.md
└── planning/
    ├── PROJECT.md
    ├── TASKS.md
    ├── STATE.md
    └── config.json

```

### Threading Model

Document the single-threaded Bubble Tea constraint. How background goroutines communicate via tea.Cmd/tea.Msg. How health checks run without blocking the TUI.

### Context Pruning Strategy

Document the per-phase context injection table (which state files each phase receives, what history is discarded).

----------

## 15. docs/INTERFACES.md

This is the most critical document. It defines every Go interface that Phase 1, 2, 3, 4, 5 will implement. Write them as valid Go code blocks inside the Markdown. Every field must have a comment explaining it.

### Required interfaces and types (write ALL of these):

**LLMProvider interface** (internal/provider/interface.go):

```go
type LLMProvider interface {
    Name() string
    FetchModels() ([]ModelInfo, error)
    ChatCompletionStream(ctx context.Context, req ChatRequest) (*StreamIterator, error)
    EstimateCost(usage Usage) float64
    HealthCheck(ctx context.Context) HealthStatus
    GetModel(id string) (*ModelInfo, error)
}

```

**ModelInfo struct** with ALL fields: ID, Provider, Name, Description, ContextLength, Pricing (struct with InputPerMToken, OutputPerMToken float64), Architecture (struct with TokenizerFamily string), TopProvider string, Capabilities (CapFlags struct with Tools bool, Reasoning bool).

**ChatRequest struct** with ALL fields: Model string, Messages []Message, MaxTokens int, Stream bool, Tools []ToolDefinition, ReasoningEnabled bool.

**StreamChunk struct**: Type string ("content"|"thinking"|"done"), Delta string, ThinkingDuration int64.

**StreamIterator struct** with Next() (*StreamChunk, error) method.

**HealthStatus struct**: Status string ("live"|"slow"|"offline"), LatencyMs int64, Error error.

**Usage struct**: PromptTokens int, CompletionTokens int, TotalTokens int.

**Tool interface** (internal/tools/interface.go):

```go
type Tool interface {
    Name() string
    Description() string
    RiskLevel() RiskLevel
    Execute(ctx context.Context, input ToolInput) (ToolResult, error)
}

```

**RiskLevel type**: const Safe, Destructive, Dangerous RiskLevel.

**ToolCall struct**: ID string, Name string, Input json.RawMessage.

**ToolResult struct**: ToolCallID string, Output string, Error string, DurationMs int64, Truncated bool.

**Message struct** (internal/session/types.go):

```go
type Message struct {
    Role      string           // "user" | "assistant" | "tool"
    Content   string           // Final assembled text
    Segments  []MessageSegment // Ordered thinking + content segments
    ToolCalls []ToolCall
    Usage     *Usage           // Only set on assistant messages
    CreatedAt time.Time
}

```

**MessageSegment struct**: Type string ("content"|"thinking"|"memory"), Content string, DurationMs int64, Visible bool.

**WorkflowPhase type** with constants: PhaseIdle, PhaseInitialize, PhaseDiscuss, PhasePlan, PhaseExecute, PhaseVerify, PhaseShip.

**Task struct**: ID int, Description string, Action string ("NEW"|"EDIT"|"DELETE"), Dependencies []int, Files []string, AcceptanceCriteria []string, Status TaskStatus, HealsAttempted int, PredictedFiles []FilePrediction, CommitHash string.

**TaskStatus type** with constants: StatusPending, StatusRunning, StatusDone, StatusFailed, StatusSkipped, StatusUnrecoverable.

**FilePrediction struct**: Path string, Action string ("create"|"modify"|"delete"), EstimatedLines int.

**ProjectState struct**: Goal string, ProjectType string, Framework string, Answers map[string]string, CreatedAt time.Time.

**Session struct**: ID string, Model string, Provider string, StartedAt time.Time, MessageCount int, WorkflowPhase WorkflowPhase, Project *ProjectState.

**Config struct** (internal/config/types.go): Write the full config struct matching the config.toml schema from the spec: Provider section, Model section, UI section, Permissions section, Features section, Ledger section.

----------

## 16. docs/TYPES.md

Document all error sentinel values that will be used across packages:

```go
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
)

```

Document all environment variables:

-   M31A_OPENROUTER_API_KEY
-   M31A_ZEN_API_KEY
-   M31A_CONFIG
-   M31A_THEME
-   M31A_DEFAULT_MODEL
-   M31A_PROVIDER

Document all constants:

-   ModelCacheTTL = 5 * time.Minute
-   HealthCheckInterval = 60 * time.Second
-   MaxFileSize = 5 * 1024 * 1024 (5MB)
-   MaxToolOutputChars = 10_000
-   MaxHealAttempts = 2
-   MaxPlanRetries = 3
-   SessionIDLength = 8
-   AutoDreamThreshold = 0.60
-   ContextWarningThreshold = 0.80

----------

## 17. walkthrough_0.md (project root)

Generate this LAST, after all other files are created. Use EXACTLY this template:

```markdown
## Walkthrough 0 — Foundation & Documentation

### What Was Built
[List every file created with a one-line description]

### Contracts Established
[Paste the full content of docs/INTERFACES.md here verbatim]

### Deviations from Spec
[List any deviation from ROADMAP.md Phase 0 tasks, or write "None"]

### Test Results
[Run: go test ./... and paste the output. Expected: "? github.com/eshanized/M31A/cmd/m31a [no test files]"]

### Build Verification
[Run: CGO_ENABLED=0 go build -o m31a ./cmd/m31a && echo "BUILD OK" and paste output]

### Open Questions / Blockers
[List anything Phase 1 needs to know, or write "None"]

```

----------

# EXECUTION ORDER

Execute tasks in this strict order:

1.  Create directory skeleton with .gitkeep files
2.  Create go.mod
3.  Run: go mod tidy
4.  Create Makefile, .gitignore, .github/workflows/ci.yml
5.  Create cmd/m31a/main.go (stub only)
6.  Run: CGO_ENABLED=0 go build -o m31a ./cmd/m31a — must succeed before continuing
7.  Create AGENTS.md
8.  Create opencode.json
9.  Create .planning/ artifacts (PROJECT.md, REQUIREMENTS.md, ROADMAP.md, STATE.md, CONTEXT.md)
10.  Create docs/ARCHITECTURE.md
11.  Create docs/INTERFACES.md
12.  Create docs/TYPES.md
13.  Run: go vet ./... — must pass
14.  Run: go test -race ./... — must pass (expected: no test files, that is OK)
15.  Create walkthrough_0.md with actual command output pasted in

----------

# HARD CONSTRAINTS

-   The binary MUST compile: CGO_ENABLED=0 go build -o m31a ./cmd/m31a
-   go vet ./... MUST produce zero warnings
-   You MUST NOT write any implementation code (no provider clients, no TUI, no tools)
-   You MUST NOT invent package names not listed in the DELIVERABLES
-   The INTERFACES.md MUST contain valid Go syntax in all code blocks
-   walkthrough_0.md MUST contain actual command output, not placeholder text
-   Every file in DELIVERABLES must be created. Missing files = incomplete phase.

----------

# WHAT SUCCESS LOOKS LIKE

When you are done, the developer can:

1.  `cd` into the project and run `make build` — binary compiles
2.  Open OpenCode in the project directory — AGENTS.md loads automatically
3.  Read docs/INTERFACES.md and know exactly what Phase 1 must implement
4.  Read .planning/STATE.md and know the project is at Phase 0 complete
5.  Read walkthrough_0.md and see actual go build + go test output