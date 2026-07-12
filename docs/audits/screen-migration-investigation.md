# Screen Migration Investigation — Router Registration Coverage

**Date:** 2026-07-12  
**Purpose:** Determine exactly which of the 34 screens are registered with the Router and which remain on the legacy switch-based path, to plan remaining migrations.

---

## Summary

| Metric | Value |
|--------|-------|
| Total Screens (enum) | 34 |
| Registered with Router | **14** (41%) |
| Legacy Switch Path | **20** (59%) |

The strangler-fig migration has covered 14 screens. The Router is the new single source of truth for Update/View dispatch — any screen registered with it bypasses the giant switch statements in `app_routing.go` and `app_view.go`.

---

## Complete Screen Registry

### ✅ Registered with Router (14/34)

| Screen | Enum Value | Migration Phase | Notes |
|--------|-----------|-----------------|-------|
| **ScreenConfirmQuit** | 28 | Pilot | First screen migrated; pure modal |
| **ScreenHelp** | 17 | Phase 1 | Self-contained; keyRegistry at init |
| **ScreenHome** | 32 | Phase 1 | Landing screen; cmdRegistry + config |
| **ScreenGhostPicker** | 26 | Phase 1 | Pair with GhostOutput; isolated feature |
| **ScreenGhostOutput** | 27 | Phase 1 | Receives SetResult() from sidebar |
| **ScreenPhaseModelPicker** | 25 | Phase 1 | 7 tests; dual-model selection UI |
| **ScreenDiscuss** | 14 | Phase 1 | Q&A flow; SetTimeout() from nav |
| **ScreenBisect** | 18 | Phase 1 | SetCommits() on nav entry |
| **ScreenDashboard** | 21 | Phase 1 | SetWorkflowState() from nav/phase result |
| **ScreenPlan** | 6 | Phase 1 | SetPlanContent/Version from Engine (mutex-protected) |
| **ScreenSettings** | 3 | Phase 1 | 6-tab editor; shared config pointer with Config |
| **ScreenConfig** | 16 | Phase 1 | Full TOML editor; independent model |
| **ScreenFileExplorer** | 23 | Phase 1 | SetRoot() on nav |
| **ScreenGoalInput** | 13 | Phase 1 | Standalone; single purpose |

**Registration Pattern:** Dual registration in both `app_routing.go` (Update path) and `app_view.go` (View path) — idempotent `router.Register()`.

---

### ❌ Legacy Switch Path (20/34)

| Screen | Enum Value | Why Not Yet Migrated |
|--------|-----------|---------------------|
| **ScreenFirstRun** | 0 | Wizard flow; writes config/keychain |
| **ScreenREPL** | 1 | Core chat; ~1300+ lines; highest coupling (sidebar, streaming, tools, autoDream, chatHistory) |
| **ScreenModelSelector** | 2 | Standalone but complex; async provider model loading |
| **ScreenResume** | 4 | Session browser; couples to SessionDetail |
| **ScreenPermission** | 5 | Modal overlay; not a full screen |
| **ScreenExecute** | 7 | Tight Plan coupling; 10+ workflow handler mutations; live output |
| **ScreenVerify** | 8 | Healing integration; spinner state; manual steps |
| **ScreenShip** | 9 | Demonstration content from phase result |
| **ScreenDiff** | 10 | Standalone viewer |
| **ScreenLedger** | 11 | LoadEntries() on nav |
| **ScreenRollback** | 12 | LoadCommits() on nav; commit chain manager |
| **ScreenMetrics** | 15 | LoadStatsCmd() on nav |
| **ScreenNotifications** | 20 | AddNotification() from toast system |
| **ScreenSessionDetail** | 22 | SetSession() from Resume cursor |
| **ScreenToolDetail** | 24 | SetContent() from sidebar click |
| **ScreenConfirmQuit** | 28 | **ALREADY MIGRATED** (see ✅) |
| **ScreenChatHistory** | 29 | View-time coupling to REPL messages (architectural smell) |
| **ScreenCommandPalette** | 30 | 524 lines; standalone |
| **ScreenRuntimeCheck** | 31 | dev server + smoke tests |
| **ScreenDecisions** | 33 | Decision log browser |

---

## Migration Order Recommendations

### Phase 2: Low Risk / Good Test Coverage
| Screen | Rationale |
|--------|-----------|
| **ScreenCommandPalette** | 524 lines; standalone; no workflow coupling |
| **ScreenFirstRun** | 780 lines; 12 tests; writes config/keychain but isolated |
| **ScreenModelSelector** | 293 lines; 7 tests; async provider loading but clean boundary |
| **ScreenResume** | 212 lines; 8 tests; only couples to SessionDetail (SetSession) |
| **ScreenSessionDetail** | 122 lines; thin wrapper around Resume selection |
| **ScreenLedger** | 156 lines; LoadEntries() on nav; no runtime mutations |
| **ScreenRollback** | 288 lines; 4 tests; LoadCommits() on nav |
| **ScreenMetrics** | 248 lines; LoadStatsCmd() on nav |
| **ScreenNotifications** | 82 lines; AddNotification() from toast; simple |
| **ScreenDiff** | 127 lines; standalone viewer |
| **ScreenToolDetail** | 92 lines; SetContent() from sidebar click |
| **ScreenDecisions** | Decision log browser; standalone |

### Phase 3: High Risk / Core Screens
| Screen | Rationale |
|--------|-----------|
| **ScreenExecute** | Tight Plan coupling; 10+ handler mutations; complex live output |
| **ScreenVerify** | Healing; spinner; manual steps |
| **ScreenShip** | Demonstration from phase result |
| **ScreenChatHistory** | View-time REPL coupling (architectural smell — fix first) |
| **ScreenREPL** | Largest (~1300+ lines); streaming, tools, sidebar, autoDream, chatHistory |
| **ScreenPermission** | Modal, not full screen — may stay on old path |
| **ScreenRuntimeCheck** | dev server + smoke tests |

---

## Architectural Notes

1. **REPL (ScreenREPL)** is the central hub — sidebar pushes token burn, tool calls, messages to it; chatHistory reads its messages at render time. This bidirectional coupling makes it the hardest migration.

2. **Sidebar** is a persistent component, not a screen — it's always visible and receives events from 15+ handlers. It will likely never be a Router screen.

3. **ScreenPermission** is a modal overlay, not a full-screen view. The Router pattern expects full screens; Permission may stay on the old path or get a separate modal router.

4. **ScreenConfirmQuit** was the pilot (Phase 0) — it works correctly as a modal registered with Router.

5. **Shared Config Pointer:** Both SettingsModel and ConfigModel hold `*config.Config`. Edits in one reflect in the other on next render. This works fine with Router since both models are independently registered.

---

## Next Steps

1. Run Phase 2 screens in any order (they're independent)
2. Each Phase 2 screen adds ~1 test file + model + dual registration
3. After Phase 2, only 8 screens remain (Phase 3)
4. Phase 3 requires architectural decisions:
   - REPL + Sidebar may need a "core" router or stay legacy
   - Permission modal needs separate handling
   - Execute/Verify/Ship are workflow-coupled — migrate as a block

---

## Verification Commands

```bash
# Which screens are registered in app_routing.go?
grep -n 'router\.Register' internal/tui/app_routing.go | sed 's/.*Screen/Screen/'

# Which screens are registered in app_view.go?
grep -n 'router\.Register' internal/tui/app_view.go | sed 's/.*Screen/Screen/'

# Which screens still have cases in the Update switch?
grep -n 'case Screen' internal/tui/app_routing.go | grep -v 'router.Register'

# Which screens still have cases in the View switch?
grep -n 'case Screen' internal/tui/app_view.go | grep -v 'router.Register'
```