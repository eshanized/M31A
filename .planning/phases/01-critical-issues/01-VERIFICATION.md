---
phase: 01-critical-issues
verified: 2026-07-13T12:00:00Z
status: gaps_found
score: 3/4 must-haves verified
behavior_unverified: 0
overrides_applied: 0
re_verification: false
gaps:
  - truth: "Bash tool allows legitimate shell syntax like $(date) and ${VAR}"
    status: failed
    reason: "containsVariableExpansion() still blocks cd ${DIR} (via ${ pattern) and echo $HOME (via $[A-Za-z_] pattern). Only echo $(date) works because $(d does not match $[A-Za-z_]. The acceptance criteria explicitly require all three to work."
    artifacts:
      - path: "internal/tools/bash.go"
        issue: "containsVariableExpansion() at line 501-513 catches ${ and $[A-Za-z_] which block legitimate shell syntax ${DIR} and $HOME"
    missing:
      - "Refine containsVariableExpansion() to only catch injection patterns (e.g., $(( followed by command), not variable references like $HOME or ${DIR}"
behavior_unverified_items: []
human_verification: []
---

# Phase 1: Critical Issues Verification Report

**Phase Goal:** Resolve every Critical issue from the Developer Experience Audit
**Verified:** 2026-07-13T12:00:00Z
**Status:** gaps_found
**Re-verification:** No — initial verification

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | Bash tool allows legitimate shell syntax like $(date) and ${VAR} | ✗ FAILED | `containsVariableExpansion()` still blocks `${DIR}` (via `${` pattern at line 504) and `$HOME` (via `$[A-Za-z_]` pattern at line 503). Only `echo $(date)` passes because `$(d` does not match `$[A-Za-z_]`. |
| 2 | Installer downloads correct URLs matching goreleaser output | ✓ VERIFIED | install.sh line 64: `m31a_${VERSION}_${OS}_${ARCH}.${EXT}`. VERSION from GitHub API tag_name includes `v` prefix. Goreleaser `.goreleaser.yaml` line 30: `{{ .ProjectName }}_{{ .Version }}_{{ .Os }}_{{ .Arch }}`. Formats match. |
| 3 | --goal headless mode executes full workflow and returns proper exit codes | ✓ VERIFIED | `cmd/m31a/main.go:57-171`: `runHeadlessWorkflow` runs all 7 phases (Initialize→Ship) sequentially, returns exit code 0 on success, 1 on failure. `workflow.NewEngine` created, `engine.RunPhase` called per phase, `engine.Transition` between phases. Error handling with `fmt.Fprintf(os.Stderr, ...)` and `return 1`. |
| 4 | Permission modal shows countdown from start of 10-minute timeout | ✓ VERIFIED | `internal/tui/components/permission.go:139-143`: `if remaining > 0` shows timeout from start (no 5-minute threshold). `TestPermissionModal_Countdown` passes. `formatDurationClock` renders mm:ss format. |

**Score:** 3/4 truths verified

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `internal/tools/bash.go` | Obfuscation blocklist without `$(`, `${`, backtick | ✓ VERIFIED | `dangerousObfuscationPatterns` (lines 465-478) contains only `base64`, `eval`, `exec`, `xargs` patterns — no `$(`, `${`, or backtick |
| `internal/tools/bash.go` | `containsVariableExpansion()` handles injection | ✗ STUB | Function exists (lines 501-513) but patterns are too broad — catches legitimate `$HOME` and `${DIR}` |
| `install.sh` | URL matches goreleaser output format | ✓ VERIFIED | Line 64 constructs correct URL with lowercase `m31a`, `v`-prefixed version, OS, arch |
| `cmd/m31a/main.go` | `runHeadlessWorkflow` fully implemented | ✓ VERIFIED | Lines 57-171: full implementation with session, dispatcher, engine, all 7 phases, error handling |
| `internal/tui/components/permission.go` | Countdown visible from start | ✓ VERIFIED | Line 141: `if remaining > 0` (no threshold), line 142: renders `Timeout in mm:ss` |

### Key Link Verification

| From | To | Via | Status | Details |
|------|-----|-----|--------|---------|
| `cmd/m31a/main.go` `--goal` flag | `runHeadlessWorkflow()` | `flag.String("goal", ...)` + conditional at line 409-424 | ✓ WIRED | Goal flag triggers headless workflow function |
| `runHeadlessWorkflow` | `workflow.Engine.RunPhase` | Direct call at line 148 | ✓ WIRED | Each phase executed via engine |
| `runHeadlessWorkflow` | `workflow.Engine.Transition` | Call at line 162 | ✓ WIRED | Phase transitions between each phase |
| `permission.go` Render | `m.Remaining()` | Direct call at line 139 | ✓ WIRED | Remaining time computed and displayed |

### Data-Flow Trace (Level 4)

| Artifact | Data Variable | Source | Produces Real Data | Status |
|----------|---------------|--------|-------------------|--------|
| `permission.go` Render | `remaining` | `m.Remaining()` → `m.timeout - m.elapsed` | Yes — `m.timeout` set from `DefaultPermissionTimeout` (10min) or constructor param | ✓ FLOWING |
| `main.go` runHeadlessWorkflow | `result` | `engine.RunPhase(ctx, phase, goal)` | Yes — calls real workflow engine | ✓ FLOWING |
| `install.sh` URL | `VERSION` | GitHub API `tag_name` or user `--version` flag | Yes — real API call or user input | ✓ FLOWING |

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
|----------|---------|--------|--------|
| Build succeeds | `make build` | `m31a (52M)` built for linux/amd64 | ✓ PASS |
| Bash tests pass | `go test ./internal/tools/ -run TestBash` | All tests pass including obfuscation detection | ✓ PASS |
| Permission tests pass | `go test ./internal/tui/components/ -run TestPermission` | All tests pass including countdown | ✓ PASS |
| Backtick still blocked | `checkDangerousCommand("\`rm -rf /\`")` returns blocked=true | Test `TestBash_ObfuscationDetection/backtick_substitution` passes | ✓ PASS |
| `$(rm -rf /)` still blocked | `checkDangerousCommand("$(rm -rf /)")` returns blocked=true | Test `TestBash_ObfuscationDetection/dollar_substitution` passes | ✓ PASS |

### Probe Execution

No probes declared for this phase. SKIPPED.

### Requirements Coverage

| Requirement | Source | Description | Status | Evidence |
|-------------|--------|-------------|--------|----------|
| C1 | DX_AUDIT.md | Bash tool blocks legitimate shell syntax | ⚠️ PARTIAL | Obfuscation blocklist fixed, but `containsVariableExpansion()` still blocks `${DIR}` and `$HOME` |
| C2 | DX_AUDIT.md | Installer downloads will 404 | ✓ SATISFIED | URL format matches goreleaser output exactly |
| C3 | DX_AUDIT.md | --goal headless mode not implemented | ✓ SATISFIED | Full implementation with 7-phase workflow |
| C4 | DX_AUDIT.md | Permission modal timeout invisible | ✓ SATISFIED | Countdown shown from start of 10-minute period |

**Note:** No `REQUIREMENTS.md` file exists in `.planning/`. Requirement IDs (C1-C4) are defined in `DX_AUDIT.md` and declared in PLAN.md frontmatter.

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| `cmd/m31a/main.go` | 419 | Unreachable nil check (`goalFlag` already dereferenced at line 423) | ℹ️ Info | Dead code — `goalFlag` cannot be nil at this point since it was already used at line 409 |

No TODO/FIXME/XXX markers found in any modified files.

### Human Verification Required

None. All verifiable items checked programmatically.

### Gaps Summary

1 gap blocking goal achievement:

**C1: `containsVariableExpansion()` blocks legitimate shell syntax** — The obfuscation blocklist fix (removing `$(`, `${`, backtick from `dangerousObfuscationPatterns`) was correctly applied. However, the `containsVariableExpansion()` function at `internal/tools/bash.go:501-513` still catches:
- `$HOME` via the `$[A-Za-z_]` pattern (line 503) — blocks `echo $HOME`
- `${DIR}` via the `${` pattern (line 504) — blocks `cd ${DIR}`

Only `echo $(date)` works because `$(d` does not match `$[A-Za-z_]` (the `$` is followed by `(`, not a letter).

The acceptance criteria state: "Bash tool allows commands like `echo $(date)`, `cd ${DIR}`, and `echo $HOME` without blocking." Two of three fail. The fix needs to refine `containsVariableExpansion()` to distinguish injection patterns (e.g., `$((malicious_cmd))`) from variable references (`$HOME`, `${DIR}`).

---

_Verified: 2026-07-13T12:00:00Z_
_Verifier: the agent (gsd-verifier)_
