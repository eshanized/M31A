# Phase 24 — TUI Redesign: Plan Verification Report

**Generated:** 2026-06-05  
**Plans verified:** 5  
**Status:** ISSUES FOUND — 1 blocker, 2 warnings

---

## Summary

| Plan | Wave | Tasks | Files | Status |
|------|------|-------|-------|--------|
| 24-01 | 1 | 3 | 8 | ⚠️ WARNING (sparkline duplication) |
| 24-02 | 2 | 2 | 5 | ✅ PASS |
| 24-03 | 3 | 3 | 4 | ✅ PASS |
| 24-04 | 3 | 2 | 2 | ✅ PASS |
| 24-05 | 4 | 3 | 6 | ✅ PASS |

---

## Requirement Coverage

All 8 requirement IDs are covered:

| Requirement | Plan(s) | Status |
|-------------|---------|--------|
| TUI-01 (Theme Enhancement) | 24-01 | ✅ Covered |
| TUI-02 (Shared Chrome) | 24-01 | ✅ Covered |
| TUI-03 (REPL Redesign) | 24-02 | ✅ Covered |
| TUI-04 (FirstRun Redesign) | 24-02 | ✅ Covered |
| TUI-05 (Plan Screen) | 24-03 | ✅ Covered |
| TUI-06 (Execute + Verify) | 24-03 | ✅ Covered |
| TUI-07 (Ship + Diff) | 24-04 | ✅ Covered |
| TUI-08 (ModelSelector + Settings + Resume) | 24-05 | ✅ Covered |

---

## Dependency Graph

```
24-01 (Wave 1) ──→ 24-02 (Wave 2)
                ──→ 24-03 (Wave 3)
                ──→ 24-04 (Wave 3)
                ──→ 24-05 (Wave 4)
```

- No circular dependencies ✅
- All referenced plans exist ✅
- Wave assignments consistent with dependencies ✅

---

## Cross-Wave File Conflict Check

| Wave | Plans | Files Modified | Conflicts |
|------|-------|----------------|-----------|
| 1 | 24-01 | theme.go, colors.go, sparkline.go, starfield.go, header.go, statusbar.go, sidebar.go, permission.go | None |
| 2 | 24-02 | repl.go, repl_view.go, firstrun.go, message.go, toolcard.go | None |
| 3 | 24-03 + 24-04 | plan.go, execute.go, verify.go, permission.go + ship.go, diff.go | None |
| 4 | 24-05 | modelselector*.go, settings.go, resume.go | None |

No same-wave file conflicts ✅

---

## Dimension Results

### Dimension 1: Requirement Coverage ✅ PASS
All 8 TUI-0X requirements mapped to plan requirements fields and covered by tasks.

### Dimension 2: Task Completeness ✅ PASS
All 13 tasks across 5 plans have: Files, Action, Verify (automated), Acceptance Criteria, Done criteria.

### Dimension 3: Dependency Correctness ✅ PASS
All plans depend only on 24-01. No cycles. Wave assignments valid.

### Dimension 4: Key Links Planned ✅ PASS
Each plan has key_links connecting artifacts. Specific patterns specified (e.g., `RenderSparkline`, `RenderStarfield`, `ToolCard|DoubleBorder`).

### Dimension 5: Scope Sanity ✅ PASS
| Plan | Tasks | Files | Assessment |
|------|-------|-------|------------|
| 24-01 | 3 | 8 | Borderline — 8 files is high but all are modifications to existing files + 2 new components |
| 24-02 | 2 | 5 | Good |
| 24-03 | 3 | 4 | Good |
| 24-04 | 2 | 2 | Good |
| 24-05 | 3 | 6 | Good |

### Dimension 6: Verification Derivation ✅ PASS
All must_haves.truths are user-observable (e.g., "REPL messages render with role-colored gutters", not "add Gutter struct"). Artifacts map to truths. Key_links connect artifacts to functionality.

### Dimension 7: Context Compliance ✅ PASS
- All locked decisions from CONTEXT.md are referenced in plan actions (D-01 in 24-01 Task 1, D-02 in 24-01 Task 3)
- No deferred ideas included (no vision, voice, ghost mode, PiP, subagents)
- Discretion areas handled appropriately

### Dimension 8: Nyquist Compliance ✅ PASS (N/A)
No VALIDATION.md found for this phase. Skipped per spec.

### Dimension 9: Cross-Plan Data Contracts ✅ PASS
Plan 24-02 and 24-05 both consume sparkline.go from 24-01 — compatible (same RenderSparkline API assumed). Plan 24-03 uses permission.go pattern from 24-01 — compatible (same Modal pattern).

### Dimension 10: AGENTS.md Compliance ✅ PASS
- All tasks use `go build`, `go test`, `go vet` per AGENTS.md
- No CSS animations (Bubble Tea Unicode spinners + frame redraws respected)
- CGO_ENABLED=0 not violated (no CGO dependencies added)
- No telemetry or analytics
- No direct Anthropic/OpenAI connections
- Single-threaded Bubble Tea model respected (all state mutations through Update())

### Dimension 11: Research Resolution ✅ PASS
RESEARCH.md has `## RESEARCH COMPLETE` marker. No open questions section found.

### Dimension 12: Pattern Compliance ✅ PASS
No PATTERNS.md found for this phase. Skipped per spec.

---

## Issues Found

### 1. [sparkline_duplication] Existing sparkline component not accounted for

**Severity:** WARNING  
**Plan:** 24-01  
**Task:** 2  
**Description:** Plan 24-01 Task 2 proposes creating `internal/tui/components/sparkline.go` with `RenderSparkline(data []float64, width int, t theme.Theme) string`. However, this file already exists with a different API: `Sparkline` struct with `Render() []string` method using `[]int` values and block characters (`▁▂▃▄▅▆▇█`), not braille characters (`⣀⣠⣤⣶⣾⣿`) as specified in the plan.

**Impact:** If executed as-is, there will be two competing sparkline implementations: the existing `Sparkline` struct (block chars, `[]int`) and the new `RenderSparkline` function (braille chars, `[]float64`). Plan 24-05 Task 1 references `RenderSparkline` from the new API, which won't exist alongside the existing implementation without careful coordination.

**Fix:** Either:
1. **Enhance existing** — Modify `internal/tui/components/sparkline.go` to add braille character support and a `RenderSparkline` convenience function that wraps the existing `Sparkline` struct, OR
2. **Replace existing** — Update the plan to explicitly replace the existing implementation with the new braille-based API, updating all callers.

The recommendation is option 1 (enhance), as it preserves backward compatibility and the existing block character rendering is useful for bar charts and other contexts.

**Fix hint:** Update Task 2 action to say "Enhance existing sparkline.go" instead of "Create sparkline.go". Add `Read first: internal/tui/components/sparkline.go` (already present but action doesn't reference existing code).

---

### 2. [wave_assignment_efficiency] Plan 24-05 wave assignment is conservative

**Severity:** INFO  
**Plan:** 24-05  
**Description:** Plan 24-05 (ModelSelector + Settings + Resume) is assigned to Wave 4, but its only dependency is 24-01 (Wave 1). It could run in Wave 2 alongside 24-02, or Wave 3 alongside 24-03/24-04, since none of those plans modify the same files. This would reduce total wall-clock time.

**Impact:** Minor — the phase takes longer than necessary. The current wave structure has 4 waves when 2-3 would suffice.

**Fix:** Consider moving 24-05 to Wave 2 or Wave 3. Since it has no file conflicts with 24-02, 24-03, or 24-04, it can safely run in parallel.

**Fix hint:** Update wave in 24-05 frontmatter from `4` to `2` or `3`. Update ROADMAP.md wave table accordingly.

---

### 3. [read_first_starfield] Starfield component has no existing file to read

**Severity:** INFO  
**Plan:** 24-01  
**Task:** 2  
**Description:** Task 2 lists `internal/tui/components/starfield.go` in `<files>` but does NOT include it in `<read_first>` (correctly, since it doesn't exist). The `read_first` lists existing component patterns (`message.go`, `toolcard.go`) which is correct. However, the task action says "Create" without acknowledging this is a net-new file with no existing code to reference.

**Impact:** None — this is correctly handled. The read_first includes existing patterns to follow, and the action clearly says "Create". No issue at execution time.

**Fix:** No fix needed. Mentioned for completeness.

---

## Detailed Plan Analysis

### Plan 24-01 — Theme Enhancement + Shared Chrome

**Objective:** Establish visual foundation for entire TUI redesign.

**Tasks:**
1. Theme Enhancement — New Color Tokens and Style Fields ✅
   - Specific: lists exact field names, exact hex colors, exact file modifications
   - verify: automated build + test
   - acceptance_criteria: 11 concrete grep-based assertions

2. Sparkline + Starfield Components ⚠️ (see Issue #1)
   - Specific: function signatures, character sets, normalization logic
   - verify: automated build + test
   - acceptance_criteria: 8 concrete assertions

3. Shared Chrome Redesign — Header, StatusBar, Sidebar, PermissionModal ✅
   - Specific: exact visual patterns (▓▓▓, PhaseBreadcrumb, CountdownBar)
   - verify: automated build + test
   - acceptance_criteria: 11 concrete assertions

**Assessment:** Strong plan with detailed action steps. The sparkline duplication issue needs resolution before execution.

### Plan 24-02 — REPL + FirstRun Redesign

**Objective:** Redesign the two highest-impact screens.

**Tasks:**
1. REPL Redesign — Mission Control ✅
   - 6 specific visual changes (gutters, timestamps, tool cards, input frame, git strip, scroll indicator)
   - All changes are visual-only (View() modifications), no state logic changes
   - 9 acceptance criteria

2. FirstRun Redesign — Launchpad ✅
   - Galaxy metaphor, 2x2 feature cards, provider constellation
   - Graceful degradation at 80 cols
   - 8 acceptance criteria

**Assessment:** Clean plan. Tasks are appropriately scoped. Dependencies on 24-01 components (RenderStarfield) are correctly referenced.

### Plan 24-03 — Plan + Execute + Verify Redesign

**Objective:** Redesign three workflow screens with consistent patterns.

**Tasks:**
1. Plan Screen — Blueprint ✅
   - Kanban layout, file impact, dependency graph
   - Tab toggle for graph view (existing showGraph field)
   - 8 acceptance criteria

2. Execute Screen — Mission Live ✅
   - Live metrics bar, progress bar, running task panel
   - Compact task list with status icons and elapsed time
   - 7 acceptance criteria

3. Verify Screen — QA Gate ✅
   - Summary bar, per-task result panels, self-heal overlay
   - 7 acceptance criteria

**Assessment:** Good grouping of related screens. Each task is well-scoped. Existing functionality explicitly preserved.

### Plan 24-04 — Ship + Diff Redesign

**Objective:** Polish the final workflow screens.

**Tasks:**
1. Ship Screen — Launch Pad ✅
   - Commit review, diff summary, action card
   - 8 acceptance criteria

2. Diff Screen Enhancement ✅
   - Syntax highlighting, line numbers, file stats
   - 6 acceptance criteria

**Assessment:** Smallest and cleanest plan. Appropriate scope for 2 screens.

### Plan 24-05 — ModelSelector + Settings + Resume Redesign

**Objective:** Redesign three utility screens.

**Tasks:**
1. ModelSelector — Observatory ✅
   - Sparklines, capability badges, detail pane, favorites
   - References sparkline from Plan 24-01 (correct dependency)
   - 8 acceptance criteria

2. Settings — Control Tower ✅
   - Icon tabs, two-column layout, unsaved indicator
   - 7 acceptance criteria

3. Resume — Session Vault ✅
   - Timeline view, session cards, phase badges, preview pane
   - 8 acceptance criteria

**Assessment:** Good plan. 3 tasks for 3 screens is appropriate scope. References sparkline component from 24-01 correctly.

---

## Conclusion

**Plans are generally well-structured** with:
- ✅ Complete task structure (Files, Action, Verify, Acceptance Criteria, Done)
- ✅ Concrete identifiers in actions (specific patterns, function names, visual elements)
- ✅ No circular dependencies
- ✅ No same-wave file conflicts
- ✅ All requirements covered
- ✅ Context compliance (locked decisions honored, deferred ideas excluded)
- ✅ AGENTS.md constraints respected
- ✅ Bubble Tea single-threaded model respected
- ✅ No full code implementations in action blocks

**One warning requires attention before execution:**
- ⚠️ Sparkline duplication: Plan 24-01 Task 2 proposes creating a file that already exists with a different API. Resolution needed to avoid two competing implementations.

**Recommendation:** Fix the sparkline issue (enhance existing or replace), then plans are ready for execution.
