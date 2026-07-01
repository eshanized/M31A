# M31A v1.6.0 — Premium UX Overhaul

## Executive Summary

M31A's current TUI is information-rich but cognitively expensive. The sidebar displays 11 data sections simultaneously. Tool calls appear as raw API names. Thinking state shows timers instead of intent. The header and footer compete for attention with decorative elements. The permission modal exposes implementation details instead of explaining consequences.

The redesign follows one principle: **reduce cognitive load at every layer.** Every pixel must answer a question the user is actually asking. Everything else disappears.

**Target feel:** Linear's calm precision. Warp's information density without noise. Apple's "the interface disappears" philosophy.

---

# Part 1: Screen-by-Screen UX Audit

## Screen Inventory (33 screens)

| # | Screen | Current Purpose | Primary Issue |
|---|--------|----------------|---------------|
| 0 | FirstRun | Setup wizard | Acceptable — minimal by nature |
| 1 | REPL | Chat + tool execution | Tool rendering is debug-oriented |
| 2 | ModelSelector | Pick model | Works well as modal |
| 3 | Settings | App settings | Dense, acceptable for power users |
| 4 | Resume | Session list | Clear, low priority |
| 5 | Permission | Approve tool calls | Exposes raw implementation details |
| 6 | Plan | Workflow plan review | Card border adds unnecessary weight |
| 7 | Execute | Task progress | Progress info is implementation-focused |
| 8 | Verify | Build/test results | Self-healing UI is unclear |
| 9 | Ship | Session summary | Stats grid is useful but layout is rigid |
| 10 | Diff | Git diff viewer | Works, low priority |
| 11 | Ledger | History table | Information-dense, acceptable |
| 12 | Rollback | Revert commits | Clear action flow |
| 13 | GoalInput | Initial goal prompt | Good — focused |
| 14 | Discuss | Q&A clarification | Question card has unnecessary decoration |
| 15 | Metrics | Usage stats | Low priority |
| 16 | Config | TOML editor | Power-user screen, acceptable |
| 17 | Help | Keybindings | Low priority |
| 18 | Bisect | Git bisect helper | Low priority |
| 20 | Notifications | Alert list | Low priority |
| 21 | Dashboard | Overview | Information overload risk |
| 22 | SessionDetail | Session inspection | Low priority |
| 23 | FileExplorer | Browse files | Low priority |
| 24 | ToolDetail | Tool output inspector | Low priority |
| 25 | PhaseModelPicker | Model per phase | Low priority |
| 26 | GhostPicker | Ghost mode | Low priority |
| 27 | GhostOutput | Ghost results | Low priority |
| 28 | ConfirmQuit | Quit confirmation | Low priority |
| 29 | ChatHistory | Browse messages | Low priority |
| 30 | CommandPalette | Slash commands | Works well |
| 31 | RuntimeCheck | Dev server status | Low priority |
| 32 | Home | Landing screen | Decorative elements waste space |
| 33 | Decisions | Decision log | Low priority |

**High-impact screens requiring redesign:** REPL (1), Permission (5), Execute (7), Home (32), Discuss (14), Plan (6), Ship (9).

---

## Detailed Audit: REPL Screen (Primary Screen)

**User's goal:** Have a conversation with M31A. See responses. Understand what M31A is doing.

**Current issues:**

1. **Tool calls show raw API names.** `FileRead`, `Glob`, `Bash` are implementation details. The user doesn't care which tool was called — they care what M31A is doing with it.

2. **Sidebar shows 11 data sections simultaneously.**
   - M31A header + version
   - Git branch + file status pills (●2 +3 -1 ?2)
   - USAGE divider + context gauge + token count + burn rate + cost + model name
   - PHASE pipeline (✓I ✓D ●P ○E ○V ○S)
   - TOOLS timeline (active tool list)
   - SPEED metrics (tasks/min, avg, ETA)
   - Sub-agent badge
   - Pending permissions badge
   - File tree or TODO list
   - Session ID
   - Keyboard hints

   This is a firehose. Most of this is invisible to the user until they look for it. But the visual weight is constant.

3. **Header has decorative leader dots** (`········`) that serve no purpose other than filling space.

4. **Footer duplicates information.** The cwd and branch appear in both the footer AND the sidebar. Token count and cost appear in both the header context meter AND the sidebar usage section AND the footer cost display.

5. **Context meter appears in THREE places:** header right zone, sidebar usage gauge, footer context ring. This is triple-reporting of the same data.

6. **Thinking state shows a timer** ("thinking · 3.2s") instead of meaningful progress.

7. **Streaming state shows "streaming…"** — generic. Should show what's being streamed.

**What should disappear:**
- Leader dots in header
- Duplicate context/token/cost displays (keep ONE)
- Sidebar sections that are empty or static (version, session ID, hints)
- Decorative section dividers in sidebar

**What should become primary:**
- The conversation (LLM responses)
- Intent-level progress ("Reading project structure" not "FileRead ×3")

---

## Detailed Audit: Permission Screen

**User's goal:** Decide whether to allow a potentially dangerous action. Understand the risk.

**Current issues:**

1. **Shows raw tool name and command.** "Tool: Bash" + "Command: rm -rf ./dist" forces the user to parse implementation details.

2. **Risk level is a label** ("high", "destructive") rather than an explanation.

3. **No context about WHY** M31A wants to run this command.

4. **Countdown timer** creates urgency without explanation.

5. **Phase/goal context** is shown but in a secondary position.

**What should disappear:**
- Raw "Tool:" label
- Raw "Command:" label
- "Risk:" label with single-word level

**What should become primary:**
- What this action will do (plain English)
- Why M31A wants to do it (goal context)
- What could go wrong (consequence)
- The command itself (secondary, expandable)

---

## Detailed Audit: Execute Screen

**User's goal:** Understand progress. Know what's happening. Trust that work is proceeding.

**Current issues:**

1. **Progress line is implementation-focused:** "3/5 tasks (1 failed) 60% · 42s · ETA 28s" — this is a debug readout.

2. **Task list shows raw action descriptions** with status badges (✓ done, ✗ failed, ● running, ○ pending). These badges are visual noise.

3. **Live output shows raw terminal output** — useful but overwhelming.

4. **Key hints** at the bottom consume space.

**What should disappear:**
- Individual status badges on each task line
- Raw live output (collapse by default)
- Explicit key hints (move to footer)

**What should become primary:**
- Overall progress with intent ("Setting up project structure")
- Current activity ("Installing dependencies…")
- Failures only (expand when failed)

---

## Detailed Audit: Home Screen

**User's goal:** Start a conversation. Understand what M31A is.

**Current issues:**

1. **Logo with glow effect** is decorative. The brand is established by the header.

2. **Glow line** (`·····`) below logo is pure decoration.

3. **Tips row** at the bottom is useful but competes with the input.

4. **Version number** at the very bottom is noise.

**What should disappear:**
- Glow effect and glow line
- Version number in content area (keep in header/sidebar only)

**What should become primary:**
- The input prompt
- A single line describing what M31A does ("AI coding agent — describe your task")

---

## Detailed Audit: Discuss Screen

**User's goal:** Answer clarifying questions quickly.

**Current issues:**

1. **ThinBorder card** around the question adds visual weight.

2. **Section divider** above the card is decorative.

3. **Dot progress indicators** (✓ ● ○) are fine but could be more compact.

4. **Key hints at the bottom** are always the same — should be in the footer.

**What should disappear:**
- Card border around question
- Section divider
- Explicit key hints (footer handles this)

**What should become primary:**
- The question text
- Progress indicator
- Input field

---

## Detailed Audit: Plan Screen

**User's goal:** Review and approve a plan. Understand what will be built.

**Current issues:**

1. **Card border** around the plan adds visual weight.

2. **Footer with key hints** duplicates what the unified footer already shows.

3. **Version indicator** ("v2") is implementation detail.

**What should disappear:**
- Card border
- Duplicated key hints
- Version number

**What should become primary:**
- Plan content
- Approval action

---

## Detailed Audit: Ship Screen

**User's goal:** Understand what was accomplished. Feel confident in the result.

**Current issues:**

1. **Stats grid in a Card** is fine structurally but has unnecessary blank lines.

2. **"Session Summary" title** is redundant — the header already says "Ship".

3. **Commit log** is useful but could be more prominent.

**What should disappear:**
- Card title "Session Summary"
- Excessive blank lines in card content

**What should become primary:**
- Task completion status
- Files changed
- Commits created

---

# Part 2: Information Hierarchy

## Global Hierarchy (All Screens)

```
PRIMARY     What is happening right now (conversation, progress, action)
SECONDARY   Where am I (phase, context, location)
TERTIARY    System state (tokens, cost, health) — quiet, always available
HIDDEN      Debug details, raw metrics, implementation specifics
```

## Screen-Specific Hierarchy

### REPL (Primary Screen)

| Level | Content | Current Position | Proposed Position |
|-------|---------|-----------------|-------------------|
| PRIMARY | LLM response / conversation | Main viewport | Main viewport (unchanged) |
| PRIMARY | Tool intent ("Reading source files") | Not shown | Inline above tool group |
| SECONDARY | Current phase / workflow state | Footer center | Footer center |
| SECONDARY | Input prompt | Bottom | Bottom (unchanged) |
| TERTIARY | Context usage | Header + sidebar + footer | Footer only (single display) |
| TERTIARY | Git branch | Footer left + sidebar | Footer left only |
| TERTIARY | Cost | Footer right + sidebar | Sidebar only (collapsed) |
| HIDDEN | Token burn rate | Sidebar | Hidden (expand on hover/focus) |
| HIDDEN | Speed metrics | Sidebar | Hidden (expand on focus) |
| HIDDEN | Tool timeline | Sidebar | Replaced by intent narrative |
| HIDDEN | Session ID | Sidebar | Hidden (accessible via /session) |
| HIDDEN | Version | Sidebar header | Header breadcrumb only |

### Permission Screen

| Level | Content | Current | Proposed |
|-------|---------|---------|----------|
| PRIMARY | What will happen | "Tool: Bash" | Plain English description |
| PRIMARY | Why | Phase/goal (secondary) | Phase/goal (primary) |
| SECONDARY | The command | "Command: rm -rf" | Expandable detail |
| TERTIARY | Risk level | "Risk: high" | Inline color accent |
| HIDDEN | Countdown | Always visible | Only when <5s |

### Execute Screen

| Level | Content | Current | Proposed |
|-------|---------|---------|----------|
| PRIMARY | Overall progress | "3/5 tasks 60%" | "Setting up project structure" |
| PRIMARY | Current activity | Live output | "Installing dependencies…" |
| SECONDARY | Progress bar | Prominent | Subtle, single line |
| TERTIARY | Per-task status | All tasks shown | Only active + failed |
| HIDDEN | ETA / elapsed | Always shown | On demand |
| HIDDEN | Live output | Expanded | Collapsed, expandable |

### Home Screen

| Level | Content | Current | Proposed |
|-------|---------|---------|----------|
| PRIMARY | Input prompt | Center | Center |
| SECONDARY | Brand identity | Logo + glow | Logo (no glow) |
| TERTIARY | Keyboard tips | Bottom row | Footer only |
| HIDDEN | Version | Bottom | Removed from content |

---

# Part 3: Redesigned Wireframes

## Wireframe: Global Chrome

### Current Layout
```
┌─────────────────────────────────────────────────────────────┐
│ M31A │ chat ·························· gpt-4o [OR] [ctx]   │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  [sidebar 30w]  │  [main content]                           │
│                 │                                           │
├─────────────────────────────────────────────────────────────┤
│ ⌂ project ⎇ main │ ⠹ thinking · 3.2s │ ctrl+p · $0.05    │
└─────────────────────────────────────────────────────────────┘
```

### Proposed Layout
```
┌──────────────────────────────────────────────────────────────
│ M31A  chat                                              gpt-4o
│
│  [content — full width, no sidebar by default]
│
│
│
─ ⌂ project  main                        ctrl+p commands
```

**Changes:**
1. **Header:** Remove `│` separators. Remove leader dots. Remove provider badge. Remove context meter. Keep only: brand (left), breadcrumb (center-left), model name (right, muted).
2. **Footer:** Remove `│` separators. Left: cwd + branch (quiet). Center: operation (only when active). Right: hints (only when relevant). Remove cost from footer entirely.
3. **Sidebar:** Hidden by default. Activated by `ctrl+b` or during workflow phases. When visible, shows only contextually relevant sections (not all 11).

---

## Wireframe: REPL Screen (Conversation)

### Current
```
┃ ◆  I need to refactor the auth module...
┃
┃ ⟳ FileRead  auth.go
┃   1→ package auth
┃   2→ import "github.com/...
┃
┃ ✓ FileRead  auth.go  120ms
┃
┃ ⟳ Glob  *.go
┃   internal/auth.go
┃   internal/auth_test.go
┃
┃ ✓ Glob  *.go  45ms
┃
┃ ⟳ Grep  "func Login"
┃   auth.go:42
┃
┃ ✓ Grep  "func Login"  30ms
┃
┃ ◆  I'll refactor the auth module by...
```

### Proposed
```
  I need to refactor the auth module...

  Reading project structure

    auth.go
    auth_test.go

  Analyzing dependencies

    Login function at auth.go:42

  I'll refactor the auth module by extracting
  the authentication logic into a separate package
  with clear interfaces...

```

**Changes:**
1. **Tool calls grouped by intent.** Instead of showing `FileRead`, `Glob`, `Grep` individually, group them under an intent label: "Reading project structure", "Analyzing dependencies".
2. **Remove tool status icons** (⟳ ✓ ✗) from inline view. Show only the result summary.
3. **Remove tool duration** from inline view. Duration is a debug metric.
4. **Remove gutter border** (`┃ ◆`). Use simple indentation or spacing.
5. **LLM responses** use clean typography without the gutter decoration.
6. **Failed tools** get a single red line with context: "✗ Edit failed — file modified since read. Retrying…"

---

## Wireframe: Tool Groups (New Component)

Instead of individual tool cards, render tool groups:

```
  Reading project structure
    auth.go
    auth_test.go
    provider.go

  Building dependency graph

    Login function at auth.go:42
    User struct at models.go:15

  Planning implementation
```

**Grouping rules:**
- Consecutive `FileRead` + `Glob` + `Grep` within same intent = one group
- `Bash` = standalone (always visible, commands are user-facing)
- `Edit` = standalone (file modifications are important)
- Failed tool = always standalone, always visible

**Group renders as:**
```
  [Intent label — muted, italic]
    [file or result 1]
    [file or result 2]
    [file or result 3]
```

No icons. No durations. No status badges. The intent label communicates what's happening. The file list communicates scope.

---

## Wireframe: Permission Modal

### Current
```
┌─────────────────────────────────────────┐
│  Tool:  Bash                            │
│  Command:                               │
│      rm -rf ./dist && npm run build     │
│  Risk:  high                            │
│  Phase: execute                         │
│                                         │
│  [y] Allow  [n] Deny  [a] Allow All    │
└─────────────────────────────────────────┘
```

### Proposed
```
┌──────────────────────────────────────────────────┐
│                                                  │
│  M31A wants to clean and rebuild the project     │
│  output directory.                               │
│                                                  │
│  This will delete ./dist/ and run a fresh build. │
│                                                  │
│  ─────────────────────────────────────────────── │
│  $ rm -rf ./dist && npm run build               │
│                                                  │
│  [y] Allow  [n] Deny  [a] Allow All             │
│                                                  │
└──────────────────────────────────────────────────┘
```

**Changes:**
1. **Plain English description** as primary content. Not "Tool: Bash" — but "M31A wants to…"
2. **Consequence explanation** below the description.
3. **Command** is secondary, shown in a code block below a divider.
4. **Risk level** is communicated through color accent on the card border (green/yellow/red), not a text label.
5. **Phase/goal context** is woven into the description, not shown as a separate field.
6. **Countdown** only appears when urgency is real (<5s), not always.

---

## Wireframe: Execute Screen

### Current
```
3/5 tasks (1 failed) 60% · 42s · ETA 28s
████████████████████░░░░░░░░░░░░░░░░░░░░
────────────────────────────────────────
  1. ✓ done  Install dependencies
  2. ✓ done  Create auth module
  3. ● running  Run tests
  4. ✗ failed  Build project
  5. ○ pending  Deploy
j/k: scroll  p: pause  Esc: back
```

### Proposed
```
Running tests…

████████████░░░░░░░░░░░  3/5 complete

  ✓ Install dependencies
  ✓ Create auth module
  ● Running tests
    $ npm test
    > auth@1.0.0 test
    > jest --coverage
  ✗ Build project
    Error: Cannot find module './auth'
  ○ Deploy
```

**Changes:**
1. **Current activity** is the primary header, not a stats line.
2. **Progress bar** is subtle, single line.
3. **Completed tasks** are collapsed to a single line each (no badges, just ✓).
4. **Running task** shows current command output (collapsed).
5. **Failed task** shows error inline (red, expanded).
6. **Pending tasks** are dimmed.
7. **ETA / elapsed** removed from primary view. Accessible via key press.
8. **Key hints** moved to footer.

---

## Wireframe: Home Screen

### Current
```
       ██╗  ██╗██╗███████╗████████╗ ██████╗ ██████╗ ██╗   ██╗
       ██║  ██║██║██╔════╝╚══██╔══╝██╔═══██╗██╔══██╗╚██╗ ██╔╝
       ███████║██║███████╗   ██║   ██║   ██║██████╔╝ ╚████╔╝
       ██╔══██║██║╚════██║   ██║   ██║   ██║██╔══██╗  ╚██╔╝
       ██║  ██║██║███████║   ██║   ╚██████╔╝██║  ██║   ██║
       ╚═╝  ╚═╝╚═╝╚══════╝   ╚═╝    ╚═════╝ ╚═╝  ╚═╝   ╚═╝
                        ··················

  ┌─────────────────────────────────────┐
  │ Type your task...                   │
  └─────────────────────────────────────┘

  ctrl+p commands  ·  ctrl+b sidebar  ·  /help

  v1.5.0
```

### Proposed
``"



       M31A

  AI coding agent — describe your task

  ┌─────────────────────────────────────┐
  │ Type your task...                   │
  └─────────────────────────────────────┘

  ctrl+p commands    /help


```

**Changes:**
1. **Logo is simplified** — just the name in brand color, no ASCII art. The ASCII art is cool but wastes 6 rows and establishes a "hacker tool" vibe rather than a "premium tool" vibe.
2. **Tagline** explains what M31A is in one line.
3. **Glow line removed.**
4. **Tips reduced** to 2 items instead of wrapping.
5. **Version removed** from content.
6. **More whitespace** — let the interface breathe.

**Alternative:** Keep the ASCII art logo but reduce it to 2-3 rows instead of 6. The brand identity matters, but not at the cost of 25% of the screen.

---

## Wireframe: Discuss Screen

### Current
```
✓ ● ○  Question 2 of 3

────────────────────────────────────
┌──────────────────────────────────┐
│ What database should be used     │
│ for the user session store?      │
└──────────────────────────────────┘

  ┌────────────────────────────────────┐
  │ Type your answer...                │
  └────────────────────────────────────┘

  Enter: submit  Esc: skip  Ctrl+Shift+S: skip all
```

### Proposed
```
  2 of 3

  What database should be used for
  the user session store?

  ┌────────────────────────────────────┐
  │ Type your answer...                │
  └────────────────────────────────────┘

```

**Changes:**
1. **Card border removed.** The question stands on its own with typography.
2. **Section divider removed.**
3. **Progress simplified** to "2 of 3" — no dot indicators needed for 3 questions.
4. **Key hints removed** (handled by footer).
5. **More whitespace** — the question has room to breathe.

---

## Wireframe: Ship Screen

### Current
```
┌─ Session Summary ──────────────────────────┐
│                                            │
│  Tasks: 5/5 ✓    Tokens: 12.3K            │
│  Files: +3 ~5 -1   Cost: $0.05            │
│                  Duration: 2m 30s          │
│                  Commits: 2                │
│                                            │
│  a1b2c3d  feat: add auth module           │
│  e4f5g6h  fix: resolve import cycle       │
│                                            │
└────────────────────────────────────────────┘
```

### Proposed
```
  All tasks complete

  5/5 tasks  ·  2m 30s  ·  $0.05

  +3 ~5 -1  ·  12.3K tokens

  Commits
  a1b2c3d  feat: add auth module
  e4f5g6h  fix: resolve import cycle

  [d] walkthrough    [Enter] continue
```

**Changes:**
1. **Card border removed.** Clean typography instead.
2. **Title "Session Summary" removed** — the header says "Ship", that's enough.
3. **Stats on fewer lines** — consolidate into 2 lines instead of a grid.
4. **Commit log** is the secondary focus, not hidden in a grid.
5. **Actions** are clear and minimal.

---

## Wireframe: Sidebar (When Active)

### Current (11 sections)
```
M31A v1.5.0
───────────────
⎇ main
●2 +3 -1 ?2
───── USAGE ─────
[████░░░░] 42%
12.3K ctx
150 tok/s · $0.003/s
$0.05 ↑
gpt-4o
───── PHASE ─────
✓I ✓D ●P ○E ○V ○S
42s in plan
───── TOOLS ─────
● Reading file   …
✓ Editing file   120ms
✓ Running cmd    3200ms
───── SPEED ─────
2.1 tasks/min
avg 15s/task
ETA 45s
⬡ 2/4 agents
⏱ 1 pending approval(s)
───── FILES ─────
  M
  ├─ internal/
  │  ├─ auth.go
  │  └─ provider.go
  └─ main.go

abc123
ctrl+b:sidebar
```

### Proposed (5 sections, context-aware)
```
M31A

⎇ main  ●2 +3

Reading project structure
  auth.go
  provider.go

42% context
$0.05
```

**Changes:**
1. **Header:** Brand name only. Version removed (available in /version).
2. **Git:** Branch + file counts on one line. No pills, no colors.
3. **Activity:** Replaces TOOLS + SPEED + PHASE. Shows current intent + files involved.
4. **Status:** Context usage + cost on one line. No gauge, no burn rate.
5. **File tree:** Removed from sidebar. Accessible via `ctrl+f` or when focused.
6. **Session ID:** Removed. Accessible via `/session`.
7. **Keyboard hints:** Removed. Footer handles this.
8. **Sub-agent badge:** Shown inline with activity when relevant.
9. **Pending permissions:** Shown as a modal overlay, not a sidebar badge.

**Sidebar is context-aware:**
- During idle: shows branch + file counts + tips
- During workflow: shows phase + activity + progress
- During tool execution: shows current tool intent + files
- Focused mode: shows all sections (for power users)

---

# Part 4: Unnecessary UI Elements

## Elements to Remove

| Element | Location | Why Remove |
|---------|----------|------------|
| Leader dots (`····`) | Header | Pure decoration. Wastes horizontal space. |
| Context meter in header | Header right | Duplicated in sidebar and footer. Keep ONE. |
| Provider badge `[OR]` | Header right | Redundant with model name. Model implies provider. |
| `│` separators in header | Header | Visual noise. Zones are distinguishable by position. |
| `│` separators in footer | Footer | Same as header. |
| Version in sidebar header | Sidebar | Available via `/version`. Clutters persistent view. |
| Session ID in sidebar | Sidebar | Rarely needed. Accessible via `/session`. |
| Sidebar keyboard hints | Sidebar | Footer already shows hints. |
| Section dividers in sidebar | Sidebar | Replace with spacing. Dividers add lines without information. |
| Burn rate in sidebar | Sidebar | Debug metric. Hidden by default. |
| Speed metrics in sidebar | Sidebar | Debug metric. Hidden by default. |
| Tool timeline in sidebar | Sidebar | Replaced by intent narrative in main content. |
| Phase elapsed time in sidebar | Sidebar | Available on hover/focus. |
| File status pills (colored) | Sidebar | Replace with simple counts: `+3 -1 ?2` |
| Context gauge bar (8 segments) | Sidebar | Replace with text: `42% context` |
| Cost trend arrows (↑↓) | Sidebar | Static display is enough. |
| Card borders on content screens | Plan, Discuss, Ship | Whitespace provides sufficient separation. |
| Status badges (✓ done, ● running) | Execute task list | ✓ / ● / ✗ icons are sufficient without text labels. |
| Tool duration in inline view | REPL | Debug metric. Available in tool detail. |
| Tool status icons in inline view | REPL | Grouped intent replaces individual status. |
| Glow effect on home logo | Home | Pure decoration. |
| Glow line below logo | Home | Pure decoration. |
| Version in home content | Home | Available in header/sidebar. |
| Explicit key hints in screens | Discuss, Plan, Execute | Footer handles this. |
| Countdown timer (always visible) | Permission | Only show when <5s. |

## Elements to Simplify

| Element | Current | Proposed |
|---------|---------|----------|
| Header | 5 components (brand, sep, crumb, dots, sep, model, sep, ctx, badge) | 3 components (brand, crumb, model) |
| Footer | 5 components (cwd, sep, branch, sep, spinner+op, sep, hints, cost) | 3 components (cwd+branch, operation, hints) |
| Sidebar | 11 sections | 4-5 sections, context-aware |
| Tool rendering | Individual cards with icons, names, durations | Grouped by intent with file lists |
| Thinking display | Timer ("thinking · 3.2s") | Intent ("Analyzing codebase") |
| Permission modal | 5 fields + 3 buttons | Description + consequence + command + 3 buttons |
| Execute progress | Stats line + bar + task list with badges | Activity label + bar + task list with icons |
| Home screen | Logo (6 rows) + input + tips + version | Logo (2-3 rows) + tagline + input |

---

# Part 5: New Information Architecture

## Global Principles

1. **One source of truth per data point.** Context usage appears in ONE place (footer). Cost appears in ONE place (sidebar when active). Branch appears in ONE place (footer left).

2. **Progressive disclosure.** Default view shows essentials. Focus mode (`ctrl+b`) shows details. Hover/focus on specific elements reveals more.

3. **Intent over implementation.** Every display answers "what is happening?" not "which API was called?"

4. **Quiet by default.** Healthy systems are silent. Only anomalies and actions are visually prominent.

## Proposed Sidebar Modes

### Idle Mode (Default)
```
M31A

⎇ main  +3 -1
```

### Workflow Mode (Active)
```
M31A

🏗 Planning
  auth.go
  provider.go

42% context
```

### Focused Mode (ctrl+b twice)
```
M31A v1.6.0

⎇ main  +3 -1 ?2

USAGE
42% context  12.3K tokens
$0.05

PHASE
✓I ✓D ●P ○E ○V ○S

TOOLS
● Reading file   auth.go
✓ Editing file   120ms

PROGRESS
████████░░░░  3/5

TODO
1. ✓ Add auth middleware
2. ● Write tests
3. ○ Update docs

abc123
```

## Proposed Tool Rendering Pipeline

```
LLM emits tool_call
  → Dispatcher executes
  → Result returned
  → Intent classifier determines group:
      FileRead + Glob + Grep → "Reading project structure"
      FileRead alone → "Reading [filename]"
      Edit → "Editing [filename]"
      Bash → "Running: [command summary]"
      FileWrite → "Creating [filename]"
  → Group buffer collects consecutive same-intent tools
  → On group boundary (different intent, LLM text, or completion):
      → Flush group as single intent block
  → Render:
      Intent label (muted, italic)
      File list or result summary
```

## Proposed Thinking Display

Replace timer with intent classification:

| Current | Proposed |
|---------|----------|
| "thinking · 3.2s" | "Analyzing codebase" |
| "thinking · 8.1s" | "Planning implementation" |
| "thinking · 15.3s" | "Generating code" |
| "streaming…" | "Writing response" |

Intent is derived from:
- System prompt phase context
- Recent tool calls
- LLM streaming metadata (if available)
- Time-based heuristics (first 5s = planning, 5-15s = generating, 15s+ = complex reasoning)

---

# Part 6: Implementation Roadmap

## Phase 1: Foundation (Highest Impact, Lowest Risk)

**Goal:** Clean up chrome and remove visual noise.

### 1.1 Simplify Header
- Remove leader dots (`····`)
- Remove `│` separators
- Remove provider badge
- Remove context meter from header
- Keep: brand (left), breadcrumb (center), model name (right, muted)
- **Files:** `layout/page.go` (BuildHeader), `app_view.go` (buildHeaderInfo)
- **Effort:** Small

### 1.2 Simplify Footer
- Remove `│` separators
- Consolidate cwd + branch into left zone
- Remove cost from footer (keep in sidebar only)
- Remove context ring from footer
- Keep: cwd+branch (left), operation (center), hints (right)
- **Files:** `layout/page.go` (BuildFooter), `repl_footer.go` (RenderStatusBar)
- **Effort:** Small

### 1.3 Remove Home Decorations
- Remove glow effect and glow line
- Remove version from content
- Add one-line tagline
- Reduce logo to brand name (or 2-3 row ASCII art)
- **Files:** `home_view.go`, `components/logo.go`
- **Effort:** Small

### 1.4 Remove Card Borders from Content Screens
- Plan screen: remove ThinBorder card
- Discuss screen: remove ThinBorder card
- Ship screen: remove Card wrapper
- Use whitespace and typography for separation
- **Files:** `plan_view.go`, `discuss_model.go`, `ship_model.go`
- **Effort:** Small

**Phase 1 delivers:** Immediate visual calm. Every screen feels lighter. No functional changes.

---

## Phase 2: Sidebar Redesign (High Impact, Medium Risk)

**Goal:** Transform sidebar from data firehose to quiet orientation.

### 2.1 Create Sidebar Modes
- Idle: branch + file counts only
- Workflow: phase + activity + progress
- Focused: full detail (current behavior, opt-in)
- **Files:** `sidebar_model.go` (View, renderHeader, renderGitStatus, etc.)
- **Effort:** Medium

### 2.2 Remove Redundant Sidebar Sections
- Remove version from sidebar header
- Remove session ID
- Remove keyboard hints from sidebar
- Remove burn rate from default view
- Remove speed metrics from default view
- Remove tool timeline from default view
- **Files:** `sidebar_model.go`
- **Effort:** Small

### 2.3 Replace Section Dividers with Spacing
- Remove all `SectionDivider` renders
- Use empty lines for separation
- **Files:** `sidebar_model.go`, `components/section_divider.go`
- **Effort:** Small

### 2.4 Simplify Git Status Display
- Replace colored pills with simple counts: `+3 -1 ?2`
- **Files:** `sidebar_model.go` (renderGitStatus)
- **Effort:** Small

### 2.5 Simplify Context Usage Display
- Replace 8-segment colored gauge with text: `42% context`
- **Files:** `sidebar_model.go` (renderTokenUsage)
- **Effort:** Small

**Phase 2 delivers:** Sidebar becomes quiet and contextual. Information density drops 60%. Cognitive load drops significantly.

---

## Phase 3: Tool Rendering (High Impact, High Risk)

**Goal:** Replace debug-log tool display with intent narrative.

### 3.1 Create Tool Group Component
- New file: `components/toolgroup.go`
- Groups consecutive same-intent tools
- Renders intent label + file list
- **Files:** `components/toolgroup.go` (new)
- **Effort:** Large

### 3.2 Implement Intent Classifier
- Map tool calls to human-readable intents
- FileRead + Glob + Grep → "Reading project structure"
- Edit → "Editing [filename]"
- Bash → "Running: [command]"
- **Files:** `components/toolgroup.go`
- **Effort:** Medium

### 3.3 Implement Group Buffer
- Buffer consecutive same-intent tools
- Flush on intent boundary, LLM text, or completion
- Handle edge cases (single tool, mixed intents)
- **Files:** `components/message.go`, `repl_model.go`
- **Effort:** Large

### 3.4 Update Message Renderer
- Replace individual ToolCard rendering with ToolGroup rendering
- Keep ToolCard for standalone tools (Bash, Edit, failures)
- **Files:** `components/message.go`
- **Effort:** Medium

### 3.5 Remove Inline Tool Decorations
- Remove tool duration from inline display
- Remove tool status icons from grouped tools
- Keep status icons only for standalone tools
- **Files:** `components/toolcard.go`, `components/toolgroup.go`
- **Effort:** Small

**Phase 3 delivers:** Tool execution reads like a story, not a debug log. Users understand intent, not implementation.

---

## Phase 4: Permission Modal Redesign (Medium Impact, Medium Risk)

**Goal:** Make permission decisions intuitive.

### 4.1 Add Plain English Description Generator
- Map tool + command to human-readable description
- "Bash: rm -rf ./dist" → "M31A wants to clean the project output directory"
- **Files:** `tools/permission.go` or new `components/permission.go`
- **Effort:** Medium

### 4.2 Add Consequence Explanation
- Derive consequence from command + risk level
- "This will delete all files in ./dist/"
- **Files:** `components/permission.go`
- **Effort:** Medium

### 4.3 Redesign Modal Layout
- Primary: plain English description
- Secondary: consequence
- Tertiary: command in code block
- Risk: color accent on border (not text label)
- Countdown: only when <5s
- **Files:** `app_view.go` (RenderPermissionModal, renderPermissionModalContent)
- **Effort:** Medium

**Phase 4 delivers:** Permission decisions are informed, not just cautious.

---

## Phase 5: Execute Screen Redesign (Medium Impact, Low Risk)

**Goal:** Progress communicates intent, not stats.

### 5.1 Add Activity Label
- Derive current activity from phase + task description
- "Setting up project structure" instead of "3/5 tasks 60%"
- **Files:** `execute_model.go` (View)
- **Effort:** Small

### 5.2 Simplify Task List
- Remove text badges (✓ done, ● running)
- Keep icons only (✓ ● ✗ ○)
- Collapse completed tasks
- Expand failed tasks with error
- **Files:** `execute_model.go` (renderTasks)
- **Effort:** Small

### 5.3 Move Key Hints to Footer
- Remove inline hints from execute screen
- **Files:** `execute_model.go` (View)
- **Effort:** Small

**Phase 5 delivers:** Execute screen feels like a progress indicator, not a dashboard.

---

## Phase 6: Thinking Display (Low Impact, Low Risk)

**Goal:** Thinking communicates intent.

### 6.1 Implement Intent-Based Labels
- Use phase context + time to derive label
- Phase "plan" + time <5s → "Planning implementation"
- Phase "execute" + time <5s → "Generating code"
- Fallback → "Processing"
- **Files:** `repl_footer.go` (RenderStatusBar), `app_view.go` (buildFooterInfo)
- **Effort:** Small

### 6.2 Replace Timer with Label
- Remove "thinking · 3.2s"
- Replace with "Planning implementation"
- Keep duration accessible via hover/focus
- **Files:** `repl_footer.go`
- **Effort:** Small

**Phase 6 delivers:** Thinking state is meaningful, not anxious.

---

## Phase 7: Discuss Screen Cleanup (Low Impact, Low Risk)

**Goal:** Clean up question display.

### 7.1 Remove Card Border
- Question stands on its own with typography
- **Files:** `discuss_model.go` (View)
- **Effort:** Small

### 7.2 Simplify Progress
- Replace dot indicators with "2 of 3"
- **Files:** `discuss_model.go` (View)
- **Effort:** Small

### 7.3 Remove Inline Hints
- Footer handles key hints
- **Files:** `discuss_model.go` (View)
- **Effort:** Small

**Phase 7 delivers:** Questions feel direct and focused.

---

## Implementation Order

```
Phase 1 (Foundation)          ← Start here. Immediate visual improvement.
  ↓
Phase 2 (Sidebar)             ← Highest cognitive load reduction.
  ↓
Phase 3 (Tool Rendering)      ← Most complex, highest payoff.
  ↓
Phase 4 (Permission Modal)    ← Improves trust and safety.
  ↓
Phase 5 (Execute Screen)      ← Polish.
  ↓
Phase 6 (Thinking Display)    ← Polish.
  ↓
Phase 7 (Discuss Cleanup)     ← Polish.
```

**Total estimated effort:** 3-4 weeks for a single developer.

**Testing approach:** Each phase should be tested with:
1. Visual regression (screenshot comparison)
2. Cognitive load testing (time-to-understand metrics)
3. Accessibility verification (contrast, terminal widths)
4. Regression testing (existing functionality preserved)

---

# Part 7: Design System Updates

## Typography Scale

| Role | Style | Use |
|------|-------|-----|
| Heading | Bold, Brand color | Screen titles, section labels |
| Body | Regular, Text color | Content, descriptions |
| Caption | Regular, Muted color | Metadata, timestamps, hints |
| Code | Monospace, Code background | Commands, file paths, errors |
| Intent | Italic, Muted color | Tool intent labels, thinking state |

## Spacing Rules

| Context | Current | Proposed |
|---------|---------|----------|
| Between messages | 0 (adjacent) | 1 blank line between LLM responses |
| After tool group | 0 | 1 blank line |
| Before input | 1 (separator) | 0 (separator removed in REPL) |
| Sidebar sections | 0 (dividers) | 1 blank line |

## Color Usage

| Element | Current | Proposed |
|---------|---------|----------|
| Brand | Headers, buttons, active states | Headers, active states only |
| Success | ✓ icons, green pills | ✓ icons only |
| Warning | Cost, burn rate, timers | Cost only |
| Error | ✗ icons, red pills, failures | ✗ icons, failures only |
| Muted | Everything次要 | Metadata, hints, inactive states |

---

# Part 8: Accessibility Considerations

## Contrast
- All text must meet WCAG AA (4.5:1 ratio)
- Muted text must still be readable (3:1 minimum)
- Error/warning colors must be distinguishable from each other

## Terminal Widths
- 40-59 cols: Sidebar hidden, header simplified, footer minimal
- 60-79 cols: Full header/footer, no sidebar
- 80+ cols: Full experience with optional sidebar
- Below 40: "Resize terminal" message (existing behavior)

## Keyboard Navigation
- All actions must be keyboard-accessible
- Focus indicators must be visible
- Tab order must be logical
- No keyboard shortcuts should conflict with terminal defaults

## Screen Reader Compatibility
- Alt text for decorative elements (logo)
- ARIA labels for interactive elements
- Semantic structure (headings, lists)
- Status announcements for state changes

---

# Appendix: Code Change Summary

## Files to Modify

| File | Phase | Changes |
|------|-------|---------|
| `layout/page.go` | 1 | Simplify BuildHeader, BuildFooter |
| `app_view.go` | 1, 4 | Simplify buildHeaderInfo, buildFooterInfo, RenderPermissionModal |
| `home_view.go` | 1 | Remove decorations, add tagline |
| `sidebar_model.go` | 2 | Add modes, remove sections, simplify displays |
| `components/toolgroup.go` | 3 | New file — tool grouping component |
| `components/message.go` | 3 | Use ToolGroup instead of individual ToolCards |
| `components/toolcard.go` | 3 | Remove inline decorations |
| `repl_footer.go` | 6 | Intent-based thinking display |
| `execute_model.go` | 5 | Activity labels, simplified task list |
| `discuss_model.go` | 7 | Remove card, simplify progress |
| `plan_view.go` | 1 | Remove card border |
| `ship_model.go` | 1 | Remove card wrapper |
| `repl_view.go` | 3 | Tool group integration |
| `components/permission.go` | 4 | New — plain English descriptions |

## Files to Create

| File | Phase | Purpose |
|------|-------|---------|
| `components/toolgroup.go` | 3 | Tool grouping and intent rendering |
| `components/permission.go` | 4 | Permission description generation |

## Testing Updates

| Test | Phase | Changes |
|------|-------|---------|
| `layout/page_test.go` | 1 | Update header/footer assertions |
| `sidebar_model_test.go` | 2 | Test mode switching, section visibility |
| `components/toolgroup_test.go` | 3 | Test grouping logic, intent classification |
| `components/message_test.go` | 3 | Test tool group rendering |
| `app_view_test.go` | 1, 4 | Test simplified chrome, permission modal |
