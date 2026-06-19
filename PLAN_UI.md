# PLAN_UI.md — M31A Terminal UI/UX Overhaul

> **Goal:** Bring M31A's terminal UI from functional-but-basic to Node/TS-grade polish — matching the visual density, interactivity, and fluidity of tools like Claude Code, Ink, and Warp.

---

## 1. Current State Assessment

### What M31A Already Does Well
| Capability | Status | Quality |
|---|---|---|
| Theme system (10 palettes, accent colors, ANSI fallback) | Done | Excellent |
| Responsive layout (4 breakpoints) | Done | Good |
| Component library (42+ components) | Done | Good |
| Unicode glyph library (60+ constants) | Done | Excellent |
| Wave separator animation | Done | Unique |
| Tool card flash effects | Done | Good |
| Markdown rendering (Glamour + Chroma) | Done | Good |
| Streaming display | Done | Adequate |
| Permission modal | Done | Good |

### The Gap: What Node/TS-Level UIs Do That M31A Doesn't

| Dimension | M31A Current | Node/TS Standard (Ink/Claude Code) |
|---|---|---|
| **Layout engine** | Manual `JoinVertical`/`JoinHorizontal` string joins | Flexbox (Yoga engine) with flex/grow/shrink/align |
| **Component composition** | Ad-hoc structs with `Render()` methods | Nesting, props, context, lifecycle hooks |
| **Screen transitions** | Instant switch (200ms timing gap, no visual) | Slide, fade, morph animations |
| **Empty states** | Plain centered text | Illustrated, actionable, branded |
| **Overlays** | Surface background, no dimming | Dimmed backdrop + focused modal |
| **Hover/focus states** | Minimal mouse feedback | Highlighted background on hover |
| **Virtual scrolling** | Full viewport re-render on every frame | Only render visible lines |
| **Style caching** | `lipgloss.NewStyle()` on every render | Pre-computed + memoized styles |
| **Toasts** | 2-col narrowing stacking | Card stacking with depth shadows |
| **Code blocks** | Rendered with syntax highlight | Copy affordance, line highlight, run button |
| **Context pressure** | Simple `[██░░░░░░] 45%` | Gradient gauge with history sparkline |
| **Search/filter** | Filtered lists | Fuzzy match highlighting in results |
| **Accessibility** | None | High contrast mode, semantic labels |

---

## 2. Architecture: The Layout Engine Problem

### Current Approach (Manual String Assembly)
```
Every View() call:
  1. Compute widths manually per screen
  2. Build strings with JoinHorizontal/JoinVertical
  3. No concept of flex, grow, shrink, align
  4. No component tree — flat string concatenation
  5. Theme passed as parameter to every component
```

### Proposed Approach: Constraint-Based Layout System

Create `internal/tui/layout/constraints.go` — a lightweight flexbox-inspired layout system that sits on top of Lip Gloss:

```go
// Box is the fundamental layout primitive
type Box struct {
    Width     int          // fixed width (0 = auto)
    Height    int          // fixed height (0 = auto)
    Flex      int          // flex grow factor
    Direction FlexDirection // Row or Column
    Align     Align        // Start, Center, End
    Gap       int          // spacing between children
    Padding   Insets        // inner padding
    Border    lipgloss.Border
    Style     lipgloss.Style
    Children  []Renderer
}

// Renderer is anything that can render to a string
type Renderer interface {
    Render(width, height int) string
}
```

This replaces the current pattern of passing `width`/`height` ints everywhere and manually computing splits.

### Files to Create
| File | Purpose |
|---|---|
| `internal/tui/layout/constraints.go` | Box, FlexDirection, Align, Flex layout solver |
| `internal/tui/layout/solver.go` | Two-pass layout algorithm (measure + allocate) |
| `internal/tui/layout/box.go` | Box renderer with border, padding, background |
| `internal/tui/layout/stack.go` | Z-order overlay composition (for modals, toasts) |

---

## 3. Phased Improvement Plan

### Phase 1: Foundation — Layout Engine + Style Cache (Week 1-2)

**Priority: CRITICAL — Everything else depends on this**

#### 1a. Layout Engine (`internal/tui/layout/`)
Create a constraint-based layout solver:

```go
// Before (current):
sidebarW := 30
contentW := m.width - sidebarW - 1
layout := lipgloss.JoinHorizontal(lipgloss.Top,
    renderSidebar(sidebarW),
    renderContent(contentW),
)

// After (proposed):
root := layout.Box{Direction: layout.Row, Width: m.width, Height: m.height}
root.Children = []layout.Renderer{
    layout.Box{Flex: 0, Width: 30}.With(renderSidebar),
    layout.Box{Flex: 1}.With(renderContent),
}
layout := root.Solve()
```

**Files to create:**
- `internal/tui/layout/constraints.go` — Box, FlexDirection, Align, Insets types
- `internal/tui/layout/solver.go` — Two-pass layout: measure → allocate
- `internal/tui/layout/box.go` — Box.Render() with border, padding, background
- `internal/tui/layout/stack.go` — Z-order composition for overlays

#### 1b. Style Cache (`internal/tui/theme/cache.go`)
Pre-compute all styles on theme change, not on every render:

```go
// Before (current):
func renderHeader(theme theme.Theme) string {
    return lipgloss.NewStyle().
        Bold(true).
        Foreground(theme.Brand).
        Render("M31A")
}

// After (proposed):
type StyleCache struct {
    Header    lipgloss.Style
    Breadcrumb lipgloss.Style
    // ... all pre-computed styles
}

func (c *StyleCache) RenderHeader() string {
    return c.Header.Render("M31A")
}
```

**Files to create:**
- `internal/tui/theme/cache.go` — StyleCache struct, built on theme change
- `internal/tui/theme/cache_gen.go` — Auto-generated style cache from Theme struct

#### 1c. Component Base (`internal/tui/components/base.go`)
Add lifecycle and context to components:

```go
type Component interface {
    Init(ctx Context) tea.Cmd
    Update(ctx Context, msg tea.Msg) (Component, tea.Cmd)
    View(ctx Context) string
}

type Context struct {
    Width   int
    Height  int
    Theme   *theme.Theme
    Cache   *theme.StyleCache
    Focused bool
    Hovered bool
}
```

**Files to create:**
- `internal/tui/components/base.go` — Component interface, Context struct

---

### Phase 2: Visual Polish — Transitions, Overlays, Empty States (Week 2-3)

#### 2a. Screen Transitions (`internal/tui/transition.go` replace)
Add visual slide/fade transitions between screens:

```go
type Transition struct {
    Type    TransitionType // SlideLeft, SlideRight, Fade, None
    Progress float64       // 0.0 → 1.0
    Duration time.Duration
}

type TransitionType int
const (
    TransitionNone TransitionType = iota
    TransitionSlideLeft
    TransitionSlideRight
    TransitionFade
)
```

**Implementation:**
- Store previous screen's `View()` output
- Interpolate between old/new frames over 150ms
- Use `tea.Tick` for animation frames
- Slide: offset old content left, new content enters from right
- Fade: blend old/new using character density (`░▒▓█`)

**Files to modify:**
- `internal/tui/transition.go` — Replace timing-only with visual transitions
- `internal/tui/app_update.go` — Trigger transition on screen change
- `internal/tui/app_view.go` — Render transition overlay

#### 2b. Overlay Dimming (`internal/tui/layout/stack.go`)
Dim background content when modals/sidebars are open:

```go
func RenderDimmed(content string, dimFactor float64) string {
    // Apply Faint(true) + reduced contrast to all characters
    // Or overlay semi-transparent background on each line
}
```

**Files to modify:**
- `internal/tui/app_view.go` — Dim content when permission modal active
- `internal/tui/sidebar_model.go` — Dim content when sidebar overlay open
- `internal/tui/toast.go` — Add depth shadows to toast cards

#### 2c. Empty State Redesign (`internal/tui/components/empty_state.go`)
Replace plain text empty states with illustrated, branded versions:

```
Current:
    ┌─────────────────────────────┐
    │                             │
    │    No messages yet          │
    │    Type something to start  │
    │                             │
    └─────────────────────────────┘

Proposed:
    ╭─────────────────────────────────────────╮
    │                                         │
    │         ◈  M 3 1 A                     │
    │                                         │
    │   Ready to build. What should we work  │
    │   on together?                          │
    │                                         │
    │   ┌─────────────────────────────────┐   │
    │   │ ▸ Fix the login bug             │   │
    │   │ ▸ Add dark mode                 │   │
    │   │ ▸ Refactor the API layer        │   │
    │   └─────────────────────────────────┘   │
    │                                         │
    │   Type a goal or press ctrl+p for       │
    │   commands                              │
    │                                         │
    ╰─────────────────────────────────────────╯
```

**Files to create:**
- `internal/tui/components/empty_state.go` — EmptyState component with icon, title, hints, actions

**Files to modify:**
- `internal/tui/repl_welcome.go` — Use new EmptyState
- `internal/tui/ledger_view.go` — Use new EmptyState
- `internal/tui/resume_view.go` — Use new EmptyState
- `internal/tui/diff_view.go` — Use new EmptyState

#### 2d. Toast System Upgrade (`internal/tui/toast.go`)
Card-based toasts with depth shadows and better stacking:

```
Current:
    ┌─ ✓ Saved ──────────────────┐
    └────────────────────────────┘
    ┌─ ✗ Failed ─────────────────┐
    └────────────────────────────┘

Proposed:
    ╭─ ✓ Saved ───────────────────╮  ← depth 0 (front)
    ╰─────────────────────────────╯
      ╭─ ✗ Failed ─────────────────╮  ← depth 1 (behind, 2-col offset)
      ╰───────────────────────────╯
        ╭─ ● Info ──────────────────╮  ← depth 2
        ╰───────────────────────────╯
```

**Files to modify:**
- `internal/tui/toast.go` — Card-based rendering, shadow depth, slide-out animation

---

### Phase 3: Component Upgrades (Week 3-4)

#### 3a. Message Bubble Redesign (`internal/tui/components/message.go`)
More distinct user vs assistant styling:

```
Current:
┃ ● you
┃   Hello, I need help with...

┃ ◆
┃   Sure! Let me help you with that.

Proposed:
╭─ you ─────────────────────────────╮
│ Hello, I need help with...        │
╰───────────────────────────────────╯

╭─ ◆ M31 ──────────────────────────╮
│ Sure! Let me help you with that.  │
╰───────────────────────────────────╯
```

Or for compact mode, keep gutters but add background tint:
```
┃ ● you           ← user: subtle secondary background
┃   Hello...      

┃ ◆ M31           ← assistant: subtle brand background tint
┃   Sure!...      
```

**Files to modify:**
- `internal/tui/components/message.go` — Add bubble variant, background tint, role badge in header

#### 3b. Code Block Upgrade (`internal/tui/components/codeblock.go`)
Add copy affordance and line highlighting:

```
Current:
┌─ Go ──────────────────────────────┐
│ 1  func main() {                  │
│ 2      fmt.Println("hello")       │
│ 3  }                              │
└───────────────────────────────────┘

Proposed:
╭─ Go ─────────────────────── ✓ copy ╮
│  1  func main() {                   │
│  2  │   fmt.Println("hello")        │  ← highlighted line
│  3  }                               │
╰─────────────────────────────────────╯
```

**Files to modify:**
- `internal/tui/components/codeblock.go` — Line numbers, copy indicator, line highlight
- `internal/tui/components/syntax.go` — Ensure Chroma integration works with line numbers

#### 3c. Context Pressure Gauge (`internal/tui/header.go`)
Rich context meter with history:

```
Current:
ctx [████░░░░░░] 45%

Proposed:
ctx [████▓░░░░░] 45% ▁▂▃▅▆  ← inline sparkline showing recent usage
```

**Files to modify:**
- `internal/tui/header.go` — Add sparkline to context meter
- `internal/tui/layout/page.go` — Update header rendering

#### 3d. Fuzzy Search Highlighting (`internal/tui/components/search.go`)
Highlight matched characters in search results:

```
Current:
  claude-3-opus
  claude-3-sonnet
  claude-3-haiku

Proposed:
  **claude**-3-opus      ← matched chars in bold/brand color
  **claude**-3-**sonnet**
  **claude**-3-**haiku**
```

**Files to modify:**
- `internal/tui/components/search.go` — Fuzzy match highlighting
- `internal/tui/modelselector_view.go` — Use highlighted results
- `internal/tui/cmdpalette_view.go` — Use highlighted results

#### 3e. Sidebar Visual Hierarchy (`internal/tui/sidebar_model.go`)
Better section separation and visual density:

```
Current:
M31A v1.0
──────
⎇ main
●3 +2 -1 ?4
USAGE
[██░░░░░░] 45%
12.4k tokens · $0.02

Proposed:
╭─ M31A ── v1.0 ╮
│ ⎇ main         │
│ ●3 +2 -1 ?4    │
╰─────────────────╯

╭─ Context ─────────────╮
│ [████▓░░░░░] 45%      │
│ 12.4k tok · $0.02 ↑   │
╰───────────────────────╯

╭─ Files ───────────────╮
│ ▸ cmd/m31a/main.go    │
│ ▸ internal/tui/       │
│   ├ app_view.go       │
│   └ repl_model.go     │
╰───────────────────────╯
```

**Files to modify:**
- `internal/tui/sidebar_model.go` — Card-based sections, visual grouping

---

### Phase 4: Interactivity + Mouse UX (Week 4-5)

#### 4a. Hover States
Visual feedback when mouse hovers over interactive elements:

```go
// In Component interface
type Component interface {
    // ... existing methods
    HandleMouse(msg tea.MouseMsg) (Component, tea.Cmd)
}

// Hover detection
if msg.X >= item.X && msg.X <= item.X+item.Width &&
   msg.Y >= item.Y && msg.Y <= item.Y+item.Height {
    item.Hovered = true
}
```

**Files to modify:**
- `internal/tui/components/base.go` — Add HandleMouse to interface
- `internal/tui/repl_mouse.go` — Hover detection for messages, tool cards
- `internal/tui/components/toolcard.go` — Highlight on hover
- `internal/tui/components/message.go` — Highlight on hover
- `internal/tui/sidebar_model.go` — Highlight file tree items on hover

#### 4b. Focus Ring System
Visual focus indicator for keyboard navigation:

```
╭─[★]─ Settings ────────────────────╮
│                                   │
│  ╭─[▸]─ General ─────────────╮   │  ← focused section
│  │  Theme: Dark               │   │
│  │  Compact: Off              │   │
│  ╰────────────────────────────╯   │
│                                   │
│  ╭─── Provider ───────────────╮   │  ← unfocused
│  │  Default: OpenRouter        │   │
│  ╰────────────────────────────╯   │
│                                   │
╰───────────────────────────────────╯
```

**Files to create:**
- `internal/tui/components/focus.go` — Focus ring rendering utilities

**Files to modify:**
- `internal/tui/settings_view.go` — Add focus indicators
- `internal/tui/config_view.go` — Add focus indicators

#### 4c. Keyboard Shortcut discoverability
Interactive keybinding hints that show on first use:

```
When user presses ctrl+p for first time:
╭─────────────────────────────╮
│  Command Palette             │
│                              │
│  ctrl+p — this menu          │
│  ctrl+b — toggle sidebar     │
│  ctrl+c — cancel stream      │
│  esc     — close modal       │
│                              │
│  Press any key to continue   │
╰─────────────────────────────╯
```

**Files to create:**
- `internal/tui/components/shortcut_tip.go` — First-use tooltip component

---

### Phase 5: Performance — Rendering Pipeline (Week 5-6)

#### 5a. Virtual Scrolling (`internal/tui/components/virtual_viewport.go`)
Only render visible messages, not entire history:

```go
type VirtualViewport struct {
    items       []RenderableItem
    scrollTop   int
    viewportH   int
    itemHeights []int  // cached heights
    cache       map[int]string  // rendered line cache
}

func (v *VirtualViewport) View() string {
    // Only render items in [scrollTop, scrollTop + viewportH]
    visible := v.getVisibleItems()
    return renderItems(visible)
}
```

**Files to create:**
- `internal/tui/components/virtual_viewport.go` — Virtual scrolling for message lists

**Files to modify:**
- `internal/tui/repl_model.go` — Replace viewport with VirtualViewport
- `internal/tui/repl_view.go` — Update view to use virtual scrolling

#### 5b. Incremental Render Diffing (`internal/tui/render_diff.go`)
Only redraw changed portions of the screen:

```go
type RenderDiff struct {
    previousFrame string
    currentFrame  string
}

func (d *RenderDiff) Update(newFrame string) string {
    // Line-by-line diff
    // Only emit ANSI codes for changed lines
    // Use cursor positioning to jump to changed lines
    return computeDiff(d.previousFrame, newFrame)
}
```

**Files to create:**
- `internal/tui/render_diff.go` — Frame diffing for minimal terminal writes

**Files to modify:**
- `internal/tui/app_view.go` — Apply diff-based rendering

#### 5c. Streaming Render Optimization
Batch token rendering to reduce frame rate pressure:

```go
type StreamBuffer struct {
    pending   strings.Builder
    lastFlush time.Time
    minInterval time.Duration // 33ms = 30fps cap
}

func (b *StreamBuffer) Append(token string) {
    b.pending.WriteString(token)
    if time.Since(b.lastFlush) >= b.minInterval {
        b.Flush()
    }
}
```

**Files to modify:**
- `internal/tui/repl_stream.go` — Buffer streaming tokens, batch flush

---

### Phase 6: Polish + Accessibility (Week 6-7)

#### 6a. High Contrast Mode
Add a high-contrast theme for accessibility:

**Files to modify:**
- `internal/tui/theme/registry.go` — Add "high-contrast" theme
- `internal/tui/theme/colors.go` — Add HighContrast() palette

#### 6b. Screen Reader Labels
Add semantic labels for screen readers (where supported):

```go
// ANSI escape sequence for screen reader announcements
func Announce(text string) string {
    return "\x1b]1337;SetStatusMessage=" + text + "\x07"
}
```

**Files to create:**
- `internal/tui/a11y/announce.go` — Screen reader announcement utilities

#### 6c. Reduced Motion Mode
Respect user preference for reduced animations:

```go
// In config
[ui]
reduced_motion = false

// In animation code
if config.UI.ReducedMotion {
    return staticFrame // skip animation
}
return animatedFrame
```

**Files to modify:**
- `internal/tui/config/config.go` — Add `ReducedMotion` field
- `internal/tui/components/spinner.go` — Respect reduced motion
- `internal/tui/repl_view.go` — Static wave when reduced motion
- `internal/tui/transition.go` — Instant transitions when reduced motion

---

## 4. File Change Summary

### New Files (18)
| File | Phase | Purpose |
|---|---|---|
| `internal/tui/layout/constraints.go` | 1 | Box, FlexDirection, Align, Insets types |
| `internal/tui/layout/solver.go` | 1 | Two-pass layout algorithm |
| `internal/tui/layout/box.go` | 1 | Box renderer with border/padding |
| `internal/tui/layout/stack.go` | 1+2 | Z-order overlay + dimming |
| `internal/tui/theme/cache.go` | 1 | StyleCache struct |
| `internal/tui/theme/cache_gen.go` | 1 | Auto-generated cache builder |
| `internal/tui/components/base.go` | 1 | Component interface + Context |
| `internal/tui/components/empty_state.go` | 2 | Branded empty state component |
| `internal/tui/components/focus.go` | 4 | Focus ring rendering |
| `internal/tui/components/shortcut_tip.go` | 4 | First-use keyboard hint tooltip |
| `internal/tui/components/virtual_viewport.go` | 5 | Virtual scrolling viewport |
| `internal/tui/render_diff.go` | 5 | Frame diffing for minimal writes |
| `internal/tui/a11y/announce.go` | 6 | Screen reader announcements |

### Modified Files (25+)
| File | Phase | Changes |
|---|---|---|
| `internal/tui/app_view.go` | 2+5 | Dim overlays, diff-based rendering |
| `internal/tui/app_update.go` | 2 | Trigger transitions on screen change |
| `internal/tui/transition.go` | 2 | Visual slide/fade transitions |
| `internal/tui/toast.go` | 2 | Card-based toasts, depth shadows |
| `internal/tui/header.go` | 3 | Sparkline in context meter |
| `internal/tui/layout/page.go` | 3 | Updated header rendering |
| `internal/tui/repl_view.go` | 2+5 | Virtual scrolling, static wave |
| `internal/tui/repl_model.go` | 5 | VirtualViewport integration |
| `internal/tui/repl_stream.go` | 5 | Token batching |
| `internal/tui/repl_welcome.go` | 2 | Use EmptyState component |
| `internal/tui/sidebar_model.go` | 3+4 | Card sections, hover states |
| `internal/tui/components/message.go` | 3+4 | Bubble redesign, hover |
| `internal/tui/components/toolcard.go` | 3+4 | Hover highlight |
| `internal/tui/components/codeblock.go` | 3 | Copy affordance, line numbers |
| `internal/tui/components/search.go` | 3 | Fuzzy match highlighting |
| `internal/tui/components/spinner.go` | 6 | Reduced motion support |
| `internal/tui/modelselector_view.go` | 3 | Highlighted search results |
| `internal/tui/cmdpalette_view.go` | 3 | Highlighted search results |
| `internal/tui/settings_view.go` | 4 | Focus indicators |
| `internal/tui/config_view.go` | 4 | Focus indicators |
| `internal/tui/ledger_view.go` | 2 | EmptyState |
| `internal/tui/resume_view.go` | 2 | EmptyState |
| `internal/tui/diff_view.go` | 2 | EmptyState |
| `internal/tui/config/config.go` | 6 | ReducedMotion field |
| `internal/tui/theme/registry.go` | 6 | High contrast theme |
| `internal/tui/theme/colors.go` | 6 | HighContrast() palette |

---

## 5. Visual Mockups — Before vs After

### 5a. Main REPL Screen

**BEFORE:**
```
M31A │ repl ···················· claude-3-opus [OR] [ctx] 45%
─────────────────────────────────────────────────────────────
┃ ● you
┃   Fix the login bug on the dashboard

┃ ◆
┃   I'll fix the login bug. Let me start by examining the
┃   authentication code.

⟳ [ Bash ] grep -r "login" ./src/                    ✓ 45ms
┌─ FileRead ── src/auth/login.go ──────────────── ✓ 12ms ─┐
│   package auth                                           │
│   import "fmt"                                           │
│   func Login(user, pass string) error {                  │
│       // bug: nil pointer on empty pass                  │
│   }                                                      │
└──────────────────────────────────────────────────────────┘
▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁
> Type a message...                           ctrl+p cmds  $0.02
```

**AFTER:**
```
M31A │ repl ···················· claude-3-opus [OR] [ctx] 45% ▂▃▅
─────────────────────────────────────────────────────────────────
╭─ you ───────────────────────────────────────────────────────╮
│ Fix the login bug on the dashboard                          │
╰─────────────────────────────────────────────────────────────╯

╭─ ◆ M31 ────────────────────────────────────────────────────╮
│ I'll fix the login bug. Let me start by examining the       │
│ authentication code.                                         │
╰─────────────────────────────────────────────────────────────╯

╭─ Bash ── grep -r "login" ./src/ ──────────────────── ✓ 45ms ╮
╰─────────────────────────────────────────────────────────────╯

╭─ FileRead ── src/auth/login.go ─────────────────── ✓ 12ms ──╮
│  1  package auth                                             │
│  2  import "fmt"                                             │
│  3  func Login(user, pass string) error {                    │
│  4  │     // bug: nil pointer on empty pass                  │
│  5  }                                                        │
╰─────────────────────────────────────────────────────────────╯

▁▂▃▄▅▆▅▄▃▂▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁
> Type a message...                              ctrl+p  $0.02
```

### 5b. Sidebar

**BEFORE:**
```
M31A v1.0
──────
⎇ main
●3 +2 -1 ?4
USAGE
[██░░░░░░] 45%
12.4k tokens · $0.02
```

**AFTER:**
```
╭─ M31A ── v1.0 ╮
│ ⎇ main         │
│ ●3 +2 -1 ?4    │
╰─────────────────╯

╭─ Context ─────────────╮
│ [████▓░░░░░] 45%      │
│ 12.4k tok · $0.02 ↑   │
╰───────────────────────╯

╭─ Files ───────────────╮
│ ▸ cmd/m31a/main.go    │
│ ▸ internal/tui/       │
│   ├ app_view.go       │
│   └ repl_model.go     │
╰───────────────────────╯
```

### 5c. Permission Modal

**BEFORE:**
```
┌─────────────────────────────────────────┐
│  🔒 Permission Required                 │
│                                         │
│  Tool: Bash                             │
│  Risk: ████████ HIGH                    │
│                                         │
│  rm -rf /tmp/test                       │
│                                         │
│  [Y] Allow  [A] Always  [N] Deny       │
│  ████░░░░░░ 30s remaining              │
└─────────────────────────────────────────┘
```

**AFTER (with dimmed background):**
```
░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░
░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░
░░╭─────────────────────────────────────────────╮░░░░░░░░░░░░░░
░░│  🔒  Permission Required                    │░░░░░░░░░░░░░░
░░│                                             │░░░░░░░░░░░░░░
░░│  Tool:  Bash                  Risk: HIGH    │░░░░░░░░░░░░░░
░░│                                             │░░░░░░░░░░░░░░
░░│  ╭─ Command ──────────────────────────────╮ │░░░░░░░░░░░░░░
░░│  │ rm -rf /tmp/test                       │ │░░░░░░░░░░░░░░
░░│  ╰────────────────────────────────────────╯ │░░░░░░░░░░░░░░
░░│                                             │░░░░░░░░░░░░░░
░░│  [Y] Allow  [A] Always  [N] Deny  [Esc]    │░░░░░░░░░░░░░░
░░│  ████████░░░░░░░░░░░░░░░░░░░░░ 30s         │░░░░░░░░░░░░░░
░░╰─────────────────────────────────────────────╯░░░░░░░░░░░░░░
░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░
```

---

## 6. Implementation Priorities

### Must-Have (V1.1 ship blockers)
1. **Style cache** — Eliminates per-render allocation, enables all other improvements
2. **Empty state redesign** — First impression matters
3. **Toast card upgrade** — Most visible notification system
4. **Fuzzy search highlighting** — Immediate UX win for model selector + command palette
5. **Reduced motion mode** — Accessibility requirement

### Should-Have (V1.1 quality targets)
6. **Layout engine** — Foundation for future component work
7. **Screen transitions** — Perceived fluidity
8. **Overlay dimming** — Focus management for modals
9. **Code block copy affordance** — Developer productivity
10. **Context pressure sparkline** — Visual richness

### Nice-to-Have (V1.2+)
11. **Virtual scrolling** — Performance for long sessions
12. **Incremental render diffing** — Terminal write optimization
13. **Hover states** — Mouse UX polish
14. **Focus ring system** — Keyboard navigation UX
15. **High contrast theme** — Full accessibility

---

## 7. Dependencies & Risks

### Dependencies
- **Lip Gloss v2** — Currently on v1.1.0; some improvements (3D compositing, color utilities) require v2. Consider upgrade.
- **Bubble Tea** — v1.3.0 is sufficient for all planned changes.
- **Glamour** — No changes needed; already handles markdown well.

### Risks
| Risk | Mitigation |
|---|---|
| Layout engine complexity | Start with minimal Box+Flex, iterate. Don't build Yoga. |
| Performance regression from transitions | Cap at 150ms, skip when `ReducedMotion` is set |
| Breaking existing screen renders | Phase changes incrementally; one screen at a time |
| Theme compatibility with new components | Test all 10 themes against every new component |

---

## 8. Success Metrics

| Metric | Current | Target |
|---|---|---|
| Empty state visual quality | Plain text | Branded, illustrated |
| Screen transition fluidity | Instant cut | 150ms slide/fade |
| Toast notification polish | Basic cards | Shadow depth + slide-out |
| Search result clarity | Filtered list | Fuzzy-highlighted matches |
| Context meter information | Percentage bar | Bar + sparkline history |
| Code block usability | Syntax highlight only | + line numbers + copy indicator |
| Accessibility score | 0/5 | 3/5 (high contrast, reduced motion, semantic labels) |
| Style allocation per render | ~50-100 `lipgloss.NewStyle()` | 0 (all cached) |

---

*Generated: 2026-06-19 | Based on deep codebase analysis of 130+ TUI files*
