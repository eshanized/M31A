# Full Audit: Phase 0 through Phase 8

You are performing a comprehensive audit of the entire M31A codebase, from Phase 0 (Foundation) through Phase 8 (Master Prompts). This is the most thorough audit possible — you will find every deviation, drift, missing implementation, wrong implementation, and architectural inconsistency.

## Source of Truth

The authoritative specification is `adrenaline/ROADMAP.md`. All phase deliverables, acceptance criteria, and architectural decisions defined there are the ground truth. Secondary references are `adrenaline/idea.md` (full V1 spec) and `adrenaline/REFERENCE.md` (project overview).

## Audit Scope

Audit ALL code in `/home/snigdha/Desktop/Helix/M31A`. Check every package, every file, every test.

---

## Part 1: Phase-by-Phase Deliverable Verification

For each phase, verify every deliverable listed in the ROADMAP exists, compiles, and behaves correctly.

### Phase 0 — Foundation

Check:
- [ ] `cmd/m31a/main.go` exists, prints version, exits cleanly
- [ ] `internal/provider/interface.go` — `LLMProvider` interface with all required methods
- [ ] `internal/tools/interface.go` — `Tool`, `ToolCall`, `ToolResult`, `RiskLevel` types
- [ ] `internal/types/` — `Message`, `MessageSegment`, `Session`, `Task`, `TaskStatus`, `WorkflowPhase`, `ProjectState`, `StreamIterator`, `StreamChunk`, `ModelInfo`, `HealthStatus`, `Usage`, `CapFlags`, `Pricing`
- [ ] `internal/errors/` — sentinel errors defined
- [ ] `internal/types/constants.go` — all constants present (`ModelCacheTTL`, `HealthCheckInterval`, `MaxFileSize`, `MaxToolOutputChars`, `MaxHealAttempts`, `MaxPlanRetries`, `SessionIDLength`, `AutoDreamThreshold`, `ContextWarningThreshold`, `HTTPDialTimeout`, `BashTimeout`, `BashOutputLimit`, `DefaultContextLength`)
- [ ] `internal/log/` — structured logger with rotation to `~/.m31a/m31a.log`
- [ ] `docs/` directory exists with at minimum `ARCHITECTURE.md`
- [ ] `go.mod` declares `go 1.22`
- [ ] CI structure (`.github/workflows/`) exists or is deferred with documentation

### Phase 1 — Provider Layer

Check:
- [ ] `internal/provider/` — `LLMProvider` interface: `Name()`, `FetchModels()`, `ChatCompletionStream()`, `EstimateCost()`, `HealthCheck()`, `GetModel()`
- [ ] `ProviderRegistry` with `Register`, `Active`, `SetActive`, `Get`, `List`, `ActiveProvider`
- [ ] `internal/provider/openrouter/` — client with correct base URL, SSE parsing, model cache with 5-min TTL, health check
- [ ] `internal/provider/zen/` — client with correct base URL, reasoning normalization, health check
- [ ] `internal/provider/cache.go` — `ModelCache` with dual-TTL (normal + stale), `IsExpired()`, `IsStale()`, `Get()`, `Set()`
- [ ] `internal/provider/cache.go` — `ModelCacheRefreshTicker` with `NewModelCacheRefreshTicker`, `Tick`, `Stop`
- [ ] `internal/provider/fallback.go` — `FallbackEvent` struct, `FindFallbackProvider()` function
- [ ] Auto-fallback logic: on 429/503, switches active provider, emits `FallbackEvent`
- [ ] Reasoning normalization: pre-content vs. interleaved thinking handled
- [ ] `EstimateCost()` accepts `modelID string` and computes real pricing
- [ ] Health check returns latency
- [ ] Background model cache refresh ticker runs at `ModelCacheTTL` interval

### Phase 2 — TUI Foundation

Check:
- [ ] `internal/tui/app.go` — `AppState` struct, `Init()`, `Update()`, `View()` implementing `tea.Model`
- [ ] Screen routing with all 10 screens: `ScreenFirstRun`, `ScreenREPL`, `ScreenModelSelector`, `ScreenSettings`, `ScreenResume`, `ScreenPermission`, `ScreenPlan`, `ScreenExecute`, `ScreenVerify`, `ScreenShip`
- [ ] `internal/tui/theme/` — `Theme` struct with dark/light palette, `Cycle()`, all named styles from spec
- [ ] `internal/tui/repl.go` — header (brand, model badge, context bar), message area (viewport), input area (textarea), status bar, spinner
- [ ] `internal/tui/firstrun.go` — provider selection, key input + validation, OS keychain storage prompt, skip option
- [ ] `internal/tui/health.go` — `HealthCheckTicker` with adaptive interval (60s normal, 120s on rate-limit), `HealthUpdateMsg`
- [ ] `internal/tui/resume.go` — session list, sort by last-modified, keys: Enter/N/D
- [ ] `internal/tui/header.go` — brand, model badge, context usage, connection status
- [ ] `internal/tui/statusbar.go` — current operation, timestamp
- [ ] `internal/tui/streaming.go` — proper `tea.Cmd` pattern (single message per invocation, not chan), `StartStreamCmd`, `StreamMsg`, `StreamDoneMsg`, `StreamErrorMsg`
- [ ] `internal/tui/components/thinking.go` — `ThinkingBlock` with `Toggle()`, `IsExpanded()`, `Duration()`, `Header()`, duration formatting
- [ ] `internal/tui/components/toolcard.go` — card layout, status cycle, auto-collapse, binary detection
- [ ] `internal/tui/components/permission.go` — centered overlay, tool name, command preview, risk badge, timeout countdown, keys Y/A/N/Esc
- [ ] `internal/tui/components/message.go` — Glamour renderer with **dynamic width** (`SetWidth()` method), dark/light styles
- [ ] Message types in `internal/tui/types.go` — `AppMsg`, `HealthUpdateMsg`, `HealthCheckTickMsg`, `ProviderSwitchMsg`, `ErrorMsg`, `PermissionRequestMsg`, `PermissionResponseMsg`, `FallbackEventMsg`, `RefreshCacheMsg`, `ModelSelectedMsg`, `SettingsSavedMsg`, `ThinkingToggleMsg`
- [ ] `formatToolInput()` implements per-tool formatting (not pass-through)
- [ ] Thinking block has `T` key binding wired in REPL

### Phase 3 — Rendering Pipeline

(Audited as part of Phase 2 above — the rendering components overlap)

Additional checks:
- [ ] Streaming renders token-by-token without full re-render
- [ ] Thinking blocks show duration counter, collapse/expand on `T`
- [ ] Tool cards render with correct colors per tool type
- [ ] Permission modal appears and gates tool execution
- [ ] Glamour renderer recreated on `WindowSizeMsg` (not fixed at 78 chars)

### Phase 4 — Tool System

Check:
- [ ] `internal/tools/bash.go` — `exec.Cmd` with 30-min timeout, signal forwarding, **io.Pipe streaming** (not bytes.Buffer), binary detection, output capped at `BashOutputLimit`
- [ ] `internal/tools/fileread.go` — encoding detection, binary detection (first 512 bytes), 5MB limit, path safety (symlink resolution, cwd boundary)
- [ ] `internal/tools/filewrite.go` — atomic write (temp file + rename), backup to session dir, `DurationMs` in result
- [ ] `internal/tools/glob.go` — `**` recursive support via doublestar, ripgrep integration, **consistent relative paths** from both backends, 1000 result limit
- [ ] `internal/tools/grep.go` — ripgrep detection, `--json` output, pure-Go fallback, **searchPath resolved relative to workDir**, `.gitignore` respect
- [ ] `internal/tools/dispatcher.go` — tool routing, permission gate (`RiskLevel` check), `PermissionRequest.Command` **populated** from `call.Input`, `RequestCh()`/`ResponseCh()` channels, `DefaultDispatcher()` factory
- [ ] `tools.PermissionRequest` has `Command` field and it is populated
- [ ] All 5 tools implement `types.Tool` interface: `Name()`, `Description()`, `RiskLevel()`, `Execute()`
- [ ] Risk levels: Bash=Medium, FileRead=Safe, FileWrite=Safe, Glob=Safe, Grep=Safe

### Phase 5 — Session State & Config

Check:
- [ ] `internal/config/` — TOML parsing (`BurntSushi/toml`), env var override, config struct with all sub-configs (`ProviderConfig`, `ModelConfig`, `UIConfig`, `PermissionsConfig`, `FeaturesConfig`, `LedgerConfig`, `GhostConfig`)
- [ ] `internal/config/types.go` — `AutoArbitrage`, `ArbitrageThreshold` in ModelConfig; `AutodreamEnabled` in FeaturesConfig
- [ ] `pkg/keychain/` — unified interface (`Get`, `Set`, `Delete`), `ErrNotImplemented` for stubs
- [ ] `pkg/keychain/keychain_linux.go` — freedesktop Secret Service or `pass` fallback
- [ ] `pkg/keychain/keychain_darwin.go` — macOS Keychain Services
- [ ] `pkg/keychain/keychain_windows.go` — Windows Credential Manager or stub
- [ ] `pkg/session/manager.go` — `NewSession`, `LoadSession`, `SaveSession`, `ListSessions`, `DeleteSession`, `ArchiveSession`
- [ ] `pkg/session/checkpoint.go` — `SaveCheckpoint`, `LoadCheckpoints`, `LatestCheckpoint`, max 2 retained
- [ ] `pkg/session/planning.go` — `SaveProject`/`LoadProject`, `SaveTasks`/`LoadTasks`, `SaveState`/`LoadState`
- [ ] Atomic writes everywhere (temp file + rename)
- [ ] `internal/tokens/estimator.go` — tiktoken-go for GPT/Claude, fallback estimation, context warning at threshold
- [ ] API key resolution order: env var → OS keychain → config file

### Phase 6 — Workflow Engine

Check:
- [ ] `internal/workflow/engine.go` — `Engine` struct with all fields including `prompts *PromptRegistry`, `RunPhase()`, `Transition()`, `buildSystemPrompt()`
- [ ] `internal/workflow/prompts/` — all 7 prompt files present (base.md, tool-use.md, plan-format.md, execute-task.md, discuss-questions.md, self-heal.md, verify-checklist.md)
- [ ] `//go:embed prompts/*.md` with `promptFS embed.FS`
- [ ] `PromptRegistry` struct with 7 fields, `LoadPrompts()` function
- [ ] Phase composition matrix:
  - Discuss = base + discuss-questions
  - Plan = base + tool-use + plan-format
  - Execute = base + tool-use + execute-task
  - Heal = base + tool-use + self-heal
- [ ] `internal/workflow/initialize.go` — goal parsing, project detection (8 file types), git init, planning dir, PROJECT.md, STATE.md
- [ ] `internal/workflow/discuss.go` — context building, LLM streaming, question parsing (numbered + fallback), Q&A save to PROJECT.md
- [ ] `internal/workflow/plan.go` — task generation, JSON extraction, markdown code block stripping, schema validation (self-refs, cycles, missing fields, duplicate IDs), 3-retry loop, TASKS.md
- [ ] `internal/workflow/execute.go` — task runner integration, tool dispatch, per-task git commits, self-heal loop (max 2), STATE.md updates, healTask with self-heal context
- [ ] `internal/workflow/verify.go` — file existence checks, syntax validation (go build), test execution (go test), self-heal on failure, bisect trigger on unrecoverable
- [ ] `internal/workflow/ship.go` — final commit, task summary, LEDGER.md append, session archive, STATE.md final update
- [ ] `pkg/taskrunner/runner.go` — Kahn's algorithm, cycle detection, dependency blocking, sequential execution, `ExecuteGroup`, `Status`, `Results`, `AllDone`, `Summary`, `Tasks`
- [ ] `pkg/bisect/bisect.go` — `Bisect.Run(good, bad, checkFn)`, log parsing, diff extraction, defer reset
- [ ] `internal/git/git.go` — all 17 operations (Init, IsRepo, Add, AddAll, Commit, CommitWithFiles returning hash, Log, Diff, DiffStaged, Status, HeadHash, CreateBranch, CurrentBranch, ResetSoft, ResetHard, StashPush, StashPop, ConfigUser)
- [ ] TUI screens: `internal/tui/plan.go`, `internal/tui/execute.go`, `internal/tui/verify.go`, `internal/tui/ship.go`

### Phase 7 — Signature Features

Check:
- [ ] `internal/tui/commands.go` — `CommandRegistry`, `ParseCommand`, `DefaultCommands`, **NO duplicate registrations**, all 16+ handlers
- [ ] `internal/tui/commands_test.go` — tests for all command handlers
- [ ] `pkg/rollback/rollback.go` — `Chain()`, `Preview()`, `SoftReset()`, `HardReset()`, `SafeReset()`, `HasUncommittedChanges()`
- [ ] `pkg/arbitrage/arbitrage.go` — `Scorer`, `Score()`, `EstimateTokens()`, `CompareModels()`, `Recommend()`, `ShouldArbitrage()`
- [ ] `pkg/autodream/autodream.go` — `Consolidator`, `CanConsolidate()`, `Consolidate()`, `GenerateSummary()`, `WarningMessage()`
- [ ] `pkg/ledger/ledger.go` — `Ledger`, `Entries()`, `EntriesFiltered()`, `Stats()`, `StatsFiltered()`, `RecentSessions()`, `Exists()`, `Append()`
- [ ] `internal/tui/modelselector.go` — `ModelSelector`, `NewModelSelector`, `Init`, `Update`, `View`, `SelectedModel`, search, provider filter, navigation
- [ ] `internal/tui/modelselector_test.go` — **MUST EXIST** with 15+ tests
- [ ] `internal/tui/settings.go` — 6 tabs (General, Provider, Model, Permissions, Features, Ledger), inline editing, save
- [ ] `internal/tui/settings_test.go` — tests for tab count, cycling, editing, saving
- [ ] `FallbackEventMsg` handled in `app.go` Update() — **MUST have case handler**
- [ ] `FallbackNotification` type in `app.go`
- [ ] Thinking toggle `T` key **wired in REPL Update()**
- [ ] `CacheRefreshTicker` scheduled in `AppState.Init()`

### Phase 8 — Master Prompts

(Already verified — confirm no regressions)
- [ ] All 7 prompt files exist with content
- [ ] `//go:embed` loading works
- [ ] Old `const systemPrompt` removed
- [ ] Phase composition correct
- [ ] Tests pass

---

## Part 2: Cross-Cutting Concerns

### Type Consistency

Check every type used across packages for consistency:
- [ ] `types.Message` — same struct definition everywhere, no duplicated variants
- [ ] `types.Task` — all fields present (`ID`, `Description`, `Action`, `Dependencies`, `Files`, `AcceptanceCriteria`, `Status`, `HealsAttempted`, `PredictedFiles`, `CommitHash`)
- [ ] `types.ToolCall` — `ID`, `Name`, `Input json.RawMessage`
- [ ] `types.ToolResult` — `ToolCallID`, `Output`, `Error`, `DurationMs`, `Truncated`
- [ ] `types.ModelInfo` — all fields present including `Pricing`
- [ ] `types.WorkflowPhase` — all 7 phases defined
- [ ] `types.TaskStatus` — all 6 statuses defined
- [ ] `types.RiskLevel` — all 4 levels defined
- [ ] No duplicated types across packages (e.g., `CommitInfo` in both `git` and `bisect`)

### AGENTS.md Compliance

Check against `AGENTS.md` rules:
- [ ] **No CGO** — no `import "C"`, all builds with `CGO_ENABLED=0`
- [ ] **Bubble Tea single-threaded** — no goroutine state mutations in TUI
- [ ] **No direct Anthropic/OpenAI** — only OpenRouter and Zen
- [ ] **No AskUserQuestion tool** in V1
- [ ] **V1 sequential execution** — no concurrency in task runner
- [ ] **No telemetry** — no phone-home, no analytics
- [ ] **No hardcoded model lists** — models discovered dynamically
- [ ] **V1 tools only**: Bash, FileRead, FileWrite, Glob, Grep — no FileEdit, WebFetch, WebSearch, AgentTool, TaskTool, GitTool
- [ ] **API keys not in plaintext** — resolved via env → keychain → config

### Import Discipline

Check for unauthorized dependencies:
- [ ] No imports beyond AGENTS.md approved list:
  - charmbracelet/bubbletea, lipgloss, bubbles, glamour
  - BurntSushi/toml
  - tiktoken-go
  - doublestar
  - creack/pty
- [ ] No `github.com/google/uuid` (should use `crypto/rand` for session IDs)
- [ ] No `github.com/stretchr/testify` (use standard library testing)

### File Organization

Check package layout matches AGENTS.md:
```
cmd/m31a/          — binary entry point only
internal/config/   — config parsing
internal/provider/ — LLMProvider interface + OpenRouter + Zen
internal/tui/      — Bubble Tea app, all screens
internal/workflow/ — six workflow phases
internal/tools/    — Bash, FileRead, FileWrite, Glob, Grep
internal/log/      — structured logger
internal/git/      — git operations
internal/types/    — shared core types
internal/errors/   — sentinel errors
internal/tokens/   — token estimation
internal/tui/theme/— theme system
internal/tui/components/ — reusable components
pkg/taskrunner/    — dependency graph
pkg/arbitrage/     — complexity scoring
pkg/bisect/        — git bisect
pkg/ledger/        — cross-session learning
pkg/rollback/      — commit rollback
pkg/autodream/     — context consolidation
pkg/session/       — session lifecycle
pkg/keychain/      — OS keychain
```

---

## Part 3: Behavioral Verification

For each critical behavior, verify the implementation actually does what the spec says:

### Streaming
- [ ] `StartStreamCmd` returns proper `tea.Cmd` (single message per call), not `chan tea.Msg`
- [ ] Stream chunks arrive token-by-token, not buffered
- [ ] Thinking segments detected and rendered as collapsible blocks
- [ ] `StreamDoneMsg` and `StreamErrorMsg` terminate streams

### Tool Execution
- [ ] Bash output streams during execution (io.Pipe), not buffered until completion
- [ ] FileWrite uses atomic write pattern (temp + rename)
- [ ] FileRead rejects paths outside workDir
- [ ] Grep resolves relative paths against workDir
- [ ] Glob returns consistent relative paths from both backends
- [ ] Dispatcher populates `PermissionRequest.Command`

### Permissions
- [ ] Dangerous/destructive tools trigger permission modal
- [ ] Permission modal shows actual command string
- [ ] Timeout auto-denies after countdown
- [ ] Allow-Always persists

### Workflow
- [ ] Each phase builds its own context (context pruning)
- [ ] Plan phase retries up to 3 times on validation failure
- [ ] Execute phase commits per task
- [ ] Self-heal limited to 2 attempts
- [ ] Verify triggers bisect on unrecoverable
- [ ] Ship appends to LEDGER.md and archives session

### Session
- [ ] Session files written atomically
- [ ] Resume reconstructs state from disk
- [ ] Checkpoints limited to 2
- [ ] LEDGER.md entries appended correctly

---

## Part 4: Test Coverage Audit

For each package:
- [ ] Report test count
- [ ] Report coverage percentage (run `go test -cover ./...`)
- [ ] Flag packages with < 50% coverage
- [ ] Flag packages with 0 tests that should have tests

Specifically check:
- [ ] `internal/provider/` — 80%+ coverage
- [ ] `internal/tools/` — 70%+ coverage
- [ ] `internal/tui/` — 70%+ coverage
- [ ] `internal/tui/components/` — 70%+ coverage
- [ ] `internal/workflow/` — all phases tested
- [ ] `pkg/taskrunner/` — 14+ tests
- [ ] `pkg/bisect/` — 11+ tests
- [ ] `pkg/rollback/` — 18+ tests
- [ ] `pkg/arbitrage/` — 16+ tests
- [ ] `pkg/autodream/` — 19+ tests
- [ ] `pkg/ledger/` — 23+ tests
- [ ] `pkg/session/` — session lifecycle tested
- [ ] `pkg/keychain/` — all platforms tested
- [ ] `internal/git/` — 16+ tests
- [ ] `internal/tokens/` — estimation tested

---

## Part 5: Build & Binary Verification

Run these commands and report output:
```
go mod tidy
CGO_ENABLED=0 go build -o m31a ./cmd/m31a
go vet ./...
go test -race -count=1 ./...
file m31a
```

Report:
- [ ] `go mod tidy` — clean or errors
- [ ] `go build` — clean or errors
- [ ] `go vet` — clean or warnings
- [ ] `go test` — total tests, passed, failed
- [ ] `file m31a` — binary type (must be "statically linked")

---

## Part 6: Deviation Register

For every issue found, create an entry:

| ID | Phase | Type | Severity | File | Description | Expected | Actual | Fix Required |
|----|-------|------|----------|------|-------------|----------|--------|-------------|
|    |       |      |          |      |             |          |        |             |

**Types**: drift (implementation differs from spec), missing (deliverable not present), wrong (implementation is incorrect), incomplete (partial implementation), test_gap (missing tests), style (code quality issue)

**Severity**: CRITICAL (blocks next phase), HIGH (affects user experience), MEDIUM (should fix), LOW (cosmetic), INFO (observation)

---

## Part 7: Walkthrough

After completing the audit, write `rush/walkthrough_audit_0_8.md` with:

1. **Executive Summary** — Overall health assessment (GO / CONDITIONAL GO / NO-GO)
2. **Audit Scorecard** — Pass/Fail per phase
3. **Deviation Register** — All issues from Part 6
4. **Test Coverage Summary** — Per-package test count and coverage
5. **Build Verification** — All command outputs
6. **Critical Path Assessment** — What must be fixed before v1.0.0
7. **Recommendations** — Prioritized fix list
8. **Remaining Work** — What phases/sub-tasks are still needed per ROADMAP.md

---

## Execution Instructions

1. Read the ROADMAP.md fully first
2. Read each file mentioned above
3. Run build/test commands
4. Compile findings into the deviation register
5. Write the walkthrough

Do NOT write any code. This is a research and audit task only.
