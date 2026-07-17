# ROADMAP.md — M31A

## Phase Structure

| Phase | Name | Goal | Dependencies |
|-------|------|------|--------------|
| 1 | Fix TUI Blank Screens | All screens render correctly; first-run → home → REPL works | — |
| 2 | Stabilize Core Workflow | All 7 phases execute end-to-end without crashes | 1 |
| 3 | Headless Modes & Session Resume | `--prompt`, `--goal`, resume on startup work reliably | 2 |
| 4 | Provider Polish & Fallback | Multi-provider with auto-fallback, model caching | 2 |
| 5 | Subagents & Parallel Execution | Child agents with isolated worktrees spawn from parent | 3, 4 |
| 6 | Observability & UX Polish | Metrics, ledger, notifications, dashboard screens | 3 |
| 7 | Release Hardening | Cross-compile, installers, docs, v1.0.0 | 1-6 |
| 9 | 5/10 | In Progress|  |

---

## Phase 1: Fix TUI Blank Screens

**Goal**: All TUI screens render content correctly. User can run `m31a`, complete first-run wizard, reach Home screen, send prompts, see responses in REPL.

**Requirements**: FR-1.1, FR-1.2, FR-1.3, FR-1.4, FR-1.5, FR-1.8, AC-1, AC-5, NFR-1, NFR-2

**Success Criteria**:

- `m31a` starts without "could not open TTY" error in real terminal
- FirstRun wizard displays all 4 steps (welcome, provider select, API key, model pick)
- After wizard, Home screen shows logo, prompt, suggestions, tips
- Typing in prompt and pressing Enter shows streaming response in REPL
- Window resize handled correctly (no blank screen on resize)
- All 30+ screens in app_screens.go render without blank content

**Technical Focus**:

- Debug why View() returns empty in real terminal but tests pass
- Verify WindowSizeMsg handling in Init → Update → View cycle
- Check theme/color issues making text invisible
- Ensure router.Screenable interface implemented for all screens
- Validate contentDimensions() calculations for sidebar + chrome

**Estimated Effort**: 2-3 days (3 plans)

**Plans**:
- Plan 01: Screenable interface audit and ReplModel.SetDimensions implementation
- Plan 02: Theme/color audit, dimension guards, PageChrome/UltraNarrow hardening
- Plan 03: Test infrastructure + manual real-terminal verification (FirstRun→Home→REPL)

---

## Phase 2: Stabilize Core Workflow

**Goal**: All 7 workflow phases execute sequentially with proper state transitions, tool execution, and verification.

**Requirements**: FR-2.1 through FR-2.10, NFR-2

**Success Criteria**:

- `--goal` runs all 7 phases to completion
- Each phase produces expected artifacts (plan, tasks, verification results)
- Self-healing on tool failures works
- Phase transitions require user confirmation (configurable)
- Checkpoint/restore at phase boundaries works

**Technical Focus**:

- Workflow engine phase execution logic
- Tool dispatcher permission flow
- Verification phase test execution
- Runtime phase dev server management
- Git commit / ship phase

**Estimated Effort**: 5-7 days

---

## Phase 3: Headless Modes & Session Resume

**Goal**: Reliable headless operation and session persistence.

**Requirements**: FR-6.1, FR-6.2, FR-6.3, FR-5.1, FR-5.2, FR-5.3, FR-5.4, AC-3, AC-4

**Success Criteria**:

- `--prompt` works with all 3 providers
- `--goal` completes full workflow headless
- ResumeOnStartup restores last session correctly
- Session browser lists, searches, exports sessions
- Sessions survive crashes (autosave on shutdown)

**Estimated Effort**: 3-4 days

---

## Phase 4: Provider Polish & Fallback

**Requirements**: FR-3.1 through FR-3.5

**Success Criteria**:

- Automatic fallback when primary provider fails
- Model cache refresh works (stale-while-revalidate)
- Health checks run periodically
- Cost tracking accurate per phase
- Keychain integration works on Linux/macOS/Windows

**Estimated Effort**: 2-3 days

---

## Phase 5: Subagents & Parallel Execution

**Requirements**: FR-4.4

**Success Criteria**:

- Parent can spawn child agents via Agent tool
- Children have isolated git worktrees
- Children have own dispatcher + tool access
- Results merged back to parent session
- No deadlocks or resource leaks

**Estimated Effort**: 4-5 days

---

## Phase 6: Observability & UX Polish

**Requirements**: FR-1.5, FR-1.6, FR-1.8, FR-5.4, NFR-1

**Success Criteria**:

- Dashboard screen shows pipeline overview
- Metrics screen shows token usage, costs, timing
- Ledger browser shows decision history
- Notifications screen captures all toasts
- Smooth 60fps animations on transitions
- Sidebar auto-hide on narrow terminals

**Estimated Effort**: 3-4 days

---

## Phase 7: Release Hardening

**Requirements**: NFR-1, NFR-2, NFR-3, NFR-4

**Success Criteria**:

- Cross-compiles to all 5 targets
- Goreleaser produces .tar.gz, .deb, .rpm, Homebrew formula
- Static binary verified (no CGO)
- Man pages + markdown docs generated
- README + CONTRIBUTING updated
- v1.0.0 tagged

**Estimated Effort**: 2-3 days

### Phase 8: Investigate and fix TUI blank screens issue - all screens render empty when running the binary

**Goal:** All TUI screens render content correctly. User can run `m31a`, complete first-run wizard, reach Home screen, send prompts, see responses in REPL.

**Requirements**: FR-1.1, FR-1.2, FR-1.3, FR-1.4, FR-1.5, FR-1.8, AC-1, AC-5, NFR-1, NFR-2
**Depends on:** —
**Plans:** 3 plans

Plans:

- [x] 08-01-PLAN.md — Screenable interface audit and ReplModel.SetDimensions implementation
- [x] 08-02-PLAN.md — Theme/color audit, dimension guards, PageChrome/UltraNarrow hardening
- [x] 08-03-PLAN.md — Test infrastructure + manual real-terminal verification (FirstRun→Home→REPL)

---

## Phase 9: Architecture Upgrade & Directory Restructuring

**Goal**: Reorganize code layout, refactor internal architecture, and establish cleaner module boundaries to improve maintainability and reduce coupling.

**Requirements**: NFR-4 (Maintainability)

**Success Criteria**:
- `pkg/` → `internal/` migration complete (17 packages)
- `internal/types/` alias layer removed
- TUI screens in `internal/tui/screens/<name>/` (34 sub-packages)
- Tools grouped into `fileops/`, `exec/`, `search/`, `ai/` (4 domains)
- Workflow engine split into `engine/`, `phases/`, `streaming/`
- TUI handlers grouped into `handlers/` sub-package
- Interface-driven boundaries: WorkflowEngine, ToolExecutor, ProviderRegistry
- Constructor injection at composition root (main.go)
- All quality gates pass: `make check`, coverage ≥ 75%, import graph clean

**Estimated Effort**: 3-4 days

**Plans** (10 plans, each independently compilable/testable):
- Plan 01: Type Layering Cleanup — Remove internal/types/ alias layer
- Plan 02: Package Reorganization — Move pkg/ to internal/
- Plan 03: TUI Screen Sub-Packages — Extract 34 screens to internal/tui/screens/
- Plan 04: Tool Domain Grouping — Group tools into fileops/, exec/, search/, ai/
- Plan 05: Workflow Engine Decomposition — Split into engine/, phases/, streaming/
- Plan 06: TUI Handler Grouping — Consolidate handlers into handlers/ sub-package
- Plan 07: Interface Extraction — WorkflowEngine + TUI-facing interfaces
- Plan 08: Tool & Provider Interfaces — ToolExecutor, ProviderRegistry
- Plan 09: Constructor Injection — Wire dependencies at composition root
- Plan 10: Final Verification — Phase gate validation

### Phase 10: Fix test errors from architecture upgrade

**Goal:** Fix test failures introduced by Phase 9 restructuring and centralize test helpers/fixtures in `internal/testutil/`. This phase reorganizes test infrastructure — it does not add new test coverage or change test behavior.
**Requirements**: NFR-4 (Maintainability)
**Depends on:** Phase 9
**Plans:** 3 plans

Plans:
- [ ] 10-01-PLAN.md — Fix build errors: fileops field name mismatch, session test temp dirs
- [ ] 10-02-PLAN.md — Centralize shared mocks and test builders in testutil/
- [ ] 10-03-PLAN.md — Create fixtures with go:embed, move e2e/integration tests to testutil/

---

## Cross-Phase Notes

- **Phase 1 is blocking** — all subsequent phases need working TUI for verification
- **Phases 2-3 can parallelize** after Phase 1 (workflow engine vs headless/resume)
- **Phase 4-5 depend on Phase 2** (provider integration, subagents need workflow)
- **Phase 6 depends on Phases 2-4** (observability needs real data)
- **Phase 7 is final integration**

## Risk Mitigation

| Risk | Mitigation |
|------|------------|
| TUI blank screens are architectural | Spike minimal Bubble Tea app to isolate issue |
| Workflow engine has hidden deadlocks | Add timeout/cancellation to all phase transitions |
| Provider APIs change | Dynamic model discovery, capability detection |
| Keychain fails on CI/headless | Graceful fallback to config file (encrypted) |
| Subagent worktree conflicts | Unique branch names, cleanup on exit |

## Canonical References

- `AGENTS.md` — build/test/lint commands, architecture rules
- `.planning/codebase/ARCHITECTURE.md` — system design
- `.planning/codebase/STACK.md` — dependencies
- `.planning/codebase/CONVENTIONS.md` — code style

EOF

### Phase 09.1: Fix test errors from architecture upgrade (INSERTED)

**Goal:** [Urgent work - to be planned]
**Requirements**: TBD
**Depends on:** Phase 9
**Plans:** 0 plans

Plans:
- [ ] TBD (run /gsd-plan-phase 09.1 to break down)
