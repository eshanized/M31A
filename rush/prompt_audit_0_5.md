# Audit — Phase 0 through Phase 5

You are a senior Go developer and security auditor performing a cross-phase audit of the M31A terminal AI coding assistant. Phases 0-5 have been implemented. You must verify consistency, correctness, and spec compliance across ALL phases.

## Scope

Audit every package that exists in the codebase. Read every file. Check for:
1. **Type consistency** — types used identically across package boundaries
2. **Interface compliance** — all interfaces have correct method signatures per spec
3. **Error handling** — no raw `fmt.Errorf` where typed errors should be used, no swallowed errors
4. **Atomic writes** — all state file writes use temp file + rename
5. **Thread safety** — all shared state protected by mutex, no goroutine mutation of Bubble Tea state
6. **Build correctness** — build tags correct, CGO-free, go.mod consistent
7. **Cross-phase integration** — packages wired together correctly
8. **Spec compliance** — behavior matches `adrenaline/idea.md` and `adrenaline/REFERENCE.md`
9. **Test quality** — tests actually assert behavior, not just coverage numbers
10. **Security** — no key leakage in logs, no path traversal, no shell injection

## Audit Areas

### Area 1: Type Consistency (CRITICAL)

Read ALL files in these packages and verify types are used identically everywhere:
- `internal/types/` — source of truth for Message, Session, Task, ToolCall, ToolResult, ToolInput, Usage, etc.
- `pkg/session/` — does it embed `types.Session` correctly? Does it use `types.Message`, `types.Task`, `types.WorkflowPhase`?
- `internal/config/types.go` — are `types.RiskLevel` references correct?
- `internal/tools/` — do all tools return `types.ToolResult` with all fields populated (Output, Error, DurationMs, Truncated)?
- `internal/tui/` — does it use `types.Message`, `types.StreamChunk`, `types.MessageSegment` consistently?
- `internal/provider/` — does `ChatRequest` use `types.Message` for history? Does `StreamIterator` match `types.StreamIterator`?

Check specifically:
- `ToolResult.DurationMs` — present in all tool outputs (FileWrite was fixed in pre-5 cycle)
- `Message.Segments` — used for streaming rendering in TUI
- `types.ToolInput.Params` — always `map[string]any`, never custom struct
- `WorkflowPhase` enum values match between types and workflow screens

### Area 2: Config System (CRITICAL)

Verify `internal/config/`:
- `Load()` returns `DefaultConfig()` when file missing (not an error)
- `M31A_CONFIG` env var overrides path
- `ResolveAPIKeys()` checks env → keychain → config file, in that exact order
- `Save()` uses atomic write (temp file + rename in same directory)
- `BurntSushi/toml` used for parsing
- No plaintext key storage defaults
- All config struct fields have correct TOML tags
- Env var overrides: `M31A_OPENROUTER_API_KEY`, `M31A_ZEN_API_KEY`, `M31A_THEME`, `M31A_DEFAULT_MODEL`, `M31A_PROVIDER`

Verify `pkg/keychain/`:
- Build tags correct: `//go:build linux`, `//go:build darwin`, `//go:build windows`
- Linux: dbus Secret Service with `pass` CLI fallback
- macOS: go-keyring (no CGO)
- Windows: stub returning `ErrNotImplemented`
- Service names: `m31a/openrouter`, `m31a/zen`
- `ErrKeyNotFound` vs `ErrKeychainUnavailable` distinguished
- No CGO imports anywhere in keychain packages

### Area 3: Session Lifecycle (CRITICAL)

Verify `pkg/session/`:
- `NewSession()` generates 8-char hex ID via `crypto/rand`
- Session directory structure matches spec: `session.json`, `messages.json`, `planning/PROJECT.md`, `planning/TASKS.md`, `planning/STATE.md`
- All writes atomic (temp file + rename)
- `LoadSession()` tolerates missing optional files (messages.json, planning files)
- `ListSessions()` skips `archived/` directory
- Corrupted session detection: JSON parse failure or missing session.json
- `ArchiveSession()` moves to `archived/` subdirectory
- `DeleteSession()` removes directory recursively

Verify markdown parsing:
- `PROJECT.md` parser handles: `**Goal:**`, `**Type:**`, `**Framework:**`, `## Questions` section
- `TASKS.md` parser handles markdown table rows
- `STATE.md` parser handles: `**Phase:**`, `**Progress:**`, `**Last Action:**`, `**Timestamp:**`
- All parsers tolerate extra whitespace, blank lines, missing optional fields

Verify checkpoint system:
- Max 2 checkpoints retained
- `checkpoint.json` format correct
- `LatestCheckpoint()` returns nil when none exist

### Area 4: Token Estimation (HIGH)

Verify `internal/tokens/`:
- `tiktoken-go` used for GPT/Claude families
- Fallback: `len([]rune(text)) / 4 * 1.3` for unknown models
- EMA calibration: `emaFactor = emaAlpha * (actual / estimated) + (1 - emaAlpha) * emaFactor`
- `emaAlpha = 0.3`
- `FormatUsage()` returns `"used / total (XX%)"` format
- `ContextWarningBanner()` triggers at configurable threshold (default 80%)
- Tests verify EMA convergence to < 5% error after 3+ calibrations
- Multi-byte character handling correct (uses `[]rune`, not `len(string)`)

### Area 5: Provider Layer Regression (HIGH)

Re-verify `internal/provider/` for regressions introduced by Phase 5:
- `LLMProvider` interface unchanged (no new methods, no signature changes)
- `ChatRequest` uses `[]types.Message` for history
- `StreamIterator.Next()` returns `(*types.StreamChunk, error)`
- `EstimateCost(modelID string, usage types.Usage) float64` — modelID parameter present
- Model cache: TTL 5min, stale fallback 24h
- Auto-fallback on 429/503
- No hardcoded model lists
- SSE parsing handles both OpenRouter and Zen formats
- Reasoning normalization: pre-content vs interleaved thinking

### Area 6: TUI Regression (HIGH)

Re-verify `internal/tui/` for regressions:
- `repl.go`: `SetWidth()` called on `WindowSizeMsg` for Glamour renderer
- `streaming.go`: `StartStreamCmd` returns proper `tea.Cmd` (func returning single message)
- `repl.go`: single-threaded Update() — no goroutine state mutations
- All screen models implement proper `Update(msg tea.Msg) ([]tea.Cmd, bool)` signature
- Theme system: dark/light palette, `Cycle()` method
- Health check ticker: 60s interval, non-blocking during streaming

Verify `internal/tui/components/`:
- `message.go`: `SetWidth()` recreates glamour renderer correctly
- `toolcard.go`: `formatToolInput()` produces per-tool formatted strings
- Tool card states: Running → Success/Error with duration display
- Thinking blocks: collapsible, duration counter

### Area 7: Tool System Regression (HIGH)

Re-verify `internal/tools/`:
- `dispatcher.go`: `extractCommandString()` populates `PermissionRequest.Command`
- `dispatcher.go`: Permission gate only triggers for Dangerous/Destructive risk levels
- `bash.go`: io.Pipe for concurrent stdout/stderr reading, limitWriter at 50KB per stream
- `bash.go`: Setpgid for process group isolation
- `bash.go`: Signal forwarding (SIGINT then SIGKILL after 5s)
- `grep.go`: searchPath resolved relative to workDir with EvalSymlinks
- `glob.go`: Both backends return consistent relative paths
- `filewrite.go`: Atomic write (temp + rename), backup before overwrite, DurationMs populated
- `fileread.go`: Binary detection, 5MB limit, path safety
- All tools return `types.ToolResult` with DurationMs, Truncated, Output, Error fields

### Area 8: Cross-Phase Integration (HIGH)

Verify packages are wired together correctly:
- Config loaded → passed to provider factory → active provider set
- Session created → messages appended → saved after each turn
- Token estimator → used for context warning in REPL header
- Settings screen → saves to config file → loaded on next startup
- Resume screen → lists sessions → loads selected → transitions to REPL
- Checkpoint → saved before phase transitions → used by `/undo`
- Keychain → used by config loader → API keys resolved → passed to provider

Check for circular dependencies:
- `internal/config/` depends on `pkg/keychain/` — ok (internal can depend on pkg)
- `pkg/session/` depends on `internal/types/` — ok
- `internal/tokens/` depends on `internal/types/` — should NOT (tokens should be independent)
- `internal/tui/` depends on `internal/config/`, `pkg/session/`, `internal/tokens/` — ok

### Area 9: Build & CI (MEDIUM)

Verify:
- `go.mod` — Go 1.22 minimum (not 1.24)
- All charmbracelet libs at compatible versions
- No CGO dependencies
- `go.sum` present and consistent
- Build tags don't exclude needed files on Linux (our primary dev platform)
- `CGO_ENABLED=0 go build` succeeds
- `go vet ./...` clean
- All tests pass with `-race` flag

### Area 10: Security Audit (MEDIUM)

Check for:
- **API key leakage**: Keys not logged via slog, not printed to stdout, not in error messages
- **Path traversal**: FileRead and FileWrite reject paths outside workDir (EvalSymlinks + prefix check)
- **Shell injection**: Bash tool uses `bash -c` with user-provided command — this is by design (LLM provides commands). Verify no additional interpolation.
- **SSRF**: No WebFetch in V1 (deferred to V1.1) — confirm no HTTP client calls to arbitrary URLs
- **Keychain security**: Keys not stored in config file by default (only if explicitly set by user)
- **Race conditions**: All shared state in Bubble Tea mutated only in Update(), provider cache protected by sync.RWMutex

## Output Format

Produce an audit report with findings categorized as:

- **CRITICAL** — Will cause runtime failure, data corruption, or security issue. Must fix before Phase 6.
- **HIGH** — Incorrect behavior, spec violation, or likely to cause bugs. Should fix before Phase 6.
- **MEDIUM** — Code quality issue, missing edge case handling, or test gap. Fix in next cycle.
- **LOW** — Style, naming, or minor improvement. Track for later.

For each finding, include:
- Severity tag (CRITICAL/HIGH/MEDIUM/LOW)
- Short title
- File path(s) affected
- Description of the issue
- Spec reference (what the spec says vs what the code does)

End with:
- Total findings by severity
- Recommendation: Proceed to Phase 6 / Fix first / More investigation needed
- Test count: X existing + Y new = Z total
