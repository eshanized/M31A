# Phase 4: Audit and remove unused/irrelevant code from M31A codebase - Research

**Researched:** 2026-07-27
**Domain:** Dead code detection, codebase cleanup, Go static analysis
**Confidence:** HIGH

## Summary

M31A is a 187K-line Go codebase (737 files) implementing an AI-powered CLI agent with TUI, workflow engine, and multi-provider LLM support. Phases 01-03 fixed 30 bugs, 59 test regressions, and CI issues. Phase 04 targets dead code removal with a conservative approach: only clearly dead code with no references, no tests, no imports.

Static analysis (`golangci-lint --enable-only unused`) found 0 issues, meaning the Go compiler's `unused` pass is satisfied. However, manual analysis reveals several categories of dead code that static analysis misses: deprecated functions with no callers, unused exported utility functions, redundant wrapper/alias functions, and unused re-exports.

**Primary recommendation:** Remove 6 clear categories of dead code totaling ~15 files/functions, all verified via grep to have zero production callers. Each removal is low-risk: deprecated functions that just delegate, utility functions with no callers, and wrapper functions that duplicate existing implementations.

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Dead code identification | Codebase (static analysis) | — | Grep for references, unused linter, import analysis |
| Scope boundary enforcement | Documentation | Code review | M31A's core = AI CLI + TUI + workflow + providers + tools |
| Removal execution | Code (file deletion) | Git (single commit) | D-07: single commit for all removals |
| Risk assessment | Code review | Testing | Conservative: only clearly dead code |

## Standard Stack

### Tools for Dead Code Detection
| Tool | Purpose | How to Use |
|------|---------|------------|
| `golangci-lint --enable-only unused` | Find unused types/functions/variables | `make lint` |
| `grep -rn` | Find zero-reference exported functions | Pattern: `grep -rn "FuncName" --include="*.go"` |
| `go mod tidy` | Detect unused dependencies | Already clean — no output |
| `go vet ./...` | Find structural issues | Build environment quota-limited |

### Code Removal Pattern
| Step | Action | Verification |
|------|--------|-------------|
| 1 | Identify dead code via grep (0 production callers) | `grep -rn "FuncName" --include="*.go" \| grep -v "_test.go:" \| grep -v "func FuncName"` |
| 2 | Remove function/type/file | `go build ./...` |
| 3 | Remove associated tests | `go test ./...` |
| 4 | Run `go mod tidy` if dependencies freed | No output = clean |
| 5 | Single commit | `chore: remove unused code (phase 4)` |

## Package Legitimacy Audit

> No new packages installed. This phase only removes code.

## Architecture Patterns

### Dead Code Categories Found

#### Category 1: Deprecated Theme Functions (no production callers)
- `BrandGradientStyle()` — deprecated, only in tests
- `ThinkingGradientStyle()` — deprecated, only in tests
- `theme.Light()` — deprecated, only in tests
- `theme.Dark()` — alias for `M31A()`, never called

#### Category 2: Unused Buffer Pool Utilities (only in tests)
- `ai.GetBuffer()` — only called in `memory_test.go`
- `ai.PutBuffer()` — only called in `memory_test.go`
- `ai.PreallocateSlice()` — only called in `memory_test.go`
- `ai.SizeHintMap()` — never called anywhere
- `ai.PoolStats()` — never called anywhere
- `ai.BufferPoolStats` type — never used

#### Category 3: Unused Wrapper/Re-export Functions
- `tui.TruncateWithEllipsis()` — wrapper for `components.TruncateWithEllipsis()`, never called
- `tui.TruncateMiddle()` — wrapper for `components.TruncateMiddle()`, never called
- `tui.TruncateEnd()` — wrapper for `components.TruncateEnd()`, never called
- `tools.LevenshteinBuf()` — re-export, never called via tools package
- `tools.CheckDangerousCommand()` — re-export, never called via tools package
- `tools.ScrubEnvironment()` — re-export, never called via tools package

#### Category 4: Unused Provider Functions (test-only)
- `provider.DetectCapabilities()` — only called in tests
- `provider.CheckModelHealth()` — only called in tests
- `provider.NewSSEParser()` — only `NewSSEParserWithContext()` is used

#### Category 5: Unused Tuitypes Utility
- `tuitypes.FormatDurationMs()` — defined but never called

#### Category 6: Unused Theme Function
- `theme.PaletteForProfile()` — defined but never called (DetectColorProfile is used, but PaletteForProfile is not)

#### Category 7: Unused Accessibility Package
- `internal/ui/tui/a11y/` — entire package only imported in tests, never in production code

### Pattern: Wrapper Function Anti-Pattern

**What:** Multiple layers of wrapper functions that delegate to the same underlying implementation:
```
tuitypes.TruncateWithEllipsis → (duplicate implementation)
tui.TruncateWithEllipsis → components.TruncateWithEllipsis
```

**Why it happens:** Gradual migration without removing old paths.

**Fix:** Remove the unused wrapper layer (tui/truncate.go) since callers use either tuitypes or components directly.

### Pattern: Deprecated Function Accumulation

**What:** Functions marked deprecated but kept "for backward compatibility" with no callers.

**Why it happens:** Fear of breaking changes, but no actual consumers exist.

**Fix:** Remove if grep confirms zero production callers.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Dead code detection | Custom AST analysis | `golangci-lint unused` + grep | Static analysis + reference counting |
| Truncation functions | Multiple implementations | Single `components.TruncateWithEllipsis` | DRY, one canonical implementation |
| Buffer pooling | Custom sync.Pool wrappers | Standard sync.Pool directly | Simpler, no abstraction needed |

## Common Pitfalls

### Pitfall 1: Removing Code That Appears Unused But Has Implicit Callers
**What goes wrong:** Deleting a function that's called via interface dispatch, reflection, or go:generate.
**Why it happens:** Go's interface satisfaction is implicit — a type can satisfy an interface without direct function calls.
**How to avoid:** Only remove functions with zero grep matches in non-test, non-definition lines. Check for `var _ Interface = (*Type)(nil)` compile-time checks.
**Warning signs:** Interface implementations, compile-time interface checks, go:generate directives.

### Pitfall 2: Breaking Tests By Removing Dead Code's Test Files
**What goes wrong:** Removing a function and its test, but the test also tested side effects or coverage.
**Why it happens:** Tests sometimes assert behaviors that are indirectly important.
**How to avoid:** D-06 says remove tests with dead code — but verify the test doesn't cover other functionality.

### Pitfall 3: Removing Re-exports That External Code Uses
**What goes wrong:** Removing a re-export from `tools` package that's used by `cmd/m31a/main.go`.
**Why it happens:** Re-exports exist specifically for backward compatibility.
**How to avoid:** Grep for `tools.FuncName` before removing. Verified: `tools.LevenshteinBuf`, `tools.CheckDangerousCommand`, `tools.ScrubEnvironment` have zero external callers.

## Code Examples

### Pattern: Verify Zero Production Callers
```bash
# For a function like BrandGradientStyle:
grep -rn "BrandGradientStyle" --include="*.go" --exclude-dir=".planning" --exclude-dir=".m31a" --exclude-dir="vendor"
# Result: only borders.go (definition) and *_test.go files
# Verdict: safe to remove
```

### Pattern: Verify Wrapper Is Unused
```bash
# For tui.TruncateWithEllipsis wrapper:
grep -rn "tui\.TruncateWithEllipsis" --include="*.go" --exclude-dir=".planning" --exclude-dir=".m31a" --exclude-dir="vendor"
# Result: zero matches (callers use tuitypes or components directly)
# Verdict: safe to remove tui/truncate.go
```

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | `BrandGradientStyle()` and `ThinkingGradientStyle()` have zero production callers | Category 1 | Low — tests still pass, function is deprecated |
| A2 | `ai.GetBuffer/PutBuffer/PreallocateSlice` are only used in tests | Category 2 | Low — utility functions, not core logic |
| A3 | `tui.Truncate*` wrappers have zero callers | Category 3 | Low — callers use tuitypes or components directly |
| A4 | `tools.LevenshteinBuf/CheckDangerousCommand/ScrubEnvironment` re-exports have zero external callers | Category 3 | Low — verified via grep |
| A5 | `provider.DetectCapabilities/CheckModelHealth` are test-only | Category 4 | Low — functions exist for future use, tests still pass |
| A6 | `tuitypes.FormatDurationMs` is never called | Category 5 | Low — utility function |
| A7 | `theme.PaletteForProfile` is never called | Category 6 | Low — DetectColorProfile is used but PaletteForProfile is not |
| A8 | `internal/ui/tui/a11y/` is only imported in tests | Category 7 | Low — accessibility feature not yet integrated |

**All claims verified via grep in this session. No unverified assumptions.**

## Open Questions

1. **Should `theme.Light()` and `theme.Dark()` be removed or kept as aliases?**
   - What we know: Both are deprecated, neither is called in production code
   - What's unclear: Whether external consumers (plugins, forks) might use them
   - Recommendation: Remove — they're internal functions, not part of public API

2. **Should `provider.DetectCapabilities/CheckModelHealth` be kept for future use?**
   - What we know: Only used in tests, no production callers
   - What's unclear: Whether these are planned for future features
   - Recommendation: Keep — they're small utility functions with tests, removing would lose functionality

3. **Should the entire `a11y` package be removed or kept for future integration?**
   - What we know: Only imported in tests, never in production code
   - What's unclear: Whether accessibility is planned for future phases
   - Recommendation: Keep — it's a complete, tested feature that may be integrated later

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Go standard `testing` package |
| Config file | None — uses `go test` directly |
| Quick run command | `make test-fast` |
| Full suite command | `make test` |

### Phase Requirements -> Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| D-04 | Conservative removal | manual verification | `grep -rn "FuncName" --include="*.go"` | N/A |
| D-06 | Remove tests with dead code | unit | `make test` after removal | N/A |

### Sampling Rate
- **Per task commit:** `make test-fast`
- **Per wave merge:** `make check`
- **Phase gate:** Full suite green before `/gsd-verify-work`

## Security Domain

> Not applicable — this phase only removes code, does not add new functionality.

## Sources

### Primary (HIGH confidence)
- Direct grep analysis of codebase (737 Go files)
- `golangci-lint --enable-only unused` (0 issues)
- `go mod tidy` (no output — all dependencies used)

### Secondary (MEDIUM confidence)
- Manual review of deprecated markers in code comments
- Architecture documentation from `.planning/codebase/`

## Metadata

**Confidence breakdown:**
- Standard Stack: HIGH — verified via grep and static analysis
- Architecture: HIGH — based on ARCHITECTURE.md and STRUCTURE.md
- Pitfalls: HIGH — derived from grep verification patterns

**Research date:** 2026-07-27
**Valid until:** 2026-08-27 (30 days — stable codebase)
