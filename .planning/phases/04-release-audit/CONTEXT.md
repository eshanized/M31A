# Phase 4: Release Audit Blockers — Context

**Gathered:** 2026-07-14
**Status:** Ready for planning
**Source:** RELEASE_AUDIT_V1.md

<domain>
## Phase Boundary

This phase resolves ALL CRITICAL and HIGH blockers discovered during the independent release audit (RELEASE_AUDIT_V1.md). The objective is to clear the path for v1.0 release. Every issue must be investigated and resolved.

Priority order:
1. CRITICAL architectural violations (C1: `pkg/` → `internal/`)
2. CRITICAL data races (C3: `e.provider` without mutex)
3. CRITICAL test suite timeouts (C2: `internal/tools` and `pkg/bisect`)
4. HIGH security bypasses (H1: command blocklist, H2: prompt injection, H3: sandbox failure, H4: subagent isolation)
5. HIGH error chain breakage (H5: 142 `fmt.Errorf` without `%w`)
6. HIGH code quality (H6: god objects, H7: magic strings, H8: test timeouts)

</domain>

<decisions>
## Implementation Decisions

### C1: Fix `pkg/` → `internal/` Architectural Violation

10 `pkg/` packages import `internal/` packages with 50+ import lines:
- `pkg/taskrunner` → `internal/errors`, `internal/types`
- `pkg/arbitrage` → `internal/types`
- `pkg/bisect` → `internal/errors`, `internal/git`
- `pkg/ledger` → `internal/errors`, `internal/fileutil`, `internal/types`
- `pkg/rollback` → `internal/git`, `internal/types`
- `pkg/autodream` → `internal/tokens`, `internal/types`
- `pkg/session` → `internal/errors`, `internal/fileutil`, `internal/types`
- `pkg/metrics` → `internal/types`
- `pkg/compaction` → `internal/provider`, `internal/tokens`, `internal/types`
- `pkg/narrative` → `internal/workflow`, `internal/types`

**Fix:** Move shared types from `internal/types/` to `pkg/types/`. Define interfaces in `pkg/` that `internal/` implements. For `pkg/narrative`, replace type switch with interface-based approach.

### C2: Fix Test Suite Timeouts

- `internal/tools` — Hangs due to DNS lookups in WebSearch/WebFetch tests
- `pkg/bisect` — Times out at 90s due to real git operations

**Fix:** Mock DNS resolution for WebSearch/WebFetch tests. Make bisect tests use mocks instead of real git operations for happy path.

### C3: Fix Data Race on `e.provider`

`engine.go:789` (`SetModel`) writes `e.provider` without synchronization. Read by `streamLLM*`, `preflightContextCheck`, `proactiveCompactCheck`.

**Fix:** Protect `e.provider` with `modelIDMu` or use `atomic.Value`. Swap both `modelID` and `provider` atomically.

### H1: Expand Command Blocklist

Current blocklist is substring-based and bypassable via:
- `$()` command substitution not detected
- Backtick substitution not detected
- Missing patterns: `mkfs.ext4`, `fdisk`, `wipefs`, `shred`, `nc -l`, `ncat -l`
- Newline chaining passes validation

**Fix:** Add `$()`, backtick detection. Expand blocklist. Add chaining awareness.

### H2: Add Prompt Injection Defense

Tool outputs passed directly into LLM conversation as raw strings. Malicious files can inject instructions.

**Fix:** Wrap tool outputs in `<tool_output>...</tool_output>` delimiters. Add system prompt instruction that content within delimiters is data, not instructions.

### H3: Fix Sandbox Failure Silent Proceed

When `applyBashSandbox()` fails, code proceeds without OS-level sandboxing.

**Fix:** Surface degraded security mode to user visibly. Consider refusing execution on unsupported platforms.

### H4: Default Subagent Isolation to Worktree

`IsolationDefault` shares parent's working directory.

**Fix:** Default to `IsolationWorktree`.

### H5: Fix 142 `fmt.Errorf` Without `%w`

Broken error chains throughout `pkg/` and `internal/`.

**Fix:** Replace `%s`/`%v` with `%w` in all `fmt.Errorf` calls that wrap errors.

### H6: Decompose God Objects

- `engine.go` (1688 lines) — 50+ methods
- `sidebar_model.go` (1652 lines) — 80+ methods
- `app_view.go` (1266 lines) — 40+ render methods

**Fix:** Extract collaborators. Split large files into focused modules.

### H7: Define Provider Name Constants

50+ hardcoded `"openrouter"`, `"zen"`, `"nvidia"` strings.

**Fix:** Define constants in `internal/types/`:
```go
const (
    ProviderOpenRouter = "openrouter"
    ProviderZen        = "zen"
    ProviderNvidia     = "nvidia"
)
```

### H8: Fix Test Suite Completion

All test suites must complete within 60s under `-short`.

**Fix:** Mock external dependencies (DNS, git operations). Add test timeouts.

### M1: Add Permission Rule Expiry/Revocation

Persisted permission rules have no expiry or revocation mechanism.

**Fix:** Add TTL to persisted rules. Add `/permissions` command to list/revoke.

### M2: Fix Unprotected Engine Fields

`e.state.intentResult`, `e.websiteTemplateDir`, `e.sessionID` written without lock.

**Fix:** Protect with appropriate mutexes.

### M3: Fix `LoadWorkflowState` File Lock

`LoadWorkflowState` calls `loadSessionMetadata()` without acquiring file lock.

**Fix:** Acquire lock before metadata read.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Audit Report
- `RELEASE_AUDIT_V1.md` — Single source of truth for all issues

### Source Files
- `internal/workflow/engine.go` — Data race (C3), god object (H6)
- `internal/tools/bash.go` — Command blocklist (H1), sandbox failure (H3)
- `internal/tools/dispatcher.go` — Prompt injection (H2)
- `internal/tools/subagent/manager.go` — Subagent isolation (H4)
- `pkg/*/` — Architectural violations (C1)
- `internal/types/` — Shared types for extraction

### Project Guidelines
- `AGENTS.md` — Build requirements, architecture, code style

</canonical_refs>

<specifics>
## Specific Ideas

### Quality Gates (Post-Phase)
- `make check` must pass (fmt → tidy → vet → lint → test)
- `make lint` must pass (golangci-lint, 5m timeout)
- `make test` must pass (race-enabled tests with coverage)
- `go build ./...` must succeed
- `grep -r '"github.com/eshanized/M31A/internal' pkg/` returns empty

### Testing Requirements
Every fix must include:
- Unit tests
- Regression tests
- Concurrency tests where applicable
- Security tests where applicable

### Completion Criteria
Continue until every blocker from the audit report is either:
- Fixed
- Verified already fixed
- Rejected with technical justification

Produce RELEASE_AUDIT_RESOLUTION.md with:
- Fixed Critical Issues
- Fixed High Issues
- Fixed Medium Issues
- Tests Added
- Lint Status
- Vet Status
- Race Status
- Remaining Risks
- Production Readiness Score
- Recommendation

End with exactly one of:
- APPROVED FOR V1.0
- APPROVED WITH MINOR CHANGES
- NOT READY FOR V1.0

</specifics>

<deferred>
## Deferred Ideas

- Decompose god objects (engine.go, sidebar_model.go) — complex, needs careful design
- Optimize codeintel double file reads — performance, not correctness
- Cache tiktoken tokenizer per model — performance optimization
- Add ledger dedup map and sort caching — performance optimization
- Reduce binary size (UPX or selective tree-sitter grammars) — release engineering
- Split config structs into sub-structs — maintainability, not correctness
- Replace `time.Sleep` in tests with event-based synchronization — test quality

None of these block v1.0 release.

</deferred>

---

*Phase: 04-release-audit*
*Context gathered: 2026-07-14 via RELEASE_AUDIT_V1.md*
