# Phase 9: Architecture Upgrade & Directory Restructuring - Validation Architecture

**Phase:** 09-architecture-upgrade  
**Created:** 2026-07-16  
**Nyquist Validation:** Enabled (workflow.nyquist_validation: true)

---

## Validation Architecture

Per RESEARCH.md "Validation Architecture" section, this phase follows the standard Go tooling verification strategy. Each plan must be independently compilable and testable.

### Verification Gates (Per Plan)

| Gate | Command | Purpose |
|------|---------|---------|
| Build | `go build ./...` | Compile check — catches import errors, type mismatches, cycles |
| Test | `go test ./... -count=1` | Behavioral verification — all tests pass |
| Quality | `make check` | Full pipeline: fmt → tidy → vet → lint → test |

### Atomic Verification Points

Per RESEARCH.md "Verification Strategy":

1. **Plan 1 (Type cleanup):** After deleting `internal/types/`, run `go build ./...` and `go test ./...`
2. **Plan 2 (pkg→internal move):** After each package move, run `go build ./...`. After all moves, run `make check`
3. **Plan 3 (TUI screen splits):** After each screen group move, run `go build ./...`
4. **Plan 4 (Tool domain grouping):** After moves, run `go build ./...`
5. **Plan 5 (Workflow decomposition):** After moves, run `go build ./...` — verify `go:embed` paths work
6. **Plan 6 (TUI handlers):** After move, run `go build ./...`
7. **Plan 7 (Workflow/TUI interfaces):** After extraction, run `go build ./...` and `go test ./...`
8. **Plan 8 (Tools/Provider interfaces):** After extraction, run `go build ./...`
9. **Plan 9 (Constructor injection):** After wiring, run `go build ./...` and `go test ./...`
10. **Plan 10 (Final verification):** Full `make check` + import graph audit + coverage check + binary smoke test

### Phase Gate (After All Plans)

Final verification requires ALL of the following to pass:

```bash
# 1. Full quality gate
make check

# 2. Import graph audit — no pkg/ imports, no cycles
go list -deps ./internal/... | grep "pkg/" | wc -l  # must be 0
go build ./... 2>&1 | grep -i "import cycle" | wc -l  # must be 0

# 3. Structural verification
ls -1 internal/ | grep -E "^(types|session|keychain|metrics|compaction|ledger|retry|rollback|taskrunner|bisect|arbitrage|coordinator|history|narrative|skills|autodream|errors)$" | wc -l  # 17
ls -1 internal/tui/screens/ | wc -l  # 34
ls -1 internal/tools/ | grep -E "^(fileops|exec|search|ai)$" | wc -l  # 4
ls -1 internal/workflow/ | grep -E "^(engine|phases|streaming)$" | wc -l  # 3
ls -1 internal/tui/ | grep "handlers" | wc -l  # 1

# 4. Legacy directories removed
test ! -d internal/types && test ! -d pkg && echo "Legacy removed"

# 5. Coverage targets
go test ./... -coverprofile=coverage.out
go tool cover -func=coverage.out | tail -1  # overall >= 75%
go tool cover -func=coverage.out | grep -E "(taskrunner|bisect|rollback)" | awk '{print $3}' | sed 's/%//' | awk '$1 < 90 {exit 1}'  # critical >= 90%

# 6. Binary smoke test
make build && ./m31a --version && ./m31a --help
```

### Nyquist Sampling

Per workflow.nyquist_validation, each plan commits at least one verification artifact:

| Plan | Verification Artifact | Location |
|------|----------------------|----------|
| 01 | Build + test output after type cleanup | 09-01-SUMMARY.md |
| 02 | Build + test output after pkg→internal | 09-02-SUMMARY.md |
| 03 | Build output after TUI screen splits | 09-03-SUMMARY.md |
| 04 | Build output after tool grouping | 09-04-SUMMARY.md |
| 05 | Build output after workflow split | 09-05-SUMMARY.md |
| 06 | Build output after handler grouping | 09-06-SUMMARY.md |
| 07 | Build + test after interface extraction | 09-07-SUMMARY.md |
| 08 | Build output after tool/provider interfaces | 09-08-SUMMARY.md |
| 09 | Build + test after constructor injection | 09-09-SUMMARY.md |
| 10 | Full verification report | 09-10-SUMMARY.md |

### Acceptance Criteria

Phase 9 is complete when:

- [ ] All 10 plans execute successfully (each independently compilable/testable)
- [ ] `make check` passes on final codebase
- [ ] Import graph clean: no `pkg/` imports from `internal/`, no cycles
- [ ] Directory structure matches RESEARCH.md Architecture Responsibility Map
- [ ] Coverage ≥ 75% overall, ≥ 90% for taskrunner/bisect/rollback
- [ ] Binary builds and runs (`--version`, `--help`)
- [ ] No `internal/types/` directory, no `pkg/` directory

---

## Traceability Matrix

| Requirement | Plans Covering | Verification Method |
|-------------|----------------|---------------------|
| NFR-4 (Maintainability) | All 10 plans | make check, coverage, import graph |
| D-01 (Conservative splitting) | 03, 04, 05 | Directory counts match RESEARCH.md map |
| D-02 (TUI screen sub-packages) | 03 | 34 screen directories |
| D-03 (Workflow sub-packages) | 05 | 3 workflow sub-directories |
| D-04 (Tool domain groups) | 04 | 4 tool domain directories |
| D-05 (Delete internal/types/) | 01 | Directory absent |
| D-06 (Audit pkg/ for internal types) | 02 | All pkg content in internal/ |
| D-07 (Domain-specific types) | 01, 02 | Types near their domain |
| D-08 (Move pkg/ to internal/) | 02 | pkg/ absent, 17 packages in internal/ |
| D-09 (Preserve package names) | 02 | Package names unchanged |
| D-10 (Keep packages separate) | 02 | 17 distinct packages |
| D-11 (WorkflowEngine interface) | 07 | Interface exists in tuitypes/ |
| D-12 (TUI handler grouping) | 06 | handlers/ sub-package exists |
| D-13 (Package boundary interfaces) | 07, 08 | Interfaces at boundaries |
| D-14 (Constructor injection) | 09 | main.go wires dependencies |