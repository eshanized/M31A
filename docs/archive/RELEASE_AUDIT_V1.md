# M31A Release Audit Report — v1.0 Readiness

**Auditor:** Principal Software Auditor (Independent)
**Date:** July 14, 2026
**Scope:** Full codebase review — security, concurrency, architecture, testing, performance, DX, maintainability
**Commit:** `ed64d51b`

---

## Executive Summary

M31A is a sophisticated terminal AI coding agent with a seven-phase workflow engine, three provider integrations, 18 built-in tools, parallel subagents, and a 33-screen Bubble Tea TUI. The ambition is impressive. The execution is mixed.

The project has **real security hardening** (SSRF protection, path containment, keychain integration, environment scrubbing) and **genuine engineering** (topological task sorting, auto-fallback providers, tree-sitter code intelligence). However, it also has **critical architectural violations** (the `pkg/` boundary is comprehensively broken), **a data race in the workflow engine** (provider assignment without synchronization), **tests that timeout** (the `internal/tools` and `pkg/bisect` test suites hang under `-short`), and **a 52MB static binary** that is unusually large.

The most damaging finding: **`pkg/` imports `internal/` across 10 packages with 50+ import lines**, completely defeating the Go module's intended package boundary. This is a CRITICAL architectural violation that would block any attempt to extract reusable packages.

**Release verdict: NOT READY FOR V1.0** — see blockers below.

---

## Release Score: 6.5 / 10

| Dimension | Score | Notes |
|-----------|-------|-------|
| Security | 7/10 | Strong SSRF/path/keychain hardening. Command blocklist bypassable via `$()`. Prompt injection unprotected. |
| Concurrency | 6/10 | Bubble Tea model correct. Data race on `e.provider` in workflow engine. Several unprotected fields. |
| Architecture | 4/10 | `pkg/` → `internal/` violation (50+ imports). God objects (engine.go 1688L). Magic strings. |
| Testing | 5/10 | All passing tests pass. But `internal/tools` and `pkg/bisect` timeout. `cmd/m31a` and `internal/decision` have 0% coverage. |
| Performance | 7/10 | Good streaming optimization. Double file reads in codeintel. Ledger O(N) scan. tiktoken re-init per invocation. |
| DX | 7/10 | Excellent Makefile. Good docs. Config externalized. 52MB binary is large. |
| Maintainability | 6/10 | 142 `fmt.Errorf` without `%w`. Config sprawl (549 lines). Over-exported workflow messages. |
| Release infrastructure | 8/10 | Goreleaser, cross-compile, install script, CI badges all present. |

---

## Production Readiness

### What Works Well

1. **Path containment is consistently applied** — Every file tool (FileRead, FileWrite, FileDelete, FileMove, Glob, Grep) resolves symlinks and checks `ContainedInWorkDir` before execution.
2. **SSRF protection is comprehensive** — DNS caching prevents TOCTOU rebinding, all resolved IPs checked (not just first), post-connect verification, redirect SSRF protection.
3. **Keychain never writes to disk** — Uses D-Bus Secret Service or `pass` CLI. Service name validation prevents injection (`^[a-z0-9][a-z0-9-]*$`).
4. **Rate limiting is real** — Token bucket (20 burst / 10 sustained) for normal tools, stricter for dangerous tools (5 burst / 2 sustained).
5. **Bubble Tea model is correct** — All state mutations go through `Update()`. Channel emitter uses bounded retry. No goroutine-triggered state mutations.
6. **Workflow engine has quality gates** — Plan checker, coverage gates, security heuristics, loop detection, pre-flight validation.
7. **Environment scrubbing** — API keys removed from bash subprocess environment.
8. **Concurrency control** — Max 8 concurrent tool executions via semaphore with `ctx.Done()` escape.
9. **Atomic file writes** — Session persistence uses temp-file-then-rename for crash safety.
10. **Zero telemetry** — No phone-home, no analytics, no data collection.

---

## Remaining Critical Issues

### C1: `pkg/` imports `internal/` — Architectural Boundary Violation

**Severity: CRITICAL**

10 `pkg/` packages import `internal/` packages with 50+ import lines:

| Package | Imports From `internal/` |
|---------|-------------------------|
| `pkg/taskrunner` | `internal/errors`, `internal/types` |
| `pkg/arbitrage` | `internal/types` |
| `pkg/bisect` | `internal/errors`, `internal/git` |
| `pkg/ledger` | `internal/errors`, `internal/fileutil`, `internal/types` |
| `pkg/rollback` | `internal/git`, `internal/types` |
| `pkg/autodream` | `internal/tokens`, `internal/types` |
| `pkg/session` | `internal/errors`, `internal/fileutil`, `internal/types` |
| `pkg/metrics` | `internal/types` |
| `pkg/compaction` | `internal/provider`, `internal/tokens`, `internal/types` |
| `pkg/narrative` | `internal/workflow`, `internal/types` |

**Impact:** `pkg/` cannot be extracted into separate modules. Any consumer importing `pkg/session` transitively pulls in `internal/`. This violates the stated architecture and blocks the "importable public packages" design goal.

**Fix:** Move shared types (`Message`, `Task`, `WorkflowPhase`, etc.) from `internal/types/` to `pkg/types/` or a new `pkg/domain/` package. For `pkg/narrative`, replace the type switch over `workflow.*Msg` types with an interface-based approach.

---

### C2: Test Suites Timeout Under `-short`

**Severity: CRITICAL**

Two critical test suites timeout or hang:

- `internal/tools` — Hangs when running all tests (DNS lookup stuck in goroutine during WebSearch/WebFetch tests). Timeout at 30s under `-short`.
- `pkg/bisect` — Times out at 90s under `-short`. The `TestBisect_Successful` test creates a real git repo and runs actual `git bisect`, which is slow.

**Impact:** CI pipelines must either skip these packages or set very long timeouts. Regressions in tools and bisect go undetected.

**Fix:** Mock DNS resolution for WebSearch/WebFetch tests. Make bisect tests use mocks instead of real git operations for the happy path.

---

### C3: Data Race on `e.provider` in Workflow Engine

**Severity: CRITICAL**

`engine.go:789` (`SetModel`) writes `e.provider` without any synchronization. Meanwhile, `e.provider` is read by `streamLLM*`, `preflightContextCheck`, and `proactiveCompactCheck` from the workflow goroutine. If `SetModel` is called from the TUI while the workflow goroutine reads `e.provider`, this is a data race.

**Impact:** Corrupted provider reference could route requests to wrong provider, leak API keys, or crash. The `modelID` is protected by `modelIDMu`, but `provider` is not.

**Fix:** Protect `e.provider` with `modelIDMu` or use `atomic.Value`. Swap both `modelID` and `provider` atomically.

---

## Remaining High Issues

### H1: Dangerous Command Blocklist Bypassable

**File:** `internal/tools/bash.go:462-511`

The blocklist uses substring matching. Multiple bypass vectors:

1. **`$()` not blocked** — `containsVariableExpansion()` checks `$VARIABLE`, `${...}`, `$((...)` but NOT `$(cmd)`. `$(rm -rf /)` passes detection.
2. **Backtick substitution not blocked** — `` `rm -rf /` `` passes detection.
3. **Missing patterns** — `mkfs.ext4`, `fdisk`, `wipefs`, `shred`, `nc -l`, `ncat -l` absent.
4. **Newline chaining** — `rm -rf /\nsleep 1` passes `validateCommandSyntax()`.

**Fix:** Add `$(...)`, backtick, and `;`/`&&`/`||` chaining awareness. Expand the blocklist. Consider an allowlist approach for production.

---

### H2: Prompt Injection — No Output Sanitization

**File:** `internal/tools/dispatcher.go:295-327`, `internal/tools/subagent/loop.go:243-248`

Tool outputs are passed directly into LLM conversation history as raw strings. A malicious file containing `"Ignore all previous instructions..."` would be interpreted by the LLM as instructions. The subagent system prompt has injection defenses, but the parent agent has none.

**Fix:** Wrap tool outputs in clear delimiters (`<tool_output>...</tool_output>`) and add instruction in the system prompt that content within these delimiters is data, not instructions.

---

### H3: Sandbox Failure Silently Proceeds

**File:** `internal/tools/bash.go:157-159`

When `applyBashSandbox()` fails, the code logs a warning but proceeds without OS-level sandboxing. On Linux kernels <5.13 (Landlock unavailable) or non-Linux platforms, bash commands run with zero filesystem restrictions.

**Fix:** At minimum, surface the degraded security mode to the user visibly. Consider refusing execution on platforms without sandbox support.

---

### H4: Subagent Default Isolation Shares Parent WorkDir

**File:** `internal/tools/subagent/manager.go:188-199`

When `Isolation == IsolationDefault`, the subagent shares the parent's working directory directly. A subagent can read, modify, or delete any file in the parent's directory including `.m31a.json` config and `.env` files.

**Fix:** Default to `IsolationWorktree` instead of `IsolationDefault`.

---

### H5: 142 `fmt.Errorf` Without `%w` — Broken Error Chains

Throughout `pkg/` and `internal/`, `fmt.Errorf` calls use `%s` or `%v` instead of `%w`:

| File | Line | Example |
|------|------|---------|
| `pkg/taskrunner/runner.go` | 106 | `fmt.Errorf("task %d: references non-existent dependency %d", ...)` |
| `pkg/arbitrage/arbitrage.go` | 139 | `fmt.Errorf("no models available")` |
| `pkg/bisect/bisect.go` | 129 | `fmt.Errorf("could not parse offending commit from bisect log")` |
| `internal/provider/openrouter/client.go` | 103, 155 | `fmt.Errorf("models fetch returned status %d")` |
| `internal/provider/nvidia/client.go` | 88, 150 | Same pattern |

**Impact:** Callers cannot programmatically distinguish error types via `errors.Is()` / `errors.As()`. The entire error taxonomy is broken.

---

### H6: God Objects — Oversized Files

| File | Lines | Concern |
|------|-------|---------|
| `internal/workflow/engine.go` | 1,688 | 50+ methods. Session state, phase dispatch, LLM streaming, pause/resume, compaction, context building, cost tracking, git config, checkpoint management. |
| `internal/tui/sidebar_model.go` | 1,652 | 80+ methods. Files, tasks, tools, token burn, phases, narrative, focus, layout. |
| `internal/tui/app_view.go` | 1,266 | 40+ `render*Content()` methods. |
| `internal/config/loader.go` | 1,154 | Config loading, validation, keychain, file watching, dot-env, variable substitution. |
| `internal/workflow/execute.go` | 1,152 | Task execution, parallel dispatch, self-healing, error classification. |
| `internal/tools/webfetch.go` | 1,147 | SSRF protection, DNS caching, redirects, retries. |

---

### H7: Hardcoded Provider Name Strings (50+ Locations)

`"openrouter"`, `"zen"`, `"nvidia"` appear as string literals in 50+ locations across 8+ packages. Renaming a provider requires a codebase-wide search. No single source of truth.

**Fix:** Define constants in `internal/types/`:
```go
const (
    ProviderOpenRouter = "openrouter"
    ProviderZen        = "zen"
    ProviderNvidia     = "nvidia"
)
```

---

### H8: Tests Timeout in CI

The `internal/tools` test suite hangs due to DNS lookups in WebSearch/WebFetch tests. Under `-short -timeout=30s`, the suite fails. The `pkg/bisect` suite times out at 90s due to real git operations.

**Impact:** CI pipelines cannot reliably run the full test suite.

---

## Medium Issues

### M1: Permission Rule Persistence Has No Expiry or Revocation

**File:** `internal/tools/permissions.go:415-437`

When a user selects "remember" on a permission prompt, the rule is persisted as a permanent allow. No TTL, no UI mechanism to revoke, no scope limitation. An accidental "remember" on a dangerous command creates a permanent bypass.

---

### M2: `e.state.intentResult`, `e.websiteTemplateDir`, `e.sessionID` Written Without Lock

**Files:** `engine.go:380`, `engine.go:584`, `engine.go:930`

Three engine fields are written without synchronization while being read from other goroutines. Practical risk is low (these are typically set before workflow starts), but they are technically data races.

---

### M3: `LoadWorkflowState` Skips File Lock

**File:** `pkg/session/manager.go:316-326`

`LoadWorkflowState` calls `loadSessionMetadata()` which does not acquire the file lock (unlike `LoadSession`). Could read stale data during concurrent writes.

---

### M4: Double File Reads in Code Intelligence

**Files:** `internal/codeintel/cache.go:33-39` + `internal/codeintel/graph.go:266`

During `BuildGraph`, files are read for parsing, then read again by `fileHash()` for cache invalidation. Each file is read twice per index build.

---

### M5: Ledger Linear Scan on Append

**File:** `pkg/ledger/ledger.go:138-142`

`Append()` does an O(N) linear scan of all entries for duplicate detection. Use a `map[string]struct{}` for O(1) lookup.

---

### M6: Ledger Entries/EntriesFiltered Sorts on Every Call

**File:** `pkg/ledger/ledger.go:192-246`

Every call to `Entries()` copies the entire slice and sorts newest-first. Called from the TUI dashboard which may re-render on tick.

---

### M7: tiktoken Re-initialized Per Agent Loop Invocation

**File:** `internal/tokens/estimator.go:122-128`

`NewEstimator` loads BPE merge files from disk. A new `Estimator` is created on every `AgentLoop` invocation (~50-200ms per model).

---

### M8: Tree-sitter Parser Created Per File

**File:** `internal/codeintel/parser.go:112-113`

A new `Parser` object is created for every file parsed. Tree-sitter parsers are expensive to initialize.

---

### M9: `pkg/narrative` Imports `internal/workflow` — Cross-Layer Coupling

**File:** `pkg/narrative/bridge.go:6`

300-line type switch over 20+ `workflow.*Msg` types. Adding/removing a message type in `workflow` breaks `narrative`. Package is not independently testable.

---

### M10: Config Struct Sprawl

**File:** `internal/config/types.go` — 549 lines, 17 top-level sections, `UIConfig` alone has ~80 fields, `FeaturesConfig` has ~40 fields.

---

### M11: 36 `time.Sleep` Calls in Tests — Flaky Test Risk

Provider tests have 5.5-second sleeps. Emitter stress tests use 50-200ms sleeps. These are fragile on slow CI.

---

### M12: Binary Size — 52MB

The static binary is 52MB. Tree-sitter grammars (pure Go via `gotreesitter`) are the primary bloat driver. Consider UPX compression or selective grammar inclusion.

---

## Low Issues

| # | Issue | File | Impact |
|---|-------|------|--------|
| L1 | `scrubEnvironment` misses non-standard secret env vars | `bash_sandbox_linux.go:172-248` | Low — requires custom var names |
| L2 | macOS sandbox-exec allows unrestricted network access | `bash_sandbox_darwin.go:60` | Low — sandbox-exec is deprecated by Apple |
| L3 | `RedactSecrets` misses `ghp_`, `AKIA`, `sk-or-v1-` key formats | `internal/logging/audit.go:11-31` | Low — runtime logging only |
| L4 | Grep path containment uses string prefix (not filepath-aware) | `internal/tools/grep.go:143` | Very low |
| L5 | `OnTaskUpdate` callback invoked while holding write lock | `pkg/taskrunner/runner.go:283,291` | Latent deadlock risk |
| L6 | Unbuffered channels for `pauseCh`, `resumeCh` | `internal/workflow/engine.go` | Potential deadlock under load |
| L7 | `dispatcher.respondQuestion` blocks 30s on shared channel | `internal/tools/dispatcher.go:478-481` | 30s stall |
| L8 | SSE scanner allocates 1MB buffer per stream | `internal/provider/sse.go:30` | GC pressure |
| L9 | `messagesToWire` allocates map per message | `internal/provider/common.go:109-134` | Proportional to message count |
| L10 | `buildGraph` reads full files before truncation | `internal/codeintel/graph.go:266-268` | OOM risk on large generated files |

---

## Security Review

### Strong Points

- **Path containment** consistently applied across all file tools with symlink resolution
- **SSRF protection** comprehensive: DNS caching, all IPs checked, post-connect verification
- **Keychain** never writes to disk; service name validation prevents injection
- **Rate limiting** real token bucket with per-risk limits
- **Concurrency control** via semaphore with `ctx.Done()` escape
- **Environment scrubbing** removes API keys from subprocesses
- **Subagent depth limit** (MaxAgentDepth=2) prevents recursive fan-out
- **ReDoS protection** in grep via pattern analysis
- **Atomic file writes** with backup for FileWrite

### Weak Points

- **Command blocklist** bypassable via `$()`, backticks, newlines
- **Prompt injection** unprotected in parent agent tool output path
- **Sandbox failure** silently proceeds without filesystem restrictions
- **Subagent default isolation** shares parent workDir
- **Permission rules** permanent with no expiry or revocation
- **Environment scrubbing** misses non-standard secret variable names

---

## Performance Review

### Optimizations Already Present

- Streaming viewport virtualization and 10fps render rate
- Resize debounce in TUI
- `singleflight.Group` for concurrent model cache refreshes
- `sync.RWMutex` for model cache and ledger
- Bounded channel emitter with retry/drop
- Token bucket rate limiting (no busy-wait)

### Bottlenecks Found

- **Double file reads** in codeintel (hash + parse)
- **Ledger O(N) scan** on append
- **Ledger sort-on-every-call** for entries
- **tiktoken re-init** per agent loop (~50-200ms)
- **Tree-sitter parser re-init** per file
- **EstimateMessages called on hot paths** (up to 50x per agent loop)
- **String concatenation in layout render** (multiple allocations per tick)

---

## Architecture Review

### Strengths

- Clean `cmd/` → `internal/` → `pkg/` layering (intent is correct)
- Bubble Tea Elm architecture properly followed
- Provider abstraction with auto-fallback
- Workflow engine with seven-phase state machine
- Tool dispatcher with permission/rate-limiting/concurrency layers

### Violations

- **`pkg/` → `internal/`** is comprehensively broken (10 packages, 50+ imports)
- **God objects** — `engine.go` (1688L) doing too many things
- **Magic strings** — provider names hardcoded 50+ times
- **Cross-layer coupling** — `pkg/narrative` imports `internal/workflow`
- **Config sprawl** — 549 lines of config structs with 17 sections
- **Over-exported types** — workflow message types exported unnecessarily

---

## Developer Experience Review

### Strengths

- Excellent Makefile with 30+ targets
- Good documentation: README, CONTRIBUTING, SECURITY, TESTING, KEYBINDINGS, CONFIG, TOOLS, ARCHITECTURE
- Cross-compilation for 6 platform/arch combos
- Install script with checksum verification
- Goreleaser configuration
- `go vet` passes clean
- `golangci-lint` configured with appropriate linter set

### Weaknesses

- 52MB binary is large for a terminal tool
- `go 1.25.0` in `go.mod` — bleeding edge, may confuse users on stable Go
- No `--version` flag documented in README (only in e2e test)
- Config file path `~/.m31a/config.toml` not created automatically on first run
- Test suite timeouts make CI unreliable

---

## Workflow Review

- Seven-phase workflow (Initialize → Discuss → Plan → Execute → Verify → Runtime → Ship) is well-implemented
- Four workflow modes (auto, full, fast, direct) provide appropriate flexibility
- Quality gates (plan checker, coverage gates, security heuristics, loop detection) are real and functional
- Chunked plan generation handles large plans
- Self-healing in Verify phase (2 retries) is practical
- **Concern:** Workflow engine is the most complex component (engine.go + execute.go = 2840 lines combined) — decomposition is incomplete

---

## Testing Review

- **All passing tests pass** — no flaky failures observed
- **`go vet` passes clean** — no static analysis warnings
- **Race detector** — `go test -race` times out (5 min), suggesting heavy test suites or potential hangs
- **Coverage gaps:**
  - `cmd/m31a`: 0% — entry point completely untested
  - `internal/decision`: 0% — audit/redact logic untested
  - `internal/workflow/prompts`: 0% — prompt loading untested
  - `internal/tui`: 29.8% — most UI logic untested
- **Timeout issues:** `internal/tools` and `pkg/bisect` timeout under `-short`
- **Network-dependent tests** skip in CI without API keys — real integration untested
- **No negative test cases** for nil inputs, empty slices in critical paths

---

## Documentation Review

- README is comprehensive and well-structured
- Architecture diagram (Mermaid) is accurate
- Security table is detailed
- Config reference is complete
- **Concern:** No CHANGELOG.md linked from README releases
- **Concern:** Wiki links may not exist or be maintained
- **Concern:** Some docs reference features not yet implemented (Ghost mode, Picture-in-picture)

---

## Long-Term Maintainability

### Positive Indicators

- Conventional commits (feat:, fix:, docs:, etc.)
- MIT license
- Contributing guidelines
- Security policy
- Code of conduct
- CI badges
- Go Report Card

### Risk Factors

- **God objects** (engine.go, sidebar_model.go) make onboarding difficult
- **Config sprawl** makes feature addition risky
- **`pkg/` → `internal/` violation** blocks package extraction
- **142 broken error chains** make debugging harder
- **50+ magic strings** make refactoring risky
- **No package-level interfaces** — tight coupling between layers

---

## Technical Debt

| Category | Count | Severity |
|----------|-------|----------|
| `pkg/` → `internal/` imports | 50+ import lines | CRITICAL |
| `fmt.Errorf` without `%w` | 142 instances | HIGH |
| Hardcoded provider strings | 50+ locations | HIGH |
| God objects (>1000 lines) | 6 files | HIGH |
| Test timeouts | 2 suites | CRITICAL |
| `time.Sleep` in tests | 36 instances | MEDIUM |
| Unprotected engine fields | 4 fields | MEDIUM |
| Config struct sprawl | 549 lines | MEDIUM |
| Missing secret patterns in redaction | 5+ key formats | LOW |

---

## Release Recommendation

**NOT READY FOR V1.0**

### Blockers (Must Fix Before Release)

1. **Fix `pkg/` → `internal/` architectural violation** — Move shared types to `pkg/` or define interfaces. This is the single most important structural fix.
2. **Fix data race on `e.provider`** in `engine.go:789` — Protect with mutex. This can cause real runtime corruption.
3. **Fix test suite timeouts** — `internal/tools` and `pkg/bisect` must complete within 60s under `-short`. CI cannot reliably run the test suite otherwise.
4. **Expand command blocklist** — Add `$()`, backtick detection, and missing destructive patterns. The current blocklist is trivially bypassable.

### Should Fix (Strongly Recommended)

5. Add prompt injection defense (wrap tool outputs in delimiters).
6. Default subagent isolation to `IsolationWorktree`.
7. Add expiry/revocation mechanism for persisted permission rules.
8. Fix 142 `fmt.Errorf` calls to use `%w` for error chain preservation.
9. Define provider name constants (50+ magic strings → single source of truth).
10. Add `cmd/m31a` and `internal/decision` test coverage.

### Can Defer (Post-1.0)

11. Decompose god objects (engine.go, sidebar_model.go).
12. Optimize codeintel double file reads.
13. Cache tiktoken tokenizer per model.
14. Add ledger dedup map and sort caching.
15. Reduce binary size (UPX or selective tree-sitter grammars).
16. Split config structs into sub-structs.
17. Replace `time.Sleep` in tests with event-based synchronization.

---

*This audit was conducted independently. All findings reference concrete file locations and line numbers. No assumptions were made about previous reports or fixes.*
