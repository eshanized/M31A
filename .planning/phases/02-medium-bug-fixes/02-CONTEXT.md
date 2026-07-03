# Phase 2: MEDIUM Bug Fixes - Context

**Gathered:** 2026-07-03
**Status:** Ready for planning
**Source:** BUG_REPORT.md (Codebase Audit) — MEDIUM severity issues

<domain>
## Phase Boundary

Fix all 22 MEDIUM severity bugs identified in the comprehensive codebase audit. These bugs include concurrency issues (double-close, TOCTOU, deadlock risks), fragile error string comparisons, unchecked errors, non-deterministic behavior, unsafe type assertions, performance issues (O(n²) allocations), and logic errors that degrade correctness, reliability, or maintainability.

</domain>

<decisions>
## Implementation Decisions

### D-01: Concurrency Bug Fixes (MEDIUM)
- M1: Fix double-close on iterator in `internal/tui/streaming/agent_loop.go:210-217`
- M2: Fix callback-under-lock deadlock risk in `pkg/taskrunner/runner.go:283-284`
- M3: Fix entry accessed after map deletion without lock in `internal/tools/devserver.go:344-362`
- M20: Fix goroutine leak on early return in `internal/tui/streaming/agent_loop.go:211-217`

### D-02: Fragile Error Handling Fixes
- M4: Replace `err.Error() == "EOF"` with `errors.Is(err, io.EOF)` in `internal/workflow/intent.go:99`
- M5: Replace `strings.Contains(err.Error(), "does not have any commits")` with typed error check in `internal/git/git.go:231`
- M6: Replace `strings.Contains(errStr, "connection refused")` with typed error check in `internal/tools/webfetch.go:411-419`
- M7: Preserve response body in error wrapping in `internal/provider/nvidia/client.go:88,150`, `openrouter/client.go:103,155`, `zen/client.go:90`

### D-03: Unchecked Error Fixes
- M8: Check errors from session persistence loads in `internal/workflow/ship.go:251,443,471`
- M9: Add nil check on `DirEntry.Info()` in `internal/tools/filelist.go:201`
- M10: Check error from `LoadProjectContext` in `internal/tui/streaming/agent_loop.go:483`

### D-04: Context Management Fixes
- M11: Replace `context.Background()` with engine's working context in `internal/workflow/engine.go:1217`
- M12: Attach compaction context to parent context in `internal/workflow/engine.go:899,1030`

### D-05: TOCTOU and Safety Fixes
- M13: Fix TOCTOU between Stat and ReadFile in `internal/tools/edit.go:103-112`
- M14: Normalize command args before blocklist matching in `internal/tools/bash.go:477-490`
- M19: Add comma-ok type assertion for `RemoteAddr()` in `internal/tools/webfetch.go:102`

### D-06: Determinism and Correctness Fixes
- M15: Sort map iteration for deterministic Q&A order in `internal/workflow/plan.go:496-498`
- M16: Sort map iteration for deterministic discuss prompt in `internal/workflow/discuss.go:265`
- M17: Fix slice in-place mutation via append in `internal/codeintel/index.go:156`
- M18: Fix same slice mutation pattern in `internal/codeintel/graph.go:94,107`

### D-07: Performance Fixes
- M21: Replace string concat with `strings.Builder` in `internal/workflow/execute.go:741-746`
- M22: Replace `fmt.Sprintf` concat with `strings.Builder` in `internal/tools/codemap.go:119-133`

### the agent's Discretion
- Fix ordering within each bug category (group by file/package when possible)
- Whether to add unit tests for each fix
- Whether to add regression tests for concurrency fixes
- Specific implementation details for error wrapping patterns

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Architecture
- `.planning/codebase/ARCHITECTURE.md` — System architecture and component responsibilities
- `.planning/codebase/CONVENTIONS.md` — Code style and conventions
- `.planning/codebase/STACK.md` — Technology stack
- `.planning/codebase/TESTING.md` — Testing patterns and requirements

### Bug Report
- `BUG_REPORT.md` — Complete bug report with all 59 issues (M1-M22 are this phase's scope)

### Project Guidelines
- `AGENTS.md` — Project-specific guidelines and build commands

### Prior Phase Patterns
- `.planning/phases/01-bug-fix/01-CONTEXT.md` — Phase 1 decisions (comma-ok, len guards)
- `.planning/phases/01-bug-fix/01-01-SUMMARY.md` through `01-04-SUMMARY.md` — How CRITICAL/HIGH bugs were fixed

</canonical_refs>

<specifics>
## Specific Ideas

- Each fix should be atomic and testable
- Group fixes by file/package when possible to minimize context switching
- Concurrency fixes should use `go test -race` to verify
- Error handling fixes should use `errors.Is()` and `errors.As()` patterns
- Performance fixes should use benchmarks to verify improvement
- Phase 1 established patterns: comma-ok type assertions, len guards, error checking

</specifics>

<deferred>
## Deferred Ideas

- LOW severity bugs (L1-L18) — deferred to Phase 3
- Performance optimizations beyond bug fixes
- New features or enhancements
- Architecture changes beyond bug fixes

</deferred>

---

*Phase: 02-medium-bug-fixes*
*Context gathered: 2026-07-03 from BUG_REPORT.md*
