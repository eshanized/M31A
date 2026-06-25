# M31A TUI Comprehensive UI Overhaul Plan

## Executive Summary

The M31A TUI has a solid foundation (Bubble Tea + Lip Gloss, 11 themes, 49 components) but many powerful features are built but not deployed. This plan addresses 10 problem areas across 5 phases, targeting the screens users see most: REPL, Sidebar, Dashboard, and modals.

---

## Phase 1: Visual Hierarchy & Depth (Foundation)

**Goal:** Establish visual depth and hierarchy across all screens using existing but unused style assets.

### 1.1 Deploy Gradient Borders to Active/Focused Elements
- **File:** `sidebar_model.go:1227-1240` — Change sidebar right border from plain `│` to `BrandGradientStyle()` when focused
- **File:** `components/card.go:140-145` — Add `CardFocused` variant using `BrandGradientBorder` instead of plain rounded border
- **File:** `layout/page.go:89-165` — Use gradient border for header bottom separator when sidebar is focused
- **Impact:** Instant visual pop on the most-viewed screen elements

### 1.2 Deploy Shadow Effects to Modals and Sidebar
- **File:** `sidebar_model.go` View method (lines 717-1241) — Wrap sidebar content in `theme.RenderWithShadow()` 
- **File:** `layout/page.go` — Apply shadow to modal overlay rendering
- **File:** `toast.go:33-83` — Add subtle shadow behind toast cards
- **Impact:** Adds depth perception, separates floating elements from content

### 1.3 Card Variant Deployment
- **File:** `dashboard_model.go:112-168` — Use `CardHeader` variant for the phase bar section, `CardElevated` for metrics
- **File:** `sidebar_model.go` — Use `CardMinimal` for the token usage section, `CardElevated` for context pressure gauge
- **File:** `settings_model.go` — Use `CardHeader` for tab sections
- **File:** `help_model.go` — Use `CardInline` for shortcut groups
- **Impact:** Creates visual hierarchy through card treatment variation

### 1.4 Focus Indicator System
- **File:** `sidebar_model.go:506-526` — When focused, highlight the currently hovered section row with `SelectionBg` background + `▸` cursor
- **File:** `sidebar_model.go:1227-1240` — Change right border to gradient + add left border accent when focused
- **File:** `components/focus.go` — Extend focus state to include background tint, not just border color
- **Impact:** Makes keyboard navigation unambiguous

**Estimated scope:** ~8 files modified, ~150 lines changed

---

## Phase 2: Sidebar Overhaul

**Goal:** Transform the sidebar from a text wall into a scannable, information-rich panel.

### 2.1 Add Section Dividers
- **File:** `sidebar_model.go:717-1241` — Insert `components.SectionDivider` between each major section (Git, Usage, Phase, Tools, Speed, Files/Todo)
- Use `Theme.DividerChar` with `Theme.TextMuted` + `Faint(true)`
- **Impact:** Immediately improves scannability of 13+ sections

### 2.2 Context Pressure Gauge Upgrade
- **File:** `sidebar_model.go:787-877` — Replace simple bar with color-coded urgency:
  - Green (< 50%): `▁▂▃` characters
  - Yellow (50-75%): `▄▅▆` characters  
  - Red (> 75%): `▇█` characters + `!!` icon
- Use `components.Sparkline` for recent pressure history
- **Impact:** Makes context limits visually salient

### 2.3 Token Burn Rate Sparkline
- **File:** `sidebar_model.go:787-877` — Replace raw `tokenBurnRate` number with mini sparkline using `components.RenderSparkline()`
- Show cost rate as secondary sparkline below token rate
- **Impact:** Turns abstract numbers into visual patterns

### 2.4 Phase Pipeline Bar
- **File:** `sidebar_model.go:880-916` — Replace `✓I ●P ○L` text with horizontal `components.SegmentedBar`:
  - Completed segments: green `✓`
  - Current segment: brand `●` with animated pulse
  - Future segments: muted `○`
  - Arrow separators between segments
- **Impact:** More immediately comprehensible than text icons

### 2.5 Tool Call Timeline Enhancement
- **File:** `sidebar_model.go:919-958` — Add duration color coding:
  - Fast (< 1s): success color
  - Medium (1-5s): warning color  
  - Slow (> 5s): error color
- Add tool name truncation with tooltip on hover
- **Impact:** Makes tool execution patterns visible at a glance

### 2.6 Sub-Agent Indicator
- **File:** `sidebar_model.go:994-1006` — Replace plain count with animated indicator:
  - Active: pulsing brand-colored badge
  - Idle: muted badge with count
- **Impact:** Shows parallel execution status

**Estimated scope:** ~2 files modified, ~200 lines changed

---

## Phase 3: Dashboard Metrics & Polish

**Goal:** Transform dashboard from a simple phase bar into a command center with live metrics.

### 3.1 Metric Card Row (Top)
- **File:** `dashboard_model.go:112-168` — Add `MetricRow` with 4 cards:
  - **Tokens Used:** `MetricCard{Value: totalTokens, Label: "Tokens", Trend: tokenTrend}`
  - **Cost:** `MetricCard{Value: cost, Label: "Cost", Trend: costTrend}`
  - **Progress:** `MetricCard{Value: progressPct, Label: "Complete", Trend: ""}`
  - **ETA:** `MetricCard{Value: eta, Label: "Remaining", Trend: ""}`
- Use `components.MetricRow()` for automatic width distribution
- **Impact:** Instant overview of session economics

### 3.2 Phase Progress Bar
- **File:** `dashboard_model.go:112-168` — Replace `WorkflowPhaseBar` with `components.AnimatedProgressBar`:
  - Fill percentage based on completed phases / total phases
  - Color transitions: brand when in-progress, success when complete
  - Label embedded in bar: "Phase 3/6 · Execute"
- **Impact:** Shows progress at a glance, not just position

### 3.3 Activity Timeline Enhancement
- **File:** `dashboard_model.go:145-152` — Enhance `TimelineView`:
  - Add timestamp to each entry
  - Color-code by entry type (tool call, message, phase change)
  - Show duration for completed entries
  - Add scroll indicator if more entries than visible
- **Impact:** Makes activity history scannable

### 3.4 Info Section Card Treatment
- **File:** `dashboard_model.go:127-143` — Wrap info items in `KeyValueGrid` component:
  - Alternating row backgrounds for readability
  - Consistent key:value styling
  - Goal text in a `CardMinimal` wrapper
- **Impact:** Professional appearance, easier to scan

### 3.5 Cost Breakdown Mini-Chart
- **File:** `dashboard_model.go` — Add `components.BarChart` showing cost by phase:
  - Horizontal bars for each phase
  - Color-coded by phase
  - Label with phase name + cost
- **Impact:** Shows where money is being spent

**Estimated scope:** ~2 files modified, ~180 lines changed

---

## Phase 4: Badge Consolidation & Component Polish

**Goal:** Unify inconsistent badge systems and polish key components.

### 4.1 Badge System Unification
- **File:** `components/badge.go` — Deprecate `SimpleBadge` and `Legacy Badge`:
  - Keep `EnhancedBadge` as the primary system
  - Add `BadgePreset` factory for backward compatibility
  - Map old `BadgePreset` types to `EnhancedBadge` variants
- **Files to update:**
  - `sidebar_model.go` — Replace `SimpleBadge` calls with `EnhancedBadge`
  - `header.go` — Replace legacy badge with `EnhancedBadge{Variant: BadgePill}`
  - `repl_footer.go` — Standardize provider badges
  - `settings_view.go` — Use consistent badge variants
- **Impact:** Visual consistency across all screens

### 4.2 Toast Notification Upgrade
- **File:** `toast.go:33-83` — Enhance toast cards:
  - Add icon prefix: ✓ for success, ✗ for error, ⚠ for warning, ℹ for info
  - Add title line (bold) + detail line (muted)
  - Add slide-in animation frames (3 frames: offscreen → partial → full)
  - Add auto-dismiss countdown indicator (fading border)
- **Impact:** More attention-grabbing, informative notifications

### 4.3 Header Context Meter Enhancement
- **File:** `layout/page.go:169-213` — Lower sparkline visibility threshold from 30% to 15%
- Add mini sparkline always-visible when context > 0
- Color transitions: brand → warning → error as usage increases
- **Impact:** Header feels more alive, context awareness improved

### 4.4 Footer Hints Optimization
- **File:** `layout/page.go:219-339` — Restructure footer for better readability:
  - Group hints by category (navigation, actions, info)
  - Use `EnhancedBadge{Variant: BadgeGhost}` for keyboard shortcuts
  - Add subtle separator between hint groups
- **Impact:** Easier to discover keyboard shortcuts

**Estimated scope:** ~8 files modified, ~200 lines changed

---

## Phase 5: Home Screen & First Impressions

**Goal:** Make the landing screen memorable and informative.

### 5.1 Logo Glow Animation
- **File:** `home_view.go:12-62` — Add pulsing glow effect:
  - Vary glow width on timer (breathing animation)
  - Use `BrandGradientStyle()` for glow gradient
  - Add subtle color shift on brand color
- **Impact:** Memorable first impression

### 5.2 Quick Stats Row
- **File:** `home_view.go:12-62` — Below tips, add metrics row:
  - Sessions completed (from ledger)
  - Total tokens used
  - Total cost
  - Last session timestamp
- Use `components.MetricRow` with compact layout
- **Impact:** Shows usage history at a glance

### 5.3 Recent Session Cards
- **File:** `home_view.go:12-62` — Below quick stats, show last 3 sessions:
  - `CardMinimal` for each session
  - Title: session goal (truncated)
  - Subtitle: date + cost + tokens
  - Footer: keyboard hint `[enter] resume`
- Use `components.Card` with `CardMinimal` variant
- **Impact:** Quick access to recent work

### 5.4 Tip Carousel
- **File:** `home_view.go:65-111` — Replace static tips with rotating tips:
  - Show 1 tip at a time
  - Rotate on timer or keypress
  - Include diverse tips: keyboard shortcuts, workflow modes, cost optimization
  - Add fade transition between tips
- **Impact:** Teaches users features incrementally

### 5.5 Version & Status Bar
- **File:** `home_view.go:12-62` — Add status indicators:
  - API key status (configured ✓ / missing ✗)
  - Default model name
  - Theme name
  - Terminal capabilities (TrueColor/256/16)
- Use `EnhancedBadge` for each indicator
- **Impact:** Quick configuration verification

**Estimated scope:** ~3 files modified, ~250 lines changed

---

## Implementation Order & Dependencies

```
Phase 1 (Foundation)
    ↓
Phase 2 (Sidebar) ← depends on Phase 1 for gradient borders + shadows
    ↓
Phase 3 (Dashboard) ← depends on Phase 1 for card variants
    ↓
Phase 4 (Badges) ← depends on Phase 1 for focus indicators
    ↓
Phase 5 (Home) ← depends on all previous phases
```

## Risk Assessment

| Risk | Mitigation |
|------|-----------|
| Gradient borders break on 256-color terminals | Check `ColorProfile()` before applying; fallback to solid brand color |
| Shadow rendering too expensive per frame | Cache shadow output; invalidate on theme change only |
| Card variant changes break existing layouts | Test each screen at 60/80/120 col breakpoints |
| Badge consolidation breaks backward compat | Keep deprecated functions as wrappers around EnhancedBadge |
| Home screen animations cause flicker | Use `tea.Tick` for animation; batch redraws |

## Testing Strategy

1. **Visual regression:** Capture screenshots at each phase completion
2. **Breakpoint testing:** Verify at 40, 60, 80, 120 column widths
3. **Theme testing:** Verify all 11 themes with new components
4. **Performance profiling:** Measure frame render time before/after
5. **Accessibility:** Verify contrast ratios meet WCAG AA for all text

## Success Metrics

- [ ] All gradient borders deployed (currently 0, target: sidebar + 3 modals)
- [ ] Sidebar section dividers added (currently 0, target: 8+)
- [ ] Dashboard metric cards deployed (currently 0, target: 4 cards)
- [ ] Badge system consolidated (3 → 1 primary system)
- [ ] Toast notifications enhanced (plain → icon + title + animation)
- [ ] Home screen features added (logo glow + stats + recent sessions)
- [ ] Focus indicators improved (border-only → border + background + cursor)
- [ ] No performance regression (frame time < 16ms at 60fps)
