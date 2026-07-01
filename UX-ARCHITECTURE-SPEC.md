# M31A v1.6.2 — UX Architecture Specification (Design Freeze)

**Document Status:** FINAL — Design Freeze
**Version:** 1.6.2
**Date:** 2026-07-01
**Classification:** Engineering Contract

---

# Table of Contents

1. [UX Architecture Specification](#1-ux-architecture-specification)
2. [Information Architecture Diagram](#2-information-architecture-diagram)
3. [Navigation Diagram](#3-navigation-diagram)
4. [Layout Grid Specification](#4-layout-grid-specification)
5. [Conversation Specification](#5-conversation-specification)
6. [Narrative Engine Specification](#6-narrative-engine-specification)
7. [Sidebar Specification](#7-sidebar-specification)
8. [Screen-by-Screen Wireframes](#8-screen-by-screen-wireframes)
9. [Interaction Guidelines](#9-interaction-guidelines)
10. [UX Design Rules](#10-ux-design-rules)
11. [Engineering Implementation Contract](#11-engineering-implementation-contract)

---

# 1. UX Architecture Specification

## 1.1 Product Identity

M31A is an autonomous software engineer that operates in the terminal. It is calm, confident, predictable, and trustworthy. It communicates through conversation and action — never through raw system events.

**Personality traits:**
- Calm — never frantic, never urgent unless genuinely urgent
- Confident — knows what it is doing and why
- Predictable — navigation, layout, and behavior never surprise
- Trustworthy — explains decisions, asks permission, shows consequences
- Focused — one task at a time, one purpose per screen
- Premium — every pixel earns its place

**Anti-traits (never exhibit these):**
- Debugger — no raw stack traces, no memory dumps, no hex dumps
- Log viewer — no scrolling walls of text, no timestamped entries
- CI console — no build matrix, no stage Pipeline, no test runner output
- Stream of internal events — no FileRead ×20, no Glob ×5, no raw tool names

## 1.2 Immutable Design Rules

These rules cannot be violated by any future implementation. They are the constitution of M31A's user experience.

### Rule 1: Conversation Is King
The user's dialogue with M31A is the primary content. Every other element exists to support it. No sidebar, header, footer, modal, or overlay may visually dominate the conversation area.

### Rule 2: Intent Before Implementation
Every display answers "what is happening?" before "how is it happening?" Tool names, API calls, internal file paths, and timing data are implementation details hidden by default.

### Rule 3: Progressive Disclosure
Every screen has exactly three visibility tiers:
- **Default:** What the user needs right now
- **Contextual:** What the user might need (sidebar, footer)
- **Inspectable:** What the user can demand (expanded tool output, metrics, debug)

No tier is permanently visible. The user controls depth.

### Rule 4: Calm Interface
Healthy systems are silent. No animations, no pulsing, no blinking, no color changes unless the state actually changes. Status is communicated through stillness, not motion.

### Rule 5: One Source of Truth
Every data point appears in exactly one location. Context usage appears in the footer. Cost appears in the sidebar. Git branch appears in the footer. No data is duplicated across components.

### Rule 6: Stable Layouts
The layout grid is immutable within a major version. Components do not resize unpredictably. The sidebar is exactly 28 columns. The header is exactly 1 row. The footer is exactly 1 row. These are contracts, not suggestions.

### Rule 7: Quiet Defaults
Every element starts invisible or minimal. The user opts into complexity. The sidebar is hidden until summoned. Tool details are collapsed until expanded. Metrics are suppressed until requested.

### Rule 8: Human-Readable Language
Every user-facing string uses plain English. "M31A wants to install dependencies" not "Tool: Bash npm install". "Reading source files" not "FileRead ×3". Technical terms appear only in expandable details.

### Rule 9: Minimal Cognitive Load
At any moment, the user should be processing no more than 3 pieces of information. If a screen presents 5+ simultaneous data points, it is violating this rule and must be redesigned.

### Rule 10: Predictable Navigation
Every screen has exactly one way to go back (Escape). Every screen has exactly one primary action. Navigation never requires memorization. The command palette (Ctrl+P) is always available as a safety net.

## 1.3 Design Philosophy

The interface follows a "conversation in a quiet room" metaphor. The room has furniture (sidebar, header, footer) but the conversation is the focus. The furniture is well-made, always in the same place, and never draws attention to itself.

When M31A is working, the user sees:
1. What M31A is trying to accomplish (narrative)
2. Where it is in the process (progress)
3. What it has done so far (history)
4. What it needs from the user (if anything)

The user never sees:
1. Which API endpoint was called
2. How many milliseconds a function took
3. Which internal file was read
4. What the raw tool output was (unless they ask)

---

# 2. Information Architecture Diagram

## 2.1 The Four UI Layers

```
┌─────────────────────────────────────────────────────────────┐
│                                                             │
│  LAYER 1: CONVERSATION                                      │
│  ─────────────────────                                      │
│  The user's dialogue with M31A.                             │
│  Always primary. Always visible.                            │
│ 佔据最多屏幕空间.                                            │
│                                                             │
│  Content:                                                   │
│  - User messages                                            │
│  - Assistant responses                                      │
│  - Streaming text                                           │
│                                                             │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  LAYER 2: NARRATIVE                                         │
│  ────────────────────                                       │
│  What M31A is trying to accomplish.                         │
│  Second most important.                                     │
│  Appears inline within the conversation.                    │
│                                                             │
│  Content:                                                   │
│  - Intent labels ("Reading project structure")              │
│  - Tool summaries (grouped by intent)                       │
│  - Progress indicators (activity, not stats)                │
│  - Error explanations                                       │
│                                                             │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  LAYER 3: CONTEXT                                           │
│  ──────────────────                                         │
│  Where the user is. What is happening.                      │
│  Quiet. Always available. Never distracting.                │
│                                                             │
│  Content:                                                   │
│  - Footer: cwd, branch, operation, hints                    │
│  - Sidebar (when active): phase, progress, git              │
│  - Header: brand, breadcrumb, model                         │
│                                                             │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  LAYER 4: SYSTEM                                            │
│  ────────────────                                           │
│  Raw tool events. Timing. Metrics. Debug.                   │
│  Hidden by default. Expandable when needed.                 │
│                                                             │
│  Content:                                                   │
│  - Tool execution details (name, input, output, duration)   │
│  - Token metrics (burn rate, cost per second)               │
│  - Timing data (elapsed, ETA, per-task duration)            │
│  - Internal workflow state                                  │
│  - Stack traces, error details                              │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

## 2.2 Layer Rendering Rules

| Layer | Default State | Triggered State | Access Method |
|-------|---------------|-----------------|---------------|
| Conversation | Always visible | Streaming, scrolling | Direct view |
| Narrative | Visible during activity | Collapses after completion | Inline in conversation |
| Context | Footer always visible; sidebar hidden | Sidebar summoned | ctrl+b for sidebar |
| System | Hidden | Expanded on demand | Click/Enter on collapsed element |

## 2.3 Data Flow

```
User Input
  → Layer 1 (Conversation): message appears
  → Layer 2 (Narrative): "Understanding your request"
  → Layer 3 (Context): footer shows "thinking..."
  → Layer 4 (System): [hidden]

LLM Response
  → Layer 1: response streams in
  → Layer 2: narrative updates ("Planning implementation")
  → Layer 3: footer shows "streaming..."
  → Layer 4: [hidden]

Tool Execution
  → Layer 1: no direct display
  → Layer 2: "Reading source files" + file list
  → Layer 3: sidebar shows current activity
  → Layer 4: tool details available on expand

Error
  → Layer 1: error message in conversation
  → Layer 2: "Failed to build project" + explanation
  → Layer 3: footer shows error state
  → Layer 4: stack trace available on expand

Completion
  → Layer 1: summary message
  → Layer 2: "All tasks complete" + stats
  → Layer 3: footer returns to idle
  → Layer 4: [hidden]
```

## 2.4 Information Priority Matrix

| Information | Layer | Visibility | Justification |
|-------------|-------|------------|---------------|
| LLM response | 1 | Always | Primary content |
| User message | 1 | Always | Primary content |
| Tool intent | 2 | During execution | Explains what's happening |
| Tool file list | 2 | During execution | Shows scope |
| Current phase | 3 | Footer always | Orientation |
| Git branch | 3 | Footer always | Orientation |
| cwd | 3 | Footer always | Orientation |
| Context usage | 3 | Footer, single display | Resource awareness |
| Model name | 3 | Header, single display | Provider awareness |
| Cost | 3 | Sidebar only | Financial awareness |
| Tool name | 4 | Expanded only | Debug |
| Tool input | 4 | Expanded only | Debug |
| Tool output | 4 | Expanded only | Debug |
| Tool duration | 4 | Expanded only | Debug |
| Burn rate | 4 | Expanded only | Debug |
| Speed metrics | 4 | Expanded only | Debug |
| Session ID | 4 | Hidden | Administrative |
| Token counts | 4 | Expanded only | Debug |

---

# 3. Navigation Diagram

## 3.1 Screen Transition Map

```
                        ┌──────────┐
                        │   HOME   │
                        └────┬─────┘
                             │ input
                             ▼
                        ┌──────────┐
                   ┌────│   REPL   │────┐
                   │    └────┬─────┘    │
                   │         │          │
              ctrl+b│    workflow       │ctrl+p
                   │         │          │
                   ▼         ▼          ▼
              ┌─────────┐ ┌──────┐ ┌──────────┐
              │ SIDEBAR │ │ GOAL │ │ COMMAND  │
              │ (overlay)│ │INPUT │ │ PALETTE  │
              └─────────┘ └──┬───┘ └──────────┘
                             │
                             ▼
                        ┌──────────┐
                        │ DISCUSS  │
                        └────┬─────┘
                             │
                             ▼
                        ┌──────────┐
                        │   PLAN   │
                        └────┬─────┘
                             │
                             ▼
                        ┌──────────┐
                        │ EXECUTE  │
                        └────┬─────┘
                             │
                             ▼
                        ┌──────────┐
                        │  VERIFY  │
                        └────┬─────┘
                             │
                             ▼
                        ┌──────────┐
                        │ RUNTIME  │
                        └────┬─────┘
                             │
                             ▼
                        ┌──────────┐
                        │   SHIP   │
                        └────┬─────┘
                             │
                             ▼
                        ┌──────────┐
                        │   HOME   │
                        └──────────┘
```

## 3.2 Navigation Rules

### Back Behavior (Escape)
- **Every screen** returns to the previous screen via Escape
- **REPL → Escape:** No-op (REPL is the home base)
- **Modal → Escape:** Closes modal, returns to underlying screen
- **Workflow screen → Escape:** Pauses workflow, returns to REPL
- **Sidebar overlay → Escape:** Closes sidebar

### Forward Behavior (Enter)
- **REPL → Enter:** Sends message (when input is non-empty)
- **Home → Enter:** Sends goal (when input is non-empty)
- **GoalInput → Enter:** Submits goal, transitions to Discuss or Plan
- **Discuss → Enter:** Submits answer, advances to next question
- **Plan → Enter:** Opens plan review mode
- **Execute → Enter:** No-op (read-only screen)
- **Verify → Enter:** Proceeds to Runtime or Ship
- **Ship → Enter:** Returns to Home

### Primary Action per Screen

| Screen | Primary Action | Trigger |
|--------|---------------|---------|
| Home | Input goal | Enter |
| REPL | Input message | Enter |
| GoalInput | Submit goal | Enter |
| Discuss | Answer question | Enter |
| Plan | Review/approve plan | y/r/Enter |
| Execute | Observe progress | (passive) |
| Verify | Review results | y/n |
| Runtime | Continue | Enter |
| Ship | Return home | Enter |
| Permission | Approve/deny | y/n/a |
| Command Palette | Select command | Enter |
| Model Selector | Select model | Enter |
| Settings | Edit setting | e |
| Sidebar | Toggle focus | ctrl+b |

### Modal Hierarchy

```
Command Palette (highest priority, full screen overlay)
  ↓ dismisses
Permission Modal (centered, dimmed background)
  ↓ dismisses
Question Modal (centered, dimmed background)
  ↓ dismisses
Model Selector (centered, dimmed background)
  ↓ dismisses
Sidebar (right-side overlay on narrow screens)
  ↓ dismisses
Screen Content (default state)
```

### Focus Management

1. **Screen transition:** Focus moves to the new screen's primary interactive element
2. **Modal open:** Focus moves to the modal. Underlying screen is dimmed but visible.
3. **Modal close:** Focus returns to the screen that was active before the modal.
4. **Sidebar toggle:** Focus stays in the main content area. Sidebar is informational, not interactive (except in focused mode).
5. **Command palette:** Focus moves to the search input. Palette is dismissed on selection or Escape.

## 3.3 Keyboard Shortcuts

### Global (All Screens)

| Shortcut | Action | Notes |
|----------|--------|-------|
| Ctrl+P | Command palette | Always available |
| Ctrl+B | Toggle sidebar | Context-aware visibility |
| Ctrl+C | Cancel current operation | Double-tap to force quit |
| Escape | Go back / dismiss | Universal back action |
| Ctrl+L | Clear screen | REPL only |

### REPL

| Shortcut | Action |
|----------|--------|
| Enter | Send message |
| Ctrl+Enter | New line in input |
| Up/Down | History navigation |
| Tab | Autocomplete (slash commands, @mentions) |
| Ctrl+F | Search in conversation |
| Ctrl+G | Focus sidebar |

### Workflow Screens

| Shortcut | Action |
|----------|--------|
| y | Approve / Yes |
| n | Deny / No |
| a | Allow all (permission) |
| r | Refine plan |
| p | Pause execution |
| j/k | Scroll viewport |
| d | View details / diff |

---

# 4. Layout Grid Specification

## 4.1 Terminal Breakpoints

| Breakpoint | Width | Height | Behavior |
|------------|-------|--------|----------|
| **UltraNarrow** | <40 cols | any | Show "Resize terminal" message only |
| **Compact** | 40-59 cols | any | Minimal chrome, no sidebar, no hints |
| **Standard** | 60-79 cols | any | Full chrome, no sidebar |
| **Full** | ≥80 cols | any | Full chrome, sidebar available |
| **UltraWide** | ≥120 cols | any | Wider conversation area, sidebar unchanged |

## 4.2 Layout Grid

### Full Width (≥80 cols)

```
├─ 28 ─┤├────────────────── 52+ ──────────────────┤
┌──────────────────────────────────────────────────┐
│ M31A  chat                              gpt-4o   │ 1 row (header)
├──────────────────────────────────────────────────┤
│                                                  │
│ [sidebar]  [        conversation area        ]   │ contentHeight - 2
│ 28 cols    [                                 ]   │
│            [                                 ]   │
│            [                                 ]   │
│            [                                 ]   │
│                                                  │
├──────────────────────────────────────────────────┤
│ ⌂ project  main              ctrl+p commands    │ 1 row (footer)
└──────────────────────────────────────────────────┘
```

### Standard Width (60-79 cols)

```
├──────────────────── 60-79 ──────────────────────┤
┌──────────────────────────────────────────────────┐
│ M31A  chat                              gpt-4o   │ 1 row
├──────────────────────────────────────────────────┤
│                                                  │
│ [           conversation area                ]   │ contentHeight - 2
│ [                                         ]   │
│ [                                         ]   │
│ [                                         ]   │
│                                                  │
├──────────────────────────────────────────────────┤
│ ⌂ project  main                                 │ 1 row
└──────────────────────────────────────────────────┘
```

### Compact Width (40-59 cols)

```
├────────────── 40-59 ────────────────┤
┌──────────────────────────────────────┐
│ M31A  chat                          │ 1 row
├──────────────────────────────────────┤
│                                      │
│ [     conversation area          ]   │ contentHeight - 2
│ [                             ]   │
│ [                             ]   │
│                                      │
├──────────────────────────────────────┤
│ ⌂ project                           │ 1 row
└──────────────────────────────────────┘
```

## 4.3 Fixed Dimensions

| Element | Dimension | Value | Notes |
|---------|-----------|-------|-------|
| Header | Height | 1 row | Immutable |
| Footer | Height | 1 row | Immutable |
| Sidebar | Width | 28 cols | When visible |
| Sidebar | Min width | 20 cols | Compact mode |
| Sidebar | Max width | 32 cols | Wide mode |
| ChromeHeight | Total | 2 rows | Header + Footer |
| ContentHeight | Formula | terminal_height - 2 | Always |
| Modal | Width | 60% of terminal | Min 40, Max 80 |
| Modal | Height | 60% of terminal | Min 10, Max 40 |
| Input area | Height | 3 rows | Textarea + separator |
| Conversation | Min width | 20 cols | Below this, resize prompt |

## 4.4 Sidebar Overlay (Narrow Screens)

When the sidebar is active on screens <80 cols, it renders as a right-side overlay:

```
┌──────────────────────────────────────┐
│ M31A  chat                          │
├──────────────────────────────────┬───┤
│                                  │ S │
│ [     conversation area     ]    │ I │
│ [                        ]    │ D │
│ [                        ]    │ E │
│                                  │ B │
├──────────────────────────────────│ A │
│ ⌂ project                       │ R │
└──────────────────────────────────────┘
```

Overlay width: min(sidebar_width, terminal_width - 10)
The underlying content is visible but partially obscured.

## 4.5 Responsive Behavior Rules

1. **Width decreases:** Drop rightmost elements first (hints → cost → model → breadcrumb → brand)
2. **Width increases:** Add elements in reverse order
3. **Height decreases:** Content area shrinks. Header/footer never shrink. Minimum content height: 4 rows.
4. **Height increases:** Content area grows. No element expands to fill extra space unless it is a viewport.

---

# 5. Conversation Specification

## 5.1 Message Types

| Type | Author | Visual Treatment | Purpose |
|------|--------|-----------------|---------|
| User message | User | Indented, plain text | What the user said |
| Assistant message | M31A | Left-aligned, markdown rendered | M31A's response |
| Narrative | System | Italic, muted, no gutter | What M31A is doing |
| Tool summary | System | Indented file list | Grouped tool results |
| Error | System | Red text, expandable | What went wrong |
| Warning | System | Yellow text | Caution information |
| Success | System | Green text, single line | Completion confirmation |
| System note | System | Muted, small | Administrative information |

## 5.2 Message Rhythm

Messages follow a consistent rhythm:

```
[user message]

[assistant response — may span multiple paragraphs]

[narrative: intent label]

  [tool summary — file list]

[assistant continues or concludes]

[if more work needed:]

[narrative: next intent label]

  [tool summary]

[assistant continues]
```

**Rules:**
- One blank line between user message and assistant response
- One blank line between assistant response and narrative
- No blank line between narrative label and tool summary
- One blank line after tool summary before next assistant text
- Maximum 3 narrative blocks before an assistant message must appear (prevents wall-of-narrative)

## 5.3 Assistant Messages

**Rendering:**
- Markdown rendered via Glamour
- Code blocks syntax-highlighted
- Headings in brand color
- Links in accent color
- Code inline with background

**Streaming:**
- Text appears character-by-character
- Cursor blinks at the leading edge
- No flash or flicker on new tokens
- Paragraph breaks appear naturally

**Length:**
- Short responses (<200 words): render inline
- Medium responses (200-1000 words): render inline with scroll
- Long responses (>1000 words): render in viewport with scroll

## 5.4 User Messages

**Rendering:**
- Plain text (no markdown)
- No gutter decoration
- Slightly indented or with subtle left border to distinguish from assistant

**Input area:**
- Textarea at bottom of conversation
- Auto-grows with content (1-5 rows)
- Placeholder: "Describe your task..." (Home), "Type a message..." (REPL)
- History navigation via Up/Down

## 5.5 Narrative Messages

**Rendering:**
- Italic text
- Muted color (not brand, not white)
- No gutter border
- Preceded by one blank line
- Followed by tool summary (if any) or next content

**Examples:**
```
Reading project structure

  auth.go
  provider.go
  main.go

Planning implementation

Applying edits

  auth.go
  middleware.go

Running verification

  $ go build ./...
  $ go test ./...
```

**Rules:**
- Never show raw tool names (FileRead, Glob, Grep)
- Always show human-readable intent
- File lists use bullet indentation (2 spaces)
- Commands in tool summaries show the actual command (user-facing)
- Maximum 5 files listed; if more, show first 3 + "and N more"

## 5.6 Tool Summaries

Tool summaries group consecutive same-intent tool calls into a single display.

**Grouping rules:**

| Tool Pattern | Intent Label | Display |
|-------------|--------------|---------|
| FileRead + Glob + Grep (any combo) | "Reading [context]" | File list |
| FileRead alone | "Reading [filename]" | Single file |
| Edit (single) | "Editing [filename]" | Filename |
| Edit (multiple, same file) | "Editing [filename]" | Filename |
| Edit (multiple, different files) | "Editing files" | File list |
| FileWrite | "Creating [filename]" | Filename |
| Bash (build command) | "Building project" | Command |
| Bash (test command) | "Running tests" | Command |
| Bash (install command) | "Installing dependencies" | Command |
| Bash (other) | "Running: [summary]" | Command summary |
| TodoWrite | "Updating task list" | (no file list) |
| Any tool (failure) | "Failed to [intent]" | Error + context |

**Rendering:**
```
[Intent label — italic, muted]

  [file or command 1]
  [file or command 2]
  [file or command 3]
```

**Failure rendering:**
```
Failed to edit auth.go

  File modified since last read. Retrying with fresh content.
```

## 5.7 Thinking State

**Current (bad):** `thinking · 3.2s`
**Proposed:** Intent-based labels

| Phase Context | Time | Label |
|---------------|------|-------|
| plan | <5s | Planning implementation |
| plan | 5-15s | Analyzing requirements |
| plan | >15s | Deep analysis in progress |
| execute | <5s | Generating code |
| execute | 5-15s | Writing implementation |
| execute | >15s | Complex reasoning in progress |
| verify | <5s | Running verification |
| verify | >5s | Analyzing results |
| (unknown) | <5s | Processing |
| (unknown) | >5s | Thinking |

**Display location:** Footer center (replaces spinner + "thinking...")

**Duration access:** Available via hover/focus on the thinking label, or via sidebar in focused mode.

## 5.8 Streaming State

**Display:** Footer center shows `streaming...` with spinner.

**Content:** Response text appears in the conversation area as it arrives.

**Completion:** Spinner stops. Narrative may appear ("Verifying build...") if M31A continues working.

## 5.9 Long-Running Operations

When a tool or operation takes >10 seconds:

1. Narrative label appears: "Running tests..."
2. A subtle elapsed indicator appears in the footer: `running · 12s`
3. If the operation produces output, show last 3 lines in a collapsed block
4. On completion, narrative updates: "Tests passed" or "Tests failed — see details"

---

# 6. Narrative Engine Specification

## 6.1 Purpose

The narrative engine translates raw tool events into human-readable progress descriptions. It is the bridge between Layer 2 (Narrative) and Layer 4 (System).

## 6.2 Intent Classification

Every tool call is classified into an intent category:

```
Tool Call → Intent Classifier → Intent Label + Display Rules
```

### Classification Table

| Tool | Input Pattern | Intent Label | Display |
|------|--------------|--------------|---------|
| FileRead | any | "Reading [basename]" | Filename |
| FileRead | (3+ consecutive) | "Reading source files" | File list (max 5) |
| Glob | pattern | "Finding files matching [pattern]" | Result count |
| Glob | (with FileRead) | "Reading project structure" | File list |
| Grep | pattern | "Searching for [pattern]" | Result count |
| Grep | (with FileRead/Glob) | "Reading project structure" | File list |
| Edit | filename | "Editing [basename]" | Filename |
| Edit | (3+ same file) | "Refining [basename]" | Filename |
| Edit | (3+ different) | "Editing files" | File list |
| FileWrite | filename | "Creating [basename]" | Filename |
| Bash | go build | "Building project" | Command |
| Bash | go test | "Running tests" | Command |
| Bash | npm install / yarn | "Installing dependencies" | Command |
| Bash | npm run dev | "Starting dev server" | Command |
| Bash | git * | "Updating repository" | Command summary |
| Bash | other | "Running: [first 40 chars]" | Command |
| TodoWrite | any | "Updating task list" | (no display) |
| AskUserQuestion | any | "Asking for clarification" | Question text |
| WebFetch | any | "Fetching [domain]" | URL |
| WebSearch | any | "Searching: [query]" | Query |

### Grouping Algorithm

```
buffer = []
current_intent = null

for each tool_call in sequence:
    intent = classify(tool_call)
    
    if intent == current_intent:
        buffer.append(tool_call)
    else:
        if buffer is not empty:
            flush(buffer, current_intent)
        buffer = [tool_call]
        current_intent = intent

    if tool_call.failed:
        flush(buffer, current_intent)  // flush before error
        render_failure(tool_call)
        buffer = []
        current_intent = null

// Flush remaining buffer at end of tool sequence
if buffer is not empty:
    flush(buffer, current_intent)
```

### Flush Rules

A buffer is flushed when:
1. The intent changes (FileRead → Edit)
2. A tool fails (render failure immediately)
3. The LLM produces text after tools (render narrative, then text)
4. 5 tools accumulate in the buffer (prevent oversized groups)
5. The tool sequence ends (LLM stops calling tools)

## 6.3 Narrative Templates

### Success Templates

```
Reading {context}
  {file_list}

Editing {filename}

Creating {filename}

Building project

Running tests

Installing dependencies

Running: {command_summary}

Updating task list
```

### Failure Templates

```
Failed to {intent}
  {error_explanation}

  {recovery_action}
```

### Progress Templates

```
{current_activity}... ({elapsed})

  {partial_results}
```

## 6.4 Narrative Timing

| Event | Narrative Appears | Narrative Disappears |
|-------|-------------------|---------------------|
| Tool starts | Immediately | When tool completes |
| Tool completes (success) | Updated to show result | When next narrative starts |
| Tool fails | Error narrative | When user dismisses or next action |
| LLM thinking | "Planning..." | When LLM starts streaming |
| LLM streaming | "Writing response..." | When streaming completes |
| Build running | "Building project..." | When build completes |
| Tests running | "Running tests..." | When tests complete |

---

# 7. Sidebar Specification

## 7.1 Purpose

The sidebar answers one question: "Where am I?" It provides orientation, not detail. It is a compass, not a map.

## 7.2 Visibility Rules

| Condition | Sidebar State |
|-----------|--------------|
| Terminal width <80 cols | Hidden (overlay when activated) |
| Terminal width ≥80 cols | Hidden by default |
| User presses ctrl+b | Toggled (visible/hidden) |
| Workflow active | Auto-shown (optional, config-controlled) |
| Permission modal active | Hidden (modal is full attention) |
| Command palette active | Hidden |
| First run screen | Always hidden |

## 7.3 Sidebar Modes

### Idle Mode (Default)

When no workflow is active:

```
M31A

⎇ main  +3 -1
```

**Content:**
- Brand name (1 line)
- Git branch + file change counts (1 line)

**Total height:** 2 lines + 1 blank = 3 lines

### Active Mode (Workflow Running)

When a workflow phase is active:

```
M31A

🏗 Planning

  auth.go
  provider.go

42% context
```

**Content:**
- Brand name (1 line)
- Current activity label (1 line, italic)
- File list if relevant (up to 5 lines)
- Context usage (1 line)

**Total height:** 3-7 lines depending on file count

### Focused Mode (ctrl+b twice)

When user explicitly requests full sidebar:

```
M31A v1.6.2

⎇ main  +3 -1 ?2

42% context  12.3K tokens

✓I ✓D ●P ○E ○V ○S

████████░░░░  3/5 complete

1. ✓ Install deps
2. ● Write tests
3. ○ Update docs

abc123
```

**Content:**
- Brand + version (1 line)
- Git branch + file counts (1 line)
- Context usage + tokens (1 line)
- Phase pipeline (1 line)
- Progress bar + count (1 line)
- Task list (up to visible height)
- Session ID (1 line)

**Total height:** 7+ lines

### Transition Between Modes

```
ctrl+b (first press)  → Idle mode becomes visible
ctrl+b (second press) → Focused mode
ctrl+b (third press)  → Hidden
```

Or: ctrl+b toggles visibility. When visible, ctrl+g switches between idle/active/focused.

## 7.4 Sidebar Content Rules

### Always Present (All Modes)
- Brand name

### Present When Relevant
- Git branch (when in a git repo)
- File change counts (when there are changes)
- Current activity (when workflow is active)
- File list (when tools are reading/writing files)
- Context usage (when tokens > 0)

### Present Only in Focused Mode
- Version number
- Phase pipeline
- Progress bar
- Task list
- Session ID

### Never Present in Sidebar
- Token burn rate (Layer 4)
- Speed metrics (Layer 4)
- Tool timeline (Layer 4, replaced by narrative)
- Cost trend arrows (Layer 4)
- Keyboard hints (footer handles this)
- Section dividers (use whitespace)
- Model name (header handles this)
- Provider badge (header handles this)

## 7.5 Sidebar Rendering Rules

1. **No section dividers.** Use blank lines for separation.
2. **No colored pills.** Git changes shown as `+3 -1 ?2` in muted text.
3. **No gauge bars.** Context shown as `42% context` in text.
4. **No borders on the sidebar itself.** Only a subtle left border when focused.
5. **Padding:** 1 space left padding on all content lines.
6. **Width:** Exactly 28 columns. Content truncated with ellipsis if longer.

---

# 8. Screen-by-Screen Wireframes

## 8.1 Home Screen

**Purpose:** Landing screen. User enters their goal.
**Primary goal:** Input a task description.
**Primary action:** Enter to submit goal.
**Secondary actions:** Ctrl+P for commands, /help for help.
**Navigation:** Forward to GoalInput → Discuss/Plan → workflow.
**Exit:** Escape → no-op (already home).
**Loading:** N/A.
**Error:** N/A.
**Empty state:** Show tagline + input.
**Success state:** Transitions to workflow.
**Failure state:** N/A.

```
Terminal ≥80 cols:

┌──────────────────────────────────────────────────────────────┐
│ M31A  Home                                              Home │
├──────────────────────────────────────────────────────────────┤
│                                                              │
│                                                              │
│                       M31A                                   │
│                                                              │
│           AI coding agent — describe your task               │
│                                                              │
│          ┌────────────────────────────────────┐              │
│          │ Build a REST API with auth...      │              │
│          └────────────────────────────────────┘              │
│                                                              │
│              ctrl+p commands        /help                    │
│                                                              │
│                                                              │
├──────────────────────────────────────────────────────────────┤
│ ⌂ project                                          ctrl+p   │
└──────────────────────────────────────────────────────────────┘


Terminal 60-79 cols:

┌──────────────────────────────────────────────────────────────┐
│ M31A  Home                                              Home │
├──────────────────────────────────────────────────────────────┤
│                                                              │
│                       M31A                                   │
│                                                              │
│           AI coding agent — describe your task               │
│                                                              │
│          ┌────────────────────────────────────┐              │
│          │ Build a REST API with auth...      │              │
│          └────────────────────────────────────┘              │
│                                                              │
│                                                              │
├──────────────────────────────────────────────────────────────┤
│ ⌂ project                                                   │
└──────────────────────────────────────────────────────────────┘


Terminal 40-59 cols:

┌──────────────────────────────────────────┐
│ M31A  Home                          Home │
├──────────────────────────────────────────┤
│                                          │
│               M31A                       │
│                                          │
│      AI coding agent                    │
│      describe your task                 │
│                                          │
│      ┌──────────────────────────┐       │
│      │ Build a REST API...      │       │
│      └──────────────────────────┘       │
│                                          │
│                                          │
├──────────────────────────────────────────┤
│ ⌂ project                               │
└──────────────────────────────────────────┘
```

---

## 8.2 REPL Screen (Primary Screen)

**Purpose:** Conversation with M31A.
**Primary goal:** Chat, see responses, understand progress.
**Primary action:** Enter to send message.
**Secondary actions:** Ctrl+B sidebar, Ctrl+P commands, Ctrl+F search.
**Navigation:** Forward to workflow screens when workflow starts.
**Exit:** Escape → no-op. Ctrl+C → cancel operation.
**Loading:** Shows "thinking..." in footer.
**Error:** Error message in conversation + narrative explanation.
**Empty state:** Welcome message or previous conversation.
**Success state:** Response appears in conversation.
**Failure state:** Error in conversation + recovery suggestion.

```
Terminal ≥80 cols (with sidebar, workflow active):

┌──────────────────────────────────────────────────────────────┐
│ M31A  chat                                         gpt-4o   │
├────────────────────────────────┬─────────────────────────────┤
│                                │ M31A                        │
│  I need to refactor the auth   │                             │
│  module to use JWT tokens.     │ 🏗 Planning                 │
│                                │                             │
│  I'll refactor the auth module │   auth.go                   │
│  to use JWT tokens. Here's my  │   provider.go               │
│  plan:                         │                             │
│                                │ 42% context                 │
│  1. Create token package       │                             │
│  2. Update auth middleware     │                             │
│  3. Add refresh token support  │                             │
│                                │                             │
│  Let me start by reading the   │                             │
│  current auth implementation.  │                             │
│                                │                             │
│  Reading source files          │                             │
│    auth.go                     │                             │
│    middleware.go                │                             │
│                                │                             │
│  Now I'll create the token     │                             │
│  package...                    │                             │
│                                │                             │
│  ───────────────────────────── │                             │
│  │ Type a message...       │   │                             │
│  └──────────────────────────── │                             │
├────────────────────────────────┴─────────────────────────────┤
│ ⌂ project  main              ctrl+p commands                 │
└──────────────────────────────────────────────────────────────┘


Terminal ≥80 cols (no sidebar, idle):

┌──────────────────────────────────────────────────────────────┐
│ M31A  chat                                         gpt-4o   │
├──────────────────────────────────────────────────────────────┤
│                                                              │
│  How do I reset a PostgreSQL database to a specific migration?│
│                                                              │
│  To reset a PostgreSQL database to a specific migration      │
│  using golang-migrate, you can use the `down` command:       │
│                                                              │
│    migrate -path ./migrations -database $DATABASE_URL down 1  │
│                                                              │
│  This rolls back one migration. To go to a specific version: │
│                                                              │
│    migrate -path ./migrations -database $DATABASE_URL goto 5 │
│                                                              │
│  Where 5 is the migration version number.                    │
│                                                              │
│  ─────────────────────────────                               │
│  │ Type a message...       │                                 │
│  └────────────────────────────                               │
├──────────────────────────────────────────────────────────────┤
│ ⌂ project  main              ctrl+p commands                 │
└──────────────────────────────────────────────────────────────┘
```

---

## 8.3 GoalInput Screen

**Purpose:** Capture the user's initial task description.
**Primary goal:** Submit a clear goal.
**Primary action:** Enter to submit.
**Secondary actions:** Escape to go back.
**Navigation:** Forward to Discuss (if questions needed) or Plan.
**Exit:** Escape → Home.
**Loading:** N/A.
**Error:** Validation message below input.
**Empty state:** Large input area with placeholder.
**Success state:** Transitions to Discuss or Plan.
**Failure state:** Validation error.

```
┌──────────────────────────────────────────────────────────────┐
│ M31A  Goal                                              Goal │
├──────────────────────────────────────────────────────────────┤
│                                                              │
│  What would you like M31A to build?                          │
│                                                              │
│  ┌──────────────────────────────────────────────────────┐    │
│  │                                                      │    │
│  │  Build a REST API with JWT authentication,           │    │
│  │  rate limiting, and comprehensive test coverage.     │    │
│  │                                                      │    │
│  └──────────────────────────────────────────────────────┘    │
│                                                              │
│  Be specific. M31A works best with clear goals.              │
│                                                              │
│                                                              │
├──────────────────────────────────────────────────────────────┤
│ ⌂ project                                           Escape │
└──────────────────────────────────────────────────────────────┘
```

---

## 8.4 Discuss Screen

**Purpose:** M31A asks clarifying questions before planning.
**Primary goal:** Answer questions to refine the plan.
**Primary action:** Enter to submit answer.
**Secondary actions:** Escape to skip question, Ctrl+Shift+S to skip all.
**Navigation:** Forward to Plan after all questions answered.
**Exit:** Escape → skip current question. After last question → Plan.
**Loading:** N/A.
**Error:** N/A.
**Empty state:** N/A (questions always present).
**Success state:** All questions answered → Plan.
**Failure state:** N/A.

```
┌──────────────────────────────────────────────────────────────┐
│ M31A  Discuss                                          Plan  │
├──────────────────────────────────────────────────────────────┤
│                                                              │
│  2 of 3                                                      │
│                                                              │
│  What database should be used for the user session store?    │
│                                                              │
│  ┌──────────────────────────────────────────────────────┐    │
│  │ PostgreSQL with pgx driver                          │    │
│  └──────────────────────────────────────────────────────┘    │
│                                                              │
│                                                              │
│                                                              │
├──────────────────────────────────────────────────────────────┤
│ ⌂ project                                    Enter to submit│
└──────────────────────────────────────────────────────────────┘
```

---

## 8.5 Plan Screen

**Purpose:** Review and approve M31A's implementation plan.
**Primary goal:** Approve, refine, or reject the plan.
**Primary action:** y to approve, r to refine.
**Secondary actions:** j/k to scroll, Escape to go back.
**Navigation:** Forward to Execute on approval.
**Exit:** Escape → REPL (pauses workflow).
**Loading:** Shows "Generating plan..." during creation.
**Error:** Plan generation failure shown in conversation.
**Empty state:** N/A (plan always present).
**Success state:** Approved → Execute.
**Failure state:** Plan rejected → Discuss for more info.

```
Plan review mode:

┌──────────────────────────────────────────────────────────────┐
│ M31A  Plan                                            Plan   │
├──────────────────────────────────────────────────────────────┤
│                                                              │
│  Implementation Plan                                         │
│                                                              │
│  Phase 1: Token Package                                      │
│  ─ Create internal/auth/token package                        │
│  ─ Implement JWT generation and validation                   │
│  ─ Add refresh token rotation                                │
│                                                              │
│  Phase 2: Middleware Update                                  │
│  ─ Update existing auth middleware to use JWT                │
│  ─ Add token extraction from headers                        │
│  ─ Implement token refresh endpoint                          │
│                                                              │
│  Phase 3: Testing                                            │
│  ─ Unit tests for token package                              │
│  ─ Integration tests for middleware                          │
│  ─ End-to-end auth flow tests                                │
│                                                              │
│  ──────────────────────────────────────────────────────────  │
│  [y] Accept & Execute   [r] Refine   [Esc] Back             │
│                                                              │
├──────────────────────────────────────────────────────────────┤
│ ⌂ project  main                                             │
└──────────────────────────────────────────────────────────────┘
```

---

## 8.6 Execute Screen

**Purpose:** Observe task execution progress.
**Primary goal:** Understand what is happening and trust progress.
**Primary action:** Passive (observe).
**Secondary actions:** p to pause, j/k to scroll, Escape to go back.
**Navigation:** Forward to Verify when all tasks complete.
**Exit:** Escape → REPL (pauses execution).
**Loading:** Shows progress bar + current activity.
**Error:** Failed task highlighted with error message.
**Empty state:** N/A (tasks always present).
**Success state:** All tasks complete → Verify.
**Failure state:** Task failed → shows error, self-healing attempt.

```
Execution in progress:

┌──────────────────────────────────────────────────────────────┐
│ M31A  Execute                                         Verify │
├──────────────────────────────────────────────────────────────┤
│                                                              │
│  Running tests…                                              │
│                                                              │
│  ████████████████░░░░░░░░░░░░  3/5 complete                 │
│                                                              │
│    ✓ Install dependencies                                    │
│    ✓ Create auth module                                      │
│    ● Running tests                                           │
│      $ go test ./...                                         │
│    ✗ Build project                                           │
│      Error: cannot find module providing package              │
│      github.com/user/project/auth                            │
│    ○ Deploy                                                  │
│                                                              │
├──────────────────────────────────────────────────────────────┤
│ ⌂ project  main                           p pause  Esc back  │
└──────────────────────────────────────────────────────────────┘


Execution paused:

┌──────────────────────────────────────────────────────────────┐
│ M31A  Execute                                         Verify │
├──────────────────────────────────────────────────────────────┤
│                                                              │
│  Paused                                                      │
│                                                              │
│  ░░░░░░░░░░░░░░░░░░░░░░░░░░░░  3/5 complete  PAUSED        │
│                                                              │
│    ✓ Install dependencies                                    │
│    ✓ Create auth module                                      │
│    ● Running tests                                           │
│    ✗ Build project                                           │
│    ○ Deploy                                                  │
│                                                              │
├──────────────────────────────────────────────────────────────┤
│ ⌂ project  main                           p resume Esc back  │
└──────────────────────────────────────────────────────────────┘
```

---

## 8.7 Verify Screen

**Purpose:** Review build/test verification results.
**Primary goal:** Confirm everything works or identify failures.
**Primary action:** y to proceed, n to heal.
**Secondary actions:** j/k to scroll, Enter to continue.
**Navigation:** Forward to Runtime or Ship.
**Exit:** Escape → back to Execute.
**Loading:** Shows verification in progress.
**Error:** Failed verification with details.
**Empty state:** N/A.
**Success state:** All checks pass → Ship.
**Failure state:** Self-healing attempted, then bisect fallback.

```
Verification passed:

┌──────────────────────────────────────────────────────────────┐
│ M31A  Verify                                          Ship  │
├──────────────────────────────────────────────────────────────┤
│                                                              │
│  All checks passed                                           │
│                                                              │
│    ✓ Build succeeded                                         │
│    ✓ Tests passed (47/47)                                    │
│    ✓ No TODO/FIXME comments                                  │
│    ✓ No hardcoded secrets                                    │
│                                                              │
│                                                              │
├──────────────────────────────────────────────────────────────┤
│ ⌂ project  main                           y proceed  Esc back│
└──────────────────────────────────────────────────────────────┘


Verification failed:

┌──────────────────────────────────────────────────────────────┐
│ M31A  Verify                                          Ship  │
├──────────────────────────────────────────────────────────────┤
│                                                              │
│  Verification failed                                         │
│                                                              │
│    ✓ Build succeeded                                         │
│    ✗ Tests failed (2/47 failed)                              │
│      auth_test.go:42: expected 200 got 401                   │
│      middleware_test.go:18: nil pointer dereference          │
│    ✓ No TODO/FIXME comments                                  │
│    ✓ No hardcoded secrets                                    │
│                                                              │
│  ──────────────────────────────────────────────────────────  │
│  [y] Retry   [h] Self-heal   [Esc] Back                     │
│                                                              │
├──────────────────────────────────────────────────────────────┤
│ ⌂ project  main                                             │
└──────────────────────────────────────────────────────────────┘
```

---

## 8.8 Runtime Screen

**Purpose:** Start and verify dev server.
**Primary goal:** Confirm the application runs.
**Primary action:** Enter to continue to Ship.
**Secondary actions:** j/k to scroll logs, Escape to go back.
**Navigation:** Forward to Ship.
**Exit:** Escape → back to Verify.
**Loading:** Shows server startup.
**Error:** Server failed to start.
**Empty state:** N/A.
**Success state:** Server responding → Ship.
**Failure state:** Server error → back to Execute for fix.

```
Runtime check passed:

┌──────────────────────────────────────────────────────────────┐
│ M31A  Runtime                                          Ship │
├──────────────────────────────────────────────────────────────┤
│                                                              │
│  Dev server running on :8080                                 │
│                                                              │
│    ✓ Server started                                          │
│    ✓ Health check: 200 OK                                    │
│    ✓ GET /api/auth/login → 200                               │
│    ✓ POST /api/auth/register → 201                           │
│                                                              │
│                                                              │
├──────────────────────────────────────────────────────────────┤
│ ⌂ project  main                          Enter to continue   │
└──────────────────────────────────────────────────────────────┘
```

---

## 8.9 Ship Screen

**Purpose:** Display session completion summary.
**Primary goal:** Understand what was accomplished.
**Primary action:** Enter to return home.
**Secondary actions:** d for walkthrough/diff.
**Navigation:** Forward to Home.
**Exit:** Escape → Home.
**Loading:** N/A.
**Error:** N/A.
**Empty state:** N/A.
**Success state:** Summary displayed.
**Failure state:** N/A.

```
┌──────────────────────────────────────────────────────────────┐
│ M31A  Ship                                              Home │
├──────────────────────────────────────────────────────────────┤
│                                                              │
│  All tasks complete                                          │
│                                                              │
│  5/5 tasks  ·  2m 30s  ·  $0.05                             │
│  +3 ~5 -1  ·  12.3K tokens                                  │
│                                                              │
│  Commits                                                     │
│  a1b2c3d  feat: add JWT auth package                         │
│  e4f5g6h  fix: update middleware for JWT                     │
│                                                              │
│              [d] walkthrough    [Enter] continue              │
│                                                              │
├──────────────────────────────────────────────────────────────┤
│ ⌂ project  main                                             │
└──────────────────────────────────────────────────────────────┘
```

---

## 8.10 Permission Modal

**Purpose:** Approve or deny potentially dangerous tool actions.
**Primary goal:** Make an informed decision.
**Primary action:** y to approve, n to deny.
**Secondary actions:** a to allow all, Escape to deny.
**Navigation:** Returns to previous screen on decision.
**Exit:** Escape → deny (safe default).
**Loading:** N/A.
**Error:** N/A.
**Empty state:** N/A.
**Success state:** Tool executes.
**Failure state:** Tool denied, M31A adjusts plan.

```
Permission modal (dimmed background):

┌──────────────────────────────────────────────────────────────┐
│░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░│
│░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░│
│░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░│
│░░░░░░░┌──────────────────────────────────────────┐░░░░░░░░░░│
│░░░░░░░│                                          │░░░░░░░░░░│
│░░░░░░░│  M31A wants to rebuild the project        │░░░░░░░░░░│
│░░░░░░░│  output directory.                        │░░░░░░░░░░│
│░░░░░░░│                                          │░░░░░░░░░░│
│░░░░░░░│  This will delete ./dist/ and run a      │░░░░░░░░░░│
│░░░░░░░│  fresh build.                             │░░░░░░░░░░│
│░░░░░░░│                                          │░░░░░░░░░░│
│░░░░░░░│  ─────────────────────────────────────── │░░░░░░░░░░│
│░░░░░░░│  $ rm -rf ./dist && npm run build        │░░░░░░░░░░│
│░░░░░░░│                                          │░░░░░░░░░░│
│░░░░░░░│  [y] Allow  [n] Deny  [a] Allow All      │░░░░░░░░░░│
│░░░░░░░│                                          │░░░░░░░░░░│
│░░░░░░░└──────────────────────────────────────────┘░░░░░░░░░░│
│░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░│
│░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░│
├──────────────────────────────────────────────────────────────┤
│ ⌂ project  main                                             │
└──────────────────────────────────────────────────────────────┘
```

---

## 8.11 Command Palette

**Purpose:** Quick access to all commands and screens.
**Primary goal:** Find and execute a command.
**Primary action:** Enter to execute selected command.
**Secondary actions:** Type to filter, j/k to navigate, Escape to close.
**Navigation:** Executes command (may navigate to any screen).
**Exit:** Escape → close palette, return to previous screen.
**Loading:** N/A.
**Error:** "No matching commands" if filter yields nothing.
**Empty state:** Show all commands.
**Success state:** Command executed.
**Failure state:** Command failed (shown in conversation).

```
┌──────────────────────────────────────────────────────────────┐
│░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░│
│░░░░░░░┌──────────────────────────────────────────┐░░░░░░░░░░│
│░░░░░░░│ > plan_                                  │░░░░░░░░░░│
│░░░░░░░│                                          │░░░░░░░░░░│
│░░░░░░░│  /plan        Create implementation plan  │░░░░░░░░░░│
│░░░░░░░│  /plan-edit   Edit the current plan       │░░░░░░░░░░│
│░░░░░░░│  /plan-show   Display current plan        │░░░░░░░░░░│
│░░░░░░░│                                          │░░░░░░░░░░│
│░░░░░░░└──────────────────────────────────────────┘░░░░░░░░░░│
│░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░│
└──────────────────────────────────────────────────────────────┘
```

---

## 8.12 Model Selector

**Purpose:** Choose which LLM model to use.
**Primary goal:** Select a model.
**Primary action:** Enter to select.
**Secondary actions:** j/k to navigate, / to filter, Escape to close.
**Navigation:** Returns to previous screen with new model.
**Exit:** Escape → close, keep current model.
**Loading:** Shows "Fetching models..." during catalog load.
**Error:** "Failed to fetch models" with retry option.
**Empty state:** "No models available" with provider setup hint.
**Success state:** Model selected, returns to previous screen.
**Failure state:** Model unavailable, keeps previous selection.

```
┌──────────────────────────────────────────────────────────────┐
│░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░│
│░░░░░░░┌──────────────────────────────────────────┐░░░░░░░░░░│
│░░░░░░░│ Select Model                             │░░░░░░░░░░│
│░░░░░░░│                                          │░░░░░░░░░░│
│░░░░░░░│ > gpt-4o                    [OR]         │░░░░░░░░░░│
│░░░░░░░│   gpt-4o-mini               [OR]         │░░░░░░░░░░│
│░░░░░░░│   claude-sonnet-4-20250514  [OR]         │░░░░░░░░░░│
│░░░░░░░│   gemini-2.5-pro            [ZEN]        │░░░░░░░░░░│
│░░░░░░░│   llama-3.3-70b             [NVIDIA]     │░░░░░░░░░░│
│░░░░░░░│                                          │░░░░░░░░░░│
│░░░░░░░│   Enter select  / filter  Esc close      │░░░░░░░░░░│
│░░░░░░░└──────────────────────────────────────────┘░░░░░░░░░░│
│░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░│
└──────────────────────────────────────────────────────────────┘
```

---

## 8.13 Settings Screen

**Purpose:** Configure M31A application settings.
**Primary goal:** Modify a setting.
**Primary action:** e to edit selected setting.
**Secondary actions:** Tab to switch tabs, j/k to navigate, Escape to go back.
**Navigation:** Back to previous screen.
**Exit:** Escape → REPL.
**Loading:** N/A.
**Error:** Validation error below edited value.
**Empty state:** N/A.
**Success state:** Setting saved, visual confirmation.
**Failure state:** Validation error, setting unchanged.

```
┌──────────────────────────────────────────────────────────────┐
│ M31A  Settings                                         Back │
├──────────────────────────────────────────────────────────────┤
│                                                              │
│  General  │  Workflow  │  UI  │  Providers                   │
│  ────────                                                       │
│                                                              │
│  Default Model     gpt-4o                                    │
│  Theme             dark                                      │
│  Show Cost         yes                                       │
│  Auto-compact      yes                                       │
│  Max Context       128000                                    │
│                                                              │
│                                                              │
├──────────────────────────────────────────────────────────────┤
│ ⌂ project  main                  Tab switch  e edit          │
└──────────────────────────────────────────────────────────────┘
```

---

## 8.14 Diff Screen

**Purpose:** View git diff of changes.
**Primary goal:** Review what changed.
**Primary action:** Read-only (scroll).
**Secondary actions:** j/k to scroll, q to close.
**Navigation:** Back to previous screen.
**Exit:** Escape or q → previous screen.
**Loading:** N/A.
**Error:** "No changes to display" if clean.
**Empty state:** "No changes" message.
**Success state:** Diff displayed.
**Failure state:** N/A.

```
┌──────────────────────────────────────────────────────────────┐
│ M31A  Diff                                    j/k scroll q   │
├──────────────────────────────────────────────────────────────┤
│                                                              │
│  diff --git a/internal/auth/middleware.go                    │
│  b/internal/auth/middleware.go                               │
│  @@ -10,7 +10,12 @@                                         │
│   package auth                                                │
│                                                              │
│  +import (                                                   │
│  +    "github.com/golang-jwt/jwt/v5"                         │
│  + )                                                         │
│                                                              │
│  -func ValidateToken(token string) error {                   │
│  +func ValidateToken(token string) (*Claims, error) {        │
│  +    claims := &Claims{}                                    │
│  +    _, err := jwt.ParseWithClaims(token, claims, ...)      │
│  +    return claims, err                                     │
│   }                                                          │
│                                                              │
├──────────────────────────────────────────────────────────────┤
│ ⌂ project  main                                             │
└──────────────────────────────────────────────────────────────┘
```

---

## 8.15 Rollback Screen

**Purpose:** Revert git commits.
**Primary goal:** Select commits to revert.
**Primary action:** Enter to revert selected commit.
**Secondary actions:** j/k to navigate, Escape to go back.
**Navigation:** Back to REPL after rollback.
**Exit:** Escape → REPL.
**Loading:** N/A.
**Error:** "Nothing to rollback" if no commits.
**Empty state:** "No commits to rollback" message.
**Success state:** Rollback complete, returns to REPL.
**Failure state:** Rollback conflict, error shown.

```
┌──────────────────────────────────────────────────────────────┐
│ M31A  Rollback                                         Back │
├──────────────────────────────────────────────────────────────┤
│                                                              │
│  Recent commits (newest first):                              │
│                                                              │
│  > a1b2c3d  feat: add JWT auth package         2 min ago    │
│    e4f5g6h  fix: update middleware for JWT     5 min ago    │
│    i7j8k9l  chore: update dependencies         10 min ago   │
│                                                              │
│  Reverting a1b2c3d will undo:                                │
│    2 files changed, 150 insertions, 30 deletions             │
│                                                              │
│  ──────────────────────────────────────────────────────────  │
│  [Enter] Revert   [Esc] Back                                │
│                                                              │
├──────────────────────────────────────────────────────────────┤
│ ⌂ project  main                                             │
└──────────────────────────────────────────────────────────────┘
```

---

## 8.16 Ledger Screen

**Purpose:** View session history.
**Primary goal:** Browse past sessions.
**Primary action:** Enter to view session detail.
**Secondary actions:** j/k to navigate, Escape to go back.
**Navigation:** Back to REPL.
**Exit:** Escape → REPL.
**Loading:** N/A.
**Error:** "No session history" if empty.
**Empty state:** "No sessions recorded" message.
**Success state:** Session list displayed.
**Failure state:** N/A.

```
┌──────────────────────────────────────────────────────────────┐
│ M31A  Ledger                                           Back │
├──────────────────────────────────────────────────────────────┤
│                                                              │
│  Date          Tasks  Cost    Duration  Summary              │
│  ────────────────────────────────────────────────────────    │
│  > 2026-07-01  5/5    $0.05   2m 30s   JWT auth module      │
│    2026-06-30  3/4    $0.03   1m 45s   Fix login bug        │
│    2026-06-29  8/8    $0.12   5m 10s   REST API scaffold    │
│                                                              │
│                                                              │
├──────────────────────────────────────────────────────────────┤
│ ⌂ project  main                          Enter to view       │
└──────────────────────────────────────────────────────────────┘
```

---

## 8.17 Metrics Screen

**Purpose:** View usage statistics.
**Primary goal:** Understand usage patterns.
**Primary action:** Read-only.
**Secondary actions:** Escape to go back.
**Navigation:** Back to REPL.
**Exit:** Escape → REPL.
**Loading:** N/A.
**Error:** N/A.
**Empty state:** "No metrics recorded yet" message.
**Success state:** Metrics displayed.
**Failure state:** N/A.

```
┌──────────────────────────────────────────────────────────────┐
│ M31A  Metrics                                          Back │
├──────────────────────────────────────────────────────────────┤
│                                                              │
│  Session Statistics                                          │
│                                                              │
│  Total tokens      45,230                                    │
│  Total cost        $0.18                                     │
│  Avg task time     12.3s                                     │
│  Tasks completed   23                                        │
│  Tasks failed      2                                         │
│                                                              │
│  Most used tools:                                            │
│    FileRead (42)  Edit (18)  Bash (12)                      │
│                                                              │
├──────────────────────────────────────────────────────────────┤
│ ⌂ project  main                                             │
└──────────────────────────────────────────────────────────────┘
```

---

## 8.18 Config Screen

**Purpose:** Edit M31A configuration file.
**Primary goal:** Modify config.
**Primary action:** Edit text, save.
**Secondary actions:** Tab to switch sections, / to search, Escape to go back.
**Navigation:** Back to REPL.
**Exit:** Escape → REPL (with save prompt if unsaved).
**Loading:** N/A.
- **Error:** TOML parse error highlighted.
**Empty state:** N/A.
**Success state:** Config saved.
**Failure state:** Parse error, revert to last valid.

```
┌──────────────────────────────────────────────────────────────┐
│ M31A  Config                                           Back │
├──────────────────────────────────────────────────────────────┤
│                                                              │
│  [ui]  [workflow]  [providers]  [tools]                      │
│                                                              │
│  [ui]                                                        │
│  theme = "dark"                                              │
│  show_cost = true                                            │
│  default_model = "gpt-4o"                                    │
│                                                              │
│  [workflow]                                                  │
│  enable_discuss = true                                       │
│  enable_verify = true                                        │
│                                                              │
│                                                              │
├──────────────────────────────────────────────────────────────┤
│ ⌂ project  main                  Tab switch  / search        │
└──────────────────────────────────────────────────────────────┘
```

---

## 8.19 Help Screen

**Purpose:** Show available commands and keybindings.
**Primary goal:** Find a command or shortcut.
**Primary action:** Read-only.
**Secondary actions:** / to search, Escape to close.
**Navigation:** Back to previous screen.
**Exit:** Escape → previous screen.
**Loading:** N/A.
**Error:** N/A.
**Empty state:** N/A.
**Success state:** Help displayed.
**Failure state:** N/A.

```
┌──────────────────────────────────────────────────────────────┐
│ M31A  Help                                              Back │
├──────────────────────────────────────────────────────────────┤
│                                                              │
│  Keyboard Shortcuts                                          │
│                                                              │
│  Global                                                      │
│    Ctrl+P        Command palette                             │
│    Ctrl+B        Toggle sidebar                              │
│    Ctrl+C        Cancel operation                            │
│    Escape        Go back                                     │
│                                                              │
│  REPL                                                        │
│    Enter         Send message                                │
│    Up/Down       History                                     │
│    Tab           Autocomplete                                │
│    Ctrl+F        Search conversation                         │
│                                                              │
│  Commands                                                    │
│    /plan         Create implementation plan                  │
│    /execute      Execute the plan                            │
│    /verify       Run verification                            │
│    /ship         Complete session                            │
│    /rollback     Revert changes                              │
│    /model        Change model                                │
│                                                              │
├──────────────────────────────────────────────────────────────┤
│ ⌂ project  main                  / search  Esc close         │
└──────────────────────────────────────────────────────────────┘
```

---

## 8.20 Session Detail Screen

**Purpose:** View details of a past session.
**Primary goal:** Review what happened in a session.
**Primary action:** Read-only.
**Secondary actions:** j/k to scroll, Escape to go back.
**Navigation:** Back to Ledger.
**Exit:** Escape → Ledger.
**Loading:** N/A.
**Error:** "Session not found" if deleted.
**Empty state:** N/A.
**Success state:** Session detail displayed.
**Failure state:** N/A.

```
┌──────────────────────────────────────────────────────────────┐
│ M31A  Session  abc123                                   Back │
├──────────────────────────────────────────────────────────────┤
│                                                              │
│  Session abc123                                              │
│  2026-07-01 14:30 — 14:32                                    │
│  Model: gpt-4o [OR]                                          │
│                                                              │
│  Tasks: 5/5 complete                                         │
│  Tokens: 12,340                                              │
│  Cost: $0.05                                                 │
│                                                              │
│  Commits:                                                    │
│    a1b2c3d feat: add JWT auth package                        │
│    e4f5g6h fix: update middleware                            │
│                                                              │
│                                                              │
├──────────────────────────────────────────────────────────────┤
│ ⌂ project  main                          j/k scroll          │
└──────────────────────────────────────────────────────────────┘
```

---

## 8.21 File Explorer Screen

**Purpose:** Browse project files.
**Primary goal:** Navigate the file tree.
**Primary action:** Enter to open file.
**Secondary actions:** j/k to navigate, Escape to go back.
**Navigation:** Back to REPL.
**Exit:** Escape → REPL.
**Loading:** N/A.
**Error:** "Cannot read directory" if permissions denied.
**Empty state:** "No files in directory" if empty.
**Success state:** File content displayed.
**Failure state:** Permission error.

```
┌──────────────────────────────────────────────────────────────┐
│ M31A  Files                                            Back │
├──────────────────────────────────────────────────────────────┤
│                                                              │
│  internal/                                                   │
│  > auth/                                                     │
│    provider/                                                 │
│    types/                                                    │
│  main.go                                                     │
│  go.mod                                                      │
│  go.sum                                                      │
│                                                              │
│                                                              │
├──────────────────────────────────────────────────────────────┤
│ ⌂ project  main                      Enter open  j/k nav     │
└──────────────────────────────────────────────────────────────┘
```

---

## 8.22 Tool Detail Screen

**Purpose:** Inspect raw tool execution details.
**Primary goal:** Debug a specific tool call.
**Primary action:** Read-only.
**Secondary actions:** j/k to scroll, Escape to go back.
**Navigation:** Back to conversation.
**Exit:** Escape → REPL.
**Loading:** N/A.
**Error:** N/A.
**Empty state:** N/A.
**Success state:** Tool details displayed.
**Failure state:** N/A.

```
┌──────────────────────────────────────────────────────────────┐
│ M31A  Tool Output                                     Back │
├──────────────────────────────────────────────────────────────┤
│                                                              │
│  Tool: Edit                                                   │
│  Duration: 120ms                                              │
│  Status: success                                              │
│                                                              │
│  Input:                                                       │
│  {                                                            │
│    "file": "internal/auth/middleware.go",                     │
│    "old": "func ValidateToken...",                            │
│    "new": "func ValidateToken(token string) (*Claims..."     │
│  }                                                            │
│                                                              │
│  Output:                                                      │
│  File edited successfully. 15 lines changed.                  │
│                                                              │
├──────────────────────────────────────────────────────────────┤
│ ⌂ project  main                          j/k scroll          │
└──────────────────────────────────────────────────────────────┘
```

---

## 8.23 Phase Model Picker Screen

**Purpose:** Choose which model to use for a specific workflow phase.
**Primary goal:** Select a model for the phase.
**Primary action:** Enter to select.
**Secondary actions:** j/k to navigate, Escape to go back.
**Navigation:** Back to previous screen.
**Exit:** Escape → previous screen.
**Loading:** N/A.
**Error:** N/A.
**Empty state:** N/A.
**Success state:** Model selected.
**Failure state:** N/A.

```
┌──────────────────────────────────────────────────────────────┐
│ M31A  Model Setup                                      Back │
├──────────────────────────────────────────────────────────────┤
│                                                              │
│  Model for: Execute phase                                    │
│                                                              │
│  > gpt-4o                    [OR]         128K context       │
│    claude-sonnet-4-20250514  [OR]         200K context       │
│    gemini-2.5-pro            [ZEN]        1M context         │
│                                                              │
│                                                              │
├──────────────────────────────────────────────────────────────┤
│ ⌂ project  main                          Enter select         │
└──────────────────────────────────────────────────────────────┘
```

---

## 8.24 Ghost Picker Screen

**Purpose:** Select a ghost (parallel agent) configuration.
**Primary goal:** Choose ghost settings.
**Primary action:** Enter to select.
**Secondary actions:** j/k to navigate, Escape to go back.
**Navigation:** Back to previous screen.
**Exit:** Escape → previous screen.
**Loading:** N/A.
**Error:** N/A.
**Empty state:** N/A.
**Success state:** Ghost configured.
**Failure state:** N/A.

```
┌──────────────────────────────────────────────────────────────┐
│ M31A  Ghost Picker                                     Back │
├──────────────────────────────────────────────────────────────┤
│                                                              │
│  Select ghost mode:                                          │
│                                                              │
│  > Parallel    Run tasks in parallel with sub-agents         │
│    Sequential  Run tasks one at a time                       │
│    Hybrid      Parallel where safe, sequential otherwise     │
│                                                              │
│                                                              │
├──────────────────────────────────────────────────────────────┤
│ ⌂ project  main                          Enter select         │
└──────────────────────────────────────────────────────────────┘
```

---

## 8.25 Ghost Output Screen

**Purpose:** View output from a ghost agent.
**Primary goal:** Review ghost's work.
**Primary action:** Read-only.
**Secondary actions:** j/k to scroll, Escape to close.
**Navigation:** Back to previous screen.
**Exit:** Escape → previous screen.
**Loading:** Shows ghost progress.
**Error:** Ghost failed, error shown.
**Empty state:** "Ghost is working..." during execution.
**Success state:** Ghost output displayed.
**Failure state:** Ghost error with details.

```
┌──────────────────────────────────────────────────────────────┐
│ M31A  Ghost Output                                     Back │
├──────────────────────────────────────────────────────────────┤
│                                                              │
│  Ghost: explore-agent                                        │
│  Status: complete                                            │
│                                                              │
│  Findings:                                                   │
│  - 3 files reference the auth middleware                     │
│  - 2 tests need updating                                    │
│  - 1 import cycle detected                                  │
│                                                              │
│                                                              │
├──────────────────────────────────────────────────────────────┤
│ ⌂ project  main                          j/k scroll          │
└──────────────────────────────────────────────────────────────┘
```

---

## 8.26 Confirm Quit Screen

**Purpose:** Confirm before quitting.
**Primary goal:** Prevent accidental quit.
**Primary action:** y to quit, n to cancel.
**Secondary actions:** None.
**Navigation:** Quits application on confirm.
**Exit:** y → quit. n → cancel, return to previous screen.
**Loading:** N/A.
**Error:** N/A.
**Empty state:** N/A.
**Success state:** Application quits.
**Failure state:** N/A.

```
┌──────────────────────────────────────────────────────────────┐
│░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░│
│░░░░░░░┌──────────────────────────────────────────┐░░░░░░░░░░│
│░░░░░░░│                                          │░░░░░░░░░░│
│░░░░░░░│  Quit M31A?                              │░░░░░░░░░░│
│░░░░░░░│                                          │░░░░░░░░░░│
│░░░░░░░│  Unsaved work will be lost.              │░░░░░░░░░░│
│░░░░░░░│                                          │░░░░░░░░░░│
│░░░░░░░│  [y] Quit  [n] Cancel                    │░░░░░░░░░░│
│░░░░░░░│                                          │░░░░░░░░░░│
│░░░░░░░└──────────────────────────────────────────┘░░░░░░░░░░│
│░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░│
└──────────────────────────────────────────────────────────────┘
```

---

## 8.27 Chat History Screen

**Purpose:** Browse previous conversations.
**Primary goal:** Find a past message.
**Primary action:** Enter to view message in context.
**Secondary actions:** j/k to navigate, / to search, Escape to go back.
**Navigation:** Back to REPL.
**Exit:** Escape → REPL.
**Loading:** N/A.
**Error:** "No chat history" if empty.
**Empty state:** "No previous messages" message.
**Success state:** History displayed.
**Failure state:** N/A.

```
┌──────────────────────────────────────────────────────────────┐
│ M31A  History                                          Back │
├──────────────────────────────────────────────────────────────┤
│                                                              │
│  Recent messages:                                            │
│                                                              │
│  > "Refactor the auth module to use JWT..."    2 min ago    │
│    "What database should we use?"              5 min ago    │
│    "Build a REST API with authentication"      8 min ago    │
│                                                              │
│                                                              │
├──────────────────────────────────────────────────────────────┤
│ ⌂ project  main                   / search  Enter view       │
└──────────────────────────────────────────────────────────────┘
```

---

## 8.28 Notifications Screen

**Purpose:** View system notifications and alerts.
**Primary goal:** Review notifications.
**Primary action:** Read-only.
**Secondary actions:** j/k to navigate, Escape to go back.
**Navigation:** Back to REPL.
**Exit:** Escape → REPL.
**Loading:** N/A.
**Error:** N/A.
**Empty state:** "No notifications" message.
**Success state:** Notifications displayed.
**Failure state:** N/A.

```
┌──────────────────────────────────────────────────────────────┐
│ M31A  Notifications                                    Back │
├──────────────────────────────────────────────────────────────┤
│                                                              │
│  Recent notifications:                                       │
│                                                              │
│  > Session abc123 completed successfully       2 min ago    │
│    Model fallback: switched to gpt-4o-mini     5 min ago    │
│    Config reloaded from m31a.toml              8 min ago    │
│                                                              │
│                                                              │
├──────────────────────────────────────────────────────────────┤
│ ⌂ project  main                          j/k navigate        │
└──────────────────────────────────────────────────────────────┘
```

---

## 8.29 Dashboard Screen

**Purpose:** Overview of current session and system state.
**Primary goal:** Get a quick status snapshot.
**Primary action:** Read-only.
**Secondary actions:** Enter to drill into section, Escape to go back.
**Navigation:** Back to REPL.
**Exit:** Escape → REPL.
**Loading:** N/A.
**Error:** N/A.
**Empty state:** "No active session" message.
**Success state:** Dashboard displayed.
**Failure state:** N/A.

```
┌──────────────────────────────────────────────────────────────┐
│ M31A  Dashboard                                         Back │
├──────────────────────────────────────────────────────────────┤
│                                                              │
│  Session: abc123                                             │
│  Model: gpt-4o [OR]                                          │
│  Phase: execute (3/5 tasks complete)                         │
│                                                              │
│  Context: 42% (53K/128K tokens)                              │
│  Cost: $0.05                                                 │
│  Duration: 2m 30s                                            │
│                                                              │
│  Git: main (+3 -1)                                           │
│                                                              │
│  Recent activity:                                            │
│    Running tests...                                          │
│    Editing auth.go                                           │
│    Reading provider.go                                       │
│                                                              │
├──────────────────────────────────────────────────────────────┤
│ ⌂ project  main                          Enter drill down     │
└──────────────────────────────────────────────────────────────┘
```

---

## 8.30 Bisect Screen

**Purpose:** Run git bisect to find problematic commit.
**Primary goal:** Identify the commit that introduced a bug.
**Primary action:** y/n to mark commits as good/bad.
**Secondary actions:** Escape to go back.
**Navigation:** Back to REPL.
**Exit:** Escape → REPL.
**Loading:** Shows bisect in progress.
**Error:** Bisect failed, error shown.
**Empty state:** "No bisect in progress" message.
**Success state:** Problematic commit identified.
**Failure state:** Bisect inconclusive.

```
┌──────────────────────────────────────────────────────────────┐
│ M31A  Bisect                                          Back │
├──────────────────────────────────────────────────────────────┤
│                                                              │
│  Git Bisect in progress                                      │
│                                                              │
│  Testing: e4f5g6h "fix: update middleware"                   │
│                                                              │
│  Is this commit good or bad?                                 │
│                                                              │
│  [y] Good  [n] Bad  [s] Skip                                │
│                                                              │
│                                                              │
├──────────────────────────────────────────────────────────────┤
│ ⌂ project  main                                             │
└──────────────────────────────────────────────────────────────┘
```

---

## 8.31 Runtime Check Screen (Alias: Runtime)

**Purpose:** Start and verify dev server.
**Primary goal:** Confirm the application runs.
**Primary action:** Enter to continue.
**Secondary actions:** j/k to scroll logs, Escape to go back.
**Navigation:** Forward to Ship.
**Exit:** Escape → back to Verify.
**Loading:** Shows server startup.
**Error:** Server failed to start.
**Empty state:** N/A.
**Success state:** Server responding.
**Failure state:** Server error.

```
(This screen is identical to Screen 8.8 Runtime)

┌──────────────────────────────────────────────────────────────┐
│ M31A  Runtime                                          Ship │
├──────────────────────────────────────────────────────────────┤
│                                                              │
│  Dev server running on :8080                                 │
│                                                              │
│    ✓ Server started                                          │
│    ✓ Health check: 200 OK                                    │
│    ✓ GET /api/auth/login → 200                               │
│                                                              │
├──────────────────────────────────────────────────────────────┤
│ ⌂ project  main                          Enter to continue   │
└──────────────────────────────────────────────────────────────┘
```

---

## 8.32 Resume Screen

**Purpose:** Resume a previous session.
**Primary goal:** Select a session to resume.
**Primary action:** Enter to resume.
**Secondary actions:** j/k to navigate, Escape to go back.
**Navigation:** Back to REPL with session restored.
**Exit:** Escape → REPL (new session).
**Loading:** N/A.
**Error:** "Session not found" if deleted.
**Empty state:** "No sessions to resume" message.
**Success state:** Session restored, returns to REPL.
**Failure state:** Session corrupted, starts new.

```
┌──────────────────────────────────────────────────────────────┐
│ M31A  Sessions                                         Back │
├──────────────────────────────────────────────────────────────┤
│                                                              │
│  Recent sessions:                                            │
│                                                              │
│  > abc123   "JWT auth"          2 min ago    5/5 tasks      │
│    def456   "REST API"          1 day ago    8/8 tasks      │
│    ghi789   "Fix login bug"     2 days ago   3/4 tasks      │
│                                                              │
│                                                              │
├──────────────────────────────────────────────────────────────┤
│ ⌂ project  main                          Enter resume         │
└──────────────────────────────────────────────────────────────┘
```

---

## 8.33 Decisions Screen

**Purpose:** View architectural decisions made during session.
**Primary goal:** Review decisions.
**Primary action:** Read-only.
**Secondary actions:** j/k to scroll, Escape to go back.
**Navigation:** Back to REPL.
**Exit:** Escape → REPL.
**Loading:** N/A.
**Error:** N/A.
**Empty state:** "No decisions recorded" message.
**Success state:** Decisions displayed.
**Failure state:** N/A.

```
┌──────────────────────────────────────────────────────────────┐
│ M31A  Decisions                                         Back │
├──────────────────────────────────────────────────────────────┤
│                                                              │
│  Decisions made in this session:                             │
│                                                              │
│  1. Use JWT for authentication (not sessions)                │
│     Reason: Stateless, works with microservices              │
│                                                              │
│  2. Use PostgreSQL for session store                         │
│     Reason: User requested, ACID compliance                  │
│                                                              │
│  3. Add rate limiting at middleware level                    │
│     Reason: Centralized, applies to all endpoints            │
│                                                              │
├──────────────────────────────────────────────────────────────┤
│ ⌂ project  main                          j/k scroll          │
└──────────────────────────────────────────────────────────────┘
```

---

# 9. Interaction Guidelines

## 9.1 Keyboard Interaction Principles

1. **One hand on the keyboard.** All actions reachable without leaving home row position (for touch typists).
2. **No chords for common actions.** Single key for frequent operations (y/n/j/k/Enter/Escape).
3. **Chords for power operations.** Ctrl+key for system operations (Ctrl+P, Ctrl+B, Ctrl+C).
4. **No conflicts with terminal.** Never override Ctrl+Z (suspend), Ctrl+D (EOF), Ctrl+S (scroll lock), Ctrl+Q (scroll unlock).
5. **Consistent meaning.** y always means yes/approve. n always means no/deny. Escape always means go back.

## 9.2 Mouse Interaction Principles

1. **Mouse is optional.** Every action has a keyboard equivalent.
2. **Scroll wheel scrolls viewports.** Natural scrolling in conversation, file tree, diffs.
3. **Click on collapsed elements expands them.** Tool output, stack traces, details.
4. **No hover-dependent information.** All information accessible via keyboard focus.
5. **Right-click is context menu** (future feature, not v1.6.2).

## 9.3 Focus Management Rules

1. **Focus follows navigation.** When a screen opens, focus moves to its primary interactive element.
2. **Focus stays in content.** Sidebar is informational; focus never moves to sidebar unless in focused mode.
3. **Modal captures focus.** When a modal opens, all keyboard input goes to the modal. Underlying content is dimmed but visible.
4. **Focus returns on dismiss.** When a modal closes, focus returns to the element that was focused before the modal opened.
5. **Tab cycles within modals.** Tab moves between interactive elements within a modal (buttons, inputs).
6. **No focus trapping in viewports.** j/k and arrow keys scroll viewports but do not "trap" focus.

## 9.4 Streaming Interaction

1. **User can type during streaming.** Input is buffered and sent after streaming completes.
2. **Ctrl+C cancels streaming.** Partial response is kept in conversation.
3. **Scroll works during streaming.** User can scroll up to read earlier content.
4. **New content auto-scrolls.** If user is at the bottom, new content scrolls into view.
5. **Manual scroll disables auto-scroll.** If user scrolls up, auto-scroll stops until they return to bottom.

## 9.5 Permission Interaction

1. **Permission modal blocks all input.** No other action possible until decision is made.
2. **Escape means deny.** Safe default. User must explicitly approve.
3. **Countdown only appears when urgent.** <5 seconds remaining. Never by default.
4. **"Allow All" is available.** For batch operations, user can approve all remaining.
5. **Decision is final.** No undo for permission decisions within the same session.

## 9.6 Error Interaction

1. **Errors appear in conversation.** Not as separate modals (except fatal errors).
2. **Errors are expandable.** Stack traces, raw output hidden behind "Show details".
3. **Errors suggest recovery.** "Retry", "Skip", "Edit configuration" are common actions.
4. **Errors do not block.** User can continue chatting after an error.
5. **Fatal errors show confirmation.** "M31A encountered a fatal error. Quit?" with details.

---

# 10. UX Design Rules

Every proposed UI element must pass this checklist before implementation:

## 10.1 Existence Test

| Question | Required Answer |
|----------|----------------|
| Why does this element exist? | Must have a clear, stated purpose |
| Does it help the user achieve their goal? | Must be "yes" |
| Can it be removed without losing essential information? | Must be "no" |

## 10.2 Duplication Test

| Question | Required Answer |
|----------|----------------|
| Is this information shown elsewhere? | Must be "no" |
| If yes, is the other location the primary source? | If yes, remove this instance |
| Does the user need this information in two places? | Must be "yes" with justification |

## 10.3 Implementation Detail Test

| Question | Required Answer |
|----------|----------------|
| Is this a tool name, API call, or internal identifier? | If yes, hide by default |
| Is this timing, metrics, or debug data? | If yes, hide by default |
| Is this a raw system event? | If yes, translate to human-readable |

## 10.4 Cognitive Load Test

| Question | Required Answer |
|----------|----------------|
| Does this element compete with the primary content? | Must be "no" |
| Can the user ignore this element without losing context? | Must be "yes" |
| Does this element require the user to parse technical information? | Must be "no" |

## 10.5 Visual Weight Test

| Question | Required Answer |
|----------|----------------|
| Is this element quieter than the primary content? | Must be "yes" |
| Does this element use color as its only hierarchy? | Must be "no" (use typography + spacing) |
| Is this element's visual weight proportional to its importance? | Must be "yes" |

## 10.6 Removal Checklist

The following elements are permanently banned from M31A's default view:

| Element | Reason |
|---------|--------|
| Tool names (FileRead, Glob, etc.) | Implementation detail |
| Tool durations (120ms, 3.2s) | Debug metric |
| Tool status icons in grouped view | Visual noise |
| Token burn rate | Debug metric |
| Speed metrics (tasks/min) | Debug metric |
| Cost trend arrows (↑↓) | Noise |
| Section dividers in sidebar | Decorative |
| Leader dots in header | Decorative |
| Provider badges [OR] [ZEN] | Redundant with model name |
| Session ID in sidebar | Rarely needed |
| Version in sidebar | Available via /version |
| Keyboard hints in sidebar | Footer handles this |
| Card borders on content screens | Whitespace suffices |
| Glow effects | Decorative |
| Countdown timers (always visible) | Only show when <5s |
| Duplicate context displays | One source of truth |

## 10.7 Approved Elements

The following elements are permanently approved for M31A:

| Element | Justification |
|---------|---------------|
| Brand name in header | Orientation |
| Breadcrumb in header | Navigation |
| Model name in header | Provider awareness |
| Cwd + branch in footer | Orientation |
| Operation in footer | Status |
| Hints in footer | Discoverability |
| Sidebar (when activated) | Orientation |
| Tool intent labels | Progress communication |
| Tool file lists | Scope communication |
| Plain English permission descriptions | Informed consent |
| Narrative progress labels | Intent communication |
| Progress bars (single line) | Visual progress |
| Error messages with context | Recovery guidance |
| Markdown in assistant messages | Rich content |

---

# 11. Engineering Implementation Contract

## 11.1 Scope

This specification governs:
- All TUI screens (33 screens)
- All layout components (header, footer, sidebar, modals)
- All conversation rendering (messages, narratives, tool summaries)
- All navigation behavior (back, forward, modals, focus)
- All keyboard and mouse interactions
- All responsive behavior (40-120+ cols)
- All empty states, error states, loading states

## 11.2 Implementation Order

| Phase | Scope | Risk | Effort | Dependencies |
|-------|-------|------|--------|-------------|
| 1 | Chrome simplification (header, footer, home) | Low | 1 week | None |
| 2 | Sidebar redesign (modes, content rules) | Medium | 1 week | Phase 1 |
| 3 | Tool grouping (intent classifier, group buffer) | High | 2 weeks | Phase 1 |
| 4 | Permission modal (plain English, consequences) | Medium | 1 week | Phase 1 |
| 5 | Execute screen (activity labels, task simplification) | Low | 3 days | Phase 3 |
| 6 | Thinking display (intent-based labels) | Low | 2 days | Phase 1 |
| 7 | Discuss screen cleanup (remove card, simplify) | Low | 1 day | Phase 1 |
| 8 | Empty states (all screens) | Low | 3 days | All phases |
| 9 | Accessibility audit (contrast, widths, keyboard) | Medium | 3 days | All phases |

**Total estimated effort:** 7-8 weeks for a single engineer.

## 11.3 File Ownership

| File | Phase | Owner | Notes |
|------|-------|-------|-------|
| `layout/page.go` | 1 | Layout | Header + footer rendering |
| `layout/responsive.go` | 1 | Layout | Breakpoint definitions |
| `sidebar_model.go` | 2 | Sidebar | Mode switching, content |
| `components/toolgroup.go` | 3 | Components | NEW FILE — tool grouping |
| `components/message.go` | 3 | Components | Tool group integration |
| `components/toolcard.go` | 3 | Components | Standalone tool display |
| `app_view.go` | 1, 4 | App | Permission modal, chrome |
| `repl_footer.go` | 1, 6 | REPL | Status bar, thinking |
| `repl_view.go` | 3 | REPL | Tool group rendering |
| `execute_model.go` | 5 | Execute | Activity labels |
| `discuss_model.go` | 7 | Discuss | Remove card |
| `plan_view.go` | 1 | Plan | Remove card border |
| `ship_model.go` | 1 | Ship | Remove card wrapper |
| `home_view.go` | 1 | Home | Remove decorations |
| `components/permission.go` | 4 | Components | NEW FILE — descriptions |

## 11.4 Testing Requirements

### Visual Regression
- Every screen must have a screenshot test at 80 cols × 24 rows
- Every screen must have a screenshot test at 120 cols × 40 rows
- Screenshots must be compared against approved baselines
- Any visual change requires design review

### Functional Testing
- Every keyboard shortcut must have a unit test
- Every navigation path must have an integration test
- Every screen transition must be tested
- Every modal open/close must be tested
- Every error state must be tested

### Accessibility Testing
- Contrast ratios verified for all text elements
- All interactive elements keyboard-accessible
- No information conveyed by color alone
- Screen reader compatibility where applicable

### Responsive Testing
- Every screen tested at 40, 60, 80, 100, 120, 160 columns
- Every screen tested at 10, 15, 24, 30, 50 rows
- Sidebar overlay tested at 60, 70, 79 columns

## 11.5 Performance Requirements

| Metric | Requirement |
|--------|-------------|
| Frame render time | <16ms (60fps) |
| Tool group classification | <1ms per tool call |
| Sidebar mode switch | <50ms |
| Screen transition | <100ms |
| Modal open/close | <50ms |
| Narrative flush | <5ms |
| Message render (cached) | <1ms |
| Message render (uncached) | <50ms |

## 11.6 Backward Compatibility

- All existing keyboard shortcuts must continue to work
- All existing slash commands must continue to work
- All existing configuration options must continue to work
- Session files from v1.6.x must be loadable
- No breaking changes to the provider interface

## 11.7 Design Review Process

1. Engineer implements phase according to this specification
2. Engineer runs visual regression tests
3. Engineer runs functional tests
4. Engineer runs accessibility tests
5. Engineer requests design review
6. Designer reviews against this specification
7. Designer approves or requests changes
8. Changes are implemented and re-reviewed
9. Final approval → merge

## 11.8 Specification Amendments

This specification may be amended by:
1. Design team proposal
2. Engineering feasibility analysis
3. User testing feedback
4. Accessibility audit findings

All amendments must:
- Be documented with rationale
- Be reviewed by both design and engineering
- Update this document with change log
- Not break backward compatibility without major version bump

---

# Appendix A: Glossary

| Term | Definition |
|------|-----------|
| Chrome | Non-content UI elements (header, footer, sidebar) |
| Narrative | Human-readable progress description |
| Intent | What M31A is trying to accomplish (not how) |
| Tool Group | Multiple same-intent tool calls rendered as one block |
| Progressive Disclosure | Showing essentials by default, details on demand |
| Layer | One of the four UI layers (Conversation, Narrative, Context, System) |
| Focused Mode | Sidebar showing all sections (opt-in) |
| Overlay | Modal or sidebar rendered on top of dimmed content |
| Viewport | Scrollable content area |
| Breakpoint | Terminal width threshold that changes layout |

---

# Appendix B: Change Log

| Version | Date | Change |
|---------|------|--------|
| 1.6.2 | 2026-07-01 | Design Freeze — initial specification |

---

# Appendix C: Approval

| Role | Name | Date | Signature |
|------|------|------|-----------|
| Principal Designer | | | |
| Lead Engineer | | | |
| Product Owner | | | |

---

*This document is the definitive UX architecture specification for M31A v1.6.2. All TUI implementation must conform to this specification. Amendments require formal review and approval.*
