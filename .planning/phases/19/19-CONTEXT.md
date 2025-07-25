# Phase 19 — Context: Comprehensive Codebase Concerns Fixes

**Gathered:** 2026-06-04
**Status:** Ready for planning
**Source:** `.planning/codebase/CONCERNS.md` (generated 2026-06-04)

<domain>
## Phase Boundary

Fix all 36 identified concerns from the codebase audit across 9 categories:
tech debt, known bugs, security, performance, fragile areas, scaling limits,
dependencies at risk, missing critical features, and test coverage gaps.

This phase does NOT add new features — it hardens, cleans, and tests existing code.

</domain>

<decisions>
## Implementation Decisions

### Tech Debt (6 items — all locked)

#### TD-1: `closeOnces` sync.Map unbounded growth
- **File:** `internal/tui/app.go:462-476`
- **Decision:** Replace global `sync.Map` with per-channel `sync.Once` stored in a wrapper struct. The wrapper holds the channel + Once together, eliminating the global map entirely.
- **Constraint:** Must not change the public API of `safeCloseOnce()`.

#### TD-2: `Stream: false` misleading field
- **Files:** `internal/workflow/engine.go:514-519`, `engine.go:543-548`
- **Decision:** Remove the `Stream` field from `ChatRequest` struct. Provider clients always hardcode `stream: true`. The field is dead code.
- **Constraint:** Update all `ChatRequest` construction sites to remove `Stream` field.

#### TD-3: Hardcoded model capability map
- **File:** `internal/provider/openrouter/client.go:48-60`
- **Decision:** Move the capability map to a package-level variable with a comment documenting how to update it. Add a `UpdateModelCapabilities()` function for runtime updates. Do NOT make this configurable via TOML (overkill for ~11 entries).
- **Constraint:** Keep the heuristic fallback for unknown models.

#### TD-4: `normalizeToolName` missing Edit/WebFetch
- **File:** `internal/workflow/engine.go:975-991`
- **Decision:** Add aliases: `edit` → `file_edit`, `search_replace` → `file_edit`, `web_fetch` → `web_fetch`, `fetch` → `web_fetch`, `http_get` → `web_fetch`.
- **Constraint:** Keep existing aliases unchanged.

#### TD-5: Deprecated `safeClose` alias
- **File:** `internal/tui/app.go:478-481`
- **Decision:** Migrate `RunPhaseCmd()` at line 356 to use `safeCloseOnce()`, then delete `safeClose()` entirely.

#### TD-6: `mergeConfig` manual field-by-field merge
- **File:** `internal/config/loader.go:118-199+`
- **Decision:** Replace with reflection-based merge using `reflect` package — copy non-zero values from project config over global defaults. This eliminates the need to update the function when adding config fields.
- **Constraint:** Must handle nested structs correctly. Test with all existing config fields.

### Known Bugs (5 items — all locked)

#### BUG-1: Verify fallback uses `HEAD~50`
- **File:** `internal/workflow/verify.go:59-64`
- **Decision:** When `sessionStartHash` is empty, use `git rev-list --max-parents=0 HEAD` to find the root commit instead of hardcoded `HEAD~50`.

#### BUG-2: Discuss timeout not configurable
- **File:** `internal/tui/app_workflow.go:147-161`
- **Decision:** Add `DiscussTimeout` field to `UIConfig` with default 5 minutes. Use it in the discuss timeout logic.

#### BUG-3: Permission timeout default mismatch
- **Files:** `internal/tools/dispatcher.go:45`, `internal/tui/app_update.go:630-632`
- **Decision:** Define `DefaultPermissionTimeout = 300` as a constant in `internal/types/constants.go` and reference it from both locations.

#### BUG-4: Git operations ignore errors silently
- **Files:** `internal/workflow/ship.go:62`, `internal/workflow/execute.go:247,344`
- **Decision:** Log errors using `slog.Warn()` instead of discarding with `_ =`. Do NOT return errors (would break the workflow), just log for observability.

#### BUG-5: (covered by BUG-3 above — permission timeout)

### Security (4 items — all locked)

#### SEC-1: SSRF TOCTOU gap in WebFetch
- **File:** `internal/tools/webfetch.go:49, 160-165`
- **Decision:** Implement a DNS cache that pins resolved IPs for the request lifecycle. Use a `sync.Map` keyed by hostname, with entries expiring after 5 minutes. `DialContext` checks the cached IP instead of re-resolving.

#### SEC-2: `os.Exit(1)` in library code
- **File:** `internal/tui/app.go:206`
- **Decision:** Change `NewApp()` to return `(*AppState, error)`. Let `cmd/m31a/main.go` handle the exit.

#### SEC-3: API key in `AppState.apiKey` field
- **File:** `internal/tui/app.go:70`
- **Decision:** Remove the `apiKey` field from `AppState`. Pass the key directly to provider constructors and do not store it in the TUI state. The key is only needed during provider initialization.

#### SEC-4: Backup directory accumulation
- **File:** `internal/tools/filewrite.go:113-134`
- **Decision:** Add backup pruning: keep at most 10 backups per file. When writing a new backup, delete the oldest if count exceeds 10.

### Performance (4 items — all locked)

#### PERF-1: `listCwdFiles` uses `filepath.Walk`
- **File:** `internal/workflow/engine.go:1155-1190`
- **Decision:** Replace `filepath.Walk` with `filepath.WalkDir` which is cheaper per-entry (no `os.Stat` on every entry).

#### PERF-2: HTML-to-Markdown repeated string operations
- **File:** `internal/tools/webfetch.go:286-538`
- **Decision:** Keep current approach (cap at 5MB, multiple passes acceptable). Add a comment documenting the tradeoff. No change needed — optimizing this is not worth the dependency on `golang.org/x/net/html`.

#### PERF-3: `extractJSONObject` re-scanning
- **File:** `internal/workflow/engine.go:848-877`
- **Decision:** Keep current approach with the 64KB `maxJSONScanBytes` cap. Document the O(n*m) complexity in a comment. Acceptable for V1.

#### PERF-4: `closeOnces` sync.Map lookup
- **Decision:** Resolved by TD-1 (eliminate the global sync.Map entirely).

### Fragile Areas (4 items — decomposition only)

#### FRAG-1: Workflow engine (1357 lines)
- **File:** `internal/workflow/engine.go`
- **Decision:** Extract message types to `engine_messages.go`, parsing functions to `engine_parse.go`, verification to `engine_verify.go`. Keep `engine.go` as the orchestrator only.

#### FRAG-2: TUI app_update.go (1238 lines)
- **File:** `internal/tui/app_update.go`
- **Decision:** Extract message handlers into per-screen handler files: `app_update_repl.go`, `app_update_workflow.go`, `app_update_model.go`. Keep the main switch in `app_update.go` but delegate to handler methods.

#### FRAG-3: Streaming pipeline
- **Files:** `internal/tui/streaming.go`, `repl_stream.go`, `repl.go`
- **Decision:** No structural changes — too risky. Add documentation comments explaining the goroutine ownership model and channel lifecycle. Run `go test -race` after any changes.

#### FRAG-4: Permission system
- **Files:** `internal/tools/permissions.go`, `dispatcher.go`
- **Decision:** Add comprehensive unit tests for glob matching, permission check flow, and timeout handling. Do NOT restructure — the complexity is inherent to the feature.

### Missing Features (3 items — all locked)

#### FEAT-1: Backup pruning
- **Decision:** Implemented as part of SEC-4.

#### FEAT-2: Session auto-cleanup
- **File:** `pkg/session/`
- **Decision:** Add `Cleanup(maxAge time.Duration)` function that removes session directories older than `maxAge`. Call it on startup from `NewApp()`. Default retention: 30 days.

#### FEAT-3: Config hot-reload
- **File:** `internal/config/loader.go`
- **Decision:** Add a file watcher goroutine that watches `~/.m31a/config.toml` for changes (using `os.Stat` polling every 5 seconds). On change, reload config and emit a `ConfigReloadMsg` to the TUI. Only reload non-provider fields (provider changes require reconnection).

### Test Coverage (7 gaps — all locked)

#### TEST-1: `internal/tui/app_update.go`
- **Decision:** Create `app_update_test.go` with tests for 5 most critical message type handlers.

#### TEST-2: `internal/tools/permissions.go`
- **Decision:** Create `permissions_test.go` with tests for `matchAnyParamValue()`, `matchToolName()`, and full permission check flow with glob patterns.

#### TEST-3: Command implementations
- **Decision:** Create `commands_test.go` covering `commands_ai.go`, `commands_config.go`, `commands_git.go`, `commands_session.go`, `commands_workflow.go` — at least one test per command handler.

#### TEST-4: `internal/tui/repl_stream.go`
- **Decision:** Create `repl_stream_test.go` for stream segment building and message assembly.

#### TEST-5: `internal/tui/repl_quickactions.go`
- **Decision:** Skip — low priority, UI convenience only.

#### TEST-6: OS-specific keychain
- **Decision:** Skip — compile-tag guards prevent cross-platform testing. Add build-tag-gated tests that run only on the target OS.

#### TEST-7: Error handling in workflow
- **Decision:** Add tests for `ship.go` git log failure and `execute.go` git HeadHash failure paths.

### Dependencies at Risk (3 items — monitoring only)

#### DEP-1: `golang.org/x/sync/singleflight`
- **Decision:** No action — appropriate usage, stable package.

#### DEP-2: `github.com/bmatcuk/doublestar/v4`
- **Decision:** Pin current version. Add a comment in `go.mod` noting the v4 major version risk.

#### DEP-3: `github.com/BurntSushi/toml`
- **Decision:** Pin current version. Add a TODO comment to migrate to v2 when ready.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Source of Truth
- `.planning/codebase/CONCERNS.md` — Full audit with file paths and line numbers
- `.planning/codebase/CONVENTIONS.md` — Code style and patterns to follow
- `.planning/codebase/TESTING.md` — Test patterns and framework usage

### Key Source Files
- `internal/tui/app.go` — AppState, safeCloseOnce, NewApp
- `internal/workflow/engine.go` — Workflow engine (1357 lines, needs decomposition)
- `internal/workflow/engine.go:514-548` — Stream field usage
- `internal/workflow/engine.go:975-991` — normalizeToolName
- `internal/workflow/engine.go:848-877` — parseToolCalls/extractJSONObject
- `internal/workflow/engine.go:1155-1190` — listCwdFiles
- `internal/workflow/verify.go:59-64` — HEAD~50 fallback
- `internal/workflow/ship.go:62` — silent git error
- `internal/workflow/execute.go:247,344` — silent git errors
- `internal/config/loader.go:118-199+` — mergeConfig
- `internal/provider/openrouter/client.go:48-60` — hardcoded capabilities
- `internal/tools/filewrite.go:113-134` — backup creation
- `internal/tools/webfetch.go:49,160-165` — SSRF TOCTOU
- `internal/tools/dispatcher.go:45` — permission timeout default
- `internal/tools/permissions.go` — glob matching (untested)
- `internal/tui/app_update.go` — 1238-line Update handler
- `internal/tui/app_workflow.go:147-161` — discuss timeout
- `internal/tui/streaming.go`, `repl_stream.go`, `repl.go` — streaming pipeline
- `cmd/m31a/main.go` — entry point (needs os.Exit fix)

</canonical_refs>

<specifics>
## Specific Ideas

- The `closeOnces` fix (TD-1) is the highest-priority tech debt item — it affects long sessions.
- The workflow engine decomposition (FRAG-1) is the highest-risk refactor — must be done carefully with tests passing at each step.
- Security fixes (SEC-1, SEC-2, SEC-3) should be Wave 1 — they are small, isolated, and high-value.
- Test coverage additions (TEST-1 through TEST-7) should be Wave 2 — they depend on the code being stable after Wave 1 fixes.

</specifics>

<deferred>
## Deferred Ideas

- PERF-2 and PERF-3 are explicitly deferred (accepted tradeoffs for V1).
- TEST-5 (quickactions) and TEST-6 (OS keychain) are deferred (low priority).
- No dependency upgrades in this phase — pin current versions only.

</deferred>

---

*Phase: 19-comprehensive-concerns-fixes*
*Context gathered: 2026-06-04 from CONCERNS.md audit*
