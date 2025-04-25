# M31A Comprehensive Integrity Audit — Phase 0 through Phase 8

**Date:** 2026-05-30
**Auditor:** Qoder CLI (automated multi-agent audit)
**Scope:** Every file, function, code path, test, and cross-package contract across all 9 phases

---

## Executive Summary

| Metric | Value |
|--------|-------|
| Total gaps found | **68** |
| Critical severity | **3** |
| High severity | **14** |
| Medium severity | **30** |
| Low severity | **21** |
| Orphaned functions | **31** |
| Dead message types | **3** |
| Dead types | **2** |
| Missing test files | **4** |
| Failing tests | **1** |
| Overall test coverage | **70.0%** (target: 75%) |
| Test files | **51** |
| Production LOC | **14,766** |
| Test LOC | **15,988** |

**Overall Health Score: 6/10** — Core workflow functions but 3 critical gaps block end-to-end reliability.

### Severity Distribution

```
Critical  ███ 3
High      ██████████████ 14
Medium    ██████████████████████████████ 30
Low       █████████████████████ 21
```

### Critical Issues (Must Fix Before v1.0)

1. **STREAM-001:** Streaming only processes the first LLM response chunk — all subsequent chunks are lost
2. **ORPHAN-001:** `pkg/arbitrage` entire package is dead code — zero production importers
3. **WORKFLOW-001:** `healTask()` never called during execute phase — self-heal loop is a no-op retry

---

## Per-Phase Gap Reports

### Phase 0 — Foundation

| # | Severity | Description | File:Line |
|---|----------|-------------|-----------|
| P0-GAP-1 | Low | `noopWriter` type is dead code — never instantiated in production | `internal/log/log.go:122-128` |
| P0-GAP-2 | Medium | `resolveLogLevel()` only supports "debug" and defaults to "info" — `warn` and `error` silently treated as `info` | `internal/log/log.go:115-120` |
| P0-GAP-3 | Medium | CI build job never runs binary smoke test (e.g., `./m31a --version`) | `.github/workflows/ci.yml:48-49` |
| P0-GAP-4 | Low | `TestRotateLogFiles_RenamesOldFile` test fails — log rotation not working as tested | `internal/log/log_test.go:170` |

**Phase 0 Health: 8/10** — Foundation is solid, log rotation test needs fixing.

---

### Phase 1 — Provider Abstraction Layer

| # | Severity | Description | File:Line |
|---|----------|-------------|-----------|
| P1-GAP-1 | High | `ModelCacheRefreshTicker` type and all methods are orphaned — never used in production | `internal/provider/cache.go:85-142` |
| P1-GAP-2 | Medium | `FetchModels` swallows all errors on stale fallback — no logging, no error return, no visibility | `internal/provider/openrouter/client.go:77-96` |
| P1-GAP-3 | Medium | `FetchModels` returns stale data on `http.NewRequestWithContext` failure — request construction errors indistinguishable from network failures | `internal/provider/openrouter/client.go:77-96` |
| P1-GAP-4 | Low | Zen provider correctly omits `HTTP-Referer` and `X-Title` headers (unlike OpenRouter) — correct but fragile if Zen ever requires them | `internal/provider/zen/client.go:143-145` |
| P1-GAP-5 | Medium | Zero tests exercise `ReasoningEnabled: true` path in `ChatCompletionStream` for either provider | `internal/provider/openrouter/client_test.go` |
| P1-GAP-6 | Medium | `SSEParser.Next()` loses scanner errors between events — partial SSE events returned without caller knowing connection is broken | `internal/provider/sse.go:27-46` |
| P1-GAP-7 | Low | `ParseSSEChunk` returns `nil, nil` for missing choices — silent skip that could hide malformed SSE payloads | `internal/provider/reasoning.go:114-127` |

**Phase 1 Health: 7/10** — Provider abstraction works but error handling is opaque and refresh ticker is dead code.

---

### Phase 2 — TUI Foundation

| # | Severity | Description | File:Line |
|---|----------|-------------|-----------|
| P2-GAP-1 | Medium | `AppMsg.InitError` field is never set by any production code — dead field | `internal/tui/types.go:33` |
| P2-GAP-2 | **Critical** | `handleStreamMsg` returns `nil` cmd — streaming stops after first chunk (see STREAM-001 below) | `internal/tui/repl.go:405` |
| P2-GAP-3 | High | `ScreenPlan`, `ScreenExecute`, `ScreenVerify`, `ScreenShip` cases discard `tea.Cmd` via `_ = cmds` — no async effects possible | `internal/tui/app.go:752-797` |
| P2-GAP-4 | Medium | `AppMsg.SessionID` set by ResumeModel but never used to load session data into REPL | `internal/tui/app.go:730-734` |
| P2-GAP-5 | Medium | `SettingsSavedMsg` handler restarts tickers but does not reload config into active components (provider, model, theme, API key) | `internal/tui/app.go:658-667` |
| P2-GAP-6 | Low | `HealthCheckInterval` defined in both `types/constants.go` and `tui/health.go` — duplicate constants | `internal/types/constants.go:7`, `internal/tui/health.go:11` |
| P2-GAP-7 | Medium | `ReplModel.fallbackBanner` expiry check creates 1-frame race between `Update()` and `View()` | `internal/tui/repl.go:140-142`, `internal/tui/repl.go:584` |
| P2-GAP-8 | Medium | `ReplModel.fallbackBanner` dismissed on ANY key (including spinner ticks, window resizes) when textarea has content — overly aggressive | `internal/tui/repl.go:322-326` |
| P2-GAP-9 | High | Four TUI screen models have zero test files: `plan_test.go`, `execute_test.go`, `verify_test.go`, `ship_test.go` | `internal/tui/` |

**Phase 2 Health: 5/10** — Streaming is broken (critical), workflow screens can't produce effects, session resume doesn't load history.

---

### Phase 3 — Message Rendering Pipeline

| # | Severity | Description | File:Line |
|---|----------|-------------|-----------|
| P3-GAP-1 | High | `thinkingBlocks` and `toolCards` maps on `ReplModel` are dead state — initialized to nil, only set back to nil, never populated | `internal/tui/repl.go:44-45` |
| P3-GAP-2 | High | `getToolCallsFromSegments()` is a stub that always returns `nil` — ToolCalls never extracted from segments | `internal/tui/repl.go:475-477` |
| P3-GAP-3 | Medium | Goroutine-side segment building in `StartStreamCmd` is dead code for REPL path — `repl.go` overwrites segments | `internal/tui/streaming.go:49-76` |
| P3-GAP-4 | Medium | Thinking chunks create one `MessageSegment` per delta instead of accumulating — fragmented segments | `internal/tui/streaming.go:107-121` |
| P3-GAP-5 | Low | `TestToolCard_AutoCollapse_TruncatedOutput` has O(n²) string mutation — performance hazard | `internal/tui/components/toolcard_test.go:135-138` |
| P3-GAP-6 | Medium | `thinking` segment `DurationMs` not propagated from stream to final message in goroutine path | `internal/tui/streaming.go:116-121` |

**Phase 3 Health: 6/10** — Tool call extraction is broken, thinking block state is dead.

---

### Phase 4 — Tool System

| # | Severity | Description | File:Line |
|---|----------|-------------|-----------|
| P4-GAP-1 | Medium | Dispatcher `requestCh` buffer size 1 silently drops concurrent permission requests — DoS for parallel tool calls | `internal/tools/dispatcher.go:74-78` |
| P4-GAP-2 | Low | `extractCommandString` only handles Bash and FileWrite — FileRead/Glob/Grep would show raw JSON if marked dangerous | `internal/tools/dispatcher.go:139-183` |
| P4-GAP-3 | Medium | Dispatcher `permissions` map only remembers "allowed", not "denied" — re-prompts on every call | `internal/tools/dispatcher.go:91-95` |
| P4-GAP-4 | Medium | `globWithRG` returns ripgrep-relative paths but output table joins them with `workDir` — produces absolute paths when `useRG=true` | `internal/tools/glob.go:88` |
| P4-GAP-5 | Low | `isDBusUnavailable` always returns `true` — masks genuine D-Bus errors | `pkg/keychain/keychain_linux.go:274-281` |
| P4-GAP-6 | Medium | `grepPureGo` defers `f.Close()` inside `filepath.Walk` callback — file handles leak until walk completes | `internal/tools/grep.go:213` |

**Phase 4 Health: 7/10** — Tools function but permission memory and file handle leaks are concerns.

---

### Phase 5 — Session State & Configuration

| # | Severity | Description | File:Line |
|---|----------|-------------|-----------|
| P5-GAP-1 | Medium | Config types reference V1.1 features (`AutodreamEnabled`, `SubagentEnabled`, `GhostConfig`) that AGENTS.md prohibits — dead config fields | `internal/config/types.go:57-70` |
| P5-GAP-2 | Low | `ResolveAPIKeys` uses `fmt.Printf` instead of structured slog logger | `internal/config/loader.go:103,116` |
| P5-GAP-3 | Medium | Windows keychain lacks `validateService` function — inconsistent error types vs Linux/Darwin | `pkg/keychain/keychain_windows.go` |
| P5-GAP-4 | Medium | `Session.Project` field is never populated — always nil despite being saved/loaded separately | `internal/session/session.go:14` |
| P5-GAP-5 | Medium | `SessionInfo.LastModified` reflects only `session.json` mtime, not message updates during streaming | `internal/session/manager.go:236-238` |
| P5-GAP-6 | Low | `config/loader_test.go` mock returns generic error instead of `keychain.ErrKeyNotFound` — fallback path untested | `internal/config/loader_test.go:19-24` |
| P5-GAP-7 | High | `tokens/estimator.go` fallback formula has incorrect operator precedence — overestimates for short strings | `internal/tokens/estimator.go:55` |
| P5-GAP-8 | High | Entire `internal/tokens` package is orphaned — `NewEstimator` called but estimator never used (see ORPHANED list) | `internal/tokens/estimator.go` |

**Phase 5 Health: 6/10** — Token estimator is orphaned, config has V1.1 bleed, Windows keychain inconsistent.

---

### Phase 6 — Six-Phase Workflow Engine

| # | Severity | Description | File:Line | Previously Fixed? |
|---|----------|-------------|-----------|-------------------|
| P6-GAP-1 | **High** | `healTask()` never called from `executeTaskWithTools()` — self-heal loop increments `HealsAttempted` but does `streamLLM()` retry (no-op), never invokes actual heal | `internal/workflow/execute.go:99-170` | N |
| P6-GAP-2 | Medium | `transitionToPlan()` in discuss.go is orphaned — duplicates `FinalizeDiscuss()` → `e.Transition()` | `internal/workflow/discuss.go:112` | N |
| P6-GAP-3 | Medium | `DiscussResult` type is defined but never used in production | `internal/workflow/discuss.go:124` | N |
| P6-GAP-4 | Medium | `CollectAnswers()` helper is orphaned — only used in tests | `internal/workflow/discuss.go:131` | N |
| P6-GAP-5 | Medium | `OnTaskStart`/`OnTaskUpdate` callbacks on `Runner` never wired in production — TUI progress updates for individual tasks never emitted | `pkg/taskrunner/runner.go:30-31` | N |
| P6-GAP-6 | Low | `execCommand()` is package-level function — not mockable in tests | `internal/workflow/engine.go:812` | N |
| P6-GAP-7 | Medium | Engine's `appendLedgerEntry()` writes markdown headers, `ledger` package writes markdown tables — incompatible formats | `internal/workflow/engine.go:715-734` | N |
| P6-GAP-8 | Low | `runDiscuss()` returns `NeedsAnswers: true` — workflow engine cannot proceed past Discuss without TUI coordination (by design) | `internal/workflow/discuss.go:16-56` | N |

**Fix Verification (13 previously claimed fixes):**

| Fix | Description | Status |
|-----|-------------|--------|
| Fix 1 | Initialize auto-transitions to Discuss | VERIFIED — `Transition()` called |
| Fix 2 | Discuss Q&A collection wired | VERIFIED — `DiscussState`, `SubmitDiscussAnswer`, `SkipDiscuss`, `FinalizeDiscuss` exist |
| Fix 3 | Plan retry feeds errors back to LLM | VERIFIED — `validationErrors` + `rawResponse` in `buildPlanContext` |
| Fix 4 | Self-heal in execute phase | PARTIAL — heal loop exists but never calls `healTask()` (P6-GAP-1) |
| Fix 5 | sessionStartHash tracked | VERIFIED — `SetGit` captures HEAD |
| Fix 6 | 3rd targeted heal after bisect | VERIFIED — `verify.go` calls `healTask` with diff |
| Fix 7 | Manual task entry fallback | VERIFIED — `RequiresManualInput` on plan failure |
| Fix 8 | PROJECT.md in execute context | VERIFIED — loaded in `buildExecuteContext` |
| Fix 9 | PlanReadyMsg defined | VERIFIED — defined in types.go but NEVER EMITTED or HANDLED (dead type) |
| Fix 10 | TaskStartMsg/TaskUpdateMsg callbacks | NOT VERIFIED — callbacks defined but never wired (P6-GAP-5) |
| Fix 11 | MEMORY.md in plan context | VERIFIED — loaded in `buildPlanContext` |
| Fix 12 | Commit messages, git add -A | VERIFIED |
| Fix 13 | Integration test runs all 6 phases | VERIFIED — `integration_test.go` exists |

**Phase 6 Health: 6/10** — 10 of 13 fixes verified, but self-heal (Fix 4) is incomplete and task callbacks (Fix 10) are unwired.

---

### Phase 7 — Signature Features

| # | Severity | Description | File:Line |
|---|----------|-------------|-----------|
| P7-GAP-1 | **Critical** | `pkg/arbitrage` entire package orphaned — zero production importers. `AutoArbitrage` config flag exists but nothing acts on it | `pkg/arbitrage/arbitrage.go` |
| P7-GAP-2 | High | `ledger.Append()`, `NewEntry()`, `Truncate()`, `Reload()` never called in production — engine uses incompatible format | `pkg/ledger/ledger.go` |
| P7-GAP-3 | Medium | `SettingsModel.saveCmd()` is orphaned — never called, Ctrl+S handler duplicates logic inline | `internal/tui/settings.go:391` |
| P7-GAP-4 | Medium | `SettingsSavedMsg` handler does not re-fetch models or update `m.activeModel` when default model changes | `internal/tui/app.go:658-667` |
| P7-GAP-5 | Low | `rollback.Chain()` sets `HasCheckpoint: false` for all entries — field hardcoded, never computed | `pkg/rollback/rollback.go:80` |
| P7-GAP-6 | Low | `rollback.SafeReset()` is defined and tested but never called from TUI commands | `pkg/rollback/rollback.go:159` |
| P7-GAP-7 | Medium | `autodream.Consolidator` never instantiated in production — `/compress` always returns "AutoDream not available" | `internal/tui/commands.go:325-335` |
| P7-GAP-8 | Low | 16 slash commands registered (out of 28+ planned): help, clear, status, model, provider, reset, quit, undo, compress, ledger, rollback, sessions, goal, phase, config, models, fallback | `internal/tui/commands.go:139-155` |

**Phase 7 Health: 5/10** — Arbitrage completely dead, ledger writes bypass the ledger package, autodream uninstantiated, 12 slash commands missing.

---

### Phase 8 — Polish, Testing & v1.0 Release

| # | Severity | Description | File:Line | From Phase 8 Prompt? |
|---|----------|-------------|-----------|---------------------|
| P8-GAP-1 | Medium | `.goreleaser.yaml` lacks `brews:` section — `brew install eshanized/tap/m31a` in README is not automated | `.goreleaser.yaml` | Y |
| P8-GAP-2 | Medium | `CHANGELOG.md` claims "~24,000 LOC" and "50+ test files" — actual: 14,766 prod LOC, 51 test files | `CHANGELOG.md:29` | N |
| P8-GAP-3 | Low | `install.sh` doesn't handle `VERSION=latest` correctly for pre-release GitHub releases | `install.sh` | Y |
| P8-GAP-4 | Low | `--help` output lacks six-phase workflow description and slash command overview | `cmd/m31a/main.go:26-37` | Y |
| P8-GAP-5 | High | Test coverage is 70.0% — below the 75% target from P8.2 | Coverage report | Y |
| P8-GAP-6 | Medium | `TestRotateLogFiles_RenamesOldFile` fails — breaks `go test ./...` | `internal/log/log_test.go:170` | N |
| P8-GAP-7 | Medium | No `CONTRIBUTING.md` exists | — | Y |

**Phase 8 Health: 5/10** — Coverage below target, failing test, missing docs, release pipeline incomplete.

---

## Orphaned Code List

### Entire Orphaned Packages

| Package | Functions | Coverage | Notes |
|---------|-----------|----------|-------|
| `pkg/arbitrage` | 8 (NewScorer, Score, EstimateTokens, CompareModels, Recommend, ShouldArbitrage, boostLevel, classifyText) | 94.2% | Config flags exist but feature never wired |

### Orphaned Constructors (New functions never called)

| Function | Defined At | Notes |
|----------|-----------|-------|
| `pkg/autodream.New` | `pkg/autodream/autodream.go:40` | Only called from tests |
| `pkg/rollback.New` | `pkg/rollback/rollback.go:38` | Only called from tests |
| `internal/tokens.NewEstimator` | `internal/tokens/estimator.go:35` | Called in app.go but estimator never used |

### Orphaned Package-Level Functions

| Function | Defined At | Notes |
|----------|-----------|-------|
| `internal/log.DefaultLogger` | `internal/log/log.go:55` | Only called from tests |
| `internal/workflow.CollectAnswers` | `internal/workflow/discuss.go:131` | TUI handles Q&A instead |
| `internal/provider.NewModelCacheRefreshTicker` | `internal/provider/cache.go:96` | TUI uses `tea.Every()` instead |
| `pkg/ledger.NewEntry` | `pkg/ledger/ledger.go:98` | Engine writes directly to file |

### Orphaned Methods

| Method | Defined At | Notes |
|--------|-----------|-------|
| `(*Engine).transitionToPlan` | `internal/workflow/discuss.go:112` | Duplicates `FinalizeDiscuss` |
| `(*Engine).BuildSummary` | `internal/workflow/ship.go:106` | `runShip` builds inline |
| `(*Estimator).Estimate` | `internal/tokens/estimator.go:47` | Estimator never used |
| `(*Estimator).Calibrate` | `internal/tokens/estimator.go:66` | Estimator never used |
| `(*Estimator).FormatUsage` | `internal/tokens/estimator.go:86` | Estimator never used |
| `(*Estimator).ContextWarningBanner` | `internal/tokens/estimator.go:101` | Estimator never used |
| `(*ModelCache).FetchTime` | `internal/provider/cache.go:69` | Unused accessor |
| `(*Git).DiffStaged` | `internal/git/git.go:166` | Unused |
| `(*Git).CurrentBranch` | `internal/git/git.go:202` | Unused |
| `(*Git).WorkDir` | `internal/git/git.go:258` | Unused |
| `(*Git).AbsPath` | `internal/git/git.go:263` | Unused |
| `theme.Dark` | `internal/tui/theme/theme.go:48` | Only used in tests |
| `theme.Light` | `internal/tui/theme/theme.go:174` | Only used in tests |
| `(*Session).SetPhase` | `pkg/session/session.go:34` | Only used in tests |
| `(*Rollback).CurrentHead` | `pkg/rollback/rollback.go:88` | Only used in tests |
| `(*Rollback).SafeReset` | `pkg/rollback/rollback.go:159` | Only used in tests |
| `(*Ledger).Append` | `pkg/ledger/ledger.go:122` | Engine bypasses this |
| `(*Ledger).Reload` | `internal/tui/ledger.go:445` | Only used in tests |
| `(*ModelCacheRefreshTicker).Interval` | `internal/provider/cache.go:107` | Entire type orphaned |
| `(*ModelCacheRefreshTicker).Tick` | `internal/provider/cache.go:113` | Entire type orphaned |
| `(*ModelCacheRefreshTicker).Stop` | `internal/provider/cache.go:133` | Entire type orphaned |
| `(*SettingsModel).saveCmd` | `internal/tui/settings.go:391` | Ctrl+S duplicates logic |

### Orphaned Types

| Type | Defined At | Notes |
|------|-----------|-------|
| `internal/workflow.DiscussResult` | `internal/workflow/discuss.go:124` | Phase returns `*PhaseResult` instead |
| `internal/log.noopWriter` | `internal/log/log.go:124` | Compile-time interface check only, never instantiated |

---

## Message Flow Matrix

| # | Message Type | Emitted At | Handled At | Status |
|---|---|---|---|---|
| 1 | `HealthCheckTickMsg` | `health.go:25`, `health.go:34` | `app.go:426` | OK |
| 2 | `ErrorMsg` | `settings.go:409` | `app.go:505`, `settings.go:500` | OK |
| 3 | `PermissionRequestMsg` | `app.go:262` | `app.go:548` | OK |
| 4 | `PermissionResponseMsg` | `app.go:300` | `app.go:556` | OK |
| 5 | `FallbackEventMsg` | `app.go:536` (conditional) | `app.go:496`, `repl.go:328` | OK |
| 6 | `ThinkingToggleMsg` | `repl.go:296` | `repl.go:335` | OK |
| 7 | `RefreshCacheMsg` | `cache.go:20`, `cache.go:33` | `app.go:444` | OK |
| 8 | `ModelSelectedMsg` | `modelselector.go:239` (wrapped in AppMsg) | `app.go:468` | OK |
| 9 | `SettingsSavedMsg` | `settings.go:411`, `settings.go:570` | `settings.go:495`, `app.go:658` | OK |
| 10 | `PhaseResultMsg` | `app.go:227,233,238` | `app.go:562` | OK |
| 11 | **`PlanReadyMsg`** | **NOWHERE** | **NOWHERE** | **DEAD** |
| 12 | **`TaskStartMsg`** | **NOWHERE** | **NOWHERE** | **DEAD** |
| 13 | **`TaskUpdateMsg`** | **NOWHERE** | **NOWHERE** | **DEAD** |
| 14 | `StreamMsg` | `streaming.go:95` | `repl.go:167` | OK |
| 15 | `StreamDoneMsg` | `streaming.go:78` | `repl.go:170` | OK (but may not be read due to stream bug) |
| 16 | `StreamErrorMsg` | `streaming.go:44,87` | `repl.go:173`, `app.go:509` | OK |
| 17 | `TickMsg` | `streaming.go:135` | `repl.go:176` | OK |
| 18 | `AppMsg` | Multiple locations | `app.go:466` | OK |

**3 dead messages:** `PlanReadyMsg`, `TaskStartMsg`, `TaskUpdateMsg` — defined in `internal/tui/types.go:88-104` but never emitted or handled.

---

## Interface Contract Audit

### `LLMProvider` Interface (`internal/provider/interface.go`)

| Method | OpenRouter | Zen | Used in Production |
|--------|-----------|-----|-------------------|
| `ChatCompletionStream` | Implemented | Implemented | Yes (`repl.go`, `streaming.go`) |
| `FetchModels` | Implemented | Implemented | Yes (`cache.go`, `modelselector.go`) |
| `GetModel` | Implemented | Implemented | Yes |
| `EstimateCost` | Implemented | Implemented | No (only arbitrage, which is orphaned) |
| `HealthCheck` | Implemented | Implemented | Yes (`health.go`) |
| `Name` | Implemented | Implemented | Yes |

### `Tool` Interface (`internal/tools/interface.go`)

| Method | Bash | FileRead | FileWrite | Glob | Grep | Used |
|--------|------|----------|-----------|------|------|------|
| `Name` | Yes | Yes | Yes | Yes | Yes | Yes |
| `Description` | Yes | Yes | Yes | Yes | Yes | Yes |
| `Parameters` | Yes | Yes | Yes | Yes | Yes | Yes |
| `Execute` | Yes | Yes | Yes | Yes | Yes | Yes (via dispatcher) |
| `RiskLevel` | Yes | Yes | Yes | Yes | Yes | Yes |

### `Dispatcher` Interface

- `Execute()` — called from `execute.go` workflow phase
- `SetPermissionHandler()` — called in `app.go` permission listener
- All methods used in production

---

## Dead Code List

### Unused Imports

None detected — all imports in non-test files are used.

### Unused Types (besides those listed above)

| Type | Location | Notes |
|------|----------|-------|
| `DiscussResult` | `internal/workflow/discuss.go:124` | Defined, never constructed |
| `noopWriter` | `internal/log/log.go:124` | Interface check only |

### Unused Config Fields

| Field | Location | Notes |
|-------|----------|-------|
| `FeaturesConfig.AutodreamEnabled` | `internal/config/types.go:57` | V1.1 feature |
| `FeaturesConfig.SubagentEnabled` | `internal/config/types.go:58` | V1.1 feature |
| `GhostConfig.Enabled` | `internal/config/types.go:67` | V1.1 feature |

---

## Roadmap Deviation Summary

| Roadmap Item | Actual Status | Deviation |
|--------------|---------------|-----------|
| Streaming: token-by-token progressive append | Only first chunk processed | **Critical deviation** |
| Tool execution: tool cards update in real-time | Tool calls never extracted from segments | High deviation |
| Slash commands: 28+ commands | 16 commands registered | Medium deviation |
| Arbitrage: auto-complexity scoring | Package orphaned, never wired | High deviation |
| Ledger: Append/dedup/truncate in production | Engine writes directly, bypassing ledger package | High deviation |
| AutoDream: context consolidation | Consolidator never instantiated | High deviation |
| Rollback: SafeReset with auto-stash | SafeReset defined but never called from TUI | Low deviation |
| Token estimation: EMA calibration | Estimator instantiated but never used | Medium deviation |
| Test coverage: 75% overall | 70.0% | Medium deviation |
| Test files: 50+ | 51 | OK |
| Homebrew tap via goreleaser | No `brews:` section in `.goreleaser.yaml` | Medium deviation |

---

## Priority Fix List

Ordered by severity × impact, with estimated effort.

### P0 — Critical (Block v1.0 Release)

| # | Issue | Effort | Impact |
|---|-------|--------|--------|
| 1 | **STREAM-001: Streaming stops after first chunk** — `handleStreamMsg` returns `nil` cmd | Medium (2-4h) | Entire LLM response truncated to first fragment |
| 2 | **ORPHAN-001: `pkg/arbitrage` dead package** — Either wire it up or delete it | Low (1h) | 8 functions, 94% coverage, zero users |
| 3 | **WORKFLOW-001: Self-heal never calls `healTask()`** — Execute phase retries with same context instead of healing | Medium (2-3h) | Failed tasks cannot self-correct during execution |

### P1 — High (Feature Completeness)

| # | Issue | Effort | Impact |
|---|-------|--------|--------|
| 4 | Wire `OnTaskStart`/`OnTaskUpdate` callbacks in `execute.go` | Low (1h) | TUI task progress updates never emitted |
| 5 | Wire `PlanReadyMsg`, `TaskStartMsg`, `TaskUpdateMsg` emission and handling | Medium (3-4h) | Task lifecycle invisible to TUI |
| 6 | Fix `getToolCallsFromSegments()` stub — extract tool calls from segments | Medium (2-3h) | Tool execution pipeline broken |
| 7 | Wire `autodream.Consolidator` instantiation in `NewApp()` | Low (1h) | `/compress` always returns "not available" |
| 8 | Wire `ledger.Append()` in engine instead of `appendLedgerEntry()` | Medium (2h) | Ledger package writes never used |
| 9 | Fix `handleStreamMsg` to schedule continuation cmd for stream | See #1 | Same root cause |
| 10 | Fix `ScreenPlan/Execute/Verify/Ship` cases to not discard `tea.Cmd` | Low (1h) | Workflow screens cannot produce async effects |
| 11 | Fix `AppMsg.SessionID` to load session into REPL on resume | Medium (2h) | Session resume shows empty conversation |
| 12 | Add 4 missing test files: `plan_test.go`, `execute_test.go`, `verify_test.go`, `ship_test.go` | High (6-8h) | Zero coverage for workflow screen models |

### P2 — Medium (Code Quality / Fragility)

| # | Issue | Effort | Impact |
|---|-------|--------|--------|
| 13 | Fix `TestRotateLogFiles_RenamesOldFile` failing test | Low (1h) | `go test ./...` fails |
| 14 | Delete orphaned `ModelCacheRefreshTicker` type | Low (30min) | Dead code, 58 lines |
| 15 | Delete orphaned `DiscussResult`, `CollectAnswers`, `transitionToPlan` | Low (30min) | Dead code |
| 16 | Fix `SettingsSavedMsg` handler to reload active config | Medium (2h) | Settings changes not reflected until restart |
| 17 | Fix `grepPureGo` file handle leak — replace defer with explicit close | Low (30min) | FD exhaustion on large directories |
| 18 | Fix `resolveLogLevel()` to support warn/error levels | Low (30min) | Config values silently ignored |
| 19 | Fix token estimator formula operator precedence | Low (30min) | Overestimation for short strings |
| 20 | Delete or wire `thinkingBlocks`/`toolCards` maps on ReplModel | Low (30min) | Dead state |
| 21 | Add tests for `ReasoningEnabled: true` streaming path | Medium (2h) | Untested reasoning feature |
| 22 | Fix `SSEParser.Next()` to check scanner errors immediately | Low (1h) | Partial events on broken connections |
| 23 | Fix `FetchModels` error swallowing — return or log errors | Medium (2h) | No visibility into fetch failures |
| 24 | Add `brews:` section to `.goreleaser.yaml` | Low (1h) | Homebrew install not automated |
| 25 | Create `CONTRIBUTING.md` | Low (1h) | Missing from Phase 8 checklist |
| 26 | Delete V1.1 config fields (`AutodreamEnabled`, `SubagentEnabled`, `GhostConfig`) | Low (30min) | Prohibited by AGENTS.md |

### P3 — Low (Cosmetic / Documentation)

| # | Issue | Effort | Impact |
|---|-------|--------|--------|
| 27 | Delete `noopWriter` type | Low (15min) | Dead code, 7 lines |
| 28 | Delete duplicate `tui.HealthCheckInterval` constant | Low (15min) | Could drift |
| 29 | Fix `--help` output to describe workflow and slash commands | Low (1h) | Poor first-run experience |
| 30 | Fix CHANGELOG.md LOC/test file claims | Low (15min) | Inaccurate |
| 31 | Fix `install.sh` VERSION=latest handling | Low (30min) | Fails on pre-release |
| 32 | Fix `TestToolCard_AutoCollapse_TruncatedOutput` O(n²) loop | Low (15min) | Slow test if limits change |
| 33 | Replace `fmt.Printf` with slog in `ResolveAPIKeys` | Low (30min) | Inconsistent logging |
| 34 | Add `validateService` to Windows keychain | Low (1h) | Inconsistent error handling |

---

## Verification Results

```
Build (CGO_ENABLED=0):    PASS
Vet:                      PASS
Tests:                    FAIL (1 failing test)
  - TestRotateLogFiles_RenamesOldFile (internal/log)
Coverage:                 70.0% (target: 75%)
Test files:               51
Production LOC:           14,766
Test LOC:                 15,988
```

### Coverage by Key Package

| Package | Coverage | Target | Status |
|---------|----------|--------|--------|
| `pkg/taskrunner` | 98.2% | 90% | PASS |
| `pkg/bisect` | (included in workflow) | 90% | See below |
| `pkg/rollback` | 92.0% | 90% | PASS |
| `pkg/arbitrage` | 94.2% | — | Dead package |
| `pkg/ledger` | 85.6% | — | Writes not used |
| `pkg/session` | 84.7% | — | OK |
| `internal/workflow` | (varies) | — | OK |
| `internal/tui` | (varies) | — | Missing 4 test files |
| **TOTAL** | **70.0%** | **75%** | **FAIL** |

---

## Recommendations

### Immediate (Before v1.0)

1. **Fix streaming** (STREAM-001) — This is the single most critical bug. The LLM response is truncated to the first chunk. The fix requires restructuring `StartStreamCmd` to either loop internally or have `handleStreamMsg` return a continuation cmd.

2. **Fix self-heal** (WORKFLOW-001) — The execute phase's heal loop is a no-op retry. `healTask()` must be called within `executeTaskWithTools()`.

3. **Fix failing test** — `TestRotateLogFiles_RenamesOldFile` breaks `go test ./...`.

4. **Arbitrage decision** — Either wire `pkg/arbitrage` into the TUI/workflow or delete the package. Having it as dead code with 94% test coverage is misleading.

### Short-Term (Post-v1.0)

5. Wire task lifecycle messages (`PlanReadyMsg`, `TaskStartMsg`, `TaskUpdateMsg`)
6. Wire `autodream.Consolidator` and `ledger.Append()`
7. Add 4 missing test files for workflow screens
8. Fix `SettingsSavedMsg` handler to reload config
9. Reach 75% coverage target

### Cleanup

10. Delete all orphaned types, functions, and config fields listed above
11. Fix error handling in provider `FetchModels`
12. Add SSE reasoning tests
13. Complete release pipeline (goreleaser brews, CONTRIBUTING.md, --help)

---

*End of Audit Report*
