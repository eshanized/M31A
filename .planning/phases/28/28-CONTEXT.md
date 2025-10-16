# Phase 28 — Duplicate Code Consolidation — Context

**Gathered:** 2026-06-07
**Status:** Ready for planning
**Source:** rush/duplicate_code_audit_report.md

<domain>
## Phase Boundary

Eliminate all 27 duplicate code patterns identified in the Duplicate Code Audit Report. Consolidate repeated logic into shared helpers, constants, and components. Reduce codebase maintenance surface by ~500 lines of duplicated code.

</domain>

<decisions>
## Implementation Decisions

### D-01: Wave-Based Execution
Organize fixes into 4 waves by dependency and risk:
- Wave 1: Independent helpers with zero downstream dependencies (makeAssistantMsg, syncReplProvider, listenerCmds, slash autocomplete removal, progress bars)
- Wave 2: Session/model lifecycle helpers (loadAndRestoreSession, propagateSessionID, ensureReplModel, ensureSidebarModel, DoubleBorder)
- Wave 3: Constants, computed values, and rendering helpers (hardcoded literals, replWidth, contentWidth, logo, search bar, theme styles, skip dirs, git config, formatDuration, section headers)
- Wave 4: Large-scale replacements and low-severity items (CenterScreen, duplicate constants, naming collisions, footer patterns, routing guards, diff styles, offline message)

### D-02: No Behavioral Changes
All fixes are pure refactors — extract helper, replace call sites, verify identical behavior. No feature changes, no new functionality.

### D-03: Test-First Verification
Each wave must pass `go test -race -cover ./...` before proceeding to the next. The existing 537 tests serve as the regression suite.

### D-04: Import Cycle Avoidance
M-1 (duplicate constants between types/ and tools/): Accept the duplication with a shared comment. Moving constants would create import cycles. Document the decision.

### D-05: Atomic Commits
Each extracted helper gets its own commit.便于 bisection if regressions appear.

### the agent's Discretion
- Exact helper signatures beyond what the audit specifies
- Whether to extract additional micro-helpers during implementation
- File placement for new shared code (components/ vs tui/ vs types/)

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Audit Report
- `rush/duplicate_code_audit_report.md` — Complete list of 27 duplicate patterns with file:line references

### Existing Components to Reuse
- `internal/tui/components/progress.go` — ProgressBar component (C-1 target)
- `internal/tui/theme/colors.go` — Theme definitions, DoubleBorder candidate (H-7)
- `internal/types/constants.go` — Shared constants (M-1, M-2 targets)

### Files with Highest Duplicate Density
- `internal/tui/app_update_screen.go` — 16+ duplicate sites (H-1, H-2, H-4, H-5, H-6, H-8)
- `internal/tui/app_update_workflow.go` — 8+ duplicate sites (H-1, H-5, H-6, H-8)
- `internal/tui/app_state.go` — 6+ duplicate sites (H-1, H-6)
- `internal/tui/repl.go` — 3 duplicate sites (H-3, M-4)
- `internal/tui/repl_keys.go` — 2 duplicate sites (H-3, M-3)

### Test Files (must not break)
- `internal/tui/app_test.go` (2642 lines)
- `internal/tui/repl_test.go` (1023 lines)
- `internal/tui/settings_test.go` (957 lines)
- `internal/tui/commands_test.go` (2227 lines)

</canonical_refs>

<specifics>
## Specific Ideas

### Wave 1 — Immediate Helpers (1-2 hours)
1. **M-3: makeAssistantMsg(content string) types.Message** — eliminates 9 instances of error message boilerplate
2. **H-1: m.syncReplProvider(sessionID string) tea.Cmd** — eliminates 12 instances of SetProvider+SetDispatcher+SetCommandRegistry
3. **H-8: m.listenerCmds() []tea.Cmd** — eliminates 20+ instances of permission+question listener command creation
4. **H-3: m.updateSlashSuggestions()** — removes duplicate 43-line block in repl_keys.go (repl.go already has it)
5. **C-1: Replace hand-rolled progress bars** — 9+ locations → components.ProgressBar

### Wave 2 — Session/Model Lifecycle (half day)
6. **H-2: m.loadAndRestoreSession(sessionID string, clearExisting bool) tea.Cmd** — 2 near-identical session loading blocks
7. **H-4: m.propagateSessionID(id string)** — 2 identical 4-line blocks
8. **H-6: m.ensureReplModel()** — 5+ nil-check+assignment blocks
9. **H-5: m.ensureSidebarModel()** — 2 identical nil-check blocks
10. **H-7: theme.DoubleBorder** — 4 identical lipgloss.Border struct literals

### Wave 3 — Constants & Computed Values (half day)
11. **M-2: Hardcoded literals → constants** — 15+ hardcoded values (300, 0.80, 0.3, URLs, date formats)
12. **M-4: ReplModel.replWidth()** — 3 identical calculation blocks
13. **M-5: calcContentWidth(width int) int** — 3 identical blocks in components/message.go
14. **M-6: Shared logo renderer** — 2 functions with optional bold param
15. **M-7: renderSearchBar with configurable label** — 2 near-identical functions
16. **M-8: applyThemeStyles(t *Theme)** — ~50 duplicated lines in Dark()/Light()
17. **M-9: SkipDirsList** — 2 data structures with same 9 values
18. **M-10: config.DefaultGitConfig()** — 2 identical struct literals
19. **M-11: Rename formatDuration variants** — 3 confusingly similar names
20. **M-12: RenderSectionHeader(title string, width int)** — 6+ header-bar patterns

### Wave 4 — Large-Scale & Low Severity (1 day)
21. **H-9: CenterScreen(content string, w, h int) string** — 30 identical centering calls
22. **L-1: Rename renderProviderCard** — naming collision between ReplModel and FirstRunModel
23. **L-2: Shared footer/hint renderer** — 4 similar but not identical functions
24. **L-3: Workflow phase routing table** — 3 identical case blocks
25. **L-4: diffLineStyle(lineType, theme)** — 2 identical switch blocks in diff_view.go
26. **L-5: const offlineModeMsg** — 4 identical error strings
27. **M-1: Document duplicate constants decision** — accept import cycle workaround

</specifics>

<deferred>
## Deferred Ideas

- Creating a base `dimensions` struct for WindowSizeMsg (architectural, out of scope for this phase)
- Embedded `spinnerable` struct for spinner tick boilerplate (architectural, out of scope)
- Splitting test files (not needed)
- New sub-packages (would require import path changes)

</deferred>

---

*Phase: 28-duplicate-code-consolidation*
*Context gathered: 2026-06-07 via audit report analysis*
