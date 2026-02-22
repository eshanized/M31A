# Test Coverage Gap Analysis — Path to 90%

**Generated**: 2026-06-10
**Toolchain**: `go test -coverprofile=cover.out -covermode=count ./...`
**Baseline**: 1 351 exported/unexported functions across 26 packages; overall weighted statement coverage **26.6%** (TUI dominates statement count).

---

## 1. Executive Summary

The codebase is **far from 90% coverage** on two distinct axes:

| Axis | Current | Target | Gap |
|---|---|---|---|
| Per-package coverage (median) | ~65% | 90% | +25 pts |
| Weighted statement coverage | 26.6% | 90% | +63 pts |
| Functions with 0% coverage | **911 / 1 351 (67%)** | ~135 | −776 funcs |
| Failing tests (must-fix first) | **3** | 0 | 3 bugs |

The single biggest lever is `internal/tui` and its subpackages: 84 Go source files, 839 functions, and **no test files at all**. It owns ~62% of all statements in the repo, so until it gets even modest coverage the weighted number cannot move meaningfully. The second lever is fixing the three broken tests, which are real code bugs (not test bugs).

---

## 2. Per-Package Coverage Heatmap

| Package | % stmts | # funcs | 0%-funcs | Test files | Priority |
|---|---|---|---|---|---|
| `cmd/m31a` | 0.0% | 3 | 3 | 0 | Skip (main) |
| `cmd/firstrunpreview` | 0.0% | 1 | 1 | 0 | Skip |
| `internal/fileutil` | 0.0% | 1 | 1 | 0 | **Easy win** |
| `internal/types` | 0.0% | 1 | 1 | 0 | **Easy win** |
| `internal/tui` | 0.0% | 839 | 839 | 0 | **Critical blocker** |
| `pkg/keychain` | 2.1% | 16 | 14 | 1 | High |
| `internal/tui/components` | 26.0% | 182 | 120 | 7 | High |
| `internal/git` | 30.8% | 47 | 24 | 1 | High |
| `internal/tui/layout` | 39.3% | 20 | 9 | 1 | Medium |
| `internal/tools` | **43.1% ⚠** | 147 | 65 | 8 | **High + failing test** |
| `internal/tui/theme` | 49.7% | 35 | 22 | 3 | Medium |
| `internal/provider` | 53.8% | 68 | 17 | 4 | High |
| `internal/config` | **62.6% ⚠** | 17 | 5 | 1 | **High + failing test** |
| `internal/workflow` | 64.5% | 75 | 24 | 5 | Medium |
| `pkg/session` | 64.7% | 56 | 16 | 6 | Medium |
| `internal/provider/openrouter` | 75.0% | 12 | 2 | 1 | Medium |
| `internal/log` | 77.6% | 5 | 0 | 1 | Easy win |
| `pkg/autodream` | 80.0% | 15 | 3 | 1 | Easy win |
| `pkg/bisect` | 83.6% | 6 | 2 | 1 | Easy win |
| `internal/errors` | 83.8% | 1 | 0 | 1 | Easy win |
| `pkg/ledger` | 85.3% | 17 | 0 | 1 | Easy win |
| `internal/provider/zen` | **89.4% ⚠** | 10 | 2 | 1 | **Near-90 + failing test** |
| `internal/tokens` | 91.1% | 8 | 1 | 1 | ✓ At target |
| `pkg/taskrunner` | 91.4% | 8 | 0 | 1 | ✓ At target |
| `pkg/rollback` | 91.5% | 11 | 0 | 1 | ✓ At target |
| `pkg/arbitrage` | 94.0% | 8 | 0 | 1 | ✓ At target |

Legend: ⚠ = package has at least one failing test.

---

## 3. Failing Tests — Must-Fix Before Anything Else

These are real code bugs, not stale assertions. Each one masks a logic defect.

### 3.1 `TestDispatcher_UnknownTool` — `internal/tools/dispatcher_test.go:52`

- **Symptom**: `expected 'unknown tool', got: tool nonexistent: invalid input JSON: unexpected end of JSON input. Raw input:`
- **Root cause** (`internal/tools/dispatcher.go:132-138`): `Execute` runs `json.Unmarshal(call.Input, &input)` **before** checking `d.tools[call.Name]`. The test passes `types.ToolCall{ID:"call1", Name:"nonexistent"}` with a nil `Input`, so unmarshal errors out before the unknown-tool branch is ever reached. In production this means the error message shown to the model/agent hides the real problem and never lists available tools.
- **Fix**: move the `d.tools[call.Name]` lookup (and the `if !ok { return … }` branch) above the JSON unmarshal, OR tolerate `len(call.Input)==0` by treating it as `{}` before unmarshalling. The second option is more robust since LLMs legitimately emit tool calls with no parameters.
- **Additional tests to add**: empty-`Input` for a *known* tool; `Input: []byte("null")`; `Input: []byte("{}")`; malformed JSON for a known tool (should surface the JSON error, not unknown-tool); concurrent `Execute` across rate-limiter bucket.

### 3.2 `TestVarSubstitution_UnsetVar` — `internal/config/loader_test.go:646`

- **Symptom**: `expected empty string (unset var replaced), got "${UNSET_VAR}"`.
- **Root cause** (`internal/config/loader.go:541-554`): the implementation deliberately **preserves** unresolved patterns (`return match` in the closure) and only warns via slog. The test's expected behavior is "replace with empty string". The code comment and the test disagree on the contract.
- **Decision required**: either (a) change `substituteVars` to replace unresolved vars with `""` (breaking — silently erases typos like `${ANTHROPIC_KEY}`), or (b) fix the test to expect the preserved pattern and add a sibling test that asserts the slog warning fires. (b) is the safer, more-correct fix; the current implementation's behavior is the one users would want when debugging a typo'd env var.
- **Coverage gap**: `WatchConfig`, `DefaultGitConfig`, `types.ParseAnimationSpeed` are all 0%.

### 3.3 `TestChatCompletionStream_WithTools` — `internal/provider/zen/client_test.go:524`

- **Symptom**: `expected tools in request body` — the test captures the JSON body the client POSTs and asserts a `tools` key. The key is absent.
- **Root cause**: the zen client's `ChatCompletionStream` (line 155+) builds its request body from `provider.ChatRequest` but does not copy the `Tools` field into the JSON payload. The openrouter client does. This is a regression — tool use through the Zen gateway is silently broken.
- **Fix**: mirror the openrouter client's tools-serialization into `internal/provider/zen/client.go:155-…`.
- **Coverage gap**: `APIKey`, `CachedModels` are 0%; `HealthCheck` has good coverage but `ChatCompletionStream` has multiple uncovered error branches (SSE parse failures, retry-after handling).

---

## 4. Package-by-Package Coverage Gaps

### 4.1 `internal/tui` — **0% (839 funcs, 84 files, 0 tests)** — the blocker

This is the reason weighted coverage is 26.6%. Files by line count:

| File | Lines | What it does | Testability |
|---|---|---|---|
| `app_update.go` | 1 703 | Master `Update()` dispatch for Bubble Tea | Hard — requires a tea.Program or harness |
| `settings_model.go` | 792 | /settings screen state + view | Medium — pure `Update` on synthetic `tea.Msg` |
| `firstrun_view.go` | 783 | First-run wizard `View()` | Easy — state → string, pure render |
| `sidebar.go` | 676 | Sidebar rendering | Easy — state → string |
| `app_view.go` | 647 | Root `View()` | Hard — dispatches to subviews |
| `firstrun_model.go` | 567 | First-run state machine | Medium — synthetic msgs |
| `cmdpalette.go` | 445 | Command palette fuzzy search + view | Medium — fuzzy logic is pure |
| `plan_model.go` | 372 | Plan-review screen | Easy — pure |
| `types.go` | 357 | Shared TUI types & constants | Trivial — mostly types |
| `repl_state.go` | 357 | REPL state transitions | Medium — pure state |
| `repl_welcome.go` | 355 | Welcome banner rendering | Easy — pure |
| `repl.go` | 379 | REPL `Update` | Hard |
| `app.go` | 383 | Root model init (`New`) | Medium — constructor |
| `statusbar.go` | ~150 | Status bar layout | Easy — pure |
| `streaming.go` | ~150 | SSE chunk accumulation | Medium — pure with fake emitter |
| `toast.go`, `transition.go`, `truncate.go`, `helpers.go` | ~300 | Small pure helpers | Easy |

**Recommendation**: introduce a `tui_test` package with a lightweight harness that constructs `Model` via `New()`, feeds it scripted `tea.Msg` values (including synthetic `tea.KeyMsg`), and asserts on `View()` output as a string. Focus first on the pure render/state functions — they are the bulk of lines and need no harness. Estimated coverage gain: **0% → ~35%** on render/state functions alone, which lifts weighted project coverage from 26.6% → ~47%.

### 4.2 `internal/tui/components` — 26% (182 funcs, 40 files)

Zero-coverage files: `badge.go`, `bash_renderer.go`, `breadcrumb.go`, `card.go`, `codeblock.go`, `confirm.go`, `datatable.go`, `divider.go`, `dropdown.go`, `file_renderers.go`, `filetree.go`, `filterchips.go`, `logo.go`, `message.go`, `metriccard.go`, `notification_list.go`, `permission.go`, `progress.go`, `question.go`, `search.go`, `sparkline.go`, `special_renderers.go`, `spinner.go`, `splitpane.go`, `starfield.go`, `statrow.go`, `tabbar.go`, `taskgraph.go`, `thinking.go`, `timeline.go`, `toolcard.go`, `toolrenderers.go`. Each is a standalone Bubble Tea `Model` — pure `Init/Update/View`. Tests should construct a model, send scripted messages, snapshot the `View()` string. **Estimated gain: 26% → ~55%**.

### 4.3 `internal/tui/theme` — 49.7% (35 funcs, 8 files)

`registry.go` (416 lines) alone has 10 uncovered funcs: `Register`, `Names`, `Get`, `Default`, `Theme.Names`, iteration helpers. Pure map-based — trivial to test. Also: `unicode.go`, `tabs.go`, `shadow.go`, `borders.go`, `colors.go` have uncovered render funcs that take a struct and return a string. **Estimated gain: 49.7% → ~85%**.

### 4.4 `internal/tui/layout` — 39.3% (20 funcs)

Layout functions are pure geometry (width/height math, pane splitting). Add table-driven tests with fixed dimensions. **Estimated gain: 39% → ~80%**.

### 4.5 `pkg/keychain` — 2.1% (16 funcs, 14 uncovered)

`keychain_linux.go` implements `Get/Set/Delete` with two backends: D-Bus `org.freedesktop.secrets` and the `pass` CLI. All 14 functions are untested:

- `Get`, `Set`, `Delete` (public API)
- `dbusGet`, `dbusSet`, `dbusDelete`
- `passGet`, `passSet`, `passDelete`
- `fmtSecret`, `isDBusUnavailable`, `isPassUnavailable`, `isPassNotFound`, `isPassGPGFailure`

**Strategy**: the dbus/pass backends shell out / use real system services — these cannot run in CI. Refactor the `Get/Set/Delete` methods to accept an injectable `backend` interface (or `exec.Command` shimming via a package-level var like `execCommand = exec.Command`). Then add tests that stub the backend and assert error-classification logic (`isPassNotFound`, `isPassGPGFailure`, etc.) using canned stderr strings. **Estimated gain: 2.1% → ~70%**.

### 4.6 `internal/git` — 30.8% (47 funcs, 24 uncovered)

Uncovered: `Run`, `CommitStaged`, `Log`, `LogSince`, `runLog`, `Diff`, `DiffStat`, `DiffStaged`, `DiffFile`, `StatusPorcelain`, `RevParse`, `IsDirty`, `HasUncommittedChanges`, `RemoteTracking`, `AbsPath`, `Fetch`, `Pull`, `Push`, `CheckoutBranch`, `StashList`, `StashApply`, `Merge`, `Tag`, `BranchList`.

The existing `git_test.go` (320 lines) already uses a real temp repo for some tests — extend it. Every function above except `Fetch/Pull/Push` (which need a second bare repo) can be tested against a single `t.TempDir()` repo with `git init` + synthetic commits. `Fetch/Pull/Push` need a `git clone --bare` sibling. **Estimated gain: 30.8% → ~80%**.

### 4.7 `internal/tools` — 43.1% (147 funcs, 65 uncovered)

**Failing test**: see §3.1.

Uncovered by file:

- `edit.go` (17 funcs): `Description`, `RiskLevel`, `Execute`, `resolvePath`, `atomicWrite`, `pruneBackups`, `replaceByLineRange`, `cascadingReplace`, `lineTrimmedReplace`, `whitespaceNormalizedReplace`, `fuzzyAnchorReplace`, `levenshteinSimilarity`, `levenshteinDistance`, `leadingWhitespace`, `detectLineEnding`, `toInt`, `generateDiffSummary`. The fuzzy matcher (`fuzzyAnchorReplace`, `levenshteinSimilarity`) is exactly the kind of pure logic that should have a property-based test suite.
- `webfetch.go` (13 funcs): `SetVersion`, `resolveAndCheck`, `Description`, `RiskLevel`, `htmlToMarkdown`, `htmlToText`, `stripTags`, `replaceBlockTag`, `replaceInlineTag`, `convertLinks`, `decodeHTMLEntities`, `stripAllTags`, `normalizeWhitespace`. SSRF guard (`resolveAndCheck`) needs a table test with loopback/private/link-local IPs.
- `filedelete.go`, `filelist.go`, `filemove.go` (5 funcs each): `Description`, `RiskLevel`, `ParameterSchema`, `Execute`, `humanSize`. `Execute` runs against `t.TempDir()`.
- `question.go`: `nextQuestionRequestID`, `Description`, `RiskLevel`, `ParameterSchema`, `Execute`.
- `todo.go`: `SetSessionID`, `getSessionID`, `Description`, `RiskLevel`, `Execute`, `statusIcon`.
- `dispatcher.go`: `UpdatePermissions`, `QuestionRequestCh`, `QuestionResponseCh`, `SetSessionID`, `Stop`, `RespondQuestion`.
- `permissions.go`: `SetPermission`, `askPermissionWithAgentDefault`.

**Estimated gain: 43.1% → ~80%**.

### 4.8 `internal/provider` — 53.8% (68 funcs, 17 uncovered)

- `cache.go`: `NewModelCacheWithStale`, `FetchTime`, `Models`.
- `capabilities.go`: `ParseModelCapabilities` — pure function, easy win.
- `common.go` (9 funcs): `UserAgent`, `SetCommonHeaders`, `BuildChatBody`, `EstimateCost`, `GetModel`, `CachedModels`, `StaleFallback`, `stripHTMLTags`, `SanitizeProviderError`. `SanitizeProviderError` is important for not leaking API keys into logs — needs explicit test.
- `fallback.go`: `InspectResponse`, `FindFallbackWithRetryAfter`.
- `registry.go`: `RollbackActive`, `Get`.
- `openrouter/client.go`: `APIKey`, `CachedModels`.
- `zen/client.go`: `APIKey`, `CachedModels` (plus failing §3.3).

**Estimated gain: 53.8% → ~85%**.

### 4.9 `internal/config` — 62.6% — see §3.2

Uncovered: `WatchConfig` (long-running goroutine, needs fsnotify fake), `DefaultGitConfig`, several branches in `Load/applyVarSubstitution`, `types.ParseAnimationSpeed`.

**Estimated gain after bug fix: 62.6% → ~85%**.

### 4.10 `internal/workflow` — 64.5% (75 funcs, 24 uncovered)

- `engine.go`: `SetPhaseModel`, `SetModel`, `SessionID`, `SetSessionID`, `SetMsgEmitter`, `HealTask`, `DiscussState`, `SetRefinementFeedback`, `PlanContent`, `PlanVersion`. Most are getters/setters — add a single constructor-style test that exercises them all.
- `plan_parser.go`: `extractFileDescription`, `extractSubsection`, `extractBulletList`. Pure markdown parsers — prime candidates for table-driven tests.
- `verify.go`: `findRootCommit`. Single function, needs a real temp repo.

**Estimated gain: 64.5% → ~85%**.

### 4.11 `pkg/session` — 64.7% (56 funcs, 16 uncovered)

- `manager.go`: `BaseDir`, `recentModelsPath`, `LoadRecentModels`, `SaveRecentModels`, `AddRecentModel`, `ToggleFavorite`, `IsFavorite`, `FilterSessions`, `ExportSessionMarkdown`, `ExportSessionJSON`, `RenameSession`. All are pure filesystem logic — use `t.TempDir()`.
- `planning.go`: `SavePlan`, `LoadPlan`, `SaveDemonstration`, `LoadDemonstration`, `SaveTasksCheckbox`. Same.

**Estimated gain: 64.7% → ~85%**.

### 4.12 Near-90% packages — quick wins

| Package | Current | Gap to 90% | Functions to add |
|---|---|---|---|
| `internal/errors` | 83.8% | +6.2% | `UserMessage` for uncovered sentinel branches |
| `internal/log` | 77.6% | +12.4% | rotation edge cases (file size threshold, signal-based reopen) |
| `pkg/autodream` | 80.0% | +10% | `SetMessages`, plus one test per consolidation strategy |
| `pkg/bisect` | 83.6% | +6.4% | `SetGit`, `Reset` error paths |
| `pkg/ledger` | 85.3% | +4.7% | one or two more entry/lookup tests |
| `internal/provider/openrouter` | 75.0% | +15% | `APIKey`, `CachedModels`, error-path SSE parsing |
| `internal/provider/zen` | **89.4%** | +0.6% | fix §3.3 + test `APIKey` and `CachedModels` getters |

---

## 5. Uncovered Files by Line Count (top 20)

These are the single biggest "dark" files where no function is exercised by any test:

| File | Lines | 0%-funcs |
|---|---|---|
| `internal/tui/app_update.go` | 1 703 | 23 |
| `internal/tui/settings_model.go` | 792 | 27 |
| `internal/tui/firstrun_view.go` | 783 | 16 |
| `internal/tui/sidebar.go` | 676 | 26 |
| `internal/tui/app_view.go` | 647 | 37 |
| `internal/tui/firstrun_model.go` | 567 | 15 |
| `internal/tui/cmdpalette.go` | 445 | 14 |
| `internal/tui/theme/registry.go` | 416 | 10 |
| `internal/tui/components/progress.go` | 401 | 10 |
| `internal/tui/app.go` | 383 | 14 |
| `internal/tui/repl.go` | 379 | 8 |
| `internal/tui/plan_model.go` | 372 | 14 |
| `internal/tui/types.go` | 357 | — (mostly type decls) |
| `internal/tui/repl_state.go` | 357 | 30 |
| `internal/tui/repl_welcome.go` | 355 | 9 |
| `internal/tui/components/toolcard.go` | 352 | — |
| `internal/tui/repl_view.go` | ~330 | 8 |
| `internal/tui/streaming.go` | ~300 | 7 |
| `internal/tui/app_update_phase.go` | ~280 | 5 |
| `internal/tui/components/bash_renderer.go` | ~260 | 7 |

---

## 6. Recommended Phased Plan

### Phase 0 — Fix the three broken tests (1 day)
1. `internal/tools/dispatcher.go` — reorder unknown-tool check before JSON unmarshal (or tolerate nil `Input`).
2. `internal/config/loader_test.go` — align the unset-var test with the documented preserve-on-unset behavior (preferred) or change the implementation.
3. `internal/provider/zen/client.go` — serialize `ChatRequest.Tools` into the request body to match the openrouter client.

### Phase 1 — Easy wins, no refactoring (3-5 days)
Add tests for pure functions in packages already ≥70% covered: `internal/errors`, `internal/log`, `internal/tokens`, `pkg/arbitrage`, `pkg/rollback`, `pkg/taskrunner`, `pkg/ledger`, `pkg/autodream`, `pkg/bisect`, `internal/provider` (cache, common, capabilities, fallback), `internal/provider/openrouter`, `internal/provider/zen`. Target: push each to ≥90%.

### Phase 2 — Temp-dir/file-based packages (3-5 days)
`pkg/session` (recent models, plans, demos, rename, export, filter), `pkg/keychain` (after backend injection refactor), `internal/config` (`WatchConfig` with fsnotify fake, `DefaultGitConfig`, animation-speed parser), `internal/fileutil.AtomicWrite`, `internal/types.SkipDirsMap`. Target: each ≥85%.

### Phase 3 — Git-requiring packages (2-3 days)
`internal/git` (extend existing temp-repo harness to the 24 uncovered funcs), `pkg/bisect` edge cases, `internal/workflow.verify.findRootCommit`. Target: `internal/git` ≥80%.

### Phase 4 — Tools package (5-7 days)
After fixing §3.1, add tests for `edit.go` (esp. fuzzy matcher — property-based), `webfetch.go` (SSRF table tests against `httptest.Server` + private-IP fixtures), `filelist/filedelete/filemove/todo/question`. Dispatcher channel tests (`UpdatePermissions`, `Stop`, `RespondQuestion`). Target: ≥85%.

### Phase 5 — Workflow (3-4 days)
Test the engine's getters/setters/`HealTask`/`DiscussState` in one sweep; table tests for `plan_parser`'s `extractFileDescription/extractSubsection/extractBulletList`. Target: ≥85%.

### Phase 6 — TUI (2-3 weeks, the long tail)
1. **Create a TUI test harness** in `internal/tui/tui_test_harness.go` (or `_test.go`): a `testProgram` that exposes `Send(msg tea.Msg)` and `View() string` by embedding `tea.Program` in a headless mode, OR bypass `tea.Program` entirely and call `Model.Update(msg)` + `Model.View()` directly. The direct-call approach works for ~80% of state transitions.
2. **Pure render functions first** — `sidebar`, `statusbar`, `firstrun_view`, `repl_welcome`, `breadcrumb`, `toast`, `truncate`, `helpers`, `types`, `theme/*`, `layout/*`. Pure: state → string.
3. **State machines second** — `firstrun_model`, `settings_model`, `cmdpalette`, `plan_model`, `repl_state`, `bisect_model`, `resume_model`, `diff_model`, `config_model`, `themepicker_model`, `phasemodelpicker`, `modelselector`. Feed synthetic `tea.Msg` sequences.
4. **Components** — one test file per component in `internal/tui/components/`. Most are already isolated `Model`s.
5. **Integration** — scripted `app_update` / `app_update_phase` / `app_update_commands` scenarios using the harness.
6. **Skip** `app_channel.go` and the main event loop (exercised indirectly).

Target: `internal/tui` 0% → 40%, subpackages → 60-80%. This alone moves weighted coverage from 26.6% → ~55%.

### Phase 7 — cmd/* (skip)
`main()` is 3 lines of wiring. Not worth testing — mark as an explicit exception in CI coverage thresholds.

---

## 7. Projected Coverage After Each Phase

| Phase | Weighted stmts | Per-package median |
|---|---|---|
| Baseline | 26.6% | ~65% |
| After Phase 0 (bug fixes) | 26.6% | ~65% (tests green) |
| After Phase 1 | 29% | ~88% |
| After Phase 2 | 32% | ~88% |
| After Phase 3 | 34% | ~88% |
| After Phase 4 | 38% | ~87% |
| After Phase 5 | 40% | ~87% |
| After Phase 6 | **~60%** | **~88%** |

Getting **per-package median to 90%** is achievable in phases 0-5 (~3-4 weeks). Getting **weighted statement coverage to 90%** requires finishing Phase 6 (another 2-3 weeks) because the TUI is so large. An honest long-term target is 80% weighted; 90% weighted would require refactoring the TUI to extract pure logic out of Bubble Tea `Model`s, which is a much larger architectural undertaking.

---

## 8. Risks & Caveats

1. **Bubble Tea single-threaded model**: `Update` and `View` must be called from the same goroutine. A test harness that calls them from `t.Parallel()` goroutines will flake. Keep TUI tests sequential per model.
2. **Exec-dependent packages** (`pkg/keychain`, `internal/git`, parts of `pkg/bisect`): require either backend injection or a real subprocess. Don't try to mock by patching package-level vars from parallel tests — use `t.Setenv`-style isolation.
3. **`WatchConfig`** depends on `fsnotify`; test with a real `t.TempDir()` and small sleeps, or make the watcher interface-injectable. The former is brittle; the latter is a worthwhile refactor.
4. **Coverage mode**: `count` mode was used for this report. `atomic` mode would be more accurate for parallel tests but is not needed for the gap analysis.
5. **The three failing tests are not test bugs** — they expose real defects in production code paths. Fixing them is higher-priority than adding new tests.
