# Codebase Concerns

**Analysis Date:** 2026-06-11

## Tech Debt

**BurntSushi/toml v1 Dependency:**
- Issue: `github.com/BurntSushi/toml` v1.6.0 is in maintenance mode; v2 has a different API
- Files: `go.mod`, `internal/config/loader.go`
- Impact: Blocks upgrade path; v1 won't receive new features or critical fixes
- Fix approach: Migrate to `github.com/BurntSushi/toml/v2` when ready; requires API changes in config loader

**Duplicated Constants Between `types` and `tools`:**
- Issue: Seven constants are duplicated between `internal/types/constants.go` and `internal/tools/constants.go`. `DateFormat` has **different values** (`"2006-01-02"` vs `"2006-01-02 15:04"`), creating a latent bug
- Files: `internal/types/constants.go`, `internal/tools/constants.go`
- Impact: Code using the wrong constant formats dates incorrectly; maintenance burden of keeping two files in sync
- Fix approach: Extract a shared `internal/constants` package with zero dependencies

**`PermissionRule` Type in Wrong Package (CR-09):**
- Issue: `PermissionRule` is defined in `internal/config/types.go` but consumed by `internal/tools/permissions.go`, violating the rule that `internal/tools/` may only import `internal/types/` and `internal/errors/`
- Files: `internal/config/types.go`, `internal/tools/permissions.go`, `internal/tools/dispatcher.go`, `internal/tools/defaults.go`
- Impact: Architecture violation documented since Phase 25; creates TUI→Tools dependency via `internal/tui/components/permission.go`
- Fix approach: Move `PermissionRule` to `internal/types/types.go` and update all import paths (cascades across 6+ files)

**TUI Package Has No Tests:**
- Issue: `internal/tui/` is the largest package (~20K LOC, 1806 lines in `app_update.go` alone) with zero test files. Screen routing, key dispatch, workflow integration are completely untested
- Files: `internal/tui/*.go` (all model files)
- Impact: Regressions in TUI behavior are undetectable; refactoring is high-risk
- Fix approach: Add integration tests for `AppState.Update()` with mock messages; test screen transitions and key dispatch

**`internal/types/` Has No Tests:**
- Issue: Core types package with 213 lines has no test file
- Files: `internal/types/types.go`, `internal/types/constants.go`
- Impact: Type marshaling/unmarshaling behavior is unverified
- Fix approach: Add tests for `Message.MarshalJSON()`, `SkipDirsMap()`, and other exported functions

## Known Bugs

**Bug #1 [Nil Pointer — Permission Timeout Race]:**
- Symptoms: Panic when user presses `n` while permission countdown auto-fires
- Files: `internal/tui/app_update.go:1615-1650`
- Trigger: Permission modal timeout fires in same Update() batch as user's esc key
- Workaround: nil-check `m.permRequest` before accessing `.ID` (line 1621 now has this guard)

**Bug #11 [NaN in fuzzyAnchorReplace]:**
- Symptoms: Edit tool fails to match when `oldLines` has exactly 2 lines (first + last, no middle)
- Files: `internal/tools/edit.go:473-478`
- Trigger: `old_string` parameter with exactly 2 lines
- Workaround: None; the Levenshtein similarity check produces NaN which always evaluates false

**Bug #14 [Slice Alias in Edit Tool]:**
- Symptoms: Edit tool corrupts file content when replacing line ranges due to Go slice-append aliasing
- Files: `internal/tools/edit.go:310`
- Trigger: `replaceByLineRange` when `newContent` lines + remaining lines exceed original slice capacity
- Workaround: None; this is a latent data corruption bug

**Bug #10 [Backup Race Condition]:**
- Symptoms: Backup just written can be pruned immediately if two writes happen in the same millisecond
- Files: `internal/tools/filewrite.go:155-160`
- Trigger: Two rapid FileWrite calls to the same file within 1ms
- Workaround: None; the backup lifecycle ordering is inverted

## Security Considerations

**Bash Blacklist Is Trivially Bypassable:**
- Risk: The bash command blacklist uses simple substring matching; `rm -rf /*` can be bypassed with `rm -r -f /` or `/sbin/mkfs.ext4`
- Files: `internal/tools/bash.go:32-46`
- Current mitigation: Permission system is the actual security boundary; blacklist is defense-in-depth only
- Recommendations: Remove the blacklist entirely (rely on permission system) or replace with AST-based analysis

**`exec.Command` Without Path Validation in Verify Phase:**
- Risk: Verify phase runs LLM-generated file paths through `exec.CommandContext` without workDir containment check
- Files: `internal/workflow/engine_verify.go:141-227`
- Current mitigation: LLM-generated paths are typically relative and benign
- Recommendations: Validate that `path` is within `workDir` using `filepath.Rel` before passing to `exec.Command`

**`json.Unmarshal` Without Size Limits in Session Manager:**
- Risk: Session files are read entirely into memory; a corrupted file could be gigabytes
- Files: `pkg/session/manager.go:186, 213, 328, 556`
- Current mitigation: `readFileLimited()` added with `MaxSessionFileSize` (50 MB) limit
- Recommendations: Verify all call sites use `readFileLimited` instead of raw `os.ReadFile`

**Permission Config Fallback Silently Disables Security:**
- Risk: When permission configuration is invalid, `tools.DefaultDispatcher` falls back to a dispatcher with **no permission rules**, making all tool calls auto-approved
- Files: `cmd/m31a/main.go:138-142`
- Current mitigation: Log warning only
- Recommendations: Fail fast on permission config errors, or show a prominent TUI warning banner

**`webfetch.go` DNS Cache TOCTOU Window:**
- Risk: DNS cache uses `sync.Map` with 5-minute TTL; a rebinding attack could exploit the window between resolution and connection
- Files: `internal/tools/webfetch.go:39-45, 207+`
- Current mitigation: Post-connect re-check of remote IP against private range
- Recommendations: Current mitigation is sufficient (pinned IP + post-connect check)

## Performance Bottlenecks

**`time.Sleep` in Task Runner Retry Logic:**
- Problem: `pkg/taskrunner/runner.go:203` uses `time.Sleep` for retry backoff, blocking the goroutine and ignoring context cancellation
- Files: `pkg/taskrunner/runner.go:203`
- Cause: Simple `time.Sleep(time.Duration(attempt+1) * time.Second)` blocks without select on `ctx.Done()`
- Improvement path: Use `time.NewTimer` with `select` on `ctx.Done()` for cancellable sleep

**`SkipDirsMap()` Returns Copy Every Call:**
- Problem: `internal/types/constants.go:127-140` returns a full map copy on every call to prevent mutation of the cached map
- Files: `internal/types/constants.go:127-140`
- Cause: Defensive copy pattern; each caller gets a new map allocation
- Improvement path: Return the cached map directly (callers should not mutate it); document immutability contract

**Unbounded LLM Response in `parseToolCalls`:**
- Problem: `parseToolCalls` does not consistently enforce `MaxLLMResponseBytes` before `json.Unmarshal`
- Files: `internal/workflow/engine_parse.go:29, 402, 430`
- Cause: Limit enforcement is at HTTP level, not at JSON parse level
- Improvement path: Enforce size limit at the SSE parser level before data reaches `json.Unmarshal`

## Fragile Areas

**`internal/tui/app_update.go` (1806 lines):**
- Files: `internal/tui/app_update.go`
- Why fragile: Single 1806-line file handling ALL Bubble Tea message types; screen routing, key dispatch, workflow integration, permission handling, streaming, and auto-fallback all live here
- Safe modification: Split by concern (e.g., `app_update_permissions.go`, `app_update_streaming.go`, `app_update_workflow.go`)
- Test coverage: Zero

**Signal Handler in `main.go`:**
- Files: `cmd/m31a/main.go:186-193`
- Why fragile: Signal handler goroutine calls `app.Shutdown()` concurrently with Bubble Tea `Update()` loop, violating the single-threaded mutation rule
- Safe modification: Send `tea.Quit` through the Bubble Tea channel instead of calling `Shutdown()` directly
- Test coverage: None

**SSE Parser Watchdog Timeout:**
- Files: `internal/provider/sse.go:34-36`
- Why fragile: Watchdog timer closes `resp.Body` on timeout, which can cause `io.ErrClosedPipe` in the scanner goroutine; the error path must handle this gracefully
- Safe modification: Ensure all callers check for `io.ErrClosedPipe` and treat it as a timeout, not an error
- Test coverage: Unit tests exist but may not cover all timeout edge cases

**`config_model.go` Division by Zero:**
- Files: `internal/tui/config_model.go:709`
- Why fragile: `f.choices[(idx+1)%len(f.choices)]` panics when `len(f.choices) == 0`
- Safe modification: Guard with `if len(f.choices) == 0 { return }` before the modulo
- Test coverage: None

## Scaling Limits

**Session File Size:**
- Current capacity: 50 MB (`MaxSessionFileSize` constant)
- Limit: Corrupted or large sessions can cause OOM before the limit is checked
- Scaling path: Already mitigated with `readFileLimited()`; verify all load paths use it

**Tool Output Cap:**
- Current capacity: 50,000 characters (`BashOutputLimit` constant)
- Limit: Very large command output is silently truncated; user may miss important error details
- Scaling path: Consider streaming output to TUI instead of buffering entire output

**Model Cache Staleness:**
- Current capacity: 5-minute TTL, 24-hour stale TTL
- Limit: If provider is unreachable for >24 hours, stale cache is discarded and no models are available
- Scaling path: Consider persistent disk cache as fallback

## Dependencies at Risk

**`github.com/BurntSushi/toml` v1:**
- Risk: In maintenance mode; v2 has breaking API changes
- Impact: Config parsing would need rewrite for v2
- Migration plan: Plan for v2 migration when ready; `toml.DecodeFile` API changes

**`github.com/pkoukk/tiktoken-go`:**
- Risk: Community-maintained; may lag behind OpenAI tokenizer updates
- Impact: Token estimation could be inaccurate for newer models
- Migration plan: Monitor for updates; consider fallback to `len(runes)/4*1.3` for unsupported models

**`github.com/godbus/dbus/v5`:**
- Risk: Linux-only dependency for keychain access; may have compatibility issues with newer systemd
- Impact: Keychain storage on Linux could break
- Migration plan: Already handled with build tags (`keychain_linux.go`); test on target platforms

## Missing Critical Features

**No TUI Integration Tests:**
- Problem: The entire `internal/tui/` package (20K+ LOC) has zero test coverage
- Blocks: Safe refactoring of screen routing, key handling, workflow integration

**No E2E Test Framework:**
- Problem: No end-to-end test harness that starts the TUI and simulates user interaction
- Blocks: Verification of complete workflow phases (initialize → discuss → plan → execute → verify → ship)

**No Structured Error Types Beyond Sentinels:**
- Problem: All errors are sentinel `errors.New()` strings; no typed errors with context (e.g., which provider failed, which task errored)
- Blocks: Rich error reporting and programmatic error handling

## Test Coverage Gaps

**`internal/tui/` (Zero Tests):**
- What's not tested: All screen models, key dispatch, workflow integration, streaming pipeline, permission modal
- Files: `internal/tui/app_update.go`, `internal/tui/repl_model.go`, `internal/tui/config_model.go`
- Risk: Regressions in core user-facing behavior go undetected
- Priority: High

**`internal/types/` (Zero Tests):**
- What's not tested: `Message.MarshalJSON()`, `SkipDirsMap()`, type conversions
- Files: `internal/types/types.go`, `internal/types/constants.go`
- Risk: Serialization bugs in session persistence
- Priority: Medium

**`cmd/m31a/` (Zero Tests):**
- What's not tested: Entry point initialization, signal handling, provider registration
- Files: `cmd/m31a/main.go`
- Risk: Startup failures are only caught by manual testing
- Priority: Low

**Workflow Engine Integration:**
- What's not tested: Full phase transitions with mock providers, checkpoint/restore cycle
- Files: `internal/workflow/engine.go`, `internal/workflow/execute.go`
- Risk: Phase transition bugs corrupt session state
- Priority: High

---

*Concerns audit: 2026-06-11*
