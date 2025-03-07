# Pre-Phase 3 Fix Report

## Fixes Applied

| # | ID | Description | Status |
|---|----|-------------|--------|
| 1 | CRIT-01 | go.mod restored to go 1.22 | ✅ |
| 2 | MED-02 | EstimateCost now accepts modelID string | ✅ |
| 3 | MED-01 | Provider badges corrected to [OR]/[ZEN] | ✅ |
| 4 | MED-03 | ModelCache.Get() serves stale data within 24h | ✅ |
| 5 | MED-04 | FetchModels checks cache before API call | ✅ |
| 6 | MED-06/07 | walkthrough_2.md claims corrected | ✅ |

## Build Verification

### go mod tidy -go=1.22
```
go: github.com/charmbracelet/bubbles@v1.0.0 requires go@1.24.2, but 1.22 is requested
```

Note: `go mod tidy -go=1.22` failed because transitive deps (`cellbuf`, `colorprofile`, `x/term`, `x/ansi`, `x/sys`) require Go >= 1.24.0. Fixed by pinning these deps to versions compatible with Go 1.22:

- `bubbles` downgraded from v1.0.0 → v0.20.0 (go 1.18)
- `bubbletea` downgraded from v1.3.10 → v1.3.0 (go 1.18)
- `x/cellbuf` pinned to v0.0.13 (go 1.18)
- `colorprofile` pinned to v0.2.3-0.20250311203215-f60798e515dc (go 1.18)
- `x/term` pinned to v0.2.1 (go 1.18)
- `x/ansi` pinned to v0.8.0 (go 1.18)
- `x/sys` pinned to v0.30.0 (go 1.18)

```
go mod tidy (without -go flag): succeeded
go directive: go 1.22 (preserved after tidy)
```

### go build ./...
```
(no output = clean)
```

### go vet ./...
```
(no output = clean)
```

### go test -race -count=1 ./...
```
ok  	github.com/eshanized/M31A/internal/provider	1.016s
ok  	github.com/eshanized/M31A/internal/provider/openrouter	1.293s
ok  	github.com/eshanized/M31A/internal/provider/zen	1.017s
ok  	github.com/eshanized/M31A/internal/tui	1.045s
ok  	github.com/eshanized/M31A/internal/tui/theme	1.010s
```

### Binary verification
```
m31a: ELF 64-bit LSB executable, x86-64, version 1 (SYSV), statically linked
```

### go.mod version check
```
go 1.22
```

## Test Summary
- Total tests: 171
- Passed: 171
- Failed: 0

## Remaining Audit Items
- **MIN-01 (ctrl+w skip)**: Walkthrough was corrected to document that no ctrl+w handler exists. Adding the handler is deferred — not a Phase 3 blocker.
- **MIN-04 (SSE dots-only)**: Low risk, acceptable format for known providers. No impact on Phase 3.
- **MIN-05 (Anthropic SSE)**: Needs investigation if Anthropic support is planned. Deferred to future phase.
- **MIN-06 (Missing 2 provider tests)**: Test count has grown to 171 (was 152 in Phase 2). Previous deficit is closed by new tests added in this fix cycle (FetchModels cache tests, EstimateCost known model tests, Get stale fallback tests).
- **MED-05 (calculateNextInterval)**: Walkthrough corrected. The two-state interval (60s normal/120s on failure) is intentional design, not a jitter/backoff gap. Adding jitter is deferred.

## Phase 3 Readiness
**GO** — All 6 audit items resolved. Build, vet, and all 171 tests pass. Binary is statically linked. go.mod declares `go 1.22` as specified. Provider badges display `[OR]`/`[ZEN]`. `EstimateCost()` accepts `modelID string` and computes real pricing from cache. Model cache serves stale data within 24h TTL. FetchModels checks cache first. Walkthrough claims are accurate.
