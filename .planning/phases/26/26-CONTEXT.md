# Phase 26 — TUI Screen Fixes & Missing Implementations

> **Status:** Ready for execution
> **Created:** 2026-06-07
> **Source:** `rush/TUI_SCREEN_DETAILED_REPORT.md` (3 missing screens, 7 moderate wiring issues, 3 minor issues)

---

## Goal

Fix all broken TUI screens, implement the 3 missing screens (Ledger, Rollback, Discuss), and resolve all wiring issues identified in the TUI screen audit. After this phase, all 16 declared screen types will be fully functional with correct routing, state management, and user feedback.

---

## Scope

### Missing Screens (HIGH priority)
| ID | Screen | Enum | Issue |
|----|--------|------|-------|
| MISS-01 | ScreenLedger | 11 | Declared but no implementation — `/ledger` command dead-ends |
| MISS-02 | ScreenRollback | 12 | Declared but no implementation — `/rollback` command dead-ends |
| MISS-03 | ScreenDiscuss | 14 | No dedicated screen — Q&A flows through REPL without progress tracking |

### Wiring Issues (MEDIUM priority)
| ID | Screen | Issue |
|----|--------|-------|
| WIR-01 | ScreenPlan | Cost estimation is placeholder: `fmt.Sprintf("%d tasks", len(tasks))` |
| WIR-02 | ScreenExecute | `paused` field exists but pause/resume not wired to workflow engine |
| WIR-03 | ScreenVerify | `healFunc` calls `HealTask()` but return value ignored — re-verification not triggered |

### Low Priority Fixes
| ID | Screen | Issue |
|----|--------|-------|
| WIR-04 | ScreenDiff | Split view toggle mentioned in comments but only unified view implemented |
| WIR-05 | ScreenModelSelector | `generateMockUsageData()` placeholder — latency stats fabricated |
| WIR-06 | ScreenGoalInput | Goal input → workflow start skips `PhaseInitialize` (project type detection) |
| WIR-07 | ScreenMetrics | Daily usage is text-only — no sparkline visualization |

### Minor Fixes
| ID | Issue |
|----|-------|
| MIN-01 | `defaultSidebarWidth = 120` hardcoded — should be configurable |
| MIN-02 | `frecentHistory` max size not configurable |
| MIN-03 | Permission queue could stall if dispatcher channel is full |

---

## Plans

```
Plans:
- [ ] 26-01-PLAN.md — ScreenLedger Implementation (Wave 1, MISS-01)
- [ ] 26-02-PLAN.md — ScreenRollback Implementation (Wave 1, MISS-02)
- [ ] 26-03-PLAN.md — ScreenDiscuss Implementation (Wave 1, MISS-03)
- [ ] 26-04-PLAN.md — Plan/Execute/Verify Wiring Fixes (Wave 2, WIR-01/WIR-02/WIR-03)
- [ ] 26-05-PLAN.md — Diff/ModelSelector/GoalInput/Metrics Fixes (Wave 2, WIR-04..WIR-07)
- [ ] 26-06-PLAN.md — Minor Polish: Sidebar Width, Frecent History, Permission Queue (Wave 3, MIN-01..MIN-03)
```

---

## Wave Structure

| Wave | Plans | Autonomous | Depends on |
|------|-------|------------|------------|
| 1 | 26-01, 26-02, 26-03 | yes, yes, yes | — |
| 2 | 26-04, 26-05 | yes, yes | Wave 1 |
| 3 | 26-06 | yes | Wave 2 |

---

## Critical Architecture Rules (from AGENTS.md)

- **Bubble Tea is single-threaded.** All state mutations go through `Update()` only. Never mutate `AppState` from a goroutine.
- **No new packages.** All screens live in `internal/tui/`. Components in `internal/tui/components/`.
- **Theme via `theme.Manager`.** All colors from `theme.Theme` struct — no raw hex strings.
- **Existing patterns:** Follow `plan.go`, `execute.go`, `verify.go`, `ship.go` for screen structure.
- **`pkg/ledger/`** already has `Ledger`, `Entry`, `Stats`, `Query` types — screen wraps these.
- **`pkg/rollback/`** already has `Rollback`, `SessionCommits`, `SoftReset`, `HardReset` types — screen wraps these.

---

## Deliverables

- All 16 screen types fully implemented and routable
- `/ledger` opens ScreenLedger with filterable session history
- `/rollback` opens ScreenRollback with commit list and diff preview
- Discuss phase has dedicated screen with progress indicator and timeout
- Plan screen shows real cost estimates from provider pricing
- Execute screen pause/resume wired to workflow engine
- Verify screen self-heal triggers re-verification
- Diff screen supports split view toggle
- ModelSelector uses real latency data (or removes mock)
- GoalInput routes through PhaseInitialize before Discuss
- Metrics screen uses sparkline component for daily usage
- Sidebar width configurable via config
- `go test -race ./...` passes
- `CGO_ENABLED=0 go build` succeeds
