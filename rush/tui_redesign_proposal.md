# M31A TUI Redesign Proposal
## A Complete New Screen Architecture for the Terminal AI Agent

> **Author:** Antigravity — Deep codebase analysis, 2026-06-05  
> **Target:** M31A v1.x → v2.0 TUI overhaul  
> **Scope:** All 10 existing screens + 6 new proposed screens

---

## Executive Summary

After a deep audit of M31A's TUI codebase (70+ files, ~350KB of Go source), this report proposes a **complete visual and interaction redesign** of all existing screens while staying true to the architectural constraints: Bubble Tea single-threaded model, `lipgloss` styling, no CSS animations, charmbracelet component ecosystem.

The current TUI is **functional but primitive** — heavy use of `strings.Builder`, minimal visual hierarchy, flat text lists for everything, and zero use of advanced terminal rendering techniques. The proposal below reimagines every screen from scratch with a bold, cohesive design language.

---

## 1. Current State Analysis

### 1.1 Existing Screens Inventory

| Screen | File | Current Pattern | Critical Issues |
|--------|------|----------------|----------------|
| `ScreenFirstRun` | `firstrun.go` | ASCII logo + simple list + textinput | Static, no flow animation, feature cards are too narrow (24 cols each) |
| `ScreenREPL` | `repl.go` | viewport + textarea + spinner | Chat bubbles not differentiated by role, no inline code highlighting in input, welcome screen is forgettable |
| `ScreenModelSelector` | `modelselector.go` | bubbles/list + search | No visual cost/capability comparison, no sparkline for usage history |
| `ScreenSettings` | `settings.go` | 6-tab layout + inline editing | Tab bar is unstyled, field editing is cramped, no description text per field |
| `ScreenResume` | `resume.go` | bubbles/list + preview pane | Preview pane is plain text, filter chips row is minimal |
| `ScreenPlan` | `plan.go` | task list + strings.Builder | Very terse rendering, no timeline visualization, no file tree preview |
| `ScreenExecute` | `execute.go` | task list + spinner + metrics | The segmented progress bar is good but task rows are plain, no per-tool output stream |
| `ScreenVerify` | `verify.go` | checklist + strings.Builder | The heal confirmation modal is basic, no diff preview of what changed |
| `ScreenShip` | `ship.go` | commit review | Not examined in detail but follows same pattern |
| `ScreenDiff` | `diff.go` | diff rendering | Uses a custom renderer |

### 1.2 Shared Chrome Components

| Component | File | Issues |
|-----------|------|--------|
| Header | `header.go` | Uses FNV cache correctly but visually minimal — just `M31A [OR] ModelName --/-- ctx [LIVE]` |
| StatusBar | `statusbar.go` | Left/right padding approach is good, but content is sparse |
| Sidebar | `sidebar.go` | Shows git status correctly; 42-col fixed width feels arbitrary |
| CommandPalette | `cmdpalette.go` | Standard pattern, not examined deeply |
| PermissionModal | `components/` | Not examined but used in dispatcher flow |

### 1.3 Theme System

The theme system in `theme/theme.go` and `theme/colors.go` is **well-structured** with 76+ color tokens. Both dark and light themes exist. The `Brand` color (typically `#7C3AED` violet) is the primary accent throughout.

---

## 2. Design Philosophy for the Redesign

### 2.1 Core Principles

1. **Layers, not flatness** — Use surface elevation (Background → Surface → SurfaceElevated) actively to create depth
2. **Information density at all sizes** — The UI must degrade gracefully from 80 cols to 220 cols without breaking
3. **Every screen tells its own story** — Each screen has a unique visual metaphor, not just "a list with a header"
4. **Keyboard first, but discoverable** — Key hints always visible, not buried in documentation
5. **Micro-state feedback** — The terminal equivalent of hover states via selection highlighting, live counters, etc.

### 2.2 Visual Language

```
╭─────────────────────────────────────────────────────────────╮
│  BRAND COLOR: #7C3AED (violet)   ░░░░░░░░░░░░░░░░░░░░░░░░  │
│  ACCENT:      #06B6D4 (cyan)     ░░░░░░░░░░░░░░░░░░░░░░░░  │
│  SUCCESS:     #10B981 (emerald)  ░░░░░░░░░░░░░░░░░░░░░░░░  │
│  WARNING:     #F59E0B (amber)    ░░░░░░░░░░░░░░░░░░░░░░░░  │
│  ERROR:       #EF4444 (red)      ░░░░░░░░░░░░░░░░░░░░░░░░  │
│  SURFACE:     #1E1E2E (deep navy) ░░░░░░░░░░░░░░░░░░░░░░░  │
╰─────────────────────────────────────────────────────────────╯
```

**Box Drawing Strategy:**
- Rounded borders (`╭─╮│╰─╯`) for panels and cards
- Double borders (`╔═╗║╚═╝`) for modal dialogs
- Half-blocks (`▀▄`) for progress bars and sparklines
- Braille patterns (`⣀⣠⣤⣶⣾⣿`) for sparkline charts
- Block elements (`█▓▒░`) for density indicators

---

## 3. Redesigned Screens

---

### 3.1 NEW: `ScreenFirstRun` — "Launchpad"

**Current problem:** The ASCII art is large but the content below is squished. Feature cards are 24 cols wide and feel toy-like. The provider selection list is a plain vertical list.

**Proposed redesign: Full-terminal "Launchpad" with animated galaxy metaphor**

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                                                                             │
│   ·  ·   ·                    ·      ·   ·                                 │
│       ·       ·  ·                ·                  ·     ·               │
│  ·          ·          ·    ·              ·                                │
│                                                                             │
│         ▐▌  ▐▌  ▄▄▄▄▄▄  ▄▄   ▄  ▄▄▄▄▄▄                                  │
│         ███████ ██   ██ ████  █ ██   ██                                    │
│         ██ █ ██      ██  ██ █ █ ██   ██                                    │
│         ██   ██  █████   ██  ██ █████████                                  │
│         ██   ██      ██  ██   █ ██   ██                                    │
│         ██   ██ ██   ██  ██   █ ██   ██                                    │
│         ██   ██  ██████  ██   █ ██   ██                                    │
│                                                                             │
│                  Terminal AI Coding Agent  v1.x                            │
│                                                                             │
│  ╭──────────────────────────╮  ╭──────────────────────────╮               │
│  │  ⚡ Fast Execution        │  │  🤖 AI-Powered Coding    │               │
│  │  Parallel task runner    │  │  Natural language goals  │               │
│  │  with dependency graph   │  │  → production code       │               │
│  ╰──────────────────────────╯  ╰──────────────────────────╯               │
│  ╭──────────────────────────╮  ╭──────────────────────────╮               │
│  │  🔄 Self-Healing          │  │  📦 Git Native           │               │
│  │  Auto-retry & rollback   │  │  Atomic commits per task │               │
│  │  via commit bisect       │  │  with rollback browser   │               │
│  ╰──────────────────────────╯  ╰──────────────────────────╯               │
│                                                                             │
│         ╭─────────────────────────────────────────╮                       │
│         │   ▶  Press Enter to begin setup          │                       │
│         ╰─────────────────────────────────────────╯                       │
│                                                                             │
│  ctrl+p commands  ·  ctrl+b sidebar  ·  /help  ·  MIT License             │
└─────────────────────────────────────────────────────────────────────────────┘
```

**Key changes:**
- Stars rendered as Unicode dots (·) scattered across the full terminal width, giving "galaxy" feeling without animation (matches Bubble Tea's frame redraw model)
- Feature cards are 2×2 grid, each 30+ cols wide, with 3-line descriptions
- CTA box has a filled background using `SurfaceElevated` + rounded border
- Footer includes keyboard shortcuts inline (not as a separate section)
- Stars use `TextMuted` color to feel subtle

**Provider Select sub-screen — "Constellation Picker":**

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  M31A ›  Provider Setup                                                     │
│  ─────────────────────────────────────────────────────────────────────────  │
│                                                                             │
│  Which AI gateway will power M31A?                                         │
│  You can add more later via /settings → Provider                           │
│                                                                             │
│  ╭──────────────────────────────────────────────────────────────────────╮  │
│  │  ▶ 1  ◆  OpenRouter                                                  │  │
│  │       100+ models · pay-per-use · key: sk-or-v1-...                  │  │
│  │       ████████████████████████████████████████░░░░  Coverage: 94%   │  │
│  ╰──────────────────────────────────────────────────────────────────────╯  │
│  ╭──────────────────────────────────────────────────────────────────────╮  │
│  │    2  ◈  Zen                                                          │  │
│  │       Fast inference · competitive pricing · key: zk-...             │  │
│  │       ████████████████████████░░░░░░░░░░░░░░░░░░░░  Coverage: 61%   │  │
│  ╰──────────────────────────────────────────────────────────────────────╯  │
│  ╭──────────────────────────────────────────────────────────────────────╮  │
│  │    3  ◆◈  Both (Recommended)                                          │  │
│  │       OpenRouter + Zen with automatic failover                        │  │
│  │       Best reliability · Zero downtime                               │  │
│  ╰──────────────────────────────────────────────────────────────────────╯  │
│                                                                             │
│    4  ○  Skip — configure via /settings later                              │
│                                                                             │
│  ↑↓ navigate  ·  Enter select  ·  1–4 jump                                │
└─────────────────────────────────────────────────────────────────────────────┘
```

**Key changes:**
- Each option is a full-width card with border (active item has Brand-colored border)
- "Coverage" bar shows how much of the model space each provider covers
- "Recommended" badge on Both option
- Skip option is not a card (less visual weight = less temptation)

---

### 3.2 REDESIGNED: `ScreenREPL` — "Mission Control"

**Current problem:** Messages use a simple viewport with text blocks. No clear visual separation between user and assistant. The input area is just a textarea with a thin border.

**Proposed redesign: Chat bubbles with role-colored gutters + rich input frame**

```
╭─ M31A ─ [OR] claude-3-5-sonnet ─ 45.2K/200K ctx ──────── [LIVE] ─────────╮
│                                                                            │
│  ┤ 14:23 ├──────────────────────────────────────────────────────────────  │
│  │ USER                                                                   │
│  │  Refactor the payment service to use the new stripe API                │
│  │                                                                        │
│  ┤ 14:23 ├──────────────────────────────────────────────────────────────  │
│  │ M31A                                                               ▼  │
│  │  I'll help you refactor the payment service. Let me first read      │  │
│  │  the current implementation to understand the structure.             │  │
│  │                                                                        │
│  │  ╔══════════════════════════════════════════════════════════════════╗ │
│  │  ║  🔧 FileRead                                    ✓ 847ms  42 lines ║ │
│  │  ║  src/services/payment.ts                                         ║ │
│  │  ╚══════════════════════════════════════════════════════════════════╝ │
│  │                                                                        │
│  │  The current implementation uses `stripe-node` v10. The new API     │  │
│  │  requires migrating to v14 with the following changes:               │  │
│  │                                                                        │
│  │  • Replace `stripe.charges.create()` → `stripe.paymentIntents`      │  │
│  │  • Update webhook signature verification                              │  │
│  │  • Add idempotency keys to all write operations                       │  │
│  │                                                                        │
│  └────────────────────────────────────────────────────────────────────── │
│                                                                            │
│  MODIFIED  src/services/payment.ts  M   src/webhooks/stripe.ts  M        │
│  ─────────────────────────────────────────────────────────────────────── │
│  │ M31A · claude-3-5-sonnet · [OR]                                      │
│  │ Type a message, /command, or goal...                                 │
│  │                                                                       │
│  ╰───────────────────────────────── Enter to send · Ctrl+C cancel ──────╯
│  Ready                                                  ctrl+p commands  │
╰────────────────────────────────────────────────────────────────────────────╯
```

**Key structural changes:**

1. **Message gutter** — Left border line (`│`) with role label, forming a "speech margin" visual metaphor
2. **Timestamp bars** — `┤ HH:MM ├───────` separators between conversation turns
3. **Tool cards** — Double-border `╔═╗` cards (distinct from panel borders) with status icon, elapsed time, and line count
4. **Inline git status strip** — Thin row above input showing modified files (replaces need for sidebar toggle in many workflows)
5. **Input frame redesign** — The textarea has `│` on left with model context line above it; `╰──────── Enter to send ──────╯` at bottom
6. **"▼" scroll indicator** — When not at bottom, a `▼` glyph appears at right edge of last message

**Welcome screen (when no messages):**

```
╭─────────────────────────────────────────────────────────────────────────╮
│                                                                         │
│         ╭───────────────────────────────────────╮                      │
│         │  ▓▓ M31A — Ready                       │                      │
│         │  claude-3-5-sonnet · OpenRouter         │                      │
│         │  ████████░░░░░░░░░░░ 45.2K ctx used    │                      │
│         ╰───────────────────────────────────────╯                      │
│                                                                         │
│  Quick starts:                                                          │
│  ──────────────────────────────────────────────────────────────────    │
│   /workflow   Start a full AI coding workflow (discuss→plan→ship)      │
│   /model      Switch the active model                                  │
│   /resume     Continue a previous session                              │
│   /settings   Configure providers, models, permissions                 │
│   /help       Show all available commands                              │
│                                                                         │
│  Or just type a goal and press Enter:                                  │
│  "Add dark mode to the settings page"                                  │
│  "Fix the race condition in the websocket handler"                     │
│  "Write tests for the authentication service"                          │
│                                                                         │
│  ─────────────────────────────────────────────────────────────────     │
│  Session history: 0 messages  ·  ctrl+p for all commands              │
╰─────────────────────────────────────────────────────────────────────────╯
```

---

### 3.3 REDESIGNED: `ScreenModelSelector` — "Model Observatory"

**Current problem:** A plain bubbles/list. No visualization of pricing differences. No comparison mode.

**Proposed redesign: Split-pane with cost sparkline + capability matrix**

```
╭─ Model Observatory ──────────────────────────────────────────────────────╮
│  / filter │  [All ▾] │  sorted: cost ↑ │  94 models                      │
│  ─────────────────────────────────────────────────────────────────────── │
│                                                                           │
│  ▶ ★  claude-3-5-sonnet-20241022              [OpenRouter]               │
│       Context: 200K  Cost: $3.00/$15.00 per M tokens                    │
│       ▁▂▃▄▅▆▇█  Used 14x this week                                      │
│                                                                           │
│    ★  claude-3-haiku-20240307                 [OpenRouter]               │
│       Context: 200K  Cost: $0.25/$1.25 per M tokens                     │
│       ▁▁▁▂▃▂▁▁  Used 3x this week                                       │
│                                                                           │
│       gpt-4o-mini                             [OpenRouter]               │
│       Context: 128K  Cost: $0.15/$0.60 per M tokens                     │
│                                                                           │
│       gemini-2.0-flash                        [OpenRouter]               │
│       Context: 1M    Cost: Free / $0.40 per M tokens                    │
│                                                                           │
│       deepseek-chat                           [OpenRouter]               │
│       Context: 64K   Cost: $0.14/$0.28 per M tokens                     │
│                                                                           │
│  ─────────────────────────────────────────────────────────────────────── │
│                                                                           │
│  ╭───────────── DETAIL: claude-3-5-sonnet-20241022 ───────────────────╮ │
│  │  Provider: OpenRouter · ID: anthropic/claude-3-5-sonnet-20241022   │ │
│  │                                                                     │ │
│  │  Context Window  ████████████████████░░░░░░  200K / 200K          │ │
│  │  Input Cost      ████░░░░░░░░░░░░░░░░░░░░░░  $3.00 / M tkn        │ │
│  │  Output Cost     ████████████░░░░░░░░░░░░░░  $15.00 / M tkn       │ │
│  │                                                                     │ │
│  │  Capabilities: ✓ Thinking  ✓ Vision  ✓ Function Calling           │ │
│  │  Architecture:  claude3.5  Release: 2024-10-22                     │ │
│  ╰─────────────────────────────────────────────────────────────────── ╯ │
│                                                                           │
│  ↑↓ navigate · Enter select · Tab detail · F favorite · P filter · / search │
╰───────────────────────────────────────────────────────────────────────────╯
```

**Key changes:**
1. **Sparkline row** per model — Shows usage frequency with braille/block characters
2. **Cost visualization** — Input/output cost shown side-by-side
3. **Capability badges** — Thinking, Vision, Function Calling shown as checkmarks
4. **Detail pane** with horizontal bar charts for cost comparison
5. **★ favorites** — Star indicator on left of model row (already in code, needs visual weight)

---

### 3.4 REDESIGNED: `ScreenSettings` — "Control Tower"

**Current problem:** Tab bar uses plain background colors. Field list has minimal visual structure. The `▶` cursor is the only focus indicator.

**Proposed redesign: Icon tabs + two-column layout + description pane**

```
╭─ Settings ───────────────────────────────────────────────────────────────╮
│                                                                           │
│  ⚙ General  │  🔑 Provider  │  🤖 Model  │  🛡 Permissions  │  Features  │
│  ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━     │
│                                                                           │
│  ┌─────────────────────────────┬───────────────────────────────────────┐ │
│  │  General Settings           │  Theme                                │ │
│  │                             │  ─────────────────────────────────── │ │
│  │  ▶ Theme          dark      │  Controls the color scheme of M31A.  │ │
│  │    Compact Mode   false     │  Options: "dark", "light", "auto"    │ │
│  │    Token Usage    true      │                                       │ │
│  │    Cost Estimate  true      │  "auto" follows your terminal's       │ │
│  │    Max Iterations 10        │  reported background color.           │ │
│  │                             │                                       │ │
│  │                             │  Current: dark                        │ │
│  │                             │  ╭──────────────╮                    │ │
│  │                             │  │  dark        │ ← active            │ │
│  │                             │  │  light       │                    │ │
│  │                             │  │  auto        │                    │ │
│  │                             │  ╰──────────────╯                    │ │
│  └─────────────────────────────┴───────────────────────────────────────┘ │
│                                                                           │
│  [Ctrl+S] Save  [Tab] Next Tab  [Shift+Tab] Prev Tab  [Esc] Discard      │
│  ● 1 unsaved change                                                       │
╰───────────────────────────────────────────────────────────────────────────╯
```

**Key changes:**
1. **Icon tabs** — Each tab has a Unicode emoji/icon prefix for faster visual scanning
2. **Two-column layout** — Left: field list, Right: description + context for focused field
3. **Inline dropdown** — For enum fields (like Theme), show a mini-dropdown in the description pane instead of inline text editing
4. **Unsaved indicator** — Bottom left `● N unsaved changes` in Warning color (not just a `▶` on the dirty field)
5. **Active tab underline** — `━━━` underline on the active tab using `BlockBorder`

---

### 3.5 REDESIGNED: `ScreenResume` — "Session Vault"

**Current problem:** The `bubbles/list` delegate is unstyled beyond colors. The preview pane appears only at width > 100. The filter chips are minimal.

**Proposed redesign: Timeline view with session cards + rich preview**

```
╭─ Session Vault ──────────────────────────────────────────────────────────╮
│  🔍  Search sessions...                                              3/47 │
│  ─────────────────────────────────────────────────────────────────────── │
│  Phase  [all ●] [idle] [discuss] [plan] [execute] [verify] [ship]        │
│  ─────────────────────────────────────────────────────────────────────── │
│                                                                           │
│  TODAY                                                                    │
│  ╭─────────────────────────────────────────────────────────────────────╮ │
│  │  ▶  a3f9c2be  ●  claude-3-5-sonnet          14:22  EXECUTE          │ │
│  │     "Refactor payment service to Stripe v14"                        │ │
│  │     24 messages · OpenRouter · 2h 14m                              │ │
│  ╰─────────────────────────────────────────────────────────────────────╯ │
│  ╭─────────────────────────────────────────────────────────────────────╮ │
│  │     b7d1e4f2     gpt-4o-mini                 11:08  SHIP            │ │
│  │     "Add pagination to user list endpoint"                          │ │
│  │     18 messages · OpenRouter · 45m                                 │ │
│  ╰─────────────────────────────────────────────────────────────────────╯ │
│                                                                           │
│  YESTERDAY                                                                │
│  ╭─────────────────────────────────────────────────────────────────────╮ │
│  │     c2a8d6e1     claude-3-haiku              09:30  IDLE             │ │
│  │     "Write unit tests for auth service"                             │ │
│  │     11 messages · OpenRouter · 22m                                 │ │
│  ╰─────────────────────────────────────────────────────────────────────╯ │
│                                                                           │
│  Enter: open  ·  N: new session  ·  D: delete  ·  /: search  ·  Esc: back │
╰───────────────────────────────────────────────────────────────────────────╯
```

**With preview (wide terminal):**

```
╭─ Session Vault ──────────────────────────────┬──── Preview ─────────────╮
│ TODAY                                        │  a3f9c2be                │
│ ▶ a3f9c2be  claude-3-5-sonnet  14:22 EXEC   │  claude-3-5-sonnet       │
│   b7d1e4f2  gpt-4o-mini       11:08 SHIP    │  OpenRouter              │
│                                              │  Started: Today 14:22    │
│ YESTERDAY                                    │  24 messages · 2h 14m    │
│   c2a8d6e1  claude-3-haiku    09:30 IDLE    │                          │
│                                              │  Phase: EXECUTE ██████░░ │
│                                              │                          │
│                                              │  Goal:                   │
│                                              │  Refactor payment        │
│                                              │  service to Stripe v14   │
│                                              │                          │
│                                              │  First message:          │
│                                              │  "Please update the      │
│                                              │  payment service to use  │
│                                              │  the new Stripe API..."  │
╰──────────────────────────────────────────────┴──────────────────────────╯
```

**Key changes:**
1. **Date groupings** — Sessions grouped by TODAY / YESTERDAY / dates instead of flat list
2. **Session cards** — Each session is a rounded-border card with the goal text visible
3. **Phase badge** — Workflow phase shown with a progress-ish indicator (EXECUTE phase = ~66% of the way through)
4. **● indicator** — Active dot for the last-used session
5. **Timeline labels** — YESTERDAY, TODAY, JUN 03, etc. as section headers

---

### 3.6 REDESIGNED: `ScreenPlan` — "Blueprint"

**Current problem:** The Plan screen is just a plain list of tasks. The dependency graph is basic ASCII tree. No file impact visualization.

**Proposed redesign: Kanban-style with timeline + impact summary**

```
╭─ Blueprint ── 8 tasks ── Est. $0.12 ── claude-3-5-sonnet ──────────────╮
│                                                                         │
│  [A]ccept  [R]etry  [D]iff  [O]ptimize  Tab=Graph  Esc=Back            │
│  ──────────────────────────────────────────────────────────────────     │
│                                                                         │
│  ╔═══════════════════════════════════════════════════════════════════╗  │
│  ║  TASK #3 (selected)                                              ║  │
│  ║  Update Stripe webhook signature verification                     ║  │
│  ║  Depends on: #1, #2  ·  Blocks: #5, #6  ·  Files: 2             ║  │
│  ╚═══════════════════════════════════════════════════════════════════╝  │
│                                                                         │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │  [ ] 1  Install stripe-node v14 dependency              NEW ✦   │   │
│  │  [ ] 2  Update TypeScript type definitions              MOD ~   │   │
│  │  [▶] 3  Update webhook signature verification           MOD ~   │   │  ← selected
│  │  [ ] 4  Migrate charges.create → paymentIntents         MOD ~   │   │
│  │  [ ] 5  Add idempotency keys                            MOD ~   │   │
│  │  [ ] 6  Update error handling for new API               MOD ~   │   │
│  │  [ ] 7  Update integration tests                        MOD ~   │   │
│  │  [ ] 8  Update documentation                            MOD ~   │   │
│  └─────────────────────────────────────────────────────────────────┘   │
│                                                                         │
│  ── File Impact ───────────────────────────────────────────────────     │
│  package.json             ✦ NEW                                         │
│  src/services/payment.ts  ~ MOD  ████████████░░░░  ~200 lines changed  │
│  src/webhooks/stripe.ts   ~ MOD  ████░░░░░░░░░░░░  ~80 lines changed   │
│  src/types/stripe.d.ts    ~ MOD  ██░░░░░░░░░░░░░░  ~30 lines changed   │
│  tests/payment.test.ts    ~ MOD  ██████░░░░░░░░░░  ~120 lines changed  │
│                                                                         │
╰─────────────────────────────────────────────────────────────────────────╯
```

**Dependency Graph view (Tab):**

```
╭─ Blueprint — Dependency Graph ─────────────────────────────────────────╮
│                                                                         │
│  ●─── #1: Install stripe-node v14                                       │
│  │                                                                       │
│  ●─── #2: Update TypeScript types                                        │
│  │                                                                       │
│  └──● #3: Update webhook sig ─────────────●─── #5: Add idempotency     │
│                                           │                             │
│         ●─── #4: Migrate charges ─────────────●─── #6: Error handling  │
│                                                                         │
│  ●─── #7: Update tests (dep: #3, #4, #5, #6)                           │
│  │                                                                       │
│  ●─── #8: Update documentation (dep: all)                               │
│                                                                         │
│  ─────────────────────────────────────────────────────────────────      │
│  ●  root task  ──  dependency edge  [Esc] back to task list             │
╰─────────────────────────────────────────────────────────────────────────╯
```

**Key changes:**
1. **Selected task detail box** — Double-border box above the list showing full detail of selected task
2. **File impact section** — Shows which files will change with estimated line counts
3. **Action badges** — `✦ NEW`, `~ MOD`, `✗ DEL` badges per task with color
4. **Horizontal dependency graph** — Left-to-right flow instead of vertical tree
5. **Arbitrage panel** — [O]ptimize key opens a mini-panel showing cost optimization suggestions

---

### 3.7 REDESIGNED: `ScreenExecute` — "Mission Live"

**Current problem:** The execution screen is functional but the running task indicator is minimal. Tool output shown in a single `toolCard` string field with no scrolling.

**Proposed redesign: Live dashboard with per-task panels + streaming tool output**

```
╭─ Mission Live ── ▶ Running ─────────────────────────────────────────────╮
│                                                                         │
│  Elapsed: 2m 14s  │  3/8 tasks  │  47 tool calls  │  $0.08  │  89K ctx │
│  ─────────────────────────────────────────────────────────────────────  │
│                                                                         │
│  ████████████████░░░░░░░░░░░░░░░░░░░░░░  3/8 complete                  │
│                                                                         │
│  ╔═══════════════════════════════════════════════════════════════════╗  │
│  ║ ▶ RUNNING  Task #3: Update webhook signature verification         ║  │
│  ║                                                                   ║  │
│  ║   ╭─────────────────────────────────────────────────────────╮   ║  │
│  ║   │  🔧 FileRead  src/webhooks/stripe.ts  ✓ 312ms  89 lines │   ║  │
│  ║   ├─────────────────────────────────────────────────────────┤   ║  │
│  ║   │  🔧 Bash  git log --oneline src/webhooks/stripe.ts      │   ║  │
│  ║   │  ⟳ running...                                           │   ║  │
│  ║   ╰─────────────────────────────────────────────────────────╯   ║  │
│  ╚═══════════════════════════════════════════════════════════════════╝  │
│                                                                         │
│  ✓  1  Install stripe-node v14                            0m 23s  12✦  │
│  ✓  2  Update TypeScript type definitions                 0m 41s   8✦  │
│  ▶  3  Update webhook signature verification              1m 12s  27✦  │
│  ·  4  Migrate charges.create → paymentIntents                          │
│  ·  5  Add idempotency keys          [blocked by: #4]                  │
│  ·  6  Update error handling                                            │
│  ·  7  Update integration tests      [blocked by: #3, #4, #5, #6]     │
│  ·  8  Update documentation          [blocked by: all]                 │
│                                                                         │
│  P=Pause  S=Skip  ↑↓=Navigate                                          │
╰─────────────────────────────────────────────────────────────────────────╯
```

**Key changes:**
1. **Live metrics bar** — Elapsed / tasks / tool calls / cost / ctx in a single dense bar
2. **Running task panel** — Double-border "RUNNING" box shows the active task with its tool card stream
3. **Tool card list inside running panel** — Multiple tool cards shown with ✓ for completed, ⟳ for in-progress
4. **Compact task list below** — Done tasks show elapsed time and tool call count (`12✦`)
5. **Block indicators** — Blocked tasks show which task IDs are blocking them

---

### 3.8 REDESIGNED: `ScreenVerify` — "QA Gate"

**Current problem:** Verification results are rendered as plain text with manual ✓/✗ strings. The self-heal confirmation is a basic string prompt.

**Proposed redesign: Per-task result panels with diff preview + heal status**

```
╭─ QA Gate ── Verification Results ──────────────────────────────────────╮
│                                                                         │
│  ─── 3 tasks verified ─── 1 warning ─── 0 failures ───────────────     │
│                                                                         │
│  ╭──────────────────────────────────────────────────────────────────╮  │
│  │  ✓  Task #1: Install stripe-node v14                             │  │
│  │     ✓ Files exist  ·  ✓ Syntax OK  ·  ✓ Tests pass              │  │
│  ╰──────────────────────────────────────────────────────────────────╯  │
│  ╭──────────────────────────────────────────────────────────────────╮  │
│  │  ✓  Task #2: Update TypeScript type definitions                  │  │
│  │     ✓ Files exist  ·  ✓ Syntax OK  ·  ✓ Tests pass              │  │
│  ╰──────────────────────────────────────────────────────────────────╯  │
│  ╭──────────────────────────────────────────────────────────────────╮  │
│  │  ⚠  Task #3: Update webhook signature verification          ▼   │  │  ← selected
│  │     ✓ Files exist  ·  ✓ Syntax OK  ·  ✗ Tests: 2 failures       │  │
│  │                                                                  │  │
│  │  Error details:                                                  │  │
│  │  ✗ FAIL  tests/webhooks/stripe.test.ts:42                        │  │
│  │    Expected: 200, received: 400                                  │  │
│  │    Webhook signature mismatch on test payload                    │  │
│  │                                                                  │  │
│  │  ┌──────────────────────────────────────────────────────────┐   │  │
│  │  │  [H] Self-heal (2 attempts remaining)  ·  [S] Skip task  │   │  │
│  │  └──────────────────────────────────────────────────────────┘   │  │
│  ╰──────────────────────────────────────────────────────────────────╯  │
│                                                                         │
│  Tasks 4-8 still running or pending...                                  │
│  ─────────────────────────────────────────────────────────────────────  │
│  ↑↓ navigate  ·  H=Self-heal  ·  S=Skip  ·  Esc=Back                   │
╰─────────────────────────────────────────────────────────────────────────╯
```

**Key changes:**
1. **Result cards** — Each task has its own rounded-border card (pass = thin border, fail/warn = Warning/Error colored border)
2. **Summary bar** — `─── 3 tasks verified ─── 1 warning ─── 0 failures ───`
3. **Error details inline** — Test failures expand within the card, no separate overlay
4. **Action row** inside card — [H] Self-heal and [S] Skip shown directly in the failing card's footer
5. **Self-heal confirmation overlay** — When H is pressed, an overlay appears centered on screen:

```
╔═══════════════════════════════════════╗
║  Self-Heal: Task #3                   ║
║  ─────────────────────────────────── ║
║  M31A will re-analyze the test       ║
║  failures and attempt a fix.         ║
║                                      ║
║  Attempts remaining: 2               ║
║                                      ║
║  [Y / Enter]  Attempt heal           ║
║  [N / Esc]    Cancel                 ║
╚═══════════════════════════════════════╝
```

---

### 3.9 REDESIGNED: `ScreenShip` — "Launch Pad"

**Proposed redesign: Git commit review + PR summary + ship confirmation**

```
╭─ Launch Pad ── Ready to Ship ──────────────────────────────────────────╮
│                                                                         │
│  ── Commits (5) ────────────────────────────────────────────────────   │
│                                                                         │
│  ▶  a3f2b1c  feat: install stripe-node v14                            │
│     b7e9d4f  feat: update TypeScript type definitions                  │
│     c2a8d6e  fix: update webhook signature verification               │
│     d1f3a9b  feat: migrate charges.create to paymentIntents           │
│     e4c7b2a  feat: add idempotency keys to all write operations        │
│                                                                         │
│  ── Diff Summary ─────────────────────────────────────────────────── ─ │
│  package.json            ✦  +12  -2                                     │
│  src/services/payment.ts ~  +89  -34                                    │
│  src/webhooks/stripe.ts  ~  +67  -28                                    │
│  src/types/stripe.d.ts   ~  +23  -0                                     │
│  tests/payment.test.ts   ~  +91  -12                                    │
│                                                                         │
│  Total: +282 lines added  /  -76 lines removed  /  5 files changed     │
│                                                                         │
│  ── Ship Action ──────────────────────────────────────────────────── ─ │
│  ╭──────────────────────────────────────────────────────────────────╮  │
│  │  [S] Ship: push branch + open PR                                 │  │
│  │  [C] Copy: copy commit hashes to clipboard                       │  │
│  │  [R] Rollback: revert commits via /rollback                      │  │
│  │  [Esc] Return to REPL without shipping                           │  │
│  ╰──────────────────────────────────────────────────────────────────╯  │
│                                                                         │
╰─────────────────────────────────────────────────────────────────────────╯
```

---

## 4. New Proposed Screens

---

### 4.1 NEW: `ScreenLedger` — "Learning Journal"

**Concept:** Surface the `~/.m31a/LEDGER.md` content as a readable, browsable TUI screen. The ledger tracks learned patterns, decisions, and surprises across sessions.

**Access:** `/ledger` command or `L` key from Settings screen.

```
╭─ Learning Journal ── LEDGER.md ────────────────────────────────────────╮
│                                                                         │
│  [All] [Decisions] [Patterns] [Surprises] [Errors]   / search          │
│  ─────────────────────────────────────────────────────────────────────  │
│                                                                         │
│  ── 2026-06-05 ───────────────────────────────────────────────────── ─ │
│                                                                         │
│  ╭──────────────────────────────────────────────────────────────────╮  │
│  │  PATTERN  Use stripe.paymentIntents instead of stripe.charges    │  │
│  │  ─────────────────────────────────────────────────────────────── │  │
│  │  Stripe v14 removes the charges API entirely. Always migrate      │  │
│  │  to paymentIntents when updating stripe-node.                    │  │
│  │  Session: a3f9c2be  ·  Jun 5 14:22                              │  │
│  ╰──────────────────────────────────────────────────────────────────╯  │
│                                                                         │
│  ╭──────────────────────────────────────────────────────────────────╮  │
│  │  DECISION  Always add idempotency keys to Stripe write calls     │  │
│  │  ─────────────────────────────────────────────────────────────── │  │
│  │  Network retries can cause duplicate charges. Idempotency keys   │  │
│  │  prevent this at the API level.                                  │  │
│  │  Session: a3f9c2be  ·  Jun 5 14:22                              │  │
│  ╰──────────────────────────────────────────────────────────────────╯  │
│                                                                         │
│  Total entries: 47  ·  ↑↓ browse  ·  / search  ·  Esc back            │
╰─────────────────────────────────────────────────────────────────────────╯
```

**Model struct:**
```go
type LedgerModel struct {
    theme       theme.Theme
    entries     []ledger.Entry
    filtered    []ledger.Entry
    selected    int
    width       int
    height      int
    searchInput textinput.Model
    filterTab   int  // 0=all, 1=decisions, 2=patterns, 3=surprises, 4=errors
    viewport    viewport.Model
}
```

---

### 4.2 NEW: `ScreenRollback` — "Time Machine"

**Concept:** Surface the `pkg/rollback` browser as a TUI screen. Shows the commit chain with diff preview, allowing the user to pick a rollback point.

**Access:** `/rollback` command.

```
╭─ Time Machine ── Commit History ───────────────────────────────────────╮
│                                                                         │
│  Branch: feat/stripe-v14  ·  5 commits ahead of main                  │
│  ─────────────────────────────────────────────────────────────────────  │
│                                                                         │
│  ▶  ●  e4c7b2a  feat: add idempotency keys          Jun 5 15:41  NOW  │
│     │  ●  d1f3a9b  feat: migrate charges.create     Jun 5 15:28       │
│     │  ●  c2a8d6e  fix: update webhook sig          Jun 5 15:10       │
│     │  ●  b7e9d4f  feat: update TypeScript types    Jun 5 14:55       │
│     │  ●  a3f2b1c  feat: install stripe-node v14    Jun 5 14:40       │
│        ●  HEAD~5   (main branch point)                                 │
│                                                                         │
│  ── Diff Preview: feat: add idempotency keys ──────────────────────    │
│                                                                         │
│  src/services/payment.ts                                                │
│  ─────────────────────────────────────────────────                     │
│  + const idempotencyKey = generateIdempotencyKey(userId, amount);      │
│  - const result = await stripe.paymentIntents.create({                 │
│  + const result = await stripe.paymentIntents.create({                 │
│  +   idempotencyKey,                                                    │
│                                                                         │
│  [R] Rollback to this point  ·  [D] View full diff  ·  Esc back        │
╰─────────────────────────────────────────────────────────────────────────╯
```

**Model struct:**
```go
type RollbackModel struct {
    theme     theme.Theme
    commits   []git.CommitInfo
    selected  int
    width     int
    height    int
    diffView  viewport.Model
    showDiff  bool
    rollback  *rollback.Rollback
}
```

---

### 4.3 NEW: `ScreenGoalInput` — "Mission Brief"

**Concept:** A dedicated full-screen goal input for starting a `/workflow` run. Currently the workflow goal is entered in the REPL textarea, which is cramped. A dedicated screen allows multi-line goal input with context.

**Access:** `/workflow start` or `W` shortcut from REPL.

```
╭─ Mission Brief ── New Workflow ────────────────────────────────────────╮
│                                                                         │
│  Define your goal. Be specific — M31A will use this to plan tasks.    │
│  ─────────────────────────────────────────────────────────────────────  │
│                                                                         │
│  ╭──────────────────────────────────────────────────────────────────╮  │
│  │                                                                  │  │
│  │  Refactor the payment service to use the new Stripe v14 API.    │  │
│  │  The old code uses stripe.charges which is deprecated. We need  │  │
│  │  to migrate to stripe.paymentIntents and add idempotency keys.  │  │
│  │                                                                  │  │
│  │  Acceptance criteria:                                            │  │
│  │  - All existing tests continue to pass                           │  │
│  │  - New tests cover idempotency behavior                          │  │
│  │  - PR description updated with migration notes                   │  │
│  │  ▌                                                               │  │
│  ╰──────────────────────────────────────────────────────────────────╯  │
│                                                                         │
│  ── Context ────────────────────────────────────────────────────────   │
│  Working directory:  /home/user/projects/myapp                         │
│  Model:              claude-3-5-sonnet · OpenRouter                    │
│  Git branch:         main                                              │
│                                                                         │
│  ── Shortcuts ─────────────────────────────────────────────────────    │
│  @path/to/file  Include file content  │  !cmd  Run shell command       │
│                                                                         │
│  [Enter] Start workflow  ·  [Esc] Cancel  ·  [Ctrl+E] Edit in $EDITOR │
╰─────────────────────────────────────────────────────────────────────────╯
```

**Model struct:**
```go
type GoalInputModel struct {
    theme       theme.Theme
    textarea    textarea.Model
    width       int
    height      int
    model       *types.ModelInfo
    provider    string
    cwd         string
    gitBranch   string
}
```

---

### 4.4 NEW: `ScreenDiscuss` — "Discovery Room"

**Concept:** The existing Discuss phase (Q&A flow) is handled inline in the REPL screen with messages. A dedicated screen gives it proper visual treatment with a progress indicator showing which question you're on.

**Access:** Automatically shown when workflow enters Discuss phase.

```
╭─ Discovery Room ── Understanding your goal ────────────────────────────╮
│                                                                         │
│  M31A needs to understand your goal before planning.                  │
│  ─────────────────────────────────────────────────────────────────────  │
│                                                                         │
│  Question 2 of 4  ██████░░░░░░░░░░░░░░  50%                           │
│                                                                         │
│  ╭──────────────────────────────────────────────────────────────────╮  │
│  │  M31A asks:                                                      │  │
│  │                                                                  │  │
│  │  Are there any existing tests for the payment service that       │  │
│  │  I should be aware of? And should I update them as part of      │  │
│  │  this migration, or handle tests separately?                    │  │
│  │                                                                  │  │
│  ╰──────────────────────────────────────────────────────────────────╯  │
│                                                                         │
│  ── Previous Q&A ──────────────────────────────────────────────────    │
│  Q1: What Stripe version is currently installed?                        │
│  A1: "We're on stripe-node v10. Need to go to v14."                   │
│                                                                         │
│  ── Your Answer ───────────────────────────────────────────────────    │
│  ╭──────────────────────────────────────────────────────────────────╮  │
│  │  Yes, there are tests in tests/payment.test.ts. Please update   │  │
│  │  them as part of this migration.                                 │  │
│  │  ▌                                                               │  │
│  ╰──────────────────────────────────────────────────────────────────╯  │
│                                                                         │
│  [Enter] Submit  ·  [/skip] Skip question  ·  [/done] Finalize         │
│  Time remaining: 4:32                                                   │
╰─────────────────────────────────────────────────────────────────────────╯
```

**Model struct:**
```go
type DiscussModel struct {
    theme          theme.Theme
    question       string
    questionIndex  int
    totalQuestions int
    previousQA     []struct{ Q, A string }
    textarea       textarea.Model
    timer          int  // seconds remaining
    width          int
    height         int
    spinner        spinner.Model
}
```

---

### 4.5 NEW: `ScreenMetrics` — "Flight Data"

**Concept:** A summary screen showing aggregate usage statistics across all sessions. Surfaces token usage, cost, model distribution, etc.

**Access:** `/metrics` command.

```
╭─ Flight Data ── Session Analytics ─────────────────────────────────────╮
│                                                                         │
│  All time  ·  47 sessions  ·  Last 30 days                            │
│  ─────────────────────────────────────────────────────────────────────  │
│                                                                         │
│  ── Usage Overview ─────────────────────────────────────────────────   │
│  ╭──────────────╮  ╭──────────────╮  ╭──────────────╮  ╭────────────╮ │
│  │  1.2M        │  │  $4.32       │  │  47          │  │  312       │ │
│  │  Total Tokens│  │  Total Cost  │  │  Sessions    │  │  Tool Calls│ │
│  ╰──────────────╯  ╰──────────────╯  ╰──────────────╯  ╰────────────╯ │
│                                                                         │
│  ── Daily Usage (last 14 days) ─────────────────────────────────────   │
│  Jun 05  ████████████████████  8 sessions                              │
│  Jun 04  ████████████          5 sessions                              │
│  Jun 03  ████                  2 sessions                              │
│  Jun 02  ████████              4 sessions                              │
│  Jun 01  ████████████████      7 sessions                              │
│                                                                         │
│  ── Model Distribution ──────────────────────────────────────────────  │
│  claude-3-5-sonnet   ████████████████░░░░  62%  ·  29 sessions        │
│  claude-3-haiku      ████████░░░░░░░░░░░░  31%  ·  15 sessions        │
│  gpt-4o-mini         ██░░░░░░░░░░░░░░░░░░   7%  ·   3 sessions        │
│                                                                         │
│  ── Cost by Model ───────────────────────────────────────────────────  │
│  claude-3-5-sonnet   $3.21 (74%)                                       │
│  claude-3-haiku      $0.87 (20%)                                       │
│  gpt-4o-mini         $0.24  (6%)                                       │
│                                                                         │
│  Esc back                                                               │
╰─────────────────────────────────────────────────────────────────────────╯
```

---

### 4.6 NEW: `ScreenPermissionRequest` (Redesigned Modal)

**Current problem:** The permission modal is a `components.PermissionModal` but the visual is not examined in the rush reports. Based on the dispatcher code, it shows a blocking dialog.

**Proposed full-screen modal redesign:**

```
╔═══════════════════════════════════════════════════════════════════════════╗
║                                                                           ║
║   ⚠  Permission Required                                                 ║
║   ─────────────────────────────────────────────────────────────────────  ║
║                                                                           ║
║   M31A wants to execute:                                                  ║
║                                                                           ║
║   ╭──────────────────────────────────────────────────────────────────╮  ║
║   │  🔧 Bash                                                         │  ║
║   │  rm -rf node_modules && npm install                              │  ║
║   ╰──────────────────────────────────────────────────────────────────╯  ║
║                                                                           ║
║   This will delete the node_modules directory and reinstall packages.    ║
║                                                                           ║
║   Task: #1 — Install stripe-node v14                                      ║
║                                                                           ║
║   ──────────────────────────────────────────────────────────────────     ║
║                                                                           ║
║   [Y / Enter]  Allow once      [A] Allow always for this tool            ║
║   [N / Esc]    Deny            [D] Deny always for this tool              ║
║                                                                           ║
║   Auto-deny in: 28s  ████████████████████████░░░░░░░░░░                 ║
║                                                                           ║
╚═══════════════════════════════════════════════════════════════════════════╝
```

**Key changes:**
1. **Double border** signals "blocking modal" semantics
2. **Tool card** shows exactly what command will run
3. **Task context** shows which task triggered this
4. **Countdown bar** with half-block characters showing time remaining before auto-deny
5. **"Allow always"** action to reduce future interruptions

---

## 5. Header & Chrome Redesign

### 5.1 New Header Layout

**Current:** `M31A [OR] ModelName --/-- ctx [LIVE]`

**Proposed:** Full-width header bar with gradient-like feel using block characters

```
▓▓▓ M31A ▓  ◆ OpenRouter  claude-3-5-sonnet  ─────────────  45.2K/200K  [LIVE]
```

- Left anchor: `▓▓▓ M31A ▓` — heavy block characters give weight
- Provider badge: colored per provider (violet=OpenRouter, cyan=Zen)
- Model name: truncated, neutral color
- Context bar: right-aligned, color-coded (green<80%, yellow<95%, red≥95%)
- Health: `[LIVE]`/`[SLOW]`/`[OFF]` — colored appropriately

### 5.2 New StatusBar Layout

**Current:** `Ready` (left) + `ctrl+p commands` (right)

**Proposed:** Multi-segment status bar that changes per screen

```
▸ discuss  ·  question 2/4  ·  4:28 remaining              ctrl+p  ctrl+b  ?
```
or during streaming:
```
⟳ thinking...  claude-3-5-sonnet  ·  ctrl+c to cancel       45.2K ctx  $0.002
```
or idle:
```
Ready                                          ctrl+p commands  ·  ctrl+b sidebar
```

### 5.3 Workflow Phase Indicator

When a workflow is running, add a **phase breadcrumb** below the header:

```
▓▓▓ M31A ▓  ◆ OpenRouter  claude-3-5-sonnet  ─────────────  45.2K/200K  [LIVE]
  DISCUSS ─── PLAN ─── EXECUTE ─── VERIFY ─── SHIP
               ↑ current phase
```

The phase trail uses:
- Past phases: `─── ` separator, muted color
- Current phase: bold + Brand color + `↑` pointer below
- Future phases: muted, no decoration

---

## 6. New Screen Registration

To add the proposed screens, the following changes are needed:

### 6.1 New Screen Constants

```go
// In internal/tui/types.go
const (
    ScreenFirstRun       Screen = iota
    ScreenREPL
    ScreenModelSelector
    ScreenSettings
    ScreenResume
    ScreenPermission
    ScreenPlan
    ScreenExecute
    ScreenVerify
    ScreenShip
    ScreenDiff           Screen = 10
    // NEW SCREENS:
    ScreenLedger         Screen = 11
    ScreenRollback       Screen = 12
    ScreenGoalInput      Screen = 13
    ScreenDiscuss        Screen = 14
    ScreenMetrics        Screen = 15
)
```

### 6.2 AppState Extensions

```go
// New fields in AppState
type AppState struct {
    // ... existing fields ...
    
    // New screens
    ledgerModel     *LedgerModel
    rollbackModel   *RollbackModel
    goalInputModel  *GoalInputModel
    discussModel    *DiscussModel
    metricsModel    *MetricsModel
    
    // Phase breadcrumb visibility
    showPhaseBreadcrumb bool
}
```

### 6.3 New Commands

| Command | Action |
|---------|--------|
| `/ledger` | Opens ScreenLedger |
| `/rollback` | Opens ScreenRollback |
| `/metrics` | Opens ScreenMetrics |
| `/workflow start` | Opens ScreenGoalInput |
| Discuss phase trigger | Auto-opens ScreenDiscuss |

---

## 7. Theme System Enhancements

The current theme has all necessary colors. The following new style keys would be useful:

```go
// Additional style fields in theme.Theme
type Theme struct {
    // ... existing fields ...
    
    // New
    CardBorder        lipgloss.Style  // rounded card border, default brand
    CardBorderActive  lipgloss.Style  // focused card border, brand + bold
    CardBorderError   lipgloss.Style  // error card border, error color
    CardBorderWarn    lipgloss.Style  // warning card border, warning color
    
    PhaseActive       lipgloss.Style  // current workflow phase
    PhasePast         lipgloss.Style  // completed workflow phases
    PhaseFuture       lipgloss.Style  // upcoming workflow phases
    
    TimelineDate      lipgloss.Style  // session timeline date headers
    MetricValue       lipgloss.Style  // large metric numbers
    MetricLabel       lipgloss.Style  // metric label below the value
    
    BlockFull         string  // "█" for full block
    BlockHigh         string  // "▓" for high density
    BlockMed          string  // "▒" for medium density  
    BlockLow          string  // "░" for low density
}
```

---

## 8. Implementation Priority

### Phase A — Visual Polish (no new screens)
1. Redesign REPL message rendering (gutter + timestamp bars + tool card styling)
2. Redesign Plan screen (selected task detail box + file impact section)
3. Redesign Execute screen (running task panel + compact task list)
4. Redesign Settings screen (two-column layout + description pane)
5. Update Header (block-character anchors + phase breadcrumb)
6. Update StatusBar (streaming state + cost indicator)

### Phase B — New Screen Types
1. `ScreenDiscuss` — Dedicated Q&A flow (highest impact, replaces inline messages)
2. `ScreenGoalInput` — Full-screen goal entry for workflow start
3. `ScreenLedger` — Browse the learning journal
4. `ScreenRollback` — Commit time machine

### Phase C — Analytics & Advanced
1. `ScreenMetrics` — Session analytics dashboard
2. Enhanced `ScreenFirstRun` with stars background

---

## 9. Key Constraints Respected

All proposed changes strictly respect M31A's architectural rules:

| Rule | How respected |
|------|---------------|
| Bubble Tea single-threaded | All new models follow `Update()→View()` pattern; no goroutine state mutation |
| No CSS animations | Visual interest achieved via Unicode art, block chars, sparklines — not time-based animation |
| No direct Anthropic/OpenAI | All new screens read from existing session/workflow data, no new API calls |
| No telemetry | ScreenMetrics reads only local session files |
| No hardcoded model lists | Model selector still uses dynamic discovery |
| No CGO | All rendering is pure Go string manipulation |
| charmbracelet stack | All components use bubbletea, lipgloss, bubbles |

---

## 10. Design Mockup — Terminal Color Scheme Reference

```
SURFACE LAYERS (dark theme):
  Background:       #0F0F1A  ─── The void (outer terminal bg)
  Surface:          #1E1E2E  ─── Card backgrounds
  SurfaceElevated:  #2A2A3E  ─── Selected items, modal backgrounds

SEMANTIC COLORS:
  Brand:     #7C3AED  ▓▓▓  Violet — M31A's identity
  Accent:    #06B6D4  ▓▓▓  Cyan — interactive elements  
  Success:   #10B981  ▓▓▓  Emerald — pass, done, live
  Warning:   #F59E0B  ▓▓▓  Amber — caution, pending
  Error:     #EF4444  ▓▓▓  Red — failure, denied
  Thinking:  #8B5CF6  ▓▓▓  Purple — AI thinking state

TEXT HIERARCHY:
  TextPrimary:    #E2E8F0  Primary content
  TextSecondary:  #94A3B8  Supporting content
  TextMuted:      #475569  Hints, timestamps
```

---

## 11. Summary

This proposal transforms M31A's TUI from a **functional flat-text interface** into a **cohesive, premium terminal experience** with:

- **6 redesigned screens** with richer visual hierarchy, card-based layouts, and information density
- **6 new screens** covering workflows, learning, rollback, and analytics not currently surfaced in the TUI
- **Consistent design language** using a surface-elevation system, typed borders, and Unicode block characters
- **Zero breaking changes** to the core architecture — all additions work within Bubble Tea's single-threaded model
- **Full constraint compliance** with M31A's AGENTS.md rules (no telemetry, no hardcoded models, CGO-free)

The designs above are expressed as terminal ASCII art mockups — ready to be translated directly into `lipgloss.NewStyle()` and `strings.Builder` Go code.
