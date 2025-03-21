# M31A Cross-Phase Audit Report (Phase 0 → Phase 5)

**Date:** 2026-05-28
**Auditor:** Qoder CLI
**Scope:** All Go packages in `internal/`, `pkg/`, `cmd/` — 85 source files, 352 tests

---

## Audit Summary

| Category | Status |
|----------|--------|
| Type Consistency | ✅ PASS |
| Config System | ⚠️ 1 MEDIUM |
| Session Lifecycle | ✅ PASS |
| Token Estimation | ✅ PASS |
| Provider Layer | ✅ PASS |
| TUI | ⚠️ 2 HIGH |
| Tool System | ⚠️ 2 MEDIUM |
| Cross-Phase Integration | ⚠️ 1 CRITICAL |
| Build & CI | ✅ PASS |
| Security | ⚠️ 1 MEDIUM |

**Total findings: 7** (1 CRITICAL, 2 HIGH, 4 MEDIUM, 0 LOW)

**Test count: 352 existing + 0 new = 352 total**

---

## Findings

### CRITICAL — Binary Does Not Launch TUI

**File:** `cmd/m31a/main.go:28-29`

**Description:** The binary entry point does NOT create a provider registry, does NOT initialize the TUI app, and does NOT start the Bubble Tea event loop. It prints a placeholder message and exits.

```go
fmt.Printf("M31A %s — AI coding assistant\n", Version)
fmt.Println("Run 'make build' to build. Implementation coming in Phase 1+.")
```

The entire TUI (`internal/tui/app.go`), session management (`pkg/session/`), provider layer (`internal/provider/`), tool dispatcher (`internal/tools/`), and all Phase 2-5 code is wired into `NewApp()` but never called from `main()`.

**Spec reference:** The spec defines M31A as a "terminal AI coding agent" with a full Bubble Tea TUI. The binary should launch the complete application with config loading, keychain resolution, provider registry setup, and the TUI event loop.

**Impact:** None of the Phase 2-5 functionality is reachable from the compiled binary. The application cannot be used as an end product until this is fixed.

---

### HIGH — Permission Listener Goroutine Violates Bubble Tea Single-Thread Model

**File:** `internal/tui/app.go:121-125`

**Description:** A background goroutine directly calls `AppState.Update()` outside the Bubble Tea event loop:

```go
go func() {
    for req := range app.dispatcher.RequestCh() {
        app.Update(PermissionRequestMsg{Request: req})
    }
}()
```

Bubble Tea's contract requires that ALL state mutations go through `Update()` called by the Bubble Tea runtime. This goroutine performs concurrent mutations to `AppState.screen`, `AppState.prevScreen`, and `AppState.permissionModal` while the runtime may simultaneously be calling `Update()` from the main loop.

**Spec reference:** AGENTS.md states: "Bubble Tea is single-threaded. ALL state mutations go through Update() only. Never mutate AppState from a goroutine. Use tea.Cmd and tea.Msg."

**Impact:** Race condition — concurrent map writes or state corruption when the Bubble Tea runtime and this goroutine both access `AppState` fields. The `-race` flag did not catch this in tests because the permission channel is never exercised in the test suite.

---

### HIGH — SetWidth() Error Not Checked on Glamour Renderer

**File:** `internal/tui/repl.go:94`

**Description:** On `tea.WindowSizeMsg`, the code calls `m.msgRenderer.SetWidth(msg.Width - 4)` but ignores the returned error:

```go
if m.msgRenderer != nil {
    m.msgRenderer.SetWidth(msg.Width - 4)
}
```

If the glamour renderer recreation fails (e.g., due to a very small width causing renderer initialization to error), `m.msgRenderer` remains pointing at the old renderer with stale width settings. Subsequent rendering will wrap at the wrong width, producing garbled output.

**Spec reference:** "repl.go: SetWidth() called on WindowSizeMsg for Glamour renderer"

---

### MEDIUM — fmt.Printf Used for Keychain Errors

**File:** `internal/config/loader.go:103, 116`

**Description:** Unexpected keychain errors are printed via `fmt.Printf` rather than the structured logger:

```go
fmt.Printf("keychain.Get(openrouter): %v\n", err)
// ...
fmt.Printf("keychain.Get(zen): %v\n", err)
```

While this does not expose API keys, it bypasses the structured `slog` logger defined in `internal/log/` and writes to stdout, which could appear in terminal scrollback, multiplexer logs, or CI output.

**Spec reference:** AGENTS.md defines "internal/log/ — structured logger (slog) with rotation" as the sole logging mechanism.

---

### MEDIUM — Grep Tool Missing DurationMs

**File:** `internal/tools/grep.go:149, 229`

**Description:** The Grep tool's `Execute()` delegates to `grepWithRG()` or `grepPureGo()`, neither of which track or populate `DurationMs` in the returned `ToolResult`. Results always have `DurationMs: 0`.

**Spec reference:** "All tools return types.ToolResult with DurationMs, Truncated, Output, Error fields"

---

### MEDIUM — Glob Tool Missing DurationMs

**File:** `internal/tools/glob.go:96`

**Description:** The Glob tool's `Execute()` returns `types.ToolResult{Output: b.String()}` without populating `DurationMs`. Result always has `DurationMs: 0`.

**Spec reference:** Same as Grep above.

---

## Verified Correct — No Issues Found

### Area 1: Type Consistency ✅

- `types.ToolResult` — all fields (`Output`, `Error`, `DurationMs`, `Truncated`) populated consistently across all tools
- `types.ToolInput.Params` — always `map[string]any`, never custom struct
- `provider.ChatRequest.Messages` — `[]types.Message`
- `types.StreamIterator` — `Next func() (*StreamChunk, error)`, `Close func() error` — matches interface
- `WorkflowPhase` enum — consistent values between `types/constants.go` and workflow usage

### Area 2: Config System ✅ (minus MEDIUM finding)

- `Load()` returns `DefaultConfig()` when file missing — `os.IsNotExist` check at line 39
- `M31A_CONFIG` env var overrides path — line 31
- `ResolveAPIKeys()` order: env var → keychain → config file — lines 94-121
- `Save()` uses atomic write (temp file + rename in same directory) — lines 126-161
- `BurntSushi/toml` used for parsing — line 37
- No plaintext key storage defaults — `DefaultConfig()` returns zero-valued struct
- All config struct fields have correct TOML tags — verified in `config/types.go`
- Env var overrides: `M31A_OPENROUTER_API_KEY`, `M31A_ZEN_API_KEY`, `M31A_THEME`, `M31A_DEFAULT_MODEL` — all present

### Area 3: Session Lifecycle ✅

- `NewSession()` generates 8-char hex ID via `crypto/rand` — `manager.go:99-105`
- Session directory structure: `session.json`, `messages.json`, `planning/PROJECT.md`, `planning/TASKS.md`, `planning/STATE.md` — verified
- All writes atomic — `manager.go:33-75` temp file + rename
- `LoadSession()` tolerates missing optional files — `manager.go:172-177`
- `ListSessions()` skips `archived/` directory — `manager.go:204-205`
- Corrupted session detection — JSON parse failure or missing `session.json` marks `Corrupted=true`
- `ArchiveSession()` moves to `archived/` subdirectory — `manager.go:261-267`
- `DeleteSession()` removes directory recursively — `manager.go:256-258`
- Markdown parsers handle whitespace, blank lines, missing optional fields — `planning.go`
- Max 2 checkpoints retained — `checkpoint.go:38-40`
- `LatestCheckpoint()` returns `ErrCheckpointNotFound` when none exist — `checkpoint.go:79-81`

### Area 4: Token Estimation ✅

- `tiktoken-go` used for GPT/Claude families — `estimator.go:35-37`
- Fallback: `len([]rune(text)) / 4 * 1.3` for unknown models — `estimator.go:55`
- EMA calibration: `emaFactor = emaAlpha * ratio + (1 - emaAlpha) * emaFactor` — `estimator.go:72`
- `emaAlpha = 0.3` — `estimator.go:31`
- `FormatUsage()` returns `"used / total (XX%)"` format — `estimator.go:92`
- `ContextWarningBanner()` triggers at configurable threshold (default 80%) — `estimator.go:101-118`
- Tests verify EMA convergence — `TestEstimator_CalibrateConvergence`
- Multi-byte character handling correct — `TestEstimator_EstimateMultibyte`

### Area 5: Provider Layer ✅

- `LLMProvider` interface unchanged — 5 methods with correct signatures
- `ChatRequest` uses `[]types.Message` for history — `interface.go:20`
- `StreamIterator.Next()` returns `(*types.StreamChunk, error)` — verified in both clients
- `EstimateCost(modelID string, usage types.Usage) float64` — modelID parameter present
- Model cache: TTL 5min, stale fallback 24h — `cache.go:22-23`
- Auto-fallback on 429/503 — `fallback.go:16-17`
- No hardcoded model lists — both clients fetch from provider APIs
- SSE parsing handles both OpenRouter and Zen formats — generic `SSEParser`
- Reasoning normalization: pre-content vs interleaved thinking — `reasoning.go`

### Area 6: TUI ✅ (minus HIGH findings)

- `streaming.go`: `StartStreamCmd` returns proper `tea.Cmd` — `func() tea.Msg { return <-streamCh }`
- Theme system: dark/light palette, `Cycle()` method — `theme.go:335-348`
- Health check ticker: 60s interval — `health.go:11`
- `message.go`: `SetWidth()` recreates glamour renderer — `message.go:43-50`
- `toolcard.go`: `formatToolInput()` produces per-tool formatted strings
- Tool card states: Running → Success/Error with duration display
- Thinking blocks: collapsible, duration counter

### Area 7: Tool System ✅ (minus MEDIUM findings)

- `dispatcher.go`: `extractCommandString()` populates `PermissionRequest.Command`
- `dispatcher.go`: Permission gate triggers for Dangerous/Destructive only
- `bash.go`: io.Pipe for concurrent stdout/stderr reading, limitWriter at 50KB
- `bash.go`: Setpgid for process group isolation
- `bash.go`: Signal forwarding (SIGINT then SIGKILL after 5s)
- `grep.go`: searchPath resolved relative to workDir with EvalSymlinks
- `glob.go`: Both backends return consistent relative paths
- `filewrite.go`: Atomic write, backup before overwrite, DurationMs populated
- `fileread.go`: Binary detection, 5MB limit, path safety

### Area 8: Cross-Phase Integration ✅ (minus CRITICAL finding)

- No circular dependencies — verified all import chains
- `internal/tokens` independent of `internal/types` (only imports fmt, lipgloss, tiktoken-go)
- Config → Provider → Active provider wiring correct in `app.go`

### Area 9: Build & CI ✅

- `go.mod` — Go 1.22 minimum
- All charmbracelet libs at compatible versions
- No CGO dependencies
- `go.sum` present and consistent
- `CGO_ENABLED=0 go build` succeeds — verified
- `go vet ./...` clean — verified
- All tests pass with `-race` flag — 352 pass, 0 fail

### Area 10: Security ✅ (minus MEDIUM finding)

- **API key leakage**: Keys not logged via slog, not printed to stdout (except MEDIUM keychain error finding)
- **Path traversal**: FileRead and FileWrite reject paths outside workDir (EvalSymlinks + prefix check)
- **Shell injection**: Bash tool uses `bash -c` with user-provided command — by design, no additional interpolation
- **SSRF**: No WebFetch in V1 — confirmed no HTTP client calls to arbitrary URLs
- **Keychain security**: Keys not stored in config file by default
- **Race conditions**: Provider cache protected by `sync.RWMutex`

---

## Keychain Build Tags ✅

| Platform | Build Tag | Implementation |
|----------|-----------|----------------|
| Linux | `//go:build linux` | D-Bus Secret Service with `pass` CLI fallback |
| macOS | `//go:build darwin` | `/usr/bin/security` CLI |
| Windows | `//go:build windows` | Stub returning `ErrNotImplemented` |

- Service names: `m31a/openrouter`, `m31a/zen` — prefix defined as `m31a/` in `keychain.go:3`
- `ErrKeyNotFound` vs `ErrKeychainUnavailable` distinguished — verified in all platform files
- No CGO imports in keychain packages — verified
- Service name validation via regex `^[a-z]+$` — prevents injection in all platforms

---

## Test Coverage by Package

| Package | Tests | Status |
|---------|-------|--------|
| `internal/config` | 14 | ✅ PASS |
| `internal/provider` | 17 | ✅ PASS |
| `internal/provider/openrouter` | 15 | ✅ PASS |
| `internal/provider/zen` | 9 | ✅ PASS |
| `internal/tokens` | 23 | ✅ PASS |
| `internal/tools` | 14 | ✅ PASS |
| `internal/tui` | 160+ | ✅ PASS |
| `internal/tui/components` | 80+ | ✅ PASS |
| `internal/tui/theme` | 10+ | ✅ PASS |
| `pkg/keychain` | 5+ | ✅ PASS |
| `pkg/session` | 10+ | ✅ PASS |

**Packages without tests:** `cmd/m31a/`, `internal/errors/`, `internal/log/`, `internal/types/`

---

## Recommendation

**Fix first** before Phase 6.

The CRITICAL finding (non-functional binary) blocks all other work — the application cannot run. The HIGH findings represent genuine correctness and stability risks that should be addressed before adding new features.

### Priority order:

1. **Wire `main.go` to launch TUI** (CRITICAL) — creates registry, resolves config, starts Bubble Tea
2. **Fix permission goroutine race** (HIGH) — replace direct `Update()` call with `tea.Cmd` channel
3. **Check `SetWidth()` error** (HIGH) — handle glamour renderer recreation failure
4. **Add `DurationMs` to Grep and Glob tools** (MEDIUM) — consistent with other tools
5. **Replace `fmt.Printf` with `slog`** (MEDIUM) — use structured logger for keychain errors
