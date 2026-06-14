# M31A — Complete Project Context

> **The terminal-native AI coding agent that ships, not just suggests.**

**Repository:** `github.com/eshanized/M31A`
**Module:** `github.com/eshanized/M31A` (Go 1.24)
**Version:** v1.0.0 (core feature complete)
**License:** MIT — Copyright (c) Eshanized
**Status:** Active development; V1.1 features on roadmap

---

## 1. What M31A Is

M31A is a terminal-based AI coding agent written entirely in Go. Unlike browser-bound assistants, it runs inside your shell and owns a **six-phase workflow end-to-end**: Initialize → Discuss → Plan → Execute → Verify → Ship. Every run ends with a verified git commit and a ledger entry. One static binary, zero telemetry, any POSIX shell.

### Key Differentiators

| Capability | M31A | Cursor | Aider | Cline |
|---|:---:|:---:|:---:|:---:|
| Terminal-native (no Electron) | **yes** | no | yes | no |
| Six-phase workflow engine | **yes** | no | no | no |
| Git commit rollback chain | **yes** | no | partial | no |
| Cross-session learning ledger | **yes** | no | no | no |
| AutoDream context consolidation | **yes** | no | no | no |
| Provider auto-fallback | **yes** | no | partial | partial |
| Static binary, no CGO | **yes** | no | no | no |
| Telemetry / phone-home | **none** | yes | none | yes |

---

## 2. Tech Stack

### Language & Runtime
- **Go 1.24** — entire codebase
- **CGO_ENABLED=0** — fully static binary, no C dependencies
- **~15-20MB** typical binary size (stripped with `-s -w` ldflags)

### Core Frameworks
- **Bubble Tea** v1.3.0 — TUI application framework (Elm architecture)
- **Lip Gloss** v1.1.0 — terminal styling/layout
- **Bubbles** v0.20.0 — pre-built TUI components (text input, viewport, spinner)
- **Glamour** v0.6.0 — markdown rendering in terminal

### Key Dependencies
- `github.com/BurntSushi/toml` v1.6.0 — TOML config parsing
- `github.com/pkoukk/tiktoken-go` v0.1.8 — token estimation for context window management
- `github.com/godbus/dbus/v5` v5.2.2 — Linux D-Bus Secret Service for API key storage
- `github.com/fsnotify/fsnotify` v1.10.1 — config file hot-reload watching
- `github.com/bmatcuk/doublestar/v4` v4.10.0 — extended glob pattern matching
- `github.com/atotto/clipboard` v0.1.4 — system clipboard access
- `golang.org/x/sync` v0.10.0 — `singleflight` for deduplicating concurrent requests

### Build & Release
- **Makefile** — build, test, lint, cross-compile, release targets
- **GoReleaser v2** — cross-platform release automation
- **golangci-lint** — static analysis (govet, staticcheck, errcheck, ineffassign, unused)
- Cross-compilation: linux/darwin/windows × amd64/arm64 (excluding windows/arm64)

---

## 3. Architecture

### System Overview

```
┌─────────────────────────────────────────────────────────────────────┐
│                          Entry Point                                │
│  `cmd/m31a/main.go` — CLI flags, config loading, provider wiring   │
└──────────────────────────────┬──────────────────────────────────────┘
                               │
                               ▼
┌─────────────────────────────────────────────────────────────────────┐
│                         TUI Layer (Bubble Tea)                      │
│  `internal/tui/` — 29 screens, page layout, screen transitions     │
│                                                                     │
│  AppState (root) → Screen enum routing → Sub-model Update/View     │
│  MsgEmitter pattern decouples workflow engine from Bubble Tea       │
└──────────────────────────────┬──────────────────────────────────────┘
                               │
                               ▼
┌─────────────────────────────────────────────────────────────────────┐
│                     Workflow Engine (6 phases)                       │
│  `internal/workflow/engine.go` — core orchestrator                  │
│                                                                     │
│  Initialize → Discuss → Plan → Execute → Verify → Ship              │
│                                                                     │
│  Prompt templates embedded via embed.FS (11 markdown files)         │
│  30+ message types for TUI communication                            │
│  Plan parser extracts tasks from LLM markdown output                │
└──────────────────────────────┬──────────────────────────────────────┘
                               │
                    ┌──────────┼──────────┐
                    ▼          ▼          ▼
┌──────────────────┐ ┌────────────────┐ ┌────────────────────────────┐
│  Provider Layer   │ │  Tool Layer    │ │  Packages (domain logic)   │
│  OpenRouter, Zen  │ │  Bash, Read,   │ │  session, ledger, rollback │
│  Fallback, SSE    │ │  Write, Glob,  │ │  bisect, taskrunner,       │
│  Health checks    │ │  Grep, Web     │ │  keychain, autodream,      │
│                   │ │  Permissions   │ │  arbitrage, history        │
└────────┬─────────┘ └───────┬────────┘ └────────────┬───────────────┘
         │                   │                        │
         ▼                   ▼                        ▼
┌─────────────────────────────────────────────────────────────────────┐
│                     Infrastructure Layer                             │
│  git/ config/ errors/ tokens/ codeintel/ fileutil/ log/             │
│  TOML config, slog logging, atomic I/O, token estimation            │
└─────────────────────────────────────────────────────────────────────┘
```

### Six-Layer Architecture

1. **TUI Layer** (`internal/tui/`) — User interaction, 29 screens, keyboard/mouse handling
2. **Workflow Engine** (`internal/workflow/`) — Six-phase orchestration, LLM streaming, plan parsing
3. **Provider Layer** (`internal/provider/`) — LLM provider abstraction with fallback
4. **Tool Layer** (`internal/tools/`) — File system/shell operations with permission control
5. **Package Layer** (`pkg/`) — Reusable domain logic (session, ledger, rollback, etc.)
6. **Infrastructure Layer** (`internal/{errors,config,tokens,codeintel,git,fileutil,log}/`) — Cross-cutting concerns

### Dependency Rule
- `pkg/` cannot import `internal/` — strict separation enforced

---

## 4. Directory Structure

```
M31A/
├── cmd/m31a/              CLI entry point (main.go)
├── internal/
│   ├── codeintel/         Project indexing, import graph, relevance scoring
│   ├── config/            TOML config loading, hot-reload, env expansion
│   ├── errors/            Sentinel errors + UserMessage() mapping
│   ├── fileutil/          Atomic file writes (temp+rename)
│   ├── git/               Shell-based git wrapper (satisfies types.GitClient)
│   ├── log/               Structured slog logging, daily rotation, 7-day retention
│   ├── provider/          LLM provider abstraction, SSE streaming, registry, fallback
│   ├── tokens/            tiktoken-go + rune fallback, EMA calibration
│   ├── tools/             Bash, FileRead, FileWrite, Glob, Grep, WebFetch, permissions
│   ├── tui/               29-screen Bubble Tea TUI (app_state, repl, streaming, themes)
│   ├── types/             Core type definitions (WorkflowPhase, Task, Message, etc.)
│   └── workflow/          Six-phase orchestration engine + 11 embedded prompt templates
├── pkg/
│   ├── arbitrage/         Model cost optimization, task complexity scoring
│   ├── autodream/         Context window consolidation (message compression)
│   ├── bisect/            Git bisect automation for finding breaking commits
│   ├── history/           Frecent prompt history (frecency scoring)
│   ├── keychain/          OS-native secure API key storage (Linux/macOS/Windows)
│   ├── ledger/            Cross-session learning records (markdown tables)
│   ├── rollback/          Git commit chain management, soft/hard/safe reset
│   ├── session/           Session lifecycle, checkpoints, planning file I/O
│   └── taskrunner/        Task dependency resolution (Kahn's) + bounded parallelism
├── docs/                  11 documentation files
├── scripts/               verify_v1.sh acceptance suite
├── .github/workflows/     CI/CD pipeline
├── go.mod / go.sum        Dependencies
├── Makefile               Build/test/lint targets
├── .goreleaser.yaml       Release config
└── install.sh             One-liner installer
```

---

## 5. Core Workflow Engine

The six-phase workflow is the heart of M31A. Each phase is implemented in a separate file under `internal/workflow/`.

### Phase 1: Initialize (`initialize.go`)
- Detects project type, framework, language
- Initializes git repo if needed
- Creates `.m31a/` planning directory
- Sets up PROJECT.md, STATE.md, TASKS.md

### Phase 2: Discuss (`discuss.go`)
- Asks clarifying questions via LLM streaming
- Gathers requirements before planning
- Streams responses to TUI in real-time

### Phase 3: Plan (`plan.go`, `plan_parser.go`)
- Generates implementation plan with tasks
- Markdown parser extracts: title, review notes, questions, proposed changes, tasks
- Supports plan refinement with retry logic (max 3 retries, max 5 refinements)
- Classifies prompt complexity: trivial → simple → moderate → complex

### Phase 4: Execute (`execute.go`)
- Task runner schedules tasks topologically (Kahn's algorithm)
- Dispatches tools (Bash, FileRead, FileWrite, etc.)
- Self-heal loop: max 2 attempts for recoverable failures
- Bounded parallelism via semaphore

### Phase 5: Verify (`verify.go`)
- File existence checks
- Syntax validation
- Test execution
- Smart file truncation for LLM context

### Phase 6: Ship (`ship.go`)
- Creates final git commit
- Writes ledger entry (cross-session learning)
- Archives session
- Generates demonstration summary

### Workflow Modes
- **auto** — classify and choose automatically (default)
- **full** — all 6 phases
- **fast** — skip Plan: Init→Discuss→Exec→Verify→Ship
- **direct** — skip Discuss, Plan, Verify: Init→Exec→Ship

---

## 6. TUI System

### 29 Screens
Built with Bubble Tea (Elm architecture). Screen routing via `AppState.screen` enum.

**Core Screens:**
- `repl_model` — Chat viewport, message history, @-mentions, slash commands
- `goalinput_model` — Full-screen goal entry
- `phasemodelpicker` — Dual-model picker (planning vs coding)
- `plan_model` — Plan review and refinement
- `discuss_model` — Clarifying questions Q&A
- `execute_model` — Task execution progress
- `verify_model` — Verification results
- `ship_model` — Ship summary

**Utility Screens:**
- `modelselector_model` — Fuzzy model search with per-token cost comparison
- `settings_model` — 6-tab settings editor
- `sidebar_model` — Git status, file tree
- `cmdpalette` — Command palette (fuzzy search)
- `dashboard_model` — Workflow pipeline overview
- `ledger_model` — Learning ledger browser
- `rollback_model` — Commit time machine
- `diff_model` — Diff viewer
- `metrics_model` — Session analytics
- `bisect_model` — Git bisect interactive
- `fileexplorer_model` — File tree browser
- `help_model` — Keybinding help overlay
- `themepicker_model` — Theme browser/preview
- `firstrun_model` — API key setup wizard
- `resume_model` — Session browser
- `notification_model` — Notification history
- `ghostpicker_model` / `ghostoutput_model` — Ghost write (V1.1 feature)
- `confirmquit_model` — Confirm quit dialog
- `subagents_model` — Sub-agent management

### Theme System
- Dark mode (default), Light mode, Auto mode
- Full theme definitions in `internal/tui/theme/`

### Key Bindings
- `Enter` — Send message
- `Ctrl+C` — Cancel stream (2nd press exits)
- `Esc` — Close modals
- `y/a/n/e` — Permission modal: allow/allow always/deny/exit

---

## 7. Provider System

### Supported Providers
1. **OpenRouter** — Primary LLM gateway (`https://openrouter.ai/api/v1`)
2. **Zen** — Secondary provider (`https://opencode.ai/zen/v1`)

### Key Components
- **LLMProvider Interface** (`internal/provider/interface.go`) — `Chat()`, `ChatStream()`, `Models()`
- **BaseClient** (`internal/provider/base_client.go`) — Shared HTTP transport, cache, health, cost estimation
- **Registry** (`internal/provider/registry.go`) — Thread-safe multi-provider management
- **Fallback** (`internal/provider/fallback.go`) — Parallel health checks, automatic failover on 429/503
- **SSE Parser** (`internal/provider/sse.go`) — Streaming response parsing with watchdog timeout

### API Key Resolution Order
1. Environment variable: `M31A_OPENROUTER_API_KEY` / `M31A_ZEN_API_KEY`
2. Standard fallback: `OPENROUTER_API_KEY` / `ZEN_API_KEY`
3. OS keychain: `m31a/openrouter` or `m31a/zen`
4. Config file: `provider.openrouter.api_key` / `provider.zen.api_key`

### Model Features
- Model catalog with TTL cache (5 min active, 24h stale fallback)
- Per-token cost comparison
- Context warning at configurable threshold (default 80%)
- Model arbitrage: automatic switching to cheapest capable model

---

## 8. Tool System

### 5 Core Tools
1. **Bash** — Shell command execution (dangerous risk, timeout, output capping)
2. **FileRead** — Read files with size limits (50MB max)
3. **FileWrite** — Write files with atomic operations (temp+rename)
4. **Glob** — File pattern matching (doublestar, 1000 result limit)
5. **Grep** — Content search (ripgrep when available, pure-Go fallback)

### Additional Tools
- **WebFetch** — URL fetching with SSRF protection (DNS pinning, private IP blocking)
- **WebSearch** — Privacy-respecting search (SearXNG or Brave Search)
- **AskUser** — Interactive user prompts
- **Agent** — Sub-agent spawning

### Security Model
- **Permission Gating**: Per-request channels with timeout (default 300s)
- **Risk Levels**: safe, medium, dangerous, destructive
- **Rate Limiting**: Token bucket via channel
- **Path Traversal Guards**: Symlink resolution + workDir prefix check
- **Output Capping**: MaxToolOutputChars (10,000) / BashOutputLimit (50,000)
- **SSRF Protection**: DNS pinning, TOCTOU prevention, redirect checking
- **Process Lifecycle**: SIGINT/SIGKILL grace period, pipe cleanup
- **Agent Configs**: Per-agent permission rules

---

## 9. Key Packages (pkg/)

### `pkg/session/` — Session Lifecycle
- Session CRUD, checkpoint/restore, planning file I/O
- Persists to `<workDir>/.m31a/session.json`, `messages.json`, `checkpoint.json`
- Max 2 checkpoints for undo/rollback support
- Auto-adds `.m31a/` to project `.gitignore`

### `pkg/ledger/` — Cross-Session Learning
- Markdown table persistence
- Stats computation, keyword search
- Append-only writes
- Stores patterns, failures, recoveries across sessions

### `pkg/rollback/` — Git Commit Chain
- `Chain()` — Lists commits with diffs
- `SoftReset()`, `HardReset()`, `SafeReset()` — With auto-stash and backup branches
- Backup naming: `m31a/rollback-backup-<unix-timestamp>`

### `pkg/bisect/` — Git Bisect Wrapper
- Automated bisect with user-provided check function
- 30s timeout fallback when git wrapper not wired
- Interface: `GitRunner` for test double injection

### `pkg/taskrunner/` — Task Execution
- Kahn's algorithm for topological sort
- Bounded parallelism (default: 4 max concurrent tasks)
- Retry with backoff

### `pkg/keychain/` — OS-Native Secure Storage
- **Linux**: D-Bus Secret Service + `pass` CLI fallback
- **macOS**: `/usr/bin/security` CLI
- **Windows**: Windows Credential Manager
- Service names: `m31a/openrouter`, `m31a/zen`
- Input validation: `[a-z0-9-]+` (Linux) or `[a-z]+` (macOS/Windows)

### `pkg/autodream/` — Context Consolidation
- Compresses old messages when context window fills up
- Protected message detection (system prompts, recent messages)
- Role-sampled summarization
- Threshold: 60% context usage triggers consolidation

### `pkg/arbitrage/` — Model Cost Optimization
- Keyword-based task complexity scoring
- Token estimation per model
- Recommends cheapest model meeting capability threshold

### `pkg/history/` — Frecent Prompt History
- Frecency scoring (recency × frequency)
- JSON persistence, search

---

## 10. Configuration

### File Locations
- **Global**: `~/.m31a/config.toml` (override with `M31A_CONFIG`)
- **Project**: `m31a.toml` in cwd (walks up 3 parent directories)

### Config Structure
```toml
[provider]
default = "openrouter"
auto_fallback = true

[model]
default = ""
auto_arbitrage = false
arbitrage_threshold = 0.1

[ui]
theme = "dark"
compact_mode = false
show_cost_estimate = true

[permissions]
default_mode = "ask"
timeout_seconds = 300

[features]
autodream_enabled = true
auto_backup = true
resume_on_startup = false
```

### Environment Variables
- `M31A_OPENROUTER_API_KEY` / `OPENROUTER_API_KEY`
- `M31A_ZEN_API_KEY` / `ZEN_API_KEY`
- `M31A_CONFIG` — Override config file path
- `M31A_LOG_LEVEL` — debug/info/warn/error
- `M31A_LOG_FORMAT` — json/text
- `M31A_THEME` — Override UI theme
- `M31A_DEFAULT_MODEL` — Override default model
- `M31A_PROVIDER` — Override default provider
- `M31A_PERMISSION_MODE` — Override permission mode
- `M31A_COMPACT` — Enable compact mode

### Hot-Reload
- Primary: `fsnotify` file watcher
- Fallback: Polling every 5s
- Debouncing: 50ms delay
- API keys stripped from config file when keychain is available

---

## 11. Error Handling

### Strategy
- Sentinel errors in `internal/errors/errors.go` (20+ sentinels)
- `UserMessage(error)` maps errors to user-friendly, actionable messages
- Pattern matching for unwrapped errors (HTTP status codes, connection errors)
- Self-heal loop (max 2 attempts) for recoverable task failures

### Key Sentinel Errors
- `ErrProviderUnreachable`, `ErrRateLimited`, `ErrInvalidKey`, `ErrNoCredits`
- `ErrContextExceeded`, `ErrSessionCorrupted`, `ErrCircularDependency`
- `ErrPrivateIPBlocked`, `ErrStreamTruncated`, `ErrToolExecution`
- `ErrPermissionDenied`, `ErrInvalidTimeout`

---

## 12. Data Flow

### Primary Workflow Path
1. User enters goal → GoalInput screen
2. PhaseModelPicker → model selection
3. Engine runs Initialize → project detection, git init, planning dir
4. Engine emits PhaseResultMsg → AppState transitions through phases
5. Execute: taskrunner schedules tasks, dispatches tools
6. Verify: file checks, syntax validation, test execution
7. Ship: final commit, ledger entry, session archival

### LLM Streaming Path
1. Engine builds prompt from embedded templates
2. Engine calls `provider.ChatStream()` via registry
3. SSE parser yields chunks
4. Engine forwards via `MsgEmitter` as `StreamChunkMsg`
5. TUI streaming display renders tokens in real-time

### Tool Execution Path
1. LLM response contains tool call JSON
2. Engine parses tool call
3. Dispatcher checks permissions
4. If permission required → emit PermissionRequestMsg → TUI shows modal
5. User approves/denies → PermissionResponseMsg → dispatcher continues
6. Tool executes → result returned to engine → fed back to LLM

---

## 13. State Management

- **Session state**: `pkg/session/Manager` persists to `.m31a/session.json`
- **Messages**: Separately persisted in `messages.json` for large history
- **Workflow state**: Persisted in session.json (goal, phase, questions) — survives restart
- **Checkpoints**: `checkpoint.json` (max 2) for undo/rollback support
- **Config**: TOML with hot-reload via fsnotify
- **TUI state**: In-memory `AppState` with screen enum routing

---

## 14. Testing

### Framework
- Go standard `testing` package (no testify, no gomock)
- Manual assertions with `t.Errorf`, `t.Fatalf`
- Table-driven tests with anonymous structs
- `t.Parallel()` used extensively
- `t.TempDir()` for filesystem isolation

### Test Types
- **Unit tests** — individual functions/methods
- **Integration tests** — real git repos, temp dirs, HTTP test servers
- **Security tests** — SSRF protection, timeout enforcement, path traversal
- **No E2E tests** — TUI tested with mock models

### Coverage Targets
- Overall: 75% (currently ~74.7%)
- Critical packages: 90% — `pkg/taskrunner` (89.9%), `pkg/bisect` (91.3%), `pkg/rollback` (89.1%)

### Coverage Gaps
- `internal/tui`: 38.6% (dangerously low for primary UI)
- `internal/tui/commands`: 10.6%
- `internal/tui/streaming`: 29.5%
- `pkg/keychain`: 20.3%
- `cmd/m31a`: 0.0%

---

## 15. CI/CD Pipeline

### GitHub Actions (`.github/workflows/ci.yml`)

| Job | Purpose | Timeout |
|-----|---------|---------|
| `lint` | gofmt + golangci-lint + GoReleaser validation | 10 min |
| `test` | `go test -race -coverprofile` + upload coverage | 10 min |
| `security` | `govulncheck ./...` | 10 min |
| `build` | Cross-platform matrix (6 targets) | 10 min |
| `release` | GoReleaser on tag push | 15 min |

**Triggers:** Push to `master`, PRs to `master`, manual dispatch

---

## 16. Security Model

### Strong Practices
- SSRF protection: DNS pinning, TOCTOU prevention, redirect checking
- Permission gating: Per-request channels, timeout, rate limiting
- Path traversal guards: Symlink resolution + workDir prefix check
- Process lifecycle: SIGINT/SIGKILL grace period, pipe cleanup
- Atomic file writes: temp file + rename
- No telemetry: no analytics, crash reporting, or usage pings
- Static binary: `CGO_ENABLED=0`
- API key resolution: env var → OS keychain → config file (never plaintext on disk)

### Known Security Concerns
- **SEC-01**: No ReDoS protection in pure-Go grep (mitigated by ripgrep availability)
- **SEC-02**: WebSearch missing DNS cache (defense-in-depth gap vs WebFetch)
- **SEC-03**: Incomplete HTML entity decoding in WebFetch

---

## 17. Known Issues & Tech Debt

### Bugs
1. **BUG-01**: Flaky git status test under race detector (timing assumption)
2. **BUG-02**: Ship phase commits unrelated files when taskFiles is empty
3. **BUG-03**: Demonstration generation can exceed context window for smaller models
4. **BUG-04**: Inconsistent `HasUncommittedChanges` implementations (porcelain vs human-readable)

### Tech Debt
- 12 ineffectual assignments across production code
- 50+ variable shadowing instances (concentrated in main.go, ship.go, rollback.go)
- Global gitignore cache has no eviction
- FileDelete has no backup pruning (unlike FileWrite and Edit)
- Dead writes in WebFetch HTML parser

### Priority Fixes
1. Fix flaky git test (blocks reliable CI)
2. Fix ship phase file staging (data integrity risk)
3. Increase TUI test coverage (38.6% → 75%)
4. Add ReDoS protection to pure-Go grep
5. Clean up dead code

---

## 18. Code Conventions

### Naming
- Files: kebab-case (`app_update.go`)
- Screen models: `{feature}_model.go` / `{feature}_view.go`
- Types: PascalCase structs, `-er` interfaces
- Constants: PascalCase exported, camelCase unexported
- Error package aliased as `m31errors`

### Style
- `gofmt` enforced in CI
- Imports: stdlib → third-party → internal (alphabetically sorted)
- Errors: `fmt.Errorf` with `%w` wrapping, lowercase messages
- Logging: `log/slog` only (no `fmt.Println` or `log.Printf`)
- No emojis in code or documentation

### Concurrency
- `sync.RWMutex` for read-heavy maps
- `sync.Once` for initialization
- Per-request channels with `sync.Map`
- Bounded goroutine pool via semaphore

### Tool Interface Pattern
```go
type Tool interface {
    Name() string
    Description() string
    RiskLevel() RiskLevel
    Execute(ctx context.Context, input ToolInput) (ToolResult, error)
}
```

### TUI Model Pattern
```go
type Model struct { /* state */ }
func NewXxxModel(...) Model { return Model{...} }
func (m Model) Init() tea.Cmd { return nil }
func (m Model) Update(msg tea.Msg) (Model, tea.Cmd) { ... }
func (m Model) View() string { ... }
```

---

## 19. Roadmap

### V1.1 Features (Planned)
- **Ghost mode** — headless runs producing structured diffs
- **Picture-in-picture** — second agent in side pane for cross-review
- **Subagents** — delegate sub-tasks to specialized agents (code, test, doc)
- **Deferred tools** — queue tool calls requiring human approval for batch review

### Missing Features
- `--check-config` flag (referenced in docs, not implemented)
- Backup pruning in FileDelete
- SSRF protection in WebSearch

---

## 20. Development Commands

```bash
make build          # optimized binary
make test           # race-enabled tests with coverage
make lint           # golangci-lint
make dev            # build + run
make test-fast      # tests without race detector
make test-specific TEST=TestFoo  # run specific test
make bench          # benchmarks
make cover          # HTML coverage report
make cross          # build for all platforms
make check          # fmt + tidy + vet + test
make help           # list all targets
```

---

## 21. File I/O Patterns

- **Atomic writes**: `internal/fileutil/atomic.go` — temp file + `os.Rename()`
- **Size limits**: `readFileLimited()` enforces 50MB max for session files
- **Backup system**: Auto-backup before file edits (default 5-10 per file)
- **Permissions**: `0644` for files, `0755` for directories
- **Skip dirs**: `node_modules`, `vendor`, `.next`, `dist`, `build`, `target`, `.venv`, `venv`, `__pycache__`

---

## 22. Token Estimation

- **tiktoken-go** v0.1.8 for OpenAI models
- **Fallback**: `utf8.RuneCountInString(text) / 4 * 1.3` for non-OpenAI models
- **EMA calibration**: Lock-free atomic CAS for runtime adjustment
- **Context warning**: Styled banner at configurable threshold (default 80%)

---

## 23. Key Constants

| Constant | Value | Purpose |
|----------|-------|---------|
| `ModelCacheTTL` | 5 min | Model cache active TTL |
| `BashTimeout` | 30 min | Max bash command duration |
| `BashOutputLimit` | 50,000 chars | Max bash output |
| `MaxToolOutputChars` | 10,000 chars | Max tool output |
| `MaxSessionFileSize` | 50 MB | Max session file read |
| `MaxHealAttempts` | 2 | Self-heal retries |
| `MaxPlanRetries` | 3 | Plan generation retries |
| `AutoDreamThreshold` | 60% | Context consolidation trigger |
| `ContextWarningThreshold` | 80% | Context warning banner |
| `DefaultPermissionTimeout` | 300s | Permission modal timeout |
| `MaxToolsPerCall` | 16 | Max tool calls per LLM response |
| `DefaultMaxParallelTasks` | 4 | Task runner concurrency |
| `SessionIDLength` | 8 chars | Session identifier length |

---

*Generated: 2026-06-14 | Source: Deep codebase analysis via parallel mapper agents*
