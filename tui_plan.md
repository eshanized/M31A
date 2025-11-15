# M31A TUI Redesign Plan — OpenCode-Inspired UI/UX Overhaul

> **Phase:** UI Polish & Redesign
> **Status:** Planning
> **Goal:** Transform the M31A terminal UI from a functional-but-plain design to a polished, modern aesthetic matching opencode's visual quality.

---

## 1. Current State Assessment

### What Exists

M31A has a complete Bubble Tea TUI with:

- **15+ screens**: REPL, Settings, ModelSelector, Plan, Execute, Verify, Ship, Resume, GoalInput, FirstRun, Ledger, Rollback, Metrics, Discuss, Diff
- **10+ components**: ToolCard, ThinkingBlock, MessageRenderer, ProgressBar, Sparkline, Badge, PermissionModal, QuestionModal, Starfield, FilterChips, Logo, MetricCard
- **Theme system**: Dark/Light/Auto modes with ~40 color/border/style fields
- **REPL layout**: viewport → half-block separator → metadata row → textarea → bottom border → status bar
- **Header**: brand + version + phase breadcrumb + model badge + health + context meter
- **Status bar**: cwd + git branch + keyboard hints + token usage + cost
- **Sidebar**: git status with file list
- **Message rendering**: Glamour Markdown + split-border gutters with `┃` character

### Pain Points

| Issue | Severity | Location |
|-------|----------|----------|
| Purple brand color (#7C3AED) is dated, clashes with modern aesthetics | High | `theme/colors.go` |
| Header is overcrowded — brand, breadcrumb, model, health, context all in 1 line | High | `header.go` |
| Plain divider lines (`strings.Repeat("─", w)`) look cheap | Medium | All screens |
| Screens have inconsistent visual hierarchy | High | `plan_model.go`, `execute_model.go`, `verify.go`, `ship_model.go` |
| No shadow/depth effects on modals | Medium | `app_view.go` |
| Tool cards use heavy double-borders (`╔═╗`) — too much visual weight | Medium | `components/toolcard.go` |
| Sidebar is a flat column with no section styling | Low | `sidebar.go` |
| Status bar is plain text with no terminal chrome | Medium | `statusbar.go` |
| Settings screen uses plain text for tabs | Low | `settings_model.go` |
| No keyboard shortcut hints shown in header | Medium | `header.go` |
| Welcome screen logo is simple ASCII art | Low | `components/logo.go` |
| Help screen is plain text | Medium | `help.go` |
| Toast notifications are plain text below main content | Low | `app_view.go` |
| No compact mode for dense terminals | High | All screens |
| Transition animations are non-existent | Low | All screens |

---

## 2. Target Design: OpenCode-Inspired

### 2.1 Design Principles

1. **Minimal chrome**: Every pixel serves content. Header is 1 line, no wasted borders.
2. **Color as information**: Use color sparingly — brand for highlights, semantic colors for state only.
3. **Consistent spacing**: 2px padding, 1px border radii (via unicode chars), 1-line separators.
4. **Clear hierarchy**: Content > chrome. Tool output gets visual priority over UI chrome.
5. **Polished details**: Rounded borders, proper alignment, truncation with ellipsis, semantic spacing.
6. **Context-aware**: Show relevant info only when needed (phase badges during workflows, git stats when dirty).

### 2.2 Color Palette Migration

```
Current (Purple)          →  Target (Warm-amber / Modern)
─────────────────────────────────────────────────────────
Brand: #7C3AED (purple)   →  #D77757 (warm amber / brand)
Thinking: #8B5CF6         →  #8AB4F8 (cool blue)
Success: #10B981 (green)  →  #81C995 (soft green)
Error: #EF4444 (red)      →  #F28B82 (soft red)
Warning: #F59E0B          →  #FDD663 (warm yellow)
Background: #0F0F1A       →  #0D0D0D (true dark)
Surface: #1E1E2E          →  #1A1A1A (near-background)
Text: #E2E8F0             →  #E2E8F0 (keep — works well)
TextMuted: #475569         →  #9AA0A6 (better contrast)
```

### 2.3 Typography & Borders

| Element | Current | Target |
|---------|---------|--------|
| Message bubble border | Rounded with gutter | Rounded, right-aligned for user, left-aligned for assistant |
| Tool card border | Double border `╔═╗` | Rounded border `╭─╮` (lighter weight) |
| Modal border | Double border | Rounded border + shadow effect |
| Divider | `strings.Repeat("─", w)` | `▁` half-block (1px tall) |
| Input separator | Half-block `▁` | Keep half-block (opencode style) |
| Separator between turns | `┤ 15:04 ├───────` | Keep — this is good |
| Gutter character | `┃` (thick) | Keep `┃` (opencode style) |

---

## 3. Implementation Plan

### Wave 1: Theme & Color System (tui_plan — Phase 0)

#### W1-T1: Migrate color palette

**Files:** `internal/tui/theme/colors.go`

- Change brand from `#7C3AED` to `#D77757` in both Dark() and Light()
- Update Thinking from `#8B5CF6` to `#8AB4F8`
- Update Success from `#10B981` to `#81C995`
- Update Error from `#EF4444` to `#F28B82`
- Update Warning from `#F59E0B` to `#FDD663`
- Update Background from `#0F0F1A` to `#0D0D0D`
- Update Surface from `#1E1E2E` to `#1A1A1A`
- Update TextMuted from `#475569` to `#9AA0A6`
- Recalculate all computed styles in `applyThemeStyles()`
- Light mode: Background `#FFFFFF`, Surface `#F8F9FA`, Brand same `#D77757`

**Acceptance:** Theme compiles, header/status bar/text all use new colors. No raw hex strings remain in rendering code.

#### W1-T2: Add missing theme properties

**Files:** `internal/tui/theme/theme.go`

Add to Theme struct:
- `DividerChar string` — unicode divider character (default `"─"`)
- `HeaderHeight int` — for consistent layout calculations
- `ShadowColor lipgloss.Color` — for modal drop shadows
- `CompactMode bool` — global compact mode flag
- `SelectionBg lipgloss.Color` — highlight/selection background
- `CardPadding int` — default padding for all cards

Initialize in Dark()/Light() with sensible defaults.

**Acceptance:** New fields exist, default to reasonable values, no compile errors.

#### W1-T3: Create border constants

**Files:** `internal/tui/theme/theme.go`

Add border style constants:
```go
// Standard borders used across the TUI
var (
    NormalBorder = lipgloss.RoundedBorder()  // ╭─╮ for all cards
    ThinBorder   = lipgloss.Border{           // ┌─┐ for compact cards
        Top: "─", Bottom: "─", Left: "│", Right: "│",
        TopLeft: "┌", TopRight: "┐", BottomLeft: "└", BottomRight: "┘",
    }
    DoubleBorder = lipgloss.DoubleBorder()     // ╔═╗ only for important modals
)
```

Standardize: tool cards → ThinBorder, modals → NormalBorder, important modals → DoubleBorder.

---

### Wave 2: Header Redesign (tui_plan — Phase 1)

#### W2-T1: Simplify header layout

**Files:** `internal/tui/header.go`

Current: Left(brand+version) + Center(phase breadcrumb) + Right(model badge+health+context)
Target: Left(brand) + Center(phase badge, shown only during workflow) + Right(model badge+context)

Specific changes:
1. **Left zone**: `M31A` in brand bold. Version removed from header (moved to status bar).
2. **Center zone**: Phase breadcrumb shown ONLY when `phase != idle`. When idle, show git branch (`⎇ main`).
3. **Right zone**: Model badge `[OR] model-name` + context bar `5.2K/128K`. Health indicator removed (moved to status bar). Show context bar only when context > 0.
4. **Header background**: Remove full-width background. Use subtle underline `▁` as the only header separator.

**Acceptance:** Header is 1 line, shows only relevant info, no full-width background, looks clean.

#### W2-T2: Restyle header brand badge

**Files:** `internal/tui/header.go`, `internal/tui/theme/colors.go`

Replace the inline brand text with a styled badge:
- `M31A` in brand color, bold, no background
- When workflow active: show small phase badge like `[init]` `[exec]` in muted brand
- Provider badge: `[OR]` with brand border, or `[ZEN]` with info color

```go
func RenderHeader(t theme.Theme, info HeaderInfo, width int) string {
    // brand
    brand := lipgloss.NewStyle().Foreground(t.Brand).Bold(true).Render("M31A")
    
    // phase badge (if active)
    var center string
    if info.Phase != "" {
        center = RenderPhaseBadge(t, info.Phase)
    } else if info.GitBranch != "" {
        center = lipgloss.NewStyle().Foreground(t.TextMuted).Render("⎇ " + info.GitBranch)
    }
    
    // right: model + context
    parts := []string{}
    if info.ModelName != "" {
        prov := ProviderShortName(info.Provider)
        provColor := t.Brand
        if prov == "Zen" { provColor = t.Info }
        badge := lipgloss.NewStyle().
            Foreground(t.Brand).
            Render("[" + prov + "]")
        parts = append(parts, badge, info.ModelName)
    }
    if info.CtxTotal > 0 {
        parts = append(parts, renderContextMeter(info.CtxUsed, info.CtxTotal, t))
    }
    right := strings.Join(parts, " ")
    
    // layout with padding
    return layoutThreeZone(brand, center, right, width)
}
```

#### W2-T3: Add context meter component

**Files:** `internal/tui/header.go`

Replace the plain `5.2K/128K ctx` with a compact visual meter:
```
ctx [████░░░░] 42%
```

```go
func renderContextMeter(used, total int, t theme.Theme) string {
    pct := float64(used) / float64(total)
    barWidth := 8
    filled := int(math.Round(pct * float64(barWidth)))
    // color based on usage
    color := t.TextMuted
    if pct > 0.8 { color = t.Error } else if pct > 0.6 { color = t.Warning }
    bar := lipgloss.NewStyle().Foreground(color).Render(
        "[" + strings.Repeat("█", filled) + strings.Repeat("░", barWidth-filled) + "]")
    return bar + " " + lipgloss.NewStyle().Foreground(t.TextMuted).Render(fmt.Sprintf("%.0f%%", pct*100))
}
```

---

### Wave 3: Status Bar Redesign (tui_plan — Phase 2)

#### W3-T1: Restructure status bar

**Files:** `internal/tui/statusbar.go`

Current: `operation  [hints]  cost`
Target: `⌂ cwd  ⎇ branch  ·  operation  ·  [hints]  [cost/tokens]`

Changes:
1. **Left zone**: Always show cwd basename with `⌂` prefix. Git branch with `⎇` prefix if available.
2. **Center zone**: Current operation (streaming, thinking, phase name) with `⋯` spinner.
3. **Right zone**: Keyboard hints (minimal, shown only when relevant), token usage, cost.
4. **No background fill** — status bar is plain text, inherits terminal background.
5. **Separator**: Use `·` (middle dot) between zones.
6. **Leader active**: Show `ctrl+x ─ waiting ─` in brand (keep existing behavior).

**Acceptance:** Status bar shows at bottom, clean layout, no background flood, info density matches opencode.

#### W3-T2: Add compact mode

**Files:** `internal/tui/statusbar.go`, `internal/tui/repl_view.go`

When terminal width < 80 columns:
- Hide keyboard hints
- Hide cost/tokens
- Shorten cwd to last component only
- Hide git branch separator

```go
func (m *ReplModel) View() string {
    if m.width < 80 {
        info.KeyboardHints = nil
        info.ShowCost = false
    }
    // ...rest of view...
}
```

---

### Wave 4: REPL Screen Polish (tui_plan — Phase 3)

#### W4-T1: Refine input area styling

**Files:** `internal/tui/repl_view.go`

Current:
```
▁▁▁▁▁▁▁▁  (half-block separator)
M31A · model [provider]  (metadata row)
[textarea]
╹▀▀▀▀▀▀  (bottom border)
```

Target (keep opencode style but refine):
```
▁▁▁▁▁▁▁▁  (half-block separator in brand — keep)
M31A · model [OR]       (metadata row — use · separator, compact)
[textarea]               (no border on textarea, inherits terminal bg)
                         (no bottom border — clean terminal edge)
status bar               (no border above status bar)
```

1. Remove bottom border (`╹▀▀▀▀` pattern) — let status bar sit flush against textarea
2. Remove border from textarea — inherit terminal background
3. Metadata row: reduce to one line, tighter spacing
4. Input separator color: brand (keep)

#### W4-T2: Refine message rendering

**Files:** `internal/tui/components/message.go`

Current layout:
```
┃ USER
┃   user message text

┃ M31A
┃   assistant response (with Glamour rendering)
┃   [tool cards]
```

Target:
```
┃ user        (lowercase, muted, no bold for user)
┃   user message

┃ M31A        (brand, bold for assistant)
┃   assistant response
┃   [tool cards — thin border]
```

Changes:
1. User label: lowercase `user`, muted color, no bold
2. Assistant label: keep `M31A` in brand, bold
3. Tool cards within messages: use ThinBorder instead of DoubleBorder
4. Add 1-line spacing between conversation turns
5. Timestamp bars: keep `┤ HH:MM ├───────` style, reduce opacity

#### W4-T3: Refine tool cards

**Files:** `internal/tui/components/toolcard.go`

Current: Heavy double-border `╔═╗` with status icon + label + info
Target: Light rounded border `╭─╮` with inline status indicator

1. Replace `doubleBorder` var with `ThinBorder` from theme (or `RoundedBorder()`)
2. Status icon: keep `✓`/`✗`/`⟳`, reduce size
3. Header: inline `toolname` badge + truncated input on same line
4. Output: no border, 2px left padding, monospace style
5. Collapsed state: single-line `✓ toolname args...  [N lines]`
6. Color mapping: keep per-tool label colors (Bash=yellow, FileRead=blue, etc.)

```go
var cardBorder = lipgloss.Border{
    Top: "─", Bottom: "─", Left: "│", Right: "│",
    TopLeft: "┌", TopRight: "┐", BottomLeft: "└", BottomRight: "┘",
}
```

#### W4-T4: Refine thinking block

**Files:** `internal/tui/components/thinking.go`

Current thinking block:
```
[▼ Thinking (1.2s)]
  thinking content...
```

Target (opencode-inspired):
```
┃ ╭─ Thinking ──────────────────────────────╮
┃ │ thinking content in italic blue          │
┃ ╰─ 1.2s ──────────────────────────────────╯
```

Implement as a collapsible panel with thin border header/footer. Use `Thinking` color (`#8AB4F8`) for the border and text. Duration shown in footer right-aligned.

#### W4-T5: Welcome screen refresh

**Files:** `internal/tui/repl_welcome.go`, `internal/tui/components/logo.go`

Current: ASCII logo + provider card + keyboard hints
Target: Minimal welcome with ASCII logo (keep) + provider badge + 2-line hints

1. Keep the ASCII logo but render it smaller (or remove and use styled text)
2. Provider card: restyle to match new theme (brand border, clean layout)
3. Remove starfield decorative row (wastes vertical space)
4. Keyboard hints: show as muted single-line, not as pills

---

### Wave 5: Screen-by-Screen Polish (tui_plan — Phase 4)

#### W5-T1: Plan screen

**Files:** `internal/tui/plan_model.go`

Current: title + cost + divider + viewport + divider + footer
Target: header + task list with visual grouping by wave

1. Header: `📋 Plan · N tasks · ~$0.0042 · model [provider]` in one line
2. Task list: group by `Wave N` with section headers
3. Each task: `[○] action — description`
4. Depends-on shown as `⇢ task-3` if applicable
5. Footer: `↵ approve  j/k scroll  q back  o optimize`
6. Color-code waves: Wave 1 brand, Wave 2 secondary, Wave 3 muted

```
📋 Plan · 4 tasks · ~$0.0042 · claude-sonnet [OR]

Wave 1 — Foundation
  ○ task-1: Set up database schema
  ○ task-2: Create API endpoints ⇢ task-1
Wave 2 — UI
  ○ task-3: Build React components ⇢ task-1
  ○ task-4: Wire up data fetching ⇢ task-3

↵ approve  j/k scroll  q back  o optimize
```

#### W5-T2: Execute screen

**Files:** `internal/tui/execute_model.go`

Current: `⚡ Execute — 2/4 tasks` + flat task list
Target: progress bar + grouped task list + live tool output region

1. Header: `⚡ Execute · 2/4 tasks · [████░░░░] 50% · 12s elapsed`
2. Task list with visual states: `✓`, `▸` (running), `○` (pending), `✗` (failed)
3. Running task: show live tool output in an inline scrollable region below the task
4. Paused state: overlay `⏸ PAUSED` in warning color, dim task list
5. Footer: `p pause  j/k scroll  q back`

#### W5-T3: Verify screen

**Files:** `internal/tui/verify.go`

Current: `✓ Verify — 3/4 passed` + flat result list
Target: pass/fail checklist with inline error details

1. Header: `✓ Verify · 3/4 passed` or `✗ Verify · 1 failed`
2. Each task: `✓ task-name` (success, green) or `✗ task-name` (fail, red)
3. Failed tasks: show 1-line detail `→ files missing`
4. Heal attempt: show `⟳ Heal attempt 1/2...` with spinner
5. Footer: `↵ continue  h heal  s skip  q back`

#### W5-T4: Ship screen

**Files:** `internal/tui/ship_model.go`

Current: `🚀 Shipped!` + plain stats
Target: summary card with branded border, compact stats grid

1. Header: brand-colored `╭─ Ship Summary ─────────────────────╮`
2. Stats in a 2-column grid:
   ```
   Tasks:     4/4 ✓       Files:   +12 ~3 -0
   Tokens:   42.1K         Cost:   $0.0042
   Duration: 3m12s         Commits: 4
   ```
3. Commit log: last 5 commits in muted style
4. Footer: `↵ start new session  q back to REPL`

#### W5-T5: Settings screen

**Files:** `internal/tui/settings_model.go`, `internal/tui/settings_tabs.go`

Current: tab bar with pill-style tabs, plain content
Target: left-nav style with active indicator, card-style content panels

1. Replace horizontal tab bar with a left sidebar (compact)
2. Active tab highlighted with brand left-border indicator `▍`
3. Content panel has thin border, consistent padding
4. Setting rows: label in muted, value in primary, aligned to columns
5. Edit mode: inline with brand-colored input border

```
▍ Provider    │  Provider Settings
  Model       │
  UI          │  Default provider: openrouter
  Keys        │  Available: openrouter, zen
  Workflow    │
  About       │  Provider Status:
              │    ✓ openrouter
              │    ✗ zen
```

#### W5-T6: Model selector screen

**Files:** `internal/tui/modelselector.go`, `internal/tui/modelselector_view.go`

Current: search bar + flat model list
Target: search bar + two-pane list (provider filter + model list) + detail pane

1. Top: search input with `⌕` prefix icon
2. Provider filter: pills above results (`All` / `OpenRouter` / `Zen`)
3. Each model row: `name [OR] 128K ctx  $2.50/$10.00/M  ⚡ reasoning`
4. Detail pane (on selection): full description, architecture, pricing breakdown
5. Footer: `↵ select  tab filter  / search  esc back`

#### W5-T7: Discuss screen

**Files:** `internal/tui/discuss.go`

Current: `Question 1 of 3` + plain question + input box
Target: progress indicator + card-style question + styled input

1. Progress: `Question 2 of 3` with dot indicators `● ● ○`
2. Question in branded card with thin border
3. Input with brand border, placeholder text
4. Timer shown only when < 30 seconds remain
5. Footer: `↵ answer  esc skip  ctrl+s skip all`

#### W5-T8: Goal input screen

**Files:** `internal/tui/goalinput.go`

Current: `Enter Goal` + description + textarea
Target: minimal goal entry with recent goals panel

1. Title: `What should M31A do?` in brand
2. Textarea: full-width, no border (terminal edge), placeholder text
3. Recent goals: slide-up panel with `ctrl+r`, show as numbered list
4. Footer: `ctrl+↵ submit  esc cancel  ctrl+r recent`

---

### Wave 6: Component Library Enhancement (tui_plan — Phase 5)

#### W6-T1: Badge component

**Files:** `internal/tui/components/badge.go`

Refine badge rendering for consistency:
```go
type BadgeType int
const (
    BadgeBrand BadgeType = iota
    BadgeSuccess
    BadgeError
    BadgeWarning
    BadgeInfo
    BadgeNeutral
)

type Badge struct {
    Text  string
    Type  BadgeType
    Theme theme.Theme
}

func (b Badge) Render() string {
    // returns: [text] with appropriate color
    // "[text]" with brackets in muted, text in type color
}
```

Use `Badge` component everywhere: phase badges, status badges, provider badges.

#### W6-T2: Section divider component

**Files:** `internal/tui/components/` — new file `divider.go`

Create a reusable section divider:
```go
type SectionDivider struct {
    Title string    // optional section title
    Width int
    Theme theme.Theme
}

func (d SectionDivider) Render() string {
    // ── title ──────────────────────────────
}
```

Replace all manual `strings.Repeat("─", w)` with this component.

#### W6-T3: Card component

**Files:** `internal/tui/components/` — new file `card.go`

```go
type Card struct {
    Title   string
    Content string
    Width   int
    Border  lipgloss.Border  // Thin, Normal, or Double
    Style   CardStyle        // Brand, Success, Error, Warning, Neutral
    Theme   theme.Theme
}

func (c Card) Render() string {
    // Renders a bordered card with header and content
}
```

Replace manual card layouts in:
- PlanModel (task list card)
- ExecuteModel (progress card)
- Permissions modal
- Question modal
- Welcome provider card

#### W6-T4: Context menu / Command palette

**Files:** `internal/tui/cmdpalette.go`

Current: centered modal with search
Target: bottom-anchored command palette (opencode `ctrl+p` style)

1. Anchor to bottom of terminal, not center
2. Add command categories with section headers
3. Show keyboard shortcut next to each command
4. Matched text highlighting in search results
5. Dark overlay behind palette (semi-transparent via lipgloss adaptation)

#### W6-T5: Toast notification system

**Files:** `internal/tui/app_view.go` (toast rendering)

Current: plain text appended below main content
Target: overlay-style toast in top-right corner

1. Position: top-right, near the header area
2. Style: rounded border, colored left border indicator
3. Auto-dismiss: fade out after 3 seconds
4. Types: success (green), error (red), warning (yellow), info (brand)
5. Max 3 visible toasts, queue overflow

```go
type Toast struct {
    Text string
    Type string       // "success", "error", "warning", "info"
    CreatedAt time.Time
}
```

---

### Wave 7: Sidebar & Diff Polish (tui_plan — Phase 6)

#### W7-T1: Sidebar refresh

**Files:** `internal/tui/sidebar.go`

Current: plain column with `◈ M31A` title + GIT section + FILES section
Target: cleaner sidebar with git graph aesthetics

1. Title: `ℹ M31A` in brand, no decorative symbols
2. Git section: show branch + remote tracking info
3. File list: show status with `●`/`+`/`−`/`?` icons, grouped by status
4. Empty state: show `✓ working tree clean` in muted green
5. Width: 28 chars default, resizable with `ctrl+[` / `ctrl+]`
6. Right border: thin vertical separator `│` in muted

#### W7-T2: Diff screen visual refresh

**Files:** `internal/tui/diff_view.go`, `internal/tui/diff_model.go`

Current: plain diff output
Target: syntax-highlighted diff with line numbers

1. Added lines: green background + `+` prefix
2. Removed lines: red background + `−` prefix
3. Context lines: dimmed, no prefix
4. File header: `─── a/file.go  +++ b/file.go` in muted
5. Hunk header: `@@ -1,5 +1,7 @@` in info color
6. Line numbers in muted right-aligned column

---

### Wave 8: Animation & Micro-interactions (tui_plan — Phase 7)

#### W8-T1: Spinner component

**Files:** `internal/tui/components/` — new or update `spinner.go`

Use `bubbles/spinner` with opencode's character set: `⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏`
Add to all loading states:
- Model fetch loading
- Stream waiting
- Tool execution
- Workflow phase transitions

#### W8-T2: Progress bar animation

**Files:** `internal/tui/components/progress.go`

When progress updates, animate the bar fill:
- Use `time.Tick` to incrementally fill over 500ms
- On task completion: flash green briefly
- On failure: flash red briefly

(Implemented via Bubble Tea tick messages — no goroutines.)

#### W8-T3: Screen transition effects

**Files:** `internal/tui/app_update.go`

Add subtle transitions between screens:
1. Fade effect: briefly dim content by overlaying a semi-transparent bg
2. Slide: new content appears with a 1-frame delay for visual separation

Keep transitions minimal (< 200ms) to avoid feeling sluggish.

---

### Wave 9: Typography & Spacing Audit (tui_plan — Phase 8)

#### W9-T1: Consistent padding audit

Audit every View() method for consistent padding:
- Left padding: 2 chars for all body content
- Section headers: 0 padding, bold
- List items: 2 chars left + 1 char gap
- Nested content: +2 chars per level
- Modals: 1, 2 padding (vertical, horizontal)

Affected files: `repl_view.go`, `plan_model.go`, `execute_model.go`, `verify.go`, `ship_model.go`, `settings_model.go`, `discuss.go`, `goalinput.go`, `modelselector_view.go`

#### W9-T2: Truncation consistency

**Files:** `internal/tui/truncate.go`

Ensure all long content truncation uses `TruncateWithEllipsis` and follows these rules:
- File paths: truncate from middle (e.g., `src/.../file.go`)
- Model names: truncate from middle
- Command output: cap at viewport width with ellipsis
- Error messages: show first 200 chars + `[...]`

---

### Wave 10: Accessibility & Terminal Compatibility (tui_plan — Phase 9)

#### W10-T1: 256-color fallback

**Files:** `internal/tui/theme/color.go`

Detect terminal color support via `TERM` env var and `lipgloss.ColorProfile()`:
- TrueColor: use hex colors (default)
- 256-color: use closest xterm-256 approximation
- 16-color: use ANSI named colors

Create a `ColorProfile` type and resolution function.

#### W10-T2: Minimum width enforcement

Add minimum terminal width checks and responsive layout:
- < 40 cols: show only REPL viewport + input (no sidebar, no header details)
- < 60 cols: compact header, no keyboard hints
- >= 80 cols: full layout with all chrome

---

## 4. Migration Strategy

### Phase Ordering

```
Wave 1: Theme & Color System    → 1 day   → theme/colors.go, theme/theme.go
Wave 2: Header Redesign         → 1 day   → header.go
Wave 3: Status Bar Redesign     → 0.5 day → statusbar.go
Wave 4: REPL Screen Polish      → 2 days  → repl_view.go, message.go, toolcard.go, thinking.go, welcome.go
Wave 5: Screen Polish           → 3 days  → plan, execute, verify, ship, settings, modelselector, discuss, goalinput
Wave 6: Component Library       → 2 days  → badge, divider, card, toast, cmdpalette
Wave 7: Sidebar & Diff          → 1 day   → sidebar.go, diff files
Wave 8: Animations              → 1 day   → spinner, progress, transitions
Wave 9: Typography & Spacing    → 1 day   → all View() methods
Wave 10: Accessibility          → 0.5 day → color fallback, min width
```

Total estimated time: ~13 days for a single developer.

### Per-File Change Summary

| File | Change Type | Description |
|------|-------------|-------------|
| `theme/colors.go` | Rewrite | Color palette migration + 256-color fallback |
| `theme/theme.go` | Extension | Add new theme properties (DividerChar, CompactMode, etc.) |
| `header.go` | Rewrite | Simplify to 1 line, remove full bg, add git branch fallback |
| `statusbar.go` | Rewrite | 3-zone layout, cwd/branch always shown, no bg fill |
| `repl_view.go` | Refine | Remove bottom border, remove textarea border, compact metadata |
| `components/message.go` | Refine | Lowercase user, thin borders for tools, spacing between turns |
| `components/toolcard.go` | Rewrite | Thin borders, inline header, cleaner output rendering |
| `components/thinking.go` | Rewrite | Panel-style with thin border header/footer |
| `components/logo.go` | Minor | ASCII art stays, version display refinement |
| `components/badge.go` | Rewrite | Full badge component with all states |
| `components/progress.go` | Extension | Animated fill, state transitions |
| `plan_model.go` | Rewrite | Grouped by wave, cleaner layout, color-coded |
| `execute_model.go` | Rewrite | Progress bar, live tool output region, pause overlay |
| `verify.go` | Rewrite | Pass/fail checklist, inline error details, heal UI |
| `ship_model.go` | Rewrite | Summary card, stats grid, compact commit log |
| `settings_model.go` | Rewrite | Left-nav tabs, card panels, column-aligned values |
| `modelselector.go` | Rewrite | Two-pane layout, detail pane, provider filter pills |
| `discuss.go` | Refine | Dot progress, card-styled question, compact timer |
| `goalinput.go` | Refine | Minimal styling, recent goals panel, no border |
| `sidebar.go` | Refine | Cleaner sections, file grouping, resize support |
| `cmdpalette.go` | Rewrite | Bottom-anchored, categories, shortcut display |
| `app_view.go` | Refine | Toast overlay, screen transition effects |
| `app_state.go` | Minor | Add new state fields (toasts, animation state) |

---

## 5. Guardrails

### What NOT to change

1. **Architecture**: Do not rewrite Bubble Tea → something else. Keep the Model/Update/View pattern.
2. **Message rendering pipeline**: Keep Glamour for Markdown. Only change presentation, not the pipeline.
3. **Streaming logic**: Do not touch `repl_stream.go` or any streaming code — purely visual changes.
4. **Key bindings**: Keep existing key bindings. Only add visual hints for them.
5. **Theme Manager API**: Keep `NewManager`, `Current()`, `Cycle()`, `Mode` types.
6. **Screen routing**: Keep `screen` enum and `renderActiveScreen()` pattern.

### Testing Requirements

After each wave:
```bash
CGO_ENABLED=0 go build -o m31a ./cmd/m31a   # Must compile
go test -race -cover ./internal/tui/...        # Must pass
```

### Rollback Plan

If a wave introduces visual regressions:
1. `git diff` to identify changed files
2. `git checkout -- <file>` to revert specific files
3. Re-apply changes with corrected values

---

## 6. Acceptance Criteria

1. [ ] Brand color migrated to `#D77757`, all screens use new palette
2. [ ] Header is clean 1-line with brand + optional phase + model + context meter
3. [ ] Status bar shows cwd, branch, operation, hints, cost in 3-zone layout
4. [ ] Tool cards use thin borders instead of double borders
5. [ ] Message rendering uses lowercase user label, consistent spacing
6. [ ] Plan screen groups tasks by wave with visual hierarchy
7. [ ] Execute screen shows progress bar and live tool output
8. [ ] Verify screen shows pass/fail checklist with inline errors
9. [ ] Ship screen shows summary card with stats grid
10. [ ] Settings screen uses left-nav tabs with content panels
11. [ ] Model selector has two-pane layout with detail view
12. [ ] Discuss screen has dot progress and card-styled questions
13. [ ] Goal input is minimal with recent goals panel
14. [ ] Badge component used consistently across all screens
15. [ ] Divider component replaces all manual `strings.Repeat("─")`
16. [ ] Toast notifications appear as overlay, not inline text
17. [ ] Command palette is bottom-anchored with categories
18. [ ] Sidebar has cleaner section layout and git graph aesthetic
19. [ ] 256-color fallback works on limited terminals
20. [ ] Responsive layout adapts to < 60 column terminals
21. [ ] `CGO_ENABLED=0 go build` succeeds
22. [ ] `go test -race -cover ./internal/tui/...` passes
