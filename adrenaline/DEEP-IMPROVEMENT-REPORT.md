# M31A Deep Codebase Improvement Report

**Date:** 2026-06-08
**Scope:** Full codebase deep-read across all 14 packages (~50K lines)
**Prior audits:** `AUDIT.md` (C-1..C-3, H-1..H-5, M-1..M-7, L-1..L-8), `PLAN-FIX-ALL.md` (Wave 0-5)
**This report:** Covers issues **not found** in prior audits, plus deeper analysis of known problems

---

## Table of Contents

1. [Critical Issues](#1-critical-issues)
2. [High Severity Issues](#2-high-severity-issues)
3. [Medium Severity Issues](#3-medium-severity-issues)
4. [Low Severity Issues](#4-low-severity-issues)
5. [Architecture & Design Debt](#5-architecture--design-debt)
6. [Performance Concerns](#6-performance-concerns)
7. [Testing Gaps](#7-testing-gaps)
8. [Build & CI/CD](#8-build--cicd)
9. [UX & Interaction Issues](#9-ux--interaction-issues)
10. [Positive Observations](#10-positive-observations)
11. [Prioritized Action Plan](#11-prioritized-action-plan)

---

## 1. Critical Issues

### D-1: View() Mutates State — Violates Bubble Tea's Core Contract

**Files:** `internal/tui/app_view.go:249`

```go
func (m *AppState) renderPermissionModal() string {
    if m.permRequest == nil {
        m.screen = ScreenREPL  // <-- MUTATION in View()
        return m.renderREPLScreen()
    }
```

Bubble Tea's architecture mandates that **all state mutations happen in Update()**, never in View(). View() is called on every frame redraw and may be called concurrently with Update() in some Bubble Tea modes. Mutating `m.screen` here is a data race that can cause the app to render the wrong screen or crash.

**Fix:** Move the screen correction to the Update() handler. In View(), just render the current screen without side effects.

---

### D-2: ensureReplModel() Silently Discards Provider Command

**Files:** `internal/tui/app_view.go:276`

```go
func (m *AppState) ensureReplModel() {
    // ...
    if m.registry != nil {
        cmd := m.replModel.SetProvider(m.registry, m.activeProvider, m.activeModel, m.sessionID, m.config)
        _ = cmd // will be run on next Init call
    }
}
```

The `SetProvider` call returns a `tea.Cmd` that fetches model information. This command is discarded with `_ = cmd`. The comment says "will be run on next Init call" but `Init()` is only called once at app startup. If `ensureReplModel()` is called after startup (which it is — from multiple screen transitions), the provider setup command never executes. This means model information may be stale or missing after session creation.

**Fix:** Return the cmd from `ensureReplModel()` and include it in the Update() command batch.

---

### D-3: Permission Channel Re-Queue Can Deadlock

**Files:** `internal/tools/permissions.go:216-231`

```go
for {
    select {
    case r := <-d.responseCh:
        if r.RequestID == req.ID {
            resp = r
            goto done
        }
        // Not ours — put it back and keep looking
        select {
        case d.responseCh <- r:
        default:
        }
    // ...
    }
}
```

When two tools request permission simultaneously, each enters `sendAndWaitForPermission` waiting for their specific `RequestID`. If tool A's response arrives first, tool B reads it, tries to put it back, but the channel may be full (buffer = `PermissionChannelBuffer`). The `default:` case silently drops the response, causing tool A to wait forever until timeout.

**Fix:** Use a per-request response channel instead of a shared channel. The dispatcher should route responses by request ID using a map of channels.

---

### D-4: Health Check Response Body Not Drained Before Close

**Files:** `internal/provider/openrouter/client.go:261-262`, `internal/provider/zen/client.go:241-242`

```go
io.Copy(io.Discard, io.LimitReader(resp.Body, types.MaxLLMResponseBytes))
defer resp.Body.Close()
```

The `defer resp.Body.Close()` runs when the function returns, but `io.Copy` runs immediately. This ordering is correct. However, if `io.Copy` fails (e.g., network error mid-drain), the body is still closed by the defer — that's fine. But the **real issue** is that `io.Copy` can block indefinitely if the server keeps the connection open without sending data. There's no read deadline set on the response body.

**Fix:** Set a read deadline on the connection or wrap the drain with a context timeout.

---

## 2. High Severity Issues

### D-5: Theme Change Logic Duplicated Three Times

**Files:** `internal/tui/app_update.go:215-260`, `app_update.go:797-807`

The theme switching logic appears in three places with near-identical code:
1. `ThemeChangedMsg` handler (lines 215-237)
2. `settingsThemeChanged` handler (lines 239-260)
3. `handleKeyAction("toggle_theme")` (lines 797-807)

Each creates a new `theme.Manager`, sets it on all sub-models, and propagates the change. Any modification (e.g., adding a new sub-model) must be updated in all three places.

**Fix:** Extract a single `applyTheme(themeName string)` method and call it from all three handlers.

---

### D-6: `context.Background()` Used in 7+ TUI Operations

**Files:** `internal/tui/health.go:22,32`, `cache_refresh.go:35`, `repl_state.go:72`, `repl_commands.go:20`, `settings_model.go:177`, `firstrun_model.go:198`

All of these should use the app's `shutdownCtx` instead. When the app exits, these operations continue running, potentially:
- Writing to closed channels
- Holding open HTTP connections
- Causing panics on nil references

The `AppState` already has `shutdownCtx` — it just isn't being passed through to these helpers.

**Fix:** Pass `shutdownCtx` through all TUI command constructors. For helpers that don't have access to `AppState`, add a context parameter.

---

### D-7: Grep and Glob Tools Don't Pass Context to `exec.Command`

**Files:** `internal/tools/grep.go:166`, `internal/tools/glob.go:146`

```go
cmd := exec.Command("rg", args...)
```

Both tools use `exec.Command` instead of `exec.CommandContext(ctx, ...)`. When the context is cancelled (user presses Ctrl+C, timeout expires), the `rg` subprocess continues running as a zombie process.

**Fix:** Use `exec.CommandContext(ctx, "rg", args...)` in both tools.

---

### D-8: Provider Version Variables Never Set from main.go

**Files:** `internal/provider/openrouter/client.go:19`, `internal/provider/zen/client.go:20`, `cmd/m31a/main.go:104-105`

**Update:** The existing AUDIT.md (L-1) flagged this, but I confirmed that main.go **does** set these at lines 104-105:

```go
openrouter.Version = Version
zen.Version = Version
```

However, the `Version` variable in main.go defaults to `"dev"` and is only overridden via `-ldflags` during release builds. The issue is actually that these are **package-level mutable globals** set from another package — a fragile pattern. If the init order changes or if a test creates a client without setting the version, it silently uses "dev".

**Fix:** Pass version through the `Options` struct at client construction time instead of relying on package globals.

---

### D-9: No Retry Logic for Transient Provider Failures

**Files:** `internal/workflow/engine.go:497-530`

The `streamLLM` function makes a single attempt to contact the LLM provider. If the provider returns a 503, times out, or has a transient network issue, the entire workflow phase fails. For an AI coding agent that may run for 10+ minutes, a single transient failure shouldn't abort the whole workflow.

The provider layer has a `fallback.go` file for provider-to-provider fallback, but there's no retry-within-provider logic.

**Fix:** Add exponential backoff retry (2-3 attempts) for transient errors (503, timeout, network errors) in `streamLLM`. Use `errors.Is` to check for `ErrProviderUnreachable` and `ErrRateLimited`.

---

### D-10: `openResumeScreen` Loads All Sessions in a Single Goroutine

**Files:** `internal/tui/app_update.go:852-870`

```go
func (m *AppState) openResumeScreen() tea.Cmd {
    return func() tea.Msg {
        var sessions []*session.Session
        // ... loads up to 20 sessions sequentially
        return resumeScreenReadyMsg{sessions: sessions}
    }
}
```

This loads up to 20 full sessions (including messages.json for each) in a single blocking goroutine. For users with many sessions, each containing hundreds of messages, this can take several seconds during which the UI shows a loading state.

**Fix:** Load session metadata only (not messages) for the list view, and load messages lazily when a session is selected.

---

## 3. Medium Severity Issues

### D-11: WebFetch `resolveAndCheck` for Redirects Bypasses DNS Cache

**Files:** `internal/tools/webfetch.go:115-119`

```go
CheckRedirect: func(req *http.Request, via []*http.Request) error {
    if err := wf.resolveAndCheck(req.Context(), req.URL.String()); err != nil {
        // ...
    }
    return nil
},
```

The redirect checker calls `resolveAndCheck` which does a fresh DNS resolution. But the dialer uses `resolveAndCache` which stores results in the DNS cache. This inconsistency means:
1. Redirect targets get a fresh DNS lookup (defeating the DNS pinning protection)
2. The redirect check's resolved IP may differ from the dialer's resolved IP (TOCTOU)

**Fix:** Use `resolveAndCache` in the redirect checker too, or share a single resolution path.

---

### D-12: SSE Parser Doesn't Handle `retry:` or `id:` Fields

**Files:** `internal/provider/sse.go:78-88`

The SSE specification defines four field types: `data:`, `event:`, `id:`, and `retry:`. The parser only handles `data:` and `event:`. While most LLM providers don't use `id:` or `retry:`, some may send `retry:` to control reconnection timing. Silently ignoring these is acceptable for V1, but should be documented.

**Fix:** Add a comment documenting the intentional omission, or log unknown fields at debug level.

---

### D-13: `replaceBlockTag` and `replaceInlineTag` Re-Lowercase HTML on Every Iteration

**Files:** `internal/tools/webfetch.go:469,501`

```go
lower := strings.ToLower(html)
for {
    // ... find match ...
    html = html[:start] + replacement + inner + html[closeEnd:]
    lower = strings.ToLower(html)  // <-- O(n) re-lowercase on every match
}
```

For large HTML documents with many tags, this is O(n*k) where n is document size and k is number of tags. Each iteration re-processes the entire document. The `convertLinks` function has the same issue.

**Fix:** Use a single-pass HTML parser or at minimum track position offsets instead of re-scanning.

---

### D-14: Edit Tool Creates Temp Files with 0666 Permissions

**Files:** `internal/tools/edit.go:225`

```go
tmpFile, err := os.Create(tmpPath)
```

`os.Create` uses `0666` permissions (before umask). The config loader's `atomicWrite` correctly uses `0600`. For a tool that may handle sensitive code, the temp file should use restrictive permissions.

**Fix:** Use `os.OpenFile(tmpPath, os.O_CREATE|os.O_WRONLY|os.O_TRUNC, 0600)` instead.

---

### D-15: `lineTrimmedReplace` Indentation Preservation Is Fragile

**Files:** `internal/tools/edit.go:327-332`

```go
for k, ncLine := range newContentLines {
    if k < len(oldLines) && i+k < len(contentLines) {
        indent := leadingWhitespace(contentLines[i+k])
        ncLine = indent + strings.TrimSpace(ncLine)
    }
    newLines = append(newLines, ncLine)
}
```

This assumes the new content lines map 1:1 to old content lines for indentation purposes. If the replacement has more or fewer lines than the original, the indentation mapping breaks — lines beyond the old count get no indentation, and lines within the count may get the wrong indentation if the structure changed.

**Fix:** Only apply indentation preservation to the first line (which anchors the block), and use the first line's indentation as the base for subsequent new lines.

---

### D-16: `isPassUnavailable` False Positive on Non-Error Exit Codes

**Files:** `pkg/keychain/keychain_linux.go:297-305`

```go
func isPassUnavailable(err error) bool {
    if exitErr, ok := err.(*exec.ExitError); ok {
        return len(exitErr.Stderr) > 0
    }
    _, isExecErr := err.(*exec.Error)
    return isExecErr
}
```

Any `pass` exit error with stderr output is treated as "pass unavailable". But `pass` writes to stderr for many reasons (GPG agent issues, pinentry timeouts, etc.) that don't mean pass is unavailable — they mean the operation failed for a specific reason. This causes the keychain to incorrectly report `ErrKeychainUnavailable` instead of the actual error.

**Fix:** Check stderr for specific "not installed" or "not found" patterns rather than any stderr output.

---

### D-17: `stripTags` in WebFetch Doesn't Handle Nested Tags of Same Type

**Files:** `internal/tools/webfetch.go:434-466`

```go
func stripTags(html string, tags ...string) string {
    for _, tag := range tags {
        for {
            start := strings.Index(html, "<"+tag)
            // ...
            closeTag := "</" + tag + ">"
            closeStart := strings.Index(html[tagEnd:], closeTag)
```

For nested `<div><div>inner</div>outer</div>`, this finds the first `<div` and the first `</div>`, removing `<div><div>inner</div>` and leaving `outer</div>`. The correct behavior would be to match balanced tags, but that requires a real parser.

**Fix:** Document the limitation. For production use, consider adding `golang.org/x/net/html` for proper HTML parsing.

---

### D-18: Config `WatchConfig` Uses Polling Instead of Filesystem Notifications

**Files:** `internal/config/loader.go:624-647`

The config watcher polls the file's modification time every 5 seconds. This works but:
1. Wastes CPU cycles checking an unchanged file
2. Has up to 5-second latency for config changes
3. Can miss rapid save-delete-recreate cycles from some editors

**Fix:** Use `fsnotify` for filesystem event notifications, falling back to polling on unsupported platforms.

---

## 4. Low Severity Issues

### D-19: `generateDiffSummary` in Edit Tool Is Not a Real Diff

**Files:** `internal/tools/edit.go:512-545`

The function compares lines by position rather than using a diff algorithm. A single line insertion at the top makes every subsequent line appear as "changed". This produces misleading diff summaries for the LLM.

**Fix:** Use a simple LCS-based diff (even Myers' algorithm) or just report the line range affected.

---

### D-20: `matchesGitignore` Resolves Path Relative to "." Instead of workDir

**Files:** `internal/tools/grep.go:344`

```go
relPath, err := filepath.Rel(".", path)
```

This resolves relative to the process's current directory, not the tool's `workDir`. If the process CWD differs from `workDir` (e.g., when running tests), gitignore matching breaks.

**Fix:** Use `filepath.Rel(t.workDir, path)` instead.

---

### D-21: Duplicate `atomicWrite` Implementations (4 Copies)

**Files:** `internal/config/loader.go:652-687`, `pkg/session/manager.go:67-109`, `internal/tools/edit.go:196-246`, `internal/tools/filewrite.go` (inline)

Four separate implementations of atomic file writing with slight variations in:
- Temp file permissions (0600 vs 0666 vs os.Create)
- Error wrapping styles
- Cleanup strategies

**Fix:** Extract to `internal/fileutil/atomic.go` with a single, well-tested implementation.

---

### D-22: `normalizeWhitespace` Already Fixed But Comment Remains

**Files:** `internal/tools/webfetch.go:615-639`

The existing AUDIT.md (L-3) flagged the O(n*k) `strings.Contains` loop. The current implementation uses a proper single-pass `strings.Builder` approach, but the PERF-2 comment at line 377 still references the old approach.

**Fix:** Update the PERF-2 comment to reflect the current implementation.

---

### D-23: Stray "package 2" Test Failure

**Files:** Unknown

Running `go test ./...` produces:
```
# 2
package 2 is not in std (/usr/lib/go/src/2)
FAIL    2 [setup failed]
```

This suggests a stray file or directory named `2` exists somewhere in the module path that Go interprets as a package.

**Fix:** Find and remove the stray file/directory. Run `find . -name "2" -type f` to locate it.

---

### D-24: `convertLinks` href Parsing Breaks on Unquoted Attributes

**Files:** `internal/tools/webfetch.go:530-583`

```go
hrefStart += start + 6 // len("href=") = 5 + 1 for space
quoteChar := html[hrefStart]
```

This assumes `href=` is always followed by a quote character and a space before `href`. It breaks on:
- `<a href=url>` (unquoted)
- `<a\nhref="url">` (newline between attribute and value)
- `<a  href="url">` (double space)

**Fix:** Add bounds checking and handle unquoted attribute values.

---

### D-25: Sidebar Width Adjustment Doesn't Persist

**Files:** `internal/tui/app_update.go:764-781`

The `sidebar_wider` and `sidebar_narrower` actions adjust the sidebar width at runtime but don't persist the preference to config. On restart, the sidebar reverts to the default width.

**Fix:** Save the sidebar width to config on change, or persist to a separate state file.

---

## 5. Architecture & Design Debt

### D-26: AppState Has 30+ Fields — God Object Pattern

**Files:** `internal/tui/app_state.go:37-142`

The `AppState` struct holds:
- 12 sub-models (repl, sidebar, cmdPalette, plan, execute, verify, ship, settings, resume, ms, firstRun, goal, ledger, rollback, discuss, diff, metrics, config)
- 6 workflow-related fields
- 4 permission modal fields
- 3 optional package references (ledger, rollback, autoDream)
- Multiple timer/tracker fields

This makes AppState a god object that's hard to test and reason about.

**Recommendation:** Extract related fields into composed structs:
- `WorkflowState` (engine, phase, goal, cancel, discussQuestions)
- `PermissionState` (request, countdown, modal, question)
- `SubModels` (all screen models in a registry)

---

### D-27: Screen Routing Uses Both Messages and Direct Mutation

**Files:** `internal/tui/app_update.go`

Screen transitions happen through three different mechanisms:
1. `AppMsg` with `Screen` field → `handleAppMsg` → `navigateToScreen`
2. `KeyActionMsg` → `handleKeyAction` → `navigateToScreen`
3. Direct `m.screen = ScreenX` assignment in Update handlers

This inconsistency makes it hard to reason about which screen is active and when transitions occur.

**Recommendation:** Funnel all screen transitions through a single `navigateTo(screen Screen) tea.Cmd` method.

---

### D-28: Provider Clients Share 80% Code But Don't Extract It

**Files:** `internal/provider/openrouter/client.go`, `internal/provider/zen/client.go`

Both clients have nearly identical implementations of:
- `New()` constructor with options
- `ChatCompletionStream()` HTTP request/response handling
- `makeIterator()` SSE iteration
- `HealthCheck()` latency measurement
- `EstimateCost()` and `GetModel()` delegation

The only differences are:
- Header setup (OpenRouter adds HTTP-Referer, X-Title)
- Model response parsing (different JSON structures)
- Error handling nuances (Zen checks for CreditsError in 401)

**Recommendation:** Extract a `baseClient` struct with shared logic, parameterized by model parser and header setup.

---

### D-29: `tools` Package Imports `config` — Inverted Dependency

**Files:** `internal/tools/dispatcher.go:13`, `internal/tools/permissions.go:10`

The `tools` package imports `config` for `PermissionRule` and `PermissionsAgentConfig` types. This creates a dependency from a lower-level package (tools) to a higher-level one (config), which PLAN-FIX-ALL.md (Violation #1) also identifies.

**Current mitigation:** None — the import cycle is avoided because `config` doesn't import `tools`.

**Recommendation:** Move `PermissionRule` and `PermissionsAgentConfig` to `internal/types/`. The `config` package can embed or alias them.

---

### D-30: Workflow Engine Mixes Orchestration and Execution

**Files:** `internal/workflow/engine.go`, `internal/workflow/execute.go`, `internal/workflow/engine_parse.go`

The workflow engine handles:
- Phase orchestration and transitions
- LLM communication (streamLLM)
- Tool execution coordination
- Prompt composition
- Response parsing (JSON, tool calls)
- Task management
- Git operations
- Self-healing logic

This is too many responsibilities for a single package.

**Recommendation:** Extract:
- `workflow/llm` — LLM communication and response parsing
- `workflow/healer` — self-healing logic
- `workflow/prompt` — prompt composition

---

## 6. Performance Concerns

### D-31: Grep Pure-Go Mode Opens Every File for Binary Detection

**Files:** `internal/tools/grep.go:269-292`

```go
f, err := os.Open(path)
// Read first 512 bytes for binary detection
header := make([]byte, 512)
n, _ := f.Read(header)
// Reset reader to beginning for scanning
f.Seek(0, 0)
```

For every file in the search path, grep opens it, reads 512 bytes, seeks back, then scans. In large repos with thousands of files, this creates excessive syscall overhead. The `rg` fallback doesn't have this issue since it handles binary detection internally.

**Fix:** Use file extension heuristics first (known binary extensions: .png, .jpg, .exe, etc.) and only read the header for ambiguous extensions.

---

### D-32: Session List Reads Every `session.json` on Every Call

**Files:** `pkg/session/manager.go:320-401`

`ListSessions` reads and parses every `session.json` file in the sessions directory. With a 2-second cache TTL, this is called frequently. For users with hundreds of sessions, this means hundreds of file reads and JSON parses.

**Fix:** Store a lightweight index file (`sessions_index.json`) with just ID, model, date, and message count. Update it on session save. Only read full session.json when loading a specific session.

---

### D-33: `renderHeader` Calls `git.CurrentBranch()` on Every Frame

**Files:** `internal/tui/app_view.go:367`

```go
if m.git != nil {
    if b, err := m.git.CurrentBranch(); err == nil {
        gitBranch = b
    }
}
```

The header is rendered on every frame redraw. `CurrentBranch()` shells out to `git rev-parse --abbrev-ref HEAD`, which takes 10-50ms. At 30fps, this adds 300-1500ms of git execution per second.

The sidebar already caches the branch via `SidebarRefreshMsg`, but the header ignores it.

**Fix:** Use the cached branch from the sidebar model or a dedicated branch cache updated on sidebar refresh ticks.

---

### D-34: `applyVarSubstitution` Manually Lists Every String Field

**Files:** `internal/config/loader.go:500-523`

```go
cfg.Provider.Default = substituteVars(cfg.Provider.Default)
cfg.Provider.OpenRouter.APIKey = substituteVars(cfg.Provider.OpenRouter.APIKey)
// ... 10+ more lines
```

Every new string field added to the config must also be added here. Missing a field means `${VAR}` substitution silently doesn't work for it.

**Fix:** Use reflection to walk all string fields, or use struct tags to mark fields that need substitution.

---

## 7. Testing Gaps

### D-35: Zero TUI Update/View Tests

**Files:** `internal/tui/app_update.go` (994 lines), `internal/tui/app_view.go` (427 lines)

The core TUI logic — all message handling, screen routing, and rendering — has **zero test coverage**. The `[no test files]` marker in `go test` output confirms this.

**Impact:** Any refactoring of Update() or View() risks regressions. The 994-line Update function is the most complex in the codebase and the least tested.

**Fix:** Add table-driven tests for:
- Key routing to correct screen
- Screen transitions
- Permission modal flow
- Toast lifecycle
- Theme change propagation

---

### D-36: No Benchmark Tests Exist

**Files:** Entire codebase

Despite having performance-sensitive code (SSE parsing, HTML conversion, token estimation, grep scanning), there are **zero benchmark tests** (`func Benchmark*`).

**Fix:** Add benchmarks for:
- `SSEParser.Next()` with various event sizes
- `htmlToMarkdown()` with realistic HTML payloads
- `tokens.Estimator.EstimateMessages()` with large message histories
- `grepPureGo()` with large repos

---

### D-37: Workflow Engine Has No Integration Tests

**Files:** `internal/workflow/`

The workflow tests (`engine_test.go`, `execute_test.go`, `workflow_test.go`) use mock providers and test individual phases in isolation. There are no tests that exercise the full discuss → plan → execute → verify → ship pipeline.

**Fix:** Add an integration test that runs the full pipeline with a mock provider and verifies:
- Phase transitions are valid
- Planning files are created
- Tasks are generated and executed
- Final state is correct

---

### D-38: No Fuzz Tests for Input Parsing

**Files:** `internal/provider/sse.go`, `internal/workflow/engine_parse.go`, `internal/tools/webfetch.go`

The SSE parser, JSON tool call parser, and HTML converter all process untrusted external input but have no fuzz tests.

**Fix:** Add `func Fuzz*(f *testing.F)` tests for:
- `SSEParser.Next()` with malformed SSE streams
- `ParseSSEChunk()` with corrupted JSON
- `htmlToMarkdown()` with adversarial HTML
- `parseToolCalls()` with malformed LLM responses

---

## 8. Build & CI/CD

### D-39: CI Uses `golangci-lint-action@v4` — Outdated

**Files:** `.github/workflows/ci.yml:28`

The golangci-lint action is on v4 but v6 is current. The v4 action may not support the latest linters or Go 1.24.

**Fix:** Update to `golangci/golangci-lint-action@v6`.

---

### D-40: No `.goreleaser.yaml` Validation in CI

**Files:** `.github/workflows/ci.yml`

The release job runs GoReleaser, but the lint/test/build jobs don't validate the GoReleaser config. A broken `.goreleaser.yaml` would only be caught at release time.

**Fix:** Add a `goreleaser check` step to the lint job.

---

### D-41: Coverage Not Enforced or Reported

**Files:** `.github/workflows/ci.yml:42-47`

Coverage is collected and uploaded as an artifact, but:
- No minimum coverage threshold is enforced
- Coverage percentage isn't displayed in PR checks
- No coverage badge in README

**Fix:** Add a coverage threshold check (e.g., 70% minimum) and use `actions/upload-artifact` with coverage summary.

---

### D-42: `tiktoken-go` Replace Directive Is a No-Op

**Files:** `go.mod:56`

```
replace github.com/pkoukk/tiktoken-go => github.com/pkoukk/tiktoken-go v0.1.8
```

This replaces the package with the exact same version — it has no effect. The comment says "replace with maintained fork if upstream goes stale" but the replace directive doesn't actually do that.

**Fix:** Remove the no-op replace directive. When the upstream goes stale, add the actual fork replacement.

---

## 9. UX & Interaction Issues

### D-43: No Visual Feedback During Model Fetching

**Files:** `internal/tui/modelselector.go`

When the model selector screen opens, it fetches models from the provider API. During this fetch (which can take 5-15 seconds on slow connections), there's no loading spinner or progress indicator — just "Loading model selector..." text.

**Fix:** Add a spinner component during model fetch.

---

### D-44: Permission Modal Countdown Doesn't Reset Between Requests

**Files:** `internal/tui/app_update.go:132`

```go
m.permCountdown = msg.Request.TimeoutSecs
```

The countdown is set from the request's `TimeoutSecs`, but if the modal is reused across requests without full re-initialization, the previous countdown value may flash briefly.

**Fix:** Reset `permCountdown` to 0 before setting the new value, or ensure the modal is always freshly created.

---

### D-45: `ctrl+c` Exits Immediately Without Confirmation

**Files:** `internal/tui/app_update.go:31-36`

```go
case "ctrl+c":
    if m.streamCancelFn != nil {
        m.streamCancelFn()
        m.streamCancelFn = nil
    }
    return m, nil
```

Pressing `ctrl+c` cancels any active stream and returns immediately. For long-running sessions, an accidental `ctrl+c` exits the entire app without saving state or confirming. The workflow state is persisted on phase transitions, but the current REPL conversation is lost.

**Fix:** First `ctrl+c` cancels the active stream (if any). Second `ctrl+c` within 2 seconds exits the app. Show a toast: "Press ctrl+c again to exit".

---

### D-46: No Keyboard Shortcut Help Overlay

**Files:** `internal/tui/keybindings.go`

The app has a rich keybinding system (leader key + chords, screen-specific shortcuts) but no way to discover them from within the app. Users must read the source code or documentation.

**Fix:** Add a `?` or `ctrl+/` shortcut that shows a keybinding reference overlay.

---

## 10. Positive Observations

The codebase has many strong patterns worth preserving:

1. **Atomic file writes** — consistently used across all file operations with proper temp-file-then-rename
2. **SSRF protection** — DNS pinning, private IP blocking, redirect checking in WebFetch is excellent
3. **Singleflight for cache refresh** — prevents thundering herd on model cache expiry
4. **Path traversal protection** — all file tools resolve symlinks and verify containment
5. **Bubble Tea single-threaded discipline** — properly maintained in all Update handlers (except D-1)
6. **Error sentinel pattern** — well-defined error types in `internal/errors` with proper wrapping
7. **Config hot-reload** — file watching with atomic writes is a nice touch
8. **Session fork/child tracking** — clean parent-child relationship management
9. **Backup pruning** — automatic cleanup prevents unbounded backup growth
10. **Context cancellation** — properly propagated through workflow phases and tool execution

---

## 11. Prioritized Action Plan

### Immediate (Sprint 1 — Critical)

| # | Issue | Effort | Impact |
|---|-------|--------|--------|
| D-1 | View() state mutation | 15min | Fixes data race |
| D-2 | Discarded provider command | 30min | Fixes stale model info |
| D-3 | Permission deadlock | 2hr | Fixes potential hang |
| D-7 | Context not passed to exec | 15min | Fixes zombie processes |
| D-14 | Temp file permissions | 5min | Security hardening |

### Short-term (Sprint 2 — High)

| # | Issue | Effort | Impact |
|---|-------|--------|--------|
| D-5 | Theme duplication | 1hr | Code quality |
| D-6 | context.Background() | 1hr | Resource leaks |
| D-9 | Provider retry | 2hr | Reliability |
| D-10 | Session loading | 1hr | UX performance |
| D-33 | Git branch in View | 30min | Frame rate |

### Medium-term (Sprint 3 — Architecture)

| # | Issue | Effort | Impact |
|---|-------|--------|--------|
| D-26 | AppState refactor | 4hr | Testability |
| D-28 | Provider base client | 3hr | Code reuse |
| D-29 | Move PermissionRule | 2hr | Dependency cleanup |
| D-21 | Atomic write extraction | 1hr | Code reuse |
| D-35 | TUI tests | 8hr | Regression safety |

### Long-term (Backlog)

| # | Issue | Effort | Impact |
|---|-------|--------|--------|
| D-36 | Benchmarks | 4hr | Performance visibility |
| D-38 | Fuzz tests | 4hr | Security hardening |
| D-37 | Workflow integration tests | 4hr | Confidence |
| D-30 | Workflow refactor | 6hr | Maintainability |
| D-18 | fsnotify config watch | 2hr | Responsiveness |

---

*Report generated: 2026-06-08*
*Total new issues: 46 (D-1 through D-46)*
*Combined with prior audits: 46 + 23 (AUDIT.md) + 20 (PLAN-FIX-ALL.md) = 89 total tracked issues*
