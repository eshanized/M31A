# Phase 18: Welcome Page Rebuild — Context

**Gathered:** 2026-06-04
**Status:** Ready for planning
**Source:** User request — rebuild the welcome/landing page shown in screenshot

## Phase Boundary

Complete rewrite of the M31A welcome/landing page (the initial screen shown when no provider is configured). The current implementation has:
- Pixelated ASCII art logo that's too large and doesn't render well
- Broken UI elements (horizontal lines in the screenshot)
- Poor layout and visual hierarchy
- "No provider configured" warning that needs better styling

**Goal:** Create a polished, professional welcome page that:
1. Uses a clean, modern logo design (not pixelated)
2. Has proper visual hierarchy and spacing
3. Provides clear setup guidance
4. Matches the M31A brand aesthetic (terra cotta #D77757 primary)
5. Works across different terminal sizes

## Implementation Decisions

### Logo Design
- Replace pixelated block characters with clean Unicode box-drawing or custom ASCII art
- Consider using lipgloss styling for gradient/shadow effects
- Logo should be compact (max 8-10 lines height)

### Layout Structure
- Vertical stack: Logo → Status Card → Input Area → Hints
- Center-aligned content with proper padding
- Responsive to terminal width changes

### Status Card
- Show provider configuration status clearly
- If unconfigured: prominent "Get Started" call-to-action
- If configured: show model/provider info

### Input Area
- Clean input box with brand-colored accent
- Placeholder text: "Type a message, /command, or goal..."
- Model/provider context line below input

### Visual Elements
- Use theme colors consistently (Brand #D77757, Surface #1A1A1A)
- Rounded borders for cards
- Subtle shadows/borders for depth
- Keyboard shortcut hints at bottom

## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Core Files
- `internal/tui/repl_view.go` — Current welcome page implementation (renderWelcome, renderBlockLogo, renderProviderCard)
- `internal/tui/firstrun.go` — First-run setup screen (alternative logo style)
- `internal/tui/theme/theme.go` — Theme color definitions
- `internal/tui/theme/colors.go` — Color palette

### Brand Guidelines
- Primary brand color: #D77757 (terra cotta)
- Background: #0D0D0D
- Surface: #1A1A1A
- Text Primary: #E0E0E0
- See `adrenaline/REFERENCE.md` for full brand spec

## Specific Ideas

1. **Clean Logo:** Use simple, elegant ASCII art that renders well in all terminals
2. **Provider Status Card:** Prominent card showing connection status with action buttons
3. **Quick Actions:** Show common commands (/config, /help, /settings)
4. **Session Stats:** Message count, tokens used, cost (when available)
5. **Keyboard Hints:** Ctrl+P commands, Ctrl+B sidebar, Ctrl+X leader

## Deferred Ideas

None — this phase covers the complete welcome page rebuild.

---

*Phase: 18-welcome-page-rebuild*
*Context gathered: 2026-06-04 via user request*
