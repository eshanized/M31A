# Codebase Structure

**Analysis Date:** 2026-06-02

## Directory Layout

```
M31A/
├── cmd/                          # Binary entry points
│   ├── m31a/                     # Production binary (main.go)
│   └── test_zen/                 # Diagnostic CLI for Zen provider (main.go)
│
├── internal/                     # Application code (not importable externally)
│   ├── config/                   # Multi-layer TOML config: loader + types
│   ├── errors/                   # Sentinel errors (leaf package)
│   ├── git/                      # git CLI wrapper (init, commit, log, diff, bisect)
│   ├── log/                      # slog logger with daily rotation
│   ├── provider/                 # LLMProvider interface + Registry + cache + SSE
│   │   ├── openrouter/           # OpenRouter-specific client
│   │   └── zen/                  # OpenCode Zen-specific client
│   ├── tokens/                   # tiktoken-go token estimator
│   ├── tools/                    # Tool implementations + Dispatcher
│   ├── tui/                      # Bubble Tea app: AppState + all screens
│   │   ├── components/           # Reusable Lipgloss renderers
│   │   └── theme/                # Dark/light palettes + Manager
│   ├── types/                    # Core types: Message, Task, Tool, etc. (leaf)
│   └── workflow/                 # Six-phase orchestrator + embedded prompts/
│
├── pkg/                          # Public packages (importable, no TUI/workflow deps)
│   ├── arbitrage/                # Complexity scoring + cost comparison
│   ├── autodream/                # Context consolidation
│   ├── bisect/                   # git bisect wrapper
│   ├── keychain/                 # OS keychain (linux/darwin/windows via build tags)
│   ├── ledger/                   # Cross-session learning ledger
│   ├── rollback/                 # Commit chain + soft/hard reset
│   ├── session/                  # Session lifecycle, planning, checkpoint, fork
│   └── taskrunner/               # Topological sort + group execution
│
├── docs/                         # Human-readable architecture references
│   ├── ARCHITECTURE.md           # Package dep graph + data flow diagrams
│   ├── INTERFACES.md             # Go interface + type reference mirror
│   ├── TYPES.md                  # Constants, errors, enums reference
│   ├── CONFIG.md                 # Config file schema documentation
│   └── SLASH_COMMANDS.md         # All 28 slash commands reference
│
├── adrenaline/                   # Project planning artifacts (ideation, roadmap)
├── scripts/                      # Build/release helper scripts
├── .github/workflows/ci.yml      # CI: lint, test, build matrix
│
├── cmd/m31a/main.go              # 189 lines — flags, init, TUI launch
├── go.mod                        # Module: github.com/eshanized/M31A (Go 1.22)
├── go.sum
├── Makefile                      # build, test, lint, vet, clean
├── .golangci.yml                 # golangci-lint config
├── .goreleaser.yaml              # Multi-platform release config
├── install.sh                    # curl-pipe install script
├── LICENSE                       # MIT
├── README.md
├── CONTRIBUTING.md
├── CHANGELOG.md
│
└── .planning/                    # GSD planning artifacts
    ├── PROJECT.md                # Project overview
    ├── REQUIREMENTS.md
    ├── ROADMAP.md
    ├── STATE.md                  # Current phase + progress
    ├── phases/                   # Per-phase plans + summaries
    └── codebase/                 # Generated codebase maps (this document lives here)
```

## Directory Purposes

**`cmd/m31a/`:**
- Purpose: Production binary entry point
- Contains: `main.go` (189 lines) — CLI flag parsing, logger init, config load, provider registration, TUI launch
- Key files: `cmd/m31a/main.go`

**`cmd/test_zen/`:**
- Purpose: Diagnostic CLI to exercise the Zen provider in isolation (no TUI)
- Contains: `main.go` (120 lines) — direct provider invocation for integration testing

**`internal/config/`:**
- Purpose: Multi-layer TOML configuration with env-var override, variable substitution, and validation
- Contains: `loader.go` (575 lines) — `Load`, `Save`, `mergeConfig`, `validateConfig`, `applyVarSubstitution`; `types.go` — `Config`, `ProviderConfig`, `PermissionsConfig`, `PermissionRule`, `PermissionsAgentConfig`, etc.
- Key files: `loader.go`, `types.go`

**`internal/errors/`:**
- Purpose: Single sentinel-error source
- Contains: `errors.go` (20 lines) — 15 `var Err* = errors.New(...)` declarations
- Imported by: every package that needs error comparison

**`internal/git/`:**
- Purpose: Thin wrapper around `git` CLI
- Contains: `git.go` — `Init`, `ConfigUser`, `IsRepo`, `Add`, `Commit`, `Log`, `StatusPorcelain`, `Diff`, `DiffStaged`, `Reset`, `HeadHash`
- Key files: `git.go`

**`internal/log/`:**
- Purpose: Structured logger (slog) writing to file only
- Contains: `log.go` — `NewLogger(version)` returns `(logger, cleanup, err)`; daily rotation, 7-day retention
- Key files: `log.go`

**`internal/provider/`:**
- Purpose: LLM provider abstraction layer
- Contains:
  - `interface.go` (33 lines) — `LLMProvider` interface + `ChatRequest` + `ToolDefinition`
  - `registry.go` (75 lines) — `Registry` with `sync.RWMutex` map of providers
  - `cache.go` (96 lines) — `ModelCache` with 5-min TTL + 24h stale window
  - `sse.go` (72 lines) — `SSEParser` for Server-Sent Events line-by-line scanning
  - `reasoning.go` — pre-content vs. interleaved thinking normalization
  - `fallback.go` (53 lines) — `FindFallbackProvider` for health-check-based failover
  - `openrouter/client.go` (327 lines) — OpenRouter HTTP client
  - `zen/client.go` — OpenCode Zen HTTP client
- Key files: `interface.go`, `registry.go`, `cache.go`, `sse.go`, `openrouter/client.go`, `zen/client.go`

**`internal/tokens/`:**
- Purpose: Token estimation with model-specific tokenizer
- Contains: `estimator.go` — tiktoken-go for GPT/Claude families, `len(runes)/4*1.3` fallback, EMA correction
- Key files: `estimator.go`

**`internal/tools/`:**
- Purpose: Tool implementations + permission-aware dispatcher
- Contains: 10 tools + dispatcher + build-tag platform split
  - `interface.go` (36 lines) — `PermissionRequest`, `PermissionResponse`, `PermissionGate`
  - `dispatcher.go` (516 lines) — `Dispatcher` with rule matching, agent profiles, channels
  - `bash.go` (243 lines) — `Bash` tool with 30-min timeout + signal forwarding
  - `bash_unix.go` (27 lines) — build-tag Unix PTY support
  - `bash_windows.go` (31 lines) — build-tag Windows pipe fallback
  - `fileread.go` (159 lines), `filewrite.go` (186 lines), `edit.go` (491 lines) — file ops
  - `glob.go` (132 lines), `grep.go` (288 lines) — search
  - `webfetch.go` (373 lines) — HTTP GET
  - `todo.go` (154 lines) — `TodoWrite` for in-session task tracking
  - `question.go` (118 lines) — `AskUserQuestion` for blocking user questions
- Key files: `dispatcher.go`, `bash.go`, `fileread.go`, `filewrite.go`, `edit.go`

**`internal/tui/`:**
- Purpose: Bubble Tea application — every screen, every message type
- Contains: ~30 files
  - `app.go` (1676 lines) — `AppState` struct, `Init/Update/View`, screen router, `RunPhaseCmd`, `workflowMsgDrainer`
  - `types.go` (125 lines) — `Screen` enum + all `tea.Msg` types
  - `commands.go` (1240 lines) — 28 slash command handlers
  - `keybindings.go` (294 lines) — `KeyRegistry` with leader-key chord dispatch
  - `repl.go` (1465 lines) — `ReplModel`: chat, streaming, history, shell mode
  - `firstrun.go` (406 lines) — `FirstRunModel` for setup flow
  - `modelselector.go`, `plan.go`, `execute.go`, `verify.go`, `ship.go` — workflow screens
  - `settings.go`, `resume.go` — config + session resume
  - `sidebar.go`, `header.go`, `statusbar.go` — chrome
  - `streaming.go`, `cache.go`, `health.go`, `history.go` — feature controllers
  - `cmdpalette.go`, `diff.go`, `providerbadge.go`, `truncate.go` — supporting UI
- Key files: `app.go`, `repl.go`, `commands.go`, `types.go`, `keybindings.go`

**`internal/tui/components/`:**
- Purpose: Reusable, theme-aware Lipgloss renderers
- Contains:
  - `message.go` — `MessageRenderer` for chat messages (markdown, thinking blocks, tool cards)
  - `toolcard.go` — `ToolCard` for tool execution visualization
  - `toolrenderers.go` — per-tool rendering hooks
  - `thinking.go` — `ThinkingBlock` for collapsible reasoning
  - `permission.go` (204 lines) — `PermissionModal` overlay
  - `question.go` — `Question` modal for `AskUserQuestion`
- Key files: `message.go`, `permission.go`, `toolcard.go`, `thinking.go`

**`internal/tui/theme/`:**
- Purpose: Color palettes + theme manager
- Contains: `theme.go` — `Theme` struct with all named Lipgloss `Style` values (dark/light), `Manager` with `Cycle()` for `/theme` command

**`internal/types/`:**
- Purpose: Leaf package with all shared core types
- Contains:
  - `types.go` (166 lines) — `RiskLevel`, `WorkflowPhase`, `TaskStatus`, `Usage`, `CapFlags`, `Pricing`, `ModelInfo`, `MessageSegment`, `ToolCall`, `Message`, `ToolInput`, `ToolResult`, `Tool` interface, `Task`, `ProjectState`, `Session`, `StreamChunk`, `StreamIterator`, `HealthStatus`
  - `constants.go` (19 lines) — `ModelCacheTTL`, `HealthCheckInterval`, `MaxFileSize`, `MaxToolOutputChars`, `MaxHealAttempts`, `MaxPlanRetries`, `SessionIDLength`, `AutoDreamThreshold`, `ContextWarningThreshold`, `HTTPDialTimeout`, `BashTimeout`, `BashOutputLimit`, `DefaultContextLength`
- Key files: `types.go`, `constants.go`
- **Imported by:** every other package

**`internal/workflow/`:**
- Purpose: Six-phase orchestrator with embedded prompt templates
- Contains:
  - `engine.go` (966 lines) — `Engine`, `PhaseResult`, `PromptRegistry`, `MsgEmitter`, `TaskStartMsg`, `TaskUpdateMsg`, JSON parsers, cycle detection, project-type detection
  - `initialize.go` (73 lines) — `runInitialize`: project type detect, git init, `PROJECT.md` write
  - `discuss.go` — `runDiscuss`: clarifying questions
  - `plan.go` — `runPlan`: LLM task generation + JSON parse + validation
  - `execute.go` (297 lines) — `runExecute`: taskrunner wiring, tool dispatch, self-heal loop
  - `verify.go` (156 lines) — `runVerify`: project-aware checks (`go build`, `npm run build`, `python3 -m py_compile`, `cargo check`)
  - `ship.go` (147 lines) — `runShip`: final commit + ledger entry
  - `prompts/*.md` (7 files) — base, tool-use, plan-format, execute-task, discuss-questions, self-heal, verify-checklist (embedded via `//go:embed`)
- Key files: `engine.go`, `execute.go`, `prompts/`

## Key File Locations

**Entry Points:**
- `cmd/m31a/main.go` — `main()`; flags, init, TUI launch
- `internal/tui/app.go:97` — `NewApp()` factory
- `internal/tui/app.go:412` — `AppState.Init()` (Bubble Tea entry)
- `internal/tui/app.go:445` — `AppState.Update()` (Bubble Tea message loop)
- `internal/tui/app.go:1386` — `AppState.View()` (Bubble Tea renderer)
- `internal/workflow/engine.go:141` — `Engine.RunPhase()` (workflow entry)

**Configuration:**
- `internal/config/types.go` — `Config` struct + all sub-configs
- `internal/config/loader.go:40` — `Load(path)` multi-layer merge entry
- `internal/config/loader.go:469` — `Config.Save(path)` atomic write
- `internal/config/loader.go:508` — `Config.ResolveAPIKeys(kc)` 3-tier resolution

**Core Logic:**
- `internal/types/types.go` — All core domain types
- `internal/provider/interface.go` — `LLMProvider` contract
- `internal/provider/registry.go:71` — `Registry.ActiveProvider()`
- `internal/tools/dispatcher.go:71` — `Dispatcher.Execute` (the gate)
- `internal/workflow/engine.go:333` — `Engine.streamLLM` (chat completion path)
- `pkg/session/manager.go:127` — `Manager.NewSession`
- `pkg/session/manager.go:173` — `Manager.LoadSession`
- `pkg/session/manager.go:289` — `Manager.ForkSession` (parent/child linking)

**Testing:**
- `*_test.go` next to every source file (Go convention)
- Test scaffolding: `httptest`, `testify`, mock providers via interface substitution
- No separate `tests/` or `testdata/` directories; all fixtures are inline or in `/tmp/`

**Build / Release:**
- `Makefile` — `build`, `test`, `lint`, `vet`, `clean`
- `.golangci.yml` — linter rules
- `.goreleaser.yaml` — multi-platform release (linux/amd64, linux/arm64, darwin/amd64, darwin/arm64, windows/amd64)
- `install.sh` — one-line curl install
- `.github/workflows/ci.yml` — CI pipeline

## Naming Conventions

**Files:**
- `snake_case.go` for Go source files (e.g. `modelselector.go`, `filewrite.go`, `bash_unix.go`)
- `snake_case_test.go` for test files (e.g. `dispatcher_test.go`, `manager_test.go`)
- Build-tag suffix for platform splits: `_linux.go`, `_darwin.go`, `_windows.go`, `_unix.go`
- One primary type per file; multiple small types allowed if closely related
- Embedded assets: `prompts/*.md` (Markdown), accessed via `//go:embed`

**Directories:**
- `internal/<package>/` — single Go package per directory, package name matches directory
- `pkg/<package>/` — public package, same convention
- `internal/tui/components/` — sibling sub-package, `package components`
- `internal/tui/theme/` — sibling sub-package, `package theme`
- `internal/workflow/prompts/` — non-Go subdir for embedded assets

**Identifiers:**
- Types: `PascalCase` (`AppState`, `ReplModel`, `PermissionModal`)
- Interfaces: noun or `-er` suffix (`LLMProvider`, `Tool`, `MsgEmitter`, `Keychain`)
- Constructors: `New<Name>` returning pointer (`NewRegistry`, `NewModelCache`, `NewPermissionModal`, `NewReplModel`)
- Options: `Options` struct + functional options pattern (`openrouter.Options`, `session.ManagerOpts`)
- Errors: `Err<Name>` sentinel (`ErrProviderUnreachable`, `ErrRateLimited`)
- Constants: `PascalCase` (`RiskSafe`, `PhaseInitialize`, `ScreenREPL`) or `SCREAMING_SNAKE_CASE` for tunables (`ModelCacheTTL`, `MaxHealAttempts`)
- Private helpers: `camelCase` (`extractArrayFrom`, `parseSingleToolCall`, `validateService`)
- Channel variables: `ch`, `requestCh`, `responseCh`, `msgChan`
- TUI messages: `<Name>Msg` (`PermissionRequestMsg`, `PhaseResultMsg`, `TaskStartMsg`, `PlanReadyMsg`)
- Bubble Tea commands: `tea.Cmd` factories named `<verb>Cmd` or `<verb>ListenerCmd`

**Comments:**
- Package-level doc comment required on every package
- Exported identifiers have `// Name does X.` doc comments
- Implementation comments for non-obvious algorithms (e.g. `extractArrayFrom` bracket-depth tracking)

## Where to Add New Code

**New LLM provider (e.g. Anthropic-direct — currently forbidden, but for the pattern):**
- Add `internal/provider/<name>/client.go` implementing `provider.LLMProvider` with `var _ provider.LLMProvider = (*Client)(nil)` assertion
- Register in `cmd/m31a/main.go` after the existing OpenRouter/Zen blocks
- Add `BaseURL`, `APIKey` fields to `internal/config/types.go:ProviderConfig`
- Add resolution logic to `internal/config/loader.go:Config.ResolveAPIKeys`
- Tests in `client_test.go` next to the new package

**New tool:**
- Add `internal/tools/<toolname>.go` implementing `types.Tool` (`Name`, `Description`, `RiskLevel`, `Execute`)
- Register in `internal/tools/dispatcher.go:DefaultDispatcher` (~line 502)
- For platform-specific behavior, add `<toolname>_unix.go` / `<toolname>_windows.go` with build tags
- Add `extractCommandString` case in `internal/tools/dispatcher.go` if it should show a friendly name in the permission modal
- Add a test file `<toolname>_test.go` with `httptest` or temp-dir fixtures
- Optionally add a renderer in `internal/tui/components/toolrenderers.go`

**New slash command:**
- Add handler function `handleXxx(args []string, ctx CommandContext) CommandResult` in `internal/tui/commands.go`
- Register in `DefaultCommands()` (~line 159): `r.Register("xxx", handleXxx, "description")`
- For commands that need a screen transition, return `CommandResult{Screen: &screen, ...}`
- For commands that need session switching, return `CommandResult{SessionID: &id, ...}`

**New TUI screen:**
- Add a screen constant in `internal/tui/types.go` (~line 12 — extend the `Screen` enum)
- Add a `case ScreenXxx:` block in `AppState.Update` (~line 1204) for message routing
- Add a `case ScreenXxx:` block in `AppState.View` (~line 1401) for rendering
- Create the screen model as a new file (e.g. `internal/tui/newscreen.go`) with its own `Update/View` methods
- Wire initialization in `internal/tui/app.go:NewApp`
- Add the new screen to `currentKeyContext` mapping in `app.go:1539` if it needs key bindings
- Add a test file with table-driven message handling

**New workflow phase:**
- Add `runXxx` method in `internal/workflow/engine.go` (or new `xxx.go` in same package)
- Add `case m31types.PhaseXxx:` in `Engine.RunPhase` switch (~line 147)
- Add phase constant to `internal/types/types.go:WorkflowPhase` enum
- Add prompt template to `internal/workflow/prompts/xxx.md` and reference in `PromptRegistry` (~line 58)
- Update `AppState.PhaseResultMsg` switch in `app.go:1004` to handle the new phase result

**New public package (e.g. a new utility under `pkg/`):**
- Create `pkg/<name>/<name>.go` with `package <name>`
- Import only `internal/types` and `internal/errors` (no `tui`, `workflow`, `config`, or other `pkg/` packages)
- Wire into `internal/tui/app.go:NewApp` if it needs to be constructed at boot
- Add `*_test.go` next to source

**New `KeyBinding`:**
- Add to `internal/tui/keybindings.go:RegisterDefaultBindings` (~line 245) in the appropriate `Ctx*` block
- For actions that need app-level state mutation, use `KeyActionMsg{Action: "..."}` and add a `case` in `AppState.handleKeyAction` (`app.go:1617`)

**New permission rule context field:**
- Extend `internal/tools/interface.go:PermissionRequest` and `PermissionContext` if the field needs to flow to the modal
- Update `internal/tools/dispatcher.go:extractCommandString` to populate the new field per-tool
- Update `internal/tui/components/permission.go:Render` to display the new info

## Special Directories

**`internal/workflow/prompts/`:**
- Purpose: Embedded Markdown prompt templates compiled into the binary
- Generated: No (handwritten)
- Committed: Yes
- Loaded by: `Engine.NewEngine` via `LoadPrompts()` which calls `promptFS.ReadFile(...)`
- Adding a prompt: drop the `.md` file here and register it in `engine.go:PromptRegistry` + the `LoadPrompts` map

**`cmd/test_zen/`:**
- Purpose: Diagnostic CLI to exercise the Zen provider outside the TUI
- Generated: No
- Committed: Yes
- Built only by: `cd cmd/test_zen && go build` (not part of the production binary)
- Used by: integration testing of provider changes

**`coverage.out`, `coverage.html`, `cover.out`, `tui.test`:**
- Purpose: Go test coverage output and a test binary from a previous run
- Generated: Yes (by `go test -cover`)
- Committed: Yes (visible in `ls` output) — should ideally be in `.gitignore`
- Safe to delete: yes; regenerated on next test run

**`used/`:**
- Purpose: An empty placeholder directory visible in `ls` output
- Committed: yes
- May be safe to delete

**`rush/`:**
- Purpose: Unknown project-specific directory (1554 bytes) — not part of the standard M31A layout
- Committed: yes
- Investigate before deleting

**`adrenaline/`:**
- Purpose: Project planning artifacts (idea, REFERENCE, ROADMAP) — corresponds to the pre-development ideation phase
- Generated: No (manually authored)
- Committed: Yes
- Reference: `adrenaline/ROADMAP.md` is the source-of-truth for the 12-phase development plan

**`.planning/`:**
- Purpose: GSD (Get-Shit-Done) planning workflow artifacts
- Contains: `PROJECT.md`, `REQUIREMENTS.md`, `ROADMAP.md`, `STATE.md`, `phases/`, `codebase/`, `config.json`
- Generated: Yes (by GSD commands)
- Committed: Yes (this is where this document lives)
- This file (`STRUCTURE.md`) and `ARCHITECTURE.md` are written to `.planning/codebase/`

**`docs/`:**
- Purpose: Human-readable architecture and reference docs (separate from `.planning/`)
- Contains: `ARCHITECTURE.md`, `INTERFACES.md`, `TYPES.md`, `CONFIG.md`, `SLASH_COMMANDS.md`
- Generated: Mixed (handwritten, but mirrors content from `*.go` files for LLM context)
- Committed: Yes
- This is the canonical docs directory; `.planning/codebase/` is the GSD-generated complement

---

*Structure analysis: 2026-06-02*
