# Walkthrough Requirements for All Future Phases

Every implementation phase MUST produce a walkthrough document at `rush/walkthrough_<N>_<name>.md`.

## Walkthrough Template

```markdown
# Phase X — <Name> Walkthrough

## Summary

One paragraph describing what was built and why.

## Files Created

| File | Lines | Purpose |
|------|-------|---------|
| ...  | ...   | ...     |

## Files Modified

| File | Lines Changed | What Changed |
|------|---------------|--------------|
| ...  | +N, -M        | ...          |

## Architecture Decisions

### Decision 1: <Title>
- **What**: ...
- **Why**: ...
- **Alternatives considered**: ...

## Test Coverage

```
go test -race -count=1 ./...
```

Output: <paste test output>

Total tests: N (all passing)

## Build Verification

```
CGO_ENABLED=0 go build -o m31a ./cmd/m31a
```

Binary: <file output>

```
go vet ./...
```

Vet: <clean or list issues>

## Deviations from Plan

| # | Planned | Actual | Reason |
|---|---------|--------|--------|
| 1 | ...     | ...    | ...    |

## Known Limitations

- ...

## Next Phase Dependencies

- ...
```

## Mandatory Sections

Every walkthrough MUST include:

1. **Summary** — What was built
2. **Files Created** — Table with file, line count, purpose
3. **Files Modified** — Table with file, lines changed, description
4. **Architecture Decisions** — Each with What/Why/Alternatives
5. **Test Coverage** — Actual `go test` output + total count
6. **Build Verification** — `go build` binary type + `go vet` output
7. **Deviations from Plan** — Table of any changes from the prompt
8. **Known Limitations** — Any incomplete or deferred items
9. **Next Phase Dependencies** — What the next phase needs from this one

## Verification Checklist

Before declaring a phase complete:

- [ ] All files from the prompt are created
- [ ] All tests from the prompt are written and passing
- [ ] `go mod tidy` succeeds
- [ ] `CGO_ENABLED=0 go build -o m31a ./cmd/m31a` succeeds
- [ ] `go vet ./...` is clean
- [ ] `go test -race -count=1 ./...` — all tests pass
- [ ] Binary is statically linked (`file m31a` shows "statically linked")
- [ ] Walkthrough document is written to `rush/walkthrough_<N>_<name>.md`
- [ ] Deviations from plan are documented in the walkthrough
- [ ] No unauthorized dependencies added to go.mod
