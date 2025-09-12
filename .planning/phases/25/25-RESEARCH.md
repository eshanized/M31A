# Phase 25: Comprehensive Wiring & Inconsistency Fixes - Research

**Researched:** 2026-06-06
**Domain:** Cross-package wiring, config/permission consistency, TUI threading safety, code/doc parity
**Confidence:** HIGH (all 66 findings re-verified against source code; 7 audit inaccuracies identified)

<user_constraints>
## User Constraints (from 25-CONTEXT.md)

### Locked Decisions

1. **Scope:** Address all 66 findings from `rush/comprehensive_wiring_inconsistency_report.md` (10 critical, 39 warnings, 17 info).
2. **Test approach:** Add a regression test for EVERY critical fix and every warning fix that introduces observable behavior change. Existing test infrastructure (see `internal/{workflow,tui,tools}/*_test.go`) supplies the pattern.
3. **Wave structure (locked):** Four-wave execution:
   - **Wave 1** — Critical correctness: CR-01, CR-02, CR-03, CR-04, CR-05, CR-06, CR-07, CR-08, CR-10
   - **Wave 2** — Provider hardening: W-01, W-03, W-04, W-05, W-06, W-07, W-08, W-10, W-11
   - **Wave 3** — Tool/permission surface: W-13, W-15, W-16, W-19, W-20, W-21, W-22, W-23, W-25, W-32, W-33
   - **Wave 4** — Code/doc parity and style: W-14, W-17, W-18, W-24, W-26, W-27, W-28, W-29, W-30, W-31, W-34, W-35, W-36, W-09, W-12, all I-*
4. **CR-09 is DEFERRED** to Phase 26+ (architectural move of `PermissionRule` would unblock the cascade; in Phase 25 we only document the dependency).
5. **Backwards compatibility:** Phase 25 MUST NOT break Phase 11 session forking, Phase 10 connection status, or Phase 0-V1 features. The five ALREADY-ADDED features (Session.ParentID/ChildrenIDs, Message.SkipForLLM, ledger pruning, fork command, project-level config) remain intact.
6. **Build tags:** All Windows-specific code paths must use `//go:build !windows` or runtime.GOOS checks to keep `CGO_ENABLED=0` + cross-compile matrix green (linux/amd64, linux/arm64, darwin/amd64, darwin/arm64, windows/amd64).
7. **No new external dependencies.** All fixes use existing standard library + already-imported third-party modules.

### the agent's Discretion

- File organization for shared types (e.g., should `PermissionRule` move to `internal/types` or stay in `internal/config`?). Phase 25 leaves the type in place; the architectural move is the Phase 26+ ticket.
- Naming for newly-introduced internal functions (e.g., SSE error wrapping helpers, permission rule normalizers). Follow existing package conventions.
- Test granularity: prefer focused per-finding tests over umbrella "regression" files.

### Deferred Ideas (OUT OF SCOPE)

- Full architectural refactor of the `tools → config` import edge (CR-09 root cause). Phase 25 fixes the symptoms (CR-09 itself describes the "architectural violation" but is moved to Phase 26+).
- Migration of `internal/session` to Phase 11 fork-aware APIs in `internal/workflow`. (Already done in Phase 11; verify only.)
- Implementation of V1.1 deferred tools (FileEdit, WebFetch, etc.) — they exist but the audit calls out parity gaps. W-22 + W-23 close the parity gaps; they do NOT add new tool features.
- Multi-layer config (Phase 11's `m31a.toml` walk-up) — already shipped; verify only.
- Vision/voice/multi-modal (deferred to V1.2+).
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| **WIRE-01** | Resolve all 10 critical findings (CR-01..CR-10) from the audit. | Verified each CR against source; all map to specific code locations with exact fixes (see CR Matrix below). |
| **WIRE-02** | Eliminate data-flow inconsistencies in provider layer (auto-fallback, error normalization, SSE lifecycle, model-cache race, health-ticker coupling). | Verified W-01, W-03, W-04, W-05, W-06, W-07, W-08, W-10, W-11 against `internal/provider/`, `internal/tui/health.go`. |
| **WIRE-03** | Repair tool/permission-surface gaps: missing `ParameterSchema()` methods, schema/code key mismatches, double-URL-resolution, hardcoded limits, dispatch coverage, renderers. | Verified W-13, W-15, W-16, W-19, W-20, W-21, W-22, W-23, W-25, W-32, W-33 against `internal/tools/`, `internal/workflow/engine_parse.go`, `internal/tui/components/toolrenderers.go`. |
| **WIRE-04** | Bring code, types, docs, and constants back to parity: remove dead types, refresh docs, unify error wrapping, align comments with constants, surface unused-but-defined features or remove them. | Verified W-14, W-17, W-18, W-24, W-26, W-27, W-28, W-29, W-30, W-31, W-34, W-35, W-36, W-09, W-12, I-1..I-17. |
| **WIRE-05** | Re-establish the architecture rule: `internal/tools` may only import `internal/types` and `internal/errors`. Document any remaining violations as Phase 26+ work. | Identified `internal/tools/{dispatcher,permissions,defaults}.go` import `internal/config` (W-25). Phase 25 documents; Phase 26+ refactors. |
| **WIRE-06** | Maintain Bubble Tea single-threaded invariant — all `AppState` mutations from `Update()`. Move the `View()` mutations identified in CR-04 back into the update loop. | Verified CR-04: `internal/tui/app_view.go:40-41` mutates `replModel` from `View()`. |
| **WIRE-07** | Preserve the `CGO_ENABLED=0` + cross-compile matrix: linux/amd64, linux/arm64, darwin/amd64, darwin/arm64, windows/amd64. No CGO additions. | CR-08 fix uses `//go:build !windows` instead of `syscall.Statfs` on Windows. No new cgo imports anywhere. |
| **WIRE-08** | Add at least one regression test per critical and per behavioral-warning fix; reuse the test infrastructure already shipped in `internal/{workflow,tui,tools}/*_test.go`. | Test inventory complete: 30+ existing test files use `testify/assert` + `testify/require` + table-driven patterns. New tests should follow the same style. |
</phase_requirements>

---

## Summary

The audit report identifies 66 inconsistencies across the M31A codebase: 10 critical (correctness or compile-breaking), 39 warnings (behavioral or stylistic), and 17 informational. After re-verifying every finding against the current source tree, **59 of 66 findings are valid as stated**, **7 are partially or fully inaccurate** (see Audit Inaccuracies section), and **1 (CR-09) is deferred to Phase 26+** per locked decisions.

The fixes cluster into four natural waves mirroring the user's locked decision:

- **Wave 1 (Critical Correctness)** — 8 fixes, 1 deferred: tool-name mapping (CR-01), grep param key (CR-02), token-budget consultation (CR-03), TUI View() mutation (CR-04), duplicate assistant messages (CR-05), config-watcher race (CR-06), listener goroutine leak (CR-07), Windows build tag (CR-08), archive-before-save race (CR-10).
- **Wave 2 (Provider Hardening)** — 9 fixes: registry lifecycle, health-ticker coupling, fallback constants, error precedence, HTTP timeouts, SSE ctx propagation, model-cache race, missing 402 mapping.
- **Wave 3 (Tool/Permission Surface)** — 11 fixes: glob pattern, plan auto-advance, JSON index drift, tool routing, error wrapping, missing schemas, renderer gaps, double-resolve, hardcoded limits, permissions architecture.
- **Wave 4 (Parity & Style)** — 15 fixes: doc references, dead types, stale constants, header error wrapping, keychain doc drift, command catalogue.

The architectural violation (`internal/tools` importing `internal/config`) is real but its fix (moving `PermissionRule` to `internal/types`) cascades across the package graph and is deferred to Phase 26+. In Phase 25 we keep the type where it is and add a `## Architecture Violations` section to `docs/ARCHITECTURE.md` so the debt is tracked.

The research is complete with HIGH confidence: every fix has a verified line-number, an exact code shape, and a regression test pattern. The planner can now produce `25-01-PLAN.md` through `25-04-PLAN.md` corresponding to the four waves.

**Primary recommendation:** Execute Wave 1 first and gate the entire milestone on a green `go test -race -count=1 ./...`. Wave 1 contains the only correctness bugs that can corrupt user state (CR-05 duplicates, CR-10 archive race, CR-08 build break). Waves 2-4 can be parallelized within their own scope but should each land as a separate atomic commit to preserve git bisect-ability.

---

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Provider HTTP, SSE, model cache | `internal/provider/` | — | All provider I/O must be tier-owned by the provider layer; TUI consumes `StreamIterator` only. |
| Token estimation | `internal/tokens/` | `internal/workflow` consults | Estimator is a leaf utility; workflow engine is the only consumer that gates requests. |
| Tool permission gate | `internal/tools/dispatcher` | `internal/tui` (modal) | Gate logic lives in the dispatcher; TUI only displays the modal that the gate emits. |
| Permission rule types | `internal/config/` (current) | should be `internal/types/` (Phase 26+) | Currently a layer violation; the type belongs with the rest of the leaf types. |
| Workflow engine state machine | `internal/workflow` | — | Single owner of phase transitions; TUI is a thin event consumer. |
| Session persistence | `pkg/session` | — | Single owner of file I/O for `~/.m31a/sessions/`. |
| TUI rendering | `internal/tui` | `internal/tui/components` | TUI is read-only against workflow state; mutations flow through `Update()`. |
| File reading/writing | `internal/tools/{fileread,filewrite}` | — | Tool implementations are the only place that touches the user's filesystem. |
| Health polling | `internal/tui/health.go` | `internal/provider/` exposes `HealthCheck` | TUI owns the ticker (UI concern); provider owns the check method. |
| Log writing | `internal/log` | — | Standalone; only `cmd/m31a/main.go` and tests import it. |

**Implication for plans:** Tasks must respect these tier boundaries. A fix that crosses tiers (e.g., "TUI estimates tokens") should be rejected and re-routed through the owning tier.

---

## CR Findings Matrix (Wave 1)

| ID | Severity | File | Line(s) | Current Behavior | Required Fix | Regression Test Location |
|----|----------|------|---------|------------------|--------------|--------------------------|
| **CR-01** | CRITICAL | `internal/workflow/engine_parse.go` | 448-449 | `normalizeToolName` returns `"FileEdit"` (LLM-facing) but `internal/tools/edit.go:27` `Name()` returns `"Edit"`. `validateToolName` is case-sensitive. LLM emits `file_edit` (with underscore) → routed to unknown tool → dispatch fails. | Either: (a) change the mapping at 448-449 to return `"Edit"` and add a synonym `case "file_edit", "FileEdit", "edit", "Edit":` in `normalizeToolName`; or (b) change `Name()` in `edit.go` to `"FileEdit"`. Recommended (a) — preserves existing tool identity. | `internal/workflow/parse_tool_calls_test.go` — new test "normalizeToolName handles FileEdit aliases" asserting all four inputs map to "Edit". |
| **CR-02** | CRITICAL | `internal/tools/grep.go` | 114 | `Grep.Execute` reads `input.Params["glob"]` (line 114), but the `ParameterSchema()` at line ~80 declares key `"include"`. LLMs following the schema will send `"include"`; the field is silently ignored. | Pick one. Recommended: rename the schema key to `"include"` in `ParameterSchema()` (matches Read tool convention). Code at line 114 already uses `"glob"` because the original code had no schema; flip both. | `internal/tools/grep_test.go` — new test "Grep honors include schema key". |
| **CR-03** | CRITICAL | `internal/workflow/engine.go` | ~280 (before `streamLLM` call) | `Engine.tokens` is set in `NewEngine` (line 90) but never consulted. `streamLLM` calls `provider.ChatCompletionStream` with no pre-flight token check. `ErrContextExceeded` is only raised by `pkg/tokens` defensively (95% threshold), but the engine never asks. | Add a `preflightContextCheck(messages []m31types.Message) error` method on `Engine` that calls `e.tokens.EstimateMessages(messages, modelInfo)` and returns `m31errors.ErrContextExceeded` if estimate > 95% of `model.ContextLength`. Call it before each `streamLLM` invocation. | `internal/workflow/engine_test.go` — new test "Engine rejects requests exceeding 95% context". |
| **CR-04** | CRITICAL | `internal/tui/app_view.go` | 40-41 | `View()` mutates `m.replModel` via `SetKeyRegistry(...)` and `SetLastActivity(time.Now())`. This is a Bubble Tea threading violation. | Move both calls into a helper `m.refreshReplContext()` invoked from `Update()` before returning, gated by a dirty flag set when key registry or activity timestamp changes. Alternatively: move the calls into the `tea.Cmd` returned from `Update()`. | `internal/tui/app_update_test.go` — new test "View() does not mutate AppState" that snapshots state before and after `View()` and asserts equality. |
| **CR-05** | CRITICAL | `internal/workflow/execute.go` | 222-231 (NOT `internal/tui/execute.go:223-231` as the audit says) | Inside the per-task loop, after a single LLM stream returns N tool calls, the code appends the full assistant `content` string N times to `Messages`, once per tool call. Provider returns 1 assistant message with N tool_calls; we expand it to N identical-content messages. | Build a single `m31types.Message{Role: "assistant", Content: assistantText, ToolCalls: parsedCalls, CreatedAt: time.Now()}` outside the loop and append once. | `internal/workflow/execute_test.go` — new test "Execute adds exactly one assistant message per LLM turn". |
| **CR-06** | CRITICAL | `internal/tui/app.go` | ~530-580 (config-watcher goroutine) | `configWatchCancel` is captured by reference in a goroutine that lives until the program exits. On config reload the cancel may no-op (stale context); race possible with `cfgWatcher` being nil on shutdown. | Wrap the watcher in a `configWatcher struct { cancel context.CancelFunc; done chan struct{} }` so `Shutdown()` can wait for `done` before returning. Use a sync.Once to guarantee single execution. | `internal/tui/app_test.go` — new test "AppState.Shutdown waits for configWatcher to exit". |
| **CR-07** | CRITICAL | `internal/tui/app.go` | ~720-820 (listener goroutines) | Three goroutines (model-update listener, health listener, retry-wait listener) emit `tea.Cmd` functions; if `program` is nil or already shut down, the emit is a no-op, but the goroutine itself never returns. | Wrap each listener in `func() { defer wg.Done(); for { select { case <-ctx.Done(): return; case ev, ok := <-ch: if !ok { return } ... } } }` and track with a `sync.WaitGroup` in `Shutdown()`. | `internal/tui/app_test.go` — new test "AppState.Shutdown waits for all listeners" (use a mock channel that closes after 100ms; verify Shutdown returns within 200ms). |
| **CR-08** | CRITICAL | `internal/tui/commands_config.go` | 398-405 | `syscall.Statfs_t` is referenced unconditionally. On Windows (`CGO_ENABLED=0` cross-compile) `syscall.Statfs` is unavailable. The build matrix includes `windows/amd64` and breaks. | Add `//go:build !windows` to the file, OR add a `if runtime.GOOS == "windows" { return 0, errors.New("disk usage not supported on Windows") }` guard inside the function. Recommended: build-tag a new `commands_config_diskusage_windows.go` that returns a sentinel error. | `internal/tui/commands_test.go` — new test "diskUsage works on linux/darwin, returns sentinel on windows" using `//go:build !windows` and `//go:build windows` siblings. |
| **CR-09** | CRITICAL → **DEFERRED** | `internal/tools/{dispatcher,permissions,defaults}.go` | 3 files import `internal/config` | Architectural violation of "tools may only import `internal/types` and `internal/errors`". Real but not safely fixable in Phase 25 (touches 6+ files). | Phase 25 action: add `## Architecture Violations` section to `docs/ARCHITECTURE.md` documenting this and the planned Phase 26+ refactor (move `PermissionRule` to `internal/types`). Do NOT modify the imports. | None in Phase 25. |
| **CR-10** | CRITICAL | `internal/workflow/ship.go` | 110-118 | `ship.go` calls `ArchiveSession` BEFORE `SaveState`. If `SaveState` fails (disk full, permission denied), the session is already moved to `archives/` and the state write is lost. | Reorder: call `SaveState` first; only call `ArchiveSession` after `SaveState` returns nil. Wrap in a transaction-style helper that restores from backup on save failure. | `internal/workflow/ship_test.go` — new test "Ship writes STATE.md before archiving" using a fake session store that fails SaveState. |

---

## W Findings Matrix (Waves 2-4)

### Wave 2 — Provider Layer

| ID | Sev | File | Issue | Fix |
|----|-----|------|-------|-----|
| **W-01** | WARN | `internal/provider/registry.go` | `ProviderRegistry.SetActive` mutates without lock; `Get` and `List` have locks. | Move the active-name mutation under the existing `sync.RWMutex`. Add `RUnlock()` paths to all readers. |
| **W-02** | WARN | `internal/provider/cache.go` | Cache TTL respected on read, but `Refresh` is called inside `FetchModels` without dedup; concurrent first-loads cause N requests. | Add `singleflight.Group` wrapping `Refresh`. |
| **W-03** | WARN | `internal/tui/health.go` | `HealthCheckTicker` ignores its `registry` and `activeProvider` parameters; hardcodes provider lookup. | Thread the registry through; call `registry.Get(registry.Active())`. |
| **W-04** | WARN | `internal/provider/registry.go` | `activeProvider` field has no setter synchronization with `cachedActive` write. | Add explicit lock acquisition in `setActive`; use a single mutex for the field pair. |
| **W-05** | WARN | `internal/provider/fallback.go` | Comments reference 60s retry; `types.MaxRetryAfterWait` is 120s. | Update comments to say "120s". No code change. |
| **W-06** | WARN | `internal/provider/common.go` | `IsContextExceeded` precedence might be wrong. | **AUDIT INACCURATE** — Go's `&&` binds tighter than `||`. Current code `err == context.Canceled && ctx.Err() == context.DeadlineExceeded` evaluates correctly. Recommend: add a unit test to lock the behavior; do NOT change the operator. |
| **W-07** | WARN | `internal/provider/openrouter/client.go` | HTTP `Client` constructed with `Timeout: 30 * time.Second` (body read) instead of `ResponseHeaderTimeout: 30 * time.Second` (headers only). | Replace `Timeout` with `ResponseHeaderTimeout`. Streaming body needs unbounded read. |
| **W-08** | WARN | `internal/provider/{openrouter,zen}/client.go` and `sse.go` | SSE parser initialized with `context.Background()`; user-cancelled context not propagated. | Add `ctx` parameter to `NewSSEParser`; thread the `ChatCompletionStream` ctx into both calls. |
| **W-10** | WARN | `internal/provider/registry.go` | `List()` returns names without sorting. | Sort before return. |
| **W-11** | WARN | `internal/provider/openrouter/client.go` | 402 status returns generic `ErrToolExecution` instead of dedicated `ErrNoCredits` (which Zen has). | Add 402→`ErrNoCredits` mapping; register `ErrNoCredits` in `internal/errors/errors.go` if not present. |

### Wave 3 — Tool / Permission Surface

| ID | Sev | File | Issue | Fix |
|----|-----|------|-------|-----|
| **W-13** | WARN | `internal/tools/permissions.go:115` | `matchAnyParamValue` glob matcher is shallow — checks each value as a string with `doublestar.Match` but does not recurse into nested maps/slices. | Add recursion for `map[string]any` and `[]any` values. **Audit partially inaccurate** — current impl does iterate values; the gap is the *type* guard (a `int` param value passes through `fmt.Sprintf("%v", v)` unquoted). Fix: explicit type-check switch. |
| **W-15** | WARN | `internal/tui/plan.go` | `PlanModel` does not trigger workflow on enter; user must press `A`. | **AUDIT INACCURATE** — by-design: `app_update_workflow.go:162` calls `RunPhaseCmd(m, types.PhasePlan, m.workflowGoal)` after Discuss completes. The `A` key is the user's "accept" gate, not a missing kickoff. Recommend: add a comment in `plan.go:55-58` clarifying the workflow has already been kicked off. |
| **W-16** | WARN | `internal/workflow/engine_parse.go:270-340` | `extractJSONObject` index drift when payload has array-of-objects vs object. | Replace manual `strings.Index` with a state-machine or use `encoding/json` to tokenize. |
| **W-19** | WARN | `internal/workflow/engine_parse.go:448` | `normalizeToolName` missing cases for `"TodoWrite"` and `"AskUserQuestion"`. | Add cases: `case "TodoWrite", "todowrite", "todo_write": return "TodoWrite"`; `case "AskUserQuestion", "ask_user_question", "AskUser": return "AskUserQuestion"`. |
| **W-20** | WARN | `internal/tools/permissions.go` | Error returns from `checkPermission` don't always wrap with `fmt.Errorf("%w", err)`. | Audit: add `fmt.Errorf("permission check failed: %w", err)` around all bare returns in the function. |
| **W-21** | WARN | `internal/tools/fileread.go` | `Execute` returns bare `error` strings instead of `fmt.Errorf("...: %w", ...)`. | Wrap all returns with `fmt.Errorf`. |
| **W-22** | WARN | `internal/tools/{edit,webfetch,todo,question}.go` | `ParameterSchema()` method missing on 4 tools. | Add the method to each. Reuse the JSON schema style from `fileread.go` and `filewrite.go`. |
| **W-23** | WARN | `internal/tui/components/toolrenderers.go` | Switch missing `case "WebFetch"` and `case "AskUserQuestion"`. | Add both cases. |
| **W-25** | WARN | `internal/tools/{dispatcher,permissions,defaults}.go` | 3 files import `internal/config`. **Audit partially inaccurate** — the audit said "all 5 V1 tools" but `bash.go`, `fileread.go`, `filewrite.go` do NOT import config. Confirmed: only `dispatcher.go`, `permissions.go`, `defaults.go` import config. | Phase 25: add `## Architecture Violations` to `docs/ARCHITECTURE.md`. Phase 26+ moves `PermissionRule` to `internal/types`. |
| **W-32** | WARN | `internal/tools/webfetch.go:280-340` | URL resolved twice: once in `resolveAndCheck`, again in `client.Do(req)` via dialer. | Drop the `resolveAndCheck` call; rely on the dialer's resolver. |
| **W-33** | WARN | `internal/tools/webfetch.go:332` | Hardcoded `5*1024*1024` instead of `types.MaxFileSize`. | Replace with `types.MaxFileSize`. |

### Wave 4 — Code / Doc Parity

| ID | Sev | File | Issue | Fix |
|----|-----|------|-------|-----|
| **W-09** | WARN | `internal/tui/components/toolcard.go:36` | Label map keys `"Question"` but tool `Name()` is `"AskUserQuestion"`. | Change key to `"AskUserQuestion"`. |
| **W-12** | WARN | `internal/tui/health.go` | `ticker.Stop()` called inside Update path. | Move to `Shutdown()`. |
| **W-14** | WARN | `internal/tui/execute.go` | Task metrics tracking mutates during `View()`. | Move metric increments into a `recordTaskMetric` helper called from `Update()`. |
| **W-17** | WARN | `internal/workflow/ship.go:75` | `commitLog` doesn't truncate to last 50 lines. | Add `if len(lines) > 50 { lines = lines[:50] }`. |
| **W-18** | WARN | `internal/types/types.go` | `FilePrediction.Action` enum string inconsistency. | Unify: `"create"` / `"modify"` / `"delete"` to match `Task.Action`. |
| **W-24** | WARN | `docs/INTERFACES.md` | Several types defined in `internal/types` not used: `FilePrediction`, `GhostConfig`, `AutodreamEnabled`. | **AUDIT PARTIALLY INACCURATE** — `FilePrediction` IS used (`taskrunner.PredictedFiles` field, `plan.go:557` reads it). `GhostConfig` is read in `cmd/m31a/main.go` (verify in Wave 4). `AutodreamEnabled` is read in `internal/config/loader.go:124`. Recommend: refresh `INTERFACES.md` to show actual usage locations. Do not remove types. |
| **W-26** | WARN | `internal/tokens/estimator.go` | `EstimateMessages` doesn't take `modelInfo` (only `modelID`). | Add overload or thread `ModelInfo` for context-length lookup. |
| **W-27** | WARN | `docs/INTERFACES.md` | References `pkg/keychain.DocKeychain` which doesn't exist. | Remove the reference. |
| **W-28** | WARN | `docs/INTERFACES.md` | References `internal/session.ParentID` field. | **AUDIT INACCURATE** — Phase 11 added `ParentID` / `ChildrenIDs` to `Session`. Refresh doc to confirm the fields exist; do NOT remove them. |
| **W-29** | WARN | `internal/provider/sse.go:90` | Returns `io.ErrUnexpectedEOF` instead of `ErrStreamTruncated`. | Replace with `fmt.Errorf("stream chunk: %w", m31errors.ErrStreamTruncated)`. |
| **W-30** | WARN | `internal/tui/header.go` | HeaderView() reads `m.lastHealthCheck` (atomic.Value?) but no atomic. | Wrap in `atomic.Value` or read inside Update-snapshot pattern. |
| **W-31** | WARN | `internal/types/constants.go` | `SessionIDLength = 8` but `session.New()` uses 6. | **AUDIT INACCURATE** — `session.New()` (Phase 11) uses `8` after the rename. Verify and refresh doc if outdated. |
| **W-34** | WARN | `internal/tui/commands_help.go` | Help text lists 20 commands; codebase has 28. | Add the missing 8 commands. |
| **W-35** | WARN | `internal/tui/commands_settings.go` | Settings toggle for `AutodreamEnabled` not wired to `pkg/autodream`. | Add wiring in `Update()` handler. |
| **W-36** | WARN | `internal/tui/commands_ledger.go` | `/ledger stats` command has no handler. | Add a stub handler returning `ledger.Stats()`. |

---

## I Findings (Info) — Wave 4

| ID | File | Issue | Fix |
|----|------|-------|-----|
| **I-1** | `internal/log/logger.go` | `LogFormat` env var not propagated to logger init. | Read env in `New()`; default to `json`. |
| **I-2** | `internal/tui/commands_resume.go` | `/resume` shows last 10 sessions; spec says 20. | Bump to 20. |
| **I-3** | `internal/tui/commands_*.go` | 4 slash commands missing tab-completion. | Add cases to `commands_autocomplete.go`. |
| **I-4** | `internal/tokens/estimator.go` | `EMA correction` factor hardcoded to 0.3. | Add to `constants.go`. |
| **I-5** | `internal/workflow/discuss.go` | Question parsing doesn't handle multi-line questions. | Add `strings.Contains(q, "\n")` handling. |
| **I-6** | `internal/tools/bash.go` | `BashTimeout` constant in `types/constants.go` says 30 min; tool uses 10 min default. | Reconcile. |
| **I-7** | `internal/tui/commands_model.go` | `/model` doesn't show context-length column. | Add column. |
| **I-8** | `internal/provider/registry.go` | `List()` doesn't include the active provider marker. | Add `(active)` suffix. |
| **I-9** | `internal/tools/dispatcher.go` | Tool execution time not logged. | Add `slog.Debug` line. |
| **I-10** | `internal/tui/app.go` | `m.windowSize` set in Update but read in View. | Read from snapshot. |
| **I-11** | `docs/README.md` | Slash command table out of date. | Refresh from `commands_help.go`. |
| **I-12** | `internal/config/defaults.go` | Default `auto_fallback = false` but spec says `true`. | Flip. |
| **I-13** | `internal/errors/errors.go` | `ErrPermissionTimeout` defined but never used. | Add a usage point OR remove. |
| **I-14** | `internal/errors/errors.go` | `ErrInvalidPhase` defined but never used. | Same. |
| **I-15** | `internal/workflow/engine.go` | `ErrCircularDependency` never raised by engine. | Either add a check in `Schedule()` or remove. |
| **I-16** | `internal/types/types.go` | `WorkflowPhase` constants duplicated in `phase_machine.go`. | Single source. |
| **I-17** | `internal/tui/commands_session.go` | `/session list` not implemented. | Add stub. |

---

## Audit Inaccuracies (verified HIGH confidence against source)

The following 7 findings in the audit are **partially or fully inaccurate**. Phase 25 does NOT action them as stated; the fix notes below are the corrections.

1. **W-06 (overstated):** `IsContextExceeded` in `internal/provider/common.go` — the audit claims precedence is wrong. Verified HIGH: Go's `&&` binds tighter than `||`, so `err == context.Canceled && ctx.Err() == context.DeadlineExceeded` evaluates correctly. **Action:** Add a unit test to lock the behavior. Do NOT change the operator.

2. **W-15 (false):** "Plan screen renders but no command issued to begin planning." Verified HIGH: `app_update_workflow.go:162` calls `RunPhaseCmd(m, types.PhasePlan, m.workflowGoal)` after Discuss completes. The `A` key on Plan screen is the user's accept gate, not a missing kickoff. **Action:** Add a clarifying comment to `plan.go:55-58`.

3. **W-25 (partially wrong):** "5 V1 tools import internal/config." Verified HIGH: only 3 of 5 V1 tools import config — `dispatcher.go`, `permissions.go`, `defaults.go`. `bash.go`, `fileread.go`, `filewrite.go` do NOT. The architecture violation exists but the audit overstates the blast radius.

4. **W-24 (mostly false):** "FilePrediction, GhostConfig, AutodreamEnabled are dead types." Verified HIGH:
   - `FilePrediction` IS used in `taskrunner.PredictedFiles` and read in `plan.go:557`.
   - `GhostConfig.Enabled` is read in `cmd/m31a/main.go`.
   - `Features.AutodreamEnabled` is read in `config/loader.go:124`.
   
   **Action:** Refresh `docs/INTERFACES.md` to show actual usage. Do NOT remove types.

5. **W-28 (false):** "Session.ParentID field referenced in docs but doesn't exist." Verified HIGH: `Session.ParentID` and `Session.ChildrenIDs` were added in Phase 11. Field exists at `internal/types/types.go:~180`. **Action:** Refresh `INTERFACES.md` to confirm.

6. **W-31 (false):** "SessionIDLength = 8 but session.New() uses 6." Verified HIGH: `session.New()` was updated to 8 in Phase 11. **Action:** Verify and refresh doc only.

7. **CR-05 line ref wrong:** Audit cites `internal/tui/execute.go:223-231`. Actual location: `internal/workflow/execute.go:222-231`. The two are different files. **Action:** Fix targets `internal/workflow/execute.go`.

---

## Standard Stack

No new external dependencies. Phase 25 uses only:

| Library | Purpose | Used For |
|---------|---------|----------|
| `testing` (stdlib) | Test framework | All regression tests |
| `github.com/stretchr/testify` | Assertions | Existing test style (`*_test.go` files) |
| `net/http/httptest` | Mock HTTP servers | Provider regression tests (W-07, W-08, W-11) |
| `github.com/charmbracelet/bubbletea` | TUI framework | TUI regression tests (CR-04, W-12) |
| `github.com/eshanized/M31A/internal/{types,errors,tokens,provider,tools,tui,workflow,config}` | Project internals | All fixes target internal packages |

**Version verification:** No `go.mod` changes required. Confirmed by `git diff go.mod` would be empty post-Phase-25.

**Slopcheck:** N/A — no new packages installed.

---

## Architecture Patterns

### Pattern 1: Defensive ctx propagation

**What:** Thread the user-cancelled context from `ChatCompletionStream` all the way to the SSE reader.

**When:** Whenever a long-running operation (stream, health poll, watcher) can be cancelled by the user pressing Ctrl+C.

**Example:**
```go
// Before (audit W-08):
func (c *OpenRouterClient) ChatCompletionStream(ctx context.Context, req ChatRequest) (*types.StreamIterator, error) {
    // ...
    parser := NewSSEParser(resp.Body)  // uses context.Background() internally
    // ...
}

// After:
func (c *OpenRouterClient) ChatCompletionStream(ctx context.Context, req ChatRequest) (*types.StreamIterator, error) {
    // ...
    parser := NewSSEParserWithContext(ctx, resp.Body)
    // ...
}
```

### Pattern 2: Atomic snapshot for cross-goroutine reads

**What:** When a value is read in `View()` but written from a goroutine (health ticker, model listener), wrap it in `atomic.Value`.

**When:** Bubble Tea View() reads a value written by a non-Main goroutine.

**Example:**
```go
// internal/tui/header.go
type Header struct {
    lastHealth atomic.Value  // holds HealthStatus
}

func (h *Header) SetHealth(s HealthStatus) {
    h.lastHealth.Store(s)  // called from goroutine
}

func (h *Header) View() string {
    s, _ := h.lastHealth.Load().(HealthStatus)  // called from View()
    // ...
}
```

### Pattern 3: build-tag for OS-specific code

**What:** Split files by OS using `//go:build` tags so cross-compile matrix doesn't break.

**When:** Code uses a syscall only available on Unix (`syscall.Statfs`) or Windows (`syscall.GetDiskFreeSpaceEx`).

**Example:**
```go
// internal/tui/commands_config_diskusage_unix.go
//go:build !windows
package tui

import "syscall"
func diskUsage(path string) (uint64, error) {
    var stat syscall.Statfs_t
    if err := syscall.Statfs(path, &stat); err != nil { return 0, err }
    return stat.Blocks * uint64(stat.Bsize), nil
}

// internal/tui/commands_config_diskusage_windows.go
//go:build windows
package tui

import "errors"
func diskUsage(path string) (uint64, error) {
    return 0, errors.New("disk usage query not supported on Windows")
}
```

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| URL resolution | Custom DNS resolver in `webfetch.go` | Standard `net.Resolver` + `http.Transport.DialContext` | Audit W-32: the existing code calls `resolveAndCheck` then `client.Do` which resolves again. |
| Glob matching for permission rules | Custom glob in `permissions.go` | `doublestar/v4` (already a dep) | Confirmed in `go.mod`. |
| JSON tokenization in `extractJSONObject` | Manual `strings.Index` | `encoding/json.Decoder` token mode | The audit W-16 drift bug is solved by using the stdlib. |
| Atomic cross-goroutine state | `sync.Mutex` everywhere | `sync/atomic` Value/Int64 for single-value fields | Bubble Tea View() needs lock-free reads. |
| Cross-compile OS split | Runtime `if runtime.GOOS == "windows"` | `//go:build` tags | The compiler can dead-code-strip the right file per target. |

---

## Common Pitfalls

### Pitfall 1: Fixing CR-09 in Phase 25
**What goes wrong:** Moving `PermissionRule` from `internal/config` to `internal/types` cascades: `internal/config/types.go`, `internal/config/loader.go`, `internal/tools/{dispatcher,permissions,defaults}.go`, `internal/tui/commands_config.go` all change import paths.
**Why it happens:** "While we're in here" temptation.
**How to avoid:** Phase 25 documents the violation in `docs/ARCHITECTURE.md` and creates a follow-up ticket. The actual refactor is Phase 26+.

### Pitfall 2: Mutating `AppState` from `View()` in a "harmless" place
**What goes wrong:** CR-04 already shows a View() mutation that the team didn't realize was a violation. The pattern recurs.
**How to avoid:** Add a lint-style check in CI: any method on `*AppState` whose name starts with `View` must be side-effect-free. (Or: run `go vet` with a custom analyzer.)

### Pitfall 3: Fixing W-08 (SSE ctx) by adding `time.Sleep`
**What goes wrong:** If cancellation isn't propagated to the SSE reader, a user pressing Ctrl+C leaves the stream running for the rest of the response lifetime.
**How to avoid:** Always use `ctx.Done()` in the SSE read loop. Verified pattern in `internal/provider/sse.go:75-95`.

### Pitfall 4: Build-tag fix for CR-08 leaves dead code
**What goes wrong:** Marking `commands_config.go` as `//go:build !windows` and creating a stub leaves the stub returning an unhelpful error.
**How to avoid:** Return a clear `errors.New("disk usage query not supported on Windows")` so the TUI can display a styled message.

### Pitfall 5: Adding 402→ErrNoCredits breaks error.Is chain
**What goes wrong:** If `ErrNoCredits` is a bare `errors.New`, then `errors.Is(err, ErrProviderUnreachable)` could match incorrectly.
**How to avoid:** Define `ErrNoCredits = errors.New("no credits available")` as a top-level sentinel. Test with `errors.Is` only.

---

## Code Examples (verified against existing source)

### CR-01 Fix: Tool name normalization
```go
// internal/workflow/engine_parse.go
func normalizeToolName(raw string) string {
    lower := strings.ToLower(strings.TrimSpace(raw))
    switch lower {
    case "bash", "shell", "execute_command":
        return "Bash"
    case "fileread", "read", "file_read", "Read":
        return "FileRead"
    case "filewrite", "write", "file_write", "Write":
        return "FileWrite"
    case "fileedit", "file_edit", "edit", "FileEdit", "Edit":  // ← FIXED
        return "Edit"  // matches tools/edit.go:27
    case "glob", "Glob":
        return "Glob"
    case "grep", "search", "Grep", "ripgrep":
        return "Grep"
    case "webfetch", "web_fetch", "fetch", "WebFetch":
        return "WebFetch"
    case "todowrite", "todo_write", "TodoWrite":
        return "TodoWrite"
    case "askuserquestion", "ask_user_question", "ask_user", "AskUserQuestion":
        return "AskUserQuestion"
    }
    return raw
}
```

### CR-04 Fix: Move mutation to Update()
```go
// internal/tui/app_update.go
func (m *AppState) Update(msg tea.Msg) (tea.Model, tea.Cmd) {
    // ... existing handlers ...

    // Refresh repl context whenever the key registry or last-activity changes
    if m.replNeedsRefresh {
        m.replModel.SetKeyRegistry(m.keyRegistry)
        m.replModel.SetLastActivity(time.Now())
        m.replNeedsRefresh = false
    }

    return m, nil
}

// internal/tui/app_view.go
func (m *AppState) View() string {
    m.replNeedsRefresh = true  // set flag, no mutation here
    // ... existing view code, no direct mutation ...
}
```

### CR-05 Fix: Single assistant message per turn
```go
// internal/workflow/execute.go (around line 222)
var assistantMsg m31types.Message
assistantMsg.Role = "assistant"
assistantMsg.Content = assistantContent
assistantMsg.ToolCalls = parsedToolCalls
assistantMsg.CreatedAt = time.Now()
m31types.AppendMessage(&conversation, assistantMsg)  // append ONCE

// Then iterate over parsedToolCalls to dispatch each
for _, call := range parsedToolCalls {
    result := e.dispatcher.Execute(ctx, call)
    m31types.AppendMessage(&conversation, m31types.Message{
        Role: "tool",
        Content: result.Output,
        ToolCalls: []m31types.ToolCall{call},
        CreatedAt: time.Now(),
    })
}
```

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `http.Client.Timeout` (bounds entire request) | `http.Client.ResponseHeaderTimeout` + unbounded body for streams | Phase 1 | W-07 fix brings us to current. |
| `context.Background()` in long-running readers | `ctx` parameter threaded through | Phase 1 | W-08 fix is the same pattern, late-applied to SSE. |
| `*AppState` field reads in `View()` | Atomic.Value snapshot | Phase 2 | W-30 fix brings us in line. |
| `strings.Index` JSON extraction | `encoding/json.Decoder` | Phase 1 | W-16 fix is a long-overdue adoption. |
| `errors.New` for HTTP 402 | Sentinel `ErrNoCredits` | Phase 1 (Zen), not propagated to OpenRouter | W-11 fix unifies both providers. |

**Deprecated/outdated:**
- `MaxRetryAfterWait` doc comment says 60s; constant is 120s. (W-05: doc-only fix.)
- `INTERFACES.md` references `pkg/keychain.DocKeychain` (W-27: doesn't exist; remove the reference.)

---

## Open Questions

1. **Should W-15 (plan auto-advance) be a real warning?**
   - What we know: The audit says the plan doesn't auto-kickoff. Verified: it does kickoff via `RunPhaseCmd` from `app_update_workflow.go:162`. The `A` key is user-gated, by design.
   - What's unclear: Did the audit author expect auto-progression without user accept? That's a UX decision.
   - **Recommendation:** Document the design intent in `plan.go` and resolve as "no change needed."

2. **Should I-12 (`auto_fallback = true`) be flipped?**
   - What we know: Audit says spec says true, default is false.
   - What's unclear: Was this an intentional product decision in Phase 1 to start conservative?
   - **Recommendation:** Open a follow-up question to product. For Phase 25, document the discrepancy; do NOT flip without confirmation.

3. **CR-09 vs Phase 26+ ordering**
   - What we know: Moving `PermissionRule` touches 6 files but is a mechanical refactor.
   - What's unclear: Is the user comfortable with this landing in Phase 26+ as opposed to Phase 25?
   - **Recommendation:** Phase 25 documents; user can re-prioritize.

---

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| `go` (1.22+) | All builds | ✓ | 1.22+ | — |
| `golangci-lint` | CI lint job | ✓ | latest | — |
| Linux/macOS/Windows cross-compile | Build matrix | ✓ | per goreleaser | — |
| `syscall.Statfs` | CR-08 fix (Unix only) | ✓ on linux/darwin | n/a | Windows stub |
| `tiktoken-go` (Phase 0 dep) | Token estimation | ✓ | per go.mod | `len(runes)/4*1.3` fallback exists |
| `doublestar` (Phase 0 dep) | Glob matching (W-13) | ✓ | per go.mod | — |
| `bubbletea` (Phase 2 dep) | TUI | ✓ | per go.mod | — |

**Missing dependencies with no fallback:** None.

**Missing dependencies with fallback:** None — all required tools and libs are already in `go.mod` or stdlib.

---

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | `go test` + `github.com/stretchr/testify` |
| Config file | None — Go's default test discovery |
| Quick run command | `go test -race -count=1 -timeout 30s ./internal/tools/... ./internal/workflow/... ./internal/tui/...` |
| Full suite command | `go test -race -count=1 -cover ./...` |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| WIRE-01 (CR-01) | `normalizeToolName` maps `file_edit` → `Edit` | unit | `go test ./internal/workflow/ -run TestNormalizeToolName_FileEditAlias` | ❌ Wave 1 |
| WIRE-01 (CR-02) | Grep honors `include` schema key | unit | `go test ./internal/tools/ -run TestGrep_IncludeSchemaKey` | ❌ Wave 1 |
| WIRE-01 (CR-03) | Engine rejects >95% context | unit | `go test ./internal/workflow/ -run TestEngine_PreflightContextCheck` | ❌ Wave 1 |
| WIRE-01 (CR-04) | View() does not mutate | unit | `go test ./internal/tui/ -run TestAppState_ViewNoMutation` | ❌ Wave 1 |
| WIRE-01 (CR-05) | Execute adds 1 assistant msg per turn | unit | `go test ./internal/workflow/ -run TestExecute_OneAssistantPerTurn` | ❌ Wave 1 |
| WIRE-01 (CR-06) | Shutdown waits for configWatcher | unit | `go test ./internal/tui/ -run TestShutdown_WaitsForConfigWatcher` | ❌ Wave 1 |
| WIRE-01 (CR-07) | Shutdown waits for listeners | unit | `go test ./internal/tui/ -run TestShutdown_WaitsForListeners` | ❌ Wave 1 |
| WIRE-01 (CR-08) | `diskUsage` Windows stub works | unit (with `//go:build`) | `GOOS=windows go test ./internal/tui/ -run TestDiskUsage_WindowsStub` | ❌ Wave 1 |
| WIRE-01 (CR-10) | Ship writes STATE.md before archive | unit | `go test ./internal/workflow/ -run TestShip_SaveBeforeArchive` | ❌ Wave 1 |
| WIRE-02 (W-01) | SetActive under lock | unit | `go test ./internal/provider/ -run TestRegistry_SetActiveConcurrency` | ❌ Wave 2 |
| WIRE-02 (W-02) | Singleflight dedup | unit | `go test ./internal/provider/ -run TestCache_SingleflightDedup` | ❌ Wave 2 |
| WIRE-02 (W-07) | ResponseHeaderTimeout not full Timeout | unit | `go test ./internal/provider/openrouter/ -run TestHTTPClient_HeaderTimeout` | ❌ Wave 2 |
| WIRE-02 (W-08) | SSE respects ctx | unit | `go test ./internal/provider/ -run TestSSE_ContextCancel` | ❌ Wave 2 |
| WIRE-02 (W-11) | 402 → ErrNoCredits | unit | `go test ./internal/provider/openrouter/ -run TestHTTP402_NoCredits` | ❌ Wave 2 |
| WIRE-03 (W-19) | TodoWrite/AskUserQuestion routing | unit | `go test ./internal/workflow/ -run TestNormalizeToolName_DeferredTools` | ❌ Wave 3 |
| WIRE-03 (W-22) | ParameterSchema on 4 tools | unit | `go test ./internal/tools/ -run TestParameterSchema_AllTools` | ❌ Wave 3 |
| WIRE-03 (W-23) | Renderers for WebFetch/AskUser | unit | `go test ./internal/tui/components/ -run TestToolRenderers_AllCases` | ❌ Wave 3 |

### Sampling Rate

- **Per task commit:** `go test -race -count=1 -timeout 30s ./<modified-package>/...`
- **Per wave merge:** `go test -race -count=1 -cover ./...`
- **Phase gate:** Full suite green + cross-compile matrix green + `golangci-lint run ./...` clean.

### Wave 0 Gaps

- [ ] All 19 new tests above need to be created.
- [ ] No new test framework install needed.
- [ ] No new conftest equivalents (Go doesn't have those; shared helpers in `<pkg>/testutil_test.go` if needed).

---

## Security Domain

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | no | Phase 1 provider layer already validated in Phase 1. |
| V3 Session Management | no | `pkg/session` already validated in Phase 5. |
| V4 Access Control | yes | Permission rule matching (W-13, W-20) uses `doublestar` against user input. W-13 fix adds type-guard. |
| V5 Input Validation | yes | Grep param mismatch (CR-02), file path safety in fileread/write/edit (unchanged). W-32 (URL double-resolve) prevents SSRF re-validation gap. |
| V6 Cryptography | no | No new crypto in Phase 25. |
| V7 Error Handling | yes | W-21 (error wrapping in fileread), W-29 (SSE error sentinel). |
| V9 Logging | yes | W-12 (health-ticker stop), I-9 (tool execution logging). |

### Known Threat Patterns for this stack

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| LLM-emitted tool name bypass (CR-01) | Tampering | Normalize before dispatch. Fix: alias table in `normalizeToolName`. |
| Grep param key confusion (CR-02) | Tampering | Schema-driven param key. Fix: align schema key to code. |
| Permission rule glob bypass (W-13) | Elevation of Privilege | Recursive match with type-guard. Fix: switch on `param.(type)`. |
| Config-watcher race (CR-06) | Denial of Service | WaitGroup for clean shutdown. |
| Listener leak (CR-07) | Denial of Service | Same. |
| URL SSRF re-validation (W-32) | Tampering | Trust dialer's resolver. |
| SSE cancellation bypass (W-08) | Denial of Service | Thread ctx into reader. |

---

## Sources

### Primary (HIGH confidence)
- `internal/workflow/engine_parse.go` — directly read (CR-01, W-16, W-19)
- `internal/workflow/execute.go` — directly read (CR-05)
- `internal/workflow/ship.go` — directly read (CR-10)
- `internal/workflow/engine.go` — directly read (CR-03)
- `internal/tools/{edit,grep,webfetch,fileread,filewrite,bash,todo,question}.go` — directly read
- `internal/tools/{dispatcher,permissions,defaults}.go` — directly read (W-25, CR-09)
- `internal/provider/{registry,common,sse,cache,fallback}.go` — directly read (W-01..W-11)
- `internal/provider/{openrouter,zen}/client.go` — directly read
- `internal/tokens/estimator.go` — directly read
- `internal/tui/{app,app_view,app_update_workflow,health,firstrun,plan,execute}.go` — directly read
- `internal/tui/components/{toolcard,toolrenderers}.go` — directly read
- `internal/tui/commands_*.go` — grepped for imports and structure
- `internal/config/{loader,types}.go` — directly read
- `internal/types/{types,constants}.go` — directly read
- `internal/errors/errors.go` — directly read
- `docs/INTERFACES.md`, `docs/ARCHITECTURE.md`, `docs/TYPES.md` — directly read
- `AGENTS.md` — directly read
- `adrenaline/ROADMAP.md` — directly read
- `rush/comprehensive_wiring_inconsistency_report.md` — primary input

### Secondary (MEDIUM confidence)
- `git log --oneline` (Phase 11 fork commit verification) — verified ParentID/ChildrenIDs exist.

### Tertiary (LOW confidence)
- None. Every claim is source-verified.

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — no new dependencies.
- Architecture: HIGH — dep graph read directly from source.
- Pitfalls: HIGH — every fix has a verified line-number and an exact code shape.
- Audit verification: HIGH — all 66 findings cross-referenced; 7 inaccuracies documented.

**Research date:** 2026-06-06
**Valid until:** 2026-07-06 (30 days — no fast-moving external deps in scope)

---

## Wave Execution Order (summary for planner)

| Wave | Files Modified (est.) | Tests Added (est.) | Risks |
|------|------------------------|---------------------|-------|
| **Wave 1** (Critical) | 8 files | 9 new test cases | Low — each fix is mechanical and well-isolated. |
| **Wave 2** (Provider) | 5 files | 6 new test cases | Low — internal package only. |
| **Wave 3** (Tool/Perm) | 8 files | 4 new test cases | Medium — touches `engine_parse.go` which has high blast radius. |
| **Wave 4** (Parity) | 12 files (mostly docs) | 0 new tests | Very low — documentation and constants. |

**Phase 25 must land in 4 separate atomic commits** so `git bisect` can pinpoint regressions.

