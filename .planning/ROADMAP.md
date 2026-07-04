# ROADMAP — M31A

## Phase 1: Bug Fixes (Critical + High)

**Goal:** Fix remaining CRITICAL and HIGH severity bugs identified in the codebase audit to ensure stability, safety, and correctness.

**Requirements:** [BUG-04, BUG-05, BUG-06, BUG-07, BUG-08, BUG-09, BUG-10, BUG-12, BUG-14, BUG-15, BUG-16, BUG-17, BUG-19]

**Plans:** 4/4 plans complete

Plans:
- [x] 01-01-PLAN.md — Fix CRITICAL index out of bounds and unsafe type assertion bugs (C4-C7)
- [x] 01-02-PLAN.md — Fix HIGH concurrency bugs (H1-H3, H5)
- [x] 01-03-PLAN.md — Fix HIGH error handling bugs (H7-H9)
- [x] 01-04-PLAN.md — Fix HIGH security and resource bugs (H10, H12)

---

## Phase 2: MEDIUM Bug Fixes

**Goal:** Fix all MEDIUM severity bugs identified in the codebase audit to improve code quality and robustness.

**Requirements:** [BUG-20, BUG-21, BUG-22, BUG-23, BUG-24, BUG-25, BUG-26, BUG-27, BUG-28, BUG-29, BUG-30, BUG-31, BUG-32, BUG-33, BUG-34, BUG-35, BUG-36, BUG-37, BUG-38, BUG-39, BUG-40, BUG-41]

**Plans:** 6 plans

Plans:
- [ ] 02-01-PLAN.md — Fix agent_loop.go: iterator double-close, goroutine leak, unchecked LoadProjectContext (M1, M10, M20)
- [ ] 02-02-PLAN.md — Fix concurrency & safety: runner deadlock, devserver race, edit TOCTOU, bash blocklist (M2, M3, M13, M14)
- [ ] 02-03-PLAN.md — Fix fragile error handling: EOF string, git error, webfetch retry, provider wrapping (M4, M5, M6, M7)
- [ ] 02-04-PLAN.md — Fix unchecked errors: session persistence, filelist nil, webfetch TCPAddr (M8, M9, M19)
- [ ] 02-05-PLAN.md — Fix context & determinism: engine context, compaction, map ordering, slice mutation (M11, M12, M15, M16, M17, M18)
- [ ] 02-06-PLAN.md — Fix performance: O(n²) string concat in execute.go and codemap.go (M21, M22)

---

## Phase 3: LOW Bug Fixes

**Goal:** Fix all LOW severity bugs identified in the codebase audit to improve code quality and maintainability.

**Requirements:** [BUG-42, BUG-43, BUG-44, BUG-45, BUG-46, BUG-47, BUG-48, BUG-49, BUG-50, BUG-51, BUG-52, BUG-53, BUG-54, BUG-55, BUG-56, BUG-57]

**Plans:** 4 plans

Plans:
- [ ] 03-01-PLAN.md — Provider consistency & SSRF security fixes (GAP-P02, GAP-SEC02, GAP-CON02, GAP-CON03)
- [ ] 03-02-PLAN.md — Documentation & config fixes (GAP-U05, GAP-DOC02, GAP-DOC03, GAP-DOC04)
- [ ] 03-03-PLAN.md — Edit tool & code quality fixes (GAP-E03, GAP-PERF01, GAP-CQ02, GAP-CQ03)
- [ ] 03-04-PLAN.md — Performance, build & session fixes (GAP-PERF02, GAP-PERF03, GAP-BR02, GAP-S03)

---

## Phase 4: Technical Debt Elimination

**Goal:** Completely eliminate every technical debt item documented throughout the repository while preserving 100% of existing functionality, APIs, behavior, CLI compatibility, tests, performance characteristics, and architectural principles.

**Requirements:** [TECH-01, TECH-02, TECH-03, TECH-04, TECH-05, TECH-06, TECH-07, TECH-08, TECH-09, TECH-10, TECH-11, TECH-12, TECH-13, TECH-14, TECH-15]

**Plans:** 10/10 plans complete

Plans:
- [x] 04-01-PLAN.md — Engine decomposition: extract PromptBuilder, ContextBuilder, CostTracker (TECH-01)
- [x] 04-02-PLAN.md — Engine decomposition: extract StateMachine, PhaseCoordinator, WorkflowCache (TECH-01)
- [x] 04-03-PLAN.md — TUI refactoring: replace giant Update() switch with modular handlers (TECH-02)
- [x] 04-04-PLAN.md — Config merge, edit tool, token estimator, capability detection (TECH-03, TECH-04, TECH-05, TECH-06)
- [x] 04-05-PLAN.md — Complexity reduction, concurrency review, memory/performance optimization (TECH-07, TECH-08, TECH-09, TECH-10, TECH-11)
- [x] 04-06-PLAN.md — Error handling, logging audit, testing improvement, documentation (TECH-12, TECH-13, TECH-14, TECH-15)
- [x] 04-07-PLAN.md — Gap: Wire orphaned PhaseCoordinator into Engine (TECH-01)
- [x] 04-08-PLAN.md — Gap: Extract app_update.go helper methods to reduce file size (TECH-02)
- [x] 04-09-PLAN.md — Gap: Increase workflow and tools test coverage to 75% (TECH-14)
- [x] 04-10-PLAN.md — Gap: Increase TUI sub-package test coverage (TECH-14)
