# Phase 29 — Complete TUI Rewrite — Context

**Gathered:** 2026-06-08
**Status:** Ready for planning
**Source:** User request + comprehensive codebase analysis

<domain>
## Phase Boundary

Complete deletion and from-scratch rewrite of `internal/tui/`, `internal/tui/components/`, and `internal/tui/theme/`. The rewrite must produce a working TUI with all 16 screens, streaming pipeline, permission modal, workflow integration, 30+ slash commands, and all reusable components. The binary must compile, all tests must pass, and the build must succeed on all 5 platforms.

## What Exists Today (to be deleted)

- 89 Go files (55 source + 18 test + 16 components/theme)
- 33,324 total lines (18,500 source + 11,000 tests + 3,800 components)
- 16 screen models with views
- 22+ message types
- 30+ slash commands
- 9 keybinding contexts
- Streaming pipeline with goroutine ownership
- Permission modal with countdown
- Workflow engine integration via drainer pattern

## What Must NOT Change

- `internal/types/` — all core types (Message, Task, ToolCall, StreamChunk, etc.)
- `internal/provider/` — LLMProvider interface, Registry, ChatRequest
- `internal/tools/` — Dispatcher, PermissionRequest/Response, Tool interface
- `internal/workflow/` — Engine, PhaseResult, PhaseGenerator, all message types
- `internal/config/` — Config struct and all sub-configs
- `internal/session/` (pkg/session/) — Manager, Session, Checkpoint
- `internal/errors/` — all sentinel errors
- `internal/git/` — Git operations
- `internal/tokens/` — Token estimation
- `pkg/` — ledger, rollback, arbitrage, bisect, autodream, session, keychain

</domain>

<decisions>
## Implementation Decisions

### D-01: Clean Slate — Delete All, Rewrite from Scratch
Delete every file in `internal/tui/`, `internal/tui/components/`, and `internal/tui/theme/`. Start from zero. No incremental migration.

### D-02: Maintain All External Interfaces
The TUI must consume the exact same types, interfaces, and method signatures from dependency packages. No changes to provider, tools, workflow, config, session, or types packages.

### D-03: Preserve All User-Facing Features
Every screen, slash command, keybinding, and visual element from the current TUI must exist in the rewrite. Features are not being removed — the code is being rewritten.

### D-04: Decompose AppState from Day One
The current AppState has 130+ fields (God Object). The rewrite decomposes into:
- `AppState` — core routing, screen management, theme, config, session
- `ProviderState` — active provider, model, registry, health, cache
- `WorkflowState` — engine, phase, context, drainer
- `ToolState` — dispatcher, permission queue, pending requests
Each sub-state is a struct嵌入d in AppState. Methods operate on the specific sub-state.

### D-05: Streaming Pipeline Architecture
```
Provider goroutine → StreamIterator.Next() → chan StreamMsg
  → drainer goroutine reads channel
  → wraps in tea.Cmd
  → AppState.Update() receives StreamMsg
  → ReplModel processes content/thinking/done
```
Channel lifecycle owned by the goroutine that creates it. TUI holds read-only reference.

### D-06: Permission Flow Architecture
```
tools.Dispatcher.Execute() → RiskLevel check
  → if dangerous/destructive:
      → sends PermissionRequest to Dispatcher.RequestCh()
      → blocks on internal responseCh
  → TUI reads from Dispatcher.RequestCh() via listener cmd
  → shows PermissionModal overlay
  → user decides Y/N/A
  → TUI sends PermissionResponse
  → Dispatcher unblocks, continues execution
```

### D-07: Workflow Engine Integration
```
AppState.initWorkflowEngine()
  → creates workflow.Engine
  → starts drainer goroutine (reads engine channel → tea.Msg)
  → engine runs phases in goroutines
  → phase results arrive as typed tea.Msg:
      PhaseSwitchMsg, PlanReadyMsg, TaskStartMsg, TaskCompleteMsg,
      TaskFailMsg, ShipSummaryMsg, StreamMsg, etc.
```

### D-08: Screen File Organization
Each screen follows the model/view decomposition pattern:
- `{screen}_model.go` — struct definition, New*(), Update()
- `{screen}_view.go` — View() rendering
- `{screen}_keys.go` — key bindings (if complex)
- `{screen}_state.go` — state management methods (if complex)

### D-09: Component Architecture
Reusable components in `internal/tui/components/`:
- ToolCard — tool execution display with state machine
- ThinkingBlock — collapsible thinking segment
- PermissionModal — permission request overlay with countdown
- MessageRenderer — Glamour-based markdown rendering
- Badge — styled pill labels
- ProgressBar — thin/thick/block/rounded indicators
- Sparkline — braille/block character data visualization
- FilterChips — horizontal filter toggle
- QuestionModel — tool question prompt
- MetricCard — large number + label display
- StatRow — label-value pair display

### D-10: Theme System
Theme struct with ~50 Lipgloss Style and Color fields:
- Background, Surface, SurfaceElevated
- Brand, BrandMuted
- TextPrimary, TextSecondary, TextMuted
- Success, Error, Warning, Info
- Thinking, ThinkingMuted
- Border, BorderActive
- Syntax highlighting colors
- Component-specific styles

ThemeManager wraps Theme with Cycle() for dark → light → auto.

### D-11: Test Strategy
Every screen gets a test file covering:
- Model initialization (New* returns valid model)
- Update() handles all relevant tea.Msg types
- View() returns non-empty string for all screen states
- Key bindings produce expected state changes
- Edge cases: nil safety, empty state, rapid input

Integration tests:
- Streaming pipeline end-to-end (mock provider)
- Permission flow (mock dispatcher)
- Workflow phase transitions (mock engine)
- Slash command routing

### D-12: Build Verification
After each wave:
- `go build ./internal/tui/...` must pass
- `go vet ./internal/tui/...` must exit 0

After all waves:
- `go test -race -count=1 ./...` must pass
- `CGO_ENABLED=0 go build ./cmd/m31a` must succeed for all 5 platforms

### the agent's Discretion
- Exact helper function signatures
- Whether to extract additional micro-helpers during implementation
- File placement for shared code within the TUI package
- Component API surface (which methods to expose)
- Specific lipgloss style values (exact colors, borders, padding)

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Core Types (DO NOT MODIFY)
- `internal/types/types.go` — Message, Task, ToolCall, StreamChunk, Usage, Session, ModelInfo, etc.
- `internal/types/constants.go` — All constants (timeouts, limits, thresholds)

### Provider Interface (DO NOT MODIFY)
- `internal/provider/interface.go` — LLMProvider, ChatRequest, StreamIterator, ToolDefinition

### Tool System (DO NOT MODIFY)
- `internal/tools/interface.go` — PermissionRequest, PermissionResponse, PermissionGate
- `internal/tools/dispatcher.go` — Dispatcher struct, RequestCh(), QuestionRequestCh()

### Workflow Engine (DO NOT MODIFY)
- `internal/workflow/engine.go` — Engine, RunPhase(), Transition(), SetMsgEmitter()
- `internal/workflow/engine_messages.go` — All tea.Msg types from engine

### Configuration (DO NOT MODIFY)
- `internal/config/types.go` — Config, ProviderConfig, ModelConfig, UIConfig, PermissionsConfig

### Session Management (DO NOT MODIFY)
- `pkg/session/` — Manager, Session, SessionInfo, Checkpoint, RecentModelsData

### Git Operations (DO NOT MODIFY)
- `internal/git/git.go` — Git struct, CommitInfo

### Current TUI (READ FOR REFERENCE, THEN DELETE)
- `internal/tui/` — All 89 files (reference only, will be deleted)
- `internal/tui/components/` — All component files (reference only, will be deleted)
- `internal/tui/theme/` — Theme files (reference only, will be deleted)

### Architecture Docs
- `docs/ARCHITECTURE.md` — Package dependency graph, data flow
- `docs/INTERFACES.md` — All Go interface definitions
- `docs/TYPES.md` — Constants, enums, error sentinels

</canonical_refs>

<specifics>
## Specific Ideas

### Screen Inventory (16 screens)

| Screen | Enum | Purpose | Key Dependencies |
|--------|------|---------|-----------------|
| GoalInput | 0 | Full-screen goal entry for workflow start | SessionManager, RecentModels |
| REPL | 1 | Main chat interface | Provider, Dispatcher, CmdRegistry, SessionManager |
| ModelSelector | 2 | Model browser overlay | Provider Registry, SessionManager |
| Settings | 3 | 6-tab settings panel | Config, SessionManager |
| Resume | 4 | Session browser with search | SessionManager |
| Permission | 5 | Tool permission modal overlay | Dispatcher |
| Plan | 6 | Task plan review | Workflow Engine |
| Execute | 7 | Task execution progress | Workflow Engine |
| Verify | 8 | Verification results | Workflow Engine |
| Ship | 9 | Session completion summary | Workflow Engine, Ledger |
| Diff | 10 | Git diff viewer | Git |
| Ledger | 11 | Learning journal browser | Ledger |
| Rollback | 12 | Commit rollback browser | Rollback |
| FirstRun | 13 | First-run onboarding | Config, SessionManager |
| Discuss | 14 | Q&A flow for discuss phase | Workflow Engine |
| Metrics | 15 | Flight data dashboard | SessionManager, Ledger |

### Message Types (22+)

| Type | Source | Purpose |
|------|--------|---------|
| StreamMsg | Provider goroutine | Content/thinking/done chunk |
| StreamDoneMsg | Provider goroutine | Stream completed with usage |
| StreamErrorMsg | Provider goroutine | Stream failed |
| PermissionRequestMsg | Dispatcher | Tool permission requested |
| PermissionResponseMsg | TUI modal | User approved/denied |
| PhaseSwitchMsg | Workflow engine | Phase transition |
| PlanReadyMsg | Workflow engine | Plan generated |
| TaskStartMsg | Workflow engine | Task execution started |
| TaskCompleteMsg | Workflow engine | Task completed |
| TaskFailMsg | Workflow engine | Task failed |
| ShipSummaryMsg | Workflow engine | Ship summary ready |
| DiscussQuestionsMsg | Workflow engine | Questions received |
| DiscussAnswerResultMsg | Workflow engine | Answer processed |
| SlashCommandMsg | REPL input | Slash command parsed |
| AppMsg | Various | Screen transitions, toast, reset |
| FallbackEventMsg | Provider | Provider fallback occurred |
| HealthCheckTickMsg | Ticker | Health check poll |
| HealthCheckResultMsg | Health goroutine | Health check response |
| ProviderModelsFetchedMsg | Model fetch goroutine | Model list updated |
| RefreshCacheMsg | Ticker | Cache refresh requested |
| CacheRefreshResultMsg | Cache goroutine | Cache refresh result |
| ToastExpiryMsg | Timer | Toast display expired |
| UpdateModelsListMsg | Various | Model selector refresh |
| WindowSizeMsg | Bubble Tea | Terminal resize |
| TeaKeyMsg | Bubble Tea | Key press |

### Slash Commands (30+)

| Command | Category | Handler |
|---------|----------|---------|
| /help | Core | commands_core.go |
| /clear | Core | commands_core.go |
| /reset | Core | commands_core.go |
| /version | Core | commands_core.go |
| /status | Core | commands_core.go |
| /model | AI | commands_ai.go |
| /context | AI | commands_ai.go |
| /think | AI | commands_ai.go |
| /compact | AI | commands_ai.go |
| /theme | Config | commands_config.go |
| /settings | Config | commands_config.go |
| /key | Config | commands_config.go |
| /set | Config | commands_config.go |
| /get | Config | commands_config.go |
| /export | Config | commands_config.go |
| /import | Config | commands_config.go |
| /disk usage | Config | commands_config.go |
| /sessions | Session | commands_session.go |
| /resume | Session | commands_session.go |
| /fork | Session | commands_session.go |
| /prev | Session | commands_session.go |
| /next | Session | commands_session.go |
| /archive | Session | commands_session.go |
| /commit | Git | commands_git.go |
| /diff | Git | commands_git.go |
| /log | Git | commands_git.go |
| /status (git) | Git | commands_git.go |
| /branch | Git | commands_git.go |
| /stash | Git | commands_git.go |
| /rollback | Git | commands_git.go |
| /workflow start | Workflow | commands_workflow.go |
| /workflow cancel | Workflow | commands_workflow.go |
| /workflow status | Workflow | commands_workflow.go |
| /plan | Workflow | commands_workflow.go |
| /verify | Workflow | commands_workflow.go |
| /ship | Workflow | commands_workflow.go |

### Keybinding Contexts (9)

| Context | Keys |
|---------|------|
| Global | ctrl+c, ?, Esc |
| REPL | enter, up, down, ctrl+u, tab, escape, pgup, pgdown, ctrl+d |
| Streaming | ctrl+c, t |
| Permission | y, n, a, Esc |
| Plan | a, e, r, d, Tab |
| Execute | p, r, s |
| Verify | h, s |
| ModelSelector | p, Tab, enter, Esc |
| Settings | Tab, enter, Esc, up, down |

</specifics>

<deferred>
## Deferred Ideas

- AppState decomposition into sub-states (Phase 29 preserves the single AppState but with cleaner internal organization)
- Ghost mode (V1.1 feature, not in scope)
- Terminal PiP (V1.1 feature, not in scope)
- Concurrent subagents (V1.1 feature, not in scope)
- New screen types (not in scope — rewrite existing 16)

</deferred>

---

*Phase: 29-complete-tui-rewrite*
*Context gathered: 2026-06-08 via comprehensive codebase analysis*
