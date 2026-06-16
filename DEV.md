---
title: "Building M31A: A Terminal-Native AI Coding Agent That Ships, Not Just Suggests"
description: "Deep dive into the architecture and design of M31A, a Go-based AI coding agent that owns the entire workflow from planning to git commit"
tags: [go, ai, terminal, architecture, devtools]
cover_image: ""
published: false
---

# Building M31A: A Terminal-Native AI Coding Agent That Ships, Not Just Suggests

Most AI coding assistants are glorified autocomplete on steroids. They suggest code, maybe write a function or two, but leave you holding the bag when it comes to testing, verification, and actually shipping the changes. 

**M31A (M31 Autonomous)** takes a different approach. It's a terminal-based AI coding agent written in Go that owns a **six-phase workflow end-to-end**: Initialize → Discuss → Plan → Execute → Verify → Ship. Every run ends with a verified git commit and a learning ledger entry. One static binary, zero telemetry, any POSIX shell.

In this post, I'll walk you through the architecture, design decisions, and technical highlights of this open-source project.

## The Problem: AI Assistants That Don't Finish the Job

Here's the typical workflow with most AI coding tools:

1. Ask the AI to write some code
2. Copy-paste the suggestion into your editor
3. Run tests manually
4. Debug the inevitable issues
5. Repeat until it works
6. Commit the changes yourself

The AI "helped" with step 1, but you're still doing 80% of the work. And if something breaks three commits later? Good luck figuring out what the AI actually changed.

M31A flips this model. Instead of being a suggestion engine, it's an **autonomous agent** that:

- Asks clarifying questions before planning
- Generates a structured implementation plan
- Executes tasks with proper dependency resolution
- Runs verification (tests, syntax checks)
- Commits verified changes to git
- Records what it learned for future sessions

## Architecture at a Glance

M31A is built with a clean six-layer architecture:

```
┌─────────────────────────────────────────────────────────────┐
│  TUI Layer (Bubble Tea)                                     │
│  29 screens, keyboard/mouse handling, streaming display     │
└─────────────────────────────────────────────────────────────┘
                           ↓
┌─────────────────────────────────────────────────────────────┐
│  Workflow Engine                                            │
│  Six-phase orchestration, LLM streaming, plan parsing       │
└─────────────────────────────────────────────────────────────┘
                           ↓
         ┌─────────────────┼─────────────────┐
         ↓                 ↓                 ↓
┌──────────────┐  ┌──────────────┐  ┌────────────────────┐
│  Providers   │  │  Tools       │  │  Domain Packages   │
│  OpenRouter  │  │  Bash        │  │  session, ledger   │
│  Zen         │  │  FileRead    │  │  rollback, bisect  │
│  Fallback    │  │  FileWrite   │  │  taskrunner        │
│              │  │  Glob, Grep  │  │  keychain          │
└──────────────┘  └──────────────┘  └────────────────────┘
         ↓                 ↓                 ↓
┌─────────────────────────────────────────────────────────────┐
│  Infrastructure Layer                                       │
│  git, config, tokens, codeintel, fileutil, logging          │
└─────────────────────────────────────────────────────────────┘
```

The key insight? **Separation of concerns at every level.** The TUI doesn't know about LLM APIs. The workflow engine doesn't know about terminal rendering. The tools don't know about workflow phases.

## The Six-Phase Workflow Engine

The heart of M31A is the workflow engine, implemented in `internal/workflow/engine.go`. Let's break down each phase:

### Phase 1: Initialize

The agent detects your project type (Go, Python, Node, etc.), initializes git if needed, and creates a `.m31a/` planning directory with:

- `PROJECT.md` — project metadata
- `STATE.md` — current workflow state
- `TASKS.md` — task list (populated later)

```go
// From internal/workflow/initialize.go
func (e *Engine) runInitialize(ctx context.Context) error {
    // Detect project type, framework, language
    project := e.detectProject()
    
    // Initialize git repo if needed
    if !e.git.IsRepository() {
        e.git.Init()
    }
    
    // Create planning directory
    os.MkdirAll(e.planningDir, 0755)
    
    // Write PROJECT.md, STATE.md
    e.writeProjectState(project)
}
```

### Phase 2: Discuss

Before jumping into code, the agent asks clarifying questions via LLM streaming. This prevents the classic "I built exactly what you asked for, but not what you wanted" problem.

The discuss phase uses embedded prompt templates (loaded via `//go:embed prompts/*.md`) to guide the LLM toward asking useful questions about scope, constraints, and edge cases.

### Phase 3: Plan

The agent generates a structured implementation plan in markdown format. A custom parser (`internal/workflow/plan_parser.go`) extracts:

- Task titles and descriptions
- Dependencies between tasks
- Files that will be modified
- Review notes and questions

```go
// From internal/workflow/plan_parser.go
type Plan struct {
    Title      string
    Tasks      []Task
    Questions  []string
    Notes      string
}

type Task struct {
    ID           int
    Action       string
    Description  string
    Files        []string
    Dependencies []int
}
```

The plan parser supports refinement with retry logic (max 3 retries, max 5 refinements) and classifies prompt complexity: `trivial → simple → moderate → complex`.

### Phase 4: Execute

This is where the rubber meets the road. The task runner (`pkg/taskrunner/runner.go`) uses **Kahn's algorithm** for topological sorting to determine execution order:

```go
// From pkg/taskrunner/runner.go
func (r *Runner) Schedule() ([][]int, error) {
    // Build adjacency list and in-degree count
    inDegree := make(map[int]int)
    dependents := make(map[int][]int)
    
    for _, t := range r.tasks {
        for _, dep := range t.Dependencies {
            inDegree[t.ID]++
            dependents[dep] = append(dependents[dep], t.ID)
        }
    }
    
    // Find all tasks with no dependencies
    var queue []int
    for _, t := range r.tasks {
        if inDegree[t.ID] == 0 {
            queue = append(queue, t.ID)
        }
    }
    
    // Process tasks in topological order
    var groups [][]int
    for len(queue) > 0 {
        groups = append(groups, queue)
        var next []int
        for _, id := range queue {
            for _, dep := range dependents[id] {
                inDegree[dep]--
                if inDegree[dep] == 0 {
                    next = append(next, dep)
                }
            }
        }
        queue = next
    }
    
    return groups, nil
}
```

Tasks within a group can run with bounded parallelism (default: 4 concurrent tasks via semaphore). The executor includes a **self-heal loop** that retries recoverable failures up to 2 times.

### Phase 5: Verify

The agent runs verification checks:

- File existence validation
- Syntax checking (language-specific)
- Test execution
- Smart file truncation for LLM context

If verification fails, the agent can rollback the commit chain using git-bisect integration.

### Phase 6: Ship

The final phase:

1. Creates a git commit with all verified changes
2. Writes a ledger entry (cross-session learning record)
3. Archives the session
4. Generates a demonstration summary

## Provider System: Multi-LLM with Automatic Fallback

M31A supports two LLM providers out of the box:

- **OpenRouter** — primary gateway with access to Claude, GPT-4, etc.
- **Zen** — secondary provider (OpenCode Zen)

The provider layer (`internal/provider/`) includes some clever engineering:

### Automatic Fallback

When a provider degrades (429 rate limit, 503 service unavailable), M31A automatically switches to a healthy provider. The fallback logic uses **parallel health checks** to minimize latency:

```go
// From internal/provider/fallback.go
func FindFallbackProvider(registry *Registry, current string) (string, *FallbackEvent, error) {
    // Collect candidate providers
    candidates := registry.ListAll()
    
    // Parallel health checks (10s timeout)
    ctx, cancel := context.WithTimeout(context.Background(), 10*time.Second)
    defer cancel()
    
    ch := make(chan result, len(candidates))
    for _, c := range candidates {
        go func(c candidate) {
            status := c.provider.HealthCheck(ctx)
            ch <- result{name: c.name, status: status}
        }(c)
    }
    
    // Return first healthy provider in priority order
    for i := 0; i < len(candidates); i++ {
        r := <-ch
        if r.status.Status == "live" || r.status.Status == "slow" {
            registry.TrySetActive(r.name)
            return r.name, &FallbackEvent{...}, nil
        }
    }
}
```

### Model Arbitrage

M31A includes a **model arbitrage system** (`pkg/arbitrage/`) that automatically switches to the cheapest model that meets the task's capability threshold:

```go
// From pkg/arbitrage/arbitrage.go
func (s *Scorer) Score(task Task) (ComplexityLevel, int) {
    level := classifyText(task.Action, task.Description)
    
    // Boost complexity when task touches many files
    if len(task.Files) > 3 {
        level = boostLevel(level, 1)
    }
    
    // Boost when task has many dependencies
    if len(task.Dependencies) > 3 {
        level = boostLevel(level, 1)
    }
    
    input, output := s.EstimateTokens(level, task)
    return level, input + output
}
```

The scorer uses keyword analysis to classify tasks as `simple`, `moderate`, or `complex`, then recommends the cheapest model that can handle that complexity level.

## Tool System: Deliberately Small, Aggressively Sandboxed

M31A ships with 5 core tools:

1. **Bash** — shell command execution
2. **FileRead** — read files with size limits (50MB max)
3. **FileWrite** — atomic file writes (temp + rename)
4. **Glob** — file pattern matching (doublestar, 1000 result limit)
5. **Grep** — content search (ripgrep when available, pure-Go fallback)

The tool surface area is intentionally small. Each tool is aggressively sandboxed with:

### Permission Gating

Every tool call is gated by a permission modal with configurable timeout (default 300s):

```go
// From internal/tools/permissions.go
type PermissionMode string

const (
    ModeAsk         PermissionMode = "ask"
    ModeAllowAll    PermissionMode = "allow_all"
    ModeDenyAll     PermissionMode = "deny_all"
)

func (d *Dispatcher) RequestPermission(ctx context.Context, tool Tool, input ToolInput) error {
    if d.mode == ModeAllowAll {
        return nil
    }
    
    // Send permission request to TUI
    ch := make(chan PermissionResponse)
    d.emitter.Emit(PermissionRequestMsg{...})
    
    // Wait for user response with timeout
    select {
    case resp := <-ch:
        if !resp.Approved {
            return ErrPermissionDenied
        }
    case <-time.After(d.timeout):
        return ErrPermissionTimeout
    }
}
```

### Security Guards

- **Path traversal guards**: symlink resolution + workDir prefix check
- **Output capping**: MaxToolOutputChars (10,000) / BashOutputLimit (50,000)
- **SSRF protection**: DNS pinning, TOCTOU prevention, redirect checking (WebFetch)
- **Process lifecycle**: SIGINT/SIGKILL grace period, pipe cleanup

### Risk Levels

Each tool declares its risk level:

```go
type RiskLevel string

const (
    RiskSafe        RiskLevel = "safe"
    RiskMedium      RiskLevel = "medium"
    RiskDangerous   RiskLevel = "dangerous"
    RiskDestructive RiskLevel = "destructive"
)
```

Bash is `dangerous`, FileWrite is `medium`, FileRead is `safe`. The permission system uses these levels to determine whether to prompt the user.

## Cross-Session Learning Ledger

One of M31A's most interesting features is the **cross-session learning ledger** (`pkg/ledger/`). Every session writes a structured record to a markdown file:

```markdown
| Session | Model | Tasks | Failed | Cost | Duration | Framework |
|---------|-------|-------|--------|------|----------|-----------|
| a1b2c3d4 | claude-3.5-sonnet | 5 | 1 | $0.12 | 8min | react |
| e5f6g7h8 | gpt-4-turbo | 3 | 0 | $0.08 | 4min | go |
```

The ledger tracks:

- Session ID and timestamp
- Model and provider used
- Task count and failures
- Cost estimate
- Duration
- Project type and framework
- Goal keywords (with stop-word filtering)

Over time, the agent can query the ledger to learn from past sessions:

```go
// From pkg/ledger/ledger.go
type LedgerStats struct {
    TotalSessions      int
    AvgTaskCount       float64
    AvgCost            float64
    AvgDurationMinutes float64
    TotalFailedTasks   int
    TopFailures        []string
    TopFrameworks       []string
    ByProjectType      map[string]int
}
```

This creates a feedback loop where the agent gets sharper over time, learning which frameworks are common, what types of tasks fail, and how long things typically take.

## AutoDream: Context Window Consolidation

Long conversations blow the context window. M31A solves this with **AutoDream** (`pkg/autodream/`), an automatic context consolidation system:

```go
// From pkg/autodream/autodream.go
func (c *Consolidator) Consolidate() (ConsolidationResult, error) {
    // Protect system prompts and recent messages
    protected := c.protectedIndices()
    candidates := c.candidateIndices(protected)
    
    // Summarize oldest 50% of non-protected messages
    midpoint := len(candidates) / 2
    toCompress := candidates[:midpoint]
    
    // Build summary prompt
    summary := c.summarize(toCompress)
    
    // Replace old messages with summary
    c.messages = c.replaceWithSummary(toCompress, summary)
    
    return ConsolidationResult{
        MessagesRemoved: len(toCompress),
        TokensSaved:     c.estimateTokensSaved(toCompress, summary),
    }
}
```

AutoDream triggers at 60% context usage by default. It uses role-sampled summarization (system prompts are never compressed) and preserves recent messages for continuity.

## TUI: 29 Screens Built with Bubble Tea

The terminal UI is built with [Bubble Tea](https://github.com/charmbracelet/bubbletea), following the Elm architecture. Screen routing uses an enum-based dispatcher:

```go
// From internal/tui/app_state.go
type Screen int

const (
    ScreenREPL Screen = iota
    ScreenGoalInput
    ScreenPhaseModelPicker
    ScreenPlan
    ScreenDiscuss
    ScreenExecute
    ScreenVerify
    ScreenShip
    ScreenModelSelector
    ScreenSettings
    // ... 19 more screens
)

func (m AppState) Update(msg tea.Msg) (tea.Model, tea.Cmd) {
    switch msg := msg.(type) {
    case SwitchScreenMsg:
        m.screen = msg.Screen
        return m, nil
    }
    
    // Route to active screen's Update function
    switch m.screen {
    case ScreenREPL:
        m.repl, cmd = m.repl.Update(msg)
    case ScreenPlan:
        m.plan, cmd = m.plan.Update(msg)
    // ...
    }
}
```

The TUI includes some nice touches:

- **Fuzzy model selector** with per-token cost comparison
- **Permission modals** with keyboard shortcuts (y/a/n/e for allow/allow always/deny/exit)
- **Streaming display** for real-time LLM output
- **Dark/light themes** with auto mode
- **Context warning banner** at 80% window usage

## Commit Rollback Chain

When verification fails, M31A can rollback the commit chain using git-bisect integration (`pkg/bisect/`):

```go
// From pkg/rollback/rollback.go
func (r *Rollback) HardReset(commit string) error {
    // Create backup branch before destructive operation
    backupName := fmt.Sprintf("m31a/rollback-backup-%d", time.Now().Unix())
    r.git.CreateBranch(backupName)
    
    // Auto-stash uncommitted changes
    if r.git.HasUncommittedChanges() {
        r.git.Stash()
        defer r.git.StashPop()
    }
    
    // Hard reset to target commit
    return r.git.ResetHard(commit)
}
```

The rollback system maintains a commit chain with soft/hard/safe reset options. Safe reset creates backup branches before any destructive operation.

## OS-Native Secure Key Storage

API keys are stored using OS-native keychain backends (`pkg/keychain/`):

- **Linux**: D-Bus Secret Service + `pass` CLI fallback
- **macOS**: `/usr/bin/security` CLI
- **Windows**: Windows Credential Manager

```go
// From pkg/keychain/keychain.go
type Keychain interface {
    Get(service string) (string, error)
    Set(service, value string) error
    Delete(service string) error
}
```

The keychain abstraction uses build tags to select the platform-specific implementation at compile time. Service names follow the pattern `m31a/openrouter`, `m31a/zen`.

**Key resolution order**:
1. Environment variable: `M31A_OPENROUTER_API_KEY`
2. Standard fallback: `OPENROUTER_API_KEY`
3. OS keychain: `m31a/openrouter`
4. Config file: `provider.openrouter.api_key`

Keys are never written to disk in plaintext when keychain is available.

## Static Binary, Zero Telemetry

M31A is compiled with `CGO_ENABLED=0`, producing a fully static binary with no C dependencies:

```makefile
# From Makefile
build:
    CGO_ENABLED=0 go build -ldflags "-s -w \
        -X main.Version=$(VERSION) \
        -X main.Commit=$(COMMIT) \
        -X main.Date=$(DATE)" \
        -o m31a ./cmd/m31a
```

The binary is typically 15-20MB (stripped with `-s -w` ldflags). Cross-compilation targets include linux/darwin/windows × amd64/arm64.

**Zero telemetry**: no analytics, no crash reporting, no usage pings. Your code never leaves your machine except when sent to the LLM provider for inference.

## Session Persistence and Recovery

Sessions persist to `<workDir>/.m31a/session.json`, including:

- Workflow state (goal, phase, questions)
- Message history (separate `messages.json`)
- Checkpoints (max 2 for undo/rollback)

If you hit `Ctrl+C`, lose network, or your laptop dies, you can resume mid-workflow:

```bash
$ m31a --resume
# Shows session browser with recent sessions
# Restores workflow state and continues from last checkpoint
```

## Testing Strategy

M31A uses Go's standard `testing` package with no external mocking frameworks:

- **Unit tests**: individual functions/methods
- **Integration tests**: real git repos, temp dirs, HTTP test servers
- **Security tests**: SSRF protection, timeout enforcement, path traversal
- **Table-driven tests**: anonymous structs with `t.Parallel()`

Coverage targets:
- Overall: 75% (currently ~74.7%)
- Critical packages: 90% — `pkg/taskrunner` (89.9%), `pkg/bisect` (91.3%), `pkg/rollback` (89.1%)

The test suite includes some interesting patterns:

```go
// Security test for SSRF protection
func TestWebFetch_BlocksPrivateIPs(t *testing.T) {
    tests := []struct {
        url      string
        wantErr  error
    }{
        {"http://127.0.0.1/admin", ErrPrivateIPBlocked},
        {"http://192.168.1.1/config", ErrPrivateIPBlocked},
        {"http://10.0.0.1/secret", ErrPrivateIPBlocked},
        {"http://169.254.169.254/metadata", ErrPrivateIPBlocked}, // AWS metadata
    }
    
    for _, tt := range tests {
        t.Run(tt.url, func(t *testing.T) {
            t.Parallel()
            _, err := WebFetch(tt.url)
            if !errors.Is(err, tt.wantErr) {
                t.Errorf("got %v, want %v", err, tt.wantErr)
            }
        })
    }
}
```

## Getting Started

Installation is a one-liner:

```bash
# macOS (Homebrew)
brew install eshanized/tap/m31a

# Linux / macOS (curl)
curl -fsSL https://raw.githubusercontent.com/eshanized/M31A/main/install.sh | bash

# From source (any OS)
git clone https://github.com/eshanized/M31A.git
cd M31A
CGO_ENABLED=0 go build -o m31a ./cmd/m31a
```

On first launch, M31A prompts for your OpenRouter or Zen API key and stores it in the OS keychain.

**Basic usage**:

```bash
$ m31a
# TUI launches
# Type your goal: "refactor the auth middleware to use JWT with RS256"
# Agent runs through six phases
# Ends with verified git commit
```

**Slash commands**:

```
/help          list all commands
/workflow      kick off the six-phase flow
/model         open the model selector (fuzzy search)
/provider      switch provider
/ledger stats  show your cross-session ledger
/rollback      show the commit chain; --hard to reset
/compress      trigger AutoDream manually
```

## What's Next

M31A is at v1.0.0 with the core feature set complete. The roadmap includes:

- **Ghost mode** — headless runs producing structured diffs
- **Picture-in-picture** — second agent in side pane for cross-review
- **Subagents** — delegate sub-tasks to specialized agents (code, test, doc)
- **Deferred tools** — queue tool calls requiring human approval for batch review

## Lessons Learned

Building M31A taught me a few things:

1. **Workflow ownership matters more than code generation**. The six-phase workflow is more valuable than any single code suggestion.

2. **Small tool surface area is a feature**. Five well-sandboxed tools are easier to secure than twenty half-baked ones.

3. **Learning compounds**. The cross-session ledger creates a feedback loop that makes the agent better over time.

4. **Terminal UIs can be delightful**. Bubble Tea proves that terminal apps don't have to be ugly or hard to use.

5. **Static binaries are liberating**. No runtime dependencies, no Docker required, just download and run.

## Conclusion

M31A is an experiment in what AI coding assistants could be if they owned the entire workflow instead of just the fun part. It's not perfect — the TUI test coverage needs work (38.6%), and there are some known bugs around git status detection — but the architecture is sound and the core workflow is production-ready.

If you're interested in the intersection of AI, developer tools, and terminal UIs, I'd love your feedback. Star the repo, open an issue, or better yet, try it on your codebase and let me know what breaks.

**Links**:
- GitHub: [github.com/eshanized/M31A](https://github.com/eshanized/M31A)
- Documentation: [docs/](https://github.com/eshanized/M31A/tree/main/docs)
- Issues: [github.com/eshanized/M31A/issues](https://github.com/eshanized/M31A/issues)

---

*Thanks to the Bubble Tea, Lip Gloss, and Glamour teams for making terminal UIs enjoyable to build. And thanks to everyone who has tried M31A and reported bugs — your feedback makes it better.*
