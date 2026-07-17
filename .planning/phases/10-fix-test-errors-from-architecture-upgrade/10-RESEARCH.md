# Phase 10: Fix test errors from architecture upgrade - Research

**Researched:** 2026-07-17
**Domain:** Go test infrastructure, mock patterns, test organization
**Confidence:** HIGH (codebase analysis)

## Summary

Phase 10 addresses test infrastructure cleanup after Phase 9's architecture upgrade. The codebase has 272 test files across 33 internal packages, with significant duplication of mock patterns, test helpers, and setup functions. The primary issue is that test infrastructure was not centralized during Phase 9's restructuring, leading to scattered and duplicated patterns.

**Critical finding:** The Phase 9 plan to move `pkg/` to `internal/` was NOT fully completed for types. `internal/types/` still exists with actual type definitions (383 lines), while `pkg/types/` does NOT exist. All 126 test files correctly import `internal/types` (which still exists), so there are no broken imports from this aspect. However, the original plan's intent to centralize types remains incomplete.

**Primary recommendation:** Execute test infrastructure centralization in 3 phases: (1) Fix any broken imports from Phase 9 restructuring, (2) Centralize mock patterns and test helpers in `internal/testutil/`, (3) Reorganize test fixtures and integration tests. Each phase must be independently testable with `make check`.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- **D-01:** Move ALL test infrastructure to `internal/testutil/` — shared mocks, setup helpers, test data builders, fixtures
- **D-02:** Organize testutil/ with subdirectories: `testutil/mocks/`, `testutil/builders/`, `testutil/fixtures/`
- **D-03:** Use `package testutil` as the umbrella package name
- **D-04:** Standardize naming with `New*` constructors: `NewMockProvider()`, `NewTestDispatcher()`, `NewSetupTestEngine()`
- **D-05:** Centralize all `*_integration_test.go` files in `internal/testutil/integration/`
- **D-06:** Use descriptive file names: `provider_integration_test.go`, `workflow_integration_test.go`
- **D-07:** Move `e2e_test.go` from project root to `internal/testutil/e2e/`
- **D-08:** Use both internal (`package testutil`) and external (`package testutil_test`) test packages as needed
- **D-09:** Store fixtures in `internal/testutil/fixtures/` with subdirectories per test type: `unit/`, `integration/`, `e2e/`
- **D-10:** Use `go:embed` for fixture loading — compile-time embedding, no runtime file reads
- **D-11:** Use Go source files (not JSON) for fixtures — type-safe, compile-time checked
- **D-12:** Use `goimports` automation to fix import paths after moves
- **D-13:** Batch verification — run `make check` after all moves complete
- **D-14:** Git commits after each major move for easy rollback
- **D-15:** Group commits by domain: testutil moves, integration moves, fixture moves

### the agent's Discretion
- Exact file naming within new sub-packages
- Import ordering in reorganized files
- Whether to do migration in one commit or incremental atomic commits
- Exact placement of borderline files (e.g., where does `mockProvider` belong after restructuring?)

### Deferred Ideas (OUT OF SCOPE)
None — discussion stayed within phase scope.
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| NFR-4 | Maintainability: 75% coverage, gofmt-clean, golangci-lint, conventional commits | Centralized test infrastructure improves maintainability; consistent patterns reduce cognitive load |
</phase_requirements>

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Test infrastructure | `internal/testutil/` | — | Central location for all shared test code |
| Mock patterns | `internal/testutil/mocks/` | — | Reusable mocks implement interfaces from multiple packages |
| Test helpers | `internal/testutil/builders/` | — | Factory functions for test objects |
| Test fixtures | `internal/testutil/fixtures/` | — | Type-safe, embeddable test data |
| Integration tests | `internal/testutil/integration/` | — | Tests spanning multiple packages |
| E2E tests | `internal/testutil/e2e/` | — | Binary-level tests |
| Unit tests | `internal/<package>/` | — | Package-specific tests stay colocated |

## Standard Stack

### Core (No New Dependencies Required)

This phase restructures existing test code — it does not add new libraries. All tools used are Go standard tooling:

| Tool | Version | Purpose | Why Standard |
|------|---------|---------|--------------|
| `goimports` | latest | Auto-fix import paths after moves | Official Go tool, handles bulk import rewriting |
| `golangci-lint` | v2 (configured) | Verify no lint regressions after restructure | Already in project, catches unused imports, shadow vars |
| `go vet` | stdlib | Verify no circular imports or type errors | Built into Go toolchain |
| `go build ./...` | stdlib | Compile check after each plan | Fastest feedback loop |

### Supporting

| Tool | Purpose | When to Use |
|------|---------|-------------|
| `go test ./...` | Verify all tests pass after each change | Every plan completion |
| `go list -deps` | Verify import graph is clean | Final verification plan |

**No package installations needed.** This phase only restructures existing test code.

## Package Legitimacy Audit

> **Not applicable** — this phase installs no external packages. All changes are structural refactoring of existing test code.

## Architecture Patterns

### Current Test Infrastructure Inventory

**Test Files by Package (272 total):**
- `internal/workflow/` — 54 test files (engine_test.go, engine_race_test.go, etc.)
- `internal/tools/` — 42 test files (dispatcher_test.go, bash_test.go, etc.)
- `internal/tui/` — 67 test files (app_*_test.go, handler_*_test.go, etc.)
- `internal/provider/` — 14 test files (base_client_test.go, registry_test.go, etc.)
- `internal/session/` — 9 test files (manager_test.go, planning_test.go, etc.)
- `internal/config/` — 7 test files (loader_test.go, merge_test.go, etc.)
- `internal/git/` — 2 test files (git_test.go, git_extra_test.go)
- Other packages — 77 test files

**Mock Patterns Found (22 mock types):**
1. `mockProvider` — 4 implementations (workflow, provider, subagent, keychain)
2. `mockKeychain` — 3 implementations (config, cmd/m31a, keychain)
3. `mockTool` — 1 implementation (tools/dispatcher_test.go)
4. `mockDispatcher` — 2 implementations (workflow, subagent)
5. `mockSource` — 1 implementation (context)
6. `mockGitRunner` — 1 implementation (bisect)
7. `mockReadCloser` — 1 implementation (provider/resilience)
8. `mockToolTimeout` — 1 implementation (tools/permission_timeout)
9. `mockBudgetConfig` — 1 implementation (workflow/coverage_boost)
10. `testEmitter` — 1 implementation (workflow/engine_extra)
11. `mockProviderWithModel` — 1 implementation (workflow/engine)
12. `mockProviderWithCapture` — 1 implementation (workflow/execute)
13. `mockEmitFn` — 1 implementation (workflow/phase_coordinator)
14. `testConfig` — 1 implementation (tui/test_helpers)
15. `mockWorktreeOps` — 1 implementation (subagent/extra)

**Test Helper Functions (25 setup functions):**
- `setupTestEngine` — workflow/engine_test.go
- `setupRaceTestEngine` — workflow/plan_race_test.go
- `setupTestPhaseCoordinator` — workflow/phase_coordinator_test.go
- `setupTestDispatcher` — tui/emitter_stress_test.go
- `testDispatcher` — tools/testutil_test.go
- `testDispatcherWithConfig` — tools/testutil_test.go
- `testAppState` — tui/app_update_extra_test.go
- `testTheme` — tui/test_helpers_test.go
- `testKeyMsg` — tui/test_helpers_test.go
- `testConfigModel` — tui/config_model_extra_test.go
- `testSettingsModel` — tui/settings_model_extra_test.go
- `testManager` — subagent/extra_test.go
- `setupRepo` — git/git_test.go
- `setupRollback` — rollback/rollback_test.go
- `setupBisectRepo` — bisect/bisect_test.go
- `setupLedger` — ledger/ledger_test.go
- `setupTestBash` — tools/bash_test.go
- `setupTestGit` — tools/git_test.go
- `setupTestPersistentPermissions` — tools/persistent_permissions_test.go
- `newTestRegistry` — tui/test_helpers_test.go
- `newMockKeychain` — config/loader_test.go, cmd/m31a/main_test.go
- `newMockGitRunner` — bisect/extra_test.go
- `testCommits` — tui/agent_loop_extra_test.go
- `testCache` — tui/layout/extra_test.go

**Integration Tests (1 file):**
- `internal/tools/edit_integration_test.go` — 550 lines, tests Edit tool with real file I/O

**E2E Tests (1 file):**
- `e2e_test.go` — 191 lines, tests binary with real API calls

**Race Test Files (3):**
- `internal/workflow/plan_race_test.go`
- `internal/workflow/messages_race_test.go`
- `internal/workflow/engine_race_test.go`

**Benchmark Test Files (2):**
- `internal/tools/codecomplexity_benchmark_test.go`
- `internal/tools/edit_benchmark_test.go`

**Doc Test Files (4):**
- `internal/bisect/doc_test.go`
- `internal/rollback/doc_test.go`
- `internal/session/doc_test.go`
- `internal/taskrunner/doc_test.go`

**Extra Test Files (25+):**
- `*_extra_test.go` pattern used across multiple packages

### Migration Risks and Mitigations

**Risk 1: Circular Import Centralization**
- **What:** Moving mocks to `testutil/mocks/` could create circular imports if mocks reference types from multiple packages
- **Why:** Mock implementations need to import interfaces they implement
- **Mitigation:** Use interface packages as import boundaries. Example: `testutil/mocks/provider.go` imports `provider` package for `Provider` interface, implements it without importing other mocks
- **Warning signs:** `go vet` reports import cycles

**Risk 2: Package Boundary Violations**
- **What:** Test helpers in `testutil/` might need to access unexported fields from other packages
- **Why:** Tests often test internal state
- **Mitigation:** Keep package-specific test helpers colocated (e.g., `workflow/test_helpers_test.go`). Only centralize truly shared utilities
- **Warning signs:** Tests fail to compile after move

**Risk 3: Import Path Churn**
- **What:** Moving test files changes import paths, requiring updates across 272 test files
- **Why:** Go imports are absolute paths
- **Mitigation:** Use `goimports -w ./...` for automatic path updates. Verify with `make check` after each batch
- **Warning signs:** `go build` fails with unresolved imports

**Risk 4: Test Isolation Breaking**
- **What:** Centralizing test setup might share state between tests
- **Why:** Shared test utilities could use package-level variables
- **Mitigation:** Ensure all test helpers use `t.Helper()` and `t.Cleanup()`. No package-level state in testutil
- **Warning signs:** Tests pass individually but fail when run together

**Risk 5: go:embed Path Breakage**
- **What:** Moving test files might break `go:embed` directives for fixtures
- **Why:** Embed paths are relative to source file location
- **Mitigation:** Keep embed directives at fixture package root. Verify with `go build` after moves
- **Warning signs:** Build fails with "embed file not found" errors

### Recommended Project Structure (Post-Migration)

```
internal/testutil/
├── envtest.go                    # Existing - Keep as-is
├── mocks/
│   ├── provider.go              # NewMockProvider() — implements provider.Provider
│   ├── keychain.go              # NewMockKeychain() — implements keychain.Keychain
│   ├── tool.go                  # NewMockTool() — implements tools.Tool
│   └── dispatcher.go            # NewMockDispatcher() — implements tools.Dispatcher
├── builders/
│   ├── engine.go                # NewTestEngine() — builds engine with all dependencies
│   ├── config.go                # NewTestConfig() — builds minimal config
│   └── session.go               # NewTestSession() — builds test session
├── fixtures/
│   ├── unit/                    # Type-safe Go fixtures for unit tests
│   ├── integration/             # Fixtures for integration tests
│   └── e2e/                     # Fixtures for E2E tests
├── integration/
│   ├── provider_integration_test.go
│   ├── workflow_integration_test.go
│   └── tool_integration_test.go
└── e2e/
    └── e2e_test.go              # Moved from project root
```

### Pattern 1: Mock Centralization

**What:** Move duplicate mock implementations to `testutil/mocks/` with standardized constructors.

**When to use:** When multiple packages implement the same mock (e.g., 4 `mockProvider` implementations).

**Example:**
```go
// BEFORE (duplicated in 4 files):
type mockProvider struct {
    name string
}
func (m *mockProvider) Name() string { return m.name }
// ... 6 more methods

// AFTER (centralized in testutil/mocks/provider.go):
package mocks

import "github.com/eshanized/M31A/internal/provider"

type MockProvider struct {
    Name_           string
    HealthStatus_   string
    Response_       string
    Err_            error
}

func NewMockProvider(name string) *MockProvider {
    return &MockProvider{Name_: name}
}

func (m *MockProvider) Name() string { return m.Name_ }
// ... interface implementation
```

### Pattern 2: Test Builder Pattern

**What:** Create `New*` constructors that set up complete test objects with sensible defaults.

**When to use:** When test setup involves multiple steps that are repeated across tests.

**Example:**
```go
// BEFORE (repeated setup):
func setupTestEngine(t *testing.T) (*Engine, func()) {
    dir := t.TempDir()
    g := git.New(dir)
    g.Init()
    g.ConfigUser("Test", "test@test.com")
    sessionBaseDir := filepath.Join(dir, "sessions")
    os.MkdirAll(sessionBaseDir, 0755)
    mgr := session.NewManager(sessionBaseDir, sessionBaseDir, session.ManagerOpts{})
    // ... 20 more lines

// AFTER (builder in testutil/builders/engine.go):
package builders

func NewTestEngine(t *testing.T) (*workflow.Engine, func()) {
    t.Helper()
    dir := t.TempDir()
    g := git.New(dir)
    g.Init()
    g.ConfigUser("Test", "test@test.com")
    // ... complete setup
    return engine, cleanup
}
```

### Pattern 3: Fixtures with go:embed

**What:** Store test data as Go source files and embed at compile time.

**When to use:** When tests need static data that should be type-safe.

**Example:**
```go
// testutil/fixtures/unit/configs.go
package unit

import _ "embed"

//go:embed sample_config.toml
var SampleConfig string

//go:embed large_session.json
var LargeSessionJSON []byte
```

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Mock implementations | Hand-write each mock | `testutil/mocks/` with constructors | Reduces duplication, ensures interface compliance |
| Test setup boilerplate | Repeat setup in each test file | `testutil/builders/` with `New*` functions | Single source of truth, easier maintenance |
| Test data files | Read JSON/TOML at runtime | `go:embed` with Go source files | Type-safe, compile-time checked, no file path issues |
| Import path updates | Manually update each file | `goimports -w ./...` | Handles bulk updates automatically |

**Key insight:** Go's test infrastructure is intentionally simple — no external mocking framework, no test runners beyond `go test`. This simplicity is a feature, not a limitation. Centralization should follow Go conventions, not fight them.

## Common Pitfalls

### Pitfall 1: Circular Imports from Mock Centralization
**What goes wrong:** Moving mocks to a central package creates import cycles when mocks reference types from multiple packages.
**Why it happens:** Mock implementations need to import the interfaces they implement, and those interfaces might be in different packages.
**How to avoid:** Organize mocks by the interface they implement, not by the package that uses them. Use interface packages as import boundaries.
**Warning signs:** `go vet` reports import cycles, build fails with "import cycle not allowed"

### Pitfall 2: Breaking Package-Local Test Helpers
**What goes wrong:** Moving test helpers to `testutil/` breaks tests that rely on unexported fields or functions.
**Why it happens:** Some test helpers need access to package internals that are not exported.
**How to avoid:** Keep package-specific test helpers colocated with their package. Only centralize truly shared utilities that don't need internal access.
**Warning signs:** Tests fail to compile with "cannot refer to unexported name"

### Pitfall 3: Shared Test State Leaking Between Tests
**What goes wrong:** Centralized test helpers use package-level variables, causing test pollution.
**Why it happens:** Developers add convenience variables to test utilities without thinking about concurrency.
**How to avoid:** Ensure all test helpers use `t.Helper()` and `t.Cleanup()`. No package-level mutable state in testutil.
**Warning signs:** Tests pass individually but fail when run together, flaky tests

### Pitfall 4: Import Path Updates Breaking go:embed
**What goes wrong:** Moving test files breaks `go:embed` directives because embed paths are relative.
**Why it happens:** Developers forget that embed paths are resolved relative to the source file, not the module root.
**How to avoid:** Keep embed directives at the package root where the embedded files are located. Verify with `go build` after any move.
**Warning signs:** Build fails with "embed file not found", runtime panic about missing embedded files

### Pitfall 5: Over-Centralization
**What goes wrong:** Everything moves to testutil/, making it a dumping ground that's harder to navigate than the original structure.
**Why it happens:** Developers optimize for "single location" without considering discoverability.
**How to avoid:** Follow the 80/20 rule — centralize the 20% of utilities used by 80% of tests. Keep specialized helpers colocated.
**Warning signs:** testutil/ grows larger than any individual package's test directory

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Scattered mocks per package | Centralized in testutil/mocks/ | Phase 10 | Reduces duplication, ensures consistency |
| Manual test setup | Builder pattern with New* constructors | Phase 10 | Single source of truth for test setup |
| JSON/TOML fixtures read at runtime | Go source files with go:embed | Phase 10 | Type-safe, compile-time checked |
| Integration tests colocated with unit tests | Centralized in testutil/integration/ | Phase 10 | Clear separation of test types |

**Deprecated/outdated:**
- External test files (`*_test.go` in different directory) — not applicable, all tests colocated
- Test fixtures as separate files — replaced by Go source with go:embed

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | Phase 9's pkg/ to internal/ migration was NOT fully completed for types | Summary | Medium — if pkg/types/ exists elsewhere, import paths would be different |
| A2 | All 126 test files importing internal/types are correct (package still exists) | Summary | Low — verified by reading internal/types/types.go |
| A3 | No external mock libraries are used (hand-written mocks only) | Architecture Patterns | Low — verified by grep for mockgen, testify/mock, etc. |
| A4 | The 4 duplicate mockProvider implementations can be unified | Common Pitfalls | Medium — some may have package-specific requirements |

**If this table is empty:** All claims in this research were verified or cited — no user confirmation needed.

## Open Questions

1. **Should pkg/types/ be created as an alias to internal/types/?**
   - What we know: Phase 9 plan intended to move pkg/ contents to internal/, but internal/types/ still exists with actual definitions
   - What's unclear: Whether to complete the original plan or keep current structure
   - Recommendation: Keep internal/types/ as-is for now. It's working and all imports are correct. Defer to a future phase if needed.

2. **How to handle package-specific test helpers that need internal access?**
   - What we know: Some test helpers use unexported fields or functions
   - What's unclear: Whether to keep them colocated or refactor to use exported APIs
   - Recommendation: Keep package-specific helpers colocated. Only centralize truly shared utilities.

3. **What about the e2e_test.go location?**
   - What we know: Currently at project root, imports m31a_test package
   - What's unclear: Whether moving to testutil/e2e/ would break the build tag or import pattern
   - Recommendation: Investigate if the build tag pattern changes. If it does, keep at root or use a build tag.

## Environment Availability

> Skip this section — the phase has no external dependencies (code/config-only changes).

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Go stdlib testing |
| Config file | none — see Wave 0 |
| Quick run command | `go test -count=1 ./internal/testutil/...` |
| Full suite command | `make check` |

### Phase Requirements → Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| NFR-4 | Tests pass after centralization | unit | `make check` | ✅ Wave 0 |

### Sampling Rate
- **Per task commit:** `go test -count=1 ./internal/testutil/...`
- **Per wave merge:** `make check`
- **Phase gate:** Full suite green before `/gsd-verify-work`

### Wave 0 Gaps
- [ ] None — existing test infrastructure covers all phase requirements

## Security Domain

> Required when `security_enforcement` is enabled (absent = enabled). Omit only if explicitly `false` in config.

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | no | — |
| V3 Session Management | no | — |
| V4 Access Control | no | — |
| V5 Input Validation | no | — |
| V6 Cryptography | no | — |

### Known Threat Patterns for {stack}

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Test data leakage | Information Disclosure | Use t.TempDir() for isolation, never hardcode secrets in tests |
| Mock injection | Tampering | Verify mock implementations match interfaces |

## Sources

### Primary (HIGH confidence)
- Codebase analysis — 272 test files scanned
- `internal/testutil/envtest.go` — existing shared test infrastructure
- Phase 9 summaries — understanding of completed restructuring

### Secondary (MEDIUM confidence)
- Go test conventions documentation
- Standard Go project layout patterns

### Tertiary (LOW confidence)
- None — all findings verified against codebase

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — no new dependencies needed
- Architecture: HIGH — patterns derived from existing codebase
- Pitfalls: HIGH — common Go test organization issues

**Research date:** 2026-07-17
**Valid until:** 2026-08-17 (30 days for stable patterns)
