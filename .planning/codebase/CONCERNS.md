---
title: CONCERNS.md
project: M31A
last_mapped: 2026-07-11
---

# Technical Concerns

## Summary

M31A is a well-structured, production-quality codebase with strong test coverage targets and clear architectural boundaries. The concerns below are nuanced — the project is notably clean (no TODO/FIXME markers, no production panics, strict linter config). Issues are structural and complexity-driven rather than code-quality failures.

---

## 1. Size and Complexity Hotspots

### `internal/tui/` — Massive, fragmented monolith

The TUI package contains **152 files** (~500 KB of Go). While intentionally split into sub-files, several files are extremely large:

| File | Size | Concern |
|------|------|---------|
| `sidebar_model.go` | 47.5 KB | Single model; logic density is very high |
| `app_view.go` | 43.3 KB | Monolithic render function |
| `firstrun_view.go` | 38.3 KB | One screen driving one huge view file |
| `app_update.go` | 25.2 KB | Top-level `Update()` dispatcher |
| `settings_model.go` | 24.5 KB | Settings screen, many sub-sections |

**Risk:** Cognitive load for contributors; any cross-cutting TUI change touches many files. Refactoring toward smaller, screened sub-packages would reduce risk.

### `internal/workflow/engine.go` — 1521-line central file

The `Engine` struct has 30+ fields and `engine.go` is 1521 lines. While sub-files exist (`engine_parse.go`, `engine_verify.go`, `engine_messages.go`), the primary file is still very large.

**Risk:** Hard to reason about the full state machine at once. Changes to phase dispatch affect many downstream behaviours.

### `internal/tools/extra_test.go` — 128 KB test file

A single test file of 128 KB is a test maintenance concern. Coverage may be padding-driven rather than scenario-driven.

**Risk:** Difficult to identify what is actually tested vs. what is filler. The existence of `coverage_boost_test.go` files suggests targeted coverage padding rather than behaviour-first tests.

---

## 2. Coverage Padding Pattern

Several packages have explicit `coverage_boost_test.go` files:

- `internal/workflow/coverage_boost_test.go` (111 KB)
- `internal/tui/` — multiple `*_extra_test.go` files totaling hundreds of KB
- `internal/tools/extra_test.go` (128 KB)

**Concern:** Coverage targets (75% overall, 90% for `taskrunner`, `bisect`, `rollback`) appear to be met in part via large synthetic test files. The `extra_test.go` pattern risks hiding gaps in meaningful behavioral tests.

**Affected packages:**
- `internal/workflow/` — `engine_extra_test.go` (107 KB), `coverage_boost_test.go` (111 KB)
- `internal/tools/` — `extra_test.go` (128 KB), `dispatcher_test.go` (28 KB)
- `internal/tui/` — many `*_extra_test.go` files

---

## 3. No Dependency Injection for LLM Providers in TUI

The provider registry is constructed in `cmd/m31a/main.go` and passed through multiple layers. The TUI's `AppState` holds a `*provider.Registry` directly. There is no interface boundary between `internal/tui` and `internal/provider`.

**Risk:** Provider mocking in TUI-level tests requires real provider structs or manual stub creation. Integration tests cannot easily substitute a fake LLM.

---

## 4. `layout.test` — 40 MB Binary Fixture in Repo

`layout.test` is a ~40 MB file committed at the repository root.

**Risk:**
- Inflates clone size significantly
- Binary files in git history are expensive (no delta compression)
- Unclear what format this file is or whether it needs to be regenerated

**Recommendation:** Investigate if this can be excluded from the main branch via `.gitignore`, replaced with a generator script, or stored in Git LFS.

---

## 5. Platform-Specific Keychain: Linux D-Bus Dependency

`pkg/keychain/keychain_linux.go` (10.7 KB) relies on D-Bus (`github.com/godbus/dbus/v5`) to access the Secret Service (GNOME Keyring / KWallet). This:

- Requires a running D-Bus session (not available in headless CI environments)
- May silently fall back or fail in Docker/container deployments
- Is ~3x larger than the Darwin or Windows implementation

**Risk:** API keys may not be stored securely in headless server deployments. The fallback path (if any) should be explicitly tested.

---

## 6. `internal/workflow/engine.go` — `WorkflowState` Mutex Coverage

`WorkflowState` has a `transitionMu sync.Mutex` for serializing phase transitions. However, other mutable fields on `WorkflowState` (`planMarkdown`, `planVersion`, `Messages`, `cachedFullPrompts`) are accessed with different granularities. The comment says Bubble Tea is single-threaded, but the Engine runs LLM calls via goroutines with `tea.Cmd` dispatches.

**Risk:** Subtle data races could exist in edge cases where phase transitions and LLM streaming overlap. The race detector in tests (`make test`) should catch this, but the locking strategy is complex.

---

## 7. Hardcoded Path Assumptions: `~/.m31a/`

Several packages reference `~/.m31a/` either literally or via `homeDir()` helper:

- `internal/tools/defaults.go`: `outputDir := filepath.Join(homeDir(), ".m31a", "tool-output")`
- Session and ledger paths are configurable but default to `~/.m31a/`

**Risk:** Portable deployments (Windows, multi-user systems, containerized environments) may conflict with different home directory layouts.

---

## 8. `e2e_test.go` — Real API Tests Skip Without Env Vars

The root-level `e2e_test.go` compiles the binary and optionally runs real API tests gated behind env vars (`OPENROUTER_API_KEY`, `ZEN_API_KEY`, `NVIDIA_API_KEY`).

**Risk:**
- CI likely never runs the real API tests unless secrets are injected
- Binary compilation is tested (good), but end-to-end LLM interaction is not part of normal CI
- Test coverage of the happy path through all 7 phases is therefore unmeasured in practice

---

## 9. `internal/wiring/` — Only One Test File

`internal/wiring/regression_test.go` (9.8 KB) is the only file in the `wiring` package. This package presumably provides integration-level wiring but has no production code — only tests.

**Risk:** If wiring logic needs to be shared beyond tests, there is no clear home for it. Package may be a vestigial test boundary.

---

## 10. Large `go.sum` vs. Slim `go.mod`

`go.sum` is 11 KB while `go.mod` has ~30 explicit dependencies. The discrepancy suggests transitive dependency chains are non-trivial. With `CGO_ENABLED=0`, any future dependency that introduces CGO (even transitively) will silently break the build constraint.

**Recommendation:** Periodically run `go mod tidy` and audit new transitive deps for CGO requirements. The `make check` target handles `tidy` but does not audit CGO.

---

## 11. Embedded 40+ MB Template / Fixtures

`internal/workflow/engine.go` uses `//go:embed templates/website-nextjs/*` to embed website templates. Combined with `layout.test` at the repo root, the working-tree footprint is significant.

**Risk:** Binary size is inflated by embedded content. Slower initial clone and build cache invalidation when templates change.

---

## Priority Summary

| # | Concern | Severity | Effort |
|---|---------|----------|--------|
| 4 | `layout.test` 40 MB binary in repo | High | Low |
| 5 | Linux D-Bus keychain in headless envs | High | Medium |
| 8 | E2E tests skip in CI (no API keys) | Medium | Medium |
| 1 | TUI/engine complexity hotspots | Medium | High |
| 2 | Coverage padding pattern | Medium | Medium |
| 6 | WorkflowState locking complexity | Medium | High |
| 3 | No provider DI boundary in TUI | Low | High |
| 7 | `~/.m31a/` path assumptions | Low | Low |
| 9 | `internal/wiring/` vestigial package | Low | Low |
| 10 | CGO risk via transitive deps | Low | Low |
| 11 | Embedded template bloat | Low | Medium |
