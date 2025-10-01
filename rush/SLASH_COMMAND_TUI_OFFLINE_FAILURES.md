# Slash Command & TUI Offline Mode Failure Report

> **Date:** 2026-06-06
> **Scope:** Why slash commands and TUI do not work completely without an API key
> **Severity:** 2 Critical, 4 Medium, 5 Low

---

## Executive Summary

The M31A TUI launches successfully without an API key (offline mode), and most slash commands work correctly. However, **several commands fail silently, show misleading errors, or crash** when the API key is missing. The root causes fall into three categories:

1. **Misleading error messages** — Workflow commands show "No active session. Start a workflow first." when the real problem is no API key configured. Users who skip first-run setup get confused.
2. **Nil pointer dereference risks** — `/history` and `/log` access `ctx.Config` without nil checks, which can panic in edge cases.
3. **Post-first-run state gaps** — After completing first-run setup and returning to REPL, the workflow engine is never re-initialized, and the session ID is never stored on `AppState`, leaving workflow commands permanently broken until restart.

---

## Architecture: How Offline Mode Works

### Startup Flow

```
main.go:78  → config.Load()           // loads ~/.m31a/config.toml (or defaults)
main.go:88  → keychain.New()          // OS keychain (graceful fallback)
main.go:93  → cfg.ResolveAPIKeys(kc)  // env → keychain → config (never errors)
main.go:99  → provider.NewRegistry()  // always created
main.go:102 → openrouter.New()        // ONLY if apiKey != ""
main.go:132 → zen.New()               // ONLY if apiKey != ""
main.go:161 → registry.SetActive()    // sets active provider
main.go:167 → warn if no provider     // logs "no active provider"
main.go:171 → tui.NewApp()            // TUI always launches
```

### Screen Selection (app.go:390-443)

| Condition | Screen |
|-----------|--------|
| `resolvedAPIKey == ""` | `ScreenFirstRun` |
| `ResumeOnStartup && sessions exist` | `ScreenResume` |
| `registry == nil \|\| ActiveProvider() == nil` | `ScreenREPL` (offline) |
| Provider available | `ScreenREPL` (normal) |

### Offline Mode Banner (app.go:422-434)

```go
app.healthStatus = types.HealthStatus{
    Status:  "offline",
    Error:   "No providers available — offline mode. History is readable but no new messages.",
}
app.currentOperation = "No providers available — offline mode."
```

---

## Bug Details

### BUG-1 (CRITICAL) — `/workflow <goal>` shows misleading "No active session" error

**File:** `internal/tui/commands_workflow.go:152-153`
**Repro:** Start M31A with no API key → skip first-run → type `/workflow build a REST API`

**Root Cause:** When no API key is configured, `initWorkflowEngine()` returns early at `app_workflow.go:20-27` because `m.registry == nil` or `m.activeProvider == ""`. The `m.workflowEngine` field remains `nil`. When the user types `/workflow build a REST API`:

1. `app_update.go:308`: The `/workflow ` prefix matches, but `m.workflowEngine != nil` is false → block skipped
2. Falls to command registry dispatch
3. `handleWorkflow` is called with args `["build", "a", "REST", "API"]`
4. At `commands_workflow.go:152`: `ctx.SessionID == ""` is true (no session was created)
5. Returns: **"No active session. Start a workflow first."**

**The real problem:** The user IS trying to start a workflow. The real error is "No API key configured."

**Impact:** Users who skip first-run setup see a confusing error that suggests they need to start a workflow first, when they are literally trying to do that.

---

### BUG-2 (CRITICAL) — `/workflow resume` shows same misleading error

**File:** `internal/tui/commands_workflow.go:123-127`
**Repro:** Same as BUG-1

**Root Cause:** Same mechanism. `ctx.SessionID == ""` because no workflow engine was initialized.

**Error shown:** "No active session. Start a workflow first."
**Should show:** "No workflow engine available. Configure an API key via /settings or restart M31A."

---

### BUG-3 (MEDIUM) — First-run completion does not re-initialize workflow engine

**File:** `internal/tui/app_update.go:770-804`
**File:** `internal/tui/app.go:341`

**Root Cause:** `initWorkflowEngine()` is called once during `NewApp()` at `app.go:341`. When no API key is configured at startup, it returns early. After the user completes first-run setup (selects a provider, enters a key), a session is created and a provider is registered — but `initWorkflowEngine()` is **never called again**. The `m.workflowEngine` remains `nil` for the entire session.

**Impact:** All workflow commands (`/workflow`, `/plan`, `/execute`, `/verify`, `/ship`, `/phase`) are permanently broken until the user restarts M31A.

---

### BUG-4 (MEDIUM) — First-run completion does not store session ID on AppState

**File:** `internal/tui/app_update.go:774-781`

```go
sessionID := ""
if m.sessionManager != nil && m.activeModel != nil && m.activeProvider != "" {
    s, err := m.sessionManager.NewSession(m.activeModel.ID, m.activeProvider)
    if err == nil {
        sessionID = s.ID
        m.dispatcher.SetSessionID(s.ID)
    }
}
```

The session ID is stored in a local variable and passed to `m.replModel.SetProvider()`, but **never assigned to `m.sessionID`** (the `AppState` field). Code that reads `m.sessionID` (e.g., workflow resume at `app_update.go:329`) sees an empty string.

**Impact:** Combined with BUG-3, even if the workflow engine were re-initialized, session-related commands would fail.

---

### BUG-5 (MEDIUM) — `/history` panics on nil `ctx.Config`

**File:** `internal/tui/commands_session.go:130`

```go
limit := ctx.Config.UI.SessionListLimit
```

`ctx.Config` is accessed without a nil check. While `m.config` is always initialized in `NewApp()`, this is a latent crash risk. If config loading fails or the config watcher reloads to a nil state, this line panics.

**Contrast with properly guarded commands:**
- `handleTheme` (commands_core.go:112): `if ctx.Config == nil { return ... }`
- `handleConfig` (commands_config.go:82): `if ctx.Config == nil { return ... }`
- `handleCost` (commands_config.go:266): `if ctx.Config == nil { return ... }`
- `handleKey` (commands_config.go:338): `if ctx.Config == nil { return ... }`

---

### BUG-6 (MEDIUM) — `/log` panics on nil `ctx.Config`

**File:** `internal/tui/commands_git.go:304`

```go
n := ctx.Config.UI.DefaultLogLines
```

Same issue as BUG-5. No nil check on `ctx.Config` before accessing `UI.DefaultLogLines`.

---

### BUG-7 (LOW) — `/models` silently falls through when registry is nil

**File:** `internal/tui/app_update.go:233-239`

```go
case "/models":
    if m.registry != nil {
        m.prevScreen = m.screen
        m.modelSelector = NewModelSelector(m.registry, m.sessionManager, m.themeManager.Current())
        m.screen = ScreenModelSelector
        return m, m.modelSelector.Init()
    }
```

When `m.registry == nil`, the `if` body is skipped with no `return`. Execution falls through to the command registry where `handleModels` shows "No active provider. Use /provider or /settings to configure one." — which is correct but the silent fallthrough is fragile.

**Impact:** Low. The fallback works, but the early interception should have an explicit else branch.

---

### BUG-8 (LOW) — `/model --selector` opens ModelSelector without checking registry

**File:** `internal/tui/commands_config.go:26-29`

```go
if args[0] == "--selector" {
    screen := ScreenModelSelector
    return CommandResult{Success: true, Screen: &screen, Message: "Opening model selector..."}
}
```

The `--selector` flag bypasses the `ctx.Registry != nil` check at line 31 and transitions to the ModelSelector screen. If registry is nil, the ModelSelector may receive a nil registry and render a broken or empty screen.

---

### BUG-9 (LOW) — Health check tickers run needlessly in offline mode

**File:** `internal/tui/app_update.go:796-803`

After first-run skip, health check and cache refresh tickers are started. They detect `m.registry == nil` and reschedule every 60s indefinitely, creating a pointless timer loop.

**Impact:** Minor resource waste (a timer tick every 60s that immediately reschedules).

---

### BUG-10 (LOW) — `/optimize` error message not actionable

**File:** `internal/tui/commands_config.go:297-299`

Returns "No active provider." without telling the user how to fix it.

**Should show:** "No active provider. Configure an API key via /settings or /key."

---

### BUG-11 (LOW) — `/key` shows "(from config)" even for env/keychain sources

**File:** `internal/tui/commands_config.go:346`

The display text always says "(from config)" regardless of whether the key was resolved from an environment variable or keychain. Misleading but cosmetic.

---

## Commands That DO Work Without API Key (Verified)

| Command | File | Status |
|---------|------|--------|
| `/help` | commands_core.go:12 | ✅ Works |
| `/status` | commands_core.go:70-88 | ✅ Works (nil-safe) |
| `/theme` | commands_core.go:111-129 | ✅ Works (nil-safe) |
| `/config` | commands_config.go:81-84 | ✅ Works (nil-safe) |
| `/settings` | commands_core.go:55-58 | ✅ Works |
| `/health` | commands_config.go:367-405 | ✅ Works (nil-safe) |
| `/clear` | commands_core.go:61-67 | ✅ Works |
| `/quit` | commands_core.go:106-108 | ✅ Works |
| `/tokens` | commands_ai.go:115-131 | ✅ Works |
| `/cost` | commands_config.go:265-282 | ✅ Works (nil-safe) |
| `/key` | commands_config.go:337-363 | ✅ Works (nil-safe) |
| `/model` (no args) | commands_config.go:17-24 | ✅ Works (nil-safe) |
| `/provider` | commands_config.go:56-78 | ✅ Works (nil-safe) |
| `/sessions` | commands_session.go:162-187 | ✅ Works (nil-safe) |
| `/ledger` | commands_git.go:16-45 | ✅ Works (nil-safe) |
| `/diff` | commands_git.go:216-288 | ✅ Works (nil-safe) |
| `/rollback` | commands_git.go:74-190 | ✅ Works (nil-safe) |
| `/tools` | commands_workflow.go:85-112 | ✅ Works (nil-safe) |
| `/save` | commands_session.go:189-204 | ✅ Works |
| Shell mode (`!cmd`) | repl.go:379-387 | ✅ Works |

---

## Commands That FAIL Without API Key

| Command | Failure Mode | Severity | File:Line |
|---------|-------------|----------|-----------|
| `/workflow <goal>` | Misleading "No active session" | **CRITICAL** | commands_workflow.go:152 |
| `/workflow resume` | Misleading "No active session" | **CRITICAL** | commands_workflow.go:123 |
| `/plan <goal>` | Silently dropped (engine nil) | MEDIUM | app_update.go:255 |
| `/execute` | Silently dropped (engine nil) | MEDIUM | app_update.go:255 |
| `/verify` | Silently dropped (engine nil) | MEDIUM | app_update.go:255 |
| `/ship` | Silently dropped (engine nil) | MEDIUM | app_update.go:255 |
| `/phase <name>` | Falls to registry, shows "idle" | LOW | app_update.go:242-300 |
| `/models` | Falls through, shows error | LOW | app_update.go:233-239 |
| `/model --selector` | Opens broken screen | LOW | commands_config.go:26 |
| `/optimize` | "No active provider" (not actionable) | LOW | commands_config.go:297 |
| `/compress` | "AutoDream not available" | LOW | commands_ai.go:48-50 |
| `/history` | Works but nil-unsafe | MEDIUM | commands_session.go:130 |
| `/log` | Works but nil-unsafe | MEDIUM | commands_git.go:304 |

---

## Root Cause Analysis

### Why Does This Happen?

1. **`initWorkflowEngine()` is called exactly once** during `NewApp()` (app.go:341). If it fails due to missing API key, there is no retry mechanism. The workflow engine stays nil forever.

2. **Session creation requires a model and provider** (app_update.go:775). Without an API key, `m.activeModel` is nil and `m.activeProvider` is empty, so no session is created. Commands that depend on `ctx.SessionID` get an empty string.

3. **Workflow commands check `m.workflowEngine != nil`** before processing (app_update.go:255, 308). When nil, they fall through to the command registry, where `handleWorkflow` and `handlePhase` check `ctx.SessionID` instead — which is also empty. This double-nil path produces misleading errors.

4. **`ctx.Config` nil checks are inconsistent.** Some commands guard against nil Config (`handleTheme`, `handleConfig`, `handleCost`, `handleKey`) while others don't (`handleHistory`, `handleLog`). This is a code quality gap.

### Why Wasn't This Caught?

- The offline mode was designed for the case where the user has no API key AND doesn't try to use workflow commands. The assumption was that users would use `/settings` to add a key before using `/workflow`.
- The first-run "Skip" path was added as a convenience, but the post-skip state (no session, no workflow engine) wasn't fully tested against all slash commands.
- Nil safety tests exist (`app_update_nilsafety_test.go`) but don't cover `SlashCommandMsg` with nil `cmdRegistry` or the first-run completion path.

---

## Recommended Fixes

### Fix 1: Re-initialize workflow engine after first-run completion

**File:** `internal/tui/app_update.go:770-804`

After the provider is registered and session is created in the first-run completion block, add:

```go
// After setting up provider, model, and session:
m.initWorkflowEngine()
```

### Fix 2: Store session ID on AppState after first-run

**File:** `internal/tui/app_update.go:774-781`

Change:
```go
if err == nil {
    sessionID = s.ID
    m.dispatcher.SetSessionID(s.ID)
}
```
To:
```go
if err == nil {
    sessionID = s.ID
    m.sessionID = s.ID
    m.dispatcher.SetSessionID(s.ID)
}
```

### Fix 3: Fix misleading workflow error messages

**File:** `internal/tui/commands_workflow.go:152-153`

Change:
```go
if ctx.SessionManager == nil || ctx.SessionID == "" {
    return CommandResult{Success: false, Message: "No active session. Start a workflow first."}
}
```
To:
```go
if ctx.WorkflowEngine == nil {
    return CommandResult{Success: false, Message: "No workflow engine available. Configure an API key via /settings or restart M31A to run first-run setup."}
}
if ctx.SessionManager == nil || ctx.SessionID == "" {
    return CommandResult{Success: false, Message: "No active session. Start a workflow first."}
}
```

Apply the same pattern to `/workflow resume` at lines 123-127.

### Fix 4: Add nil guards to `/history` and `/log`

**File:** `internal/tui/commands_session.go:130`

Add before line 130:
```go
if ctx.Config == nil {
    limit = 20
} else {
    limit = ctx.Config.UI.SessionListLimit
}
```

**File:** `internal/tui/commands_git.go:304`

Add before line 304:
```go
n := 20
if ctx.Config != nil {
    n = ctx.Config.UI.DefaultLogLines
}
```

### Fix 5: Add explicit error for `/models` when registry is nil

**File:** `internal/tui/app_update.go:233-239`

Change:
```go
case "/models":
    if m.registry != nil {
        // ... open model selector
    }
```
To:
```go
case "/models":
    if m.registry == nil {
        if m.replModel != nil {
            m.replModel.AddSystemMessage("No providers configured. Use /settings to add an API key.")
        }
        return m, nil
    }
    m.prevScreen = m.screen
    m.modelSelector = NewModelSelector(m.registry, m.sessionManager, m.themeManager.Current())
    m.screen = ScreenModelSelector
    return m, m.modelSelector.Init()
```

### Fix 6: Guard `/model --selector` against nil registry

**File:** `internal/tui/commands_config.go:26-29`

Change:
```go
if args[0] == "--selector" {
    screen := ScreenModelSelector
    return CommandResult{Success: true, Screen: &screen, Message: "Opening model selector..."}
}
```
To:
```go
if args[0] == "--selector" {
    if ctx.Registry == nil {
        return CommandResult{Success: false, Message: "No providers configured. Use /settings to add an API key."}
    }
    screen := ScreenModelSelector
    return CommandResult{Success: true, Screen: &screen, Message: "Opening model selector..."}
}
```

### Fix 7: Skip health check tickers in offline mode

**File:** `internal/tui/app_update.go:796-803`

Wrap the health/cache ticker start in a provider check:
```go
if m.activeProvider != "" && m.registry != nil {
    healthCmd := HealthCheckTicker(context.Background(), types.HealthCheckInterval)
    cmds = append(cmds, healthCmd)
    cmds = append(cmds,
        CacheRefreshTicker(m.activeProvider, provider.DefaultCacheRefreshInterval),
        providerCmd)
}
```

### Fix 8: Make `/optimize` error actionable

**File:** `internal/tui/commands_config.go:297-299`

Change:
```go
return CommandResult{Success: false, Message: "No active provider."}
```
To:
```go
return CommandResult{Success: false, Message: "No active provider. Configure an API key via /settings or /key."}
```

---

## Testing Recommendations

1. **Add nil safety test for `SlashCommandMsg`** in `app_update_nilsafety_test.go`:
   - Test with nil `cmdRegistry`
   - Test with nil `workflowEngine`
   - Test with nil `config`
   - Test with nil `sessionManager`

2. **Add first-run completion integration test**:
   - Simulate first-run skip → verify offline mode
   - Simulate first-run with key → verify workflow engine is initialized
   - Verify `m.sessionID` is set after first-run completion

3. **Add slash command offline test suite**:
   - Test every slash command with nil registry/provider
   - Verify no panics, verify error messages are helpful
   - Test `/workflow`, `/plan`, `/execute`, `/verify`, `/ship` specifically

---

## Summary

| Category | Count | Details |
|----------|-------|---------|
| Critical | 2 | Misleading workflow errors (BUG-1, BUG-2) |
| Medium | 4 | Workflow engine not re-init (BUG-3), session ID not stored (BUG-4), nil Config panics (BUG-5, BUG-6) |
| Low | 5 | Silent fallthroughs, broken screens, resource waste (BUG-7 through BUG-11) |
| **Total** | **11** | |

The most impactful fix is **Fix 3** (correcting the misleading error messages), which directly addresses the user's reported symptom. The most architecturally important fix is **Fix 1** (re-initializing the workflow engine after first-run), which makes the entire post-first-run flow functional.
