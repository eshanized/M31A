# I Built an AI Coding Agent That Actually Ships Code (Not Just Suggestions)

## Most AI assistants write code and walk away. M31A stays through testing, verification, and the git commit. Here's how I built it.

---

There's a moment every developer knows. You ask an AI assistant to write a function. It produces something that looks right. You paste it in, run the tests, and watch half of them fail. Then you spend the next twenty minutes debugging code you didn't write and don't fully understand.

The AI "helped." But it didn't finish the job.

I kept hitting this wall until I decided to build something different. Not a better autocomplete, but an **autonomous agent** that owns the entire workflow from the moment you describe what you want to the moment verified changes land in your git tree.

I call it **M31A** (M31 Autonomous). It's written in Go, runs in your terminal, and it's completely open source.

Here's what I learned building it.

---

## The Idea: What If the AI Owned the Whole Loop?

Every AI coding tool I've used follows the same pattern: generate code, hand it back to you, done. You're still responsible for integration, testing, verification, and committing.

That's like hiring a chef who cooks the meal but leaves it on the counter and walks out.

I wanted something that would:

1. Ask me clarifying questions before writing a single line
2. Build a structured plan with task dependencies
3. Execute the plan, file by file
4. Run the tests
5. Fix what's broken
6. Commit the result

Six phases. **Initialize → Discuss → Plan → Execute → Verify → Ship.** Every run ends with a verified commit or a rollback. No half-finished suggestions floating in your clipboard.

---

## Why Go, Why Terminal, Why Static Binary

Three decisions that shaped everything else.

**Go** because I wanted a single binary that compiles to every platform without a runtime dependency. Python would have required users to manage environments. Node would have required npm. Go gives you a 15MB executable that just runs.

**Terminal** because I spend my life in the terminal. Every Electron-based coding tool feels like context-switching to a different universe. I wanted the AI to live where I work.

**Static binary with CGO_ENABLED=0** because I was tired of tools that required Docker, or a specific glibc version, or a brew tap that breaks on ARM. M31A compiles to linux/darwin/windows × amd64/arm64 with zero C dependencies. Download it. Run it. That's it.

```makefile
build:
    CGO_ENABLED=0 go build -ldflags "-s -w \
        -X main.Version=$(VERSION) \
        -X main.Commit=$(COMMIT)" \
        -o m31a ./cmd/m31a
```

Fifteen megabytes. No installer. No daemon. No telemetry.

---

## The Architecture: Six Layers, Clean Boundaries

I spent more time on the architecture than on any single feature. Here's what emerged:

```
  TUI Layer (Bubble Tea) — 29 screens, Elm architecture
           ↓
  Workflow Engine — six-phase orchestration
           ↓
  ┌────────┼────────┐
  ↓        ↓        ↓
Providers  Tools   Packages
(LLM)     (Bash,   (session, ledger,
           Read,    rollback, bisect,
           Write)   taskrunner)
           ↓
  Infrastructure — git, config, tokens, logging
```

The rule is simple: **each layer only knows about the layer below it.** The TUI doesn't import LLM APIs. The workflow engine doesn't import terminal rendering code. The tools don't know what phase they're running in.

This made testing dramatically easier. I could test the workflow engine with a mock provider. I could test tools with a mock dispatcher. I could test the TUI with a mock model. No integration test spaghetti.

---

## The Workflow Engine: Where the Magic Happens

The engine lives in `internal/workflow/engine.go` and it's the beating heart of the project. Let me walk through each phase.

### Phase 1: Initialize

Before any code gets written, the agent needs to understand what it's working with. It detects your project type, framework, and language. It initializes a git repo if one doesn't exist. It creates a `.m31a/` planning directory.

This sounds trivial, but it's the foundation. Without knowing your project structure, the agent can't make intelligent decisions later.

### Phase 2: Discuss

This is the phase most tools skip. The agent asks clarifying questions via LLM streaming:

- What's the scope?
- Are there backward compatibility concerns?
- What edge cases should we handle?
- Are there existing patterns to follow?

I embedded prompt templates using Go's `embed.FS` — eleven markdown files that guide the LLM toward asking useful questions instead of generic ones.

### Phase 3: Plan

The agent generates a structured implementation plan. A custom markdown parser extracts tasks, dependencies, file lists, and review notes:

```go
type Task struct {
    ID           int
    Action       string
    Description  string
    Files        []string
    Dependencies []int
}
```

The plan isn't just a list. It's a **directed acyclic graph** with explicit dependencies. Task 3 won't start until Task 1 and Task 2 complete. This matters when you're modifying shared files.

### Phase 4: Execute

The task runner uses **Kahn's algorithm** for topological sorting. It groups tasks by dependency level, then executes them with bounded parallelism (four concurrent tasks by default):

```go
func (r *Runner) Schedule() ([][]int, error) {
    inDegree := make(map[int]int)
    dependents := make(map[int][]int)

    for _, t := range r.tasks {
        for _, dep := range t.Dependencies {
            inDegree[t.ID]++
            dependents[dep] = append(dependents[dep], t.ID)
        }
    }

    var queue []int
    for _, t := range r.tasks {
        if inDegree[t.ID] == 0 {
            queue = append(queue, t.ID)
        }
    }

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

There's also a self-heal loop. If a task fails, the agent retries up to two times before giving up. Most transient LLM failures recover on the second attempt.

### Phase 5: Verify

File existence checks. Syntax validation. Test execution. If anything fails, the agent either self-heals or triggers a rollback.

### Phase 6: Ship

The final commit. The ledger entry. The session archive. This phase is satisfying — you watch the agent create a clean, verified commit with a descriptive message.

---

## The Provider System: What Happens When an LLM Goes Down

This was one of the hardest problems to solve well.

M31A supports OpenRouter and Zen as LLM providers. But providers go down. They rate-limit you. They have intermittent 503s. And when you're in the middle of a twenty-task execution, you can't afford to wait five minutes for a retry.

The solution is **parallel health checks with automatic fallback**:

```go
func FindFallbackProvider(registry *Registry, current string) (string, error) {
    candidates := registry.ListAll()
    ch := make(chan result, len(candidates))

    ctx, cancel := context.WithTimeout(context.Background(), 10*time.Second)
    defer cancel()

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
            return r.name, nil
        }
    }
    return "", ErrProviderUnreachable
}
```

All alternative providers get health-checked simultaneously. The first healthy one wins. Total switchover time: under ten seconds.

I also built **model arbitrage** — the agent classifies task complexity (simple, moderate, complex) using keyword analysis, then recommends the cheapest model that can handle it. A simple file rename doesn't need GPT-4. A complex refactoring probably does.

---

## Five Tools, Not Fifty

I made a deliberate choice to keep the tool surface area small:

| Tool | Risk Level | Purpose |
|------|-----------|---------|
| **Bash** | Dangerous | Shell command execution |
| **FileRead** | Safe | Read files (50MB limit) |
| **FileWrite** | Medium | Atomic file writes |
| **Glob** | Safe | File pattern matching |
| **Grep** | Safe | Content search |

That's it. Five tools.

Every additional tool is another attack surface, another thing to sandbox, another thing to test. Five well-sandboxed tools are easier to secure than twenty half-baked ones.

### Permission Gating

Every tool call goes through a permission system:

```go
func (d *Dispatcher) RequestPermission(ctx context.Context, tool Tool, input ToolInput) error {
    ch := make(chan PermissionResponse)
    d.emitter.Emit(PermissionRequestMsg{...})

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

The TUI shows a modal: **allow (y)**, **allow always (a)**, **deny (n)**, or **exit (e)**. Dangerous tools like Bash always require approval. Safe tools can be auto-approved.

### Security Guards

The Bash tool enforces:
- **Output capping** at 50,000 characters
- **Timeout** at 30 minutes
- **Process lifecycle** with SIGINT/SIGKILL grace period

The WebFetch tool (an additional tool for URL fetching) includes:
- **SSRF protection** with DNS pinning
- **Private IP blocking** (127.0.0.1, 10.x, 192.168.x, 169.254.x)
- **TOCTOU prevention** between DNS resolution and connection

FileWrite uses **atomic writes** — write to a temp file, then `os.Rename()`. If the process crashes mid-write, you don't get a corrupted file.

---

## The Feature I'm Most Proud Of: Cross-Session Learning

Every session writes a record to a markdown-based learning ledger:

```markdown
| Session | Model | Tasks | Failed | Cost | Duration |
|---------|-------|-------|--------|------|----------|
| a1b2c3d4 | claude-3.5-sonnet | 5 | 1 | $0.12 | 8min |
| e5f6g7h8 | gpt-4-turbo | 3 | 0 | $0.08 | 4min |
```

Over time, the agent builds a picture of:
- Which frameworks you work with most
- What types of tasks tend to fail
- How long things typically take
- Which models give the best cost-to-success ratio

This creates a **compound learning effect**. The hundredth session is measurably better than the first because the agent has data to inform its decisions.

```go
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

The ledger is stored as a markdown table — human-readable, git-trackable, easy to inspect.

---

## AutoDream: Solving the Context Window Problem

Long conversations fill up the context window. When you're forty messages into a complex refactoring, you're approaching the limit.

**AutoDream** solves this with automatic context consolidation. At 60% context usage, it kicks in:

1. Identifies "protected" messages (system prompts, recent messages) that should never be compressed
2. Takes the oldest 50% of non-protected messages
3. Summarizes them into a condensed form
4. Replaces the old messages with the summary

```go
func (c *Consolidator) Consolidate() (ConsolidationResult, error) {
    protected := c.protectedIndices()
    candidates := c.candidateIndices(protected)

    midpoint := len(candidates) / 2
    toCompress := candidates[:midpoint]

    summary := c.summarize(toCompress)
    c.messages = c.replaceWithSummary(toCompress, summary)

    return ConsolidationResult{
        MessagesRemoved: len(toCompress),
        TokensSaved:     c.estimateTokensSaved(toCompress, summary),
    }
}
```

The result: you can work on hour-long tasks without ever hitting the context window limit. The agent remembers what matters and compresses what doesn't.

---

## Building a 29-Screen TUI with Bubble Tea

[Bubble Tea](https://github.com/charmbracelet/bubbletea) by Charm is a Go framework for building terminal UIs using the Elm architecture. It was the right choice — declarative, composable, and delightful.

The TUI has 29 screens:

- **REPL** — main chat interface with streaming display
- **Model Selector** — fuzzy search with per-token cost comparison
- **Plan Review** — browse and refine the implementation plan
- **Execute** — live task progress with tool call rendering
- **Settings** — six-tab configuration editor
- **Ledger Browser** — cross-session learning history
- **Rollback** — commit time machine
- **Diff Viewer** — side-by-side change review

And 18 more.

Screen routing uses a simple enum dispatcher:

```go
func (m AppState) Update(msg tea.Msg) (tea.Model, tea.Cmd) {
    switch msg := msg.(type) {
    case SwitchScreenMsg:
        m.screen = msg.Screen
    }

    switch m.screen {
    case ScreenREPL:
        m.repl, cmd = m.repl.Update(msg)
    case ScreenPlan:
        m.plan, cmd = m.plan.Update(msg)
    }
    return m, cmd
}
```

The MsgEmitter pattern decouples the workflow engine from the TUI. The engine emits typed messages (`PhaseTransitionMsg`, `StreamChunkMsg`, `PermissionRequestMsg`), and the TUI routes them to the appropriate screen. The engine has no idea what a "screen" is.

---

## Secure by Default: OS Keychain Integration

API keys are the most sensitive thing in any AI tool. I refused to store them in plaintext config files.

The keychain package (`pkg/keychain/`) uses OS-native backends:

- **Linux**: D-Bus Secret Service with `pass` CLI fallback
- **macOS**: `/usr/bin/security` CLI (Keychain Access)
- **Windows**: Windows Credential Manager

```go
type Keychain interface {
    Get(service string) (string, error)
    Set(service, value string) error
    Delete(service string) error
}
```

Build tags select the platform implementation at compile time. The key resolution order is:

1. Environment variable
2. OS keychain
3. Config file (last resort, with a warning)

---

## Session Persistence: Ctrl+C Doesn't Kill Your Work

Sessions persist to `.m31a/session.json` — workflow state, message history, checkpoints (max 2 for undo).

If you hit Ctrl+C, lose network, or your laptop dies, you can resume:

```bash
$ m31a --resume
```

The session browser shows recent sessions. Pick one. It restores the workflow state and continues from the last checkpoint.

This feature alone has saved me hours.

---

## The Numbers

- **~15,000 lines** of Go across 150+ files
- **29 TUI screens**, 5 themes
- **11 embedded prompt templates**
- **75% test coverage** (90%+ for critical packages)
- **20+ sentinel errors** with user-friendly messages
- **15-20MB** static binary
- **Zero** telemetry, analytics, or phone-home calls

---

## What I'd Do Differently

A few things I'd change if I started over:

**Test the TUI earlier.** TUI coverage is at 38.6%, and it's the hardest part to retroactively test. I should have written TUI tests from day one.

**Use interfaces more aggressively.** The `types.GitClient` interface was added late. Before that, testing git operations required real git repos. The interface made mocking trivial.

**Start with the error model.** I added sentinel errors incrementally. A comprehensive error taxonomy from the start would have saved refactoring time.

---

## What's Next

The V1.1 roadmap:

- **Ghost mode** — headless runs producing structured diffs without the TUI
- **Picture-in-picture** — second agent in a side pane for cross-review
- **Subagents** — delegate sub-tasks to specialized agents
- **Deferred tools** — queue tool calls requiring human approval for batch review

---

## The Bigger Picture

I built M31A because I believe the current generation of AI coding tools optimizes for the wrong thing. They optimize for "impressive demo" — the moment the AI generates code that looks right. But the hard part of software engineering isn't generating code. It's **integration, verification, and shipping.**

M31A optimizes for the moment you type `git log` and see a clean, verified commit that does what you asked for. That moment is less photogenic than a streaming code generation animation. But it's the moment that actually matters.

The project is open source under the MIT license: [github.com/eshanized/M31A](https://github.com/eshanized/M31A)

If you try it and something breaks, open an issue. If it saves you time, drop a star. Both mean the world.

---

*Built with [Bubble Tea](https://github.com/charmbracelet/bubbletea), [Lip Gloss](https://github.com/charmbracelet/lipgloss), [Glamour](https://github.com/charmbracelet/glamour), and [tiktoken-go](https://github.com/pkoukk/tiktoken-go). Thanks to every contributor and bug reporter.*
