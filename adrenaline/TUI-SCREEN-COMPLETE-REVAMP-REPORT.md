# M31A TUI Complete Screen & Component Revamp Report

**Date:** 2026-06-10
**Scope:** Full audit of 95+ `.go` files across `internal/tui/`, `internal/tui/components/`, `internal/tui/theme/`, `internal/tui/layout/`, `internal/types/`, `internal/workflow/`
**Methodology:** Line-by-line source review of every screen model, routing path, keybinding, command, component, and state management pattern

---

## Table of Contents

1. [Executive Summary](#1-executive-summary)
2. [Complete Screen Inventory](#2-complete-screen-inventory)
3. [Screen Wiring & Routing Map](#3-screen-wiring--routing-map)
4. [Missing Screens](#4-missing-screens)
5. [Broken & Degraded Screens](#5-broken--degraded-screens)
6. [Component Inventory & Gaps](#6-component-inventory--gaps)
7. [State Management Gaps](#7-state-management-gaps)
8. [Navigation & Keybinding Gaps](#8-navigation--keybinding-gaps)
9. [Command System Gaps](#9-command-system-gaps)
10. [Screen-by-Screen Revamp Plan](#10-screen-by-screen-revamp-plan)
11. [New Screen Specifications](#11-new-screen-specifications)
12. [Component Revamp Plan](#12-component-revamp-plan)
13. [Implementation Priority Matrix](#13-implementation-priority-matrix)

---

## 1. Executive Summary

M31A has **17 defined screens** (Screen constants 0-16), but only **7 are fully functional**. The remaining 10 are broken, degraded, or unreachable. Additionally, at least **8 screens are missing** that the architecture and ROADMAP imply should exist.

**Screen Health Summary:**

| Status | Count | Screens |
|--------|-------|---------|
| Fully Working | 7 | REPL, FirstRun, Settings, ModelSelector, Config, Diff, GoalInput |
| Broken (won't load data) | 3 | Ledger, Rollback, Discuss |
| Degraded (partial function) | 4 | Plan, Execute, Verify, Ship |
| Broken routing | 1 | Resume |
| Unreachable (dead code) | 1 | Help (model exists, no screen constant) |
| Partially broken | 1 | Metrics (resize issues) |

**Component Health:** 28 component files exist, but many are under-utilized or have dead code. 12+ critical components are missing for a premium TUI experience.

**Total issues found:** 47 bugs + 8 missing screens + 12 missing components + 23 state management gaps

---

## 2. Complete Screen Inventory

### 2.1 Defined Screens (types.go:17-35)

| # | Constant | Label | Model Type | Model File | View File | Status |
|---|----------|-------|------------|------------|-----------|--------|
| 0 | `ScreenFirstRun` | Setup | `FirstRunModel` | `firstrun_model.go` | `firstrun_view.go` | Working |
| 1 | `ScreenREPL` | Chat | `ReplModel` | `repl_model.go` | `repl_view.go` | Working |
| 2 | `ScreenModelSelector` | Models | `ModelSelector` | `modelselector.go` | `modelselector_view.go` | Working |
| 3 | `ScreenSettings` | Settings | `SettingsModel` | `settings_model.go` | `settings_view.go` | Working |
| 4 | `ScreenResume` | Sessions | `ResumeModel` | `resume_model.go` (implicit) | `resume_view.go` (implicit) | **Broken** |
| 5 | `ScreenPermission` | Permission | `PermissionModal` | `components/permission.go` | modal overlay | Working |
| 6 | `ScreenPlan` | Plan | `PlanModel` | `plan_model.go` | `plan_view.go` | **Degraded** |
| 7 | `ScreenExecute` | Execute | `ExecuteModel` | `execute_model.go` | `execute_view.go` | **Degraded** |
| 8 | `ScreenVerify` | Verify | `VerifyModel` | `verify.go` | `verify.go` (inline) | **Degraded** |
| 9 | `ScreenShip` | Ship | `ShipModel` | `ship_model.go` | `ship_view.go` | **Degraded** |
| 10 | `ScreenDiff` | Diff | `DiffModel` | `diff_model.go` | `diff_view.go` | Working |
| 11 | `ScreenLedger` | Ledger | `LedgerModel` | `ledger.go` | `ledger.go` (inline) | **Broken** |
| 12 | `ScreenRollback` | Rollback | `RollbackModel` | `rollback.go` | `rollback.go` (inline) | **Broken** |
| 13 | `ScreenGoalInput` | Goal | `GoalInputModel` | `goalinput.go` | `goalinput.go` (inline) | Working |
| 14 | `ScreenDiscuss` | Discuss | `DiscussModel` | `discuss_model.go` (implicit) | `discuss_view.go` (implicit) | **Broken** |
| 15 | `ScreenMetrics` | Metrics | `MetricsModel` | `metrics.go` | `metrics.go` (inline) | **Degraded** |
| 16 | `ScreenConfig` | Config | `ConfigModel` | `config_model.go` | `config_model.go` (inline) | Working |

### 2.2 Undefined But Implemented

| Model | File | Lines | Issue |
|-------|------|-------|-------|
| `HelpModel` | `help.go` | 232 | No `ScreenHelp` constant, no `helpModel` field in `AppState`, no routing path |

### 2.3 AppState Sub-model Fields (app_state.go:96-114)

```
replModel     *ReplModel          ✓ used
sidebarModel  *SidebarModel       ✓ used
cmdPalette    *CommandPaletteModel ✓ used
planModel     *PlanModel          ✓ used
executeModel  *ExecuteModel       ✓ used
verifyModel   *VerifyModel        ✓ used
shipModel     *ShipModel          ✓ used
settingsModel *SettingsModel      ✓ used
resumeModel   *ResumeModel        ✓ used
msModel       *ModelSelector      ✓ used
firstRunModel *FirstRunModel      ✓ used
goalInput     *GoalInputModel     ✓ used
ledgerModel   *LedgerModel        ✓ used
rollbackModel *RollbackModel      ✓ used
discussModel  *DiscussModel       ✓ used
diffModel     *DiffModel          ✓ used
metricsModel  *MetricsModel       ✓ used
configModel   *ConfigModel        ✓ used
```

**Missing from AppState:**
- `helpModel *HelpModel` — implemented but not wired
- `bisectModel` — no bisect screen exists
- `themePickerModel` — no theme picker screen exists
- `notificationCenterModel` — no notification center exists

---

## 3. Screen Wiring & Routing Map

### 3.1 Full Routing Path Analysis

```
AppState.Update()
  ├── tea.KeyMsg → routeKeyMsg()
  │     ├── cmdPalette open? → cmdPalette.Update()
  │     ├── pendingConfirm? → y/n/esc
  │     ├── ctrl+g → sidebar.ToggleFocus()
  │     ├── sidebar focused? → sidebarModel.HandleKey()
  │     ├── leader key → keyRegistry.Handle()
  │     └── screen switch:
  │           ├── ScreenREPL → replModel.Update()
  │           ├── ScreenPermission → handlePermissionKey()
  │           ├── ScreenModelSelector → msModel.Update()
  │           ├── ScreenSettings → settingsModel.Update()
  │           ├── ScreenResume → resumeModel.Update()
  │           ├── ScreenPlan → planModel.Update()
  │           ├── ScreenExecute → executeModel.Update()
  │           ├── ScreenVerify → verifyModel.Update()
  │           ├── ScreenShip → shipModel.Update()
  │           ├── ScreenDiscuss → discussModel.Update()
  │           ├── ScreenDiff → diffModel.Update()
  │           ├── ScreenGoalInput → goalInput.Update()
  │           ├── ScreenFirstRun → firstRunModel.Update()
  │           ├── ScreenLedger → ledgerModel.Update()
  │           ├── ScreenRollback → rollbackModel.Update()
  │           ├── ScreenConfig → configModel.Update()
  │           └── ScreenMetrics → esc/q only (NO forwarding)
  │
  ├── AppMsg → handleAppMsg()
  │     ├── ModelSelected → ScreenREPL
  │     ├── Screen/Action → routeAppMsgAction()
  │     └── SessionID → loadAndRestoreSession()
  │
  ├── SlashCommandMsg → handleSlashCommand()
  │     ├── ! prefix → shell command
  │     ├── / prefix → cmdRegistry.Execute()
  │     └── else → sendChatMessage()
  │
  ├── PhaseResultMsg → handlePhaseResult()
  │     ├── PhaseInitialize → PhaseDiscuss
  │     ├── PhaseDiscuss → ScreenDiscuss or PhasePlan
  │     ├── PhasePlan → ScreenPlan
  │     ├── PhaseExecute → ScreenVerify
  │     ├── PhaseVerify → ScreenShip
  │     └── PhaseShip → ScreenShip
  │
  └── default: → forward to sub-models
        ├── ScreenModelSelector → msModel.Update()
        ├── ScreenSettings → settingsModel.Update()
        ├── ScreenResume → resumeModel.Update()
        └── ScreenDiscuss → discussModel.Update()
        ❌ MISSING: ScreenMetrics, ScreenGoalInput, ScreenLedger,
           ScreenRollback, ScreenConfig, ScreenDiff, ScreenShip,
           ScreenPlan, ScreenExecute, ScreenVerify
```

### 3.2 ensureSubModel Coverage (app_update.go:855-939)

| Screen | ensureSubModel | Creates Model? | Calls Init()? |
|--------|---------------|----------------|---------------|
| ScreenREPL | ✓ | ✓ (ensureReplModel) | via routeToScreen |
| ScreenModelSelector | ✓ | ✓ | ✓ |
| ScreenSettings | ✓ | ✓ | ✓ |
| ScreenResume | ✓ | delegates to openResumeScreen | via async load |
| ScreenGoalInput | ✓ | ✓ | ✓ |
| ScreenPlan | ✓ | resize only | ✗ |
| ScreenExecute | ✓ | resize only | ✗ |
| ScreenVerify | ✓ | resize only | ✗ |
| ScreenShip | ✓ | resize only | ✗ |
| ScreenLedger | ✓ | ✓ | ✗ (LoadEntries not called) |
| ScreenRollback | ✓ | ✓ | ✗ (LoadCommits not called) |
| ScreenMetrics | ✓ | ✓ | via LoadStats |
| ScreenConfig | ✓ | ✓ | ✗ |
| ScreenDiscuss | ❌ | **MISSING** | **MISSING** |
| ScreenDiff | ❌ | handled by DiffScreenMsg | ✗ |
| ScreenFirstRun | via routeToScreen | ✓ | ✓ |

### 3.3 renderActiveScreen Coverage (app_view.go:234-271)

All 17 screens have render functions. However, several return loading placeholders permanently due to broken initialization (Ledger, Rollback).

### 3.4 handleWindowResize Coverage (app_update.go:500-577)

**Resized in handleWindowResize:**
- replModel ✓
- planModel ✓
- executeModel ✓
- verifyModel ✓
- metricsModel ✓
- settingsModel ✓
- cmdPalette ✓
- msModel ✓
- resumeModel ✓
- diffModel ✓

**NOT resized (critical gaps):**
- goalInput ❌
- ledgerModel ❌
- rollbackModel ❌
- shipModel ❌
- firstRunModel ❌
- configModel ❌
- discussModel ❌

---

## 4. Missing Screens

### 4.1 Help Screen (SCREEN-01)

**Status:** Model fully implemented (232 lines in `help.go`) but completely unreachable.

**Missing:**
- `ScreenHelp` constant in `types.go`
- `helpModel *HelpModel` field in `AppState`
- Case in `ensureSubModel()`
- Case in `renderActiveScreen()`
- Case in `routeKeyMsg()`
- Keybinding `?` to open help from REPL
- Slash command `/help` should navigate to screen instead of printing text

**Files to modify:** `types.go`, `app_state.go`, `app_update.go`, `app_view.go`, `keybindings_screens.go`, `commands_core.go`

### 4.2 Bisect Screen (SCREEN-02)

**Status:** `pkg/bisect/` package exists with full bisect logic, `/bisect` command exists but only prints usage text. No TUI screen.

**Need:**
- `ScreenBisect` constant
- `BisectModel` with commit list, good/bad marking, progress display
- Integration with `pkg/bisect` for interactive bisect workflow
- Key handlers: `g` (mark good), `b` (mark bad), `s` (skip), `r` (reset)

### 4.3 Theme Picker Screen (SCREEN-03)

**Status:** 10 theme palettes registered in `theme/registry.go` (Catppuccin, Nord, Tokyo Night, Gruvbox, Rose Pine, Dracula, Solarized, Monochrome + Dark/Light). No picker screen to browse and preview them.

**Need:**
- `ScreenThemePicker` constant
- `ThemePickerModel` with:
  - Grid of theme swatches (background + brand + text colors)
  - Live preview: applying the hovered theme to a sample card/badge/progress bar
  - Selection persists to config
- Integration with `/theme` command

### 4.4 Notification Center Screen (SCREEN-04)

**Status:** Toast system exists (up to 3 visible, queue overflow) but no way to review past notifications. No notification history.

**Need:**
- `ScreenNotifications` constant
- `NotificationCenterModel` with scrollable history of all toasts
- Persistence of notification history (optional, in-memory is fine for V1)
- Keybinding `ctrl+x !` or similar

### 4.5 Workflow Dashboard Screen (SCREEN-05)

**Status:** Workflow has 6 phases but no overview screen showing the full pipeline with current state.

**Need:**
- `ScreenDashboard` constant
- `DashboardModel` showing:
  - Phase pipeline visualization: `Initialize → Discuss → Plan → Execute → Verify → Ship`
  - Current phase highlighted with brand color
  - Completed phases with checkmarks
  - Each phase clickable to navigate to its screen
  - Goal display, elapsed time, cost so far

### 4.6 Session Detail Screen (SCREEN-06)

**Status:** Resume screen shows session list but no detail view before loading.

**Need:**
- `ScreenSessionDetail` constant
- `SessionDetailModel` showing:
  - Session goal, model, provider, phase, message count
  - First few messages as preview
  - Cost and token stats
  - Actions: Resume, Fork, Export, Delete

### 4.7 File Explorer Screen (SCREEN-07)

**Status:** Sidebar shows git-changed files only. No full file tree browser.

**Need:**
- `ScreenFileExplorer` constant
- `FileExplorerModel` showing:
  - Tree view of project files (respecting `SkipDirs`)
  - File preview in right pane
  - `@` mention integration: selecting a file inserts it as mention
  - Keybinding: `ctrl+x f`

### 4.8 Tool Output Detail Screen (SCREEN-08)

**Status:** Tool cards in REPL show truncated output. No way to expand to full output.

**Need:**
- Expandable tool card modal or dedicated screen
- Full output with syntax highlighting
- Scroll, search within output
- Copy output to clipboard

---

## 5. Broken & Degraded Screens

### 5.1 CRITICAL: Resume Screen — Session Never Restored (BUG-01)

**File:** `app_update.go:751-767`

`handleAppMsg()` checks `msg.Screen != 0` before `msg.SessionID != ""`. Since `ScreenREPL = 1`, the session ID path is never reached.

**Fix:** Check `msg.SessionID != ""` first.

### 5.2 CRITICAL: Discuss Screen — Questions Never Populated (BUG-02)

**File:** `app_update_phase.go:43-54`

`m.discussQuestions` is never populated from `PhaseResultMsg`. The discuss model renders "No questions to answer" and immediately completes.

**Fix:** Add `Questions []string` field to `PhaseResultMsg`, populate from `workflowEngine.DiscussState()` before creating model.

### 5.3 CRITICAL: Ledger Screen — LoadEntries Never Called (BUG-03)

**File:** `app_update.go:907-912`

`ensureSubModel` creates `LedgerModel` but never calls `LoadEntries()`. View shows "Loading..." permanently.

**Fix:** Call `m.ledgerModel.LoadEntries()` after creation.

### 5.4 CRITICAL: Rollback Screen — LoadCommits Never Called (BUG-04)

**File:** `app_update.go:913-916`

Same pattern: `ensureSubModel` creates `RollbackModel` but never calls `LoadCommits()`. View shows "No commits found" permanently.

**Fix:** Call `m.rollbackModel.LoadCommits()` after creation.

### 5.5 CRITICAL: Help Screen — Fully Implemented but Unreachable (BUG-05)

**File:** `help.go` (232 lines)

No `ScreenHelp`, no `helpModel` field, no navigation path. 232 lines of dead code.

**Fix:** Wire as described in SCREEN-01.

### 5.6 HIGH: DiscussModel.Init() Never Called (BUG-06)

**File:** `app_update_phase.go:50-55`

`ensureSubModel` has no `ScreenDiscuss` case. `DiscussModel.Init()` is never called, so the textinput cursor never blinks.

**Fix:** Add `ScreenDiscuss` case to `ensureSubModel`.

### 5.7 HIGH: Metrics Screen — No Message Forwarding (BUG-07)

**File:** `app_update.go:437-469`

The `default:` case in `Update()` forwards messages for 4 screens but not `ScreenMetrics`. Metrics screen never receives non-key messages after creation.

**Fix:** Add `ScreenMetrics` to the `default:` forwarding block.

### 5.8 HIGH: Plan Screen — Stale Tasks on Initial Load (BUG-08)

**File:** `app_update_phase.go:60-79`

When discuss completes without answers, `PlanModel` is created with discuss phase tasks (empty). Plan phase runs and produces real tasks, but the model may not update properly.

**Fix:** Ensure `m.planModel.UpdateTasks(msg.Tasks)` is called in `PhasePlan` result handler.

### 5.9 HIGH: GoalSubmittedMsg Always Restarts from PhaseInitialize (BUG-09)

**File:** `app_update.go:299-303`

`runWorkflowFromGoal()` always starts from `PhaseInitialize`, ignoring `ResumePhase` from `/resume-task`.

**Fix:** Accept a starting phase parameter in `runWorkflowFromGoal`.

### 5.10 HIGH: Theme Not Propagated to 7+ Models (BUG-10)

**File:** `app_update.go:1021-1064`

`applyTheme()` misses: `diffModel`, `goalInput`, `ledgerModel`, `rollbackModel`, `configModel`, `firstRunModel`, `msModel`.

**Fix:** Add theme propagation for all missing models.

### 5.11 MEDIUM: Ship Screen — No esc/q Handler (BUG-11)

**File:** `ship_model.go:66-100`

No `esc` or `q` handler. User must press `enter` or `n` to leave. Inconsistent with all other screens.

**Fix:** Add `case "esc", "q":` to return to REPL.

### 5.12 MEDIUM: DiffModel Viewport Created with height=0 (BUG-12)

**File:** `app_update.go:383-396`

`SetDiff()` creates viewport using `dm.height` before `dm.height` is set. Viewport is always 3 rows until resize.

**Fix:** Set `width` and `height` BEFORE calling `SetDiff`.

### 5.13 MEDIUM: popScreen() Never Called (BUG-13)

**File:** `app_update.go:843-852`

`screenStack` grows unbounded. `popScreen()` is defined but never called. `esc` always goes to REPL.

**Fix:** Wire `esc` to `popScreen()` on sub-screens.

### 5.14 MEDIUM: Question Response Type Mismatch (BUG-14)

**File:** `app_update.go:1212-1220`

`QuestionModel.Update()` may emit `tools.QuestionResponse` but `AppState` handles `QuestionResponseMsg`. Response silently dropped.

**Fix:** Verify emission type matches handler type.

### 5.15 MEDIUM: Workflow Phase Screens Show Loading Forever (BUG-15)

**Files:** `commands_workflow.go:51-119`

`/plan`, `/execute`, `/verify`, `/ship` navigate to screens but `ensureSubModel` only resizes existing models — doesn't create them. If no workflow has run, screens show "Loading..." permanently.

**Fix:** Block phase navigation when no workflow is active, or show "No active workflow" message.

---

## 6. Component Inventory & Gaps

### 6.1 Existing Components (internal/tui/components/)

| Component | File | Purpose | Used By | Status |
|-----------|------|---------|---------|--------|
| `PermissionModal` | `permission.go` | Tool approval dialog | AppState | Working |
| `QuestionModel` | `question.go` | AskUserQuestion modal | AppState | Working |
| `Card` | `card.go` | Bordered content container | Welcome, Ship, Settings | Working |
| `Badge` / `SimpleBadge` | `badge.go` | Status pills | Execute, Verify, Plan | Working |
| `ToolCard` | `toolcard.go` | Tool execution display | REPL messages | Working |
| `ToolRenderers` | `toolrenderers.go` | Tool-specific renderers | ToolCard | Working |
| `BashRenderer` | `bash_renderer.go` | Bash output styling | ToolCard | Working |
| `FileRenderers` | `file_renderers.go` | File read/write display | ToolCard | Working |
| `SpecialRenderers` | `special_renderers.go` | Web/glob/grep display | ToolCard | Working |
| `MessageRenderer` | `message.go` | Chat message rendering | REPL | Working |
| `ThinkingBlock` | `thinking.go` | Reasoning block display | REPL messages | Working |
| `Spinner` | `spinner.go` | Loading animation | Execute, Verify, ModelSelector | Working |
| `ProgressBar` | `progress.go` | Progress display (4 styles) | Execute | Working |
| `AnimatedProgressBar` | `progress.go` | Animated progress with flash | Execute | Working |
| `Sparkline` | `sparkline.go` | Mini chart display | Metrics (potential) | Working |
| `MetricCard` | `metriccard.go` | Stat display card | Metrics | Working |
| `StatRow` | `statrow.go` | Stat row display | Metrics | Working |
| `Divider` | `divider.go` | Section separator | Various | Working |
| `Logo` | `logo.go` | ASCII art logo | Welcome | Working |
| `Starfield` | `starfield.go` | Background decoration | FirstRun | Working |
| `SearchModel` | `search.go` | Search overlay | **Not wired** | **Dead code** |
| `FilterChips` | `filterchips.go` | Filter tag pills | **Not wired** | **Dead code** |
| `Truncate` | `truncate.go` | String truncation | Various | Working |

### 6.2 Missing Components

| Component | Priority | Purpose |
|-----------|----------|---------|
| **WorkflowPhaseBar** | P0 | Horizontal pipeline visualization showing all 6 phases with current state |
| **TaskGraph** | P1 | ASCII dependency graph for plan screen showing task relationships |
| **ConfirmDialog** | P0 | Reusable Y/N confirmation modal (currently ad-hoc in processCommandResult) |
| **DataTable** | P1 | Sortable, scrollable table for ledger, sessions, metrics breakdowns |
| **FileTree** | P1 | Tree-view with indentation for file explorer and sidebar |
| **CodeBlock** | P2 | Syntax-highlighted code block with language label and copy hint |
| **TimelineView** | P1 | Vertical timeline for commit history, session history |
| **TabBar** | P2 | Reusable horizontal tab bar (currently only in Settings) |
| **Breadcrumb** | P2 | Navigation breadcrumb trail (header uses simple text) |
| **SplitPane** | P2 | Side-by-side content layout for wide terminals |
| **Dropdown** | P2 | Selection dropdown for settings fields |
| **NotificationList** | P2 | Scrollable notification history |

### 6.3 Dead Code Components

| Component | File | Lines | Issue |
|-----------|------|-------|-------|
| `SearchModel` | `search.go` | ~100+ | Never instantiated or wired to any screen |
| `FilterChips` | `filterchips.go` | ~80+ | Never instantiated or wired to any screen |

---

## 7. State Management Gaps

### 7.1 handleWindowResize Missing Models

| Model | Impact |
|-------|--------|
| `goalInput` | Textarea width wrong on resize |
| `ledgerModel` | Viewport doesn't adapt |
| `rollbackModel` | Commit list clips on resize |
| `shipModel` | Stats grid misaligned |
| `firstRunModel` | Wizard layout breaks |
| `configModel` | Config view clips on resize |
| `discussModel` | Question text clips |

### 7.2 default: Message Forwarding Missing Screens

The `default:` case in `Update()` forwards unhandled messages to only 4 screens. Missing:

| Screen | Impact |
|--------|--------|
| `ScreenMetrics` | No tick/animation updates |
| `ScreenGoalInput` | No textarea blink updates |
| `ScreenLedger` | No viewport scroll updates |
| `ScreenRollback` | No viewport scroll updates |
| `ScreenConfig` | No viewport scroll updates |
| `ScreenDiff` | No viewport scroll updates |
| `ScreenShip` | No viewport scroll updates |
| `ScreenPlan` | No viewport scroll updates |
| `ScreenExecute` | No tick/animation updates |
| `ScreenVerify` | No tick/animation updates |

### 7.3 Theme Propagation Missing Models

| Model | Impact |
|-------|--------|
| `diffModel` | Diff colors don't update on theme change |
| `goalInput` | Textarea keeps old colors |
| `ledgerModel` | Table keeps old colors |
| `rollbackModel` | Commit list keeps old colors |
| `configModel` | Config view keeps old colors |
| `firstRunModel` | Wizard keeps old colors |
| `msModel` | Model picker keeps old colors |

### 7.4 Init() Never Called

| Model | Impact |
|-------|--------|
| `DiscussModel` | Cursor doesn't blink |
| `LedgerModel` | No initial data load |
| `RollbackModel` | No initial data load |
| `ConfigModel` | No initial setup |

---

## 8. Navigation & Keybinding Gaps

### 8.1 Keybinding Context Coverage (keybindings.go:13-24)

| Context | Constant | Registered Bindings |
|---------|----------|-------------------|
| Global | `CtxGlobal` | `ctrl+x s` (settings) |
| REPL | `CtxREPL` | 7 bindings (sidebar, session, model, theme) |
| Palette | `CtxPalette` | **0 bindings** |
| Sidebar | `CtxSidebar` | **0 bindings** |
| Settings | `CtxSettings` | **0 bindings** |
| ModelSel | `CtxModelSel` | **0 bindings** |
| Resume | `CtxResume` | **0 bindings** |
| PermModal | `CtxPermModal` | **0 bindings** |
| FirstRun | `CtxFirstRun` | **0 bindings** |

**Missing contexts entirely:**
- `CtxPlan`, `CtxExecute`, `CtxVerify`, `CtxShip`
- `CtxDiscuss`, `CtxDiff`, `CtxLedger`, `CtxRollback`
- `CtxMetrics`, `CtxGoalInput`, `CtxConfig`, `CtxHelp`

### 8.2 Unreachable Navigation Paths

| From | To | Issue |
|------|----|-------|
| Any screen | Help | No `?` binding, no `/help` screen navigation |
| Any screen | Bisect | No screen exists |
| Any screen | ThemePicker | No screen exists |
| Ship | Previous screen | No `esc`/`q` handler |
| Sub-screen | Previous screen | `popScreen()` never called; `esc` always goes to REPL |
| Plan | Execute (auto) | Plan requires manual approval (correct), but no auto-advance option |
| Metrics | Any sub-view | Only `esc`/`q` handled; no drill-down |

### 8.3 Leader Key Chord Gaps

**Current chords:** `ctrl+x s`, `ctrl+x b`, `ctrl+x n`, `ctrl+x r`, `ctrl+x m`, `ctrl+x t`, `ctrl+x [`, `ctrl+x ]`

**Missing useful chords:**
- `ctrl+x h` — Help screen (help.go references this but it's not registered)
- `ctrl+x f` — File explorer
- `ctrl+x d` — Dashboard/workflow overview
- `ctrl+x !` — Notification center
- `ctrl+x p` — Theme picker
- `ctrl+x w` — Workflow status
- `ctrl+x l` — Ledger
- `ctrl+x k` — Rollback (time machine)

---

## 9. Command System Gaps

### 9.1 Registered Commands vs Implementation

37 slash commands registered. Key gaps:

| Command | Registered | Functional | Issue |
|---------|-----------|------------|-------|
| `/help` | ✓ | Partial | Prints text in REPL, doesn't open HelpModel screen |
| `/new` | ✓ | ✓ | Opens goal input |
| `/bisect` | ✓ | Stub | Prints usage text, no screen |
| `/metrics` | via `/phase` | ✓ | Opens metrics screen |
| `/clear` | ✓ | ✓ | Has confirmation dialog |
| `/reset` | ✓ | ✓ | Has confirmation dialog |

### 9.2 CommandRegistry Execution Flow

```
User types /command
  → cmdRegistry.Execute(input, ctx)
    → handleXxx(args, ctx) → CommandResult
      → processCommandResult(result)
        → confirm? → pending dialog
        → screen? → navigateToScreen()
        → sessionID? → loadAndRestoreSession()
        → workflowResume? → runWorkflowFromGoal()
        → cmd? → execute callback
```

**Gap:** `CommandContext` has no `ThemeManager` reference, so commands cannot create themed output.

---

## 10. Screen-by-Screen Revamp Plan

### 10.1 REPL Screen (ScreenREPL) — Status: Working

**Current:** Viewport + textarea + welcome screen + streaming + tool cards + thinking blocks.

**Revamp needs:**
- [ ] `j`/`k` scroll when textarea is empty (P1)
- [ ] Search within conversation (`ctrl+f`) — `SearchModel` exists but unwired (P1)
- [ ] Copy message to clipboard (`ctrl+y` copies last, need per-message) (P1)
- [ ] "New messages" indicator when scrolled up (P1)
- [ ] Auto-expand textarea height based on content (P1)
- [ ] Shift+Enter multi-line input (P0)
- [ ] Message cursor/selection mode for keyboard navigation (P2)
- [ ] Viewport focus mode (modal editing with `esc` to toggle) (P2)
- [ ] Scroll indicators (gradient fade at top/bottom of viewport) (P3)
- [ ] Streaming cursor (blinking `▌` at end of streaming text) (P2)

### 10.2 Plan Screen (ScreenPlan) — Status: Degraded

**Current:** Viewport with task list grouped by waves, confirm mode, refine mode.

**Revamp needs:**
- [ ] Fix stale tasks on initial load (BUG-08) (P0)
- [ ] Rich markdown rendering for plan content (currently raw text in viewport) (P1)
- [ ] Dependency graph visualization (Tab key to toggle graph view) (P1)
- [ ] Diff preview per task (`D` key for predicted files) (P1)
- [ ] Per-task arbitrage info (`O` key) (P2)
- [ ] Cost estimate card at top (P1)
- [ ] Time estimate display (P1)
- [ ] Segmented progress bar showing task categories (P2)
- [ ] Approval action bar: `[ Approve All ] [ Approve Selected ] [ Refine ] [ Reject ]` (P1)

### 10.3 Execute Screen (ScreenExecute) — Status: Degraded

**Current:** Task list with status badges, live output, animated progress bar, pause/resume.

**Revamp needs:**
- [ ] Fix message forwarding for TickMsg animation (BUG-07 related) (P0)
- [ ] Real-time token/cost ticker during execution (P1)
- [ ] ETA display based on task completion rate (P2)
- [ ] Expandable tool output detail (press `enter` on running task) (P2)
- [ ] Pause overlay with dimmed background and "PAUSED" banner (P2)
- [ ] Task completion celebration (brief flash + toast) (P3)
- [ ] Background execution indicator when navigating away (P1)

### 10.4 Verify Screen (ScreenVerify) — Status: Degraded

**Current:** Task list with pass/fail badges, heal cursor, manual steps display.

**Revamp needs:**
- [ ] Fix message forwarding (P0)
- [ ] Heal progress bar (not just spinner) showing attempt progress (P2)
- [ ] Error detail expandable per task (P1)
- [ ] "Re-run all" option (P2)
- [ ] Summary card: pass rate, time, categories of failures (P1)
- [ ] Auto-scroll to first failed task on load (P2)

### 10.5 Ship Screen (ScreenShip) — Status: Degraded

**Current:** Stats grid (tasks, files, tokens, cost, commits), demonstration view.

**Revamp needs:**
- [ ] Add `esc`/`q` handler (BUG-11) (P0)
- [ ] Fix message forwarding (P0)
- [ ] Share/export summary as markdown (P2)
- [ ] PR description generation button (P2)
- [ ] Diff summary card (files changed with +/- stats) (P1)
- [ ] Session duration with human-readable format (P2)
- [ ] "Open in browser" for git remote (P3)

### 10.6 Discuss Screen (ScreenDiscuss) — Status: Broken

**Current:** Q&A interface for clarify questions from the Discuss phase.

**Revamp needs:**
- [ ] Fix question population (BUG-02) (P0)
- [ ] Fix Init() call (BUG-06) (P0)
- [ ] Add to ensureSubModel (P0)
- [ ] Fix window resize handling (P0)
- [ ] Progress indicator showing question N of M (P1)
- [ ] "Skip all" option with confirmation (P1)
- [ ] Rich text rendering for questions (markdown support) (P2)
- [ ] Previous answers review panel (P2)

### 10.7 Resume Screen (ScreenResume) — Status: Broken

**Current:** Session list with timestamps, search bar, rename, export.

**Revamp needs:**
- [ ] Fix session restore routing (BUG-01) (P0)
- [ ] Session detail preview before loading (P1)
- [ ] Session cards with goal, duration, cost, file count (P1)
- [ ] Delete session with confirmation (P1)
- [ ] Sort by date/cost/project (P2)
- [ ] Filter by provider/model/phase (P2)
- [ ] Empty state: "No sessions yet. Start with /new" (P1)

### 10.8 Ledger Screen (ScreenLedger) — Status: Broken

**Current:** Table of ledger entries with stats footer.

**Revamp needs:**
- [ ] Fix LoadEntries call (BUG-03) (P0)
- [ ] Fix window resize (P0)
- [ ] Fix theme propagation (P0)
- [ ] Sortable columns (P1)
- [ ] Filter by date range, model, provider (P2)
- [ ] Entry detail view (P2)
- [ ] Export as CSV/JSON (P2)
- [ ] Sparkline of cost over time (P2)

### 10.9 Rollback Screen (ScreenRollback) — Status: Broken

**Current:** Commit list with diff view, soft/hard reset.

**Revamp needs:**
- [ ] Fix LoadCommits call (BUG-04) (P0)
- [ ] Fix window resize (P0)
- [ ] Fix theme propagation (P0)
- [ ] Confirmation dialog before reset (P0 — data loss risk)
- [ ] Diff stats per commit (+N -N) (P1)
- [ ] Branch indicator on current HEAD (P1)
- [ ] Per-file rollback option (P2)
- [ ] Dry-run preview before executing reset (P2)

### 10.10 Metrics Screen (ScreenMetrics) — Status: Degraded

**Current:** Metric cards (sessions, messages, tokens, providers), breakdown lists.

**Revamp needs:**
- [ ] Fix message forwarding (BUG-07) (P0)
- [ ] Fix window resize (currently works via handleWindowResize but not Update default) (P0)
- [ ] Dashboard grid layout (2x2 or 3x2 based on width) (P1)
- [ ] Sparklines for daily usage (P2)
- [ ] Bar charts for model costs (P2)
- [ ] Streak counter (P3)
- [ ] Export as markdown (P2)
- [ ] Date range filter (P2)

### 10.11 Diff Screen (ScreenDiff) — Status: Working

**Current:** Colored diff output in viewport with stats.

**Revamp needs:**
- [ ] Fix viewport height initialization (BUG-12) (P1)
- [ ] Hunk navigation (`n`/`N` to jump between `@@` blocks) (P2)
- [ ] Side-by-side mode for wide terminals (P2)
- [ ] Line numbers gutter (P2)
- [ ] Word-level highlighting within changed lines (P2)
- [ ] Stats bar: `+12 -8 lines, 3 files changed` (P1)

### 10.12 Settings Screen (ScreenSettings) — Status: Working

**Current:** 6-tab layout (Provider, Model, UI, Keys, Workflow, About), inline editing.

**Revamp needs:**
- [ ] Theme preview when changing theme setting (P1)
- [ ] Validation feedback for invalid values (P1)
- [ ] Per-setting reset to default (P2)
- [ ] Import/export settings (P2)
- [ ] Search within settings (P3)

### 10.13 ModelSelector Screen (ScreenModelSelector) — Status: Working

**Current:** Provider tabs, model list with search, pricing display.

**Revamp needs:**
- [ ] Model comparison side-by-side (P2)
- [ ] Capability badges (tools, reasoning, vision) (P1)
- [ ] Context length display (P1)
- [ ] Recent models section (P2)
- [ ] Fix theme propagation (P1)

### 10.14 FirstRun Screen (ScreenFirstRun) — Status: Working

**Current:** Multi-step wizard (welcome, provider, API key, model pick, done).

**Revamp needs:**
- [ ] Fix window resize (P1)
- [ ] API key validation before proceeding (P1)
- [ ] Connectivity test animation ("Testing..." → "✓ Connected!") (P1)
- [ ] Keychain storage prompt (P1)
- [ ] Fix theme propagation (P1)

### 10.15 GoalInput Screen (ScreenGoalInput) — Status: Working

**Current:** Textarea with recent goals panel.

**Revamp needs:**
- [ ] Fix window resize (P1)
- [ ] Fix theme propagation (P1)
- [ ] Goal suggestions based on project context (P2)
- [ ] Character count display (P3)

### 10.16 Config Screen (ScreenConfig) — Status: Working

**Current:** Full config viewer with TOML content display.

**Revamp needs:**
- [ ] Fix window resize (P1)
- [ ] Fix theme propagation (P1)
- [ ] Editable fields (currently read-only view) (P2)
- [ ] Config validation display (P2)

---

## 11. New Screen Specifications

### 11.1 Help Screen (SCREEN-01)

```
Constant: ScreenHelp = 17
Model: HelpModel (already implemented in help.go)
AppState field: helpModel *HelpModel

ensureSubModel:
  case ScreenHelp:
    if m.helpModel == nil {
      m.helpModel = NewHelpModel(m.themeManager.Current())
    }
    m.helpModel.SetDimensions(contentW, contentH)
    return nil

routeKeyMsg:
  case ScreenHelp:
    if m.helpModel != nil {
      newHelp, cmd := m.helpModel.Update(msg)
      if nh, ok := newHelp.(*HelpModel); ok {
        m.helpModel = nh
      }
      return cmd
    }

renderActiveScreen:
  case ScreenHelp:
    return m.renderHelpContent(chrome)

handleWindowResize:
  if m.helpModel != nil {
    m.helpModel.SetDimensions(contentW, contentH)
  }

applyTheme:
  if m.helpModel != nil {
    m.helpModel.SetTheme(t)
  }

keybindings_screens.go:
  r.Register(CtxREPL, "ctrl+x h", "Help", emit("open_help"))
  // Also wire '?' in repl.go to navigate to ScreenHelp

handleKeyAction:
  case "open_help":
    return m.navigateToScreen(ScreenHelp)

commands_core.go handleHelp:
  screen := ScreenHelp
  return CommandResult{Success: true, Screen: &screen}
```

### 11.2 Workflow Dashboard (SCREEN-05)

```
Constant: ScreenDashboard = 18
Model: DashboardModel
AppState field: dashboardModel *DashboardModel

Layout:
  ┌───────────────────────────────────────────────────┐
  │  Goal: "Add authentication to the API"             │
  │                                                    │
  │  ○ Initialize → ● Discuss → ○ Plan → ○ Execute    │
  │                              → ○ Verify → ○ Ship   │
  │                                                    │
  │  Phase: discuss    Elapsed: 2m 34s                 │
  │  Model: claude-4   Cost: $0.0234                   │
  │                                                    │
  │  Recent Activity:                                  │
  │    14:23  Discuss phase started                    │
  │    14:22  Goal submitted                           │
  │    14:22  Workflow initialized                     │
  │                                                    │
  │  [enter] Go to current phase   [esc] Back          │
  └───────────────────────────────────────────────────┘
```

### 11.3 Theme Picker (SCREEN-03)

```
Constant: ScreenThemePicker = 19
Model: ThemePickerModel
AppState field: themePickerModel *ThemePickerModel

Layout:
  ┌───────────────────────────────────────────────────┐
  │  Themes                                            │
  │                                                    │
  │  ▶ Catppuccin    ■ #1e1e2e  ■ #cba6f7  ■ #cdd6f4 │
  │    Nord          ■ #2e3440  ■ #88c0d0  ■ #d8dee9  │
  │    Tokyo Night   ■ #1a1b26  ■ #7aa2f7  ■ #c0caf5  │
  │    Gruvbox       ■ #282828  ■ #d3869b  ■ #ebdbb2  │
  │    Rosé Pine     ■ #191724  ■ #c4a7e7  ■ #e0def4  │
  │    Dracula       ■ #282a36  ■ #ff79c6  ■ #f8f8f2  │
  │    Solarized     ■ #002b36  ■ #268bd2  ■ #839496  │
  │    Monochrome    ■ #0a0a0a  ■ #ffffff  ■ #ffffff  │
  │                                                    │
  │  ┌─ Preview ─────────────────────────────────┐    │
  │  │  Card with brand border                   │    │
  │  │  [badge]  Progress ████░░ 67%              │    │
  │  │  ✓ done  ✗ failed  ○ pending               │    │
  │  └────────────────────────────────────────────┘    │
  │                                                    │
  │  [enter] Apply   [esc] Cancel                      │
  └───────────────────────────────────────────────────┘
```

### 11.4 Bisect Screen (SCREEN-02)

```
Constant: ScreenBisect = 20
Model: BisectModel
AppState field: bisectModel *BisectModel

Layout:
  ┌───────────────────────────────────────────────────┐
  │  Git Bisect                                        │
  │                                                    │
  │  Range: abc1234..def5678 (42 commits)             │
  │  Current: 21/42 — commit a1b2c3d4                 │
  │  Status: Testing...                                │
  │                                                    │
  │  a1b2c3d4  Fix authentication bug        ◐ testing│
  │  e5f6g7h8  Add user model                 ○ pending│
  │  i9j0k1l2  Update dependencies            ○ pending│
  │                                                    │
  │  [g] Good   [b] Bad   [s] Skip   [r] Reset        │
  │  [esc] Cancel bisect                               │
  └───────────────────────────────────────────────────┘
```

---

## 12. Component Revamp Plan

### 12.1 ConfirmDialog Component (COMP-01)

Replace the ad-hoc `pendingConfirm` / `confirmPrompt` in AppState with a reusable component:

```go
type ConfirmDialog struct {
    Title       string
    Message     string
    ConfirmText string  // default "Yes"
    CancelText  string  // default "No"
    Danger      bool    // red border for destructive actions
    Theme       theme.Theme
}

func (cd *ConfirmDialog) View() string { ... }
func (cd *ConfirmDialog) Update(msg tea.Msg) (*ConfirmDialog, tea.Cmd) { ... }
```

### 12.2 WorkflowPhaseBar Component (COMP-02)

```go
type WorkflowPhaseBar struct {
    Phases      []types.WorkflowPhase
    Current     types.WorkflowPhase
    Completed   map[types.WorkflowPhase]bool
    Theme       theme.Theme
    Width       int
}

func (wpb *WorkflowPhaseBar) View() string {
    // Renders: ○ Initialize → ● Discuss → ○ Plan → ○ Execute → ○ Verify → ○ Ship
    // ○ = pending (muted), ● = current (brand + pulse), ✓ = completed (success)
}
```

### 12.3 DataTable Component (COMP-03)

```go
type DataTable struct {
    Columns     []Column
    Rows        []Row
    SortColumn  int
    SortAsc     bool
    Cursor      int
    Offset      int
    Width       int
    Height      int
    Theme       theme.Theme
}

type Column struct {
    Header string
    Width  int
    Align  Alignment
}

func (dt *DataTable) View() string { ... }
func (dt *DataTable) Update(msg tea.Msg) (*DataTable, tea.Cmd) { ... }
func (dt *DataTable) Sort(col int) { ... }
func (dt *DataTable) Filter(query string) { ... }
```

### 12.4 FileTree Component (COMP-04)

```go
type FileTree struct {
    Root     *FileNode
    Cursor   int
    Expanded map[string]bool
    Theme    theme.Theme
    Width    int
    Height   int
}

type FileNode struct {
    Name     string
    Path     string
    IsDir    bool
    Children []*FileNode
    Status   string  // git status: M, A, D, ?
}

func (ft *FileTree) View() string {
    // Renders tree with ├── └── indentation
    // Git status icons: ● modified, + added, ✗ deleted, ? untracked
}
```

---

## 13. Implementation Priority Matrix

### Phase 1: Critical Fixes (P0) — Do Immediately

| # | Task | Screen | Files | Effort |
|---|------|--------|-------|--------|
| 1 | Fix Resume session restore routing | Resume | `app_update.go` | 15min |
| 2 | Fix Discuss questions population | Discuss | `app_update_phase.go`, `types.go` | 30min |
| 3 | Fix Ledger LoadEntries call | Ledger | `app_update.go` | 10min |
| 4 | Fix Rollback LoadCommits call | Rollback | `app_update.go` | 10min |
| 5 | Wire Help screen (add ScreenHelp, routing) | Help | `types.go`, `app_state.go`, `app_update.go`, `app_view.go` | 45min |
| 6 | Fix DiscussModel.Init() call | Discuss | `app_update.go` | 10min |
| 7 | Fix GoalSubmittedMsg resume phase | Workflow | `app_update.go` | 20min |
| 8 | Fix Ship esc/q handler | Ship | `ship_model.go` | 5min |
| 9 | Fix DiffModel viewport height init | Diff | `app_update.go` | 10min |
| 10 | Add ConfirmDialog component | All | `components/confirm.go` | 1hr |

### Phase 2: High Priority (P1) — Ship in Next Version

| # | Task | Screen | Effort |
|---|------|--------|--------|
| 11 | Fix all handleWindowResize gaps | 7 models | 1hr |
| 12 | Fix all theme propagation gaps | 7 models | 1hr |
| 13 | Fix all message forwarding gaps | 10 screens | 2hr |
| 14 | Add WorkflowPhaseBar component | Dashboard, Header | 2hr |
| 15 | Add Workflow Dashboard screen | New screen | 3hr |
| 16 | Add Theme Picker screen | New screen | 3hr |
| 17 | Fix Plan stale tasks on load | Plan | 30min |
| 18 | Add DataTable component | Ledger, Resume, Metrics | 3hr |
| 19 | Plan screen: rich markdown rendering | Plan | 2hr |
| 20 | Plan screen: dependency graph visualization | Plan | 3hr |
| 21 | Plan screen: diff preview per task | Plan | 2hr |
| 22 | Resume: session detail preview | Resume | 2hr |
| 23 | Resume: empty state UX | Resume | 30min |
| 24 | REPL: j/k scroll (empty input) | REPL | 30min |
| 25 | REPL: search within conversation | REPL | 2hr |
| 26 | REPL: shift+enter multi-line | REPL | 30min |
| 27 | REPL: "New messages" indicator | REPL | 1hr |
| 28 | Rollback: confirmation before reset | Rollback | 30min |
| 29 | Wire popScreen() for esc navigation | All | 1hr |
| 30 | Add missing leader key chords | Keybindings | 1hr |

### Phase 3: Medium Priority (P2) — Backlog

| # | Task | Screen | Effort |
|---|------|--------|--------|
| 31 | Add Bisect screen | New screen | 4hr |
| 32 | Add File Explorer screen | New screen | 4hr |
| 33 | Add FileTree component | Component | 3hr |
| 34 | Add Notification Center screen | New screen | 3hr |
| 35 | Add Session Detail screen | New screen | 3hr |
| 36 | Add Tool Output Detail screen | New screen | 2hr |
| 37 | Execute: real-time cost ticker | Execute | 1hr |
| 38 | Execute: background indicator | Execute | 1hr |
| 39 | Verify: error detail expand | Verify | 2hr |
| 40 | Ship: PR description generation | Ship | 2hr |
| 41 | Diff: hunk navigation | Diff | 1hr |
| 42 | Diff: side-by-side mode | Diff | 3hr |
| 43 | Ledger: sortable columns | Ledger | 2hr |
| 44 | Metrics: dashboard grid layout | Metrics | 2hr |
| 45 | Metrics: sparklines and charts | Metrics | 3hr |
| 46 | Settings: theme preview | Settings | 2hr |
| 47 | ModelSelector: capability badges | Models | 1hr |

### Phase 4: Nice to Have (P3) — Polish

| # | Task | Effort |
|---|------|--------|
| 48 | Split-pane layout for wide terminals | 6hr |
| 49 | Breadcrumb navigation component | 2hr |
| 50 | Tab bar reusable component | 2hr |
| 51 | Dropdown component for settings | 2hr |
| 52 | Code block with syntax highlighting | 3hr |
| 53 | Timeline view component | 2hr |
| 54 | Execute: completion celebration | 1hr |
| 55 | REPL: viewport focus mode | 2hr |
| 56 | REPL: streaming cursor | 1hr |
| 57 | Diff: word-level highlighting | 2hr |

---

## Appendix A: File Index

### TUI Files (internal/tui/)

```
app.go                    — Init, Shutdown, workflow engine, permission listeners
app_state.go              — AppState struct, NewApp, handleFirstRunComplete
app_update.go             — Update dispatch, routing, resize, navigation, session
app_update_commands.go    — Slash command handling, chat message sending
app_update_phase.go       — Workflow phase result handling, plan approve/refine
app_view.go               — View rendering, screen delegation, chrome composition
app_channel.go            — Channel emitter for workflow events
cache_refresh.go          — Async model cache refresh commands
cmdpalette.go             — Command palette overlay (ctrl+p)
commands_ai.go            — /memory, /compress, /optimize, /model, /models, /fallback, /provider
commands_config.go        — /settings, /config, /theme, /cost, /log, /key, /tokens
commands_config_diskusage_unix.go — Disk usage for Unix
commands_config_diskusage_windows.go — Disk usage for Windows
commands_core.go          — /help, /clear, /status, /reset, /quit, /undo, /history, /health, /tools
commands_git.go           — /diff, /rollback, /bisect
commands_session.go       — /sessions, /fork, /prev, /next, /save, /goal, /resume, /ledger, /export
commands_workflow.go      — /new, /workflow, /phase, /refine, /pause, /resume-task
config_model.go           — Config viewer screen
constants.go              — Width thresholds (40, 60, 80)
diff_model.go             — Diff viewer model
diff_view.go              — Diff rendering with color
execute_model.go          — Execute progress model
execute_view.go           — Execute rendering (progress bars, task list)
firstrun_model.go         — Setup wizard model
firstrun_view.go          — Setup wizard rendering
goalinput.go              — Goal input textarea screen
header.go                 — Header rendering (moved to layout/page.go)
health.go                 — Health check commands
helpers.go                — Utility functions
history.go                — Frecent prompt history
keybindings.go            — Key registry, leader key, which-key
keybindings_screens.go    — Default keybinding registrations
ledger.go                 — Learning ledger browser screen
mention.go                — @-mention file resolution
mention_view.go           — @-mention dropdown rendering
metrics.go                — Session analytics screen
modelselector.go          — Model/provider picker model
modelselector_list.go     — Model list rendering
modelselector_view.go     — Model selector view
plan_model.go             — Plan review model
plan_view.go              — Plan rendering
plan_refine.go            — Plan refinement input
provider_registration.go  — Provider registration helpers
providerbadge.go          — Provider badge rendering
repl.go                   — REPL key handling, main update loop
repl_clipboard.go         — Clipboard operations
repl_commands.go          — REPL command execution
repl_keys.go              — Package anchor (keys in repl.go)
repl_model.go             — REPL model definition
repl_quickactions.go      — Quick action suggestions
repl_state.go             — REPL message state management
repl_stream.go            — Streaming response handling
repl_thinking.go          — Thinking block handling
repl_view.go              — REPL rendering
repl_welcome.go           — Welcome screen rendering
rollback.go               — Commit time machine screen
settings_edit.go          — Settings field editing
settings_model.go         — Settings model definition
settings_tabs.go          — Settings tab definitions
settings_view.go          — Settings rendering
ship_model.go             — Ship summary model
ship_view.go              — Ship rendering (stats grid, commit log)
sidebar.go                — Sidebar model (git status, file list)
statusbar.go              — Status bar rendering (legacy, now in layout/page.go)
streaming.go              — Stream command infrastructure
toast.go                  — Toast notification system
toasts.go                 — Toast rendering
transition.go             — Screen transition overlay
truncate.go               — String truncation utilities
types.go                  — Screen constants, message types
verify.go                 — Verification results screen
```

### Component Files (internal/tui/components/)

```
badge.go           — Badge and SimpleBadge pills
bash_renderer.go   — Bash tool output rendering
card.go            — Card container with variants
divider.go         — Section dividers
file_renderers.go  — FileRead/FileWrite display
filterchips.go     — Filter tag pills (UNUSED)
logo.go            — ASCII art logo
message.go         — Chat message renderer
metriccard.go      — Metric display cards
permission.go      — Permission approval modal
progress.go        — Progress bars (4 styles + animated)
question.go        — AskUserQuestion modal
search.go          — Search overlay (UNUSED)
sparkline.go       — Mini chart component
special_renderers.go — Web/glob/grep display
spinner.go         — Loading spinner
starfield.go       — Background decoration
statrow.go         — Statistics row
thinking.go        — Reasoning block display
toolcard.go        — Tool execution card
toolrenderers.go   — Tool-specific renderers
truncate.go        — Truncation utilities
```

### Layout Files (internal/tui/layout/)

```
page.go          — Unified page layout (header + content + footer)
responsive.go    — Breakpoint detection, responsive flags
minscreen.go     — Minimum screen size guard
page_test.go     — Layout tests
```

### Theme Files (internal/tui/theme/)

```
borders.go       — Border definitions (Normal, Thin, Double, Split)
colors.go        — Dark/Light palette constructors
registry.go      — 10 theme presets (Catppuccin, Nord, Tokyo, etc.)
shadow.go        — Shadow rendering utilities
tabs.go          — Tab styling
theme.go         — Theme struct definition, Mode enum
theme_test.go    — Theme tests
unicode.go       — Unicode character constants
```

---

## Appendix B: Message Flow Diagram

```
User Input
  │
  ├─ tea.KeyMsg
  │    └─ routeKeyMsg()
  │         ├─ cmdPalette? → palette.Update()
  │         ├─ pendingConfirm? → y/n/esc
  │         ├─ ctrl+g → sidebar.ToggleFocus()
  │         ├─ sidebar focused? → sidebar.HandleKey()
  │         ├─ leader key? → keyRegistry.Handle()
  │         └─ screen switch → active model.Update()
  │              └─ emits: AppMsg, SlashCommandMsg, PlanApproveMsg,
  │                         PlanRefineMsg, ExecutePauseMsg, etc.
  │
  ├─ AppMsg (screen routing)
  │    └─ handleAppMsg() → routeAppMsgAction() / loadAndRestoreSession()
  │
  ├─ SlashCommandMsg
  │    └─ handleSlashCommand()
  │         ├─ ! prefix → shell command
  │         ├─ / prefix → cmdRegistry.Execute() → processCommandResult()
  │         └─ else → sendChatMessage()
  │
  ├─ PhaseResultMsg (workflow completion)
  │    └─ handlePhaseResult()
  │         ├─ Initialize → run Discuss
  │         ├─ Discuss → show Discuss screen or run Plan
  │         ├─ Plan → show Plan screen
  │         ├─ Execute → run Verify
  │         ├─ Verify → run Ship
  │         └─ Ship → show Ship screen
  │
  ├─ workflow.*Msg (task/tool/heal events)
  │    └─ Update execute/verify models, drain emitter
  │
  └─ Infrastructure (Health, Cache, Toast, Permission, etc.)
```

---

*Report compiled 2026-06-10 from complete source review of all 95+ TUI source files, 28 component files, 8 theme files, 4 layout files, and all supporting packages.*
