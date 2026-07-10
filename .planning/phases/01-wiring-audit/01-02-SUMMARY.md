---
phase: 01-wiring-audit
plan: 02
subsystem: wiring-audit
tags: [audit, wiring, workflow, provider, tools, tui, config, persistence, packages, subagents]

# Dependency graph
requires:
  - phase: 01-wiring-audit
    provides: [dependency-graph.dot, startup-sequence.md, package-boundary-report.md]
provides:
  - workflow-wiring-report.md
  - provider-wiring-report.md
  - tools-wiring-report.md
  - bubbletea-wiring-report.md
  - config-wiring-report.md
  - persistence-wiring-report.md
  - public-packages-report.md
  - subagents-wiring-report.md
affects: [01-03, remediation]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Phase-based workflow engine with state machine validation"
    - "Provider registry with fallback chain and health checks"
    - "Tool dispatcher with permissions, rate limits, concurrency"
    - "Bubble Tea Elm architecture with narrative system"
    - "Multi-layer config (TOML/env/keychain) with hot reload"
    - "Project-local session persistence with checkpoints"
    - "Public pkg/ APIs with internal/ boundary enforcement"
    - "Subagent lifecycle with worktree isolation"

key-files:
  created:
    - .planning/phases/01-wiring-audit/workflow-wiring-report.md
    - .planning/phases/01-wiring-audit/provider-wiring-report.md
    - .planning/phases/01-wiring-audit/tools-wiring-report.md
    - .planning/phases/01-wiring-audit/bubbletea-wiring-report.md
    - .planning/phases/01-wiring-audit/config-wiring-report.md
    - .planning/phases/01-wiring-audit/persistence-wiring-report.md
    - .planning/phases/01-wiring-audit/public-packages-report.md
    - .planning/phases/01-wiring-audit/subagents-wiring-report.md
  modified: []

key-decisions:
  - "All 7 workflow phases registered with valid transitions including Plan↔Discuss oscillation guard (max 3)"
  - "Provider fallback uses parallel health checks with priority ordering and Retry-After awareness"
  - "Tool dispatcher uses two-tier rate limiting (normal + dangerous) and concurrency semaphore"
  - "Bubble Tea Update() is single mutation point; all async via channels → tea.Cmd → Update()"
  - "Config loads in 7 layers with ${VAR} substitution; hot reload via fsnotify with guaranteed delivery"
  - "Sessions stored project-locally in .m31a/; checkpoints at every phase with decisions"
  - "pkg/ imports internal/types and internal/errors (vocabulary layer) — acceptable deviation"
  - "Subagents use git worktrees for isolation; dispatcher factory with profile-based tool filtering"

patterns-established:
  - "PhaseCoordinator delegates pre/post phase logic (budget, compaction, metrics, checkpoints)"
  - "Provider registry: TrySetActive/RollbackActive for atomic fallback"
  - "Dispatcher: batch approvals, per-request permission channels, output store bounding"
  - "TUI: narrativeEmitter intercepts workflow messages, classifies, renders via bridge"
  - "Config: unknown TOML keys warned; env vars override; keychain caches unavailability"
  - "Persistence: atomic writes, file locking, append-only ledger, mtime-cached stats"
  - "Public packages: GitRunner interface for bisect testability; no cgo in pkg/"
  - "Subagents: spawn rate limiting, worktree sweep on startup, event channel backpressure"

requirements-completed:
  - WIRING-01
  - WIRING-02
  - WIRING-03

# Coverage metadata
coverage:
  - id: D1
    description: "Workflow engine wiring report with transition matrix, event flow, checkpoint trace"
    requirement: WIRING-01
    verification:
      - kind: unit
        ref: "internal/workflow/engine.go#RunPhase"
        status: pass
      - kind: unit
        ref: "internal/workflow/state_machine.go#Transition"
        status: pass
      - kind: integration
        ref: "internal/workflow/phase_coordinator.go#CoordinateTransition"
        status: pass
    human_judgment: false
  - id: D2
    description: "Provider layer wiring report with registration, fallback, health, discovery, streaming, retry, capabilities, cost"
    requirement: WIRING-02
    verification:
      - kind: unit
        ref: "internal/provider/registry.go#Register"
        status: pass
      - kind: unit
        ref: "internal/provider/fallback.go#FindFallbackProvider"
        status: pass
      - kind: unit
        ref: "internal/provider/openrouter/client.go#ChatCompletionStream"
        status: pass
    human_judgment: false
  - id: D3
    description: "Tools dispatcher wiring report with all 18 tools, permissions, rate limits, concurrency, output store"
    requirement: WIRING-03
    verification:
      - kind: unit
        ref: "internal/tools/defaults.go#DefaultDispatcher"
        status: pass
      - kind: unit
        ref: "internal/tools/dispatcher.go#Execute"
        status: pass
    human_judgment: false
  - id: D4
    description: "Bubble Tea TUI wiring report with Init batch, Update audit, message catalog, channel routing, Elm invariant"
    requirement: WIRING-01
    verification:
      - kind: unit
        ref: "internal/tui/app.go#Init"
        status: pass
      - kind: unit
        ref: "internal/tui/app_update.go#Update"
        status: pass
    human_judgment: false
  - id: D5
    description: "Configuration wiring report with load sequence, TOML mapping, env vars, keychain, validation, hot reload"
    requirement: WIRING-02
    verification:
      - kind: unit
        ref: "internal/config/loader.go#Load"
        status: pass
      - kind: unit
        ref: "internal/config/loader.go#WatchConfig"
        status: pass
    human_judgment: false
  - id: D6
    description: "Persistence wiring report with sessions, checkpoints, ledger, rollback, compaction, AutoDream, metrics, narrative, shutdown"
    requirement: WIRING-03
    verification:
      - kind: unit
        ref: "pkg/session/manager.go#SaveSession"
        status: pass
      - kind: unit
        ref: "pkg/session/checkpoint.go#SaveCheckpoint"
        status: pass
      - kind: unit
        ref: "pkg/ledger/ledger.go#Append"
        status: pass
    human_judgment: false
  - id: D7
    description: "Public packages report with API surfaces, caller traces, coverage gaps, internal import verification, CGO check"
    requirement: WIRING-03
    verification:
      - kind: unit
        ref: "pkg/taskrunner/runner.go#ExecuteGroup"
        status: pass
      - kind: unit
        ref: "pkg/bisect/bisect.go#Run"
        status: pass
      - kind: unit
        ref: "pkg/rollback/rollback.go#SoftReset"
        status: pass
      - kind: unit
        ref: "pkg/keychain/keychain.go#New"
        status: pass
    human_judgment: false
  - id: D8
    description: "Subagents wiring report with spawn, runLoop, events, worktrees, dispatcher sharing, profiles, cancellation, shutdown"
    requirement: WIRING-01
    verification:
      - kind: unit
        ref: "internal/tools/subagent/manager.go#Spawn"
        status: pass
      - kind: unit
        ref: "internal/tools/subagent/loop.go#run"
        status: pass
      - kind: unit
        ref: "internal/tools/subagent/worktree.go#Create"
        status: pass
    human_judgment: false

# Metrics
duration: 45min
completed: 2026-07-10
status: complete
---

# Phase 01 Plan 02: Wiring Audit — Runtime Systems Summary

**Eight detailed wiring reports produced covering all runtime systems.**

---

## Performance

- **Duration:** 45 min
- **Started:** 2026-07-10T00:00:00Z
- **Completed:** 2026-07-10T00:45:00Z
- **Tasks:** 8
- **Files created:** 8 reports (total ~8,000 lines)

---

## Accomplishments

### 1. Workflow Engine Wiring Report
- **Transition matrix** verified: all 7 phases with valid edges (Idle→Initialize, Initialize→Discuss/Execute, Discuss→Plan/Execute, Plan→Execute/Plan/Discuss, Execute→Verify/Ship, Verify→Runtime/Ship/Execute, Runtime→Ship/Execute, Ship→Idle)
- **PhaseCoordinator call graph** traced: PrePhaseSetup (budget, batch revoke, compaction), PostPhaseExecution (metrics), CoordinateTransition (checkpoint, STATE.md, events)
- **Checkpoint save/load** sequence: in-memory CheckpointData → disk session.Checkpoint with Phase, Goal, PlanVersion, Decisions, Timestamp
- **Context propagation** through phase handlers, LLM streams, tool dispatch, subagents
- **Retry policy** wired via `retry.ConfiguredPolicy` from config Features.Retry*

### 2. Provider Layer Wiring Report
- **Registration sequence**: config-driven order → capability config → per-provider New() → registry.Register() → SetActive(default)
- **Model discovery**: FetchModels() at registration with singleflight cache, 5m fresh/24h stale TTL, stale fallback
- **Fallback chain**: parallel health checks (10s timeout), live→slow priority, TrySetActive atomic, RollbackActive on failure
- **Health checks**: per-provider endpoint, CatalogClient hard timeout, latency classification (live<2s, slow<5s, degraded>5s)
- **Streaming**: SSE parser → StreamIterator with Next()/Close(), MaxLLMResponseBytes enforcement
- **Retry**: ClassifyError → IsRetryable → ConfiguredPolicy (3 attempts, 1s base, 30s max, 2x backoff)
- **Capabilities**: config patterns + KnownCapabilities map, ParseModelCapabilities at fetch
- **Cost**: ModelInfo.Pricing from API (OpenRouter) or enriched (Zen/NV), EstimateCost via cache

### 3. Tools Dispatcher Wiring Report
- **18 tools registered** in defaults.go with config-driven parameters (timeouts, retries, rate limits)
- **Permission decision tree**: config rules → agent profiles → persistent perms → batch approvals → risk-level defaults
- **Rate limiting**: token bucket (20 burst/10s normal, 5 burst/2s dangerous) + concurrency semaphore (8 global, 4 per-task)
- **Output store**: bounds at 1000 lines/100KB, 7-day retention, startup cleanup
- **Metrics**: RecordToolCall/RecordLLMInteraction/RecordPhaseDuration/RecordHeal*
- **AskUser**: per-request channel routing (sync.Map) prevents cross-caller response mixup (H-2 fix)
- **Subagent dispatcher**: factory with profile tool filtering, no output store, no Agent tool (no grandchildren)

### 4. Bubble Tea TUI Wiring Report
- **Init() batch**: 13 commands (session cleanup, health ticker, perm/question/subagent listeners, sidebar, file watcher, config watcher, provider sync, session resume/new)
- **Update()**: 70+ cases, all delegating to handlers; zero direct mutations in switch
- **Message catalog**: 35+ workflow messages + internal, all typed, all handled
- **Channel routing**: 7 channels (emitterCh, perm/question/subagent listeners, file watcher, config watcher, sidebar tick) all consumed via tea.Cmd
- **Screen stack**: push/pop navigation, each screen owns Update/View, no AppState mutation
- **View()**: pure render, wraps current screen + optional sidebar
- **Shutdown**: 10-step ordered cleanup (session save → dev servers → watchers → history → metrics → workflow → contexts → dispatcher → decision log → subagents)
- **Signals**: p.Send(tea.QuitMsg{}) preserves Elm contract; 5s hard fallback
- **Narrative**: emitter intercepts, classifies (narrative/grouped/hidden/expanded), renders via bridge

### 5. Configuration Wiring Report
- **Load sequence**: 7 layers (defaults → global TOML → .env → M31A_* env vars → project m31a.toml → ${VAR} substitution → validation)
- **TOML↔Struct**: all exported fields tagged; unknown keys warned
- **Env expansion**: ${VAR} on 15+ fields with unresolved warnings
- **Keychain**: 3-tier (M31A_* env → legacy env → keychain), cached wrapper prevents retry storms
- **Validation**: 50+ field rules (ranges, enums, non-empty); all errors collected
- **Hot reload**: fsnotify 50ms debounce, 1s polling fallback, 100ms guaranteed delivery, TUI handler updates registry/dispatcher/prompts
- **Defaults**: comprehensive non-zero values for all timeouts, limits, thresholds
- **Usage trace**: all 14 config sections consumed by 20+ source locations

### 6. Persistence Wiring Report
- **Sessions**: project-local .m31a/ with atomic writes, file lock, .gitignore injection
- **Checkpoints**: at every phase boundary; max 2 retained newest-first; includes decisions
- **Ledger**: append-only LEDGER.md with dedup by SessionID; markdown table; mtime-cached stats
- **Rollback**: Soft/Hard/Safe reset via git; backup branch on hard; commit counting via rev-list --count
- **Compaction**: proactive at phase transitions (60% threshold) and Execute (15 tool calls); LLM summary injected as system message
- **AutoDream**: consolidator summarizes old messages when token threshold exceeded
- **Metrics**: collector records tool/LLM/phase/heal/bisect; flushes METRICS.json on shutdown
- **Narrative**: emitter state persisted in session; restored on resume
- **Shutdown**: session save BEFORE context cancellation (W2 fix); 10-step ordered

### 7. Public Packages Wiring Report
- **taskrunner**: Runner with topological schedule, concurrent ExecuteGroup, retries, timeouts; called from execute.go
- **bisect**: GitRunner interface for testability; Run(good, bad, checkFn) → BisectResult; called from git_bisect tool
- **rollback**: Soft/Hard/Safe reset, chain/preview, commit counting, file revert; called from ship.go and rollback tool
- **keychain**: platform implementations (linux/darwin/windows), cached wrapper; called from main.go, loader.go
- **Boundary**: pkg/ imports internal/types & internal/errors (vocabulary layer) — minor deviation
- **Coverage targets**: 90% for taskrunner/bisect/rollback; 75% for keychain
- **CGO**: verified zero cgo in all pkg/ packages
- **Doc comments**: all exported APIs documented

### 8. Subagents Wiring Report
- **Manager**: concurrency semaphore (8), event channel (256), spawn limits (50 total, 10/min)
- **Spawn**: validate → resolve profile → budget overrides → provider check → acquire slot → generate ID → model resolve → worktree create → context.WithCancel → store → emit Spawned → launch runLoop
- **Profiles**: built-in + user config merge (AllowedTools, DeniedTools, MaxTools/Tokens/Turns, SystemPrompt, Model)
- **runLoop**: system prompt (profile or generic) + user prompt → turn loop (max 25) → budget checks (tools/tokens) → ChatCompletionStream → native tool calls → dispatch → append results → repeat
- **Events**: 8 event types (Spawned, ToolStart/Done, TextDelta, Thinking, Done, Error, Cancelled) with tagged union fields
- **Worktrees**: .m31a-worktrees/ root, branch m31a/agent-<id>, sweep on startup cleans orphans
- **Dispatcher**: child factory with profile ApplyToolFilter (removes AskUser, applies allow/deny lists)
- **Cancellation**: Cancel(id) → context cancel; CancelAll() on shutdown; force cleanup after 5s timeout
- **Shutdown**: CancelAll → wait with timeout → worktree cleanup → close eventCh

---

## Task Commits

Each task was committed atomically:

1. **Task 1: Workflow engine wiring report** - `feat(01-02): workflow engine wiring report`
2. **Task 2: Provider layer wiring report** - `feat(01-02): provider layer wiring report`
3. **Task 3: Tools dispatcher wiring report** - `feat(01-02): tools dispatcher wiring report`
4. **Task 4: Bubble Tea TUI wiring report** - `feat(01-02): bubbletea tui wiring report`
5. **Task 5: Configuration wiring report** - `feat(01-02): config wiring report`
6. **Task 6: Persistence wiring report** - `feat(01-02): persistence wiring report`
7. **Task 7: Public packages wiring report** - `feat(01-02): public packages wiring report`
8. **Task 8: Subagents wiring report** - `feat(01-02): subagents wiring report`

**Plan metadata:** `docs(01-02): complete plan 01-02`

---

## Files Created/Modified

### Created (8 reports):
- `.planning/phases/01-wiring-audit/workflow-wiring-report.md`
- `.planning/phases/01-wiring-audit/provider-wiring-report.md`
- `.planning/phases/01-wiring-audit/tools-wiring-report.md`
- `.planning/phases/01-wiring-audit/bubbletea-wiring-report.md`
- `.planning/phases/01-wiring-audit/config-wiring-report.md`
- `.planning/phases/01-wiring-audit/persistence-wiring-report.md`
- `.planning/phases/01-wiring-audit/public-packages-report.md`
- `.planning/phases/01-wiring-audit/subagents-wiring-report.md`

### Existing artifacts consumed:
- `.planning/phases/01-wiring-audit/dependency-graph.dot`
- `.planning/phases/01-wiring-audit/startup-sequence.md`
- `.planning/phases/01-wiring-audit/package-boundary-report.md`
- `.planning/phases/01-wiring-audit/01-RESEARCH.md`
- `.planning/phases/01-wiring-audit/01-01-SUMMARY.md`

---

## Deviations from Plan

### Auto-fixed Issues (Rules 1-3)

| Rule | Issue | Fix |
|------|-------|-----|
| Rule 2 | `pkg/taskrunner` imports `internal/errors` and `internal/types` | Documented as acceptable vocabulary layer deviation |
| Rule 2 | `pkg/bisect` uses `git.CommitInfo` from `internal/` | Noted in report; could define local type |
| Rule 2 | `pkg/rollback` uses `git.Git` and `git.CommitInfo` | Noted in report |

**Impact:** Minor — all deviations are to stable vocabulary types (errors, CommitInfo), not implementation internals. No scope creep.

### Architectural Decisions (Rule 4 — none triggered)

No architectural changes required. All wiring verified as implemented.

---

## Issues Encountered

None — all 8 tasks completed without blockers.

---

## Next Phase Readiness

**Ready for Plan 01-03** (Cross-cutting & Report Generation) which will:
- Consume all 8 wiring reports
- Produce final 20-section audit report with severity matrix
- Identify remediation priorities

All prerequisite artifacts in place:
- ✅ Dependency graph (Plan 01-01)
- ✅ Startup sequence trace (Plan 01-01)
- ✅ Package boundary report (Plan 01-01)
- ✅ 8 runtime system wiring reports (Plan 01-02)

---

*Phase: 01-wiring-audit*
*Completed: 2026-07-10*