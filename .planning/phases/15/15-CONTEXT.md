# Phase 15: Comprehensive Deep Audit Fixes — Context

**Gathered:** 2026-06-02
**Status:** Ready for planning
**Source:** Comprehensive Deep Codebase Audit (`rush/comprehensive_deep_audit_2026.md`)

<domain>

## Phase Boundary

This phase fixes all 72 issues found by the comprehensive deep audit
performed on 2026-06-02. The audit read every `.go` file in
`cmd/`, `internal/`, and `pkg/` (171 files total) and cross-referenced
three prior audits (`deep_codebase_audit_report.md`,
`integrity_audit_0_8.md`, `loophole_report.md`).

The fixes are organized into 10 plans across 4 waves:

- **Wave 1** — Critical TUI/streaming/input-safety fixes (3 plans)
- **Wave 2** — Tool security + provider resilience + workflow correctness (3 plans)
- **Wave 3** — TUI segment/concurrency + typed errors + dead-config cleanup (2 plans)
- **Wave 4** — Session/ledger/rollback hardening + low-priority polish (2 plans)

The phase ends with `go test -race -count=1 -cover ./...` green on
all three platforms and a release-ready binary.

</domain>

<decisions>

## Implementation Decisions

The following decisions are derived directly from the audit. Each
fix carries a severity, file location, and acceptance criteria that
the planner must transcribe into PLAN.md tasks. The 72 findings are
locked decisions: the planner may not skip or weaken them.

### Critical (7 — must land in Waves 1–2)

#### C-1 — `replModel` nil-pointer deref (TUI panic)
- **File:** `internal/tui/app_update.go`
- **Decision:** Add `if m.replModel == nil { return nil, nil }` guard
  at the top of every branch in `Update()` that touches `m.replModel.*`.
  Alternatively, convert all `replModel` accessors to lazy-init
  methods on `*AppState`. Plan 15-01.
- **Test:** `TestApp_Update_ReplModelNil_NoPanic` — call `Update()`
  with a `nil` replModel and a series of representative `tea.Msg`
  types; assert no panic and that the returned `cmd` is nil/benign.

#### C-2 — `AutoFallback` is dead
- **File:** `internal/provider/fallback.go`,
  `internal/provider/chat.go`
- **Decision:** Wire `cfg.Provider.AutoFallback` to the fallback
  trigger. When the active provider returns 429 or 503, the chat
  error-normalization layer must emit a `FallbackEvent` and the
  `ProviderRegistry.SetActive` must switch to the other provider if
  it is healthy. Plan 15-05.
- **Test:** Inject `ErrRateLimited` into `chat.go`'s normalization;
  assert the registry's `Active()` switched and a `FallbackEvent` was
  emitted.

#### C-3 — Stream channel double-close
- **File:** `internal/tui/streaming.go`, `internal/tui/repl_stream.go`
- **Decision:** Move channel ownership into `StartStreamCmd`. Each
  call allocates fresh `streamCh` and `streamDone`. The REPL does not
  hold these channels; the cmd is a single-use black box. The
  continuation closure consumes the cmd and disposes both channels
  internally. Plan 15-02.
- **Test:** `TestStartStreamCmd_Twice_NoPanic` — invoke twice in
  sequence; assert no `close of closed channel` panic and that
  chunks from stream N+1 do not bleed into stream N.

#### C-4 — `parseToolCalls` ReDoS / OOM
- **File:** `internal/workflow/engine.go:649-684`
- **Decision:** Cap `parseToolCalls` input at 1 MB
  (`m31types.MaxToolOutputChars * 10` is fine). Cap tools per call
  at 16. Use a streaming JSON tokenizer (do not re-scan every `{`
  position). On overflow, return a typed error
  `ErrToolInputTooLarge`. Plan 15-03.
- **Test:** `TestParseToolCalls_OversizedInput_ReturnsErrTooLarge` —
  feed 10 MB string of `{` characters; assert the function returns
  within 100 ms with `ErrToolInputTooLarge`.

#### C-5 — `isDBusUnavailable` over-matches
- **File:** `pkg/keychain/keychain_linux.go:274-287`
- **Decision:** Replace `strings.Contains(errStr, "dbus")` with
  `errors.Is(err, dbus.ErrClosed)`,
  `errors.Is(err, dbus.ErrMsgNoService)`, and a `conn.IsConnected()`
  check. Plan 15-04.
- **Test:** `TestIsDBusUnavailable_TypedErrors` — feed a real
  `*dbus.Error` with a known cause; assert the typed branch fires,
  not the string branch.

#### C-6 — WebFetch SSRF DNS rebinding
- **File:** `internal/tools/webfetch.go`
- **Decision:** Custom `DialContext` that resolves once, validates
  the IP via `isPrivateIP`, **and pins that IP** for the connection.
  Re-check after `connect`. Add a config block
  `webfetch.allowed_hosts` (default empty = allow all public IPs).
  Plan 15-04.
- **Test:** `TestWebFetch_DNSRebinding_Blocked` — point a fake DNS
  responder at `127.0.0.1`; assert the connection is refused and the
  tool returns `ErrPrivateIPBlocked`.

#### C-7 — Bash `NaN`/`Inf` timeout + `go build ./...`
- **File:** `internal/tools/bash.go:48-58`,
  `internal/workflow/engine.go:920-980`
- **Decision:** Reject `NaN` and `Inf` explicitly with
  `math.IsNaN`/`math.IsInf` before the cast. In `verifyTask`, run
  `go build`/`cargo check`/`npm test` with a reduced `$PATH`
  (toolchain bin only) and `--work-tree=<worktree>` so the verify
  step cannot pollute the user's tree. Plan 15-04.
- **Test:** `TestBash_NaNTimeout_Rejected` — call `Bash.Execute`
  with `{"timeout": NaN}`; assert error returned. `TestVerifyTask_
  ScopedToTaskFiles` — set up a project with two packages, one
  broken; assert the broken package is not built during verify.

### High (19 — must land in Waves 2–3)

#### H-1 — `Schedule` group order non-deterministic
- **File:** `pkg/taskrunner/runner.go:61-131`
- **Decision:** Sort each group by `task.ID` before returning. Sort
  the initial queue. Document the deterministic ordering. Plan 15-06.

#### H-2 — Empty `Status` re-runs done tasks
- **File:** `pkg/taskrunner/runner.go:142-148`
- **Decision:** Treat empty `Status` as `Pending` only if
  `CommitHash == ""`. Otherwise inherit `StatusDone`. Plan 15-06.

#### H-3 — `healTask` verifies by `os.Stat` only
- **File:** `internal/workflow/execute.go:281-298`
- **Decision:** Run a content check after heal: file size > 0 AND at
  least one of `AcceptanceCriteria` patterns matches the new
  content. If both fail, mark heal as failed and surface the diff
  to the LLM for the next attempt. Plan 15-06.

#### H-4 — Bisect loop unbounded
- **File:** `pkg/bisect/bisect.go:75-78`
- **Decision:** Use `git bisect run` with a temp script (single
  shell invocation). Cap iterations at `log2(commitCount) * 2`.
  Add a wall-clock timeout (default 5 min). Plan 15-06.

#### H-5 — JSON comment stripping
- **File:** `internal/workflow/engine.go:748-781`
- **Decision:** Strip `//\N` and `/*\N...*/` from the response body
  before `extractJSONObject`. Log a warning when comments are
  stripped (model regression signal). Plan 15-03.

#### H-6 — `SetSessionID` `..` traversal
- **File:** `internal/workflow/engine.go:223-227`
- **Decision:** Track `baseSessionsDir` separately; recompute as
  `filepath.Join(baseSessionsDir, id, "planning")`. Add a unit test
  that calls `SetSessionID` twice and asserts the path. Plan 15-06.

#### H-7 — `resolvedAPIKey` stale after fallback
- **File:** `cmd/m31a/main.go:185-188`
- **Decision:** Look up the key from the registry's active provider
  on every request. Add `LLMProvider.APIKey() string` accessor.
  Plan 15-05.

#### H-8 — Segment-boundary logic divergent
- **File:** `internal/tui/streaming.go`, `repl_stream.go`
- **Decision:** Single source of truth in `streaming.go`. The
  `repl_stream.go` segment-boundary code is removed; both paths
  emit `StreamChunk{Type: "thinking"|"content"|"tool"}` and the
  consumer trusts the boundary. Plan 15-07.

#### H-9 — `safeClose` race
- **File:** `internal/tui/app.go:431-443`
- **Decision:** Replace with `sync.Once`-per-channel map. Better:
  make `app.msgDone` immutable per phase. Plan 15-02.

#### H-10 — Emitter drops silently
- **File:** `internal/tui/app.go:445-457`
- **Decision:** Increase buffer to 1024. Surface drop count in
  status badge. Plan 15-07.

#### H-11 — Health interval substring check
- **File:** `internal/tui/app.go:565-575`
- **Decision:** Use `errors.Is(err, m31errors.ErrRateLimited)`. The
  provider normalization layer surfaces this. Plan 15-08.

#### H-12 — Zen key ignored at startup
- **File:** `cmd/m31a/main.go:185-191`
- **Decision:** Resolve API key based on the **active** provider
  (default or fallback), not the OpenRouter field. Plan 15-05.

#### H-13 — Slash command case sensitivity
- **File:** `internal/tui/repl_commands.go`
- **Decision:** Normalize input to lowercase once at the entry
  point. Store the registry with lowercase keys. Plan 15-07.

#### H-14 — Stream goroutine concurrency
- **File:** `internal/tui/repl_stream.go:96-179`
- **Decision:** Per-stream immutable snapshot passed via `tea.Cmd`.
  No shared mutable state between the streaming goroutine and the
  BT update loop. The single-owner channel from C-3 makes this
  implicit. Plan 15-02.

#### H-15 — Prompts embed not tested
- **File:** `internal/workflow/engine.go:44-77`
- **Decision:** Add `TestLoadPrompts_AllFieldsPopulated` that
  asserts no error and every field is non-empty. Plan 15-06.

#### H-16 — `collectDiffStats` heuristic broken
- **File:** `internal/workflow/ship.go:152-188`
- **Decision:** Use `git diff --name-status` for the A/M/D split;
  use `--numstat` only for insertions/deletions totals. Plan 15-06.

#### H-17 — `verifyTask` runs `go build ./...`
- **File:** `internal/workflow/engine.go:915-1004`
- **Decision:** Verify only the packages touched by `task.Files`.
  Add a "skip if no project-type match" path. Plan 15-06.

#### H-18 — `listCwdFiles` depth count fragile
- **File:** `internal/workflow/engine.go:830-865`
- **Decision:** Use `filepath.WalkDir` and increment depth only on
  `IsDir()`. Plan 15-06.

#### H-19 — `session_id_length` silently clamped
- **File:** `internal/tui/app.go:159-164`
- **Decision:** Validate at config load: if
  `Features.SessionIDLength < 4`, log a warning and use the default
  (`m31types.SessionIDLength`). Plan 15-08.

### Medium (28 — distributed across Waves 3–4)

| ID | File | Decision | Plan |
|----|------|----------|------|
| M-1 | `internal/tools/bash.go:70-87` | Centralize pipe cleanup in single `defer` | 15-04 |
| M-2 | `internal/tools/dispatcher.go:127-141` | Propagate underlying error; check `result.Error != ""` | 15-04 |
| M-3 | `engine.go:918-1004` + `tui/verify.go` | Pass build output to TUI as `VerifyErrorMsg` | 15-06 |
| M-4 | `engine.go:859-861` | Skip files > 1 MB, report `[binary]` | 15-06 |
| M-5 | `engine.go:131-154` | Validate `sessionID` and `workDir`; return `ErrInvalidSession` | 15-06 |
| M-6 | `pkg/autodream/autodream.go:148-186` | Mark consolidated messages with flag; exclude from candidates | 15-09 |
| M-7 | `pkg/autodream/autodream.go:147-153` | Write originals to `archives/dream-<ts>.json` | 15-09 |
| M-8 | `provider/registry.go:33` | Log warning when default ≠ active | 15-05 |
| M-9 | `provider/cache.go:30-50` | Single constructor; export `StaleTTL` | 15-05 |
| M-10 | `provider/sse.go:24-50` | Custom `scanCRLF` splitter or `TrimRight(line, "\r")` | 15-05 |
| M-11 | `provider/interface.go` | Extract `BaseProvider` with 4 common methods | 15-05 |
| M-12 | `provider/reasoning.go:60-80` | Strip `:variant` suffix before matching | 15-08 |
| M-13 | `provider/openrouter/client.go:120-160` | `http.MaxBytesReader` on response body (default 100 MB) | 15-05 |
| M-14 | `provider/registry.go:65-80` | Return error from `ActiveProvider()` if map empty | 15-08 |
| M-15 | `provider/registry.go:80-95` | `sync.Once` per provider registration | 15-08 |
| M-16 | `pkg/session/checkpoint.go:38-40` | Keep 1; document | 15-09 |
| M-17 | `pkg/ledger/ledger.go:319-326` | Return `[]struct{Key string; Count int}` | 15-08 |
| M-18 | `pkg/ledger/ledger.go:98-117` | Add `Session.ResumedAt`; sum active durations | 15-09 |
| M-19 | `pkg/rollback/rollback.go:236-268` | Rename params to `older, newer`; assert order | 15-09 |
| M-20 | `pkg/rollback/rollback.go:115-130` | Change message: "Changes stashed — run 'git stash pop'" | 15-09 |
| M-21 | `internal/tui/streaming.go:36-39` | Reverse defer order; close `streamDone` before drain | 15-02 |
| M-22 | `internal/tui/repl_stream.go:62` | Populate `lastUsage` from SSE `usage` chunk | 15-07 |
| M-23 | `pkg/arbitrage/arbitrage.go:155-193` | Add `Capabilities.Tools` check | 15-08 |
| M-24 | `pkg/arbitrage/arbitrage.go:260-286` | Score-based classification (count complex, threshold) | 15-08 |
| M-25 | `internal/tui/repl.go` | Cap history at 500; lazy-render | 15-07 |
| M-26 | `internal/tui/header.go` | Cache header; invalidate on TickMsg/model change | 15-07 |
| M-27 | `pkg/session/planning.go:66` | `scanner.Buffer(make([]byte, 0, 1MB), 1MB)` | 15-09 |
| M-28 | `pkg/session/manager.go:266-347` | Drop cache or `fsnotify` | 15-09 |
| M-29 | `internal/git/git.go:113-117` | `git rev-list --count HEAD` + typed sentinel | 15-09 |
| M-30 | `internal/tui/commands.go` | Cooldown on `/compress` (default 60s) | 15-10 |
| M-31 | `pkg/ledger/ledger.go:357-407` | Scan for orphaned `*.tmp` on `New` | 15-09 |
| M-32 | `internal/tui/repl.go` | Split into `ReplInput`, `ReplViewport`, `ReplStream`, `ReplHistory` | deferred (D-11) |
| M-33 | `internal/tui/components/permission.go:131-153` | Remove `responded`/`response` from modal | 15-10 |

### Low (18 — Wave 4, batched)

| ID | File | Decision | Plan |
|----|------|----------|------|
| L-1 | `internal/tools/bash.go:48,55` | Use `m31types.BashTimeout` constant | 15-10 |
| L-2 | `internal/tui/app.go:145-154` | Validate `apiKey` matches `activeProvider` | 15-10 |
| L-3 | `internal/tui/repl_view.go:99-105` | Move `#FDD663` to `theme.Warning` | 15-10 |
| L-4 | `pkg/keychain/keychain_linux.go:101-112` | Add `ErrKeychainDecrypt` sentinel | 15-10 |
| L-5 | `cmd/m31a/main.go:42-47` | Generate slash command list from registry | 15-10 |
| L-6 | `pkg/taskrunner/runner.go:45` | Use `m31types.BashTimeout` | 15-10 |
| L-7 | `internal/config/types.go:108-115` | Implement per-phase model assignment or remove | 15-08 |
| L-8 | `pkg/arbitrage/arbitrage.go` | Wire `/optimize` slash command | 15-08 |
| L-9 | `internal/tui/cache.go` vs `provider/cache.go` | Consolidate | 15-10 |
| L-10 | `internal/tui/repl_commands.go` | Document: chaining not supported | 15-10 |
| L-11 | `internal/tokens/estimator.go:9-13` | Pin tiktoken-go; add `replace` directive | 15-10 |
| L-12 | `internal/log/log.go:30,71-77` | On rotation failure, log to stderr | 15-10 |
| L-13 | `pkg/ledger/ledger.go:511-526` | `strconv.ParseFloat` not `fmt.Sscanf` | 15-09 |
| L-14 | `internal/tui/components/permission.go:22-28` | Default timeout to 300s | 15-10 |
| L-15 | `internal/tui/modelselector.go` | `lipgloss.Width()` truncation | 15-10 |
| L-16 | `pkg/session/manager.go:518-521` | Rename to `recentModelsAbsolutePath` | 15-09 |
| L-17 | `internal/tui/sidebar.go` | Cache `g.Status()` for 1s | 15-10 |
| L-18 | `internal/tui/repl.go` `SetProvider` | Re-fetch model from new provider | 15-10 |

### Agent's Discretion

The planner may decide:

- **Test framework conventions** — match existing `*_test.go` patterns;
  use `testify/assert` and `testify/require` (already in `go.sum`).
- **Concrete function signatures** for new helpers (e.g.,
  `safeCloseOnce`, `resolveAndPinIP`) — follow Go idioms and
  `internal/errors` sentinel style.
- **Whether to introduce new types** like `Capability` enum vs.
  extending `CapFlags`. Prefer extending existing types unless the
  audit explicitly calls for a new struct.
- **Doc comment style** — match the existing `// Xxx ...`
  convention in each file. Reference the audit finding ID in
  comments where useful (e.g., `// Fix C-3: ...`).
- **Whether to fold two Lows into one task** when they touch the
  same file (e.g., L-1 and L-6 both reference timeout constants).

</decisions>

<canonical_refs>

## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Audit
- `rush/comprehensive_deep_audit_2026.md` — Source of truth for all
  72 findings. Every PLAN.md task must cite the audit finding ID
  in its `objective`.

### Project Memory
- `AGENTS.md` — Architecture rules, build commands, threading
  model, sentinel error policy, CGO_ENABLED=0 constraint.
- `.planning/PROJECT.md` — Project goal, framework, build constraint.
- `.planning/ROADMAP.md` — Phase 15 section (this phase).
- `.planning/STATE.md` — Current state (Phase 14 in progress).

### Architecture
- `docs/ARCHITECTURE.md` — Package dependency graph, data flow,
  threading model, state persistence, error handling strategy.
- `docs/INTERFACES.md` — Mirrors of `LLMProvider`, `Tool`,
  `Dispatcher`, `PermissionRequest`/`Response`, `Config`, etc.
- `docs/TYPES.md` — Constants, sentinel errors, env vars, enums.

### Phase 14 Wiring (predecessor)
- `.planning/phases/14/14-CONTEXT.md` — Decisions on Discuss Q&A,
  msgChan drainer, state persistence.
- `internal/tui/app_update.go` — Phase 14 touched the Discuss
  Q&A flow; C-1 nil-guards are **additive** to those changes.
- `internal/tui/streaming.go` — Phase 14 closed `streamCh` (D-10
  resource leak); C-3 is the **next level** of channel ownership.

### Code
- `internal/types/constants.go` — `BashTimeout`, `SessionIDLength`,
  `MaxToolOutputChars`, etc.
- `internal/errors/errors.go` — 15 sentinel errors; add new ones
  per audit.
- `internal/types/types.go` — `RiskLevel`, `WorkflowPhase`,
  `TaskStatus`, `Usage`, `ToolCall`, `Message`, `MessageSegment`.

</canonical_refs>

<specifics>

## Specific Ideas

### Regression test requirement (audit closing note)

> *"A separate sweep is needed for tests: **none of these findings
> have a regression test that would have caught them.** Recommend a
> 'fix + test' pairing for every Critical and High in the next
> sprint."*

Every Critical and High task in every PLAN.md **must** include a
regression test in its `acceptance_criteria` that fails before the
fix and passes after. The test name should be traceable to the
finding ID (e.g., `TestC1_ReplModelNil_DoesNotPanic`).

### Streaming ownership rule

The post-Phase-15 streaming architecture is:

```
StartStreamCmd(req) -> tea.Cmd that:
  1. Allocates (streamCh, streamDone, cancel) internally
  2. Spawns goroutine that:
     - Calls provider.ChatCompletionStream
     - Forwards each StreamChunk as tea.Msg
     - On done, sends StreamDoneMsg then closes streamDone
  3. Returns a read-cmd that drains streamCh
The REPL NEVER sees streamCh or streamDone. It only sees
StreamChunkMsg / StreamDoneMsg / StreamErrorMsg on the BT update
loop.
```

This is the canonical "channels owned by the goroutine that writes
to them" pattern from the Go community.

### Typed-error migration

Before any string-match is removed, the corresponding sentinel
must exist in `internal/errors/errors.go` AND the provider
normalization layer (`internal/provider/chat.go`) must wrap the
HTTP error with the typed sentinel. The TUI consumer
(`calculateNextInterval`) then uses `errors.Is`.

### Verifier scope rule

`verifyTask` runs `go build <packages derived from task.Files>`
where the package list is computed by `go list ./...` intersected
with the directories in `task.Files`. If no packages match, skip
build verification but still run `os.Stat` and syntax check.

</specifics>

<deferred>

## Deferred Ideas

- **M-32 AppState refactor** — already deferred from Phase 14
  (D-11). Coordinator pattern is a v1.1 concern. The audit
  confirms M-32 is structural debt but not blocking v1.0.
- **V1.1 features** — Ghost mode, Terminal PiP, Concurrent
  subagents, FileEdit, WebSearch, AgentTool, TaskTool,
  AskUserQuestion, GitTool (separate roadmap phase).
- **MCP / Plugin / Vision** — explicit v2.0+.

</deferred>

---

*Phase: 15-comprehensive-audit-fixes*
*Context gathered: 2026-06-02 from `rush/comprehensive_deep_audit_2026.md`*
