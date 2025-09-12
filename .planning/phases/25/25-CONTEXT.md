# Phase 25 Context: Comprehensive Wiring & Inconsistency Fixes

**Gathered:** 2026-06-06
**Status:** Ready for planning
**Source:** `rush/comprehensive_wiring_inconsistency_report.md` (66 findings: 10 critical, 39 warnings, 17 info)

---

## Goal

Fix all 66 findings from the comprehensive wiring inconsistency report. The audit covered 148 Go source files across 23 packages using 6 parallel deep-audit agents. The findings span 10 critical (feature breakage, data corruption, build failure), 39 warnings (operator precedence, dead sentinels, incomplete schemas, architecture violations, listener goroutine leaks), and 17 info items (clean dependency graph, vet/test pass, reasoning normalization wired).

## Reference Material

- **Target Report:** `rush/comprehensive_wiring_inconsistency_report.md`
- **Related Phases:** Phase 14 (TUI↔Core Wiring), Phase 15 (Deep Audit), Phase 17 (Post-Audit), Phase 19 (Concerns), Phase 20 (Internal Wiring), Phase 21 (TUI Logical), Phase 22 (Hardcoded Values), Phase 23 (Function Simplification), Phase 24 (TUI Redesign)

## Critical Findings (P0 — 1-2 hrs each)

| ID | File | Issue |
|----|------|-------|
| CR-01 | `internal/workflow/engine_parse.go:448-449` | `normalizeToolName` maps `edit`→`FileEdit` but tool registers as `Edit`; Edit unreachable |
| CR-02 | `internal/tools/grep.go:59` vs `:114` | Schema declares `include` param, code reads `glob`; filter always ignored |
| CR-05 | `internal/tui/execute.go:223-231` | N tool calls → N duplicated assistant messages in history |
| CR-08 | `internal/tui/commands_config.go:400-401` | `syscall.Statfs_t`/`syscall.Statfs` used without `//go:build`; Windows build fails |
| CR-03 | `internal/workflow/engine.go` | `Engine.tokens` populated but never consulted; context protection disabled |
| CR-04 | `internal/tui/app_view.go:40-41` | `View()` mutates state via `SetKeyRegistry`/`SetLastActivity`; violates Bubble Tea contract |
| CR-06 | `internal/tui/app.go:356` | `config.WatchConfig` goroutine never cancelled; leaks fd |
| CR-07 | `internal/tui/permissions.go`, `question.go` | `permissionListenerCmd`/`questionListenerCmd` block on channels never closed |
| CR-09 | `internal/config/loader.go` → `tools/dispatcher.go` | Permission rules parsed but never injected into dispatcher; security feature dead |
| CR-10 | `internal/workflow/ship.go` | Ship phase archives session before writing STATE.md; crash leaves no state |

## Warning Findings (P1 — 2-4 hrs each, P2 — 1-3 hrs each)

| ID | Severity | File | Issue |
|----|----------|------|-------|
| W-01 | P1 | `provider/registry.go:74` | `Registry.Get()` returns `ErrProviderUnreachable` for missing; should be `ErrProviderNotFound` |
| W-02 | P2 | `provider/cache.go:46-63` | Singleflight dedup dead; both clients bypass `cache.Refresh()` with `cache.Set()` |
| W-03 | P2 | `tui/health.go:12-13` | `HealthCheckTicker` has unused `registry`/`activeProvider` parameters |
| W-04 | P1 | `provider/registry.go:46` | `SetActive("")` wraps `ErrProviderNotFound`; should use `ErrInvalidProvider` |
| W-05 | P2 | `provider/fallback.go:19,105` | `maxRetryAfter` comment says 60s, constant is 120s |
| W-06 | P1 | `provider/common.go:34` | `IsContextExceeded` has implicit operator precedence; needs parens |
| W-07 | P2 | `provider/openrouter/client.go:79`, `zen/client.go:75` | `ResponseHeaderTimeout=30s` may abort slow reasoning streams |
| W-08 | P2 | `provider/openrouter/client.go:212`, `zen/client.go:201` | SSE parser uses `context.Background()` instead of request ctx |
| W-09 | P2 | `tui/components/toolcard.go:36` | `ToolIcons` map key `"Question"` doesn't match `AskUserQuestion` name |
| W-10 | P2 | `provider/registry.go:25` | `Register()` returns bare `fmt.Errorf`; inconsistent with `SetActive` |
| W-11 | P2 | `provider/openrouter/client.go:197` | OpenRouter missing `ErrNoCredits` handling for HTTP 402 (Zen has it) |
| W-12 | P2 | `tui/repl_stream.go` | Pending stream chunks only flushed on `PhaseIdle`; intermediate transitions lose chunks |
| W-13 | P2 | `tui/commands_workflow.go` | Discuss→Plan transition skips engine transition guard |
| W-14 | P2 | `tui/commands_workflow.go` | `finalizeDiscussAndAdvance` silently ignores transition errors |
| W-15 | P2 | `tui/plan.go` | Plan screen shows but workflow stalls (no kickoff) |
| W-16 | P2 | `workflow/engine_parse.go` | `extractJSONObject` scans wrong indices after `stripJSONComments` |
| W-17 | P2 | `tui/commands_ai.go` | Discuss timer doesn't validate question index (stale fire) |
| W-18 | P2 | `tui/app_workflow.go`, `workflow/ship.go` | Ship phase double-writes workflow state |
| W-19 | P2 | `workflow/engine_parse.go` | `normalizeToolName` missing TodoWrite/AskUserQuestion snake_case |
| W-20 | P2 | `tools/permissions.go` | `matchAnyParamValue` only checks `path`/`url`/`command`/`pattern` |
| W-21 | P2 | `tools/fileread.go`, `filewrite.go` | Return bare errors without `ErrToolExecution` wrap |
| W-22 | P2 | `tools/edit.go`, `todo.go`, `question.go`, `webfetch.go` | Missing `ParameterSchema()`; LLM sees `{}` |
| W-23 | P2 | `tui/components/toolrenderers.go:112` | `RendererForTool` missing WebFetch/AskUserQuestion cases |
| W-24 | P3 | `docs/INTERFACES.md` | Stale vs `types.go`; phantom/missing fields |
| W-25 | P1 | `tools/bash.go`, `fileread.go`, `filewrite.go` | Import `internal/config`; architecture violation |
| W-26 | P2 | `tui/components/permission.go` | Imports `internal/tools`; tight coupling |
| W-27 | P2 | `config/loader.go` | `pkg/keychain` import undocumented |
| W-28 | P3 | `errors/errors.go:35` | `ErrBisectFailed` dead code |
| W-29 | P3 | `errors/errors.go:32` | `ErrStreamTruncated` dead code |
| W-030 | P3 | `tui/firstrun.go:798` | Returns `fmt.Errorf("invalid API key")` without `ErrInvalidKey` |
| W-031 | P3 | `tui/commands_ai.go` | `discussQuestionsTimeout` doesn't validate active question |
| W-032 | P3 | `tools/webfetch.go` | Double-resolves DNS (TOCTOU risk) |
| W-033 | P3 | `tools/webfetch.go` | Hardcodes 5MB instead of `MaxFileSize` constant |

## Locked Decisions

1. **P0 fixes are mandatory** — feature breakage, build failure, data corruption block release
2. **Architecture rules are non-negotiable** — `internal/tools` may only import `internal/types` + `internal/errors`; remove `internal/config` imports
3. **Bubble Tea single-thread invariant** — `View()` must be pure; all state mutations in `Update()`
4. **Sentinel error discipline** — every wrapped error uses `errors.Is`; dead sentinels removed or wired
5. **No breaking changes** — fixes maintain backward compatibility with sessions/configs from prior phases
6. **Test every P0/P1 fix** — regression tests for CR-01 through CR-10
7. **All 5 build targets must compile** — linux/amd64, linux/arm64, darwin/amd64, darwin/arm64, windows/amd64

## Wave Structure (from Priority Matrix)

| Wave | Findings | Complexity | Hours | Blocking |
|------|----------|------------|-------|----------|
| 1 (P0) | CR-01, CR-02, CR-05, CR-08 | Medium | 1-2 each | All on critical path |
| 2 (P1) | CR-03, CR-04, CR-09, CR-10, W-01, W-06, W-25 | High | 2-4 each | Security/correctness |
| 3 (P2) | CR-06, CR-07, W-02, W-07, W-09, W-19, W-20, W-22 | Medium | 1-3 each | Resource leaks, UX |
| 4 (P3 + docs + verify) | W-03, W-04, W-05, W-08, W-10, W-11, W-12-W-18, W-21, W-23-W-33, I-* | Low-Medium | 0.5-1 each | Quality, consistency |

## Key Files to Modify

| File | Findings |
|------|----------|
| `internal/workflow/engine.go` | CR-03 (Engine.tokens consultation) |
| `internal/workflow/engine_parse.go` | CR-01 (normalizeToolName), W-16, W-19 |
| `internal/workflow/ship.go` | CR-10 (write state before archive), W-18 |
| `internal/tools/edit.go` | CR-01 (Name() returns "Edit"), W-22 (ParameterSchema) |
| `internal/tools/grep.go` | CR-02 (param key) |
| `internal/tools/permissions.go` | CR-09 (dispatcher integration), W-20 (param coverage) |
| `internal/tools/fileread.go`, `filewrite.go` | W-21 (ErrToolExecution), W-25 (config import) |
| `internal/tools/bash.go` | W-25 (config import) |
| `internal/tools/todo.go`, `question.go`, `webfetch.go` | W-22 (ParameterSchema), W-32, W-033 |
| `internal/tools/webfetch.go` | W-32 (DNS), W-033 (MaxFileSize) |
| `internal/tools/dispatcher.go` | CR-09 (Rules injection point) |
| `internal/tui/app_view.go` | CR-04 (View() purity) |
| `internal/tui/app.go` | CR-06 (configWatchCancel on quit) |
| `internal/tui/execute.go` | CR-05 (message history dedup) |
| `internal/tui/commands_config.go` | CR-08 (build tags for Statfs) |
| `internal/tui/permissions.go`, `question.go` | CR-07 (listener cleanup) |
| `internal/tui/plan.go` | W-15 (kickoff) |
| `internal/tui/commands_workflow.go` | W-13, W-14 (transition guard) |
| `internal/tui/commands_ai.go` | W-17, W-031 (timer validation) |
| `internal/tui/repl_stream.go` | W-12 (chunk flush) |
| `internal/tui/firstrun.go` | W-030 (ErrInvalidKey) |
| `internal/tui/components/toolcard.go` | W-09 (icon key) |
| `internal/tui/components/toolrenderers.go` | W-23 (renderer coverage) |
| `internal/tui/components/permission.go` | W-26 (tight coupling) |
| `internal/tui/health.go` | W-03 (unused params) |
| `internal/provider/registry.go` | W-01, W-04, W-10 (sentinels) |
| `internal/provider/cache.go` | W-02 (singleflight) |
| `internal/provider/common.go` | W-06 (precedence) |
| `internal/provider/fallback.go` | W-05 (comment) |
| `internal/provider/openrouter/client.go` | W-07, W-08, W-11 |
| `internal/provider/zen/client.go` | W-07, W-08 |
| `internal/config/loader.go` | CR-09 (rules injection), W-27 (keychain doc) |
| `internal/errors/errors.go` | W-28, W-29 (dead sentinels) |
| `docs/INTERFACES.md` | W-24 (stale doc) |

## Verification

After all fixes:
- `go build -o m31a ./cmd/m31a` succeeds on linux/amd64, linux/arm64, darwin/amd64, darwin/arm64, windows/amd64
- `go vet ./...` exits 0
- `go test -race -count=1 ./...` passes
- `golangci-lint run ./...` exits 0
- `grep -rn 'package tools' --include='*.go' | xargs grep -l '"github.com/...M31A/internal/config"' || true` returns empty
- Edit tool: `normalizeToolName("edit")` returns `"Edit"`; dispatcher finds it
- Grep tool: LLM-sent `{"include": "*.go"}` actually filters
- Token estimation: requests exceeding 95% context blocked
- View() is pure: no AppState mutations
- Task execution: 1 assistant message + N tool results, not N duplicates
- Ship phase: STATE.md written before archive move
- Windows build: `GOOS=windows go build ./...` succeeds
- Permission rules: `config.toml` rules applied during tool dispatch
- All sentinels used; no dead `errors.go` entries
- All tools implement `ParameterSchema()` returning valid JSON Schema
- `RendererForTool("WebFetch")` and `RendererForTool("AskUserQuestion")` return non-generic renderers

## Out of Scope (Deferred)

- Auto-trigger for `pkg/autodream` (manual `/compress` only — by design per AGENTS.md)
- Auto-trigger for `pkg/bisect` (manual only — by design)
- Ledger context injection during Initialize (manual mode — by design)
- V1.1 features (ghost mode, PiP, subagents) — Phase 9
- New tool additions beyond current set
- New providers beyond OpenRouter + OpenCode Zen
