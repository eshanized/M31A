# 08-CONTEXT.md — Phase 8: Investigate and Fix TUI Blank Screens

## Domain
**Fix TUI rendering so all screens display content correctly.** User runs `m31a` and sees blank terminal instead of FirstRun wizard, Home screen, or REPL.

## Carrying Forward from Earlier Phases
- Project uses Go 1.25+, Bubble Tea (Elm architecture), CGO_ENABLED=0
- Three LLM providers: OpenRouter, Zen, Nvidia (dynamic model discovery)
- API keys via OS keychain only
- Cross-compile: linux/amd64, linux/arm64, darwin/amd64, darwin/arm64, windows/amd64
- 75% overall test coverage, 90% for critical packages

## Spec Lock
No SPEC.md exists for this phase.

## Canonical Refs
- `.planning/codebase/ARCHITECTURE.md` — TUI architecture (Bubble Tea, single-threaded Update)
- `.planning/codebase/STACK.md` — Go, Bubble Tea, lipgloss, charm libraries
- `.planning/codebase/STRUCTURE.md` — `internal/tui/` layout, 30+ screen models
- `.planning/PROJECT.md` — Vision, success criteria AC-5: "No blank screens at any point"
- `.planning/ROADMAP.md` — Phase 1: Fix TUI Blank Screens
- `.planning/REQUIREMENTS.md` — FR-1.1 through FR-1.8, AC-1, AC-5

## Code Context (Reusable Assets)
- **Screen registry**: `app_screens.go:renderScreenContent()` — 30+ cases mapping Screen enum to renderers
- **Router**: `router.go` — Screenable interface (Init, Update, View, SetDimensions, SetTheme)
- **Dimensions**: `app_nav.go:contentDimensions()` — computes content W/H minus chrome (2 rows) and sidebar
- **Window resize**: `app_input_resize.go:handleWindowResize()` — resizes all sub-models via SetDimensions or direct field access
- **Theme**: `internal/tui/theme/` — Dark-only Apple-inspired palette (BgBase #0F1117, TextPrimary #E2E4E9)
- **FirstRun**: `firstrun_model.go` / `firstrun_view.go` — 4-step wizard with fallbacks for 0 dims
- **Home**: `home_model.go` / `home_view.go` — logo, prompt, suggestions, tips
- **REPL**: `repl_model.go` / `repl_view.go` — chat interface, streaming, viewport

## Decisions

### 1. TTY Requirement — Real Terminal First
**Decision**: Prioritize fixing TUI in real interactive terminal. Non-TTY environments (CI, scripts) get virtual TTY support later.
**Rationale**: Bubble Tea requires TTY for alt-screen mode. "could not open TTY" errors in logs confirm this. Tests pass because they mock dimensions.

### 2. WindowSizeMsg Timing — Expected Bubble Tea Behavior
**Decision**: Accept that View() returns "" for 0×0 dims before first WindowSizeMsg. This is correct Elm architecture pattern.
**Action**: Verify WindowSizeMsg arrives in real terminal (add debug logging if needed). No code change unless msg not arriving.

### 3. Theme/Color — Lipgloss Compatibility Audit
**Decision**: Audit theme tokens and lipgloss styles for color combinations that render invisible on common terminals (xterm-256color, truecolor).
**Rationale**: Colors look correct (#0F1117 bg, #E2E4E9 fg) but lipgloss may emit ANSI sequences that some terminals misinterpret. Check:
- `theme/cache.go` styles use `Foreground(t.TextPrimary)` consistently
- No style accidentally sets `Background(t.Background)` + `Foreground(t.Background)`
- Border colors contrast with background

### 4. Screenable Implementation — Full Audit Required
**Decision**: Verify all 30+ screens implement complete Screenable interface (Init, Update, View, SetDimensions, SetTheme).
**Finding**: ReplModel missing SetDimensions (uses direct width/height fields). Other screens may have gaps.
**Action**: Add SetDimensions to ReplModel and any other incomplete implementations. Ensure router.Register() called for all screens.

### 5. Dimension Calculation — contentDimensions() Edge Cases
**Decision**: Audit `contentDimensions()` for cases where sidebar width > terminal width or chrome subtraction leaves ≤0 content height.
**Check points**:
- `app_nav.go:358` — contentH = height - 2 (chrome); sidebar subtraction
- `layout/page.go:44` — PageChrome.ContentHeight() = Height - 2
- Minimum dims guard in `app_view.go:263` — UltraNarrow < 40 cols or height < 10 rows shows "too narrow"

## Deferred Ideas
- CI-compatible headless TUI testing (vhs, expect, gotty)
- Light/auto theme support (currently dark only)
- Windows ARM64 target

## Next Steps
1. **Immediate**: Add debug logging to track WindowSizeMsg arrival and View() calls in real terminal
2. **Audit**: Screenable interface completeness across all 30+ screens
3. **Theme**: Verify lipgloss color output with `TERM=xterm-256color` and truecolor
4. **Dimensions**: Add guards in contentDimensions() for negative/zero content area
5. **Test**: Run `m31a` in real terminal, verify FirstRun → Home → REPL flow renders

## Discussion Log
- TTY: Real terminal first, virtual TTY later
- WindowSizeMsg: Expected behavior confirmed
- Theme: Lipgloss compatibility suspected
- Screenable: Audit all implementations
- Dimensions: Edge case guards needed
EOF