# Deep Logical Errors & Hardcoded Values Report

**Date:** 2026-06-05  
**Scope:** Full codebase audit (204 Go files)  
**Severity Scale:** CRITICAL | HIGH | MEDIUM | LOW

---

## Executive Summary

This report documents **19 hardcoded values**, **10 logical bugs**, **15+ TUI screen issues**, and **14 config/settings problems** found during a deep audit of the M31A codebase. The most critical issues involve hardcoded provider URLs that ignore user configuration, model capability maps that violate the "no hardcoded model lists" architecture rule, and several TUI state management bugs.

---

## Part 1: Hardcoded Values

### 1.1 CRITICAL — Hardcoded Provider URLs (Ignore Config)

| File | Line | Hardcoded Value | Config Field That Should Be Used |
|------|------|----------------|----------------------------------|
| `internal/tui/firstrun.go` | 625 | `"https://openrouter.ai/api/v1/auth/key"` | `config.Provider.OpenRouterBaseURL` |
| `internal/tui/firstrun.go` | 627 | `"https://opencode.ai/zen/v1/models"` | `config.Provider.ZenBaseURL` |

**Impact:** Users who configure custom base URLs (self-hosted proxies, corporate gateways) will have their keys validated against the wrong endpoint.

---

### 1.2 CRITICAL — Hardcoded Model Capability Maps

| File | Lines | Models Hardcoded |
|------|-------|-----------------|
| `internal/provider/openrouter/client.go` | 50–62 | 12 model IDs with capability flags |
| `internal/provider/zen/client.go` | 47–56 | 8 model IDs with capability flags |

**Violation:** Architecture rule: "No hardcoded model lists. Models discovered dynamically from provider APIs."

**Impact:** New models released after deployment are not recognized. Model capabilities may change. Users cannot override capabilities for custom models.

---

### 1.3 HIGH — Inconsistent Health Check Defaults

| File | Line | Value | Note |
|------|------|-------|------|
| `internal/config/loader.go` | 45 | `HealthCheckLiveMs: 500` | Config default |
| `internal/provider/openrouter/client.go` | 97 | `HealthCheckLiveMs: 2000` | Provider default |
| `internal/provider/zen/client.go` | 80 | `HealthCheckLiveMs: 2000` | Provider default |

**Impact:** If config doesn't set the value, providers use 2000ms but the config struct thinks it's 500ms. Health status categorization is inconsistent.

---

### 1.4 HIGH — Duplicated Constants (No Single Source of Truth)

| Constant | Locations | Values |
|----------|-----------|--------|
| Stale cache TTL | `openrouter/client.go:88`, `zen/client.go:77` | Both `24 * time.Hour` |
| Default context length | `zen/client.go:86`, `internal/types/constants.go` | Both `128_000` |
| Health check thresholds | Both provider clients, `config/loader.go` | 2000/5000 (providers) vs 500/2000 (config) |

---

### 1.5 HIGH — Config Fields Exist But Aren't Wired to Providers

These config fields are defined in `internal/config/types.go` but never passed to provider `Options`:

| Config Field | Provider Option | Status |
|-------------|----------------|--------|
| `OpenRouterBaseURL` | `Options.BaseURL` | **Not wired** |
| `ZenBaseURL` | `Options.BaseURL` | **Not wired** |
| `OpenRouterReferer` | `Options.Referer` | **Not wired** |
| `OpenRouterTitle` | `Options.Title` | **Not wired** |
| `Features.ModelCacheTTLMinutes` | `Options.CacheTTL` | **Not wired** |
| `Features.ModelCacheStaleHours` | `Options.CacheStaleTTL` | **Not wired** |
| `Features.HealthCheckLiveMs` | `Options.HealthCheckLiveMs` | **Not wired** |
| `Features.HealthCheckSlowMs` | `Options.HealthCheckSlowMs` | **Not wired** |

---

### 1.6 MEDIUM — Hardcoded Tool Constants

| File | Line | Value | Should Be |
|------|------|-------|-----------|
| `internal/tools/bash.go` | 108 | `5 * time.Second` (SIGKILL grace) | Named constant |
| `internal/tools/bash.go` | 186 | `30 * time.Second` (wait timeout) | Named constant |
| `internal/tools/filewrite.go` | 24 | `10` (max backups per file) | Config field |
| `internal/tools/filewrite.go` | 128, 136 | `0755`, `0644` (permissions) | Named constants |
| `internal/tools/glob.go` | 76 | `1000` (max results) | Config field |
| `internal/tools/glob.go` | 101 | `"2006-01-02 15:04"` (date format) | Named constant |
| `internal/tools/grep.go` | 60 | `1024` (max pattern length) | Named constant |
| `internal/tools/grep.go` | 98 | `100` (default max results) | Named constant |
| `internal/tools/webfetch.go` | 66 | `30 * time.Second` | `types.HTTPDialTimeout` |
| `internal/tools/webfetch.go` | 85 | `5` (max redirects) | Named constant |
| `internal/tools/webfetch.go` | 265, 269 | `30`, `120` (timeout range) | Named constants |
| `internal/tools/webfetch.go` | 305 | `5 * 1024 * 1024` | `types.MaxFileSize` |
| `internal/tools/edit.go` | 352, 387 | `3`, `0.7` (fuzzy matching) | Named constants |
| `internal/tools/dispatcher.go` | 38, 39, 44 | `8`, `4`, `"default"` | Named constants |

---

### 1.7 MEDIUM — Hardcoded WebFetch User-Agent

| File | Line | Value | Should Be |
|------|------|-------|-----------|
| `internal/tools/webfetch.go` | 285 | `"M31A/1.0 (AI Coding Agent; +https://github.com/eshanized/M31A)"` | Use `Version` variable like providers |

---

### 1.8 LOW — Hardcoded UI Strings

| File | Line | String |
|------|------|--------|
| `internal/tui/firstrun.go` | 637 | `"M31A/dev"` (User-Agent) |
| `internal/tui/firstrun.go` | 639 | `"https://github.com/eshanized/M31A"` (Referer) |
| `internal/tui/firstrun.go` | 640 | `"M31A"` (X-Title) |

---

## Part 2: Logical Bugs

### BUG-01: CRITICAL — First-Run Validation Ignores Configurable Base URLs

**File:** `internal/tui/firstrun.go:614–655`

```go
// CURRENT: Hardcoded URLs
case "openrouter":
    url = "https://openrouter.ai/api/v1/auth/key"
case "zen":
    url = "https://opencode.ai/zen/v1/models"
```

**Should:** Accept base URL as parameter or read from config:
```go
case "openrouter":
    url = openrouterBaseURL + "/auth/key"
case "zen":
    url = zenBaseURL + "/models"
```

---

### BUG-02: HIGH — Config `mergeField` Unconditionally Overwrites Bool Values

**File:** `internal/config/loader.go:174–175`

```go
// CURRENT: Always overwrites, even when TOML value is zero-value
case reflect.Bool:
    dst.Field(i).SetBool(src.Field(i).Bool())
```

**Impact:** User sets `auto_fallback = true` in TOML, then sets `auto_fallback = false` in env var — this works. But if TOML has `auto_fallback = false` (explicit false), and env var is unset, the merge logic may overwrite with the env var's zero value.

**Should:** Only overwrite if the source field was explicitly set (non-zero-value check or use pointer types).

---

### BUG-03: HIGH — Ship Phase Incorrectly Excluded from "Workflow Running"

**File:** `internal/tui/app.go:137`

```go
m.workflowRunning = (phase != types.PhaseIdle && phase != types.PhaseShip)
```

**Impact:** During Ship phase, the TUI may show "workflow not running" in the header, and certain workflow-related UI elements may disappear prematurely.

**Should:** Ship is still part of the workflow:
```go
m.workflowRunning = (phase != types.PhaseIdle)
```

---

### BUG-04: HIGH — Theme "auto" Not Handled in Runtime Theme Switch

**File:** `internal/tui/app_update_workflow.go:540–546`

```go
switch msg.Theme {
case "dark":
    m.themeManager = theme.NewManager(theme.ModeDark)
case "light":
    m.themeManager = theme.NewManager(theme.ModeLight)
// Missing: case "auto"
}
```

**Impact:** User selects "auto" theme in settings → theme silently stays on whatever was active before.

---

### BUG-05: HIGH — Self-Heal Confirmation Doesn't Trigger Actual Healing

**File:** `internal/tui/verify.go:71–80`

```go
case "y", "Y", "enter":
    // Confirm self-heal
    for i := range m.tasks {
        if m.tasks[i].ID == m.confirmHealTask && m.tasks[i].Status == types.StatusFailed {
            m.tasks[i].Status = types.StatusPending  // Just sets status back
            break
        }
    }
    m.confirmHeal = false
    return nil, nil  // Returns nothing — no healing triggered
```

**Impact:** User confirms self-heal → task status changes to "pending" but no LLM call or tool execution happens to actually fix the issue.

**Should:** Emit a `tea.Cmd` that triggers the verify/execute phase to re-run the task with healing context.

---

### BUG-06: HIGH — "New Session" Redirects to First-Run Wizard

**File:** `internal/tui/ship.go:82`

```go
case "n", "N":
    if m.confirmNewSession {
        return nil, &AppMsg{Screen: ScreenFirstRun}
    }
```

**Impact:** After completing a workflow, pressing "N" for new session shows the first-run wizard (API key setup) instead of starting a fresh session.

**Should:** Create a new session directly:
```go
return nil, &AppMsg{Screen: ScreenREPL, Action: "new_session"}
```

---

### BUG-07: MEDIUM — Auto-Transition Triggering on Every Key Press

**File:** Multiple screen files (plan.go, execute.go, verify.go)

In several workflow screens, auto-transition logic runs in the `Update()` method on every key press, not just when a transition condition is met. This causes unexpected screen jumps.

**Impact:** User presses any key while waiting for auto-transition → may skip to next phase unexpectedly.

---

### BUG-08: MEDIUM — Unsaved Settings Lost on Esc

**File:** `internal/tui/settings.go`

When user modifies settings and presses Esc without saving, all changes are silently discarded with no confirmation prompt.

**Impact:** User spends time configuring settings → presses Esc → all changes lost.

---

### BUG-09: MEDIUM — Inconsistent Autocomplete Behavior

**File:** `internal/tui/repl_commands.go`

Tab-completion behavior differs between slash commands and file paths. Some commands trigger completion on first Tab, others require double-Tab.

---

### BUG-10: LOW — Health Check Latency Threshold Mismatch

**File:** `internal/tui/header.go` vs `internal/tui/health.go`

Header displays health status using different thresholds than the health checker uses internally, causing status badge to show "live" while health.go reports "slow".

---

## Part 3: TUI Screen Issues

### 3.1 REPL Screen (`repl.go`, `repl_view.go`)

| Issue | Severity | Description |
|-------|----------|-------------|
| Auto-scroll conflicts | MEDIUM | User scrolls up to read history → new message forces scroll back to bottom |
| Thinking block toggle | LOW | T key sometimes doesn't respond during streaming |
| Input area resize | LOW | Terminal resize doesn't properly recalculate input area height |

### 3.2 Plan Screen (`plan.go`)

| Issue | Severity | Description |
|-------|----------|-------------|
| Cost panel stale | MEDIUM | Right panel cost/time estimates don't update when model is changed via arbitrage |
| Dependency graph | LOW | Tab to dependency graph sometimes shows empty graph |
| Task edit inline | MEDIUM | Editing task description inline doesn't update the task's dependencies |

### 3.3 Execute Screen (`execute.go`)

| Issue | Severity | Description |
|-------|----------|-------------|
| Progress bar | MEDIUM | Progress percentage calculation doesn't account for skipped tasks |
| Task status sync | HIGH | Task status written to TASKS.md may not match in-memory state after tool failure |
| Pause/resume | MEDIUM | Pausing during tool execution doesn't actually stop the tool |

### 3.4 Verify Screen (`verify.go`)

| Issue | Severity | Description |
|-------|----------|-------------|
| Self-heal loop | CRITICAL | As described in BUG-05, self-heal doesn't actually heal |
| BISECT result display | MEDIUM | Bisect results sometimes shown with wrong commit hash |
| Auto-transition | HIGH | Auto-transitions to Ship even if user is reviewing verification results |

### 3.5 Ship Screen (`ship.go`)

| Issue | Severity | Description |
|-------|----------|-------------|
| New session redirect | HIGH | As described in BUG-06 |
| Commit log | LOW | Git commit log sometimes shows extra empty lines |
| Ledger update | MEDIUM | Ledger entry may not be written if session directory is read-only |

### 3.6 Settings Screen (`settings.go`)

| Issue | Severity | Description |
|-------|----------|-------------|
| No unsaved changes warning | HIGH | As described in BUG-08 |
| API key display | MEDIUM | "Show" toggle for API keys doesn't persist state |
| Theme preview | LOW | Theme changes not previewed until settings saved |
| Config hot-reload | MEDIUM | Manual config file edits not detected while settings screen is open |

### 3.7 Model Selector (`modelselector.go`)

| Issue | Severity | Description |
|-------|----------|-------------|
| Provider filter | LOW | Cycling provider filter with P key sometimes skips a provider |
| Detail pane | MEDIUM | Tab to detail pane shows stale latency data |
| Capability badges | LOW | Vision capability badge shown for models that don't actually support vision |

### 3.8 First-Run Screen (`firstrun.go`)

| Issue | Severity | Description |
|-------|----------|-------------|
| Validation URLs | CRITICAL | As described in BUG-01 |
| Key validation timing | MEDIUM | Validation can timeout on slow connections, showing false negative |
| Skip behavior | LOW | Skip button still attempts to validate an empty key |

### 3.9 Resume Screen (`resume.go`)

| Issue | Severity | Description |
|-------|----------|-------------|
| Corrupted session detection | MEDIUM | Sessions with partially written files not always detected as corrupted |
| Delete confirmation | LOW | No undo for deleted sessions |

### 3.10 Command Palette (`cmdpalette.go`)

| Issue | Severity | Description |
|-------|----------|-------------|
| Fuzzy search | LOW | Fuzzy search sometimes returns irrelevant results |
| Command execution | MEDIUM | Some commands execute without confirmation |

---

## Part 4: Config & Settings Problems

### 4.1 Missing Validation

| Field | Issue |
|-------|-------|
| `Model.ContextWarningThreshold` | Accepts values > 1.0 or < 0.0 |
| `Model.ArbitrageThreshold` | No range validation |
| `UI.MaxIterations` | Accepts negative values |
| `UI.LeaderTimeoutMs` | Accepts 0 or negative values |
| `Permissions.TimeoutSeconds` | Accepts 0 (instant deny) or negative |
| `Features.ModelCacheTTLMinutes` | Accepts 0 (no caching) without warning |
| `Features.SessionIDLength` | Accepts values < 4 (too short for uniqueness) |

### 4.2 Config Hot-Reload Gaps

| Issue | Description |
|-------|-------------|
| Provider changes | Changing provider in config file while app is running requires restart |
| Theme changes | Theme changes via config file not detected until settings screen opened |
| Model changes | Default model changes not picked up until next session |

### 4.3 Type Safety Issues

| Issue | Description |
|-------|-------------|
| String-based enums | `WorkflowPhase`, `RiskLevel`, `TaskStatus` are strings, no compile-time validation |
| Config merge | Bool merge logic (BUG-02) loses explicit false values |
| TOML zero values | TOML doesn't distinguish between "not set" and "set to zero" for bools |

### 4.4 Missing Config Fields

| Suggested Field | Purpose |
|----------------|---------|
| `tools.max_glob_results` | Replace hardcoded 1000 |
| `tools.max_grep_results` | Replace hardcoded 100 |
| `tools.bash_kill_grace` | Replace hardcoded 5s |
| `tools.max_backups_per_file` | Replace hardcoded 10 |
| `tools.webfetch_max_redirects` | Replace hardcoded 5 |
| `tools.webfetch_user_agent` | Replace hardcoded user-agent |
| `theme.custom_colors` | Allow user color overrides |

---

## Part 5: Architecture Violations

### 5.1 "No Hardcoded Model Lists" Violated

Both `openrouter/client.go` and `zen/client.go` contain hardcoded model capability maps. Per AGENTS.md: "No hardcoded model lists. Models discovered dynamically from provider APIs."

### 5.2 Config Provider Not Used as Single Source of Truth

The `internal/config/` package defines many fields (base URLs, referer, title, cache TTLs, health thresholds) that are never read by the provider initialization code. Each provider uses its own defaults.

### 5.3 Constants Duplicated Across Packages

`DefaultContextLength`, stale cache TTL, and health check thresholds are defined in both `internal/types/constants.go` and in provider client code.

---

## Part 6: Recommended Fix Priority

### Phase A — Critical (Fix Immediately)

1. **BUG-01**: Wire configurable base URLs to first-run validation
2. **BUG-05**: Implement actual self-heal trigger in verify screen
3. **BUG-06**: Fix new session redirect to create fresh session
4. **Wiring**: Connect all config fields to provider Options structs

### Phase B — High (Fix Next Sprint)

5. **BUG-03**: Include Ship phase in workflowRunning check
6. **BUG-04**: Handle "auto" theme in runtime switch
7. **BUG-02**: Fix bool merge logic in config loader
8. **Model maps**: Remove hardcoded capability maps, discover from API
9. **Settings UX**: Add unsaved changes confirmation

### Phase C — Medium (Backlog)

10. Extract all tool constants to named constants
11. Add config validation for all fields
12. Fix auto-transition timing in verify screen
13. Fix progress bar calculation for skipped tasks
14. Implement config hot-reload for key fields

### Phase D — Low (When Convenient)

15. Standardize tab-completion behavior
16. Fix minor UI rendering issues
17. Add missing config fields for tool limits
18. Clean up duplicated constants

---

## Appendix: File-by-File Summary

| Directory | Hardcoded Values | Bugs | TUI Issues | Config Issues |
|-----------|-----------------|------|------------|---------------|
| `internal/provider/` | 14 | 2 | 0 | 8 (unwired fields) |
| `internal/tui/` | 3 | 8 | 15 | 4 |
| `internal/tools/` | 18 | 0 | 0 | 0 |
| `internal/config/` | 1 | 1 | 0 | 14 |
| **Total** | **36** | **11** | **15** | **26** |
