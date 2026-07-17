# Phase 10: Fix test errors from architecture upgrade - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-07-17
**Phase:** 10-fix-test-errors-from-architecture-upgrade
**Areas discussed:** What goes in testutil/, Integration test location, Test fixture files, Import path migration

---

## What goes in testutil/

| Option | Description | Selected |
|--------|-------------|----------|
| Shared mocks + setup helpers | mockProvider, testDispatcher, setupTestEngine — things used by 2+ packages | |
| Only generic utilities | Keep mocks in their package tests, only move generic utilities | |
| All test infrastructure | Move everything test-related: mocks, helpers, fixtures, data builders | ✓ |

**User's choice:** All test infrastructure
**Notes:** User wants comprehensive testutil/ with all test helpers centralized

---

## How should testutil/ be organized internally?

| Option | Description | Selected |
|--------|-------------|----------|
| Flat files in testutil/ | One file per domain: mocks.go, builders.go, fixtures.go | |
| Subdirectories in testutil/ | testutil/mocks/, testutil/builders/, testutil/fixtures/ | ✓ |
| Domain-based subdirs | Follow existing pattern from internal/provider/mock/ | |

**User's choice:** Subdirectories in testutil/
**Notes:** Structured organization with clear separation of concerns

---

## How should testutil/ packages be named?

| Option | Description | Selected |
|--------|-------------|----------|
| package testutil | import testutil directly: testutil.NewMockProvider() | ✓ |
| package per subdir | import subdirs: testutil/mocks.NewProvider() | |
| Umbrella package | testutil package re-exports subdirs: testutil.MockProvider() | |

**User's choice:** package testutil
**Notes:** Simple import path, direct access to all utilities

---

## How should test utilities be named in testutil/?

| Option | Description | Selected |
|--------|-------------|----------|
| Keep current names | Keep descriptive names: mockProvider, testDispatcher | |
| New* constructors | Standardize: NewMockProvider, NewTestDispatcher | ✓ |
| Group by type | Group by type: Mocks.Provider(), Helpers.Dispatcher() | |

**User's choice:** New* constructors
**Notes:** Go convention for constructors, clear and consistent

---

## Where should integration tests live?

| Option | Description | Selected |
|--------|-------------|----------|
| Keep colocated | Keep *_integration_test.go next to source files they test | |
| Centralize in testutil/ | Move all integration tests to internal/testutil/integration/ | ✓ |
| Separate integration pkg | Create internal/integration/ for cross-package integration tests only | |

**User's choice:** Centralize in testutil/
**Notes:** All test infrastructure in one place, easier to find and maintain

---

## How should integration test files be named in testutil/?

| Option | Description | Selected |
|--------|-------------|----------|
| Descriptive names | integration_test.go, e2e_test.go, provider_integration_test.go | ✓ |
| Per-package files | One test file per package being tested | |
| Single file, subtests | Single integration_test.go with subtests for each package | |

**User's choice:** Descriptive names
**Notes:** Clear naming indicates what each test file covers

---

## Where should e2e_test.go live?

| Option | Description | Selected |
|--------|-------------|----------|
| Keep at root | Keep e2e_test.go at project root — tests the binary | |
| Move to testutil/e2e/ | Move to testutil/e2e/ for consistency | ✓ |
| Move to cmd/m31a/ | Move to cmd/m31a/e2e_test.go — closer to entry point | |

**User's choice:** Move to testutil/e2e/
**Notes:** Consistent with centralizing all test infrastructure

---

## What package name should integration tests use?

| Option | Description | Selected |
|--------|-------------|----------|
| External test package | package testutil_test — can't access unexported helpers | |
| Internal test package | package testutil — can access unexported helpers | |
| Both packages | Use both: package testutil for unit tests, package testutil_test for integration | ✓ |

**User's choice:** Both packages
**Notes:** Flexibility to test both internal and external behavior

---

## Where should test fixture files live?

| Option | Description | Selected |
|--------|-------------|----------|
| testutil/fixtures/ | testutil/fixtures/ with subdirs per domain | ✓ |
| Keep inline in tests | Keep JSON inline in test files — no separate fixture directory | |
| testdata/ per package | testdata/ directories per package — Go convention | |

**User's choice:** testutil/fixtures/
**Notes:** Centralized fixtures with clear organization

---

## What types of fixtures should go in testutil/fixtures/?

| Option | Description | Selected |
|--------|-------------|----------|
| All fixture types | JSON configs, mock API responses, sample TOML files, test data | |
| JSON only | Only JSON fixtures — configs and responses are most reusable | |
| Group by test type | Group by test type: unit fixtures, integration fixtures, e2e fixtures | ✓ |

**User's choice:** Group by test type
**Notes:** Clear organization by test category

---

## How should fixtures be loaded in tests?

| Option | Description | Selected |
|--------|-------------|----------|
| Load function | testutil.LoadFixture("providers/openrouter.json") | |
| io.Reader style | testutil.Fixtures.Open("providers/openrouter.json") | |
| go:embed | go:embed fixtures/* — compile-time embedding | ✓ |

**User's choice:** go:embed
**Notes:** Compile-time embedding, no runtime file reads, type-safe

---

## What format should fixture files use?

| Option | Description | Selected |
|--------|-------------|----------|
| JSON files | JSON files with .json extension — standard Go testdata convention | |
| Go source files | Go files with exported variables — type-safe, compile-time checked | ✓ |
| Mixed approach | Both: JSON for API responses, Go for typed test data | |

**User's choice:** Go source files
**Notes:** Type-safe, compile-time checked, no parsing needed

---

## How should import path updates be handled?

| Option | Description | Selected |
|--------|-------------|----------|
| Move then fix | Move all helpers first, then fix all import paths in one pass | |
| Gradual migration | Create aliases in old locations, migrate gradually | |
| goimports automation | Use goimports to auto-fix import paths after moves | ✓ |

**User's choice:** goimports automation
**Notes:** Automated tool handles import path updates

---

## How should import path correctness be verified?

| Option | Description | Selected |
|--------|-------------|----------|
| Incremental testing | Run make test after each move to catch broken imports early | |
| Batch verification | Move everything first, then run make check at the end | ✓ |
| Lint-based verification | Use golangci-lint to catch import issues during migration | |

**User's choice:** Batch verification
**Notes:** Move everything first, then verify all at once

---

## How should rollback be handled if migration breaks things?

| Option | Description | Selected |
|--------|-------------|----------|
| Git commits | Git commit after each major move — easy to revert if issues | ✓ |
| Branch strategy | Branch before migration — merge after verification | |
| Fix forward | No special strategy — fix forward if issues | |

**User's choice:** Git commits
**Notes:** Easy rollback with git revert

---

## How should migration commits be structured?

| Option | Description | Selected |
|--------|-------------|----------|
| Per-package commits | One commit per package moved — granular, easy to bisect | |
| Single commit | One commit for all moves — simpler history | |
| Grouped commits | Group by domain: all testutil moves in one, integration moves in another | ✓ |

**User's choice:** Grouped commits
**Notes:** Organized by domain for clear commit history

---

## the agent's Discretion

- Exact file naming within new sub-packages
- Import ordering in reorganized files
- Whether to do migration in one commit or incremental atomic commits
- Exact placement of borderline files (e.g., where does `mockProvider` belong after restructuring?)

## Deferred Ideas

None — discussion stayed within phase scope.
