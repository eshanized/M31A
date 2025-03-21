# Pre-Phase 6 Fix Prompt

Fix 5 findings from the Phase 0-5 cross-phase audit (`rush/audit_0_5.md`). All fixes are surgical — no refactoring, no new features, only targeted changes to address audit findings. Do not modify any file beyond what is specified below.

---

## Fix 1: Wire main.go to Launch TUI (CRITICAL)

**File:** `cmd/m31a/main.go`

**Problem:** The binary prints `"M31A %s — AI coding assistant"` and `"Run 'make build' to build. Implementation coming in Phase 1+."` then exits immediately. None of the Phase 2-5 functionality is reachable. The TUI app (`internal/tui/app.go`) has a complete `NewApp()` constructor and implements `tea.Model`, but `main()` never calls it or starts the Bubble Tea event loop.

**What to do:** Replace the placeholder `main()` body (lines 28-29) with a full application bootstrap sequence.

### Step 1: Add imports

Keep existing imports (`fmt`, `os`, `runtime`, `github.com/eshanized/M31A/internal/log`). Add these:

```go
tea "github.com/charmbracelet/bubbletea"
"github.com/eshanized/M31A/internal/config"
"github.com/eshanized/M31A/internal/provider"
or "github.com/eshanized/M31A/internal/provider/openrouter"
"github.com/eshanized/M31A/internal/provider/zen"
"github.com/eshanized/M31A/internal/tui"
"github.com/eshanized/M31A/pkg/keychain"
```

### Step 2: Replace lines 28-29 with full bootstrap

Remove:
```go
fmt.Printf("M31A %s — AI coding assistant\n", Version)
fmt.Println("Run 'make build' to build. Implementation coming in Phase 1+.")
```

Replace with:

```go
// Resolve config path
configPath := os.ExpandEnv("$HOME/.m31a/config.toml")
if envPath := os.Getenv("M31A_CONFIG"); envPath != "" {
    configPath = envPath
}

// Ensure config directory exists
if err := os.MkdirAll(filepath.Dir(configPath), 0755); err != nil {
    logger.Error("failed to create config directory", "error", err)
    os.Exit(1)
}

// Load config (returns DefaultConfig if file missing — not an error)
cfg, err := config.Load(configPath)
if err != nil {
    logger.Error("failed to load config", "error", err)
    os.Exit(1)
}

// Initialize keychain (may fail gracefully — keychain is optional)
kc, kcErr := keychain.New()
if kcErr != nil {
    logger.Warn("keychain unavailable", "error", kcErr)
}

// Resolve API keys: env var → keychain → config file
if kc != nil {
    cfg.ResolveAPIKeys(kc)
}

// Create provider registry
registry := provider.NewRegistry()

var resolvedAPIKey string

// Register OpenRouter if key available
if apiKey := cfg.Provider.OpenRouter.APIKey; apiKey != "" {
    orClient, err := or.New(apiKey)
    if err != nil {
        logger.Warn("failed to create OpenRouter client", "error", err)
    } else {
        registry.Register("openrouter", orClient)
        if cfg.Provider.Default == "openrouter" {
            resolvedAPIKey = apiKey
        }
    }
}

// Register Zen if key available
if apiKey := cfg.Provider.Zen.APIKey; apiKey != "" {
    zenClient, err := zen.New(apiKey)
    if err != nil {
        logger.Warn("failed to create Zen client", "error", err)
    } else {
        registry.Register("zen", zenClient)
        if cfg.Provider.Default == "zen" {
            resolvedAPIKey = apiKey
        }
    }
}

// Set active provider from config, or default to first registered
if cfg.Provider.Default != "" {
    registry.SetActive(cfg.Provider.Default)
}

// Fallback: use first registered provider's key if no default set
if resolvedAPIKey == "" {
    if cfg.Provider.OpenRouter.APIKey != "" {
        resolvedAPIKey = cfg.Provider.OpenRouter.APIKey
    } else if cfg.Provider.Zen.APIKey != "" {
        resolvedAPIKey = cfg.Provider.Zen.APIKey
    }
}

// Create and start TUI
app := tui.NewApp(Version, registry, resolvedAPIKey, configPath)
p := tea.NewProgram(app, tea.WithAltScreen())
if _, err := p.Run(); err != nil {
    logger.Error("TUI exited with error", "error", err)
    os.Exit(1)
}
```

### Step 3: Add `filepath` to imports

The existing imports include `os` and `runtime`. Add `"path/filepath"` to the stdlib imports.

### Verification

After this change:
- `CGO_ENABLED=0 go build -o m31a ./cmd/m31a` must succeed
- Running `./m31a` with a valid API key in env var or config file must launch the Bubble Tea TUI
- Running `./m31a` with no API key must show the first-run setup screen
- Running `./m31a` with no config file must show the first-run setup screen
- The structured logger must still initialize and log the startup line
- `go vet ./...` must be clean

### Do NOT:
- Remove the `log.NewLogger()` initialization or `defer cleanup()`
- Remove the `logger.Info("M31A starting", ...)` line
- Add interactive prompts before the TUI launches (the first-run screen handles that)
- Create the config file automatically (first-run flow handles that)
- Exit with error if keychain is unavailable (it's optional — env vars and config file are valid fallbacks)

---

## Fix 2: Fix Permission Listener Goroutine Race Condition (HIGH)

**File:** `internal/tui/app.go` — lines 120-125

**Problem:** A background goroutine directly calls `app.Update(PermissionRequestMsg{...})` outside the Bubble Tea event loop:

```go
// Permission listener goroutine
go func() {
    for req := range app.dispatcher.RequestCh() {
        app.Update(PermissionRequestMsg{Request: req})
    }
}()
```

This violates the core Bubble Tea constraint: ALL state mutations must go through `Update()` called by the Bubble Tea runtime. The goroutine concurrently mutates `AppState.screen`, `AppState.prevScreen`, and `AppState.permissionModal` while the runtime may simultaneously call `Update()` from the main loop. This is a race condition — concurrent map writes or state corruption. The `-race` flag does not catch this in tests because the permission channel is never exercised in the test suite.

**Spec reference (AGENTS.md):** "Bubble Tea is single-threaded. ALL state mutations go through Update() only. Never mutate AppState from a goroutine. Use tea.Cmd and tea.Msg."

**What to do:** Remove the goroutine. Replace with a `tea.Cmd` that blocks on the channel and returns messages into the Bubble Tea event loop.

### Step 1: Remove the goroutine

Delete lines 120-125 entirely from `NewApp()`:
```go
// Permission listener goroutine
go func() {
    for req := range app.dispatcher.RequestCh() {
        app.Update(PermissionRequestMsg{Request: req})
    }
}()
```

### Step 2: Add the listener cmd function

Add this function near the top of `app.go` (after `NewApp()` and before `Init()`):

```go
// permissionListenerCmd returns a tea.Cmd that watches the dispatcher's
// permission request channel and feeds requests into the Bubble Tea event loop.
// Each invocation returns exactly one PermissionRequestMsg.
func permissionListenerCmd(dispatcher *tools.Dispatcher) tea.Cmd {
    return func() tea.Msg {
        req := <-dispatcher.RequestCh()
        return PermissionRequestMsg{Request: req}
    }
}
```

### Step 3: Update `Init()` to return the listener cmd

Current `Init()`:
```go
func (m *AppState) Init() tea.Cmd {
    if m.screen == ScreenREPL && m.registry != nil && m.activeProvider != "" {
        return HealthCheckTicker(context.Background(), m.registry, m.activeProvider, types.HealthCheckInterval)
    }
    return nil
}
```

Replace with:
```go
func (m *AppState) Init() tea.Cmd {
    cmds := []tea.Cmd{permissionListenerCmd(m.dispatcher)}
    if m.screen == ScreenREPL && m.registry != nil && m.activeProvider != "" {
        cmds = append(cmds, HealthCheckTicker(context.Background(), m.registry, m.activeProvider, types.HealthCheckInterval))
    }
    return tea.Batch(cmds...)
}
```

### Step 4: Re-queue the listener in every Update() return path

Since each `tea.Cmd` invocation consumes one message from the channel, the listener must be re-returned after every `Update()` cycle to keep watching. Add `permissionListenerCmd(m.dispatcher)` to **every** `tea.Batch()` return in `Update()`.

Update these return paths in `Update()`:

**4a. tea.WindowSizeMsg handler (line ~149):**
```go
if m.replModel != nil {
    m.replModel.Update(msg)
}
if m.firstRunModel != nil {
    m.firstRunModel.Update(msg)
}
return m, tea.Batch(permissionListenerCmd(m.dispatcher))
```

**4b. Permission modal key handler (line ~166):**
```go
return m, tea.Batch(func() tea.Msg {
    return PermissionResponseMsg{Response: resp}
}, permissionListenerCmd(m.dispatcher))
```

**4c. HealthCheckTickMsg handler (line ~203):**
```go
return m, tea.Batch(NextHealthTick(calculateNextInterval(result)), permissionListenerCmd(m.dispatcher))
```

**4d. AppMsg handler (line ~223):**
```go
return m, tea.Batch(permissionListenerCmd(m.dispatcher))
```

**4e. ErrorMsg handler (line ~227):**
```go
return m, tea.Batch(permissionListenerCmd(m.dispatcher))
```

**4f. PermissionRequestMsg handler (line ~235):**
```go
return m, tea.Batch(permissionListenerCmd(m.dispatcher))
```

**4g. PermissionResponseMsg handler (line ~241):**
```go
m.dispatcher.ApprovePermission(msg.Response.Allowed, msg.Response.Remember)
m.screen = m.prevScreen
m.permissionModal = nil
return m, tea.Batch(permissionListenerCmd(m.dispatcher))
```

**4h. ScreenFirstRun Update (line ~268):**
```go
return m, tea.Batch(append(cmds, permissionListenerCmd(m.dispatcher))...)
```

**4i. ScreenREPL Update (line ~279):**
```go
return m, tea.Batch(append(cmds, permissionListenerCmd(m.dispatcher))...)
```

**4j. ScreenSettings Update (line ~289):**
```go
return m, tea.Batch(append(cmds, permissionListenerCmd(m.dispatcher))...)
```

**4k. ScreenResume Update (line ~303):**
```go
return m, tea.Batch(append(cmds, permissionListenerCmd(m.dispatcher))...)
```

**4l. Default case at end of Update() (line ~306):**
```go
return m, tea.Batch(permissionListenerCmd(m.dispatcher))
```

**4m. Early returns that currently return `nil` for cmds:**

The `ctrl+c` handler (line ~171) and the `ScreenPermission` default case (line ~164) currently return `m, nil`. These should also re-queue the listener:

```go
// Line ~164 (default case in permission modal handler):
return m, tea.Batch(permissionListenerCmd(m.dispatcher))

// Line ~171 (ctrl+c):
return m, tea.Batch(tea.Quit, permissionListenerCmd(m.dispatcher))
// Note: tea.Quit will terminate the program, so the listener is irrelevant here,
// but for consistency we include it. Alternatively, just return m, tea.Quit.
```

For the `/settings` and `/resume` handlers that return `m, nil` (lines ~177, ~183), also re-queue:
```go
m.screen = ScreenSettings
return m, tea.Batch(permissionListenerCmd(m.dispatcher))
```

```go
m.screen = ScreenResume
return m, tea.Batch(permissionListenerCmd(m.dispatcher))
```

### Why this pattern works

`permissionListenerCmd` returns a `tea.Cmd` — a `func() tea.Msg`. When Bubble Tea batches this cmd, it calls the function in a goroutine managed by the runtime. The function blocks on `<-dispatcher.RequestCh()`. When a permission request arrives, it returns `PermissionRequestMsg`. The Bubble Tea runtime feeds this message back into `Update()` on the next tick. The message handler creates the permission modal, and returns the listener cmd again to keep watching. This is the standard pattern for channel-based async events in Bubble Tea.

### Verification

- `go test -race ./internal/tui/...` must pass with no race warnings
- Permission requests from the dispatcher must correctly show the permission modal
- After permission granted/denied, the listener must continue watching for future requests
- No goroutine leaks — each cmd invocation spawns one goroutine that blocks on the channel, and the channel is never closed during normal operation

---

## Fix 3: Check SetWidth() Error on Glamour Renderer (HIGH)

**File:** `internal/tui/repl.go` — line 94

**Problem:** On `tea.WindowSizeMsg`, the code calls `m.msgRenderer.SetWidth(msg.Width - 4)` but ignores the returned error:

```go
if m.msgRenderer != nil {
    m.msgRenderer.SetWidth(msg.Width - 4)
}
```

The `SetWidth()` method (in `internal/tui/components/message.go`) closes the old renderer and creates a new one. If the new glamour renderer fails to initialize (e.g., due to a very small width), `SetWidth()` returns an error but `m.msgRenderer` remains pointing at the old (now closed) renderer. Subsequent `Render()` calls on a closed renderer will fail silently or panic.

**Spec reference (audit finding):** "repl.go: SetWidth() called on WindowSizeMsg for Glamour renderer"

**What to do:** Check the error and fall back gracefully.

### Step 1: Fix the resize handler

In `repl.go`, update the `WindowSizeMsg` handler (around line 93-95):

```go
if m.msgRenderer != nil {
    if err := m.msgRenderer.SetWidth(msg.Width - 4); err != nil {
        // Renderer recreation failed — mark lastStatus so user sees the issue
        // but rendering continues with stale width rather than crashing
        m.lastStatus = fmt.Sprintf("renderer resize error: %v", err)
    }
}
```

The `lastStatus` field exists on `ReplModel` (line 31 of repl.go) and is displayed in the status bar. This is a non-fallback error — rendering continues with the old width setting, which is better than a nil or closed renderer.

### Step 2: Fix SetTheme() error handling

In `repl.go`, the `SetTheme()` method (line 323-328) also recreates the renderer:

```go
func (m *ReplModel) SetTheme(t theme.Theme) {
    m.theme = t
    if m.msgRenderer != nil {
        m.msgRenderer, _ = components.NewMessageRenderer(t, m.width-4)
    }
}
```

The blank identifier discards the error. If renderer creation fails, `m.msgRenderer` becomes `nil`, causing nil pointer dereference on subsequent renders. Fix:

```go
func (m *ReplModel) SetTheme(t theme.Theme) {
    m.theme = t
    if m.msgRenderer != nil {
        newRenderer, err := components.NewMessageRenderer(t, m.width-4)
        if err == nil {
            m.msgRenderer = newRenderer
        }
        // If error, keep old renderer — better stale rendering than nil
    }
}
```

### Verification

- Resize terminal to very small width (< 10 chars) — app should not crash
- Theme cycle via `/theme` — should not leave nil renderer
- `go vet ./...` clean
- Existing tests pass

---

## Fix 4: Add DurationMs to Grep Tool (MEDIUM)

**File:** `internal/tools/grep.go`

**Problem:** The Grep tool's `Execute()` delegates to `grepWithRG()` or `grepPureGo()`, neither of which track execution time. Both sub-functions return `types.ToolResult` without setting `DurationMs`. Results always have `DurationMs: 0`. This is inconsistent with Bash, FileRead, FileWrite, and Glob tools which all report execution duration.

**Spec reference:** "All tools return types.ToolResult with DurationMs, Truncated, Output, Error fields"

**What to do:** Add timing at the `Execute()` wrapper level. Do NOT modify `grepWithRG()` or `grepPureGo()` — the wrapper adds `DurationMs` after the sub-function returns.

### Step 1: Add time import

Add `"time"` to the imports at the top of `grep.go`.

### Step 2: Add timing to Execute()

Current `Execute()` structure (lines 43-86):
```go
func (t *Grep) Execute(ctx context.Context, input types.ToolInput) (types.ToolResult, error) {
    // ... parameter parsing ...
    // ... searchPath resolution ...
    // ... globFilter and maxResults parsing ...

    if t.hasRg {
        return t.grepWithRG(pattern, searchPath, globFilter, maxResults)
    }
    return t.grepPureGo(pattern, searchPath, globFilter, maxResults)
}
```

Replace the final return block (lines 82-86) with:

```go
func (t *Grep) Execute(ctx context.Context, input types.ToolInput) (types.ToolResult, error) {
    start := time.Now()

    // ... existing parameter parsing code (lines 44-80) unchanged ...

    var result types.ToolResult
    var err error
    if t.hasRg {
        result, err = t.grepWithRG(pattern, searchPath, globFilter, maxResults)
    } else {
        result, err = t.grepPureGo(pattern, searchPath, globFilter, maxResults)
    }

    result.DurationMs = time.Since(start).Milliseconds()
    return result, err
}
```

This approach:
- Does not modify `grepWithRG()` or `grepPureGo()` (they return `ToolResult` as-is)
- Adds `DurationMs` to ALL return paths (both success and error)
- Uses the same pattern as FileWrite (`start := time.Now()` + `time.Since(start).Milliseconds()`)

### Verification

- Grep tool tests still pass
- `DurationMs` is non-zero in test assertions
- No changes to `grepWithRG()` or `grepPureGo()` return values beyond `DurationMs` field

---

## Fix 5: Add DurationMs to Glob Tool (MEDIUM)

**File:** `internal/tools/glob.go`

**Problem:** The Glob tool's `Execute()` returns `types.ToolResult{Output: b.String()}` without populating `DurationMs`. Always 0. Inconsistent with other tools.

**Spec reference:** Same as Grep above.

**What to do:** Add timing to `Execute()`.

### Step 1: Add time import

Add `"time"` to the imports at the top of `glob.go`.

### Step 2: Add timing to Execute()

Current `Execute()` (lines 36-97):
```go
func (t *Glob) Execute(ctx context.Context, input types.ToolInput) (types.ToolResult, error) {
    // ... existing code ...
    // (no start time tracking)

    // ... existing code ...

    return types.ToolResult{Output: b.String()}, nil
}
```

Add `start := time.Now()` at the top of the function (after the opening brace, before parameter parsing). Update the final return statement (line 96):

```go
func (t *Glob) Execute(ctx context.Context, input types.ToolInput) (types.ToolResult, error) {
    start := time.Now()

    // ... all existing code unchanged through line 94 ...

    return types.ToolResult{
        Output:     b.String(),
        DurationMs: time.Since(start).Milliseconds(),
        Truncated:  truncated,
    }, nil
}
```

Note: `truncated` is already declared at line 70. The return now properly sets both `DurationMs` and `Truncated`.

### Verification

- Glob tool tests still pass
- `DurationMs` is non-zero in test assertions
- `Truncated` field is now properly populated (was missing before)

---

## Complete Verification Sequence

After all 5 fixes are applied, run:

```bash
go mod tidy
CGO_ENABLED=0 go build -o m31a ./cmd/m31a
go vet ./...
go test -race -cover ./...
```

Expected results:
- Binary builds successfully as static CGO-free binary
- `./m31a` launches the Bubble Tea TUI (not a placeholder message)
- `go vet` reports no issues
- All 352+ tests pass with race detector enabled
- No race condition warnings from the permission listener
- Grep and Glob tools return non-zero `DurationMs`
- Glamour renderer resize errors are handled gracefully
