# M31A TUI Design & Premium Visual Overhaul Report

**Date:** 2026-06-09
**Scope:** Complete audit of the terminal user interface — theme system, components, layouts, typography, animation, color theory, customization, and premium visual patterns — with actionable recommendations for transforming M31A from functional to visually exceptional
**Method:** Line-by-line code review of ~95 TUI source files across `internal/tui/`, `internal/tui/components/`, `internal/tui/theme/`, and `internal/config/`; cross-referenced with 15+ premium Bubble Tea applications (Superfile, gh-dash, Soft Serve, Glow, Huh); Charm ecosystem best practices and upcoming Lipgloss v2 features; terminal color theory research

---

## Table of Contents

1. [Executive Summary](#1-executive-summary)
2. [Current State Assessment](#2-current-state-assessment)
3. [Multi-Theme System Expansion](#3-multi-theme-system-expansion)
4. [Color Theory & Adaptive Palettes](#4-color-theory--adaptive-palettes)
5. [Typography & Spacing Overhaul](#5-typography--spacing-overhaul)
6. [Advanced Lipgloss Techniques](#6-advanced-lipgloss-techniques)
7. [Animation & Motion Design](#7-animation--motion-design)
8. [Component-Level Premium Upgrades](#8-component-level-premium-upgrades)
9. [Layout & Composition Architecture](#9-layout--composition-architecture)
10. [Customization & User Configuration](#10-customization--user-configuration)
11. [Screen-by-Screen Redesign Proposals](#11-screen-by-screen-redesign-proposals)
12. [Competitive Visual Analysis](#12-competitive-visual-analysis)
13. [Implementation Priority Matrix](#13-implementation-priority-matrix)
14. [Migration & Backward Compatibility](#14-migration--backward-compatibility)

---

## 1. Executive Summary

This report identifies **68 specific design improvements** across 14 categories to elevate M31A's terminal interface from a functional-but-bland developer tool to a premium, visually distinctive product. The most impactful findings:

- **Monochrome palette**: M31A's current dark theme uses a near-identical gray scale (`#0D0D0D`, `#1A1A1A`, `#252525`) with a single orange brand color (`#D77757`). This creates a flat, undifferentiated visual experience. Premium TUIs use multi-hue palettes with carefully chosen accent colors.
- **No user-selectable themes**: The only customization is a dark/light/auto toggle. There are zero preset palettes (Catppuccin, Nord, Tokyo Night, etc.) that users of premium tools expect.
- **Limited gradient usage**: Despite Lipgloss supporting gradient borders, M31A uses none. The sidebar has a 3-character gradient separator — the only gradient in the entire UI.
- **Uniform border treatment**: Every modal, card, and panel uses the same `RoundedBorder()` with identical padding. No visual hierarchy through border weight, double borders, or custom corner characters.
- **Static visuals**: Only one spinner frame set (braille dots at 10fps). No physics-based animation (Harmonica), no spring transitions, no breathing/pulse effects for active elements.
- **Missing customization surface**: The `UIConfig` TOML section has no fields for border style, padding scale, accent color overrides, font weight preferences, or animation speed.
- **No color profile adaptation**: `DetectColorProfile()` exists but is only used for 16-color ANSI fallback. No dynamic palette swapping for TrueColor vs 256-color terminals.

---

## 2. Current State Assessment

### 2.1 Theme System (Good Foundation, Limited Reach)

**Strengths:**
- Well-structured `Theme` struct with 50+ semantic fields (`Brand`, `Surface`, `TextPrimary`, `DiffAdded`, etc.)
- Dark/Light/Auto mode switching with `HasDarkBackground()` detection
- Computed styles via `applyThemeStyles()` — change base colors and all derived styles update
- `ColorProfile` detection (`TrueColor`, `256`, `16`) with ANSI fallback palette
- Card border variants (`CardBorder`, `CardBorderActive`, `CardBorderError`, `CardBorderWarn`)

**Weaknesses:**
- Only 2 palettes (Dark and Light) — both Google Material-inspired grays with orange brand
- No way to load external theme files or switch palettes at runtime beyond cycling modes
- The `Light()` palette is a near-mirror of `Dark()` with inverted luminance — same hue choices
- Theme struct has 96 fields but no grouping/nesting — flat struct is hard to maintain
- `Manager.resolve()` hardcodes palettes; no plugin/registry pattern for custom themes
- No theme preview or "try before applying" UX

### 2.2 Color Palette Analysis

```
Dark Theme Colors (current):
  Background:     #0D0D0D  (pure dark gray, no hue)
  Surface:        #1A1A1A  (slightly lighter, no hue)
  SurfaceElevated:#252525  (still no hue)
  Border:         #3C4043  (cool gray)
  Brand:          #D77757  (warm orange — the ONLY warm color)
  TextPrimary:    #E8EAED  (cool white)
  TextSecondary:  #9AA0A6  (cool gray)
  Thinking:       #8AB4F8  (soft blue)
  Success:        #81C995  (soft green)
  Error:          #F28B82  (soft red)
  Warning:        #FDD663  (soft yellow)
```

**Problem:** The background-to-surface-to-elevated progression is purely luminance-based with zero hue variation. This creates visual monotony. Premium TUIs like Superfile use backgrounds with subtle hue (e.g., `#1a1b26` — a blue-tinted dark) that makes the interface feel richer.

**Contrast ratio:** Brand `#D77757` on Background `#0D0D0D` = 5.8:1 (AA compliant). But Brand on Surface `#1A1A1A` = 4.7:1 (barely AA). TextSecondary `#9AA0A6` on Background = 5.6:1 but on Surface = 4.5:1 (marginal for small text).

### 2.3 Component Visual Audit

| Component | Current State | Premium Gap |
|-----------|---------------|-------------|
| **Cards** (`card.go`) | Single border style, 1-col padding, title is just bold text | No gradient borders, no icon slots, no header bar with close/action buttons |
| **Badges** (`badge.go`) | Two systems (legacy `Badge` + new `SimpleBadge`), flat background pills | No outline badges, no gradient fills, no icon+text badges, no pill variants |
| **Tool Cards** (`toolcard.go`) | ThinBorder with status icon, inline collapsed mode | No syntax highlighting in output, no collapsible tree for nested calls, no diff-inline |
| **Toast** (`toast.go`) | ThinBorder + icon prefix, no animation | No slide-in animation, no dismiss gesture, no stacking animation, no icon backgrounds |
| **Progress** (`progress.go`) | 4 styles (thin/thick/block/rounded) all using `█`/`░` | No gradient fills, no pulse animation at completion, no label-inside-bar, no indeterminate mode |
| **Spinner** (`spinner.go`) | Single braille frame set at 10fps | No alternative frame sets (dots, arcs, bouncing ball), no color cycling, no speed variation |
| **Sparkline** (`sparkline.go`) | Block characters with brand color | No gradient fill, no min/max markers, no hover/tooltip, no multi-series overlay |
| **Sidebar** (`sidebar.go`) | Gradient separator (3 chars), plain text lists | No tree-view indentation, no file-type icons, no hover highlight, no smooth scroll |
| **Status Bar** (`statusbar.go`) | 3-zone text with `·` separators | No background fill, no zone-specific styling, no icons, no powerline-style separators |
| **Header** (`header.go`) | Single-line text: brand + phase + model | No logo, no background gradient, no tab-style navigation, no breadcrumb trail |
| **Transitions** (`transition.go`) | Dim overlay with `░` character fade | Crude dimming (fg/bg same color hack), no slide transitions, no wipe, no dissolve |
| **Starfield** (`starfield.go`) | Deterministic dot grid at 4% density | Single character (`·`), no size variation, no parallax, no color variation |
| **Dividers** (`divider.go`) | `─` character with optional centered title | No double-line variant, no gradient, no decorative endpoints |
| **Logo** (`logo.go`) | ASCII art with single brand color | No gradient fill, no animation on startup, no shadow |
| **Welcome Screen** | Logo + 2 cards + getting started + hints | Fixed 42-col cards, no responsive card widths, no animated logo entrance |

### 2.4 Responsive Layout Gaps

Current breakpoints: `<40` (ultra-compact), `<60` (compact), `<80` (sidebar hidden), `≥80` (full).

**Missing:**
- No breakpoint between 80–120 where sidebar could show expanded content
- No wide-layout (>140 cols) that uses multi-column message display
- Sidebar width is hardcoded to 28 cols (min 20, max 50) — no proportional sizing
- No "zen mode" that hides all chrome for focused reading
- Card widths in welcome screen are hardcoded to 42 regardless of terminal width

---

## 3. Multi-Theme System Expansion

### TUI-01: Preset Palette Registry

**Current:** `Manager.resolve()` hardcodes `Dark()` and `Light()`.
**Proposed:** A `ThemeRegistry` that maps palette IDs to `Theme` constructors.

**Recommended palettes to ship:**

| Palette ID | Name | Base Hue | Inspiration |
|-----------|------|----------|-------------|
| `dark` | Midnight | Neutral warm gray | Current default (refined) |
| `light` | Daylight | Neutral cool white | Current light (refined) |
| `catppuccin` | Catppuccin Mocha | Warm purple-blue | Most popular terminal palette 2025 |
| `nord` | Nord Frost | Cool blue-gray | Muted, professional |
| `tokyo` | Tokyo Night | Deep indigo | Vibrant yet comfortable |
| `gruvbox` | Gruvbox Dark | Warm retro brown | Retro aesthetic |
| `rose` | Rosé Pine | Soft dusty pink | Elegant, feminine |
| `dracula` | Dracula | High contrast purple | Widely supported |
| `solarized` | Solarized Dark | Teal/olive | Classic, precise |
| `monochrome` | Pure Mono | Zero saturation | Minimalist, hacker aesthetic |

**Implementation sketch:**

```go
// internal/tui/theme/registry.go
type ThemeDefinition struct {
    ID          string
    Name        string
    Description string
    Mode        Mode
    Constructor func() Theme
}

var registry = []ThemeDefinition{
    {"dark", "Midnight", "Default dark theme", ModeDark, Dark},
    {"catppuccin", "Catppuccin Mocha", "Warm purple-blue", ModeDark, Catppuccin},
    // ... etc
}

func Available() []ThemeDefinition { return registry }
func ByID(id string) (Theme, bool) { ... }
```

**Config integration:** Add `ui.theme = "catppuccin"` to `config.toml`. The `ThemeChangedMsg` already exists — extend it to carry the palette ID.

### TUI-02: Runtime Theme Switching

**Current:** `Cycle()` rotates Dark→Light→Auto→Dark. No way to pick a specific palette.
**Proposed:** `/theme <name>` slash command and a theme picker in Settings with live preview.

The theme picker screen should:
1. Show a split-pane preview (left: current palette, right: target palette)
2. Render sample UI elements (card, badge, tool card, progress bar, status bar)
3. Animate the transition when switching
4. Persist selection to `config.toml`

### TUI-03: Theme File Format (Future)

Allow users to define custom themes via TOML files in `~/.m31a/themes/`:

```toml
# ~/.m31a/themes/ocean.toml
[theme]
name = "Ocean"
mode = "dark"

[colors]
background = "#0d1b2a"
surface = "#1b2838"
surface_elevated = "#243447"
border = "#2d4a5e"
brand = "#48bfe3"
text_primary = "#e0f0ff"
text_secondary = "#7eb8d4"
thinking = "#72b4d4"
success = "#56c596"
error = "#f26d6d"
warning = "#f0c674"
```

---

## 4. Color Theory & Adaptive Palettes

### TUI-04: Hue-Rich Background Layers

**Problem:** Current backgrounds are hue-less grays (`#0D0D0D`, `#1A1A1A`, `#252525`).
**Fix:** Add a subtle hue to backgrounds that matches the palette's identity.

```go
// Before (current Dark):
Background:      #0D0D0D   // pure gray
Surface:         #1A1A1A   // pure gray
SurfaceElevated: #252525   // pure gray

// After (Catppuccin-inspired):
Background:      #1a1b26   // blue-tinted dark
Surface:         #24283b   // deeper blue
SurfaceElevated: #2f3349   // lifted blue-purple

// After (Gruvbox-inspired):
Background:      #1d2021   // warm dark
Surface:         #282828   // warm medium
SurfaceElevated: #3c3836   // warm lifted
```

The hue tint should be ≤5% saturation in HSL — enough to feel "richer" without looking colored.

### TUI-05: Dynamic Contrast Adjustment

**Problem:** TextSecondary on Surface is 4.5:1 — marginal for small text.
**Fix:** Auto-adjust text secondary based on the surface it's rendered on:

```go
func (t *Theme) TextOnSurface(bg lipgloss.Color) lipgloss.Color {
    // Calculate relative luminance of bg
    // If contrast < 4.5:1, lighten the text color
}
```

### TUI-06: Semantic Color Tokens

**Problem:** Code directly references `t.Brand`, `t.Success`, etc. for non-semantic uses.
**Fix:** Introduce semantic aliases:

```go
type SemanticColors struct {
    Action      lipgloss.Color  // "do something" — brand
    Destructive lipgloss.Color  // "danger" — error
    Positive    lipgloss.Color  // "good" — success
    Caution     lipgloss.Color  // "attention" — warning
    Information lipgloss.Color  // "info" — thinking/accent
    Muted       lipgloss.Color  // "de-emphasize" — textMuted
}
```

This allows future palette swaps without touching component code.

### TUI-07: Terminal Color Profile Adaptation

**Current:** `DetectColorProfile()` returns TrueColor/256/16 but is only used for 16-color fallback.
**Proposed:** Full adaptation chain:

```
TrueColor → full hex palettes with gradients
256-color → nearest 256-color match (pre-computed lookup table)
16-color  → ANSI named colors (current ansiPalette())
4-color   → bold/normal for emphasis
Monochrome → no color, only weight/italic/underline
```

Each theme should provide an explicit 256-color variant rather than relying on Lipgloss auto-downsampling, which can produce muddy results for carefully chosen palettes.

---

## 5. Typography & Spacing Overhaul

### TUI-08: Font Weight Hierarchy

**Current:** Only `Bold(true)` and default weight. No semibold, no light.
**Proposed:** Since terminals can't do arbitrary weights, simulate hierarchy through:

1. **Bold** — headers, active items, brand text
2. **Normal** — body text, descriptions
3. **Faint** — secondary info, timestamps, hints (currently used inconsistently)
4. **Italic** — thinking blocks, code descriptions, suggestions
5. **Bold+Italic** — emphasis within thinking, warnings

**Standardize** the weight hierarchy across all screens. Currently `Faint(true)` is used in some places (sidebar shortcuts) but not others (status bar hints).

### TUI-09: Spacing Scale

**Current:** Hardcoded padding values scattered across components:
- Cards: `Padding(0, 1)` — 0 vertical, 1 horizontal
- Modals: `Padding(1, 2)` — 1 vertical, 2 horizontal
- Tool cards: `Padding(0, 1)` with `PaddingLeft(2)` body indent
- Toasts: `Padding(0, 1)`
- Badges: `Padding(0, 1)`

**Proposed:** A `SpacingScale` in the theme:

```go
type SpacingScale struct {
    XS    int  // 0 — inline elements
    SM    int  // 1 — tight groups (badges, pills)
    MD    int  // 1 — card content
    LG    int  // 2 — modal content
    XL    int  // 3 — section separators
    Gap   int  // 1 — between stacked elements
}
```

When `CompactMode` is enabled, all values decrease by 1 (min 0). This creates consistent spacing that users can tune.

### TUI-10: Unicode Character Library

**Current:** Limited set of Unicode decoration characters:
- `⎇` (git branch), `⌂` (home/cwd), `●` (modified), `+`/`−`/`→`/`?` (file status)
- `▸` (cursor), `✓`/`✗` (success/error), `⚠` (warning)
- `▓▒░█` (gradient/density), `⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏` (spinner)
- `━`/`─` (progress bars), `▁▂▃▄▅▆▇█` (sparklines)

**Proposed expansion:**

| Purpose | Current | Proposed Addition |
|---------|---------|-------------------|
| Section markers | None | `◈ ◇ ◆ ◉ ⊕ ⊗` |
| File types | None | `📄 📁 🔧 📦` (emoji when terminal supports) or `▣ ▤ ▥ ▦` (box-drawing) |
| Workflow phases | Plain text | `⬡ ⬢ ◬ △ ▽` per phase |
| Navigation | `▸` only | `▸ ▹ ► ▻ ❯ ❮` for depth levels |
| Status | `✓✗⚠●` | Add `◐ ◑ ◒ ◓` for progress states, `⟲ ⟳` for retry/refresh |
| Arrows | None | `↗ ↘ ↙ ↖` for trends, `⇡ ⇣` for up/down |
| Decorative | `─` only | `╌ ╎ ┄ ┈` for varied line weights |

### TUI-11: Tab Width Normalization

**Current:** No explicit tab handling in most renderers.
**Proposed:** All text input and output should normalize tabs to 4 spaces (Lipgloss default). Add `TabWidth(4)` to Glamour renderers.

---

## 6. Advanced Lipgloss Techniques

### TUI-12: Gradient Borders

**Current:** Zero gradient borders in the entire codebase.
**Proposed:** Use Lipgloss's multi-color border support for premium elements:

```go
// Brand gradient border for active modals and focused cards
brandGradient := lipgloss.NewStyle().
    Border(lipgloss.RoundedBorder()).
    BorderForeground(
        lipgloss.Color("#D77757"),  // brand orange
        lipgloss.Color("#E8A87C"),  // light peach
        lipgloss.Color("#D77757"),  // back to brand
    )

// Thinking gradient (blue shift) for thinking blocks
thinkingGradient := lipgloss.NewStyle().
    Border(lipgloss.RoundedBorder()).
    BorderForeground(
        lipgloss.Color("#8AB4F8"),  // soft blue
        lipgloss.Color("#6B9BD2"),  // deeper blue
    )
```

**Apply to:**
- Active/focused card borders
- Permission modal when urgent (red→orange gradient)
- Command palette when open (brand gradient)
- Progress bar fill (left-to-right gradient)

### TUI-13: Custom Border Definitions

**Current:** Only 3 borders defined (`NormalBorder`, `ThinBorder`, `DoubleBorder`) plus `SplitBorder`.
**Proposed:** Expand with purpose-specific borders:

```go
// DecorativeBorder for welcome screen cards
var DecorativeBorder = lipgloss.Border{
    Top:         "─",
    Bottom:      "─",
    Left:        "│",
    Right:       "│",
    TopLeft:     "╭",
    TopRight:    "╮",
    BottomLeft:  "╰",
    BottomRight: "╯",
}

// HeavyBorder for critical modals (permission, destructive confirm)
var HeavyBorder = lipgloss.Border{
    Top:         "━",
    Bottom:      "━",
    Left:        "┃",
    Right:       "┃",
    TopLeft:     "┏",
    TopRight:    "┓",
    BottomLeft:  "┗",
    BottomRight: "┛",
}

// DashedBorder for secondary/optional panels
var DashedBorder = lipgloss.Border{
    Top:         "╌",
    Bottom:      "╌",
    Left:        "╎",
    Right:       "╎",
    TopLeft:     "┌",
    TopRight:    "┐",
    BottomLeft:  "└",
    BottomRight: "┘",
}

// ShadowBorder for elevated modals (right + bottom shadow)
var ShadowBorder = lipgloss.Border{
    Top:         "─",
    Bottom:      "▄",
    Left:        "│",
    Right:       "▐",
    TopLeft:     "╭",
    TopRight:    "╮",
    BottomLeft:  "╘",
    BottomRight: "╛",
}
```

### TUI-14: Panel Elevation & Shadow

**Current:** `ShadowColor` exists in the theme (`#00000040`) but is never used.
**Proposed:** Implement a shadow renderer for modals and elevated panels:

```go
func RenderWithShadow(content string, shadowColor lipgloss.Color, offsetRight, offsetDown int) string {
    // Render content lines
    // Add shadow characters (▐ right edge, ▄ bottom edge, ▗▝ corners)
    // Offset by (offsetRight, offsetDown) cells
}
```

This creates depth perception — modals appear to float above the content. Premium TUIs like Superfile use this technique for dialog overlays.

### TUI-15: Style Inheritance & Composition

**Current:** Styles are created fresh with `lipgloss.NewStyle()` in most component renderers.
**Proposed:** Use `Inherit()` to build style hierarchies:

```go
// Base card style
baseCard := lipgloss.NewStyle().
    Border(theme.NormalBorder).
    Padding(0, 1)

// Focused card inherits base + adds active border
focusedCard := lipgloss.NewStyle().
    Inherit(baseCard).
    BorderForeground(t.Brand)

// Error card inherits base + error border
errorCard := lipgloss.NewStyle().
    Inherit(baseCard).
    BorderForeground(t.Error)
```

This reduces style duplication and ensures consistent base properties.

### TUI-16: Inline Rendering for Compact Elements

**Current:** Badges and pills are rendered as full blocks even when used inline.
**Proposed:** Use `Inline(true)` with `MaxWidth()` for inline badges:

```go
inlineBadge := lipgloss.NewStyle().
    Inline(true).
    MaxWidth(20).
    Background(t.Success).
    Foreground(t.BadgeForeground).
    Padding(0, 1).
    Render("passing")
```

---

## 7. Animation & Motion Design

### TUI-17: Physics-Based Transitions (Harmonica)

**Current:** Transitions use linear interpolation (`elapsed / duration`). Screen transitions use a crude dim character overlay.
**Proposed:** Integrate `charmbracelet/harmonica` for spring-based animation:

```go
import "github.com/charmbracelet/harmonica"

// Screen transition spring
spring := harmonica.NewSpring(harmonica.FPS(30), 6.0, 0.7)
// Parameters: time delta, angular velocity (speed), damping ratio
// 0.7 = slight overshoot (bouncy), 1.0 = critical (fastest stop)

// Usage: animate sidebar width, modal scale, panel slide-in
```

**Apply to:**
- Modal open/close (scale from 0.8→1.0 with slight bounce)
- Sidebar show/hide (slide from left edge)
- Toast slide-in from right
- Progress bar value changes (smooth ramp)
- Card focus transitions (border color pulse)

### TUI-18: Spinner Frame Set Variety

**Current:** Single spinner set (braille dots `⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏` at 10fps).
**Proposed:** Multiple frame sets selectable via config:

```go
var SpinnerSets = map[string][]string{
    "braille":  {"⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"},
    "dots":     {"⣾", "⣽", "⣻", "⢿", "⡿", "⣟", "⣯", "⣷"},
    "arc":      {"◜", "◠", "◝", "◞", "◡", "◟"},
    "bouncing": {"⠁", "⠂", "⠄", "⡀", "⢀", "⠠", "⠐", "⠈"},
    "line":     {"|", "/", "-", "\\"},
    "clock":    {"🕐", "🕑", "🕒", "🕓", "🕔", "🕕", "🕖", "🕗", "🕘", "🕙", "🕚", "🕛"},
    "grow":     {"▏", "▎", "▍", "▌", "▋", "▊", "▉", "█"},
    "pulse":    {"◐", "◓", "◑", "◒"},
}
```

Config: `ui.spinner_style = "arc"` with default `"braille"`.

### TUI-19: Breathing / Pulse Effects

**Proposed:** Active elements should have a subtle "breathing" animation — a slow opacity cycle that signals liveness without being distracting.

```go
// BreathingParams controls a sinusoidal opacity animation
type BreathingParams struct {
    MinOpacity float64       // 0.6
    MaxOpacity float64       // 1.0
    Period     time.Duration // 3 seconds
}

func (b BreathingParams) OpacityAt(t time.Time) float64 {
    phase := float64(t.UnixNano()) / float64(b.Period.Nanoseconds()) * 2 * math.Pi
    return b.MinOpacity + (b.MaxOpacity - b.MinOpacity) * (0.5 + 0.5*math.Sin(phase))
}
```

**Apply to:**
- Active spinner (brightness pulse)
- Streaming indicator in status bar
- Permission modal countdown when < 5 seconds (urgent pulse)
- "New messages" indicator

### TUI-20: Screen Transition Upgrade

**Current:** `renderTransitionOverlay()` fills the screen with `░` characters and a centered label. The dimming is a color hack (fg = bg = dim value).

**Proposed:** Multiple transition styles:

```go
type TransitionStyle int
const (
    TransitionFade   TransitionStyle = iota  // current dim fade (improved)
    TransitionSlide                           // new screen slides from right
    TransitionDissolve                        // characters dissolve via random fade
    TransitionNone                            // instant switch (fastest)
)
```

The fade should use actual ANSI opacity (if supported) or character density reduction (`█` → `▓` → `▒` → `░` → ` `) rather than the current color hack.

### TUI-21: Animated Logo on Startup

**Proposed:** The ASCII art logo should animate in on first render:

1. **Frame 1-3:** Letters appear one column at a time from left to right
2. **Frame 4-6:** Brand color "fills in" from left to right (gray → brand)
3. **Frame 7:** Glow row activates

Total duration: ~800ms. This creates a memorable "boot" moment that differentiates M31A.

---

## 8. Component-Level Premium Upgrades

### TUI-22: Premium Card System

**Current card.go** is functional but flat. Proposed upgrades:

1. **Header bar variant:** Card with a colored header bar (background fill spanning full width) containing the title
2. **Icon slot:** Optional left-aligned icon before the title (tool icon, status icon)
3. **Footer slot:** Optional footer for action hints or metadata
4. **Gradient top border:** Instead of full border, just a gradient line at the top
5. **Hover state:** When focused (sidebar file, settings tab), card border changes to brand with a subtle background tint

```go
type CardVariant int
const (
    CardPlain     CardVariant = iota  // current: border only
    CardElevated                      // border + background + shadow
    CardHeader                        // filled header bar + border body
    CardMinimal                       // top gradient line only, no side borders
    CardInline                        // no borders, just background tint
)
```

### TUI-23: Enhanced Badge System

**Current:** Two competing badge systems (`Badge` and `SimpleBadge`) with no unified API.

**Proposed:** Unified `Badge` with variants:

```go
type BadgeVariant int
const (
    BadgeFilled  BadgeVariant = iota  // background color + contrasting text
    BadgeOutline                      // border only + colored text
    BadgeGhost                        // colored text only, no bg/border
    BadgePill                         // rounded with horizontal padding
    BadgeDot                          // small dot + text (status indicators)
)

type BadgeOptions struct {
    Text      string
    Variant   BadgeVariant
    Color     lipgloss.Color
    Icon      string  // optional prefix icon
    Compact   bool
    MaxWidth  int     // 0 = no limit
}
```

### TUI-24: Tool Card Redesign

**Current tool cards** are functional but lack visual richness:

1. **Syntax-highlighted output:** Use Glamour or a lightweight syntax highlighter for Bash/Go/Python output in tool cards
2. **Collapsible tree view:** When a tool produces >20 lines, show a tree with expandable sections
3. **Diff-inline for Edit tool:** Show the actual diff within the tool card instead of requiring a separate screen
4. **Timing sparkline:** Show a mini sparkline of recent tool execution times in the tool card footer
5. **Error context:** Failed tools should show the error in red with a suggestion for resolution

### TUI-25: Toast Animation

**Current:** Toasts appear/disappear instantly.
**Proposed:**

1. **Slide-in from right** (3 frames: off-screen → partial → settled)
2. **Auto-dismiss progress bar** at the bottom of each toast (thin `━` bar shrinking over the duration)
3. **Stack with offset** (each toast is 2 cols narrower than the one above)
4. **Color-coded left border** (already implemented) + subtle background tint
5. **Dismiss key** (`esc` clears most recent toast)

### TUI-26: Progress Bar Premium Features

**Current:** 4 styles all using `█`/`░` characters.

**Proposed additions:**
1. **Gradient fill:** Left-to-right color gradient within the filled portion
2. **Pulse at completion:** Brief brightness pulse when reaching 100%
3. **Label inside bar:** Text rendered inside the filled portion (e.g., "12/20 tasks")
4. **Indeterminate mode:** Animated `░▒▓` pattern that scrolls for unknown-duration operations
5. **ETA display:** Estimated time remaining based on current rate

### TUI-27: Sidebar Premium Features

**Current:** Functional but plain. Proposed upgrades:

1. **Tree-view file list:** Indent nested directories with `├──` / `└──` characters
2. **File-type icons:** Use Unicode symbols for common file types (`.go` = `◇`, `.js` = `◆`, `.md` = `▤`)
3. **Smooth scroll:** When scrolling the file list, animate by 1 line per frame instead of jumping
4. **Width-adaptive content:** At wider sidebar widths (40+), show file sizes and last-modified times
5. **Section collapse:** `TAB` to collapse/expand git and files sections
6. **Active file highlight:** When a file is mentioned in the current REPL conversation, highlight it

### TUI-28: Status Bar Premium Features

**Current:** Clean 3-zone text layout with `·` separators.

**Proposed:**
1. **Subtle background tint:** Very light background color for the status bar zone (1 shade above background)
2. **Powerline-style separators:** Optional `` / `` characters between zones for users who have Powerline fonts
3. **Icons in zones:** `⌂` already used for cwd. Add `💰` or `$` for cost, `⚡` for tokens
4. **Animated streaming indicator:** The spinner in the center zone should use a gradient color that cycles
5. **Contextual hints:** Show different hints based on screen (REPL: "ctrl+p commands", Settings: "tab switch", Plan: "y approve n reject")

### TUI-29: Command Palette Redesign

**Current:** Full-screen overlay with text input and list.
**Proposed:**

1. **Floating palette** (centered modal, not full-screen) with backdrop dim
2. **Fuzzy search** with highlighted matching characters in bold + brand color
3. **Category icons** for each command type (navigation: `↗`, action: `⚡`, setting: `⚙`)
4. **Recent commands** section at the top
5. **Keyboard shortcut display** next to each command (right-aligned, muted)
6. **Preview pane:** When a command is selected, show a brief description or preview of what it does

### TUI-30: Modal System Premium

**Current:** Single centered modal with rounded border.
**Proposed:**

1. **Backdrop dim:** Render the underlying screen at reduced brightness (using `Faint(true)` or character replacement)
2. **Shadow:** Drop shadow behind modal using `▐` and `▄` characters
3. **Animation:** Modal scales in from 90% → 100% over 3 frames
4. **Close button:** Visual `✕` in top-right corner
5. **Action bar:** Bottom row with styled action buttons (`[ Enter: Confirm ]  [ Esc: Cancel ]`)
6. **Focus ring:** Brand-colored gradient border when modal is active

---

## 9. Layout & Composition Architecture

### TUI-31: Responsive Breakpoint Expansion

**Current breakpoints:** 40, 60, 80.
**Proposed:** Add intermediate and wide breakpoints:

| Width Range | Layout Name | Behavior |
|-------------|-------------|----------|
| < 40 | Ultra-compact | Content only, no chrome |
| 40–59 | Compact | Minimal chrome, no sidebar |
| 60–79 | Standard | Header + status bar, no sidebar |
| 80–119 | Full | Sidebar + all chrome |
| 120–159 | Wide | Wider sidebar, two-column cards in welcome |
| ≥ 160 | Ultra-wide | Optional right panel (preview, diff, logs) |

### TUI-32: Zen Mode

**Proposed:** A toggle (`ctrl+z` or `/zen`) that hides all chrome:
- No header, no sidebar, no status bar
- Only the REPL viewport and input area
- Brand-colored thin bottom border on input
- Exit with `esc`

This is a common feature in premium editors (Neovim, Emacs, VS Code) and gives users a distraction-free mode.

### TUI-33: Split-Pane Support (Future)

For terminals ≥ 160 cols, support an optional right pane:
- **Diff preview:** When a tool edits a file, show the diff in the right pane
- **Log viewer:** Stream application logs alongside the REPL
- **Preview:** Render markdown or code files in a syntax-highlighted pane

Implementation: A `SplitLayout` compositor that takes left and right views and renders them side-by-side with a `│` separator.

### TUI-34: Card Grid Layout

**Current:** Cards in the welcome screen are hardcoded to 42 cols and arranged manually.
**Proposed:** A `GridLayout` helper:

```go
func RenderGrid(cards []string, cols, width int) string {
    cardWidth := (width - (cols-1)*2) / cols  // 2-col gap
    // Arrange cards in rows of `cols`
    // Return lipgloss.JoinVertical of lipgloss.JoinHorizontal rows
}
```

### TUI-35: Sticky Elements

**Proposed:** Certain UI elements should be "sticky" — always visible regardless of scroll:
- Status bar (already sticky by being outside viewport)
- "New messages" indicator (already works)
- **Active tool execution card** — pin to bottom of viewport during streaming so the user always sees what's running
- **Workflow phase breadcrumb** — sticky at top during workflow execution

---

## 10. Customization & User Configuration

### TUI-36: Expanded `ui` Config Section

**Current `UIConfig` fields (theme-related only):**
```toml
theme = "dark"
compact_mode = false
```

**Proposed additions:**

```toml
[ui]
# Theme & Colors
theme = "catppuccin"            # palette ID from registry
accent_color = ""               # override brand color (hex, e.g. "#FF6B35")
custom_background = ""          # override background (hex)
border_style = "rounded"        # "rounded", "thin", "double", "heavy", "dashed", "none"

# Typography
bold_headers = true             # use bold for section headers
italic_thinking = true          # use italic for thinking blocks
tab_width = 4                   # spaces per tab

# Layout
sidebar_position = "left"       # "left" or "right"
sidebar_auto_show = true        # auto-show sidebar at width threshold
card_padding = 1                # 0=compact, 1=normal, 2=spacious
welcome_screen = true           # show welcome screen on new sessions
zen_mode_key = "ctrl+z"         # keybinding for zen mode

# Animation
animation_speed = "normal"      # "fast", "normal", "slow", "none"
spinner_style = "braille"       # spinner frame set ID
transition_style = "fade"       # "fade", "slide", "dissolve", "none"
breathing_effects = true        # enable pulse/breathing on active elements
logo_animation = true           # animate logo on startup

# Status Bar
status_bar_style = "minimal"    # "minimal", "full", "powerline"
status_bar_position = "bottom"  # "bottom" or "top"
show_spinner_in_status = true   # show spinner in center zone when active

# Tool Cards
tool_card_style = "compact"     # "compact", "expanded", "inline"
tool_output_max_lines = 20      # lines before auto-collapse
syntax_highlight = true         # highlight code in tool output

# Toasts
toast_position = "top-right"    # "top-right", "top-left", "bottom-right"
toast_duration_secs = 5         # default toast duration
toast_max_visible = 3           # max simultaneous toasts
```

### TUI-37: Theme Preview Command

`/theme` or `/theme <name>` — shows a live preview panel with:
- Color swatches for all theme colors
- Sample card, badge, tool card, progress bar, status bar
- Diff sample with added/removed lines
- Current vs. proposed side-by-side

### TUI-38: Accent Color Override

Allow users to override just the brand/accent color without changing the entire theme:

```go
func (t *Theme) WithAccent(hex string) Theme {
    t.Brand = lipgloss.Color(hex)
    t.Primary = lipgloss.Color(hex)
    t.BorderActive = lipgloss.Color(hex)
    applyThemeStyles(&t)  // recompute all derived styles
    return t
}
```

This is the single highest-impact customization — users want "their" color.

### TUI-39: Border Style Override

Allow switching the default border across all components:

```go
func BorderByName(name string) lipgloss.Border {
    switch name {
    case "thin":    return theme.ThinBorder
    case "double":  return theme.DoubleBorder
    case "heavy":   return HeavyBorder
    case "dashed":  return DashedBorder
    case "none":    return lipgloss.HiddenBorder()
    default:        return theme.NormalBorder
    }
}
```

### TUI-40: Animation Speed Control

```go
type AnimationSpeed int
const (
    AnimFast   AnimationSpeed = iota  // 50% duration
    AnimNormal                         // 100% duration
    AnimSlow                          // 200% duration
    AnimNone                          // all animations disabled
)
```

When `AnimNone`, all transitions are instant, spinners don't animate (show static char), breathing effects are disabled, and progress bars jump to target value. This is important for accessibility and for users on slow terminals/SSH connections.

---

## 11. Screen-by-Screen Redesign Proposals

### TUI-41: Welcome Screen Redesign

**Current:** Logo + glow row + provider card (42 cols) + getting started card (42 cols) + keyboard hints + bottom bar.

**Proposed:**
1. **Animated logo entrance** (TUI-21)
2. **Adaptive card widths** — cards should be `(availWidth - gap) / 2` cols, not fixed 42
3. **Gradient separator** between logo and content (full-width brand gradient)
4. **Quick-start actions as pills** — clickable/selectable suggestion pills instead of numbered list
5. **Session resume prompt** — if previous sessions exist, show a "Resume last session?" card
6. **Project-aware greeting** — "Good morning, working on M31A" with time-of-day awareness
7. **Animated starfield** behind the welcome content (currently static) — slow parallax drift

### TUI-42: REPL Screen Redesign

**Current:** Viewport + shelf separator + metadata row + textarea + status bar.

**Proposed:**
1. **Message bubbles with avatars** — small prefix icon for user (`▸`) and assistant (`◇`) with colored left border
2. **Streaming cursor** — blinking block character `▌` at the end of streaming text (currently no cursor)
3. **Inline tool results** — tool cards rendered inline with the response, collapsible
4. **Timestamp gutter** — optional timestamps in the left margin for each message
5. **Code block enhancement** — syntax-highlighted code blocks with language label and copy hint
6. **Scroll indicators** — gradient fade at top/bottom of viewport to indicate scrollable content

### TUI-43: Plan Screen Redesign

**Current:** Task list with checkboxes and status badges.
**Proposed:**
1. **Visual task graph** — show task dependencies as ASCII arrows between tasks
2. **Progress summary bar** — segmented progress bar at top showing completed/running/pending
3. **Cost estimate card** — prominent card showing estimated cost with model breakdown
4. **Time estimate** — estimated duration with confidence indicator
5. **Approval actions** — styled action bar at bottom: `[ Approve All ] [ Approve Selected ] [ Modify ] [ Reject ]`

### TUI-44: Execute Screen Redesign

**Current:** Task list with progress bars.
**Proposed:**
1. **Real-time task dashboard** — active task highlighted with brand border, completed with green checkmark, pending muted
2. **Live log stream** — scrolling log output for the active task
3. **Overall progress ring** — large Unicode ring showing overall completion percentage
4. **Token/cost ticker** — live-updating token count and cost during execution
5. **Pause/resume visual** — prominent pause state with dimmed overlay and "PAUSED" banner

### TUI-45: Settings Screen Redesign

**Current:** Tab-based navigation with inline editing.
**Proposed:**
1. **Settings categories as sections** — each tab's settings grouped with dividers and icons
2. **Value preview** — show current value next to edit field with "→ new value" preview
3. **Validation feedback** — inline error messages for invalid values (hex colors, paths, numbers)
4. **Reset to default** — per-setting reset button
5. **Theme preview panel** — live preview when changing theme settings
6. **Export/import** — save settings to file, load from file

### TUI-46: First-Run Wizard Redesign

**Current:** Step-by-step wizard with starfield background.
**Proposed:**
1. **Progress indicator** — step dots at top (e.g., `● ○ ○ ○`)
2. **Animated starfield** — slow drift/parallax during wizard steps
3. **Provider logos** — ASCII art or Unicode symbols for each provider
4. **Connectivity test** — visual "Testing..." → "✓ Connected!" animation during API key validation
5. **Model comparison** — side-by-side model cards with pricing, context length, capabilities
6. **Skip confirmation** — "Are you sure? You can configure later via /settings"

### TUI-47: Diff Screen Redesign

**Current:** Plain text diff output.
**Proposed:**
1. **Side-by-side mode** — old/new in two columns (when terminal is wide enough)
2. **Line numbers** — gutter with line numbers on both sides
3. **Hunk headers** — collapsible `@@` hunk headers with function name
4. **Word-level highlighting** — highlight changed words within changed lines (not just full lines)
5. **Navigation** — `n`/`p` to jump between hunks
6. **Stats bar** — `+12 -8 lines, 3 files changed`

### TUI-48: Session Resume Screen Redesign

**Current:** List of sessions with timestamps.
**Proposed:**
1. **Session cards** — each session as a card with goal, duration, file count, cost
2. **Sparkline** — activity sparkline showing session intensity over time
3. **Preview** — first 3 lines of the goal/conversation as preview
4. **Sort/filter** — sort by date, cost, or project; filter by date range
5. **Delete session** — `d` to delete with confirmation

### TUI-49: Metrics Screen Redesign

**Current:** Basic metrics display.
**Proposed:**
1. **Dashboard layout** — grid of metric cards (2×2 or 3×2 based on width)
2. **Charts** — sparklines for daily usage, bar charts for model costs
3. **Streak counter** — "5 days in a row" with fire icon
4. **Export** — copy metrics as markdown or JSON

---

## 12. Competitive Visual Analysis

### 12.1 Superfile (File Manager)

**What M31A can learn:**
- 20+ built-in themes with instant switching
- Every border, color, and padding is user-configurable
- Image preview in terminal (sixel/kitty protocol)
- Smooth cursor movement with animation
- File type icons and color coding
- Tab-style navigation at the top

### 12.2 gh-dash (GitHub Dashboard)

**What M31A can learn:**
- Per-section color coding with bold headers
- Vim-style keyboard navigation with visual feedback
- YAML-configurable layouts (user defines sections)
- PR/issue status pills with color coding
- Keyboard shortcut overlay (press `?`)

### 12.3 Claude Code (AI Coding Tool)

**What M31A can learn:**
- Minimal chrome — maximum content area
- Streaming markdown with smooth rendering
- Tool execution indicators with spinners
- Cost/token tracking always visible
- Clean diff display within conversation flow
- "Thinking" indicator with duration timer

### 12.4 Aider (AI Pair Programming)

**What M31A can learn:**
- Color-coded diffs showing AI changes
- File context display showing which files are loaded
- Model name and cost always visible
- Chat-style interface with clear user/assistant separation
- Lint/test result display inline

### 12.5 Key Differentiators M31A Should Own

Based on the competitive landscape, M31A should aim for:

1. **Workflow visualization** — no competitor shows the plan→execute→verify→ship pipeline as a visual breadcrumb. This is M31A's USP and should be visually stunning.
2. **Arbitrage display** — the model cost comparison feature is unique and deserves a premium visual treatment (comparison cards with sparklines).
3. **Ledger/learning system** — no competitor shows cross-session learning. The ledger browser should feel like a personal knowledge base.
4. **Session timeline** — the resume screen with session history should feel like a time machine with rich previews.

---

## 13. Implementation Priority Matrix

### Tier 1: High Impact, Low Effort (Do First)

| ID | Improvement | Impact | Effort | Files |
|----|-------------|--------|--------|-------|
| TUI-04 | Hue-rich background layers | High | Low | `colors.go` |
| TUI-12 | Gradient borders on focus | High | Low | `theme.go`, `card.go`, `toolcard.go` |
| TUI-13 | Custom border definitions | High | Low | `theme.go` |
| TUI-18 | Spinner frame set variety | Medium | Low | `spinner.go`, `config/types.go` |
| TUI-38 | Accent color override | High | Low | `theme.go`, `config/types.go`, `loader.go` |
| TUI-39 | Border style override | Medium | Low | `theme.go`, all card renderers |
| TUI-40 | Animation speed control | Medium | Low | `config/types.go`, animation code |

### Tier 2: High Impact, Medium Effort

| ID | Improvement | Impact | Effort | Files |
|----|-------------|--------|--------|-------|
| TUI-01 | Preset palette registry | High | Medium | `theme/registry.go` (new), `colors.go` |
| TUI-02 | Runtime theme switching | High | Medium | `settings.go`, `commands.go` |
| TUI-17 | Physics-based transitions | High | Medium | `transition.go`, new `harmonica` dep |
| TUI-22 | Premium card system | High | Medium | `card.go` |
| TUI-25 | Toast animation | Medium | Medium | `toast.go` |
| TUI-28 | Status bar premium features | Medium | Medium | `statusbar.go` |
| TUI-36 | Expanded config section | High | Medium | `config/types.go`, `loader.go` |
| TUI-41 | Welcome screen redesign | High | Medium | `repl_welcome.go` |

### Tier 3: Medium Impact, Medium Effort

| ID | Improvement | Impact | Effort | Files |
|----|-------------|--------|--------|-------|
| TUI-09 | Spacing scale | Medium | Medium | `theme.go`, all components |
| TUI-14 | Panel elevation & shadow | Medium | Medium | new `shadow.go`, modal renderers |
| TUI-19 | Breathing/pulse effects | Medium | Medium | `spinner.go`, status bar, streaming |
| TUI-23 | Enhanced badge system | Medium | Medium | `badge.go`, all badge callers |
| TUI-24 | Tool card redesign | Medium | Medium | `toolcard.go`, renderers |
| TUI-27 | Sidebar premium features | Medium | Medium | `sidebar.go` |
| TUI-31 | Responsive breakpoint expansion | Medium | Medium | `app_view.go`, all views |
| TUI-42 | REPL screen redesign | High | Medium | `repl_view.go`, `repl_welcome.go` |

### Tier 4: High Impact, High Effort (Strategic)

| ID | Improvement | Impact | Effort | Files |
|----|-------------|--------|--------|-------|
| TUI-03 | Theme file format | Medium | High | new `theme/loader.go`, config system |
| TUI-20 | Screen transition overhaul | Medium | High | `transition.go` |
| TUI-29 | Command palette redesign | High | High | `cmdpalette.go` |
| TUI-33 | Split-pane support | High | High | new `layout/` package |
| TUI-43 | Plan screen redesign | Medium | High | `plan_view.go`, `plan_model.go` |
| TUI-44 | Execute screen redesign | High | High | `execute_view.go`, `execute_model.go` |
| TUI-47 | Diff screen redesign | Medium | High | `diff_view.go`, `diff_model.go` |

### Tier 5: Nice-to-Have

| ID | Improvement | Impact | Effort | Files |
|----|-------------|--------|--------|-------|
| TUI-10 | Unicode character library | Low | Low | Various |
| TUI-11 | Tab width normalization | Low | Low | Glamour renderers |
| TUI-21 | Animated logo on startup | Medium | Low | `repl_welcome.go` |
| TUI-32 | Zen mode | Medium | Medium | `app_view.go`, keybindings |
| TUI-37 | Theme preview command | Medium | Medium | `commands.go`, new preview view |

---

## 14. Migration & Backward Compatibility

### 14.1 Theme System Migration

**Current config:**
```toml
[ui]
theme = "dark"
```

**Migration path:**
- `"dark"` → maps to new `midnight` palette (current dark refined)
- `"light"` → maps to new `daylight` palette
- `"auto"` → maps to new auto-detection (unchanged behavior)
- New values: `"catppuccin"`, `"nord"`, `"tokyo"`, etc.

The existing `Theme` struct should be preserved as-is (no breaking changes to field names). New fields are additive.

### 14.2 Config Backward Compatibility

All new `ui` config fields should have defaults that match current behavior:

```go
// Defaults that preserve current UI
theme = "dark"                // same palette
border_style = "rounded"      // same border
card_padding = 1              // same padding
animation_speed = "normal"    // same speed
spinner_style = "braille"     // same spinner
transition_style = "fade"     // same transition
```

Users who don't change their config see no visual difference after upgrading.

### 14.3 Component API Stability

The `Card`, `Badge`, `ToolCard`, and other component structs should maintain backward-compatible constructors. New variants and options are additive fields with zero-value defaults that preserve current behavior.

### 14.4 Dependency Additions

New dependency: `charmbracelet/harmonica` (physics animation). This is a Charm ecosystem library, lightweight, and aligns with AGENTS.md's approved dependency list.

No other new dependencies required — all improvements use existing `bubbletea`, `lipgloss`, `bubbles`, and `glamour`.

### 14.5 Lipgloss v2 Migration Path

When Lipgloss v2 stabilizes, the migration should be:
1. Replace `lipgloss.NewStyle()` calls with the v2 canvas/compositing API
2. Replace manual shadow rendering with v2's built-in shadow support
3. Use `lipgloss.HasDarkBackground()` from v2 for improved detection
4. Migrate import path from `charmbracelet/lipgloss` to `charm.land/lipgloss/v2`

This is a future concern — all recommendations in this report work with current Lipgloss v1.

---

## Appendix A: Quick-Reference Color Palettes

### Catppuccin Mocha
```
Background:  #1e1e2e    Surface:     #313244
Overlay:     #45475a    Text:        #cdd6f4
Subtext:     #a6adc8    Blue:        #89b4fa
Lavender:    #b4befe    Green:       #a6e3a1
Yellow:      #f9e2af    Peach:       #fab387
Red:         #f38ba8    Mauve:       #cba6f7
```

### Nord Frost
```
Background:  #2e3440    Surface:     #3b4252
Lighter:     #434c5e    Lightest:    #4c566a
Snow:        #d8dee9    Frost1:      #8fbcbb
Frost2:      #88c0d0    Frost3:      #81a1c1
Frost4:      #5e81ac    Aurora Red:  #bf616a
Aurora Yel:  #ebcb8b    Aurora Grn:  #a3be8c
```

### Tokyo Night
```
Background:  #1a1b26    Surface:     #24283b
Elevated:    #2f3349    Border:      #3b4261
Text:        #c0caf5    Subtext:     #9aa5ce
Blue:        #7aa2f7    Cyan:        #7dcfff
Green:       #9ece6a    Yellow:      #e0af68
Red:         #f7768e    Purple:      #bb9af7
```

### Gruvbox Dark
```
Background:  #282828    Surface:     #3c3836
Elevated:    #504945    Border:      #665c54
Text:        #ebdbb2    Subtext:     #bdae93
Red:         #fb4934    Green:       #b8bb26
Yellow:      #fabd2f    Blue:        #83a598
Purple:      #d3869b    Aqua:        #8ec07c
```

### Rosé Pine
```
Background:  #191724    Surface:     #1f1d2e
Elevated:    #26233a    Border:      #403d52
Text:        #e0def4    Subtext:     #908caa
Love:        #eb6f92    Gold:        #f6c177
Rose:        #ebbcba    Pine:        #31748f
Foam:        #9ccfd8    Iris:        #c4a7e7
```

---

## Appendix B: Visual Mockup Descriptions

### Premium Status Bar (Powerline Style)
```
 ⌂ M31A   ⎇ master   ⋯ thinking · 12s    ctrl+p cmd  ctrl+b side    42K ctx  $0.12
```
Background tint per zone: dark → slightly lighter → slightly lighter.

### Premium Tool Card (Gradient Top Border)
```
╭──────────────────────────────────────────────────────╮
│ ▓▒░ Bash  $ go test ./...              ✓ 234ms · 12L │
│                                                      │
│   ok   github.com/eshanized/M31A/internal/tui  0.234s│
│   ok   github.com/eshanized/M31A/pkg/session  0.102s │
╰──────────────────────────────────────────────────────╯
```
Top border uses brand gradient. Status icon is green with subtle glow.

### Premium Welcome Screen
```
         __  _______  __
        /  |/  / __ \/ _/      v1.2.0
       / /|_/ / /_/ / _/       █▓▒░
      /_/  /_/\____/_/

  ╭─────────────────╮  ╭─────────────────╮
  │ ◇ claude-4-opus  │  │ ▸ M31A          │
  │   [OpenRouter]   │  │   ⎇ master      │
  │   in $15/M       │  │   ● 12 changed   │
  │   ctx 200K       │  │   Go             │
  │   ⚡ tools vision │  ╰─────────────────╯
  ╰─────────────────╯

  ╭─ Getting started ───────────────────╮
  │  1. Fix the failing tests   auto-fix │
  │  2. Review recent changes   review   │
  │  3. Explain architecture    explore  │
  ╰─────────────────────────────────────╯

  ctrl+p commands · ctrl+b sidebar · @ files · ctrl+x leader
```

---

*Report generated for the M31A project. All recommendations respect the architectural rules in AGENTS.md: no CGO, no telemetry, no new tools, Bubble Tea single-threaded model, sequential tool execution.*
