# Phase 18: Welcome Page Rebuild — Research

**Researched:** 2026-06-04
**Researcher:** gsd-phase-researcher

## Technical Analysis

### Current Implementation Issues

1. **Pixelated Logo** (`repl_view.go:386-405`)
   - Uses Unicode block characters (░███) that render poorly in many terminals
   - Takes up 8 lines of height, too large for the viewport
   - No visual refinement, just raw characters

2. **Broken UI Elements** (screenshot shows horizontal lines)
   - Likely caused by lipgloss border rendering issues
   - May be theme-related (missing color definitions)
   - Need to check border style compatibility

3. **Layout Issues**
   - Content not properly centered
   - Spacing between elements inconsistent
   - Input box styling doesn't match the rest of the UI

### Recommended Approach

#### Logo Redesign Options

**Option A: Clean ASCII Art (Recommended)**
```
  __  _______  __
 /  |/  / __ \/_  /
/ /|_/ / /_/ / / /
/_/  /_/\____/ /___/
```
- Simple, clean, renders everywhere
- 4 lines height (compact)
- Easy to style with lipgloss

**Option B: Box Drawing Characters**
```
╔══════════════════════╗
║   M 3 1 A            ║
║   Terminal AI Agent   ║
╚══════════════════════╝
```
- Professional look
- Clear branding
- Works in all terminals

**Option C: Minimal Text Logo**
```
M31A
────
Terminal AI Coding Agent
```
- Ultra-clean
- Modern aesthetic
- Easy to maintain

#### Layout Structure

```
┌─────────────────────────────────────┐
│           [LOGO]                    │
│                                     │
│  ┌─────────────────────────────┐   │
│  │ ● No provider configured    │   │
│  │   Run /settings to begin    │   │
│  └─────────────────────────────┘   │
│                                     │
│  ┌─────────────────────────────┐   │
│  │ Type a message...           │   │
│  │ model · provider            │   │
│  └─────────────────────────────┘   │
│                                     │
│  ctrl+p commands  ctrl+b sidebar   │
└─────────────────────────────────────┘
```

### Files to Modify

1. **`internal/tui/repl_view.go`**
   - `renderWelcome()` — Complete rewrite
   - `renderBlockLogo()` → `renderLogo()` — New implementation
   - `renderProviderCard()` — Update styling

2. **`internal/tui/theme/theme.go`** (if needed)
   - May need to add new style constants

3. **`internal/tui/components/`** (optional)
   - Could extract reusable card components

### Testing Strategy

1. **Visual Testing**
   - Manual verification in multiple terminals (iTerm2, Alacritty, Windows Terminal, GNOME Terminal)
   - Test at different terminal sizes (80x24, 120x40, 160x50)

2. **Functional Testing**
   - Provider configured/unconfigured states
   - Theme switching (dark/light)
   - Terminal resize handling

3. **Regression Testing**
   - Ensure all existing tests pass
   - Add visual snapshot tests if feasible

### Risk Assessment

| Risk | Impact | Mitigation |
|------|--------|------------|
| Logo doesn't render in some terminals | Medium | Use only ASCII-safe characters, test in 5+ terminals |
| Layout breaks at small sizes | Low | Responsive design with minimum width check |
| Theme colors inconsistent | Low | Use existing theme variables, no hardcoded colors |

## Validation Architecture

### Dimension 1: Functional Correctness
- Welcome page renders without errors
- All interactive elements work (input, commands)
- Theme colors applied correctly

### Dimension 2: Visual Quality
- Logo is clean and professional
- Layout is balanced and centered
- Spacing is consistent

### Dimension 3: Responsiveness
- Works at 80x24 minimum
- Scales properly to larger terminals
- No content overflow or truncation

### Dimension 4: Accessibility
- Sufficient color contrast
- Readable text sizes
- Keyboard navigation works

---

*Research complete: 2026-06-04*
