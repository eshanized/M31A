# Phase 7: Signature Features — Context

**Gathered:** 2026-05-28
**Status:** Ready for planning
**Source:** PHASE7_PROMPTS.md (detailed implementation specs for all P7.* sub-components)

<domain>
## Phase Boundary

Phase 7 delivers all "signature features" that differentiate M31A from basic CLI tools: model selector UI, cost-aware model arbitrage, cross-session learning ledger, commit rollback chain, AutoDream context consolidation, slash command system, and settings screen with inline editing. Also includes gap fixes for fallback notifications, thinking block T key binding, and background model cache refresh.

**Depends on:** Phase 6 (Workflow Engine), Phase 5 (Session State & Configuration), Phase 2 (TUI Foundation)

**Duration:** 3 weeks | **Complexity:** 7/10 | **Milestone:** All 26 acceptance criteria pass

### Sub-components

1. **P7.1 — Model Selector UI** (`internal/tui/modelselector.go` + tests)
   - Full-screen overlay with bubbles/list; provider filter cycles All→OpenRouter→Zen via `P` key
   - Real-time fuzzy search over ModelInfo.Name and ModelInfo.ID
   - Detail pane on Tab; same model on two providers shown as separate entries
   - Uses lipgloss styling, provider.EstimateCost() for pricing display

2. **P7.2 — Cost-Aware Model Arbitrage** (`pkg/arbitrage/arbitrage.go` + tests)
   - Complexity scoring: Simple (<2000 tokens), Moderate (2000-8000), Complex (>8000)
   - Model cost comparison with sorting by total cost ascending
   - Recommend() function: scores task, estimates tokens, compares models, returns recommendation with reasoning
   - ShouldArbitrage() threshold comparison for cost-aware decisions

3. **P7.3 — Cross-Session Learning Ledger** (`pkg/ledger/ledger.go` + tests)
   - Parse LEDGER.md at `~/.m31a/LEDGER.md`
   - Filtered queries (by provider, model, date range, limit, offset)
   - Aggregate stats (total sessions, tasks, avg duration, success rate, top models/providers)
   - Atomic append via temp-file + rename

4. **P7.4 — Commit Rollback Chain** (`pkg/rollback/rollback.go` + tests)
   - Chain browsing (last N commits with diffs, checkpoint markers)
   - Three reset modes: SoftReset (staged), HardReset (discard), SafeReset (stash+reset+pop)
   - User-friendly RollbackResult messages
   - Checkpoint-awareness via timestamps

5. **P7.5 — AutoDream Context Consolidation** (`pkg/autodream/autodream.go` + tests)
   - ContextInfo computation from message Usage data
   - Consolidation at threshold (default 0.60), warning at 0.80
   - Preserve system messages, last 5 messages, tool call messages
   - Summary generation with "[AutoDream Context Summary]" prefix

6. **P7.6 — Slash Command System** (`internal/tui/commands.go` + tests)
   - ParseCommand() / CommandRegistry / CommandHandler pattern
   - 16 commands: help, clear, status, model, provider, reset, quit, undo, compress, ledger, rollback, sessions, goal, phase, config, models
   - Pure functions returning CommandResult — no direct AppState mutation

7. **P7.7 — Settings Screen Inline Editing** (`internal/tui/settings.go`)
   - 2 new tabs (Model=4, Ledger=5); total 6 tabs
   - Inline editing via textinput.Model; e=edit, space=toggle, a=add, d=delete
   - General tab: edit MaxIterations, ShowTokenUsage, ShowCostEstimate
   - Provider tab: edit DefaultProvider, masked API key with keychain
   - Permissions tab: edit DefaultMode, add/delete rules
   - Features tab: toggle AutodreamEnabled, SubagentEnabled, AutoBackup, ResumeOnStartup, AutoArbitrage
   - Model tab: browse models, set default
   - Ledger tab: recent sessions table, aggregate stats

8. **Gap Fixes:**
   - FallbackEvent TUI emission: notification banner + dismiss with `x` key
   - Thinking block `T` key binding toggle
   - Background ModelCacheRefreshTicker at 5-minute interval
</domain>

<decisions>
## Implementation Decisions

### P7.6 — Slash Command System
- Commands are pure functions returning `CommandResult` with desired state changes
- TUI's `Update()` applies `CommandResult` (screen transitions, config saves)
- Use `strings.Fields()` for arg splitting, `strings.TrimPrefix(input, "/")` for command name
- Error messages: `"unknown command: /foo. Type /help for available commands."`
- CommandContext provides environment: Registry, SessionManager, Config, Dispatcher, Git

### P7.4 — Commit Rollback Chain
- Chain() limits to 20 commits by default, entries newest-first (HEAD at index 0)
- All reset methods stash first if uncommitted changes exist (data loss prevention)
- Preview capped at 50,000 chars (types.BashOutputLimit)
- Checkpoint comparison via timestamps

### P7.2 — Cost-Aware Model Arbitrage
- Complexity scoring keywords: Simple (fix/add/update/change/rename), Moderate (implement/create/refactor/restructure), Complex (design/architect/migrate/rewrite/system)
- Token ranges: Simple (1000-3000 in, 500-1500 out), Moderate (3000-8000 in, 1500-4000 out), Complex (8000-20000 in, 4000-10000 out)
- Models with missing pricing handled gracefully (skip or $0.00)
- Cost: `inputTokens * inputPricePerToken + outputTokens * outputPricePerToken`

### P7.5 — AutoDream Context Consolidation
- Consolidation preserves: system messages, last 5 messages, tool call messages, initial goal/context
- Summary max 500 tokens; prefix: "[AutoDream Context Summary]"
- Token estimation for messages without Usage: words × 1.3

### P7.3 — Cross-Session Learning Ledger
- Parsing: Markdown table format with `|` delimiters, 9 columns. Header row: Session ID | Model | Provider | Tasks Done | Tasks Failed | Total Cost | Duration | Timestamp | Project Type. Each subsequent row is one entry. Append by appending a new row. (User-approved deviation from original CONTEXT.md spec — approved 2026-05-28)
- Malformed entries skipped gracefully
- Atomic write: temp file + rename in same directory

### P7.1 — Model Selector UI
- bubbles/list with custom DefaultDelegate
- Search case-insensitive on ID, name, description, provider
- Provider filter cycles through registered providers
- Cost display uses provider.EstimateCost() with standard usage (100K in, 50K out)

### P7.7 — Settings Screen
- Inline editing replaces current value display during edit
- API key fields masked as "****"
- Model and Ledger tabs show loading state during fetch
- dirty flag set on any field modification

### Gap Fixes
- FallbackEvent: notification banner at screen top, dismiss with `x`
- Thinking block toggle: `T` key in REPL, state persisted across messages
- Cache refresh: ModelCacheRefreshTicker at types.ModelCacheTTL (5 minutes)
</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Existing Interfaces & Types
- `internal/tui/types.go` — Screen enum, AppMsg, HealthUpdateMsg, ProviderSwitchMsg
- `internal/tui/app.go` — AppState struct with registry, provider, model, sessionManager, config, dispatcher
- `internal/tui/theme/theme.go` — Theme struct, Manager, ToolLabel map
- `internal/types/types.go` — ModelInfo, Task, Message, Usage, WorkflowPhase, CapFlags, Pricing
- `internal/provider/interface.go` — LLMProvider, ChatRequest, StreamIterator, ProviderRegistry
- `internal/provider/registry.go` — Registry with List(), Get(), ActiveProvider(), SetActive()
- `internal/provider/cache.go` — ModelCache with ttl, staleTTL, IsExpired(), IsStale()
- `internal/tools/interface.go` — Tool, ToolCall, ToolResult, RiskLevel, Dispatcher
- `internal/config/types.go` — Config with Provider, Model, UI, Permissions, Features, Ledger, Ghost sub-configs
- `internal/git/git.go` — Git struct with Log, ResetSoft, ResetHard, StashPush, HeadHash, Diff, Status
- `internal/errors/errors.go` — Sentinel errors

### Existing Packages
- `pkg/session/` — Manager, Session lifecycle
- `pkg/session/checkpoint.go` — SaveCheckpoint(), LoadCheckpoints(), LatestCheckpoint()
- `pkg/session/manager.go` — ListSessions() returning []SessionInfo
- `pkg/keychain/` — Keychain interface with Get/Set/Delete

### Documentation
- `docs/INTERFACES.md` — All Go interface definitions
- `docs/TYPES.md` — Constants, enums, sentinel errors
- `docs/ARCHITECTURE.md` — Package dependency graph
- `AGENTS.md` — Project build/test commands and conventions
- `PHASE7_PROMPTS.md` — Full detailed specs for all P7.* sub-components

### Prior Phase Context
- `.planning/phases/05/05-CONTEXT.md` — Settings screen base, keychain, session lifecycle decisions
- `.planning/phases/02/02-01-SUMMARY.md` — TUI types and theme foundation
- `.planning/STATE.md` — Current project state and key decisions
</canonical_refs>

<specifics>
## Specific Implementation Details

### Wave Structure
Based on dependency analysis from PHASE7_PROMPTS.md:

**Wave 1** (no dependencies, fully parallel):
- Plan 01: P7.6 — Slash Command System (`internal/tui/commands.go`)
- Plan 02: P7.4 — Commit Rollback Chain (`pkg/rollback/`)
- Plan 03: P7.2 — Cost-Aware Model Arbitrage (`pkg/arbitrage/`)

**Wave 2** (depends on Wave 1 patterns):
- Plan 04: P7.5 — AutoDream Context Consolidation (`pkg/autodream/`)
- Plan 05: P7.3 — Cross-Session Learning Ledger (`pkg/ledger/`)
- Plan 06: Gap Fixes (FallbackEvent, Thinking Toggle, Cache Refresh)

**Wave 3** (depends on P7.2 from Wave 1):
- Plan 07: P7.1 — Model Selector UI (`internal/tui/modelselector.go`)

**Wave 4** (depends on P7.1 + P7.3 from Waves 2+3):
- Plan 08: P7.7 — Settings Screen Updates (`internal/tui/settings.go`)

### Key Patterns to Follow
- All TUI components follow Bubble Tea model (Init/Update/View)
- New pkg/ packages use the existing sentinel errors from internal/errors/
- Tests use temporary directories/files where appropriate
- All settings/state modifications go through CommandResult or direct config mutation (never env vars at runtime)
</specifics>

<deferred>
## Deferred Ideas
- Full Tab-completion for slash commands (mentioned in ROADMAP but deferred from PHASE7_PROMPTS spec)
- Vision support, voice interaction, multi-modal outputs (V1.2+)
- Plugin system and team collaboration (post V1.1)
</deferred>

---

*Phase: 07-signature-features*
*Context gathered: 2026-05-28 via PHASE7_PROMPTS.md*
