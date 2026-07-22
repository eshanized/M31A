# Phase 3: Internal Package Organization — Validation

**Created:** 2026-07-23
**Status:** Reference for verification strategy

## Validation Strategy

### Pre-Migration Baseline

| Check | Command | Expected |
|-------|---------|----------|
| All tests pass | `make test` | Green |
| Build succeeds | `make build` | Binary produced |
| Lint clean | `make lint` | No warnings |
| Full check | `make check` | Pass |

### Wave 1 Validation (Plan 01 — Tools)

| Check | Command | Scope |
|-------|---------|-------|
| New directories exist | `ls internal/tools/{git,todo,codeanalysis,network}/` | Filesystem |
| Package declarations correct | `grep -r '^package' internal/tools/*/` | Each subdir |
| Tools compile | `go build ./internal/tools/...` | Package |
| Tool registration works | `go vet ./internal/tools/...` | Package |
| Tests pass | `go test ./internal/tools/...` | Package |

### Wave 2 Validation (Plan 02 — TUI)

| Check | Command | Scope |
|-------|---------|-------|
| Responsibility dirs exist | `ls internal/ui/tui/{core,handlers,input,update,routing}/` | Filesystem |
| Screen models in screens/ | `ls internal/ui/tui/screens/*/` | No *_model.go at tui root |
| Repl files in screens/repl/ | `ls internal/ui/tui/screens/repl/repl_*.go` | No repl_*.go at tui root |
| Package declarations correct | `grep -r '^package' internal/ui/tui/{core,handlers,input,update,routing}/` | Each subdir |
| TUI compiles | `go build ./internal/ui/tui/...` | Package |
| Tests pass | `go test ./internal/ui/tui/...` | Package |

### Wave 3 Validation (Plan 03 — Imports + Tests)

| Check | Command | Scope |
|-------|---------|-------|
| No stale imports | `grep -r 'internal/tools"' --include='*.go' . \| grep -v reexport \| grep -v _test` | Codebase |
| No stale TUI imports | `grep -r 'internal/ui/tui"' --include='*.go' . \| grep -v _test` | Codebase |
| Full build | `go build ./...` | Codebase |
| Full vet | `go vet ./...` | Codebase |
| All tests pass | `make test` | Codebase |
| Lint clean | `make lint` | Codebase |
| Full check | `make check` | Codebase |
| Binary builds | `make build` | Binary |

### Test Classification Validation (D-13)

For each test file, verify classification:

| Pattern | Classification | Action |
|---------|---------------|--------|
| `package <same>` + unexported refs | White-box | Stay with source |
| `package <same>_test` + exported only | Black-box | Move to tests/ |
| Integration/e2e patterns | Integration | Move to tests/ |

Grep patterns for unexported access:
```bash
# Find white-box tests (package same, not _test suffix)
grep -l '^package [a-z]$' internal/tools/*_test.go internal/ui/tui/*_test.go

# Find tests referencing unexported identifiers (heuristic)
grep -rn '[^A-Z][a-z][a-zA-Z]*(' internal/tools/*_test.go | grep -v 'func Test' | grep -v 'import'
```

### Post-Migration Final Gate

| Check | Command | Pass Criteria |
|-------|---------|---------------|
| `make check` | Full quality suite | Zero failures |
| `make build` | Optimized binary | Binary produced |
| `make test` | Race-enabled tests | All pass |
| `make lint` | golangci-lint | Clean |
| Directory structure | Visual inspection | Matches plan |
