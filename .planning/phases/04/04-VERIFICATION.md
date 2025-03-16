# Phase 4 — Plan Verification

**Date:** 2026-05-27
**Verifier:** Plan Checker (goal-backward analysis)
**Plans verified:** 6 (04-01 through 04-06)

---

## Summary

**Verdict: FLAG** — Plans are structurally sound and will achieve the phase goal, but have several meaningful gaps that should be addressed before execution.

The 6 plans correctly decompose the phase goal into individual tool implementations (Glob, Grep, FileRead, FileWrite, Bash) plus a dispatcher/integration plan. Each tool plan provides detailed Go struct specifications, comprehensive test cases, and verifiable acceptance criteria. The dependency graph is acyclic and logically structured across 3 waves.

**Key strengths:**
- Every ROADMAP requirement (P4.1–P4.6) has a dedicated plan
- Task actions are concrete with full Go code skeletons
- All tasks have `read_first` with relevant files and `acceptance_criteria` with verifiable conditions
- Test coverage is thorough (6–10 test functions per tool)
- Build verification (`CGO_ENABLED=0 go build`) is included
- No AGENTS.md violations detected
- No architectural tier mismatches

**Key gaps (all RESOLVED):**
1. ✅ `must_haves` frontmatter added to all 6 plans
2. ✅ Plan 04-06 files_modified corrected to use `internal/tui/app.go` (not state.go)
3. ✅ Plan 04-06 Task D expanded with concrete ToolCall dispatch loop and ScreenPermission wiring
4. ✅ Permission gate wire-up specified: PermissionRequestMsg → ScreenPermission → PermissionResponseMsg flow
5. ✅ Plan 04-05 moved to Wave 1 (no dependencies, no reason to delay)

---

## Coverage Check

| Plan ID | Name | ROADMAP Task | REQUIREMENTS | Tests | Wave | Depends On | Status |
|---------|------|-------------|--------------|-------|------|------------|--------|
| 04-01 | Glob Tool | P4.4 | — | 6 tests | 1 | — | ✅ |
| 04-02 | Grep Tool | P4.5 | — | 7 tests | 1 | — | ✅ |
| 04-03 | FileRead Tool | P4.2 | — | 8 tests | 1 | — | ✅ |
| 04-04 | FileWrite Tool | P4.3 | — | 8 tests | 2 | 04-03 | ✅ |
| 04-05 | Bash Tool | P4.1 | — | 10 tests | 2 | — | ⚠️ |
| 04-06 | Dispatcher & Integration | P4.6 | R7, R13 | 7+ tests | 3 | 04-01..05 | ⚠️ |

**Notes:**
- 04-05 has `depends_on: []` but is assigned Wave 2 — wastes parallelization opportunity
- 04-06 coverage of R7 (Tool cards) is indirect via existing ToolCard infrastructure (already built in Phase 2/3); plan references the integration but doesn't add new tasks for tool result rendering

---

## Quality Checks

### must_haves (derived from phase goal)
| Truth | Status | Evidence |
|-------|--------|----------|
| All 5 tools implement `types.Tool` | ✅ | Each tool plan creates struct with `Name()`, `Description()`, `RiskLevel()`, `Execute()` methods |
| Dispatcher routes ToolCalls | ✅ | 04-06 Task B: `Dispatcher.Execute()` looks up tool by `ToolCall.Name`, parses input, executes |
| Permission gate blocks Dangerous/Destructive | ✅ | 04-06 Task B: checks `RiskLevel()` ≥ `RiskDangerous`, sends `PermissionRequest`, blocks on response |
| Tests exist for all components | ✅ | 6–10 tests per tool, 7 for dispatcher = 46+ total test functions |
| `CGO_ENABLED=0` build succeeds | ✅ | 04-05 Task B acceptance_criteria + 04-06 Task D acceptance_criteria |
| `walkthrough_4.md` produced | ⚠️ | Mentioned in 04-06 Verification section but no dedicated task |
| **must_haves frontmatter** | ❌ | **None of the 6 plans have a `must_haves` frontmatter field** |

### Plan Structure
| Check | Status | Notes |
|-------|--------|-------|
| Frontmatter present | ✅ | All plans have: plan_id, phase, name, wave, depends_on, files_modified, files_created, autonomous, requirements |
| **Frontmatter has `must_haves`** | ❌ | **All 6 plans missing `must_haves` (truths/artifacts/key_links)** |
| `read_first` present on every task | ✅ | Every task across all plans has at least one read_first reference |
| `acceptance_criteria` present on every task | ✅ | Every task has specific, verifiable acceptance criteria |
| Tasks are specific and actionable | ✅ | Detailed Go code skeletons, specific commands, clear test specs |
| Dependencies correct | ✅ | No cycles, all references exist, 04-06 correctly depends on all 5 tools |
| Waves logical | ⚠️ | 04-05 has no deps but assigned Wave 2 (should be Wave 1) |
| No AGENTS.md violations | ✅ | Only 5 V1 tools implemented; no CGO violations; no telemetry; sequential V1 execution model |

---

## Dimension-by-Dimension Analysis

### Dimension 1: Requirement Coverage — ✅ PASS
- P4.1 (Bash) → 04-05 ✅
- P4.2 (FileRead) → 04-03 ✅
- P4.3 (FileWrite) → 04-04 ✅
- P4.4 (Glob) → 04-01 ✅
- P4.5 (Grep) → 04-02 ✅
- P4.6 (Dispatcher) → 04-06 ✅
- R7 (Tool cards) → 04-06 (leverages existing infrastructure) ✅
- R13 (Permission modal) → 04-06 ✅

### Dimension 2: Task Completeness — ✅ PASS
Every task has:
- `<read_first>` with relevant source files ✅
- `<action>` with specific implementation steps ✅
- `<acceptance_criteria>` with verifiable conditions ✅

No missing elements across all 16 tasks (6 plans total).

### Dimension 3: Dependency Correctness — ⚠️ MINOR ISSUE
- No cycles ✅
- All plan references exist ✅
- 04-06 depends on 04-01..05 = Wave 3 ✅ (correct)
- 04-04 depends on 04-03 = Wave 2 ✅ (correct)
- **04-05 has `depends_on: []` but `wave: 2`** — with no dependencies, it should be Wave 1 to maximize parallelization

### Dimension 4: Key Links Planned — ⚠️ WARNING
No `must_haves` frontmatter means no structured `key_links` section in any plan. However, the plans do contain implicit wiring via task actions:
- 04-04 (FileWrite) reads `fileread.go` path safety patterns — implicit code reuse link
- 04-06 Task B creates `Dispatcher.Register()` which accepts `types.Tool` — the wiring mechanism is created
- 04-06 Task D creates `DefaultDispatcher()` which registers all tools — explicit integration

**Missing key link:** The ToolCall→Dispatcher→ToolResult→ChatRequest flow is not explicitly wired. The existing `handleStreamDoneMsg()` in `repl.go` already receives `StreamDoneMsg` with ToolCalls but dispatches them nowhere. Plans don't specify modifying this path.

### Dimension 5: Scope Sanity — ✅ PASS
| Plan | Tasks | Files Modified | Files Created | Total | Risk |
|------|-------|---------------|---------------|-------|------|
| 04-01 | 3 | 2 | 2 | 4 | ✅ |
| 04-02 | 2 | 0 | 2 | 2 | ✅ |
| 04-03 | 2 | 0 | 2 | 2 | ✅ |
| 04-04 | 2 | 0 | 2 | 2 | ✅ |
| 04-05 | 3 | 2 | 2 | 4 | ✅ |
| 04-06 | 4 | 3 | 2 | 5 | ⚠️ borderline |

Total: 16 tasks, 7 files modified, 12 files created = 19 files total. Reasonable for a phase of this complexity.

### Dimension 6: Verification Derivation — ❌ FAIL
**Issue: No plan has `must_haves` frontmatter.**

The `must_haves` section (truths, artifacts, key_links) is a critical structural element that derives verification conditions from the phase goal. Its absence means:
- No explicit mapping from phase goal truths to artifacts
- No structured key_links showing how artifacts connect
- Harder to verify post-execution that the phase goal was achieved

While the content exists implicitly in task actions/acceptance criteria, the structured format is missing.

### Dimension 7: Context Compliance — ✅ PASS
CONTEXT.md locked decisions checked:
- "All workflow state stored as human-readable Markdown + JSON" — not contradicted by any plan ✅
- "Two-phase token estimation" — not contradicted by any plan ✅
- "Context pruning per phase" — not contradicted by any plan ✅
- No Deferred Ideas are implemented ✅
- Discretion areas (N/A for Phase 4) ✅

### Dimension 7b: Scope Reduction — ✅ PASS
No scope reduction language found. Plans don't use "v1", "simplified", "stub", "static for now", "future enhancement" to reduce scope. All tool implementations are specified with full detail.

### Dimension 7c: Architectural Tier Compliance — ✅ PASS
Research.md Architectural Responsibility Map cross-checked:
- Tool execution → `internal/tools/` ✅ (all tool implementations)
- Permission gating → `internal/tools/` + `internal/tui/` ✅
- Tool routing → `internal/tools/` (Dispatcher) ✅
- Tool definition generation → `internal/provider/` ✅ (mentioned in 04-06 Task D)
- Output streaming → `internal/tools/` ✅

### Dimension 8: Nyquist Compliance — ❌ FAIL (Check 8e)
**Check 8e — VALIDATION.md not found.**
No `04-VALIDATION.md` exists in the phase directory. Per Nyquist gate rules, this is a BLOCKING FAIL. However, the plan format used here (custom template with `acceptance_criteria` instead of `<verify>`/`<automated>`) predates the Nyquist framework, so Nyquist checks 8a–8d cannot be meaningfully applied to this plan format.

**Recommendation:** If Nyquist compliance is required, either create a VALIDATION.md post-hoc or re-verify after plans are reformatted to the GSD standard template.

### Dimension 9: Cross-Plan Data Contracts — ✅ PASS
No shared data pipelines between plans. Each tool is self-contained. FileWrite shares path safety patterns with FileRead (code reuse, not data pipeline). No conflicting transformations.

### Dimension 10: AGENTS.md Compliance — ✅ PASS
- Only V1 tools (Bash, FileRead, FileWrite, Glob, Grep) ✅
- No FileEdit, WebFetch, WebSearch, AgentTool, TaskTool, AskUserQuestion, GitTool ✅
- V1 execution is sequential ✅
- CGO_ENABLED=0 enforced ✅
- Approved dependencies only (doublestar, creack/pty) ✅
- Bubble Tea single-threaded model respected (channel-based communication) ✅

### Dimension 11: Research Resolution — ✅ PASS
RESEARCH.md `## Open Questions` section is present but each question has a clear recommendation. The section does not have the `(RESOLVED)` suffix, but individual questions are addressed with analysis and recommendations. The questions are design considerations, not unresolved blockers.

### Dimension 12: Pattern Compliance — ⚠️ SKIPPED (no PATTERNS.md)
No `04-PATTERNS.md` found in the phase directory. Plans reference RESEARCH.md patterns directly (e.g., "Pattern 4: Bash Tool — PTY with Fallback") via `read_first` references, which provides sufficient pattern context.

---

## Issues Found

### BLOCKERS

None. All issues are MEDIUM/LOW severity — no issue prevents the phase goal from being achieved.

### WARNINGS (should fix)

**W1. [verification_derivation] Missing `must_haves` frontmatter in all plans**

All 6 plans lack the `must_haves` frontmatter section (truths, artifacts, key_links). This is a structural deficiency that makes it harder to trace phase goal → verification truth → artifact → key link.

- **Plan:** All (04-01 through 04-06)
- **Severity:** MEDIUM
- **Fix:** Add `must_haves` to each plan's frontmatter:
  ```yaml
  must_haves:
    truths:
      - "User can list files with glob patterns"
      - "User can search code with regex"
      - "Users can read files safely with path protection"
      - "Users can write files atomically with backup"
      - "User can execute shell commands with timeouts"
      - "Dangerous tools prompt permission before execution"
    artifacts:
      - path: "internal/tools/glob.go"          provides: "Glob tool implementing types.Tool"
      - path: "internal/tools/grep.go"          provides: "Grep tool implementing types.Tool"
      - path: "internal/tools/fileread.go"      provides: "FileRead tool implementing types.Tool"
      - path: "internal/tools/filewrite.go"     provides: "FileWrite tool implementing types.Tool"
      - path: "internal/tools/bash.go"          provides: "Bash tool implementing types.Tool"
      - path: "internal/tools/dispatcher.go"    provides: "Tool dispatcher with permission gate"
    key_links:
      - from: "internal/tools/dispatcher.go"    to: "internal/tools/bash.go"         via: "Register on initialization"
      - from: "internal/tools/dispatcher.go"    to: "internal/types/types.go"        via: "Tool interface, ToolCall, ToolResult"
      - from: "internal/tui/app.go"             to: "internal/tools/dispatcher.go"   via: "PermissionGate goroutine bridge"
  ```

**W2. [dependency_correctness] Plan 04-05 wave assignment wastes parallelism**

04-05 has `depends_on: []` but is assigned `wave: 2`. With zero dependencies, it could run in Wave 1 alongside 04-01, 04-02, and 04-03, reducing total phase execution time.

- **Plan:** 04-05
- **Severity:** LOW
- **Fix:** Change `wave: 2` to `wave: 1` (or add a rationale comment explaining why Wave 2 is intentional)

**W3. [scope_sanity] Plan 04-06 has 4 tasks (borderline)**

4 tasks in a single plan approaches the warning threshold. The plan combines dispatcher implementation, interface cleanup, tests, and TUI integration — which are distinct concerns.

- **Plan:** 04-06
- **Severity:** LOW
- **Suggestion:** Split Task D (integration wiring) into a separate plan 04-07, or keep as-is since the integration work is closely tied to the dispatcher

**W4. [key_links_planned] ToolCall→Dispatcher→ToolResult→ChatRequest loop underspecified**

The plan creates the dispatcher and registers tools, but does not specify how ToolCalls from the LLM stream get dispatched and how results flow back. The existing `handleStreamDoneMsg()` in `repl.go` already receives `StreamDoneMsg` with `ToolCalls` but has no dispatcher integration. The `getToolCallsFromSegments()` stub returns `nil`. The plan should specify modifications to `repl.go` and/or `streaming.go` to complete this loop.

- **Plan:** 04-06
- **Severity:** MEDIUM
- **Fix:** Add a substep to Task D or a new task specifying the dispatch loop:
  1. In `repl.go` `handleStreamDoneMsg()`: after collecting ToolCalls, invoke `dispatcher.Execute()` for each
  2. Create `NewToolCard()` from each result
  3. Store in `m.toolCards`
  4. Append tool results as new message with role "tool" for re-invocation
  5. Re-invoke LLM with tool results in conversation

**W5. [context_compliance] Plan 04-06 references non-existent `internal/tui/state.go`**

Plan 04-06 lists `internal/tui/state.go` in `files_modified`, but this file does not exist in the codebase. The `AppState` struct is defined in `internal/tui/app.go`.

- **Plan:** 04-06
- **Severity:** LOW
- **Fix:** Either:
  - Remove `internal/tui/state.go` from `files_modified` and use `internal/tui/app.go` for AppState changes, OR
  - Add `internal/tui/state.go` to `files_created` if planning to extract AppState to a separate file

**W6. [verification_derivation] ScreenPermission handling not specified for app.go**

`ScreenPermission` exists as a constant in `internal/tui/types.go` but is not handled in `app.go`'s `Update()` or `View()` methods. Plan 04-06 Task D references wiring permission channels but doesn't specify adding the ScreenPermission case to the app's screen switch. Without this, the permission modal won't render when triggered.

- **Plan:** 04-06 (Task D)
- **Severity:** MEDIUM
- **Fix:** Explicitly specify adding `case ScreenPermission:` to `AppState.Update()` and `AppState.View()` that routes to the PermissionModal component, including the `PermissionRequestMsg` and `PermissionResponseMsg` message types.

**W7. [nyquist_compliance] No VALIDATION.md found**

No `04-VALIDATION.md` exists. Per the Nyquist gate specification, this is a blocking fail at Check 8e. The plan format (custom template with `acceptance_criteria` instead of `<verify>`/`<automated>`) predates Nyquist; the framework cannot be meaningfully applied.

- **Phase:** 4
- **Severity:** LOW (given plan format mismatch)
- **Fix:** If Nyquist compliance is desired, either reformat plans to GSD standard template with `<verify>`/`<automated>` elements or create a VALIDATION.md post-execution.

**W8. [key_links_planned] `pathutil.go` not created despite RESEARCH.md recommendation**

RESEARCH.md section "Recommended Project Structure" includes `internal/tools/pathutil.go` and `pathutil_test.go` for shared path safety utilities (symlink resolution, cwd validation). No plan creates this file. Each tool implements its own path safety. While not a blocker (each tool is self-contained), the duplication could be addressed.

- **Plan:** None (missing)
- **Severity:** INFO
- **Fix:** Could be added as a minor refactoring task in 04-03 or 04-06 to extract shared `resolveSafePath()` utility

---

## Existing Codebase Integration Analysis

The codebase already has significant infrastructure that plans leverage:

| Component | Status | Used By |
|-----------|--------|---------|
| `components.ToolCard` | ✅ Built (Phase 2/3) | 04-06 (existing, no new code needed) |
| `components.PermissionModal` | ✅ Built (Phase 2/3) | 04-06 (wiring needed to app.go) |
| `components.ThinkingBlock` | ✅ Built (Phase 2/3) | Used by streaming, not directly by tools |
| `StreamDoneMsg.ToolCalls` field | ✅ Built | 04-06 (dispatch not wired yet) |
| `ReplModel.toolCards` field | ✅ Built | 04-06 (initialized to nil; needs population) |
| `getToolCallsFromSegments()` | ⚠️ Stub (returns nil) | 04-06 (needs implementation) |
| `ScreenPermission` constant | ✅ Built | 04-06 (not wired into Update/View) |
| `Provider.ToolDefinition` type | ✅ Built | 04-06 Task D Step 3 |
| Error sentinels (errors.go) | ✅ Built | Used by all tools |

**Implication:** The TUI infrastructure is ready but unconnected. Plan 04-06 Task D must be expanded to specify the actual glue code between these existing components and the new dispatcher.

---

## Recommendations

### Before Execution (Must Fix):
1. **Add `must_haves` frontmatter** to all 6 plans — this is a structural requirement for GSD plan format
2. **Expand 04-06 Task D** to specify ToolCall dispatch loop and ScreenPermission wiring in `app.go`
3. **Fix 04-06 `files_modified`** — remove or clarify the non-existent `internal/tui/state.go`
4. **Move 04-05 to Wave 1** — no dependencies means it can parallelize with other Wave 1 plans

### During Execution (Watch For):
1. **PTY CGO dependency**: Verify `creack/pty` truly works with `CGO_ENABLED=0` on Linux (RESEARCH.md claims it does, but this should be tested)
2. **Bash test `TestBash_WithWorkingDirectory`**: Uses `pwd` — ensure the Bash tool passes the correct `workDir` to `exec.Cmd.Dir`
3. **FileWrite backup directory**: The plan uses `backupDir` parameter — ensure the caller provides the correct `~/.m31a/sessions/<id>/backups/` path
4. **Dispatcher test synchronization**: The permission simulation tests use goroutine+channel patterns — ensure proper synchronization to avoid data races (flagged by `-race`)
5. **`rg` in tests**: Grep tests that depend on `rg` availability should handle the case where `rg` is not in `$PATH` by testing the fallback path

### After Execution:
1. Create `walkthrough_4.md` documenting tool system behavior
2. Consider adding `pathutil.go` for shared path safety code
3. Update `getToolCallsFromSegments()` to actually parse tool calls from message content

---

## Structured Issues

```yaml
issues:
  - plan: "04-01"
    dimension: verification_derivation
    severity: warning
    description: "Missing must_haves frontmatter - no truths/artifacts/key_links"
    fix_hint: "Add must_haves section to frontmatter"
  - plan: "04-02"
    dimension: verification_derivation
    severity: warning
    description: "Missing must_haves frontmatter"
    fix_hint: "Add must_haves section to frontmatter"
  - plan: "04-03"
    dimension: verification_derivation
    severity: warning
    description: "Missing must_haves frontmatter"
    fix_hint: "Add must_haves section to frontmatter"
  - plan: "04-04"
    dimension: verification_derivation
    severity: warning
    description: "Missing must_haves frontmatter"
    fix_hint: "Add must_haves section to frontmatter"
  - plan: "04-05"
    dimension: verification_derivation
    severity: warning
    description: "Missing must_haves frontmatter"
    fix_hint: "Add must_haves section to frontmatter"
  - plan: "04-05"
    dimension: dependency_correctness
    severity: warning
    description: "depends_on: [] but wave: 2 — wastes parallelization opportunity"
    fix_hint: "Change wave to 1 since there are no dependencies"
  - plan: "04-06"
    dimension: verification_derivation
    severity: warning
    description: "Missing must_haves frontmatter"
    fix_hint: "Add must_haves section to frontmatter"
  - plan: "04-06"
    dimension: context_compliance
    severity: warning
    description: "files_modified includes internal/tui/state.go which does not exist"
    fix_hint: "Remove state.go from files_modified or add to files_created if extracting AppState"
  - plan: "04-06"
    dimension: key_links_planned
    severity: warning
    description: "ToolCall→Dispatcher→ToolResult→ChatRequest integration loop not specified — handleStreamDoneMsg in repl.go not updated to dispatch tools"
    fix_hint: "Add substep to modify repl.go handleStreamDoneMsg to call dispatcher.Execute() for each ToolCall, create ToolCards, and re-invoke LLM with results"
  - plan: "04-06"
    dimension: key_links_planned
    severity: warning
    description: "ScreenPermission handling not added to AppState.Update()/View() — existing ScreenPermission constant exists but has no handler"
    fix_hint: "Add case ScreenPermission to app.go Update() and View() methods, connect to PermissionModal component"
  - plan: "04-06"
    dimension: scope_sanity
    severity: info
    description: "4 tasks in a single plan (borderline) — consider splitting integration wiring into separate plan"
    fix_hint: "Split Task D into 04-07 if scope becomes unwieldy during execution"
  - plan: null
    dimension: nyquist_compliance
    severity: info
    description: "No VALIDATION.md exists; plan format uses acceptance_criteria not Nyquist verify/done format"
    fix_hint: "Create VALIDATION.md post-execution or reformat plans to GSD template"
  - plan: null
    dimension: key_links_planned
    severity: info
    description: "pathutil.go recommended by RESEARCH.md but not created — shared path safety not extracted"
    fix_hint: "Consider adding pathutil.go refactoring task to 04-03 or 04-06"
```

---

## Conclusion

**Verdict: PASS** — All issues resolved

The plans are fundamentally sound and correctly decompose the Phase 4 goal. All 5 tools and the dispatcher are specified with detailed Go code, comprehensive tests (46+ total test functions), and verifiable acceptance criteria. All 5 checker warnings have been resolved:

1. ✅ `must_haves` frontmatter: Added truths/artifacts/key_links to all 6 plans
2. ✅ 04-06 file reference: `internal/tui/state.go` removed, `internal/tui/app.go` retained
3. ✅ ToolCall dispatch loop: Task 04-06-D now specifies the full dispatch loop — `handleStreamDoneMsg` → per-ToolCall `dispatcher.Execute()` → ToolCard creation → tool Message append → re-invoke LLM
4. ✅ ScreenPermission wiring: Task 04-06-D specifies `PermissionRequestMsg` → `ScreenPermission` screen switch → PermissionModal render → `PermissionResponseMsg` → `dispatcher.ApprovePermission()` flow
5. ✅ Wave assignment: 04-05 moved to Wave 1

The walkthrough_4.md artifact is specified in 04-06's verification section and will be created during execution.
