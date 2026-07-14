---
phase: 04-release-audit
mode: standard
granularity: standard
wave: 1-5
must_haves:
  - "Zero `pkg/` → `internal/` imports (grep returns empty)"
  - "Data race on `e.provider` fixed (mutex protected)"
  - "All test suites complete within 60s under `-short`"
  - "Command blocklist catches `$()`, backticks, and missing destructive patterns"
  - "Tool outputs wrapped in `<tool_output>` delimiters"
  - "All `fmt.Errorf` calls use `%w` for error wrapping"
  - "Provider names defined as constants (single source of truth)"
  - "`cmd/m31a` and `internal/decision` have test coverage"
  - "go vet passes clean"
  - "make lint passes clean"
  - "Race detector passes on all packages"
---

# Phase 4: Release Audit Blockers — PLAN

**Goal:** Resolve all CRITICAL and HIGH blockers from RELEASE_AUDIT_V1.md to clear path for v1.0 release.

## Wave 1: Architectural Foundation (C1)

### Task 1: Extract Shared Types to `pkg/types/`

**Files to modify:**
- Create `pkg/types/` package with shared types from `internal/types/`
- Update all `pkg/` imports to use `pkg/types/` instead of `internal/types/`

**Acceptance Criteria:**
- `pkg/types/` exists with `Message`, `Task`, `WorkflowPhase`, `WorkflowMode`, `RiskLevel` types
- All `pkg/` packages import from `pkg/types/` not `internal/types/`
- `grep -r '"github.com/eshanized/M31A/internal/types' pkg/` returns empty
- `go build ./...` succeeds
- `go vet ./...` passes

**Verification:**
```bash
# Must return empty
grep -r '"github.com/eshanized/M31A/internal/types' pkg/ --include='*.go'

# Must succeed
go build ./...
go vet ./...
```

### Task 2: Extract Error Types to `pkg/errors/`

**Files to modify:**
- Create `pkg/errors/` package with sentinel errors from `internal/errors/`
- Update all `pkg/` imports to use `pkg/errors/` instead of `internal/errors/`

**Acceptance Criteria:**
- `pkg/errors/` exists with all sentinel errors
- All `pkg/` packages import from `pkg/errors/` not `internal/errors/`
- `grep -r '"github.com/eshanized/M31A/internal/errors' pkg/` returns empty

**Verification:**
```bash
# Must return empty
grep -r '"github.com/eshanized/M31A/internal/errors' pkg/ --include='*.go'
```

### Task 3: Fix `pkg/narrative` Cross-Layer Coupling

**Files to modify:**
- `pkg/narrative/bridge.go` — Replace type switch with interface

**Acceptance Criteria:**
- `pkg/narrative` does not import `internal/workflow`
- Define `WorkflowEvent` interface in `pkg/narrative`
- `internal/workflow` message types implement the interface
- Type switch replaced with interface method calls

**Verification:**
```bash
# Must return empty
grep -r '"github.com/eshanized/M31A/internal/workflow' pkg/ --include='*.go'
```

### Task 4: Fix Remaining `pkg/` → `internal/` Imports

**Files to modify:**
- `pkg/compaction/compaction.go` — Remove `internal/provider` import
- `pkg/autodream/autodream.go` — Remove `internal/tokens` import
- `pkg/bisect/bisect.go` — Remove `internal/git` import
- `pkg/rollback/rollback.go` — Remove `internal/git` import
- `pkg/ledger/ledger.go` — Remove `internal/fileutil` import
- `pkg/session/manager.go` — Remove `internal/fileutil` import

**Acceptance Criteria:**
- `grep -r '"github.com/eshanized/M31A/internal' pkg/` returns empty
- All `pkg/` packages are independently importable
- `go build ./...` succeeds

**Verification:**
```bash
# Must return empty
grep -r '"github.com/eshanized/M31A/internal' pkg/ --include='*.go' | grep -v '_test.go'
```

---

## Wave 2: Critical Security & Concurrency (C2, C3, H1-H4)

### Task 5: Fix Data Race on `e.provider` (C3)

**Files to modify:**
- `internal/workflow/engine.go` — Protect `e.provider` with mutex

**Acceptance Criteria:**
- `e.provider` protected by `modelIDMu` or dedicated mutex
- `SetModel()` acquires lock before writing `e.provider`
- All readers acquire lock before reading `e.provider`
- Race detector passes: `go test -race ./internal/workflow/...`

**Verification:**
```bash
go test -race -count=1 -timeout=60s ./internal/workflow/...
```

### Task 6: Fix Test Suite Timeouts (C2)

**Files to modify:**
- `internal/tools/websearch_test.go` — Mock DNS resolution
- `internal/tools/webfetch_test.go` — Mock HTTP responses
- `pkg/bisect/bisect_test.go` — Use mocks for git operations

**Acceptance Criteria:**
- `go test -short -count=1 -timeout=60s ./internal/tools/...` passes
- `go test -short -count=1 -timeout=60s ./pkg/bisect/...` passes
- No real network calls in tests
- No real git operations in bisect happy path tests

**Verification:**
```bash
go test -short -count=1 -timeout=60s ./internal/tools/...
go test -short -count=1 -timeout=60s ./pkg/bisect/...
```

### Task 7: Expand Command Blocklist (H1)

**Files to modify:**
- `internal/tools/bash.go` — Add missing patterns and detection

**Acceptance Criteria:**
- `$(...)` command substitution detected
- Backtick substitution detected
- Missing patterns added: `mkfs.ext4`, `fdisk`, `wipefs`, `shred`, `nc -l`, `ncat -l`
- Newline chaining detected
- Tests for all new patterns

**Verification:**
```bash
go test -run TestBash_Dangerous -v ./internal/tools/...
```

### Task 8: Add Prompt Injection Defense (H2)

**Files to modify:**
- `internal/tools/dispatcher.go` — Wrap tool outputs
- `internal/tui/streaming/agent_loop.go` — Update system prompt

**Acceptance Criteria:**
- Tool outputs wrapped in `<tool_output>...</tool_output>` delimiters
- System prompt includes instruction that content within delimiters is data
- All tool outputs (FileRead, Grep, WebFetch, Bash) wrapped
- Subagent outputs also wrapped

**Verification:**
```bash
grep -r '<tool_output>' internal/tools/ internal/tui/streaming/
```

### Task 9: Fix Sandbox Failure Silent Proceed (H3)

**Files to modify:**
- `internal/tools/bash.go` — Surface degraded mode

**Acceptance Criteria:**
- When sandbox fails, user sees visible warning
- Warning includes platform and reason
- Consider refusing execution on unsupported platforms (configurable)

**Verification:**
```bash
go test -run TestBash_Sandbox -v ./internal/tools/...
```

### Task 10: Default Subagent Isolation to Worktree (H4)

**Files to modify:**
- `internal/tools/subagent/manager.go` — Change default

**Acceptance Criteria:**
- `IsolationDefault` maps to `IsolationWorktree`
- Subagents get isolated worktree by default
- User can opt-in to shared directory via `isolation: "default"`

**Verification:**
```bash
go test -run TestSubagent_Isolation -v ./internal/tools/subagent/...
```

---

## Wave 3: Error Chains & Constants (H5, H7)

### Task 11: Fix 142 `fmt.Errorf` Without `%w` (H5)

**Files to modify:**
- All files with `fmt.Errorf` without `%w`

**Acceptance Criteria:**
- Every `fmt.Errorf` that wraps an error uses `%w`
- Only new errors (not wrapping) use `%v` or `%s`
- `errors.Is()` and `errors.As()` work correctly
- Tests verify error chain preservation

**Verification:**
```bash
# Count remaining %v/%s wrapping (should be zero for error-wrapping cases)
grep -r 'fmt.Errorf.*%[vs]' --include='*.go' internal/ pkg/ | grep -v '_test.go' | wc -l
```

### Task 12: Define Provider Name Constants (H7)

**Files to modify:**
- `internal/types/providers.go` — Define constants
- All files with hardcoded provider strings — Use constants

**Acceptance Criteria:**
- `ProviderOpenRouter`, `ProviderZen`, `ProviderNvidia` constants defined
- All 50+ hardcoded strings replaced with constants
- Single source of truth for provider names
- Renaming a provider requires changing one constant

**Verification:**
```bash
# Should only find constant definitions, not string literals
grep -r '"openrouter"\|"zen"\|"nvidia"' --include='*.go' internal/ pkg/ | grep -v 'const\|Provider'
```

---

## Wave 4: Test Coverage & Medium Issues (H8, M1-M3)

### Task 13: Add `cmd/m31a` Test Coverage (H8)

**Files to modify:**
- Create `cmd/m31a/main_test.go`

**Acceptance Criteria:**
- Tests for flag parsing (`--version`, `--help`, `--prompt`, `--goal`)
- Tests for config loading from env/file
- Tests for provider registration
- Coverage > 50% for `cmd/m31a`

**Verification:**
```bash
go test -cover ./cmd/m31a/...
```

### Task 14: Add `internal/decision` Test Coverage (H8)

**Files to modify:**
- Create `internal/decision/*_test.go`

**Acceptance Criteria:**
- Tests for `redact.go` (PII safety)
- Tests for `query.go` (audit trail)
- Tests for `receipt.go` (receipt creation)
- Coverage > 70% for `internal/decision`

**Verification:**
```bash
go test -cover ./internal/decision/...
```

### Task 15: Add Permission Rule Expiry/Revocation (M1)

**Files to modify:**
- `internal/tools/permissions.go` — Add TTL
- `internal/tools/permissions.go` — Add revocation

**Acceptance Criteria:**
- Permission rules have optional TTL (default: session-only)
- `/permissions` command lists active rules
- `/permissions revoke <id>` removes a rule
- Rules expire after TTL

**Verification:**
```bash
go test -run TestPermission_Expiry -v ./internal/tools/...
```

### Task 16: Fix Unprotected Engine Fields (M2)

**Files to modify:**
- `internal/workflow/engine.go` — Add mutexes

**Acceptance Criteria:**
- `e.state.intentResult` protected by mutex
- `e.websiteTemplateDir` protected by mutex
- `e.sessionID` protected by mutex
- Race detector passes

**Verification:**
```bash
go test -race -count=1 -timeout=60s ./internal/workflow/...
```

### Task 17: Fix `LoadWorkflowState` File Lock (M3)

**Files to modify:**
- `pkg/session/manager.go` — Acquire lock

**Acceptance Criteria:**
- `LoadWorkflowState` acquires file lock before reading
- No stale data reads during concurrent writes
- Tests verify concurrent access safety

**Verification:**
```bash
go test -race -count=1 -timeout=60s ./pkg/session/...
```

---

## Wave 5: Verification & Cleanup

### Task 18: Run Full Verification Suite

**Acceptance Criteria:**
- `make check` passes (fmt → tidy → vet → lint → test)
- `make lint` passes (0 issues)
- `make test` passes (race-enabled)
- `go build ./...` succeeds
- Binary size < 55MB

**Verification:**
```bash
make check
make lint
make test
go build -o m31a ./cmd/m31a
du -h m31a
```

### Task 19: Update Documentation

**Files to modify:**
- `README.md` — Update security table
- `SECURITY.md` — Update threat model
- `AGENTS.md` — Update architecture rules

**Acceptance Criteria:**
- Security table reflects new defenses
- Threat model includes prompt injection
- Architecture rules enforce `pkg/` boundary

### Task 20: Create Release Audit Resolution Report

**Files to create:**
- `RELEASE_AUDIT_RESOLUTION.md`

**Acceptance Criteria:**
- All CRITICAL issues marked RESOLVED
- All HIGH issues marked RESOLVED
- All MEDIUM issues marked RESOLVED or DEFERRED
- Production readiness score updated
- Release recommendation: APPROVED FOR V1.0

---

## Threat Model

### STRIDE Analysis

| Threat | Mitigation |
|--------|------------|
| **Spoofing** | Permission system gates all tool execution |
| **Tampering** | Path containment prevents file writes outside workdir |
| **Repudiation** | Audit logging with secret redaction |
| **Information Disclosure** | Environment scrubbing removes API keys |
| **Denial of Service** | Rate limiting (token bucket) and concurrency control (semaphore) |
| **Elevation of Privilege** | Command blocklist, sandbox execution, subagent depth limit |

### ASVS Level

Level 1 (basic security controls) — appropriate for v1.0 release.

---

## Verification Strategy

### Automated Checks

1. **Architectural boundary:** `grep -r '"github.com/eshanized/M31A/internal' pkg/` returns empty
2. **Data races:** `go test -race ./...` passes
3. **Test timeouts:** All suites complete within 60s under `-short`
4. **Lint:** `make lint` passes (0 issues)
5. **Vet:** `go vet ./...` passes
6. **Build:** `go build ./...` succeeds

### Manual Checks

1. **Prompt injection:** Verify `<tool_output>` delimiters in system prompt
2. **Command blocklist:** Test `$()` and backtick bypass attempts
3. **Sandbox failure:** Verify visible warning on unsupported platforms

---

*Phase: 04-release-audit*
*Created: 2026-07-14*
