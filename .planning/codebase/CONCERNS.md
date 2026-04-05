# CONCERNS.md — Technical Debt, Issues & Areas of Concern

**Last updated:** 2026-06-13
**Project:** M31A — Terminal AI Coding Agent

## Critical Concerns

### Signal Handler Race Condition (`cmd/m31a/main.go:262-287`)
The signal handler goroutine previously called `app.Shutdown()` concurrently with the Bubble Tea `Update()` loop, violating Bubble Tea's single-threaded contract. This has been partially mitigated — the current code now sends `tea.QuitMsg` through the program channel instead, but still has a 5-second hard `os.Exit(1)` fallback timer that can race with cleanup. If SIGINT arrives mid-Update partially-mutated state could still occur.

**Status:** Mitigated but not fully resolved — the 5-second fallback `os.Exit(1)` in the timeout path still skips deferred cleanup.

### Large Test Files — Maintenance Burden
Several test files are excessively large, indicating they may be testing too broadly:
- `internal/tools/extra_test.go` — 4,393 lines
- `internal/workflow/engine_extra_test.go` — 3,394 lines
- `internal/tui/tui_test.go` — 1,831 lines
- `internal/tui/components/extra_test.go` — 1,860 lines

These files mix unit and integration tests and would benefit from splitting into focused test files per function/area.

### Provider Degradation — Silent Failure (`cmd/m31a/main.go:143-146`)
When no API key is configured or all providers fail to register, the app starts silently with no LLM capability. The user sees no visible error — the REPL simply does nothing. This creates a confusing first-time user experience.

**Status:** Known issue, needs a startup banner or first-run detection.

## Technical Debt

### Unused Parameters and Dead Code
The `adrenaline/UNUSED-PARAMETERS-REPORT.md` (514 lines) catalogues extensive dead code:
- `internal/git/git.go:143` — `oneline` parameter accepted but never read
- `internal/git/git.go:162` — `parseLog` parameter `oneline` passed through but ignored
- Several unused function parameters, return values, struct fields, and dead functions across the codebase
- Multiple unused types, constants, and variables

### Config Complexity (`internal/config/types.go`)
The `UIConfig` struct has grown to 50+ fields with many "New" additions (line 100+: accent colors, custom backgrounds, animation settings, status bar styles, tool card styles, toast settings). This complexity makes the config surface hard to document and maintain. Consider grouping into sub-configs.

### Large Files
- `internal/config/loader.go` — 1,012 lines (config loading + validation + hot-reload + migration)
- `internal/tui/app_update.go` — 2,124 lines (main update handler)
- `internal/tui/firstrun_view.go` — 1,115 lines
- `internal/tui/config_model.go` — 1,103 lines

These files handle multiple responsibilities and would benefit from decomposition.

### Dependency Pins with Migration Notes (`go.mod`)
```go
// DEP-3: BurntSushi/toml v1 — in maintenance mode; v2 has different API
// DEP-2: doublestar v4 — pin current version; check for breaking changes
// DEP-1: golang.org/x/sync/singleflight — stable, appropriate usage
```
Three dependency-anchoring comments in `go.mod` with known upgrade paths that haven't been actioned.

## Known Bugs

### Glob Tool RG Path Issue (`internal/tools/glob_test.go:164-169`)
```go
// BUG(glob): rg code path has a known issue where os.Stat fails on relative
// paths when CWD != workDir.
```
The `rg` (ripgrep) code path in the Glob tool has a known CWD/workDir mismatch issue. Tests are written to skip rather than fail on this issue. This affects glob results when the working directory differs from the tool's configured workDir.

## Security Considerations

### SSRF Protection (`internal/errors/errors.go:35`)
WebFetch tool blocks private IP ranges, loopback, and link-local addresses. This is well-implemented.

### API Key Security
- Keys resolved via OS keychain with file-based fallback
- Provider error messages sanitized to 200 chars max to prevent key leakage
- Config file permissions left to user; no explicit protection beyond what the OS provides

### File Size Limits
Multiple size limits protect against OOM: 5MB max file read, 50MB max session file, 1MB max LLM response, 50KB bash output limit. Well-covered.

## Performance Concerns

### Token Estimation Accuracy
- `internal/tokens/estimator.go` — uses tiktoken-go with EMA calibration
- EMA alpha default: 0.3 — may calibrate too slowly for workloads with rapidly changing token patterns
- Calibration rate configurable but requires user tuning

### Model Cache
- Provider model cache: 5 min TTL live, 24h stale TTL
- Health check interval: 60s
- Session cache TTL: 2s
- Reasonable defaults but no cache warming on startup

### SSE Streaming
- SSE stream parsing in `internal/provider/sse.go`
- `MaxRetryAfterWait` of 120s for rate-limited providers
- Stream truncation detection via `ErrStreamTruncated`
- No backpressure mechanism on stream reads

## Architectural Concerns

### TUI Model Proliferation
The `internal/tui/` directory has grown to 50+ source files with 20+ model types. The Model-View-Update pattern is clean, but the number of inter-model message types (`app_channel.go`) creates coupling between models. Adding a new screen requires message handler changes in multiple files.

### Workflow Engine Size
`internal/workflow/engine.go` is 828 lines and `engine_extra_test.go` is 3,394 lines. The engine handles too many responsibilities: prompt loading, phase transitions, tool execution, streaming, self-healing, and progress reporting. A more modular design would split these concerns.

### Adrenaline Reports Proliferation
The `adrenaline/` directory contains 28 audit/review reports totaling significant documentation. While these provide valuable analysis, many contain overlapping findings and some may be outdated. Consider consolidating into a living quality document rather than static snapshots.

### Provider Registration at Startup
Both providers (OpenRouter, Zen) are initialized at startup regardless of whether they're configured. This wastes memory on unused provider objects and their cached model lists. Lazy initialization would be more efficient.

## Dependencies

### Indirect Dependency Risk
The `go.sum` file has grown to 54 indirect dependencies pulled in by the Bubble Tea ecosystem (glamour, lipgloss, bubbles). This is expected for a rich TUI but creates supply chain surface area. Dependencies like `bluemonday` (HTML sanitization) and `chroma` (syntax highlighting) are transitive deps that add non-trivial binary size.

### Pin Lock
Several dependencies are pinned to specific versions from 2020-2023 era (`golang.org/x/net v0.0.0-20221002...`, `microcosm-cc/bluemonday v1.0.21`). These should be reviewed for updates, particularly `x/net` which had security advisories in later versions.

## Code Quality Notes

### Inline Comments
- `go.mod` comments follow a `DEP-N:` convention for dependency tracking — good practice
- `main.go` comments reference design rules (e.g., `§2.3`, `WP-C03`, `H-24`) — suggests a design document or audit report is the source of truth
- Inline comments generally explain "why" not "what" — good practice

### Error Handling
- Well-structured sentinel errors with user-friendly message mapping
- Pattern matching for HTTP status codes, connection errors, TLS errors
- `%w` wrapping for `errors.Is` compatibility
- Clean and consistent pattern across the codebase
