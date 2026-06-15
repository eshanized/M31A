# M31 Autonomous: A Terminal-Native AI Coding Agent with Six-Phase Workflow Orchestration

**A Technical Research Paper**

**Author:** Eshan Roy &lt;eshanized@proton.me&gt;
**Date:** June 15, 2026
**Repository:** [github.com/eshanized/M31A](https://github.com/eshanized/M31A)
**Module:** `github.com/eshanized/M31A` (Go 1.25)
**Version:** v1.0.0
**License:** MIT

---

## Abstract

The proliferation of AI-assisted coding tools has produced a landscape dominated by browser-bound assistants and editor plugins. M31 Autonomous takes a fundamentally different approach: a terminal-native agent, written entirely in Go, that owns a six-phase software engineering workflow end-to-end. From initialization through discussion, planning, execution, verification, and shipping, every run concludes with a verified git commit and a cross-session learning record. This paper presents a deep technical analysis of M31 Autonomous's architecture, its core innovations, and the engineering decisions that make it both powerful and safe. We examine the workflow engine, the 29-screen terminal UI, the provider abstraction layer, the security model, and the suite of domain-specific packages that together form what we believe is the most complete terminal-native AI coding agent available today.

---

## 1. Introduction

### 1.1 The Problem Space

Modern software development increasingly relies on AI to assist with code generation, refactoring, and debugging. However, most existing tools operate within one of two constrained paradigms: either they are embedded inside an editor (losing the flexibility of the terminal), or they are simple command-line wrappers around a single LLM call (lacking the structured workflow that real engineering tasks demand).

Consider a typical refactoring task: migrating an authentication middleware from session-based to JWT-based auth. This is not a single-turn interaction. It requires understanding the existing codebase, discussing tradeoffs, creating a plan, executing changes across multiple files, verifying correctness through tests, and committing the result. Each phase demands different capabilities, different levels of autonomy, and different safety guarantees.

M31 Autonomous was designed from the ground up to solve this exact problem. It is not an autocomplete with shell access. It is a structured workflow engine that happens to live in your terminal.

### 1.2 Design Philosophy

M31 Autonomous's design is guided by five principles:

1. **Terminal-native, not terminal-attached.** The terminal is the primary interface, not a fallback. Every interaction is designed for keyboard-driven efficiency.

2. **Own the loop.** The agent orchestrates the entire lifecycle of a task, from understanding intent to shipping verified code.

3. **Safety by default.** Every file operation and shell command requires explicit permission. No silent writes, no surprise commits.

4. **Cross-session intelligence.** Patterns, failures, and recoveries persist across sessions. The agent learns from its own history.

5. **Zero telemetry.** No analytics, no crash reporting, no usage pings. The binary does exactly what the code does and nothing more.

### 1.3 Contributions

This paper makes the following contributions:

- A detailed architectural analysis of a six-phase workflow engine for AI-assisted coding.
- Documentation of a novel context consolidation system (AutoDream) that prevents context window overflow.
- Analysis of a multi-provider LLM abstraction with automatic fallback and cost optimization.
- A comprehensive security model for tool execution in an AI agent context.
- Full documentation of the 47 slash commands and 29-screen TUI system.

---

## 2. System Architecture

### 2.1 High-Level Overview

M31 Autonomous follows a strict six-layer architecture with a clear dependency rule: higher layers may depend on lower layers, but never the reverse. The `pkg/` directory contains public, importable packages, while `internal/` contains private implementation details.

```
┌─────────────────────────────────────────────────────────────────┐
│                       TUI Layer (Bubble Tea)                    │
│              29 screens · Elm architecture · Themes             │
└──────────────────────────────┬──────────────────────────────────┘
                               │
                               ▼
┌─────────────────────────────────────────────────────────────────┐
│                   Workflow Engine (6 phases)                    │
│          Orchestrator · Prompt templates · Plan parser          │
└──────────────────────────────┬──────────────────────────────────┘
                               │
                    ┌──────────┼──────────┐
                    ▼          ▼          ▼
              ┌──────────┐ ┌────────┐ ┌──────────┐
              │ Provider │ │  Tools │ │ Packages │
              │  Layer   │ │  Layer │ │ (pkg/)   │
              └──────────┘ └────────┘ └──────────┘
                               │
                               ▼
┌─────────────────────────────────────────────────────────────────┐
│                   Infrastructure Layer                          │
│         Config · Errors · Tokens · CodeIntel · Git              │
└─────────────────────────────────────────────────────────────────┘
```

The entry point, `cmd/m31a/main.go`, orchestrates a 15-step initialization sequence that wires together all these layers. This sequence is worth examining in detail because it reveals the system's dependency ordering.

### 2.2 Initialization Pipeline

The `run()` function (the entire `main.go` is a single `run()` function with deferred cleanup) executes the following 15-step sequence. Each step is deliberately ordered — dependencies flow downward, and failures at critical points cause an immediate exit with a descriptive error. The `run()` pattern ensures all `defer` statements execute before `os.Exit(0)`, guaranteeing cleanup even on error paths.

---

#### Step 1: Command Registry Construction

```go
cmdRegistry := tui.DefaultCommands()
```

**What it does:** Builds the slash command registry containing all 47 commands. Each command is registered with its name, description, category, and handler function. The registry is used both for the TUI's command processing and for the `--help` flag's usage display.

**Why it's first:** The command registry is needed for the `--help` flag's `printUsage()` function, which must be available before any other initialization. It has no dependencies on other components.

**Failure mode:** None — this is a pure in-memory construction with no I/O.

---

#### Step 2: CLI Flag Parsing

```go
versionFlag := flag.Bool("version", false, "Print version and exit")
helpFlag := flag.Bool("help", false, "Show usage information")
flag.Parse()
```

**What it does:** Parses `--version` and `--help` flags using Go's standard `flag` package. The version string is injected at build time via `var Version = "dev"` — when built with GoReleaser, it receives the git tag. The Go runtime version is normalized to strip build-tag noise (e.g., `"X:nodwarf5"` becomes `"go1.25"`).

**Why it's second:** These are "short-circuit" flags — if the user asks for version or help, the program should exit immediately without loading config, connecting to providers, or initializing the TUI.

**Failure mode:** `flag.Parse()` returns on the first non-flag argument, which is intentional for M31 Autonomous's argument handling.

---

#### Step 3: Environment Loading (.env)

```go
config.LoadDotEnv()
```

**What it does:** Reads a `.env` file from the current working directory, parsing `KEY=VALUE` pairs into the process environment. This happens before the logger is initialized because `os.Setenv` is not goroutine-safe — if the logger were already running background goroutines, calling `os.Setenv` could cause a data race.

**Security checks:**
- Rejects files with group or world-writable permissions (`perm & 0o022 != 0`).
- Skips lines longer than 4096 bytes.
- Does not override already-set environment variables (checked via `os.LookupEnv`).
- Guarded by `sync.Once` — even though `LoadDotEnv()` is called from both `main.go` and `config.Load()`, it only executes once.

**Why it's third:** Environment variables must be loaded before the config system (which reads `M31A_THEME`, `M31A_DEFAULT_MODEL`, etc.) and before the logger (which reads `M31A_LOG_LEVEL`).

**Failure mode:** Non-fatal — if the `.env` file doesn't exist or is malformed, the function silently continues. Only actual parsing errors in well-formed files trigger warnings.

---

#### Step 4: Logger Initialization

```go
logger, cleanup, err := log.NewLogger(Version)
```

**What it does:** Creates a structured `slog` logger with the following features:
- **Output location:** `~/.m31a/m31a.log`
- **Format:** JSON (default) or text, controlled by `M31A_LOG_FORMAT`.
- **Level:** `M31A_LOG_LEVEL` — debug, info (default), warn, error.
- **Daily rotation:** renames `m31a.log` to `m31a.log.YYYY-MM-DD` when last modified before today.
- **7-day retention:** deletes rotated files older than 7 days.
- **Singleton:** `sync.Once` ensures only one logger instance exists.

The `cleanup` function is deferred to flush the log file on exit. If initialization fails, the program falls back to stderr and continues — logging is important but not fatal.

**Why it's fourth:** The logger must be initialized after `.env` loading (which sets `M31A_LOG_LEVEL`) but before everything else (so all subsequent steps can log their progress and errors).

**Failure mode:** Non-fatal — falls back to stderr. Rotation failures are also non-fatal (warns and continues with append-only).

---

#### Step 5: Configuration Resolution

```go
configPath := os.Getenv("M31A_CONFIG")
// ... resolve to ~/.m31a/config.toml if not set
cfg, err := config.Load(configPath)
```

**What it does:** Loads the TOML configuration through the 7-step pipeline documented in Section 9.1:
1. Check `M31A_CONFIG` env var for custom path.
2. Fall back to `~/.m31a/config.toml`.
3. Create the config directory with `0755` permissions if it doesn't exist.
4. Load defaults, then overlay global TOML, env vars, project config, variable substitution, and validation.

If the config file doesn't exist, a default config is used silently. Unknown TOML keys trigger a warning log.

**Why it's fifth:** The config must be loaded before the keychain (which needs to know which providers to resolve), the logger (which needs the log level), and the TUI (which needs theme and UI settings).

**Failure mode:** Fatal — if the config file exists but is malformed or has invalid values, the program exits with a descriptive error including all validation failures.

---

#### Step 6: Keychain Integration

```go
kc, kcErr := keychain.New()
if kc != nil {
    cfg.ResolveAPIKeys(kc)
}
```

**What it does:** Initializes OS-native secret storage:
- **Linux:** D-Bus Secret Service with `pass` CLI fallback.
- **macOS:** `/usr/bin/security` CLI.
- **Windows:** Windows Credential Manager via `advapi32.dll`.

Then resolves API keys through the 4-tier priority chain (env var -> standard env var -> OS keychain -> config file). If the keychain is available and stores a key successfully, the key is cleared from the TOML file on next save.

**Why it's sixth:** The keychain must initialize before the provider registry (step 7), because API keys flow through the keychain. It must come after config loading (step 5), because the config file may contain fallback keys.

**Failure mode:** Non-fatal — if the keychain is unavailable (D-Bus not running, `pass` not installed), the system logs a warning and falls back to config file keys. The program continues without keychain integration.

---

#### Step 7: Provider Registry

```go
registry := provider.NewRegistry()
if cfg.Provider.OpenRouter.APIKey != "" {
    tui.RegisterProvider(registry, cfg, "openrouter", cfg.Provider.OpenRouter.APIKey, Version)
}
if cfg.Provider.Zen.APIKey != "" {
    tui.RegisterProvider(registry, cfg, "zen", cfg.Provider.Zen.APIKey, Version)
}
if cfg.Provider.Default != "" {
    registry.SetActive(cfg.Provider.Default)
}
```

**What it does:** Creates a thread-safe provider registry and registers OpenRouter and Zen providers if their API keys are configured. Each provider is initialized with:
- API key (from the resolution chain).
- Custom base URL (if configured).
- HTTP headers (OpenRouter's `HTTP-Referer` and `X-Title`).
- Shared HTTP transport (created once via `sync.Once`, reused across all providers for connection pool sharing).

Sets the active provider from `cfg.Provider.Default`. Warns if no provider is available — LLM features will be unavailable but the TUI still launches.

**Why it's seventh:** The provider registry depends on the keychain (step 6) for API keys and the config (step 5) for provider settings. It must be initialized before the tool dispatcher (step 10), because tools need to know which provider is active.

**Failure mode:** Non-fatal per provider — if one provider fails to register (invalid key, network error), the other may still succeed. If no providers register, the TUI launches with a warning.

---

#### Step 8: Working Directory

```go
workDir, err := os.Getwd()
```

**What it does:** Resolves the current working directory from `os.Getwd()`. This is the project root — all file operations, session storage, and git operations are relative to this directory.

**Why it's eighth:** The working directory is needed by almost every subsequent component: session manager (project-local storage), tool dispatcher (path traversal guards), git client (repository root), and domain packages.

**Failure mode:** Fatal — if `os.Getwd()` fails, the program cannot determine where it's running and exits immediately.

---

#### Step 9: Session Manager

```go
globalConfigDir := filepath.Dir(configPath)
sessionMgr := session.NewManager(globalConfigDir, workDir, session.ManagerOpts{})
```

**What it does:** Creates a session manager that handles the full lifecycle of agent sessions. Sessions are stored project-local in `<workDir>/.m31a/` as flat JSON and Markdown files. The manager is initialized with:
- `globalConfigDir` — for global data (recent models, ledger).
- `workDir` — for project-local sessions.
- Empty options (defaults: 8-char session IDs from `crypto/rand`, 10 max recent models, 2s session cache TTL).

The manager also ensures `.m31a/` is added to the project's `.gitignore` on first session creation.

**Why it's ninth:** The session manager needs the working directory (step 8) for project-local storage. It's needed by the workflow engine (which persists state) and the TUI (which loads/saves sessions).

**Failure mode:** Non-fatal — if the `.m31a/` directory can't be created, the manager logs a warning. Sessions won't persist, but the application continues.

---

#### Step 10: Tool Dispatcher

```go
backupDir := filepath.Join(workDir, ".m31a", "backups")
dispatcher, err := tools.DefaultDispatcher(workDir, backupDir, backupDir, &cfg.Permissions)
```

**What it does:** Creates the tool execution layer, registering all 15 tools (Bash, FileRead, FileWrite, Edit, Glob, Grep, WebFetch, WebSearch, TodoWrite, AskUserQuestion, FileList, FileDelete, FileMove, CodeMap, Agent). Each tool is wired with:
- Working directory for path resolution.
- Backup directory for file versioning.
- Permission configuration for access control.
- Rate limiter (token bucket: 20 burst, 10 tools/sec sustained).

The dispatcher validates the permission configuration at creation time — invalid rules cause an immediate exit (WP-C04).

**Why it's tenth:** The tool dispatcher needs the working directory (step 8) for path resolution, the backup directory (derived from workDir), and the permission config (step 5). It's needed by the workflow engine (which executes tools) and the TUI (which handles permission requests).

**Failure mode:** Fatal — invalid permission configuration causes an immediate exit with a descriptive error. This is deliberate: misconfigured permissions could allow unintended file operations.

---

#### Step 11: Git Client

```go
gitClient := git.New(workDir)
```

**What it does:** Creates a shell-based git wrapper that executes git commands in the working directory. The wrapper provides both high-level convenience methods (Commit, Log, Diff, Status) and low-level access (Run for arbitrary commands). All git refs are validated to prevent flag injection (`--` prefix), range injection (`..`), and null/newline characters.

**Why it's eleventh:** The git client is a simple wrapper with no dependencies beyond the working directory. It's needed by the rollback package (step 12), the workflow engine (which creates commits), and the TUI (which displays git status).

**Failure mode:** Non-fatal — git operations fail gracefully if the directory isn't a git repository. The workflow engine can initialize one during the Initialize phase.

---

#### Step 12: Domain Packages

```go
ledgerClient := ledger.New(ledgerPath)
rollbackClient := rollback.New(gitClient)
autoDreamClient := autodream.New(nil)
```

**What it does:** Initializes three domain packages:
- **Ledger** — cross-session learning store at `~/.m31a/LEDGER.md`. Loads existing entries, computes statistics, and prepares for appending new entries after each shipped session.
- **Rollback** — git commit chain manager. Wraps the git client to provide soft/hard/safe reset with backup branch creation.
- **AutoDream** — context window consolidator. Initialized with empty messages; the REPL injects messages later. Starts in unpaused state with consolidation statistics reset.

**Why it's twelfth:** These packages depend on the git client (step 11) and the working directory (step 8). They're needed by the workflow engine (which uses all three) and the TUI (which displays ledger, rollback, and compression status).

**Failure mode:** Non-fatal — each package handles missing files gracefully (empty ledger, no commits to rollback, no messages to consolidate).

---

#### Step 13: Theme Selection

```go
themeMode := theme.ModeDark
switch cfg.UI.Theme {
case "light": themeMode = theme.ModeLight
case "auto":  themeMode = theme.ModeAuto
}
```

**What it does:** Maps the configuration string to a `theme.Mode` enum (Dark, Light, Auto). The theme manager is created later by the TUI, but this initial mode is passed to the app constructor. Auto mode uses `lipgloss.HasDarkBackground()` to detect the terminal's background color at runtime.

**Why it's thirteenth:** Theme selection depends on the config (step 5) for the `ui.theme` value. It's a simple mapping with no I/O, so it can happen at any point after config loading.

**Failure mode:** None — defaults to Dark if the config value is unrecognized.

---

#### Step 14: Subagent System

```go
subagentMgr := subagent.NewManager(subagent.Dependencies{...})
dispatcher.Register(tools.NewAgent(subagentMgr, false))
app.SetSubagentManager(subagentMgr)
subagent.Sweep(context.Background(), workDir)
```

**What it does:** Creates the subagent manager and wires it into the tool dispatcher:
1. **Manager creation** — with dependencies: workDir, provider registry, active model info, logger, git worktrees, and a `DispatcherFactory` that creates fresh tool dispatchers per subagent worktree.
2. **Agent tool registration** — registers the Agent tool on the parent dispatcher with `isChild=false` (can spawn background subagents). Children created by the factory get `isChild=true` and cannot spawn grandchildren.
3. **Stale worktree sweep** — runs at startup, prunes git worktree metadata, deletes orphaned `m31a/agent-*` branches from prior crashes.

**Why it's fourteenth:** The subagent system depends on the provider registry (step 7) for model info, the tool dispatcher (step 10) for registering the Agent tool, and the git client (step 11) for worktree operations. It's the last component before the TUI launch.

**Failure mode:** Non-fatal — sweep failures are logged as warnings. If the Agent tool fails to register, the program exits (this is a wiring error that shouldn't happen in正常 builds).

---

#### Step 15: TUI Launch

```go
app := tui.NewApp(cfg, configPath, registry, sessionMgr, dispatcher, gitClient,
    ledgerClient, rollbackClient, autoDreamClient, Version, themeMode)
app.SetCwd(workDir)
app.SetKeychain(kc)
app.SetSubagentManager(subagentMgr)

p := tea.NewProgram(app, tea.WithAltScreen(), tea.WithMouseCellMotion())
```

**What it does:** Constructs the Bubble Tea application with all dependencies injected, then launches it:

1. **App creation** — `tui.NewApp()` receives all 12 dependencies and creates the root `AppState` with 29 lazily-initialized sub-models.
2. **CWD, keychain, subagent manager** — set via setter methods to avoid constructor argument explosion.
3. **Program creation** — `tea.NewProgram` with alt screen (full-terminal buffer) and mouse cell motion (enables mouse scroll and click).
4. **Signal handler** — a goroutine watches for SIGTERM/SIGINT and sends `tea.QuitMsg{}` through the program channel instead of calling `app.Shutdown()` directly. This preserves Bubble Tea's single-threaded state mutation contract. A 5-second hard fallback writes a `.force-exit` sentinel file and calls `os.Exit(1)` if the TUI doesn't quit gracefully.
5. **Resume on startup** — if `cfg.Features.ResumeOnStartup` is true, the most recent session is loaded and set for automatic resume.
6. **`p.Run()`** — blocks until the TUI exits, then cancels the signal goroutine, calls `app.Shutdown()` for cleanup, and returns exit code 0.

**Why it's last:** The TUI depends on every other component — it's the integration point where all layers come together. Nothing can be initialized after the TUI starts because `p.Run()` blocks.

**Failure mode:** Fatal on TUI crash — if `p.Run()` returns an error, the program logs it and exits with code 1. The signal handler's 5-second fallback ensures the program doesn't hang if the TUI becomes unresponsive.

### 2.3 The Bubble Tea Foundation

M31 Autonomous's TUI is built on Bubble Tea, an Elm-architecture framework for terminal applications. Understanding Bubble Tea's model is essential to understanding how M31 Autonomous's 29 screens, dozens of concurrent operations, and complex workflow interactions cohere into a single, predictable application.

---

#### The Elm Architecture Pattern

Bubble Tea implements the Elm architecture (also known as The Elm Architecture or TEA), a pattern that enforces unidirectional data flow:

```
┌─────────────────────────────────────────────────────┐
│                    Bubble Tea Runtime                 │
│                                                       │
│   ┌─────────┐    msg     ┌─────────────────────┐    │
│   │         │ ─────────> │                     │    │
│   │  Model  │            │   Update(msg)       │    │
│   │ (state) │ <───────── │   → (newModel, cmd) │    │
│   │         │  new state │                     │    │
│   └─────────┘            └────────┬────────────┘    │
│        │                          │                   │
│        │ View()                   │ cmd               │
│        ▼                          ▼                   │
│   ┌──────────┐            ┌──────────────┐           │
│   │ Terminal │            │  Command     │           │
│   │ Output   │            │  (side effect)│           │
│   └──────────┘            └──────────────┘           │
│                                  │                    │
│                                  │ produces new msg   │
│                                  ▼                    │
│                           Back to Update()            │
└─────────────────────────────────────────────────────┘
```

Every Bubble Tea model is a Go struct implementing three methods:

```go
type Model struct { /* state */ }
func NewXxxModel(...) Model { return Model{...} }
func (m Model) Init() tea.Cmd { return nil }
func (m Model) Update(msg tea.Msg) (Model, tea.Cmd) { ... }
func (m Model) View() string { ... }
```

**`Init()`** — returns an initial command to execute when the model is created. Most models return `nil` (no initial command). Some return a `tea.Batch()` of multiple commands to kick off concurrent operations.

**`Update(msg tea.Msg)`** — the heart of the architecture. Receives a message (key press, mouse event, timer tick, workflow event, streaming chunk) and returns a new model state plus a command to execute. The command is a function that returns a new message when it completes — this is how side effects (HTTP requests, file I/O, LLM streaming) are handled without mutating state directly.

**`View()`** — renders the current state as a string for display. Called after every `Update()`. The view function is pure — it reads state but never mutates it.

The critical rule: **no goroutine may mutate AppState directly**. All state mutations go through `Update()`. This eliminates race conditions, makes the TUI predictable, and enables testability — you can test any screen by sending messages and asserting on the resulting state.

---

#### The AppState: Root Model

The `AppState` struct (`internal/tui/app_state.go`) is the root model that holds all application state. It is a single, unified struct containing:

**Layout state:**
- `width` / `height` — terminal dimensions, updated on `tea.WindowSizeMsg`.

**Screen routing:**
- `screen` — current active screen (enum: 29 values).
- `prevScreen` — previous screen (for transition detection).
- `screenStack` — back-stack for Esc-to-go-back navigation (capped at 16 entries).
- `screenCap` — maximum stack size.

**Theme:**
- `themeManager` — holds the current `Theme` struct with 80+ style fields, the color mode, profile, border style, and accent color.

**Configuration:**
- `config` — loaded `*config.Config` (all 10 TOML sections).
- `configPath` — path to the config file for hot-reload writes.

**Session:**
- `sessionManager` — `*session.Manager` for CRUD operations.
- `sessionID` — current session identifier.

**Provider/model:**
- `registry` — `*provider.Registry` for multi-provider management.
- `activeProvider` / `activeModel` — currently selected provider and model.
- `version` — build version string.

**Tools:**
- `dispatcher` — `*tools.Dispatcher` for permission-gated tool execution.

**Git:**
- `git` — `*git.Git` wrapper.

**Workflow:**
- `workflowEngine` — the six-phase workflow engine (set after initialization).
- `workflowPhase` / `workflowMode` / `workflowGoal` — current workflow state.
- `workflowCancel` — cancellation function for aborting running workflows.
- `shutdownCtx` / `shutdownCancel` — application-wide shutdown context.
- `emitterCh` — channel for receiving workflow engine messages.

**Sub-models (29 screens):**
Each screen has a corresponding pointer field on `AppState`. Sub-models are lazily created — they're only instantiated when first navigated to via `ensureSubModel()`. This avoids the cost of initializing all 29 screens at startup.

**Additional state:**
- Toast notifications (up to 3 visible, with auto-dismiss timers).
- Permission modal state (request, countdown, rich modal component).
- Confirmation dialog state.
- Arbitrage scorer.
- Health tracking.
- Screen transition overlay.
- Stream cancellation function.
- Double Ctrl+C exit tracking.
- File watcher for sidebar refresh.
- Subagent manager and model.
- Autonomous agent mode flag.
- Frecent history.
- Leader key and chord state.

---

#### Screen Routing

Screen routing is managed by four fields on `AppState`:

```go
screen      Screen        // current active screen
prevScreen  Screen        // previous screen (for transition detection)
screenStack []Screen      // back-stack for Esc-to-go-back navigation
screenCap   int           // max screen stack size (default 16)
```

**Navigation flow (`navigateToScreen`):**
1. Save current `screen` to `prevScreen`.
2. Push current screen onto `screenStack` (unless the target is the Permission overlay, which doesn't stack).
3. Start a `ScreenTransition` overlay (200ms dim-and-reveal animation) for real screen changes.
4. Call `ensureSubModel(screen)` to lazily create/resize the target sub-model.
5. Set `screen` to the new value.

**Back navigation (`popScreen`):**
1. Pop from `screenStack` and set `screen` to the popped value.
2. Fall back to `ScreenREPL` if the stack is empty.

**Key routing (`routeKeyMsg`):**
1. Check command palette first (highest priority overlay).
2. Check pending confirmation dialog.
3. Check sidebar focus (routes keys to sidebar when focused).
4. Handle leader key activation via `KeyRegistry`.
5. Route to the active screen's sub-model `.Update(msg)`.

The screen stack is capped at 16 entries to prevent unbounded growth during long sessions with many screen transitions. When the cap is reached, the oldest entry is dropped.

---

#### Screen Transitions

M31 Autonomous provides visual continuity between screen changes via a transition overlay:

```go
type ScreenTransition struct {
    active   bool
    progress float64  // 0.0 to 1.0
    duration time.Duration
    start    time.Time
}
```

When `navigateToScreen` is called for a real screen change (not just a refresh), a `ScreenTransition` is created with a 200ms duration. The `TransitionTick()` function advances the animation on each `TickMsg`, dimming the current screen and revealing the new one. When progress reaches 1.0, the transition completes and the screen switch is finalized.

This animation is purely visual — it has no effect on state or behavior. But it provides an important UX benefit: the user always sees a smooth transition rather than an abrupt screen swap, which reduces cognitive load during fast navigation.

---

#### The Message System

Bubble Tea's message system is the glue that connects concurrent operations to the single-threaded state mutation model. M31 Autonomous uses several categories of messages:

**User input messages:**
- `tea.KeyMsg` — keyboard input (key, modifiers).
- `tea.MouseMsg` — mouse events (cell motion, clicks, scroll).

**System messages:**
- `tea.WindowSizeMsg` — terminal resize (width, height).
- `tea.TickMsg` — timer fires (for animations, periodic refreshes).
- `tea.BatchMsg` — multiple commands executing concurrently.

**Workflow messages (from `MsgEmitter`):**
- `TaskStartMsg` / `TaskUpdateMsg` — task lifecycle events.
- `ToolStartMsg` / `ToolCompleteMsg` — tool execution events.
- `SelfHealStartMsg` / `SelfHealCompleteMsg` — self-healing events.
- `PhaseTransitionStartMsg` / `PhaseTransitionCompleteMsg` — phase transitions.
- `StreamChunkMsg` — progressive LLM streaming tokens.
- `DemonstrationReadyMsg` — ship phase demonstration content.

**Streaming messages:**
- `StreamMsg{Chunk}` — individual streaming chunks from the LLM.
- `StreamDoneMsg` — streaming complete (final message + usage).
- `StreamErrorMsg` — streaming error.

**Agent messages:**
- `AgentStreamMsg` / `AgentToolStartMsg` / `AgentToolDoneMsg` — agent loop events.
- `AgentDoneMsg` / `AgentErrorMsg` — agent completion/error.

**TUI messages:**
- `PermissionRequestMsg` / `PermissionResponseMsg` — tool permission flow.
- `QuestionRequestMsg` / `QuestionResponseMsg` — user question flow.
- `ConfigReloadMsg` — config file changed.
- `SubagentEventMsg` — subagent lifecycle events.
- `ScreenTransition` — screen change animation.
- `Toast` — notification display.

All messages flow through `AppState.Update()`, which pattern-matches on the message type and routes to the appropriate handler. This central routing point ensures all state mutations are serialized and predictable.

---

#### The View Rendering Pipeline

The `View()` method on `AppState` renders the terminal output in a specific order:

1. **Command palette overlay** — if open, rendered on top of everything (highest priority).
2. **Permission/question modal overlay** — rendered on top of the active screen.
3. **Minimum screen guard** — if terminal is smaller than 40 columns or 10 rows, shows a "terminal too small" message.
4. **Unified chrome** — header + content + footer via `layout.RenderPage()`. The header shows the app name, model, and workflow phase. The footer shows context-appropriate keyboard hints.
5. **Sidebar composition** — split view on wide terminals (>= `sidebar_width_threshold`), overlay on narrow terminals.
6. **Toast overlay** — top-right of content area, stacked vertically.
7. **Screen transition dim effect** — semi-transparent overlay during 200ms transitions.

Each screen's content renderer returns ONLY the content area (no chrome). This separation means every screen gets consistent headers, footers, and sidebars without duplicating layout code.

---

## 3. The Workflow Engine

The workflow engine is the heart of M31 Autonomous. It implements a six-phase pipeline that transforms a user's intent into verified, committed code.

### 3.1 Phase Overview

The workflow engine implements a six-phase pipeline. Each phase is a self-contained module with its own file, its own system prompt composition, and its own error handling. Phases communicate via the `MsgEmitter` interface, which sends typed messages to the TUI without coupling the engine to Bubble Tea.

| Phase | Purpose | Key Files | Entry Function |
|-------|---------|-----------|----------------|
| Initialize | Project detection, git setup, planning directory | `initialize.go` | `runInitialize()` |
| Discuss | Clarifying questions via LLM streaming | `discuss.go` | `runDiscuss()` |
| Plan | Structured implementation plan generation | `plan.go`, `plan_parser.go` | `runPlan()` |
| Execute | Task scheduling, tool dispatch, self-healing | `execute.go` | `runExecute()` |
| Verify | Build validation, test execution, bisect | `verify.go`, `engine_verify.go` | `runVerify()` |
| Ship | Final commit, ledger entry, session archival | `ship.go` | `runShip()` |

---

#### Phase Transition Rules

Not all phase transitions are legal. A `validPhaseTransitions` map enforces the allowed paths:

```
Initialize → Discuss, Plan, Execute, Ship
Discuss → Plan, Execute, Ship
Plan → Execute, Ship
Execute → Verify, Ship
Verify → Execute, Ship, Discuss
Ship → (terminal — returns to REPL)
```

Key constraints:
- You cannot go backward from Ship to any earlier phase.
- You cannot skip from Initialize directly to Verify (you must execute first).
- The Verify phase can loop back to Execute (for re-try after self-healing) or to Discuss (for clarification).

A **Plan-Discuss oscillation guard** caps round-trips between Plan and Discuss at 3 cycles. Without this, the agent could theoretically oscillate forever — Plan generates questions, Discuss answers them, Plan regenerates with new questions, and so on. The counter resets when the workflow leaves the Plan/Discuss subgraph (i.e., when it moves to Execute or beyond).

---

#### Workflow Modes

M31 Autonomous supports four workflow modes that determine which phases are executed:

| Mode | Phase Sequence | When Used |
|------|---------------|-----------|
| **full** | Init → Discuss → Plan → Execute → Verify → Ship | Moderate/Complex goals |
| **fast** | Init → Discuss → Execute → Verify → Ship | Simple goals (skips Plan) |
| **direct** | Init → Execute → Ship | Trivial goals (skips Discuss, Plan, Verify) |
| **auto** | Classifies goal, then maps to full/fast/direct | Default behavior |

The `auto` mode uses the complexity classifier (Section 3.7) to determine which mode is appropriate. Trivial goals like "rename the foo function to bar" go through Direct mode — no discussion, no plan, no verification. Simple goals like "add a string helper function" go through Fast mode — brief discussion but no formal plan. Moderate and Complex goals get the full six-phase treatment.

---

#### Per-Phase Model Selection

Each phase can use a different model. The engine resolves the model for each phase through a 4-level priority chain:

1. **Per-phase models** — set interactively by the TUI via `SetPhaseModel` (dual-model picker).
2. **Agents config** — per-phase overrides from `config.toml` (`[agents]` section).
3. **Global agent default** — `cfg.Agents.Default`.
4. **Engine model** — the fallback model set at initialization.

This allows users to use a cheap, fast model for Discuss (which is conversational) and an expensive, capable model for Execute (which writes code). The dual-model picker in the TUI makes this effortless — you select a "planning model" and a "coding model" before the workflow starts.

### 3.2 The Engine Struct

The `Engine` struct is the central orchestrator. It holds references to the session manager, provider, model ID, tool dispatcher, git client, token estimator, and domain packages. It also caches tool definitions, base prompts, project state, and codebase intelligence to avoid redundant computation.

Two design decisions stand out:

**Atomic cost tracking.** The engine tracks cumulative LLM cost across phases using `atomic.Uint64` storing a `float64` as raw bits. This allows lock-free concurrent updates via `CompareAndSwapUint64` while still supporting floating-point arithmetic. The budget check happens at the start of every `RunPhase()` call, and the cost accumulator adds after each phase completes.

```go
// Atomic CAS for budget tracking (engine.go)
oldBits := atomic.LoadUint64(&e.totalCostBits)
for {
    oldCost := math.Float64frombits(oldBits)
    newCost := oldCost + cost
    newBits := math.Float64bits(newCost)
    if atomic.CompareAndSwapUint64(&e.totalCostBits, oldBits, newBits) {
        break
    }
    oldBits = atomic.LoadUint64(&e.totalCostBits)
}
```

**Phase transition guards.** A `validPhaseTransitions` map defines which phase transitions are legal. For example, you can go from Initialize to Discuss, but not from Ship to Execute. A Plan-Discuss oscillation guard caps round-trips at three cycles, preventing the agent from getting stuck in an infinite refinement loop.

### 3.3 Prompt System

M31 Autonomous's prompt system is built on eleven markdown templates embedded into the binary at compile time via Go's `embed.FS`. This approach ensures prompts are always available (no filesystem dependency), versioned with the code, and never accidentally modified by users.

---

#### Template Inventory

| Template | Injected In | Purpose |
|----------|-------------|---------|
| `base.md` | Every phase | Identity, core principles, read-before-write rule |
| `tool-use.md` | Plan, Execute, Heal | Tool usage guidelines and safety rules |
| `plan-format.md` | Plan | Structured plan output format (H1-H4, JSON task list) |
| `execute-task.md` | Execute | Task execution process instructions |
| `discuss-questions.md` | Discuss | Clarifying question generation rules |
| `self-heal.md` | Execute, Verify | Diagnostic process and root cause taxonomy |
| `demonstration-format.md` | Ship | Post-completion walkthrough generation |
| `autonomous.md` | Direct, Fast modes | Autonomous execution mode instructions |
| `context-awareness.md` | Plan, Execute | Context window management awareness |
| `code-quality.md` | Plan, Execute, Heal | Code quality guidelines and standards |
| `code-intelligence.md` | Plan, Execute, Heal | Codebase intelligence integration instructions |

---

#### System Prompt Composition

System prompts are composed by concatenating the base prompt with phase-specific templates, separated by `---` markers. The composition happens in `buildSystemPrompt()`:

```
[base.md content]
---
[phase-specific template 1]
---
[phase-specific template 2]
---
[...]
```

**Per-phase prompt composition:**

| Phase | Templates Combined |
|-------|-------------------|
| Initialize | `base.md` |
| Discuss | `base.md` + `discuss-questions.md` |
| Plan | `base.md` + `tool-use.md` + `plan-format.md` + `context-awareness.md` + `code-quality.md` + `code-intelligence.md` |
| Execute | `base.md` + `tool-use.md` + `execute-task.md` + `context-awareness.md` + `code-quality.md` + `code-intelligence.md` |
| Verify (heal) | `base.md` + `tool-use.md` + `self-heal.md` + `code-quality.md` + `code-intelligence.md` |
| Ship | `base.md` + `demonstration-format.md` |
| Direct/Fast | `base.md` + `autonomous.md` + `tool-use.md` + `execute-task.md` |

The base prompt is cached via `sync.Once` since it never changes after initialization — this avoids recomposing the base prompt on every phase transition.

---

#### What the Base Prompt Contains

The base prompt (`base.md`) establishes the agent's identity and core behavioral rules:

- **Identity:** "You are M31 Autonomous, a terminal-native AI coding agent."
- **Read-before-write rule:** Always read files before modifying them. Never assume file contents.
- **Tool-first approach:** Use tools to verify assumptions rather than guessing.
- **Error reporting:** When a tool fails, report the error to the user rather than silently continuing.
- **Safety:** Never execute destructive commands without explicit user approval.
- **Conciseness:** Keep responses focused and actionable.

---

#### Prompt Injection Prevention

The prompt system includes several defenses against injection:

- **Delimiter separation:** `---` markers between prompt sections make it harder for user input to escape its context.
- **Role separation:** User messages are always in the `user` role, system prompts in the `system` role. The LLM provider enforces this boundary.
- **Context length awareness:** The `context-awareness.md` template instructs the model to be aware of its context window and truncate gracefully when approaching limits.
- **Tool result framing:** Tool results are framed as system messages with clear delimiters, preventing user input in tool arguments from being interpreted as instructions.

### 3.4 Phase Deep Dive: Initialize

The Initialize phase (`initialize.go`) performs project detection by probing for file markers in priority order: `go.mod` for Go, `Cargo.toml` for Rust, `pyproject.toml` for Python, `package.json` for Node.js, and so on. It initializes a git repository if one doesn't exist, creates the `.m31a/` planning directory, and writes `PROJECT.md` and `STATE.md` files.

Every step checks for context cancellation, allowing the user to abort cleanly at any point. This is a small but important detail: the engine never blocks the user.

### 3.5 Phase Deep Dive: Discuss

The Discuss phase bridges the gap between the user's initial goal and the implementation plan. Rather than jumping straight to planning, it streams clarifying questions from the LLM to ensure the plan is well-informed.

---

#### The Discuss Flow

1. **Context assembly:** The engine builds a context containing the system prompt (base + `discuss-questions.md`), the session's `MEMORY.md` (cross-session learnings), `PROJECT.md` (project type, framework), and the user's goal.

2. **LLM streaming:** The engine calls `streamLLMStreaming()` which returns a raw `StreamIterator`. The TUI renders tokens progressively as they arrive — the user sees the questions being "typed" in real-time.

3. **Question parsing:** As tokens arrive, the engine parses questions using three cascading strategies:
   - **Primary:** Numbered items ending with "?" (e.g., "1. What database are you using?") — highest confidence.
   - **Fallback:** Numbered items without "?" (e.g., "1. Database choice") — for models that omit punctuation.
   - **Last resort:** Any line containing "?" — lowest confidence, used when the other strategies find nothing.

4. **Question capping:** At most 4 questions are returned. If the LLM generates more, only the first 4 are kept. This prevents overwhelming the user.

5. **Answer collection:** The TUI displays the questions and collects answers via the `DiscussModel`. The user can answer all questions or skip remaining ones via `/skip`.

6. **State persistence:** Answers are stored in a `DiscussState` struct (`map[int]string`) and persisted to `PROJECT.md` as a Q&A section. This ensures the answers survive session restarts.

7. **Repetition prevention:** Already-answered questions are injected into subsequent discuss contexts. When the LLM sees "Q: What database? A: PostgreSQL", it won't ask the same question again.

---

#### DiscussState Structure

```go
type DiscussState struct {
    Questions []string          // parsed questions from LLM
    Answers   map[int]string    // question index -> user answer
    NeedsAnswers bool           // true while answers are pending
}
```

The `NeedsAnswers` flag controls whether the TUI shows the answer input or proceeds to the next phase. When all questions are answered (or skipped), `FinalizeDiscuss()` is called, which:
1. Formats the Q&A as markdown.
2. Appends it to `PROJECT.md`.
3. Returns `NeedsAnswers: false`, allowing the workflow to transition to Plan.

---

#### When Discuss is Skipped

In **Fast** and **Direct** modes, the Discuss phase is skipped entirely. The workflow moves directly from Initialize to Execute (Fast) or from Initialize to Execute (Direct). This is appropriate for simple tasks where clarifying questions would add overhead without value — renaming a function doesn't require a discussion about architecture.

The `autonomous.md` prompt template is injected in these modes to instruct the LLM to make reasonable assumptions rather than asking questions.

---

#### Edge Cases

- **LLM generates no questions:** If the three parsing strategies find nothing, the phase completes with an empty Q&A and transitions to Plan. The agent proceeds with the goal as-is.
- **User skips all questions:** The phase completes immediately with whatever answers were provided (possibly none).
- **Context window pressure:** If the discuss context is too large (e.g., very long MEMORY.md), the engine truncates older messages to fit.
- **Streaming interruption:** If the user presses Ctrl+C during streaming, the stream is cancelled and the phase transitions to Plan with whatever questions were parsed so far.

### 3.6 Phase Deep Dive: Plan

The Plan phase is the most sophisticated component of the workflow engine. It generates a structured implementation plan from the user's goal, discuss answers, and project context — then validates, retries, and refines until the plan is correct.

---

#### The Plan Generation Flow

1. **Context assembly:** The engine builds a rich context containing:
   - System prompt (base + tool-use + plan-format + context-awareness + code-quality + code-intelligence).
   - `MEMORY.md` — cross-session learnings from previous runs.
   - CWD file schema — directory structure of the project.
   - Codebase intelligence summary — import graphs, symbol indices, relevance scores.
   - Discuss answers (if available).
   - Previous plan + feedback (on refinement iterations).

2. **LLM streaming:** The LLM generates a structured plan in markdown format. The response is streamed to the TUI in real-time.

3. **Plan parsing:** The `ParsePlan()` function extracts structured data from the markdown output using regex-based section extraction:
   - **Title** from the first H1 heading.
   - **Summary** from the first paragraph.
   - **Review Notes** from GitHub-style admonitions (`[!IMPORTANT]`, `[!WARNING]`).
   - **Open Questions** with optional suggested defaults.
   - **Proposed Changes** from H3 category groups and H4 `[NEW]`/`[MODIFY]` file entries.
   - **Verification Plan** with automated and manual subsections.
   - **Task List** from JSON in a fenced code block.

4. **Validation:** The parsed plan is validated against six criteria:
   - Duplicate task ID detection.
   - Self-reference detection (a task depending on itself).
   - Missing field validation (all required fields present).
   - Cycle detection via iterative DFS on the dependency graph.
   - Granularity warnings for tasks touching >3 files or with descriptions >80 words.
   - Reference validation (task dependencies must reference existing task IDs).

5. **Retry loop:** If validation fails, the errors are fed back to the LLM with the previous plan (truncated to 4000 chars) and the user's feedback. The LLM regenerates the plan incorporating the corrections. This retry loop runs up to `MaxPlanRetries = 3` times.

6. **Refinement:** If the user provides feedback (via `/refine`), the plan is regenerated with the feedback injected. Up to `MaxPlanRefinements = 5` refinements are allowed.

---

#### The Plan Parser (`plan_parser.go`)

The plan parser uses regex-based section extraction with cached compiled patterns:

```go
var compiledPatterns = sync.Map{}  // cache compiled regex patterns
```

**Extraction strategy:**
- Sections are identified by markdown heading levels (H1, H2, H3, H4).
- Task lists are extracted from JSON in fenced code blocks (```json ... ```).
- File entries are identified by `[NEW]` and `[MODIFY]` prefixes.
- Admonitions are identified by `[!IMPORTANT]` and `[!WARNING]` markers.

**Fallback parsing:** If the primary regex-based extraction fails (e.g., the LLM doesn't follow the exact format), a fallback `parseTasksFromJSON()` function attempts to extract tasks from any JSON array in the response. This handles models that output valid JSON but don't follow the markdown structure.

---

#### Task Validation Detail

Each task in the plan is validated:

```go
type Task struct {
    ID          string   `json:"id"`
    Description string   `json:"description"`
    Dependencies []string `json:"dependencies"`
    Files       []string `json:"files"`
    Type        string   `json:"type"`  // "create", "modify", "delete"
}
```

**Validation rules:**
- `ID` must be non-empty and unique across all tasks.
- `Description` must be non-empty.
- `Dependencies` must reference existing task IDs (no dangling references).
- No self-references (`id` not in `dependencies`).
- No circular dependencies (detected via iterative DFS with white/gray/black coloring).
- Granularity warnings: tasks with >3 files or >80-word descriptions are flagged for potential splitting.

---

#### Plan Output Structure

The parsed plan is stored as a `Plan` struct:

```go
type Plan struct {
    Title           string
    Summary         string
    ReviewNotes     []string
    OpenQuestions   []Question
    ProposedChanges []ChangeCategory
    VerificationPlan VerificationPlan
    Tasks           []Task
    RawMarkdown     string
}
```

The `RawMarkdown` field preserves the original LLM output for display in the Plan review screen. The structured fields are used by the Execute phase for task scheduling and by the Verify phase for validation.

---

#### Refinement Flow

When the user submits feedback via `/refine`:

1. The feedback is appended to the conversation context.
2. The previous plan (truncated to 4000 chars) is included for context.
3. The LLM regenerates the plan, incorporating the feedback.
4. The new plan is parsed and validated.
5. If validation passes, the plan is presented to the user for approval.
6. If validation fails, the retry loop continues.

The refinement counter resets when the workflow leaves the Plan/Discuss subgraph. This prevents infinite loops — after 3 oscillation cycles, the system forces a transition to Execute regardless of plan quality.

### 3.7 Complexity Classification (`internal/workflow/classify.go`)

Before entering a workflow, M31 Autonomous classifies the user's goal into one of four complexity levels. This classification determines which workflow mode is used, directly affecting how many phases the agent runs and how much human interaction is required.

---

#### The Four Complexity Levels

| Level | Workflow Mode | Phases Executed | Human Interaction |
|-------|--------------|-----------------|-------------------|
| **Trivial** | Direct | Init → Execute → Ship | Minimal (approve permission requests) |
| **Simple** | Fast | Init → Discuss → Execute → Verify → Ship | Brief discussion, no formal plan |
| **Moderate** | Full | Init → Discuss → Plan → Execute → Verify → Ship | Full discussion and planning |
| **Complex** | Full | Init → Discuss → Plan → Execute → Verify → Ship | Full discussion, planning, and refinement |

---

#### Indicator Categories

The classifier uses three categories of keyword indicators, matched via substring search (case-insensitive):

**Trivial indicators (10):**
"add", "create", "delete", "remove", "rename", "fix", "update", "bump", "pin", "chore"

These suggest small, well-scoped changes that don't require architectural discussion.

**Complex indicators (17):**
"build", "implement", "design", "architect", "refactor", "migrate", "integrate", "feature", "pipeline", "microservice", "full stack", "e2e", "authentication", "database", "api", "multi-step", "multi-phase"

These suggest significant engineering work that benefits from planning.

**Code complexity signals (40+):**
- **Security:** race, deadlock, oauth, jwt, ssrf, xss, cors, csrf, injection, sanitization, vulnerability, encryption, tls, ssl, auth, permission, access control, row level security
- **Concurrency:** goroutine, mutex, channel, concurrent, parallel, async, synchronization, lock, race condition, thread safe
- **Payments:** transaction, rollback, migration, payment, billing, invoice, subscription, refund, chargeback
- **Infrastructure:** websocket, grpc, middleware, load balancer, cache, queue, pub sub, event driven, circuit breaker, retry, backpressure

These signals indicate domain complexity that requires careful implementation.

---

#### The Classification Algorithm

The `ClassifyPrompt()` function executes in this order:

1. **Count indicators:** Scan the goal text for complex and trivial indicators via substring match.

2. **Multi-sentence check:** If the goal has multiple sentences (detected by `.`, `!`, `?` delimiters) AND word count > 20 AND contains complex indicators → **Complex**.

3. **Complex indicator check:** If any complex indicator is found → **Moderate** (may be upgraded later).

4. **Project size boosting:** Count files in the working directory. If >30 files AND the goal contains "add", "implement", or "create" → boost to **Moderate** (even if it would otherwise be Simple). Larger projects make "simple" changes more complex.

5. **Position-weighted code signals:** Scan for code complexity signals. Signals appearing in the **first 30%** of the goal text get a **2x score boost**. This reflects the observation that users tend to lead with the most important information — "fix the race condition in the payment handler" is more complex than "I want to fix the race condition in the payment handler."

6. **Code signal thresholds:**
   - 2+ code signals, or 1+ code signal mixed with complex indicators → **Complex**.
   - 1 code signal (alone) → **Moderate**.

7. **Trivial detection:** If word count ≤ 8 AND contains a trivial indicator AND is a single sentence → **Trivial**.

8. **Simple detection:** If word count ≤ 15 AND no complex indicators → **Simple**.

9. **Default:** If none of the above rules match → **Moderate**.

---

#### Mode Mapping (`WorkflowModeForComplexity`)

```go
func WorkflowModeForComplexity(level ComplexityLevel) types.WorkflowMode {
    switch level {
    case Trivial:
        return types.ModeDirect
    case Simple:
        return types.ModeFast
    case Moderate, Complex:
        return types.ModeFull
    default:
        return types.ModeFull
    }
}
```

This mapping is applied in `auto` mode. In `full`, `fast`, or `direct` modes, the classification is bypassed and the user's explicit choice is used.

---

#### Example Classifications

| Goal | Classification | Reasoning |
|------|---------------|-----------|
| "rename foo to bar" | Trivial | 4 words, single sentence, trivial indicator "rename" |
| "add a string helper function" | Simple | 6 words, no complex indicators |
| "fix the race condition in the auth middleware" | Moderate | 1 code signal ("race"), position-weighted (first 30%) |
| "implement JWT authentication with RS256 and refresh tokens" | Complex | Complex indicators ("implement", "authentication"), code signals ("jwt"), multi-word |
| "refactor the database layer to use connection pooling and add retry logic" | Complex | Complex indicators ("refactor", "database"), code signals ("retry"), multi-sentence potential |

### 3.8 Phase Deep Dive: Execute

The Execute phase is where the engine earns its keep. It creates a `taskrunner` that uses Kahn's algorithm for topological sorting, organizing tasks into dependency groups. Within each group, tasks execute in parallel with bounded concurrency (default: 4 concurrent goroutines via a semaphore channel).

Each task goes through a self-heal loop with up to 2 retry attempts:

1. Build execution context with system prompt, project state, plan narrative, task specification, codebase intelligence, and fresh file content from disk (capped at 32KB).
2. Call the LLM with tool definitions.
3. Parse tool calls from the response — either native function calling or text-based JSON fallback.
4. Execute tool calls in parallel with the semaphore.
5. Feed ALL tool results back to the LLM, including errors.
6. If tools fail, trigger `healTask()` which builds enhanced context with acceptance criteria, git diff, and codebase intelligence.
7. Commit changes scoped to task files.

The engine guards against degenerate cases: file-changing tasks that produce zero tool calls are automatically marked as failed.

### 3.9 Phase Deep Dive: Verify

The Verify phase validates that the Execute phase actually delivered working code. It runs four independent checks per completed task, attempts self-healing on failures, and falls back to git bisect when standard healing doesn't work. This is the quality gate between execution and shipping.

---

#### Verification Flow

1. **Load tasks:** Read `TASKS.md` from the session directory. If no tasks exist (e.g., Direct mode), return success immediately.

2. **Per-task verification:** For each task with `Status == Done`, run `verifyTask()`. Skipped tasks with declared files are tracked separately as warnings.

3. **Self-heal on failure:** If any check fails, the engine invokes `healTask()` with a diagnostic prompt containing the verification errors. The self-heal prompt uses the `self-heal.md` template and instructs the LLM to identify and fix the root cause.

4. **Bisect fallback:** If self-healing fails (or produces a result that still doesn't pass verification), the engine tries `tryBisectHeal()`. This runs `git bisect` between the session start commit and HEAD to find the exact commit that introduced the failure, then re-heals with that specific context.

5. **Re-verify after healing:** After any successful heal (direct or bisect), the task is re-verified. If it still fails, the task is marked as `Failed` or `Unrecoverable`.

6. **Save results:** Updated task statuses are saved to `TASKS.md`, a checkpoint is saved, and `STATE.md` is updated.

7. **Manual steps:** The plan's `Verification.Manual` section (if present) is passed to the TUI for display — these are steps the user should perform manually (e.g., "open browser and verify the UI looks correct").

---

#### The Four Verification Checks

| Check | What It Does | Failure Condition |
|-------|-------------|-------------------|
| **File existence** | `os.Stat()` each file declared in the task | File not found on disk |
| **Semantic check** | `git diff --name-only HEAD` to verify task files appear in the diff | Task files not in diff (for Modify actions) |
| **Build validation** | Project-type-specific compilation/syntax check | Compiler or syntax error |
| **Test execution** | Project-type-specific test runner | Test failure |

**Build validation by project type:**

| Project Type | Build Command | Detection |
|-------------|--------------|-----------|
| Go | `go build ./...` | `*.go` files in task |
| Node.js | `{pm} build` then `tsc --noEmit` fallback | `*.js`, `*.ts`, `*.jsx`, `*.tsx` files |
| Python | `python3 -m py_compile {file}` per file | `*.py` files |
| Rust | `cargo check` | `*.rs` files |

**Test execution by project type:**

| Project Type | Test Command | Detection |
|-------------|-------------|-----------|
| Go | `go test ./...` | `*_test.go` files or test files in same directory |
| Node.js | `{pm} test` (npm/yarn/pnpm/bun) | `*.test.js`, `*.test.ts` files |
| Python | `python3 -m pytest` | `*_test.py` files |

**Package manager detection** (`detectPackageManager()`): Checks for lock files in priority order: `pnpm-lock.yaml` → `yarn.lock` → `bun.lockb` → `package-lock.json`. Falls back to `npm run` if only `package.json` exists.

**Custom commands override:** If `[verify].build_command` or `[verify].test_command` is set in config, auto-detection is bypassed entirely. This allows projects with non-standard build systems to use their own commands.

---

#### Smart File Truncation

When the self-heal prompt needs to read task files for context, `readTaskFiles()` handles large files intelligently:

1. **Small files (≤8KB):** Read in full.
2. **Large files (>8KB):**
   - Always show the first 80 lines (package declaration + imports).
   - Search for function declarations (`func `, `def `, `function `, `async function `, `pub fn `) in the remaining lines.
   - If a match is found, show 60 lines starting from that function (including doc comments above it).
   - If no match, fall back to header + last 30 lines.

This ensures the LLM always sees the import context and the most relevant code, even for files that are thousands of lines long.

---

#### The Bisect Fallback

The bisect fallback is triggered when:
1. Direct self-healing fails (heal result is unsuccessful), OR
2. Self-healing succeeds but re-verification still fails.

The `tryBisectHeal()` function:

1. **Gets the base hash:** Uses `sessionStartHash` (captured at workflow start) or falls back to `git rev-list --max-parents=0 HEAD` (the root commit).

2. **Runs bisect:** `bisect.New()` creates a bisect runner, `b.Run()` executes `git bisect` with a check function that re-runs `verifyTask()`. Git bisect binary-searches the commit range to find the exact commit where the check started failing.

3. **Heals with context:** The offending commit's diff is included in the failure prompt: "bisect identified commit {hash} as introducing the failure: {diff}". This gives the LLM precise information about what broke.

4. **Re-verifies:** After healing, the task is re-verified. If it still fails, the heal attempt counter is incremented.

**Max heal attempts:** `MaxHealAttempts = 2` per task. After 2 failed heal attempts (whether direct or bisect-assisted), the task is marked `Unrecoverable`.

---

#### Timeout Protection

All verification commands run within a `verifyTaskTimeout = 5 minutes` deadline. This context deadline is derived from the parent context (which may itself have a deadline from session cancellation), so cancellation propagates cleanly. This prevents a hung `go build` or `cargo check` from blocking the entire verification phase indefinitely.

### 3.10 Phase Deep Dive: Ship

The Ship phase is the terminal phase of the workflow. It creates the final git commit, writes a ledger entry for cross-session learning, generates a demonstration walkthrough via the LLM, archives the session, and records learnings in `MEMORY.md`. This phase ensures every workflow run leaves a permanent, traceable artifact.

---

#### Ship Flow (8 Steps)

1. **Load tasks for summary:** Read `TASKS.md` and compute counts (total, done, failed, skipped) via `taskrunner.Summary()`.

2. **Final git commit:**
   - Collect all file paths declared across all tasks into `taskFiles`.
   - If there are uncommitted changes, check which dirty files are task-related vs unrelated. Log warnings for unrelated files.
   - If `taskFiles` is non-empty, `git add` only those files (scoped commit).
   - If `taskFiles` is empty (BUG-02 known issue), fall back to `git add all` — this commits unrelated files.
   - Check `git diff --cached` (staged changes). If empty, skip the commit entirely (no empty commits).
   - Commit with message: `{ship_prefix}: ship {session_id}` (e.g., `m31a: ship abc12345`).

3. **Build summary:** Compute duration since workflow start, collect commit log since start time (truncated to last 50 commits).

4. **Update ledger:** Write a summary entry to `~/.m31a/LEDGER.md` via `pkg/ledger`. Entry includes: session, total tasks, failed tasks, skipped tasks, commit count, duration.

5. **Save state:** Write `STATE.md` with phase=Ship, message="complete".

6. **Save checkpoint:** Record a checkpoint with phase=Ship and current timestamp. This is the last checkpoint before archival.

7. **Generate demonstration:** Call `generateDemonstration()` which builds a context from the original goal, implementation plan (truncated to 3000 chars), completed task summary, and commit log. This context is sent to the LLM with the `demonstration-format.md` template to produce a human-readable walkthrough of what was built. The demonstration is saved to `DEMONSTRATION.md` in the session directory and emitted as a `DemonstrationReadyMsg` to the TUI.

8. **Write MEMORY.md:** Append a session summary to `.m31a/MEMORY.md` for cross-session recall.

---

#### MEMORY.md Structure

The memory entry appended by Ship contains:

```markdown
## Session {id} ({date})
- Goal: {goal}
- Tasks done: {done}/{total}
- Project: {project_type} ({framework})
- Model: {model_used}
- Failed tasks:
  - Task {id} ({status}): {description} (heals attempted: {n})
- File patterns: .go(3), .py(1), .js(2)
- Duration: 12s
```

**What gets recorded:**
- Goal and task completion counts.
- Project type and framework (from `PROJECT.md`).
- Model used for the Execute phase.
- Failed tasks with their status and heal attempt counts (for diagnosing patterns across sessions).
- File extension patterns (e.g., `.go(3), .py(1)`) showing what types of files were touched.
- Duration (rounded to seconds).

This memory file is injected into future Discuss and Plan contexts, allowing the agent to learn from past sessions. For example, if a previous session failed to fix a race condition in Go code, the memory entry warns the next session to be careful with concurrency patterns.

---

#### Diff Stats Collection

`collectDiffStats()` computes file change statistics for the Ship summary:

1. **Base ref selection:** Prefers `sessionStartHash` (captured at workflow start). Falls back to `HEAD^` if available, or plain diff (working tree vs HEAD) as last resort.

2. **Numstat parsing:** Runs `git diff --numstat {base}..HEAD` and parses the output (`additions\tdeletions\tfilepath`).

3. **Classification refinement:** Runs `git diff --name-only --diff-filter=A` for added files and `--diff-filter=D` for deleted files, then subtracts from the total to get true modification count. This fixes a previous heuristic that misclassified add-only modifications (BUG-15).

**Output structure:**
```go
type DiffStats struct {
    Insertions     int
    Deletions      int
    FilesAdded     int
    FilesDeleted   int
    FilesModified  int
}
```

---

#### Ledger Entry

The ledger (`~/.m31a/LEDGER.md`) is a cross-session learning record. Each entry is a markdown table row containing:

| Field | Source | Purpose |
|-------|--------|---------|
| Session ID | `e.sessionID` | Unique identifier |
| Goal | First 80 chars of goal | What was attempted |
| Tasks | `done/total` | Completion ratio |
| Failed | `failed` count | Error tracking |
| Skipped | `skipped` count | Scope reduction tracking |
| Commits | `len(commits)` | Git activity |
| Duration | `time.Since(startTime)` | Performance tracking |

The ledger is append-only — entries are never modified or deleted. Stats can be computed from the ledger to track improvement over time.

---

#### Demonstration Generation

The `generateDemonstration()` function creates a human-readable walkthrough:

1. **Context assembly:** Combines the original goal, implementation plan (truncated to 3000 chars), task summary (completed/failed/skipped), and commit log.

2. **LLM generation:** Sends the context to the LLM with the `demonstration-format.md` template. The template instructs the model to produce a walkthrough describing what was built, how to run it, and any notable decisions.

3. **Fallback:** If LLM generation fails (network error, context overflow), the demonstration is empty and the phase continues without it.

4. **Output:** Saved to `DEMONSTRATION.md` in the session directory and emitted to the TUI as a `DemonstrationReadyMsg` for display.

---

#### Checkbox Task Update

After all other steps, `SaveTasksCheckbox()` writes a human-readable `tasks.md` file (separate from the machine-readable `TASKS.md`) with checkbox syntax:

```markdown
- [x] Task 1: Create the handler
- [x] Task 2: Add tests
- [ ] Task 3: Update docs (skipped)
```

This file is for user consumption — it provides a quick visual overview of what was completed.

---

#### Error Handling

Ship is designed to be resilient:
- If task loading fails, an empty task list is used (Ship still proceeds).
- If git operations fail, warnings are logged but Ship continues.
- If ledger update fails, a warning is logged but the session is still archived.
- If demonstration generation fails, an empty string is used.
- If MEMORY.md write fails, a warning is logged.

The only hard failure is the git commit itself — if `CommitStaged()` returns an error, Ship returns an error. This is intentional: a failed commit means the changes aren't persisted, which is a critical failure.

**Final success/failure:** If any tasks have status `Failed` or `Unrecoverable`, the phase result is marked as failed with an error message. This prevents the TUI from showing a "all clear" indicator when tasks actually failed.

---

## 4. The Provider System

### 4.1 Provider Abstraction

M31 Autonomous abstracts LLM providers behind a clean interface:

```go
type LLMProvider interface {
    Name() string
    APIKey() string
    FetchModels(ctx context.Context) ([]types.ModelInfo, error)
    CachedModels() []types.ModelInfo
    ChatCompletionStream(ctx context.Context, req ChatRequest) (*types.StreamIterator, error)
    EstimateCost(modelID string, usage types.Usage) float64
    HealthCheck(ctx context.Context) types.HealthStatus
    GetModel(id string) (*types.ModelInfo, error)
}
```

Two providers ship out of the box: OpenRouter (primary) and Zen (secondary). Both implement the same interface, making them interchangeable.

### 4.2 SSE Streaming Architecture

The SSE (Server-Sent Events) parser is a critical component. It reads streaming responses from LLM providers using `bufio.Scanner` with a 1MB line buffer. Key features include:

- **Watchdog timer** — a 30-second `time.AfterFunc` closes the HTTP body if no data arrives, preventing indefinite blocking on dead connections. The watchdog resets on every successful read.
- **Context cancellation** — checked between lines for clean abort.
- **Carriage return handling** — trims `\r` characters from providers that use `\r\n` line endings.
- **Empty data handling** — skips keep-alive and empty events instead of returning errors.
- **Idempotent close** — uses `sync.Once` to prevent double-close panics.

The parser extracts three types of data from streaming chunks: content tokens, reasoning/thinking tokens (with model-family-specific field paths), and native tool call deltas.

### 4.3 Model Cache with Singleflight

Model catalogs are cached with a dual-TTL strategy: fresh data for 5 minutes, stale data for 24 hours. When a network request is needed, `singleflight.Group` ensures only one HTTP request is made even if multiple goroutines request models simultaneously. A `refreshing` atomic flag prevents concurrent refresh attempts.

### 4.4 Automatic Fallback

When a provider degrades (429 rate limit, 503 unavailable, connection failure), M31 Autonomous automatically switches to an alternative. The fallback mechanism runs parallel health checks with a 10-second timeout, collects results, and switches deterministically based on candidate priority order.

For non-blocking scenarios, `FindFallbackWithRetryAfter()` returns a `FallbackAfterWait` result with the delay duration, allowing the TUI to schedule the switch asynchronously without blocking the event loop.

### 4.5 Thinking and Reasoning Model Support

M31 Autonomous supports extended thinking/reasoning across four model families (`internal/provider/reasoning.go`):

| Model Family | Prefix | SSE Field Path | Request Parameters |
|-------------|--------|---------------|-------------------|
| DeepSeek | `deepseek` | `choices.0.delta.reasoning_content` | (none) |
| OpenAI o-series | `openai/o-` | `choices.0.delta.reasoning` | `reasoning_effort: "medium"` |
| Anthropic | `anthropic` | `choices.0.delta.content` (type=thinking) | `thinking: {type: "enabled", budget_tokens: 1024}` |
| Qwen | `qwen` | `choices.0.delta.reasoning_content` | (none) |

The `ParseSSEChunk()` function extracts thinking, content, tool_call, done, and usage chunks from streaming responses. Anthropic thinking is detected via `delta.type == "thinking"` rather than a separate field path. Pre-computed `SSEFieldParts` avoid per-chunk `strings.Split` for performance. The `ApplyReasoningParams()` function injects model-specific parameters into the request body before sending.

---

## 5. The Tool System

### 5.1 Tool Interface

Every tool implements a simple interface:

```go
type Tool interface {
    Name() string
    Description() string
    RiskLevel() RiskLevel  // safe, medium, dangerous, destructive
    Execute(ctx context.Context, input ToolInput) (ToolResult, error)
}
```

The `RiskLevel` determines permission behavior: safe tools execute automatically, while dangerous and destructive tools always prompt the user.

### 5.2 The 15 Registered Tools

Each tool is a self-contained module implementing the `Tool` interface. They are registered on the tool dispatcher at startup and invoked by the LLM via tool calls. The following details cover every tool's purpose, parameters, security behavior, and practical use cases.

---

#### **Bash** — Shell Command Execution

**Risk:** dangerous | **Timeout:** 30 minutes | **Output cap:** 50,000 characters

The Bash tool executes arbitrary shell commands in the project directory. It is the most powerful and most dangerous tool in the arsenal — every invocation requires user approval unless explicitly allowed in the permission config.

**Security measures:**
- Non-interactive environment injection: `CI=true`, `DEBIAN_FRONTEND=noninteractive`, `npm_config_yes=true`, `PIP_NO_INPUT=1`. These prevent commands from hanging on interactive prompts.
- Explicit stdin closure via `cmd.Stdin = nil` — prevents commands from reading user input.
- Process group setup (`cmd.SysProcAttr = &syscall.SysProcAttr{Setpgid: true}`) for clean signal forwarding.
- SIGINT sent on context cancellation; SIGKILL after a 5-second grace period (`bash_kill_grace_secs` config).
- Thread-safe output limiting via `limitWriter` — silently drops data after the 50K cap without crashing.
- Binary detection via null byte check in the first 512 bytes of output.

**Use cases:** Running build commands (`go build ./...`), executing tests (`pytest`), installing dependencies (`npm install`), checking git status, running linters, or any operation that requires shell access.

**Example inputs:**
- `{"command": "go test ./..."}` — run Go tests
- `{"command": "git log --oneline -5"}` — view recent commits
- `{"command": "find . -name '*.go' | wc -l"}` — count Go files

---

#### **FileRead** — Read File Contents

**Risk:** safe | **Limit:** 5MB per file

Reads a file from disk and returns its contents as text. Automatically detects binary files (null byte check in first 512 bytes) and rejects them with a descriptive error. Enforces a 5MB size limit to prevent memory exhaustion.

**Security measures:**
- Path traversal guards: symlinks are resolved to their real paths before checking that the resolved path is within the working directory prefix.
- Binary detection: files containing null bytes in the first 512 bytes are rejected with `ErrNoBinaryContent`.

**Use cases:** Reading source files to understand existing code, inspecting configuration files, reviewing log output, or examining any text file in the project. The LLM uses this tool extensively before making changes — the "read-before-write" principle is enforced in the base prompt.

**Example inputs:**
- `{"path": "main.go"}` — read a Go source file
- `{"path": "internal/config/types.go", "start_line": 50, "end_line": 100}` — read a specific line range

---

#### **FileWrite** — Write File Contents

**Risk:** destructive | **Backup:** automatic with pruning

Writes content to a file, creating it if it doesn't exist or overwriting it if it does. This is a destructive operation — it replaces the entire file contents. Every write is atomic and backed up.

**Write process:**
1. Path resolution with symlink evaluation.
2. Path traversal check (resolved path must be within workDir).
3. Backup existing file with timestamp + random suffix.
4. **Prune BEFORE write** — prevents the new backup from being accidentally pruned on fast disks.
5. Write to temp file (`m31a_tmp_<random>`).
6. `fsync()` for durability.
7. `os.Rename()` for atomic replacement.

**Backup rotation:** keeps up to `MaxBackupsPerFile` (default: 10) backups per file. Older backups are pruned on each write.

**Use cases:** Creating new files, completely replacing file contents (e.g., regenerating a config file), or writing generated code. For partial edits, prefer the Edit tool which preserves unchanged content.

**Example inputs:**
- `{"path": "README.md", "content": "# My Project\n\n..."}` — create or overwrite README
- `{"path": ".env", "content": "API_KEY=secret"}` — write environment file

---

#### **Edit** — Partial File Modification

**Risk:** dangerous | **Strategies:** 5 cascading approaches

The Edit tool modifies a specific section of a file without rewriting the entire contents. It is the primary tool for code changes — the LLM generates a search string and a replacement, and the tool finds and replaces the match.

**Five cascading strategies (tried in order):**

1. **Line-range** — `start_line` + `end_line` parameters. Replaces lines[startIdx..endIdx] inclusive. Most precise, no ambiguity.
2. **Exact match** — `strings.Index()` for verbatim string matching. Works when the search text is unique.
3. **Line-trimmed** — trims whitespace before comparison, preserves original indentation via relative indent computation. Handles inconsistent indentation.
4. **Whitespace-normalized** — collapses all whitespace to single spaces. Works when formatting changed but content is the same.
5. **Fuzzy-anchor** — requires >= 3 lines. Matches first and last lines exactly, checks middle lines via Levenshtein similarity (threshold: 0.7). Handles LLM output that doesn't perfectly match the file.

**Additional features:**
- Binary content rejection.
- File size check against `MaxFileSize` (5MB).
- Atomic write with backup and pruning after successful replacement.
- Diff summary generated for each edit.

**Use cases:** Refactoring a function, adding an import statement, modifying a struct definition, updating a return value, or any change that affects a specific region of a file without rewriting the whole thing.

**Example inputs:**
- `{"path": "main.go", "old_string": "func oldName()", "new_string": "func newName()"}` — rename a function
- `{"path": "config.go", "start_line": 10, "end_line": 15, "new_string": "// updated config"}` — replace a line range

---

#### **Glob** — File Pattern Matching

**Risk:** safe | **Limit:** 1000 results

Finds files matching glob patterns using the doublestar library, which supports `**` for recursive directory matching. Results are sorted by modification time (newest first).

**Features:**
- Respects `.gitignore` rules when ripgrep is available (rg-aware filtering).
- Skips hidden directories, `.git`, `node_modules`, `vendor`, and other build artifacts.
- 1000 result cap to prevent overwhelming output on large projects.

**Use cases:** Finding all Go files (`**/*.go`), locating test files (`**/*_test.go`), finding configuration files (`**/package.json`), or discovering files by name pattern across the project.

**Example inputs:**
- `{"pattern": "**/*.go"}` — all Go source files
- `{"pattern": "internal/**/*.go"}` — Go files under internal/
- `{"pattern": "**/*config*"}` — files with "config" in the name

---

#### **Grep** — Content Search

**Risk:** safe | **Limit:** 100 results

Searches file contents using regular expressions. Prefers ripgrep (`rg`) when available for performance, falling back to a pure-Go implementation.

**Features:**
- ripgrep with JSON output for structured results.
- Pure-Go fallback with ReDoS detection — blocks patterns with nested quantifiers likely to cause catastrophic backtracking.
- Respects `.gitignore` with mtime-based cache invalidation (re-reads from disk only when `.gitignore` modification time changes).
- 100 result cap.

**Use cases:** Finding where a function is defined (`func\s+ProcessPayment`), locating all usages of a variable, searching for TODO comments (`TODO|FIXME|HACK`), or finding specific error handling patterns.

**Example inputs:**
- `{"pattern": "func.*ProcessPayment", "include": "*.go"}` — find Go functions matching a name
- `{"pattern": "TODO|FIXME", "include": "*.go"}` — find all TODO/FIXME comments
- `{"pattern": "os\\.ReadFile", "path": "internal/"}` — search within a specific directory

---

#### **WebFetch** — URL Content Retrieval

**Risk:** medium | **Limit:** 5MB | **Max redirects:** 5

Fetches content from a URL and converts HTML to markdown. Designed for reading documentation, API references, and other web content during development.

**SSRF protection (three layers):**
1. **DNS pinning** — resolves and caches IP addresses for 5 minutes via `sync.Map`. Prevents TOCTOU rebinding attacks where an attacker changes DNS between resolution and connection.
2. **Private IP blocking** — checks loopback, link-local, RFC1918, IPv6 ULA, and cloud metadata addresses (169.254.169.254). Uses `net.IP.IsPrivate()` (Go 1.17+).
3. **Post-connect verification** — re-checks the remote address after TCP connection (paranoid check).

**Additional security:**
- Redirect protection: resolves DNS for redirect targets, checks for private IPs, enforces max 5 redirects.
- DNS cache eviction: threshold-gated (every 64 inserts) to prevent unbounded memory growth.
- HTML-to-markdown conversion using `bluemonday` for sanitization and `goldmark` for rendering.
- 5MB content limit.

**Use cases:** Reading API documentation, checking library versions, fetching Stack Overflow answers, or retrieving any web content that helps with the current task.

**Example inputs:**
- `{"url": "https://pkg.go.dev/net/http"}` — read Go HTTP package docs
- `{"url": "https://docs.python.org/3/library/json.html", "format": "markdown"}` — read Python docs

---

#### **WebSearch** — Privacy-Respecting Search

**Risk:** medium | **Max query length:** 500 characters

Searches the web using SearXNG or Brave Search APIs. Returns structured results with titles, URLs, and snippets.

**Features:**
- Configurable base URL (`tools.websearch_base_url`, default: SearXNG instance).
- Can be disabled via `tools.websearch_enabled = false`.
- Query length limit of 500 characters to prevent abuse.
- Results returned as formatted text with source attribution.

**Use cases:** Finding documentation for unfamiliar libraries, searching for error solutions, checking current best practices, or researching API endpoints when documentation isn't available locally.

**Example inputs:**
- `{"query": "Go context cancellation best practices 2026"}` — search for Go patterns
- `{"query": "React useEffect cleanup function"}` — search for React docs

---

#### **TodoWrite** — Task List Management

**Risk:** safe

Manages a task list stored in the session directory. Supports creating, updating, and completing tasks with markdown persistence.

**Features:**
- Tasks stored in `.m31a/todos.md` as a markdown checklist.
- Supports task status: pending, in_progress, completed.
- Priority levels: high, medium, low.
- Tasks are session-scoped and persist across messages.

**Use cases:** Tracking multi-step work within a session, managing a checklist of subtasks, or maintaining a running list of items to address. The LLM uses this to maintain state across long conversations.

**Example inputs:**
- `{"todos": [{"content": "Add input validation", "status": "in_progress", "priority": "high"}]}` — update task status

---

#### **AskUserQuestion** — Interactive User Prompts

**Risk:** safe

Sends a question to the user and waits for a response. The question appears in the REPL as a system message, and the user's response is returned to the LLM as a tool result.

**Features:**
- Per-request routing via `sync.Map` for concurrent tool execution.
- Timeout: configurable via `permissions.timeout_seconds` (default: 300s).
- Response is captured exactly as typed, including multi-line input.

**Use cases:** Asking the user for clarification on ambiguous requirements, confirming a design decision, requesting a password or API key, or gathering any information that requires human judgment.

**Example inputs:**
- `{"question": "Should I use PostgreSQL or MongoDB for this project?"}` — ask for a technical decision
- `{"question": "What port should the server listen on?"}` — ask for a configuration value

---

#### **FileList** — Directory Listing

**Risk:** safe

Lists the contents of a directory with file sizes. Returns a formatted table of files and subdirectories.

**Features:**
- Shows file sizes in human-readable format (bytes, KB, MB).
- Lists both files and directories.
- Sorted by name.
- Respects `.gitignore` for filtering.

**Use cases:** Exploring project structure, checking what files exist in a directory, verifying that a build output directory was created, or inspecting a directory before making changes.

**Example inputs:**
- `{"path": "."}` — list current directory
- `{"path": "internal/tools"}` — list the tools directory

---

#### **FileDelete** — Remove Files

**Risk:** dangerous

Deletes a file from disk. This is a dangerous operation — deleted files cannot be recovered through M31 Autonomous (though git may have a copy).

**Features:**
- Path traversal guards (symlink resolution + workDir prefix check).
- Confirmation required via permission modal.
- No backup pruning (unlike FileWrite and Edit — a known tech debt item).

**Use cases:** Removing generated files, cleaning up temporary files, deleting unused configuration files, or removing files that were created by mistake.

**Example inputs:**
- `{"path": "tmp/output.log"}` — delete a temporary log file
- `{"path": "old_config.yaml"}` — remove an obsolete config file

---

#### **FileMove** — Rename or Move Files

**Risk:** dangerous

Renames or moves a file from one path to another. Both source and destination paths are validated against path traversal guards.

**Features:**
- Source path traversal check (symlink resolution + workDir prefix check).
- Destination path traversal check (must also be within workDir).
- Atomic rename via `os.Rename()`.
- Confirmation required via permission modal.

**Use cases:** Renaming source files to match new class/function names, reorganizing project structure, moving files between directories, or correcting file locations after refactoring.

**Example inputs:**
- `{"source": "old_name.go", "destination": "new_name.go"}` — rename a file
- `{"source": "utils.go", "destination": "internal/utils/utils.go"}` — move to a subdirectory

---

#### **CodeMap** — Project Structure Analysis

**Risk:** safe

Analyzes the project structure and returns a high-level overview including directory tree, file counts by language, and key configuration files.

**Features:**
- Scans the project directory recursively.
- Counts files by extension (go, js, ts, py, rs, etc.).
- Identifies key files: go.mod, package.json, Cargo.toml, pyproject.toml, Makefile, Dockerfile.
- Returns a formatted summary suitable for LLM consumption.

**Use cases:** Understanding an unfamiliar project before making changes, verifying project type detection, getting a quick overview of the codebase structure, or providing context for the LLM before planning.

**Example inputs:**
- `{"path": "."}` — map the entire project
- `{"path": "internal/"}` — map only the internal directory

---

#### **Agent** — Subagent Spawning

**Risk:** safe | **Max concurrent:** 8 | **Max tools:** 50 | **Max turns:** 25

Spawns a parallel subagent that runs in an isolated git worktree with its own tool dispatcher and message history. Subagents can run in background (returns immediately) or foreground (blocks until completion) mode.

**Features:**
- **Worktree isolation** — creates git worktrees under `.m31a-worktrees/<agentID>` on branches `m31a/agent-<agentID>[-<suffix>]`.
- **Stale worktree sweep** — runs at startup, prunes git worktree metadata, deletes orphaned branches.
- **Per-subagent budgets** — 50 tools, 50,000 tokens, 25 LLM round-trips.
- **Event system** — 8 event types (spawned, tool_start, tool_done, text_delta, thinking, done, error, cancelled) via buffered channel (capacity 256).
- **`isChild` flag** — prevents recursive fan-out; child agents are forced to foreground mode.

**Use cases:** Delegating parallel tasks (e.g., "write tests for auth while I refactor the API"), running background analysis, or splitting a large task into independent subtasks that can be worked on simultaneously.

**Example inputs:**
- `{"task": "Write unit tests for the payment package", "background": false}` — run foreground subagent
- `{"task": "Analyze the security of the auth module", "background": true}` — run background subagent

---

### 5.3 Permission Model

The permission system is three-layered:

1. **Rule-based** — config-defined patterns matching tool names and parameter values using doublestar glob syntax.
2. **Agent profile** — per-agent default action from configuration.
3. **Risk-level fallback** — dangerous+ tools always prompt the user.

Permission requests use per-request channels with `sync.Map` for concurrent tool execution. Each request gets a dedicated channel, and the permission modal displays a y/n/a/e choice (allow once, allow always, deny, exit). "Allow always" decisions are cached by tool name and command, so repeated operations don't prompt again.

The default permission timeout is 300 seconds. If the user doesn't respond, the tool call times out cleanly.

### 5.4 Security Features

**Bash tool security** is particularly thorough:

- Non-interactive environment injection (`CI=true`, `DEBIAN_FRONTEND=noninteractive`, `npm_config_yes=true`).
- Explicit stdin closure to prevent interactive prompts.
- Process group setup for clean signal forwarding.
- SIGINT on context cancel, SIGKILL after a 5-second grace period.
- Thread-safe output limiting via a `limitWriter` that silently drops data after the cap.
- Binary detection via null byte check in the first 512 bytes.

**SSRF protection** in WebFetch uses three layers:

1. DNS pinning — resolves and caches IP addresses for 5 minutes, preventing TOCTOU rebinding attacks.
2. Private IP blocking — checks loopback, link-local, RFC1918, IPv6 ULA, and cloud metadata addresses.
3. Post-connect verification — re-checks the remote address after TCP connection.

**Path traversal guards** appear on every file tool: symlinks are resolved to their real paths, and the resolved path must be within the working directory prefix.

### 5.5 The Edit Tool's Five-Strategy Cascading Replace

The Edit tool (`internal/tools/edit.go`) deserves special attention for its sophisticated text replacement algorithm. Rather than relying on a single matching strategy, it cascades through five approaches in order of strictness:

1. **Line-range replacement** — when `start_line` and `end_line` are provided, replaces lines[startIdx..endIdx] inclusive. Most precise, no ambiguity.

2. **Exact match** — `strings.Index()` for verbatim string matching. Works when the search text is unique in the file.

3. **Line-trimmed match** — trims whitespace from both search and content lines before comparison, then preserves original indentation by computing relative indent between matched and replacement lines. This handles files with inconsistent indentation.

4. **Whitespace-normalized match** — collapses all whitespace sequences to single spaces using `strings.Fields()` + join. Pre-computes normalized content lines for performance. Works when formatting has changed but content is the same.

5. **Fuzzy-anchor match** — requires at least 3 lines (`MinLinesForFuzzy`). Matches the first and last lines exactly, then checks middle lines using Levenshtein similarity. Accepts if average similarity >= 0.7 (`LevenshteinThreshold`). The Levenshtein implementation uses single-row DP for O(min(m,n)) space efficiency.

Each strategy preserves original line endings and generates a diff summary. The cascade ensures the tool works even when the LLM's output doesn't perfectly match the file's current state — a common occurrence when working with generated code.

### 5.6 Atomic File Operations

M31 Autonomous's file persistence follows a consistent atomic write pattern (`internal/fileutil/atomic.go`):

1. **Permission preservation** — if the target file exists, inherits its current permissions instead of defaulting to 0644.
2. **Temp file creation** — `os.CreateTemp(dir, ".m31a_tmp_*")` with explicit chmod to override the restrictive 0600 default.
3. **Write + Sync** — data is flushed to disk via `fsync()` before rename.
4. **Atomic rename** — `os.Rename(tmpPath, path)` is POSIX-atomic on the same filesystem.
5. **Deferred cleanup** — temp file is removed on error paths.

This pattern is replicated in the Edit tool with additional backup creation and pruning (keeping `MaxBackupsPerFile` per file), and in the FileWrite tool with pre-write backup pruning to prevent new backups from being accidentally pruned on fast disks.

---

## 6. The Autonomous Agent Loop and Subagent System

Beyond the six-phase workflow engine, M31 Autonomous includes an autonomous agent loop that can independently plan, execute, and iterate on tasks without human intervention.

### 6.1 The Agent Loop (`internal/tui/streaming/agent_loop.go`)

The `AgentLoop()` function implements a self-directed tool-use cycle:

1. Send messages to the LLM.
2. Parse tool calls from the response (native function calling or text-based JSON fallback).
3. Execute tool calls and feed results back.
4. Repeat until done or limits are hit.

**Key parameters:**
- `agentMaxIterations = 50` — hard cap on loop iterations.
- `DefaultMaxTools = 50` — maximum tool calls per session.
- `DefaultMaxTokens = 50,000` — token budget.
- `maxTurns = 25` — maximum LLM round-trips.

**Eight message types** communicate state to the TUI: `AgentStreamMsg`, `AgentToolStartMsg`, `AgentToolDoneMsg`, `AgentDoneMsg`, `AgentErrorMsg`, `AgentThinkingMsg`, `AgentIterationDoneMsg`, `AgentIterationMsg`.

### 6.2 Context Pruning

The agent loop implements a three-tier progressive pruning system to prevent context window overflow:

| Threshold | Action | Details |
|-----------|--------|---------|
| **70%** of context | Prune old tool results | Keeps last 3 tool results in full; older ones replaced with `"[tool result pruned to save context]"` |
| **80%** of context | Also prune old assistant content | Keeps last 5 assistant messages in full; older ones truncated to 1000 chars |
| **95%** of context | Abort with error | Returns `AgentErrorMsg` with estimated vs limit token count |

Additionally, `TruncateMessagesForLLM()` proactively reserves 20% of context for response generation before the loop starts, keeping system messages first then adding most recent messages until the budget is exhausted.

### 6.3 Text-Based JSON Fallback

For models that don't support native function calling, the agent loop includes `parseTextToolCalls()` which scans up to 64KB of response text for JSON objects with `"name"` or `"tool"` fields. This handles models that embed tool calls in markdown code blocks — a common pattern with open-weight models.

### 6.4 The Subagent System

M31 Autonomous can spawn parallel subagents that run in isolated environments:

**Architecture:**
- `Manager` orchestrates lifecycles with semaphore-bounded concurrency (`MaxConcurrent = 8`).
- Each subagent runs in its own goroutine with its own message history, tool dispatcher, and working directory.
- **Worktree isolation** — `GitWorktrees` creates git worktrees under `.m31a-worktrees/<agentID>` on branches `m31a/agent-<agentID>[-<suffix>]`.
- **Stale worktree sweep** — runs at startup, prunes git worktree metadata, deletes orphaned `m31a/agent-*` branches for crash recovery.
- **Background/foreground** — `req.Background = false` blocks until the subagent finishes; `true` returns immediately.
- **DispatcherFactory** — builds a fresh tool dispatcher per subagent workspace, avoiding circular imports.

**Event system:** 8 event types (spawned, tool_start, tool_done, text_delta, thinking, done, error, cancelled) communicated via a buffered channel (capacity 256). Lifecycle events block up to 500ms; verbose deltas are dropped when the channel is full.

---

## 7. The TUI System

### 7.1 Screen Inventory

M31 Autonomous has 29 screens (plus 2 overlay screens), organized into five categories. Each screen is a Bubble Tea model with `Init()`, `Update()`, and `View()` methods. Screens are lazily created via `ensureSubModel()` — they're only instantiated when first navigated to, avoiding the cost of initializing all 29 at startup.

---

#### Core Workflow Screens (8)

These screens correspond to the six phases of the workflow engine plus the goal entry and model selection screens that precede them.

##### **repl_model** — The Main Chat Interface

The REPL is the primary screen and the application's home base. It's a split-pane layout with a scrollable message viewport on top and a text input area at the bottom.

**What it displays:**
- Conversation history (up to 1000 messages, configurable via `MaxMessageHistory`).
- Streaming LLM responses in real-time with token-by-token rendering.
- Tool call results (file reads, bash outputs, grep results) in collapsible cards.
- Welcome screen when no messages are present (version, quick-start hints).
- Permission modals when the workflow engine requests tool access.

**Key components:**
- `viewport.Model` — scrollable message list with mouse wheel support.
- `textarea.Model` — multi-line text input with 3-row height, 2000-char limit.
- `components.Spinner` — animated spinner during streaming.
- `components.MessageRenderer` — renders markdown-formatted messages with syntax highlighting.

**State fields:**
- `messages []types.Message` — conversation history.
- `streaming bool` — whether a stream is active.
- `thinking bool` — whether the LLM is in "thinking" mode.
- `streamSegments []types.MessageSegment` — parsed streaming segments for display.
- `userScrolled bool` — tracks whether the user has scrolled up (prevents auto-scroll).

**Interactions:**
- `Enter` — send message.
- `Ctrl+C` — cancel active stream (2nd press exits).
- `Ctrl+B` — toggle sidebar.
- `Ctrl+P` — open command palette.
- `?` — toggle help overlay.
- `Up/Down` — scroll message history.
- Mouse wheel — scroll viewport.

---

##### **goalinput_model** — Full-Screen Goal Entry

A dedicated full-screen text area for entering workflow goals. This screen appears when the user presses `Ctrl+N` (new session) or selects "New Goal" from the command palette.

**What it displays:**
- A large text area (adapts to terminal height minus 10 rows, minimum 3 rows).
- Placeholder text: "Describe the goal for this coding session..."
- Recent goals list (frecency-sorted, from `pkg/history`).
- Character count (2000-char limit).

**Key components:**
- `textarea.Model` — full-width text input with placeholder.
- Recent goals navigation via `Up/Down` when `showRecent` is true.

**State fields:**
- `recentGoals []string` — history of past goals.
- `showRecent bool` — whether the recent goals list is visible.
- `recentIdx int` — cursor position in the recent goals list.

**Interactions:**
- `Enter` — submit goal and transition to workflow.
- `Up/Down` — navigate recent goals (when visible).
- `Esc` — return to REPL.
- `Tab` — toggle between recent goals and text input.

---

##### **phasemodelpicker** — Dual-Model Picker

A dual-panel model selection screen that lets users choose separate models for planning and coding phases. This appears after goal submission and before the workflow starts.

**What it displays:**
- Two side-by-side panels: "Planning Model" (left) and "Coding Model" (right).
- Each panel has a search input, scrollable model list, and per-token cost display.
- Models are fetched asynchronously from all configured providers.
- Loading spinners while models are being fetched.

**Key components:**
- Two `pickerPanel` structs, each containing: `textinput.Model` for search, model list with cursor/scroll, `components.Spinner` for loading state.
- Fuzzy search across model ID, name, and provider.

**State fields:**
- `panels [2]pickerPanel` — planning and coding panels.
- `activePanel int` — which panel is focused (0=planning, 1=coding).
- `confirmed bool` — whether selection is confirmed.

**Interactions:**
- `Tab` — switch between planning and coding panels.
- `Up/Down` — navigate model list within active panel.
- `Enter` — select model for active panel.
- `/` — focus search input.
- `Ctrl+A` — select same model for both panels.
- `Enter` (on confirmation) — start workflow with selected models.

---

##### **plan_model** — Plan Review and Refinement

Displays the implementation plan generated by the Plan phase. Users can review tasks, see cost estimates, and provide feedback for refinement.

**What it displays:**
- Plan markdown content in a scrollable viewport.
- Task list with wave groupings (dependency levels).
- Cost and time estimates for the selected model.
- Plan version counter (incremented on each refinement).
- Refine input mode for providing feedback.

**Key components:**
- `viewport.Model` — scrollable plan content display.
- `PlanRefineModel` — inline text input for refinement feedback.

**State fields:**
- `tasks []types.Task` — parsed task list.
- `waves [][]types.Task` — pre-computed wave groupings.
- `planContent string` — raw markdown plan.
- `planVersion int` — refinement counter.
- `confirmMode bool` — whether the user is confirming the plan.
- `refineMode bool` — whether the refine input is active.

**Interactions:**
- `y` — approve plan and start execution.
- `e` — enter refine mode to provide feedback.
- `Up/Down` — scroll plan content.
- `Esc` — return to REPL.

---

##### **discuss_model** — Clarifying Questions Q&A

Presents clarifying questions from the Discuss phase one at a time. This screen appears when the workflow engine generates questions before planning.

**What it displays:**
- Current question (large text).
- Progress indicator (e.g., "Question 2 of 4").
- Text input for the answer.
- Timeout countdown (if configured).
- Previous questions and answers (in a scrollable list above).

**Key components:**
- `textinput.Model` — single-line answer input with 500-char limit.
- Timeout tracking via `time.Timer`.

**State fields:**
- `questions []string` — all questions from the LLM.
- `current int` — index of the current question.
- `answers []string` — collected answers.
- `timeout int` — seconds until auto-skip (0 = no timeout).
- `hasDeadline bool` — whether a timeout is active.

**Interactions:**
- `Enter` — submit answer and move to next question.
- `Esc` — skip remaining questions.
- `Tab` — cycle through previous answers.

---

##### **execute_model** — Task Execution Progress

Displays real-time task execution progress with animated progress bars and live output streaming.

**What it displays:**
- Task list with status indicators (pending, running, done, failed, skipped).
- Animated progress bar showing overall completion.
- Live output from the currently running task (last 200 lines).
- Elapsed time since execution started.
- Task dependency wave indicators.

**Key components:**
- `viewport.Model` — scrollable task list and output.
- `components.Spinner` — animated spinner for the running task.
- `components.AnimatedProgressBar` — smooth progress animation.

**State fields:**
- `tasks []types.Task` — task list with statuses.
- `liveOutput []string` — output lines from the current task.
- `currentTask int` — index of the currently running task (-1 if none).
- `paused bool` — whether execution is paused.
- `startedAt time.Time` — execution start time.
- `prevDone int` — for detecting task completion changes.

**Interactions:**
- `p` — pause/resume execution.
- `Up/Down` — scroll task list/output.
- `Enter` — expand selected task details.

---

##### **verify_model** — Verification Results

Displays the results of the Verify phase, including per-task check outcomes and self-healing controls.

**What it displays:**
- Task verification results (file existence, syntax, build, tests).
- Pass/fail indicators per check type.
- Manual verification steps from the plan (if any).
- Healing controls: "Heal" button for failed tasks.
- Healing progress (attempt count, max attempts).

**Key components:**
- `viewport.Model` — scrollable verification results.
- `components.Spinner` — animated spinner during healing.

**State fields:**
- `results map[int]workflow.VerificationResult` — per-task results.
- `manualSteps []string` — manual verification steps.
- `healFunc func(taskID int) tea.Cmd` — callback for triggering heal.
- `healingTaskID int` — which task is being healed (-1 if none).
- `healAttempt int` — current heal attempt number.
- `healCursor int` — cursor position in the failed tasks list.

**Interactions:**
- `h` — trigger heal for the selected failed task.
- `Up/Down` — navigate task list.
- `Enter` — expand task details.
- `Esc` — proceed to Ship (or return to Execute if tasks failed).

---

##### **ship_model** — Ship Summary

Displays the workflow completion summary with task counts, commit information, diff stats, and the generated demonstration walkthrough.

**What it displays:**
- Session ID and model/provider used.
- Task completion stats (done/total, failed, skipped).
- Commit list with short hashes and messages.
- Diff stats (files added/modified/deleted, insertions/deletions).
- Duration and total token/cost estimates.
- Demonstration walkthrough (toggleable viewport).

**Key components:**
- `viewport.Model` — scrollable demonstration content.

**State fields:**
- `summary ShipSummary` — all completion statistics.
- `demonstration string` — LLM-generated walkthrough.
- `demoViewport viewport.Model` — viewport for demonstration display.
- `showDemo bool` — whether the demonstration is visible.

**Interactions:**
- `d` — toggle demonstration display.
- `Up/Down` — scroll demonstration content.
- `Esc` — return to REPL.

---

#### Utility Screens (14)

These screens provide supplementary functionality — configuration, browsing, analytics, and management.

##### **modelselector_model** — Fuzzy Model Search

A full-screen model/provider picker with search, scroll, and per-token cost comparison. Unlike the `phasemodelpicker` (which selects two models), this screen selects a single model for ad-hoc use.

**What it displays:**
- Models grouped by provider name.
- Search input with fuzzy matching across model ID, name, and provider.
- Per-token cost display for each model.
- Loading spinners per provider.
- Error indicators for providers that failed to fetch models.

**State fields:**
- `providers []string` — provider names.
- `modelsByProv map[string][]models.ModelInfo` — models grouped by provider.
- `searchInput textinput.Model` — search field.
- `activeProvider string` — currently selected provider tab.
- `filtered []types.ModelInfo` — filtered model list.
- `cursor/offset int` — scroll position.
- `errored map[string]bool` — providers with errors.

---

##### **settings_model** — 6-Tab Settings Editor

A comprehensive settings editor with six tabs for configuring all aspects of M31 Autonomous.

**Tabs:**
1. **Provider** — API keys, auto-fallback toggle, health status.
2. **Model** — Default model, auto-arbitrage toggle, threshold.
3. **UI** — Theme, compact mode, cost estimate display.
4. **Keys** — Keybindings, leader key configuration.
5. **Workflow** — Permission mode, timeout, autodream toggle.
6. **About** — Version, license, system info.

**What it displays:**
- Tab bar at the top with tab names.
- Settings fields with type-appropriate controls: text inputs, toggles, dropdowns, password fields.
- Provider health check results (async).
- Save confirmation toast.

**State fields:**
- `tabs []SettingsTab` — tab definitions.
- `activeTab int` — current tab index.
- `fields []settingsField` — fields for the active tab.
- `healthResults []providerHealthStatus` — async health check results.

---

##### **sidebar_model** — Git Status & File Tree

A collapsible sidebar panel that shows git status, file tree, token usage, and session info.

**What it displays:**
- Git branch name and remote.
- Modified/added/deleted files with status indicators.
- File tree (collapsible directories) for navigation.
- Token usage stats (total tokens, context length, cost).
- Current model name and provider.
- Session ID and version.

**State fields:**
- `git *git.Git` — git wrapper.
- `files []git.FileStatus` — changed files.
- `branch/remote string` — git branch info.
- `tree *components.FileTree` — file tree component.
- `visible bool` — sidebar visibility.
- `focused bool` — whether keyboard focus is on the sidebar.
- `totalTokens/contextLen int`, `cost float64` — token stats.

**Interactions:**
- `Ctrl+B` — toggle visibility.
- `Ctrl+G` — focus sidebar (routes keys to sidebar).
- `Enter` — open selected file in diff viewer.
- `Left/Right` — expand/collapse directories.
- `Esc` — unfocus sidebar.

---

##### **cmdpalette** — Command Palette

A fuzzy-search command palette (like VS Code's Ctrl+P). Lists all available commands with descriptions.

**What it displays:**
- Search input at the top.
- Filtered command list with icons and descriptions.
- Keyboard shortcut hints.

**Interactions:**
- Type to filter commands.
- `Enter` — execute selected command.
- `Esc` — close palette.

---

##### **dashboard_model** — Workflow Pipeline Overview

A visual pipeline overview showing all six phases with completion status.

**What it displays:**
- Phase pipeline: Initialize → Discuss → Plan → Execute → Verify → Ship.
- Current phase highlighted.
- Completed phases marked with checkmarks.
- Goal, model, provider, and cost info.
- Activity timeline (recent events).

**State fields:**
- `phases []string` — all phase names.
- `current string` — current phase.
- `completed map[string]bool` — completed phases.
- `activity []components.TimelineEntry` — event timeline.

---

##### **ledger_model** — Learning Ledger Browser

Displays the cross-session learning ledger (`~/.m31a/LEDGER.md`) in a scrollable viewport.

**What it displays:**
- Ledger entries in a formatted table.
- Session ID, goal, task counts, commit count, duration per entry.
- Scrollable viewport for browsing history.

**State fields:**
- `ledger *ledger.Ledger` — ledger data source.
- `entries []ledger.LedgerEntry` — parsed entries.
- `viewport viewport.Model` — scrollable display.

---

##### **rollback_model** — Commit Time Machine

Shows the git commit timeline and allows resetting to any commit with soft/hard/safe reset options.

**What it displays:**
- Commit list with hashes, messages, and timestamps.
- Diff viewer for the selected commit.
- Reset confirmation dialog (soft/hard/safe).

**State fields:**
- `rollback *rollback.Rollback` — rollback data source.
- `entries []rollback.RollbackEntry` — commit entries.
- `showDiff bool` — whether the diff viewer is visible.
- `confirmReset string` — pending reset type ("soft"/"hard"/"").

**Interactions:**
- `Enter` — toggle diff viewer for selected commit.
- `s` — soft reset to selected commit.
- `h` — hard reset (with confirmation).
- `Up/Down` — navigate commit list.

---

##### **diff_model** — Diff Viewer

Displays a git diff with syntax coloring and line-level additions/deletions.

**What it displays:**
- File path header.
- Diff content with syntax coloring (green for additions, red for deletions).
- Addition/deletion counts.
- Scrollable viewport.

**State fields:**
- `diff string` — raw diff content.
- `title string` — diff title.
- `filePath string` — file being diffed.
- `additions/deletions int` — line counts.

---

##### **metrics_model** — Session Analytics

Displays aggregate session analytics across all past sessions.

**What it displays:**
- Total sessions, messages, and tokens.
- Average tokens per session.
- Active (non-archived) sessions count.
- Provider usage breakdown.
- Model usage breakdown.
- Phase distribution (which phases are used most).

**State fields:**
- `stats metricsStats` — aggregate statistics.
- `Providers/Models/Phases map[string]int` — usage breakdowns.

---

##### **bisect_model** — Git Bisect Interactive

An interactive git bisect interface for manually finding breaking commits.

**What it displays:**
- Commit list with status indicators (pending, good, bad, skip, testing).
- Progress indicator (current/total).
- Current commit being tested.
- Status message.

**State fields:**
- `commits []bisectCommit` — commit list with statuses.
- `current int` — index of current commit.
- `total int` — total commits in range.
- `status string` — "idle", "testing", "done".

**Interactions:**
- `g` — mark current commit as good.
- `b` — mark current commit as bad.
- `s` — skip current commit.
- `Up/Down` — navigate commit list.

---

##### **fileexplorer_model** — File Tree Browser

A full-screen file tree browser for navigating the project structure.

**What it displays:**
- Collapsible directory tree.
- File sizes.
- Directory expand/collapse indicators.

**State fields:**
- `tree *components.FileTree` — file tree component.

**Interactions:**
- `Enter` — expand/collapse directory or open file.
- `Up/Down` — navigate tree.
- `Esc` — return to previous screen.

---

##### **help_model** — Keybinding Help Overlay

A scrollable overlay showing all keybinding sections.

**Sections:**
- **Global** — `?`, `ctrl+c`, `ctrl+p`, `ctrl+b`, `ctrl+g`, `esc`.
- **REPL** — message navigation, input controls.
- **Workflow** — phase-specific keys.
- **Navigation** — screen transitions.
- **Sidebar** — file tree controls.

**State fields:**
- `sections []helpSection` — help content.
- `viewport viewport.Model` — scrollable display.

---

##### **themepicker_model** — Theme Browser with Live Preview

Lets users browse and preview 10 built-in theme presets.

**Built-in presets:**
Dark (Default), Light, Nord, Tokyo Night, Gruvbox, Rose Pine, Dracula, Solarized, Monochrome, Catppuccin.

**What it displays:**
- Theme list with color swatches (background, brand, text colors).
- Live preview of the selected theme.
- Current theme indicator.

**State fields:**
- `themes []themePreset` — theme definitions.
- `cursor int` — selected theme index.

---

##### **firstrun_model** — API Key Setup Wizard

A 5-step wizard for first-time setup: Welcome → Provider Select → API Key → Model Pick → Done.

**What it displays:**
- Step-by-step progression with visual indicators.
- Provider selection (OpenRouter, Zen).
- API key input (masked).
- Categorized model browser with icons.
- Completion confirmation.

**State fields:**
- `step firstRunStep` — current wizard step.
- `provider string` — selected provider.
- `apiKey string` — entered API key.
- `modelBrowser modelBrowserState` — model selection state.

---

##### **resume_model** — Session Browser

Lists past sessions for resuming work. Sessions are displayed with search and scroll support.

**What it displays:**
- Session list with IDs, goals, dates, and statuses.
- Search input for filtering sessions.
- Scroll support for long lists.

**State fields:**
- `sessions []session.SessionInfo` — session list.
- `searchInput textinput.Model` — search field.
- `searching bool` — whether search mode is active.

---

##### **notification_model** — Notification History

Displays a history of toast notifications with type indicators (info, warning, error).

**What it displays:**
- Notification list with timestamps and type badges.
- Scrollable viewport.

**State fields:**
- `list components.NotificationList` — notification data.

---

##### **confirmquit_model** — Confirm Quit Dialog

A confirmation dialog that appears when the user tries to quit during active processing.

**What it displays:**
- Warning message: "An operation is in progress. Are you sure you want to quit?"
- Yes/No options.

**Interactions:**
- `y` — confirm quit.
- `n` — cancel and return.

---

#### Overlay Screens (2)

These are special screens that overlay the current screen rather than replacing it.

##### **ghostpicker_model** — Ghost Write File Selector (V1.1)

A file selector for ghost write operations — selecting files to be modified by a background agent.

**What it displays:**
- File list with checkboxes for selection.
- Select all/none controls.

**State fields:**
- `files []ghostFileEntry` — files with selection state.

---

##### **subagents_model** — Sub-Agent Management

A panel that shows parallel sub-agent activity. Displays between the header and REPL in wide layouts.

**What it displays:**
- Ordered list of sub-agent rows (never reordered for mental model stability).
- Per-agent: ID, status, last event (e.g., "Grep foo", "Read x.go", "done").
- Expandable rows for detailed output.

**State fields:**
- `rows []SubagentRow` — agent snapshots.
- `index map[string]int` — ID-to-position mapping.
- `cursor int` — highlighted row.

---

#### Additional Screens (5)

##### **tooldetail_model** — Expanded Tool Output

A full-screen viewer for expanded tool output with scrolling. Shows the complete content of a tool call result (bash output, file content, grep results) that was truncated in the REPL view.

**State fields:**
- `title string` — tool name.
- `content string` — full tool output.
- `viewport viewport.Model` — scrollable display.

---

##### **sessiondetail_model** — Session Detail View

Shows detailed information about a session before loading it. Displays session metadata, task list, and messages.

**State fields:**
- `sess *session.Session` — session data.

### 7.2 Theme System

The theme system supports three modes (dark, light, auto) and 10 registered palettes:

| ID | Name |
|----|------|
| `dark` | Midnight |
| `light` | Daylight |
| `catppuccin` | Catppuccin Mocha |
| `nord` | Nord Frost |
| `tokyo` | Tokyo Night |
| `gruvbox` | Gruvbox Dark |
| `rose` | Rose Pine |
| `dracula` | Dracula |
| `solarized` | Solarized Dark |
| `monochrome` | Pure Mono |

Each theme defines 80+ fields covering colors, pre-computed lipgloss styles, diff colors, card styles, workflow phase styles, and layout constants. The theme manager detects the terminal's color profile (TrueColor, 256-color, or 16-color ANSI) and applies appropriate fallbacks.

Theme changes propagate to all 28+ sub-models via a `SetTheme()` method, ensuring visual consistency across every screen.

### 7.3 Key Binding System

M31 Autonomous uses a `KeyRegistry` with 17 key contexts and a leader key system (default: `ctrl+x` with a 1-second timeout). This allows chord-based shortcuts like `ctrl+x s` for settings or `ctrl+x d` for dashboard, without conflicting with text input.

**Global chords (available from any screen):**

| Chord | Action |
|-------|--------|
| `ctrl+x s` | Settings |
| `ctrl+x h` | Help |
| `ctrl+x l` | Ledger |
| `ctrl+x k` | Rollback |
| `ctrl+x d` | Dashboard |
| `ctrl+x p` | Theme picker |
| `ctrl+x !` | Notifications |
| `ctrl+x f` | File explorer |
| `ctrl+x a` | Subagents panel |
| `ctrl+x c` | Config viewer |
| `ctrl+x i` | Session detail |
| `ctrl+x o` | Tool output |

### 7.4 The MsgEmitter Pattern

The workflow engine and TUI communicate through a channel-based message emitter. The engine defines a `MsgEmitter` interface with a single `Emit(msg any)` method. The TUI implements this with a buffered channel (capacity 128) and a timeout-based send that drops messages if the channel is full (preventing deadlock).

The TUI's `drainEmitterCmd()` function reads from this channel in a loop, processing each message and re-registering for the next read. This creates a continuous drain chain that processes workflow events (task starts, tool completions, phase transitions, self-heal events) without blocking the Bubble Tea event loop.

### 7.5 The Command Palette

Activated via `ctrl+p`, the command palette (`internal/tui/cmdpalette.go`) provides fuzzy search across all 47 commands organized into 6 categories: Core, AI, Config, Session, Git, and Workflow.

**Fuzzy scoring** awards bonuses for consecutive character matches (+1) and word boundary matches (+2). The palette renders at 2/3 terminal width (min 40, max 72 columns), bottom-anchored above the status bar with a rounded border in the brand color. Matched characters are highlighted in bold brand color. The palette also maps 18 keyboard shortcuts for quick access (e.g., `?` for help, `ctrl+m` for model, `ctrl+x d` for dashboard).

### 7.6 The Sidebar

The sidebar (`internal/tui/sidebar_model.go`) provides persistent project context:

- **Git information** — branch name, remote tracking, colored status pills (modified=yellow, added=green, deleted=red, untracked=gray).
- **Token usage display** — context meter bar (8 segments), token count, cost estimate, model name.
- **File tree** — directory expand/collapse with file status from `git status --porcelain`. Built with O(1) child lookup via per-level index.
- **Periodic refresh** — async git status reload via `SidebarRefreshTickMsg`.
- **Mouse support** — click to show file diff, click directory to toggle.
- **Resizable** — default 30 columns, min 20, max 50, adjustable via `ctrl+x [` and `ctrl+x ]`.

### 7.7 The @-Mention System

The REPL supports `@path` file references (`internal/tui/mention.go`):

- **File scanning** — scans the working directory on demand, caching results for 30 seconds. Skips hidden dirs, `.git`, `node_modules`, `vendor`, `__pycache__`, `dist`, `build`, `target`. Maximum 500 entries.
- **Filter scoring** — basename prefix match = 3, path prefix match = 2, path contains match = 1. Returns top 8 candidates.
- **Resolution** — `ResolveMentions()` scans the message for `@path` tokens, reads file contents, and injects them as context. Each file is capped at 8,000 bytes. Files under 100KB get line counts counted for display.

### 7.8 The Toast Notification System

Toast notifications (`internal/tui/toast.go`) provide non-intrusive feedback:

- **Types** — success (green checkmark), error (red cross), warning (yellow triangle), info (brand-colored dot).
- **Position** — top-right, right-aligned with 2-char margin.
- **Stacking** — up to 3 toasts visible simultaneously, with a buffer of 5.
- **Duration** — configurable per toast, default 5 seconds.
- **Progress bar** — visual countdown using filled/empty block characters.
- **Animation** — slide-in from right (20-char offset over frames 0-1).
- **Notification history** — all toasts are recorded in a scrollable list accessible via `/notifications`.

### 7.9 The Diff Viewer

The diff viewer (`internal/tui/diff_model.go`, `diff_view.go`) provides colorized git diff display:

- **Color coding** — `+` lines: green background; `-` lines: red background; `@@` hunk headers: bold thinking color; context lines: muted.
- **Line numbers** — right-aligned 4-char width with `|` separator.
- **Stats bar** — proportional add/delete ratio bar scaled to 1/6 terminal width, with addition/deletion counts.
- **Scrollable** — wraps a `viewport.Model` for keyboard/mouse navigation.

### 7.10 Streaming Segments

The REPL's streaming display manages three segment types: `"thinking"` (reasoning content), `"content"` (normal response text), and `"tool_use"` (tool call JSON). Segments are tracked via `streamSegments` and finalized when streaming completes. Thinking blocks are collapsible (toggled by `ShowThinkingByDefault` config) with duration tracked in milliseconds. Tool cards are built from finalized `tool_use` segments and displayed with execution status.

---

## 8. Domain Packages

### 8.1 Session Management (`pkg/session/`)

Sessions are project-local, stored in `<workDir>/.m31a/` as flat JSON and Markdown files. This design makes sessions portable alongside project directories and avoids the complexity of a global session database.

Key features:
- **Checkpoint/restore** — lightweight snapshots (phase, timestamp, message count) with a maximum of 2 retained checkpoints.
- **Workflow state persistence** — goal, phase, and discussion questions survive application restarts.
- **Atomic writes** — all persistence goes through `fileutil.AtomicWrite()` (temp file + rename).
- **File size limiting** — reads capped at 50MB to prevent OOM.
- **Cryptographic session IDs** — 8 hex characters from `crypto/rand`.
- **Gitignore management** — automatically adds `.m31a/` to `.gitignore` on session creation.

### 8.2 Cross-Session Learning Ledger (`pkg/ledger/`)

Every shipped session's metadata is recorded to a persistent markdown table at `~/.m31a/LEDGER.md`. This enables the system to learn from past sessions: identifying which models work best for which project types, computing average costs, and tracking failure patterns.

The ledger uses append-only writes for efficiency (only the new row is appended to the existing file) with a fallback to full rewrite on append failure. An mtime-based cache avoids recomputing statistics when the file hasn't changed.

### 8.3 Commit Rollback Chain (`pkg/rollback/`)

The rollback system provides three types of reset:

- **SoftReset** — preserves staged changes, creates a backup branch.
- **HardReset** — discards everything, creates a timestamped backup branch (`m31a/rollback-backup-<unix-timestamp>`).
- **SafeReset** — discards changes, then restores uncommitted changes from stash.

All reset operations auto-stash if the working tree is dirty. The chain browsing function returns commits with diffs, capped at 50,000 characters.

### 8.4 Git Bisect Automation (`pkg/bisect/`)

Bisect wraps `git bisect` to automatically identify which commit introduced a regression. A user-supplied check function runs at each bisection step. The implementation uses the narrow `GitRunner` interface (just `Run(args ...string)`) for dependency injection, enabling test doubles.

### 8.5 Task Runner (`pkg/taskrunner/`)

The task runner implements Kahn's algorithm for topological sorting, organizing tasks into dependency groups. Within each group, tasks execute with bounded parallelism via a semaphore channel. Failed tasks can retry with linear backoff, and dependency cascade skips propagate failures to downstream tasks.

### 8.6 AutoDream Context Consolidation (`pkg/autodream/`)

AutoDream solves the context window overflow problem. When conversations grow long, older messages are summarized into a compact "memory segment." The system protects critical messages from consolidation:

- First message (initial goal)
- All system messages
- All messages with tool calls
- All tool result messages
- User messages containing plan specs or task lists
- Last 5 messages (recency protection)

The consolidation algorithm uses role-sampled summarization: it selects the first user message, last user message, last assistant message, and remaining messages until a word budget (~384 words) is exhausted. A reentrancy guard using `atomic.Bool` with CAS prevents nested consolidation calls.

### 8.7 Model Arbitrage (`pkg/arbitrage/`)

Arbitrage automatically selects the cheapest model capable of handling a given task. It classifies task complexity via keyword analysis (10 trivial indicators, 14 complex indicators, 42 code complexity signals), estimates token usage, and compares pricing across models.

For complex tasks, it enforces a minimum context window requirement (>64K tokens). The arbitrage decision triggers when switching saves more than a configurable threshold proportion of current cost.

### 8.8 OS-Native Keychain (`pkg/keychain/`)

The keychain package provides a uniform interface across three platforms:

- **Linux** — D-Bus Secret Service with `pass` CLI fallback.
- **macOS** — `/usr/bin/security` CLI.
- **Windows** — Windows Credential Manager via `advapi32.dll`.

Input validation prevents command injection: Linux service names must match `[a-z0-9-]+`, and macOS/Windows names must match `[a-z]+`. The Windows implementation uses `unsafe.Pointer` for Win32 API calls, with careful struct mirroring for the `CREDENTIAL` type.

### 8.9 Code Intelligence (`internal/codeintel/`)

CodeIntel provides project-level intelligence: parsing source files, building import graphs, indexing symbols, and scoring file relevance to tasks. It supports Go (using `go/ast` for full-fidelity parsing), TypeScript, Python, and Rust (using compiled regex patterns).

The relevance scoring algorithm uses six factors:
1. Direct mention in task description: +10.0
2. Direct import/imported-by relationship: +5.0
3. Symbol referenced in task: +7.0
4. Same package: +3.0
5. Transitive dependency with depth decay: +2.0/(depth+2)
6. CamelCase and snake_case decomposition for identifier matching.

### 8.10 Token Estimation (`internal/tokens/`)

The token estimator uses tiktoken-go for OpenAI models and a rune-based fallback for others. EMA (Exponential Moving Average) calibration continuously corrects estimates against actual API response usage:

```go
ratio = actual / estimated
newFactor = alpha * ratio + (1 - alpha) * oldFactor
```

This uses lock-free atomic CAS for thread-safe concurrent updates, with the correction factor clamped to [0.1, 10.0] to prevent extreme values. The estimation accounts for per-message role overhead (~4 tokens), tool call input JSON, and tool call names.

---

## 9. Configuration System

M31 Autonomous's configuration system is a meticulously engineered pipeline that balances flexibility, safety, and ease of use. It operates on a 7-step loading sequence, supports hot-reload, and enforces comprehensive validation to catch misconfigurations before they cause runtime issues.

### 9.1 Loading Pipeline

The `Load(path string)` function executes seven steps in strict order:

1. **M31A_CONFIG override** — if the `M31A_CONFIG` environment variable is set, it replaces the config file path entirely.
2. **Defaults** — `DefaultConfig()` returns a `*Config` with documented sane constants. Fields not set remain at Go zero values.
3. **Global TOML decode** — parses `~/.m31a/config.toml` (or the overridden path). Missing files are silently accepted. Unknown top-level keys trigger a warning log to catch typos.
4. **`.env` auto-loading** — `LoadDotEnv()` reads a `.env` file from the current working directory with security checks (rejects world-writable files, lines capped at 4096 bytes).
5. **Environment variable overrides** — five `M31A_*` variables override TOML values for theme, default model, provider, permission mode, and compact mode.
6. **Project-level `m31a.toml`** — walks up from cwd (max 3 parent directories) looking for a project-specific config file, then merges via reflection.
7. **Variable substitution and validation** — replaces `${VAR}` patterns with environment variable values, then runs comprehensive field-level validation.

### 9.2 Complete TOML Configuration Reference

M31 Autonomous's configuration file uses TOML format with 10 top-level sections. Every field, its type, default value, and purpose is documented below.

#### `[provider]` — LLM Provider Configuration

```toml
[provider]
default = ""                        # string: primary LLM provider name
auto_fallback = false               # bool: auto-switch on provider failure
openrouter_base_url = ""            # string: custom base URL (default: https://openrouter.ai/api/v1)
zen_base_url = ""                   # string: custom base URL (default: https://opencode.ai/zen/v1)
openrouter_referer = ""             # string: HTTP-Referer header (default: https://github.com/eshanized/M31A)
openrouter_title = ""               # string: X-Title header (default: M31 Autonomous)

[provider.openrouter]
api_key = ""                        # string: resolved via priority chain (see Section 8.4)

[provider.zen]
api_key = ""                        # string: resolved via priority chain
```

#### `[model]` — Model Selection and Context Management

```toml
[model]
default = ""                        # string: default model ID
context_warning_threshold = 0.80    # float64 [0.0-1.0]: fraction of context window before warning
show_thinking_by_default = false    # bool: auto-expand thinking/reasoning blocks
auto_collapse_tools = false         # bool: auto-collapse tool cards after completion
auto_arbitrage = false              # bool: auto-suggest cheaper models for simple tasks
arbitrage_threshold = 0.0           # float64 [0.0-1.0]: cost ratio triggering model switch
default_context_length = 128000     # int: fallback context length when provider omits it
token_ema_alpha = 0.30              # float64 [0.0-1.0]: EMA calibration rate (0 = disabled)
```

#### `[ui]` — Terminal Interface

```toml
[ui]
theme = "dark"                      # string: "dark", "light", or "auto"
compact_mode = false                # bool: reduce spacing for dense terminals
show_token_usage = false            # bool: display token count in status bar
show_cost_estimate = false          # bool: display inferred cost in status bar
max_iterations = 100                # int: tool call limit per workflow phase
leader_key = ""                     # string: chord prefix key (e.g. "ctrl+x")
leader_timeout_ms = 1000            # int: time to wait for chord (milliseconds)
sidebar_width_threshold = 120       # int: min terminal width before sidebar auto-shows
discuss_timeout = 300               # int: Q&A timeout in seconds (5 min)
thinking_max_lines = 20             # int: max lines for thinking block content
permission_modal_width = 60         # int: permission modal width in columns
sidebar_width = 42                  # int: sidebar width in columns
max_message_history = 1000          # int: max message history entries
fallback_banner_timeout_secs = 15   # int: fallback banner display duration (seconds)
default_log_lines = 20              # int: default log lines for /log command
session_list_limit = 20             # int: max sessions in resume screen
thinking_opacity = 0.6              # float64: thinking block opacity (0.0-1.0)
frecent_history_size = 100          # int: frecent history max entries
accent_color = ""                   # string: custom accent color (hex)
custom_background = ""              # string: custom background color
border_style = ""                   # string: border rendering style
bold_headers = false                # bool: bold header rendering
italic_thinking = false             # bool: italic thinking block rendering
tab_width = 0                       # int: tab display width
sidebar_position = ""               # string: "left" or "right"
sidebar_auto_show = false           # bool: auto-show sidebar on wide terminals
card_padding = 0                    # int: card padding in columns
welcome_screen = false              # bool: show welcome screen on launch
zen_mode_key = ""                   # string: key binding for zen mode
animation_speed = ""                # string: "fast", "normal", "slow", or "none"
spinner_style = ""                  # string: spinner rendering style
transition_style = ""               # string: screen transition style
breathing_effects = false           # bool: enable breathing animation effects
logo_animation = false              # bool: enable logo animation
status_bar_style = ""               # string: status bar rendering style
status_bar_position = ""            # string: status bar position
show_spinner_in_status = false      # bool: show spinner in status bar
tool_card_style = ""                # string: tool card rendering style
tool_output_max_lines = 0           # int: max lines for tool output display
syntax_highlight = false            # bool: enable syntax highlighting
toast_position = ""                 # string: toast notification position
toast_duration_secs = 0             # int: toast display duration (seconds)
toast_max_visible = 0               # int: max simultaneous toasts
```

#### `[permissions]` — Tool Execution Safety

```toml
[permissions]
default_mode = ""                   # string: "prompt", "allow", or "deny"
timeout_seconds = 300               # int: auto-deny after N seconds (0 = no timeout)

# Global permission rules (glob-based tool name matching)
[[permissions.rules]]
tool = "Bash"                       # string: tool name pattern (doublestar glob)
pattern = ""                        # string: input parameter pattern to match
risk_level = "dangerous"            # RiskLevel: "safe", "medium", "dangerous", "destructive"
action = "ask"                      # string: "allow", "deny", or "ask"

# Per-agent permission profiles
[permissions.agents.build]
default_action = "allow"            # string: default permission action for this agent
[[permissions.agents.build.rules]]
tool = "Edit"
action = "deny"
```

#### `[features]` — Workflow and Runtime Behavior

```toml
[features]
auto_backup = false                 # bool: backup files before editing
resume_on_startup = false           # bool: auto-resume last session on launch
workflow_mode = ""                  # string: "auto", "full", "fast", or "direct"
model_cache_ttl_minutes = 5         # int: model cache active TTL (minutes)
model_cache_stale_hours = 24        # int: stale model cache TTL (hours)
healthcheck_live_ms = 500           # int: latency below which provider is "live" (ms)
healthcheck_slow_ms = 2000          # int: latency below which provider is "slow" (ms)
session_id_length = 8               # int: hex chars in session IDs (valid: 4-16, 0 = default)
max_recent_models = 10              # int: max recent models remembered
session_retention_days = 30         # int: session retention period (days)
health_check_timeout_secs = 10      # int: health check request timeout (seconds)
rate_limit_backoff_secs = 120       # int: wait time after 429 response (seconds)
budget_limit_usd = 0.0              # float64: per-session USD budget limit (0 = unlimited)
```

#### `[ledger]` — Cross-Session Learning

```toml
[ledger]
enabled = false                     # bool: write cross-session learning ledger
max_entries = 0                     # int: maximum ledger entries (0 = unlimited)
```

#### `[tools]` — Tool Execution Parameters

```toml
[tools]
max_glob_results = 1000             # int: glob tool result cap
max_grep_results = 100              # int: grep tool result cap
bash_kill_grace_secs = 5            # int: grace period before SIGKILL (seconds)
max_backups_per_file = 10           # int: backup rotation limit per file
webfetch_max_redirects = 5          # int: HTTP redirect follow limit
webfetch_user_agent = "M31 Autonomous/dev"    # string: User-Agent header for WebFetch
skip_dirs = [                       # []string: directories to skip in file operations
  "node_modules", "vendor", ".next", "dist", "build",
  "target", ".venv", "venv", "__pycache__"
]
websearch_base_url = "https://search.sagibo.net"  # string: WebSearch API base URL
websearch_enabled = true            # bool: enable WebSearch tool
```

#### `[agents]` — Per-Phase Model Overrides

```toml
[agents]
default = ""                        # string: model override for all phases
plan = ""                           # string: model override for Plan phase
execute = ""                        # string: model override for Execute phase
verify = ""                         # string: model override for Verify phase
ship = ""                           # string: model override for Ship phase
discuss = ""                        # string: model override for Discuss phase
```

#### `[git]` — Commit Conventions

```toml
[git]
commit_prefix = "feat"              # string: conventional commit type for execute phase
fix_prefix = "fix"                  # string: prefix for fix-phase commits
ship_prefix = "chore"               # string: prefix for ship-phase commits
user_name = "M31 Autonomous"                  # string: git user name for M31 Autonomous commits
user_email = "m31a@local"           # string: git user email for M31 Autonomous commits
```

#### `[verify]` — Build and Test Overrides

```toml
[verify]
build_command = ""                  # string: custom build command (empty = auto-detect by project type)
test_command = ""                   # string: custom test command (empty = auto-detect by project type)
```

When both commands are empty, the verify phase falls back to project-type-specific auto-detection: `go build ./...` and `go test ./...` for Go, `npm run build` and `npm test` for Node.js, `python -m py_compile` and `pytest` for Python. If only one command is configured, the other still uses auto-detection.

### 9.3 Environment Variable Reference

M31 Autonomous reads the following environment variables:

| Variable | Overrides | Purpose |
|----------|-----------|---------|
| `M31A_CONFIG` | Config file path | Override `~/.m31a/config.toml` location |
| `M31A_THEME` | `ui.theme` | Force a specific theme |
| `M31A_DEFAULT_MODEL` | `model.default` | Override default model |
| `M31A_PROVIDER` | `provider.default` | Override default provider |
| `M31A_PERMISSION_MODE` | `permissions.default_mode` | Override permission behavior |
| `M31A_COMPACT` | `ui.compact_mode` | Enable compact mode (`"true"` or `"1"`) |
| `M31A_OPENROUTER_API_KEY` | OpenRouter API key | Highest-priority API key source |
| `M31A_ZEN_API_KEY` | Zen API key | Highest-priority API key source |
| `M31A_LOG_LEVEL` | Log verbosity | `debug`, `info`, `warn`, or `error` |
| `M31A_LOG_FORMAT` | Log output format | `json` or `text` |
| `OPENROUTER_API_KEY` | OpenRouter API key | Fallback after M31A_OPENROUTER_API_KEY |
| `ZEN_API_KEY` | Zen API key | Fallback after M31A_ZEN_API_KEY |
| `COLORTERM` | Terminal capability | Truecolor detection (`truecolor` or `24bit`) |
| `TERM` | Terminal type | 256-color detection |

### 9.4 API Key Resolution Priority Chain

API keys are resolved through a four-tier priority chain per provider:

1. **M31 Autonomous-prefixed env var** (highest) — `M31A_OPENROUTER_API_KEY` or `M31A_ZEN_API_KEY`
2. **Standard env var** — `OPENROUTER_API_KEY` or `ZEN_API_KEY`
3. **OS keychain** — `kc.Get("openrouter")` or `kc.Get("zen")` (Linux D-Bus, macOS Keychain, Windows Credential Manager)
4. **Config file** (lowest) — `provider.openrouter.api_key` or `provider.zen.api_key`

On save (`SaveWithKeychain`), the system attempts to store keys in the OS keychain. If successful, keys are cleared from the TOML file. If the keychain is unavailable, keys persist in the config file as a fallback. Project-level config (`SaveProject`) always clears API keys regardless of keychain availability.

### 9.5 Reflection-Based Merge Algorithm

The `mergeConfig` function uses Go's `reflect` package for recursive struct merging. The merge rules depend on field type:

| Type | Merge Rule |
|------|-----------|
| `string` | Overlay non-empty string wins |
| `bool` | If key is in the `defined` set (explicitly set in TOML), always use overlay value (even `false`). Otherwise, only overwrite if overlay is `true`. |
| `int` | Overlay non-zero wins |
| `float64` | Overlay non-zero wins |
| `slice` | Overlay non-nil and non-empty fully replaces base |
| `map` | Overlay entries merged into base (additive) |
| `struct` | Recursive merge |

The `defined` set is the critical innovation here. It is built from TOML metadata keys during decode, enabling the distinction between "user explicitly set `auto_backup = false`" versus "user omitted the field entirely." Without this, it would be impossible to distinguish a deliberate `false` from a Go zero value, since both are identical in Go's type system.

### 9.6 .env File Handling

The `.env` loader (`LoadDotEnv`) reads environment variable definitions from a `.env` file in the current working directory:

- **Guarded by `sync.Once`** — called from both `main.go` and the config loader, but only executes once.
- **Security check** — rejects files with group or world-writable permissions (`perm & 0o022 != 0`).
- **Line limit** — skips lines longer than 4096 bytes.
- **Comment and blank line support** — lines starting with `#` are ignored.
- **Quoted value support** — strips surrounding `"` or `'` from values.
- **No override** — does not override already-set environment variables.

The `.env` file must be loaded before logger initialization because `os.Setenv` is not goroutine-safe.

### 9.7 Hot-Reload Mechanism

Configuration changes are detected and applied in real-time through two mechanisms:

**Primary: fsnotify watcher.** `WatchConfig()` creates an `fsnotify.Watcher` that monitors the parent directory of the config file. Events are filtered to only the target file and debounced with a 50ms `time.AfterFunc`. Write, Create, and Rename operations trigger a reload.

**Fallback: polling.** If fsnotify is unavailable or fails, a 5-second polling loop compares `os.Stat().ModTime()` against the last known modification time.

Reload messages are delivered via a channel with a 100ms retry window. The system never silently drops messages — if the receiver is busy, it blocks until delivered or the context is cancelled.

### 9.8 Project Context Files

M31 Autonomous supports project-level context files that inject domain knowledge into the workflow:

- **`AGENTS.md`** — highest priority, checked in cwd.
- **`.m31a/agents.md`** — alternative location.
- **`MEMORY.md`** — cross-session memory file, written by the ship phase.

These files are capped at 8KB and injected into the LLM's system prompt as `## Project Context`. The ship phase appends session learnings (goal, completed tasks, failed tasks with root causes, file patterns, duration) to `MEMORY.md`, creating a persistent learning loop across sessions.

### 9.9 Validation

The validation layer performs field-level type and range checks:

| Field | Rule |
|-------|------|
| `provider.default` | Must be non-empty when `auto_fallback` is true |
| `model.context_warning_threshold` | `0.0 <= x <= 1.0` |
| `model.arbitrage_threshold` | `0.0 <= x <= 1.0` |
| `model.default_context_length` | `>= 0` |
| `model.token_ema_alpha` | `0.0 <= x <= 1.0` |
| `ui.theme` | Must be `"dark"`, `"light"`, or `"auto"` |
| `ui.max_iterations` | `>= 0` |
| `ui.leader_timeout_ms` | `>= 0` |
| `permissions.default_mode` | `""`, `"prompt"`, `"allow"`, or `"deny"` |
| `permissions.timeout_seconds` | `>= 0` |
| `features.healthcheck_slow_ms` | `>= healthcheck_live_ms` |
| `features.session_id_length` | `0` or `4-16` |
| `tools.*_results` | `>= 0` |

All validation errors are collected, joined, and returned with the `ErrValidation` sentinel. Post-validation fixup applies the default EMA alpha (0.30) if the user sets it to 0, since zero silently disables calibration.

### 9.10 Unknown Key Detection

The config loader caches a set of known top-level TOML keys (`provider`, `model`, `ui`, `permissions`, `features`, `tools`, `git`, `ledger`, `ghost`, `agents`, `openrouter`, `zen`). Any unrecognized key in the config file triggers a warning log message, helping users catch typos early.

---

## 10. Error Handling

M31 Autonomous uses a dual error matching strategy:

**Tier 1 — Sentinel errors.** 22+ sentinel errors defined in `internal/errors/errors.go` with specific user messages. These cover provider failures, rate limiting, invalid keys, context overflow, session corruption, circular dependencies, and more.

**Tier 2 — Pattern matching.** String-based matching for unwrapped errors: connection refused, context canceled, EOF, i/o timeout, TLS errors, and HTTP status codes via regex (`\b401\b`, `\b429\b`, `\b503\b`).

The `UserMessage(error)` function maps errors to user-friendly, actionable strings that appear in notification banners. This separation of error detection and error presentation keeps the business logic clean while providing excellent user experience.

---

## 11. Slash Commands Reference

M31 Autonomous provides 47 slash commands organized into seven categories. Each command is processed through a registry that parses input, runs the handler, and routes the result — whether that's navigating to a screen, displaying a message, confirming a destructive action, or loading a session. Unknown commands trigger a "Did you mean?" suggestion via Levenshtein distance.

### 11.1 Core Commands

**`/help`** — Opens the help overlay screen listing all available commands, their descriptions, and associated keybindings. The help screen uses a scrollable viewport and adapts to the current terminal size. Use this when you're unsure what commands exist or what a shortcut does.

**`/clear`** — Wipes the conversation history from the current session. Requires explicit confirmation (y/n) to prevent accidental data loss. Useful when you want to start a fresh conversation without creating a new session — for example, after a failed workflow attempt where the context has become cluttered with error messages.

**`/status`** — Displays a snapshot of the current session: session ID, active provider, model in use, current workflow phase, and total message count. Handy for quick diagnostics when something feels off — you can verify which model is responding or confirm which phase the workflow is in.

**`/reset`** — Returns M31 Autonomous to its first-run state, prompting for API key setup. Requires confirmation. This is the nuclear option: it clears session data and reinitializes the setup wizard. Use it when you've changed API keys, switched providers, or want a completely clean slate.

**`/quit`** — Exits the application cleanly. Equivalent to pressing `Ctrl+C` twice. Deferred cleanup (session saves, log flushes) executes before exit.

**`/undo`** — Shows the latest checkpoint: the phase, timestamp, message count, and task count at the time of the snapshot. M31 Autonomous maintains up to 2 checkpoints (FIFO), so you can see what state you can roll back to. Useful before attempting risky operations.

**`/history`** — Displays the top 20 frecent prompts from your history, scored by a frecency algorithm that combines frequency and recency. If you frequently ask similar questions (e.g., "explain this function"), this command surfaces them quickly. You can also type `/` in the REPL to trigger autocomplete from history.

**`/health`** — Triggers a health check against the active provider. The check hits the provider's API endpoint and classifies latency as live (<500ms), slow (<2s), or degraded (>2s). Use this when responses are slow or failing to diagnose whether the issue is provider-side.

**`/tools`** — Lists all 15 registered tools with their descriptions and risk levels. Useful for understanding what capabilities the agent has in the current session, especially after configuration changes that may have enabled or disabled tools.

**`/copy-error`** — Copies the last error message to the system clipboard. Designed for reporting issues: when the agent encounters an error, you can quickly copy the full error text and paste it into a bug report or support channel.

### 11.2 Configuration Commands

**`/settings`** — Opens the 6-tab interactive settings editor covering provider, model, UI, permissions, features, and tools. Changes are applied immediately and persisted to `~/.m31a/config.toml`. This is the primary way to customize M31 Autonomous's behavior without editing TOML files manually.

**`/config`** — Opens the full configuration viewer/editor, showing the raw TOML structure. More powerful than `/settings` for advanced users who want to see and edit every field, including those not exposed in the settings tabs (git conventions, verify commands, agent overrides).

**`/theme`** — Switches the color theme. Accepts a mode (`dark`, `light`, `auto`) or a specific palette name (`catppuccin`, `nord`, `tokyo`, `gruvbox`, `rose`, `dracula`, `solarized`, `monochrome`). Without arguments, cycles through dark -> light -> auto. The theme change propagates to all 28+ sub-models instantly.

**`/cost`** — Toggles the cost estimate display in the status bar footer. When enabled, shows the inferred cost of the current session based on token usage and model pricing. Useful for keeping tabs on API spending during long sessions.

**`/log`** — Displays recent log entries from `~/.m31a/m31a.log`. Accepts an optional count argument (default: 20). Logs are written in JSON format with daily rotation and 7-day retention. Use this to debug issues — the log captures provider requests, config reloads, permission decisions, and tool executions.

**`/key`** — Shows the API key status for OpenRouter and Zen providers. Displays whether each key is configured (via env var, keychain, or config file) without revealing the actual key value. Essential for verifying that keychain integration is working correctly.

**`/tokens`** — Estimates the token count for all messages in the current session. Uses tiktoken-go for OpenAI models or a rune-based fallback for others. The estimate includes per-message role overhead and tool call JSON. Useful for gauging how close you are to the context window limit.

### 11.3 AI/Model Commands

**`/compress`** — Triggers AutoDream context consolidation, compressing older messages into a compact "memory segment." Protected messages (system prompts, tool calls, recent messages, plan specs) are preserved. Use this when you see the context warning banner or when responses start degrading due to a full context window. Has a 60-second cooldown between invocations.

**`/memory`** — Manages the AutoDream consolidation system. Subcommands: `view` (show current memory state), `pause` (disable automatic consolidation), `resume` (re-enable), `revert` (undo last consolidation). Useful for fine-tuning when and how context gets compressed.

**`/optimize`** — Consults the model arbitrage engine to suggest a cheaper model for the current task. Analyzes task complexity via keyword scoring, estimates token usage, and compares pricing across available models. Returns a recommendation with estimated savings. Use this when you want to reduce costs without sacrificing quality.

**`/model` or `/models`** — Opens the fuzzy model selector with per-token cost comparison. You can search models by name, compare input/output pricing, and see context window sizes. The selector supports keyboard navigation and instant switching. Use this when you want manual control over model selection rather than relying on auto-selection.

**`/fallback`** — Shows the current fallback status or manually switches to an alternative provider. When the active provider is degraded, this command displays which providers are healthy and allows you to trigger a switch. With `auto_fallback` enabled, this happens automatically, but this command gives you manual override.

**`/provider`** — Delegates to the fallback system to show or switch the active provider. Similar to `/fallback` but focused on provider identity rather than health status. Useful when you want to explicitly choose between OpenRouter and Zen.

### 11.4 Git Commands

**`/diff`** — Shows the git diff for the current project. Without arguments, displays unstaged changes. Accepts optional arguments to show staged changes, or diffs between specific refs (e.g., `/diff HEAD~3`). The diff is colorized in the diff viewer with addition/deletion stats. Use this to review what the agent has changed before committing.

**`/rollback`** — Opens the commit time machine, browsing the chain of commits made by M31 Autonomous sessions. Shows each commit with its hash, message, and diff preview (capped at 50K characters). You can soft-reset to any commit, which preserves staged changes, or hard-reset which creates a backup branch first. Essential for undoing agent mistakes without losing work.

**`/bisect`** — Starts an interactive git bisect session. You provide a "good" commit (where things worked) and a "bad" commit (where they broke), and M31 Autonomous runs a check function at each bisection step to automatically identify the offending commit. The results include the commit hash and its diff. Powerful for diagnosing when a regression was introduced.

### 11.5 Session Commands

**`/sessions`** — Lists recent sessions for the current project with metadata: ID, label, model, message count, and timestamp. Sessions are project-local (stored in `.m31a/`), so you only see sessions relevant to the current codebase.

**`/export`** — Exports the current session to a file. Supports two formats: markdown (human-readable with role headers) and JSON (full session data with `json.MarshalIndent`). The file is written atomically to the specified path. Use this for archival, sharing session transcripts, or importing into other tools.

**`/save`** — Manually saves the current session to disk. M31 Autonomous auto-saves on every message, but this command forces an immediate save — useful before closing your terminal or when you want to ensure a checkpoint is captured.

**`/goal`** — Sets or displays the session goal. Without arguments, opens the full-screen goal input. With arguments (e.g., `/goal refactor auth middleware`), sets the goal directly. The goal is persisted in `STATE.md` and used as context for all workflow phases.

**`/resume`** — Opens the session browser for resuming past sessions. Shows sessions from the current project with their goals, phases, and timestamps. Selecting a session restores its full message history and workflow state, allowing you to continue exactly where you left off.

**`/ledger`** — Opens the cross-session learning ledger browser. Without arguments, shows aggregate statistics: total sessions, average cost, top failures, and project type distribution. The ledger records metadata from every shipped session to `~/.m31a/LEDGER.md`, enabling the agent to learn patterns across sessions.

### 11.6 Workflow Commands

**`/new`** — Starts a new workflow by opening the full-screen goal input. This is the primary entry point for the six-phase workflow. After entering a goal, you'll be prompted to select models for planning and coding phases, then the workflow begins.

**`/workflow`** — Displays the current workflow phase, status, and progress. Shows which phases have been completed, which is active, and what's pending. Use this to orient yourself when resuming a session or checking on a long-running workflow.

**`/plan`** — Transitions to the plan phase. The LLM generates a structured implementation plan with tasks, dependencies, and a verification strategy. You can review the plan, request refinements, or approve it to proceed to execution.

**`/refine`** — Submits feedback on the current plan to trigger a refinement iteration. The feedback is injected into the next plan generation attempt, which also receives the previous plan for context. M31 Autonomous supports up to 5 refinement iterations with a maximum of 3 plan-discuss oscillation cycles.

**`/execute`** — Transitions to the execute phase. Tasks are scheduled topologically (Kahn's algorithm) and executed with bounded parallelism. Each task goes through the self-heal loop with up to 2 retry attempts. You can monitor progress in the execute screen.

**`/verify`** — Transitions to the verify phase. Runs file existence checks, semantic validation (git diff), build verification, and test execution. When verification fails, the agent attempts self-healing, falls back to git bisect, and retries with diagnostic context.

**`/ship`** — Transitions to the ship phase. Creates the final git commit, writes a ledger entry, archives the session, and generates a demonstration walkthrough. The commit is scoped to task files only.

**`/phase`** — Shows the current phase or transitions to a specified phase. Acts as a shorthand for the individual phase commands. Accepts phase names like `plan`, `execute`, `verify`, `ship`.

**`/pause`** — Pauses the current workflow. The workflow state (goal, phase, questions) is persisted to `STATE.md` so it survives application restarts. Use this when you need to step away and want to resume later without losing progress.

**`/resume-task`** — Resumes a previously paused workflow. Restores the persisted state and continues from the last active phase. The workflow picks up exactly where it left off, including any in-progress task context.

**`/agent-mode`** — Toggles autonomous agent mode. When enabled, the agent runs the full workflow without pausing for human input at each phase transition. Useful for straightforward tasks where you trust the agent's judgment. Can be toggled on/off at any time.

**`/metrics`** — Opens the session analytics dashboard. Displays token usage over time, cost breakdown by model, tool execution statistics, and workflow phase durations. Helps you understand where time and money are being spent.

**`/dashboard`** — Opens the workflow pipeline overview, showing all six phases as a visual pipeline with their current status (pending, active, completed, failed). Provides a high-level view of workflow progress.

**`/themes`** — Opens the theme picker with live preview. Browse all 10 registered palettes (Midnight, Daylight, Catppuccin, Nord, Tokyo Night, Gruvbox, Rose Pine, Dracula, Solarized, Monochrome) and see how they look before applying.

**`/notifications`** — Opens the notification center, showing a scrollable history of all toast notifications (success, error, warning, info) with timestamps. Useful for reviewing what happened during a workflow without scrolling through the chat.

**`/files`** — Opens the file explorer, showing the project's file tree with git status indicators. Navigate with j/k, expand/collapse directories, and view file diffs on demand. A keyboard-driven alternative to the sidebar.

**`/ghost`** — Ghost write files (V1.1 feature). Opens a file selector for the ghost write mode, which generates structured diffs without touching the TUI. Designed for headless/automated workflows.

### 11.7 Subagent Commands

**`/agent`** — Spawns a parallel subagent or lists active ones. Without arguments, shows currently running subagents with their IDs, status, and progress. With a task description (e.g., `/agent write tests for the auth package`), spawns a new subagent that runs in an isolated git worktree with its own tool dispatcher. Subagents can run in background (returns immediately) or foreground (blocks until completion) mode.

**`/agent-cancel`** — Cancels a running subagent by its ID or "all" to cancel every active subagent. Sends a cancellation signal that propagates through the subagent's goroutine, triggering cleanup (worktree removal, branch deletion). Use this when a subagent is stuck or no longer needed.

### 11.8 Shell Command Prefix

The `!` prefix executes shell commands directly without going through the LLM. For example, `!ls -la` runs `ls -la` in the project directory. Commands run with a 30-second timeout and output capped at 4000 characters. This is useful for quick file inspection, checking git status, or running build commands without consuming API tokens. The output is displayed in the REPL as a system message.

### 11.9 Command Processing Flow

1. User input is checked for `!` prefix (shell command) or `/` prefix (slash command).
2. Shell commands execute directly via `exec.CommandContext` with a 30-second timeout.
3. Slash commands go to the command registry, which parses the input, resolves arguments, and runs the registered handler.
4. The handler returns a `CommandResult` that can contain: a confirmation prompt (y/n), a message to display, a screen to navigate to, a session to load, a workflow to resume, or a `tea.Cmd` callback.
5. `processCommandResult()` routes the result to the appropriate handler.
6. Unknown commands trigger a "Did you mean?" suggestion via Levenshtein distance computation against all registered command names.

---

## 12. Performance Optimizations

M31 Autonomous employs numerous performance optimizations:

- **Pre-compiled regexes** at package level for plan parsing, tool call extraction, and error matching.
- **Pre-parsed tool schemas** (`ParametersParsed` field) to avoid repeated `json.Unmarshal` during request building.
- **Cached tool definitions** built once per session.
- **Cached base prompt** via `sync.Once`.
- **Cached project state and parsed plan** to avoid redundant recomputation.
- **Shared HTTP transport** across all providers, reusing connection pools and TLS session caches.
- **Pre-computed reasoning field paths** avoiding per-chunk string splitting.
- **Single-pass HTML processing** in WebFetch (compute lowercase once, reuse across tag operations).
- **Concurrent git status+numstat** via goroutines with WaitGroup.
- **Mtime-based ledger stats cache** avoiding recomputation when the file hasn't changed.
- **`singleflight.Group`** for model catalog refreshes.

---

## 13. Security Analysis

### 12.1 Threat Model

M31 Autonomous executes shell commands and file operations on behalf of the user. The threat model addresses:

1. **Unintended file modifications** — path traversal guards, permission gating, atomic writes.
2. **Shell injection** — non-interactive environment, stdin closure, process group management.
3. **SSRF attacks** — DNS pinning, TOCTOU prevention, private IP blocking, post-connect verification.
4. **API key exposure** — OS keychain storage, masking in error messages, no plaintext on disk.
5. **Context window attacks** — output capping on all tools and streams.
6. **Supply chain** — static binary, no CGO, no telemetry.

### 12.2 Known Security Concerns

Three known security gaps exist:

1. **SEC-01**: No ReDoS protection in pure-Go grep (mitigated by regex pattern detection and ripgrep availability).
2. **SEC-02**: WebSearch missing DNS cache (defense-in-depth gap vs WebFetch).
3. **SEC-03**: Incomplete HTML entity decoding in WebFetch.

---

## 14. Testing Strategy

M31 Autonomous uses Go's standard `testing` package with table-driven tests and `t.Parallel()`. Coverage targets are 75% overall and 90% for critical packages.

| Package | Coverage | Notes |
|---------|----------|-------|
| `pkg/taskrunner` | 89.9% | Critical for execution reliability |
| `pkg/bisect` | 91.3% | High coverage with test doubles |
| `pkg/rollback` | 89.1% | Safety-critical operations |
| `internal/tools` | 80%+ | Security-sensitive code |
| `internal/provider` | 75%+ | Network-facing code |
| `internal/tui` | 38.6% | Known gap, improvement planned |
| `cmd/m31a` | 0.0% | Bootstrap code, difficult to unit test |

The CI pipeline runs on every push to master and on PRs, executing lint (golangci-lint + gofmt), race-enabled tests, security scanning (govulncheck), cross-platform builds (6 targets), and releases via GoReleaser.

---

## 15. Planning File System

M31 Autonomous maintains six planning files in the `.m31a/` directory that track workflow state and enable cross-session continuity:

| File | Written By | Contents |
|------|-----------|----------|
| **PROJECT.md** | Initialize, Discuss | Goal, project type, framework, discuss Q&A |
| **STATE.md** | Engine.Transition, Initialize, Verify, Ship | Current phase, status, description |
| **TASKS.md** | Plan, updated after Verify | Task list with status checkboxes |
| **plan.md** | Plan phase | Rich plan markdown content, versioned |
| **DEMONSTRATION.md** | Ship phase | LLM-generated walkthrough document |
| **MEMORY.md** | Ship phase | Cross-session learnings: goal, tasks done/failed, project type, model used, duration |

These files serve dual purposes: they provide the LLM with structured context about the current session, and they persist across sessions for the learning system. The ship phase reads `MEMORY.md` from previous sessions to inject historical context, then writes new learnings back — creating a feedback loop that improves the agent's performance over time.

---

## 16. Log System

The logging system (`internal/log/log.go`) provides structured logging with daily rotation:

- **Location:** `~/.m31a/m31a.log`
- **Format:** JSON (default) or text, controlled by `M31A_LOG_FORMAT`.
- **Level:** `M31A_LOG_LEVEL`: debug, info (default), warn, error.
- **Daily rotation:** renames `m31a.log` to `m31a.log.YYYY-MM-DD` when last modified before today.
- **7-day retention:** deletes rotated files older than 7 days.
- **Rotation failure is non-fatal** — warns to stderr and continues with append-only.
- **Singleton pattern** via `sync.Once` ensures only one logger instance exists.

---

## 17. CI/CD Pipeline

The GitHub Actions pipeline (`.github/workflows/ci.yml`) defines five jobs:

| Job | Timeout | Steps | Details |
|-----|---------|-------|---------|
| **lint** | 10 min | gofmt check, golangci-lint (5m timeout), GoReleaser validation | Uses golangci-lint-action@v9 |
| **test** | 10 min | `go test -race -coverprofile=coverage.out -covermode=atomic ./...` | Race detector enabled |
| **security** | 10 min | `govulncheck ./...` | Vulnerability scanning |
| **build** | 10 min | Cross-platform matrix: ubuntu/macos/windows x amd64/arm64 (excluding windows/arm64), `CGO_ENABLED=0` | 5 build targets |
| **release** | 15 min | GoReleaser `release --clean` + race tests | Only on tag push, depends on all 4 other jobs |

**Triggers:** push to master, PRs to master, manual dispatch. Go version: 1.25.

---

## 18. Known Issues and Tech Debt

### Bugs

| ID | Description | Impact |
|----|-------------|--------|
| BUG-01 | Flaky git status test under race detector (timing assumption) | Blocks reliable CI |
| BUG-02 | Ship phase commits unrelated files when taskFiles is empty | Data integrity risk |
| BUG-03 | Demonstration generation can exceed context window for smaller models | Runtime failure |
| BUG-04 | Inconsistent `HasUncommittedChanges` implementations (porcelain vs human-readable) | Correctness |

### Security Concerns

| ID | Description | Mitigation |
|----|-------------|------------|
| SEC-01 | No ReDoS protection in pure-Go grep | Mitigated by ripgrep availability and pattern detection |
| SEC-02 | WebSearch missing DNS cache | Defense-in-depth gap vs WebFetch |
| SEC-03 | Incomplete HTML entity decoding in WebFetch | Minor impact |

### Tech Debt

- 12 ineffectual assignments across production code
- 50+ variable shadowing instances (concentrated in main.go, ship.go, rollback.go)
- Global gitignore cache has no eviction
- FileDelete has no backup pruning (unlike FileWrite and Edit)
- Dead writes in WebFetch HTML parser
- TUI test coverage at 38.6% (improvement planned)

---

## 19. Comparative Analysis

| Capability | M31 Autonomous | Cursor | Aider | Cline |
|------------|:----:|:------:|:-----:|:-----:|
| Terminal-native | yes | no | yes | no |
| Six-phase workflow | yes | no | no | no |
| Git commit rollback chain | yes | no | partial | no |
| Cross-session learning | yes | no | no | no |
| Context consolidation | yes | no | no | no |
| Provider auto-fallback | yes | no | partial | partial |
| Static binary, no CGO | yes | no | no | no |
| Telemetry | none | yes | none | yes |
| OS keychain integration | yes | no | no | no |
| 29-screen TUI | yes | no | no | no |
| Slash commands (47) | yes | no | partial | no |
| Model cost arbitrage | yes | no | no | no |
| Git bisect automation | yes | no | no | no |
| Subagent spawning | yes | no | no | partial |

M31 Autonomous's closest competitor in the terminal-native space is Aider, which provides excellent git integration and multi-file editing but lacks the structured workflow engine, cross-session learning, and context consolidation that M31 Autonomous offers.

---

## 20. Future Work

The V1.1 roadmap includes several features:

- **Ghost mode** — headless runs producing structured diffs without TUI interaction.
- **Picture-in-picture** — second agent in a side pane for cross-review during execution.
- **Subagents** — delegated sub-tasks to specialized agents (code, test, doc) with worktree isolation.
- **Deferred tools** — queued tool calls requiring human approval for batch review.

Additional improvements under consideration:

- Increasing TUI test coverage from 38.6% to 75%.
- Adding ReDoS protection to pure-Go grep.
- Implementing backup pruning in FileDelete.
- Adding SSRF protection to WebSearch.
- Cleaning up dead code and 12 ineffectual assignments across production code.

---

## 21. Conclusion

M31 Autonomous represents a comprehensive approach to AI-assisted software engineering in the terminal. Its six-phase workflow engine provides structure without rigidity, its security model provides safety without friction, and its cross-session learning provides intelligence without complexity.

The codebase demonstrates several notable engineering decisions: the MsgEmitter pattern for decoupling the workflow engine from the TUI, the atomic CAS pattern for lock-free cost tracking, the cascading plan parser with retry loops, the self-healing execution with git bisect fallback, and the AutoDream context consolidation system.

At approximately 15-20MB for a fully static binary with zero dependencies, M31 Autonomous is both powerful and portable. It runs on any POSIX shell, stores nothing in the cloud, and learns from every session. For developers who live in the terminal and want an AI agent that owns the loop — not just an autocomplete with dangerous capabilities — M31 Autonomous is the tool to reach for.

---

## References

1. Charmbracelet. "Bubble Tea — A powerful little TUI framework." https://github.com/charmbracelet/bubbletea
2. Charmbracelet. "Lip Gloss — Declarative terminal styling." https://github.com/charmbracelet/liploss
3. Charmbracelet. "Glamour — Markdown rendering for the terminal." https://github.com/charmbracelet/glamour
4. pkoukk. "tiktoken-go — Go port of OpenAI's tiktoken." https://github.com/pkoukk/tiktoken-go
5. BurntSushi. "TOML — TOML parser for Go." https://github.com/BurntSushi/toml
6. godbus. "dbus — Go bindings for D-Bus." https://github.com/godbus/dbus
7. fsnotify. "fsnotify — Filesystem notifications for Go." https://github.com/fsnotify/fsnotify
8. bmatcuk. "doublestar — Glob pattern matching with doublestars." https://github.com/bmatcuk/doublestar
9. OpenAI. "Server-Sent Events specification." https://html.spec.whatwg.org/multipage/server-sent-events.html
10. Kahn, Arthur B. "Topological sorting of large networks." Communications of the ACM, 1962.

---
