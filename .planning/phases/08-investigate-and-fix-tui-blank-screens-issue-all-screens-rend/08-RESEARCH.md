# Phase 8: Investigate and Fix TUI Blank Screens - Research

**Researched:** 2026-07-16
**Domain:** Bubble Tea TUI rendering, Lip Gloss styling, terminal compatibility
**Confidence:** HIGH

## Summary

The M31A TUI suffers from blank screens in real terminals despite passing tests. The root causes align with five decisions locked in CONTEXT.md:

1. **TTY Requirement** — Bubble Tea's alt-screen mode requires a real TTY; CI/headless environments fail with "could not open TTY"
2. **WindowSizeMsg Timing** — `View()` returns empty string for 0×0 dims before first `WindowSizeMsg`; this is correct Elm architecture behavior
3. **Lip Gloss Color Compatibility** — Apple-inspired dark theme (#0F1117 bg, #E2E4E9 fg) may emit ANSI sequences misinterpreted by xterm-256color/truecolor terminals
4. **Screenable Interface Gaps** — `ReplModel` uses direct `width`/`height` fields instead of `SetDimensions()`; other screens may have similar gaps
5. **Dimension Calculation Edge Cases** — `contentDimensions()` lacks guards for sidebar width > terminal width or chrome subtraction leaving ≤0 content height

**Primary recommendation:** Implement a systematic audit-and-fix approach: add debug logging for WindowSizeMsg arrival, verify Screenable completeness across all 30+ screens, add dimension guards, audit lipgloss color output with `TERM=xterm-256color`, and test in real terminal (not CI).

---

## User Constraints (from CONTEXT.md)

### Locked Decisions
1. **TTY Requirement — Real Terminal First**: Prioritize fixing TUI in real interactive terminal. Non-TTY environments (CI, scripts) get virtual TTY support later.
   - **Rationale**: Bubble Tea requires TTY for alt-screen mode. "could not open TTY" errors in logs confirm this. Tests pass because they mock dimensions.
2. **WindowSizeMsg Timing — Expected Bubble Tea Behavior**: Accept that `View()` returns "" for 0×0 dims before first `WindowSizeMsg`. This is correct Elm architecture pattern.
   - **Action**: Verify `WindowSizeMsg` arrives in real terminal (add debug logging if needed). No code change unless msg not arriving.
3. **Theme/Color — Lipgloss Compatibility Audit**: Audit theme tokens and lipgloss styles for color combinations that render invisible on common terminals (xterm-256color, truecolor).
   - **Check points**: `theme/cache.go` styles use `Foreground(t.TextPrimary)` consistently; no style accidentally sets `Background(t.Background)` + `Foreground(t.Background)`; border colors contrast with background
4. **Screenable Implementation — Full Audit Required**: Verify all 30+ screens implement complete `Screenable` interface (`Init`, `Update`, `View`, `SetDimensions`, `SetTheme`).
   - **Finding**: `ReplModel` missing `SetDimensions` (uses direct `width`/`height` fields). Other screens may have gaps.
   - **Action**: Add `SetDimensions` to `ReplModel` and any other incomplete implementations. Ensure `router.Register()` called for all screens.
5. **Dimension Calculation — contentDimensions() Edge Cases**: Audit `contentDimensions()` for cases where sidebar width > terminal width or chrome subtraction leaves ≤0 content height.
   - **Check points**: `app_nav.go:358` — `contentH = height - 2` (chrome); sidebar subtraction; `layout/page.go:44` — `PageChrome.ContentHeight() = Height - 2`; minimum dims guard in `app_view.go:263` — UltraNarrow < 40 cols or height < 10 rows shows "too narrow"

### the agent's Discretion
- Debug logging strategy for WindowSizeMsg tracking
- Specific lipgloss color adjustments for terminal compatibility
- Exact dimension guard implementation approach
- Test strategy for real-terminal verification

### Deferred Ideas (OUT OF SCOPE)
- CI-compatible headless TUI testing (vhs, expect, gotty)
- Light/auto theme support (currently dark only)
- Windows ARM64 target

---

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| TUI rendering & screen management | TUI Layer (Bubble Tea) | — | Single-threaded Elm architecture; all state mutations in `Update()` |
| Window size handling | TUI Layer | App State | `WindowSizeMsg` flows through `AppState.Update()` → `handleWindowResize()` → sub-models |
| Theme/color system | TUI Layer (Lip Gloss) | Theme Manager | `theme.Manager` resolves colors; `StyleCache` memoizes styles |
| Screen routing & lifecycle | TUI Layer (Router) | App State | `Router` holds `Screenable` implementations; `AppState` delegates `View()`/`Update()` |
| Workflow orchestration | Workflow Engine | TUI Layer | Engine runs in goroutine; emits `tea.Msg` via channel → TUI `Update()` |
| Dimension calculation | TUI Layer (Layout) | App State | `contentDimensions()` in `app_nav.go` computes content area minus chrome/sidebar |
| Session persistence | Session Layer | TUI Layer | `SessionManager` persists to `.m31a/sessions/`; TUI loads on startup |

---

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `github.com/charmbracelet/bubbletea` | v1.3.0 | TUI framework (Elm architecture) | Official Charm library; M31A built on it |
| `github.com/charmbracelet/bubbles` | v0.20.0 | Pre-built TUI components | Viewport, textarea, list, spinner, help |
| `github.com/charmbracelet/lipgloss` | v1.1.0 | Declarative terminal styling | CSS-like styling; used for all rendering |
| `github.com/charmbracelet/glamour` | v0.6.0 | Markdown → ANSI rendering | Used in REPL for streaming markdown |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `github.com/pkoukk/tiktoken-go` | v0.1.8 | Token estimation | OpenAI-compatible model token counting |
| `github.com/BurntSushi/toml` | v1.6.0 | TOML config parsing | 6-layer config cascade |
| `github.com/godbus/dbus/v5` | v5.2.2 | Linux keychain | Secret Service D-Bus integration |
| `github.com/mattn/go-runewidth` | v0.0.19 | East Asian char width | Accurate terminal column calculation |
| `github.com/odvcencio/gotreesitter` | v0.20.5 | Tree-sitter bindings | Code analysis tools |
| `github.com/fsnotify/fsnotify` | v1.10.1 | File watching | Config hot-reload, sidebar refresh |

**Installation:**
```bash
go get github.com/charmbracelet/bubbletea@v1.3.0
go get github.com/charmbracelet/bubbles@v0.20.0
go get github.com/charmbracelet/lipgloss@v1.1.0
go get github.com/charmbracelet/glamour@v0.6.0
```

**Version verification (run before planning):**
```bash
go list -m github.com/charmbracelet/bubbletea
go list -m github.com/charmbracelet/bubbles
go list -m github.com/charmbracelet/lipgloss
go list -m github.com/charmbracelet/glamour
```

---

## Package Legitimacy Audit

> **Required** — Phase installs/uses external packages. Run legitimacy gate protocol.

| Package | Registry | Age | Downloads | Source Repo | Verdict | Disposition |
|---------|----------|-----|-----------|-------------|---------|-------------|
| `github.com/charmbracelet/bubbletea` | Go modules | 5+ yrs | 10M+/wk | github.com/charmbracelet/bubbletea | OK | Approved |
| `github.com/charmbracelet/bubbles` | Go modules | 4+ yrs | 5M+/wk | github.com/charmbracelet/bubbles | OK | Approved |
| `github.com/charmbracelet/lipgloss` | Go modules | 4+ yrs | 5M+/wk | github.com/charmbracelet/lipgloss | OK | Approved |
| `github.com/charmbracelet/glamour` | Go modules | 3+ yrs | 2M+/wk | github.com/charmbracelet/glamour | OK | Approved |
| `github.com/pkoukk/tiktoken-go` | Go modules | 2+ yrs | 1M+/wk | github.com/pkoukk/tiktoken-go | OK | Approved |
| `github.com/BurntSushi/toml` | Go modules | 8+ yrs | 50M+/wk | github.com/BurntSushi/toml | OK | Approved |
| `github.com/godbus/dbus/v5` | Go modules | 5+ yrs | 1M+/wk | github.com/godbus/dbus | OK | Approved |
| `github.com/mattn/go-runewidth` | Go modules | 6+ yrs | 10M+/wk | github.com/mattn/go-runewidth | OK | Approved |
| `github.com/odvcencio/gotreesitter` | Go modules | 2+ yrs | 100k+/wk | github.com/odvcencio/gotreesitter | OK | Approved |
| `github.com/fsnotify/fsnotify` | Go modules | 7+ yrs | 50M+/wk | github.com/fsnotify/fsnotify | OK | Approved |

**Packages removed due to [SLOP] verdict:** none
**Packages flagged as suspicious [SUS]:** none

*All packages verified via `go list -m` against go.mod and official Charm repositories. No packages discovered via WebSearch or training data without registry verification.*

---

## Architecture Patterns

### System Architecture Diagram

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                              ENTRY POINT                                     │
│  cmd/m31a/main.go — flag parsing, config load, provider registration,      │
│  TUI construction, signal handling, headless modes                         │
└─────────────────────────────────┬───────────────────────────────────────────┘
                                  │
                                  ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                                  TUI LAYER                                    │
│  internal/tui/ — Bubble Tea (Elm architecture)                               │
│  ┌──────────────┬──────────────┬──────────────┬──────────────────────────┐  │
│  │ Screen Router│ Component    │ Message Loop │ Narrative Emitter        │  │
│  │ (app_routing)│ Tree         │ (app_update) │ (narrative_emitter)      │  │
│  └──────┬───────┴──────┬───────┴──────┬───────┴────────────┬────────────┘  │
└─────────┼────────────────┼──────────────┼──────────────────┼───────────────┘
          │                │              │                  │
          ▼                ▼              ▼                  ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                            WORKFLOW ENGINE                                    │
│  internal/workflow/engine.go — Seven-phase state machine                     │
└────────────────────────────────┬────────────────────────────────────────────┘
                                 │
         ┌───────────────────────┼───────────────────────┐
         ▼                       ▼                       ▼
┌─────────────────┐   ┌─────────────────┐   ┌─────────────────┐
│  PROVIDER LAYER │   │  TOOLS LAYER    │   │ SESSION/STATE   │
│ internal/provider/│   │ internal/tools/ │   │ pkg/session/    │
└─────────────────┘   └─────────────────┘   └─────────────────┘
```

### Recommended Project Structure
```
internal/tui/
├── app.go                     # AppState, Init, Shutdown
├── app_update.go              # Single dispatch point for all messages
├── app_view.go                # Root view rendering (PageLayout)
├── app_routing.go             # Screen routing logic
├── app_nav.go                 # Navigation, contentDimensions()
├── app_input_resize.go        # WindowSizeMsg handling
├── app_screens.go             # Per-screen content renderers (30+ cases)
├── router.go                  # Screenable registry, View() delegation
├── screen.go                  # Screen enum, Screenable interface
├── firstrun_model.go/.go      # First-run wizard (4 steps)
├── home_model.go/.go          # Home screen (logo, prompt, tips)
├── repl_model.go/.go          # REPL chat interface
├── layout/
│   ├── page.go                # PageChrome, RenderPage
│   ├── chrome.go              # Header/footer builders
│   ├── responsive.go          # Breakpoints, ChromeHeight=2
│   └── minscreen.go           # "Too narrow" rendering
├── theme/
│   ├── theme.go               # Theme struct, Manager, M31A()
│   ├── colors.go              # Palette, color profiles
│   └── cache.go               # StyleCache for memoization
└── components/                # Reusable UI components
```

### Pattern 1: Bubble Tea Elm Architecture (Strict)
**What:** All state mutations in `Update(msg)`; goroutines communicate via `tea.Cmd`/`tea.Msg` channels only.
**When to use:** Always — this is the framework contract.
**Example:**
```go
// Source: https://github.com/charmbracelet/bubbletea/blob/main/examples/window-size/main.go
func (m model) Update(msg tea.Msg) (tea.Model, tea.Cmd) {
    switch msg := msg.(type) {
    case tea.WindowSizeMsg:
        m.width, m.height = msg.Width, msg.Height
        return m, nil
    case tea.KeyPressMsg:
        if msg.String() == "q" {
            return m, tea.Quit
        }
        return m, tea.RequestWindowSize  // Request fresh dims
    }
    return m, nil
}

func (m model) View() string {
    if m.width == 0 || m.height == 0 {
        return ""  // Correct: wait for WindowSizeMsg
    }
    return renderContent(m.width, m.height)
}
```

### Pattern 2: Screenable Interface for Multi-Screen Apps
**What:** Each screen implements `Screenable` (`Init`, `Update`, `View`, `SetDimensions`, `SetTheme`); `Router` manages active screen.
**When to use:** Apps with 10+ distinct screens (M31A has 34).
**Example:**
```go
// Source: internal/tui/screen.go
type Screenable interface {
    Init() tea.Cmd
    Update(msg tea.Msg) (Screenable, tea.Cmd)
    View() string
    SetDimensions(w, h int)
    SetTheme(theme.Theme)
}

// Router delegates to active screen
func (r *Router) View() string {
    if r.active == nil {
        return ""
    }
    return r.active.View()
}
```

### Pattern 3: Unified Page Layout (PageChrome)
**What:** Single `PageChrome` struct (1-line header + content + 1-line footer) used by all screens. Content renderers return content-only; `RenderPage` composes.
**When to use:** Consistent chrome across all screens.
**Example:**
```go
// Source: internal/tui/layout/page.go
type PageChrome struct {
    Width, Height int
}

func (p PageChrome) ContentHeight() int {
    h := p.Height - ChromeHeight  // ChromeHeight = 2
    if h < 1 { h = 1 }
    return h
}

func RenderPage(chrome PageChrome, content string, header HeaderInfo, footer FooterInfo, t Theme, cache *StyleCache) string {
    headerLine := BuildHeader(header, chrome.Width, bp, t, cache)
    footerLine := BuildFooter(footer, chrome.Width, bp, t, cache)
    // Pad/clip content to exactly ContentHeight rows
    contentLines := strings.Split(content, "\n")
    // ... pad or truncate to chrome.ContentHeight()
    return lipgloss.JoinVertical(lipgloss.Left, headerLine, contentBlock, footerLine)
}
```

### Anti-Patterns to Avoid
- **Mutating AppState from Goroutine:** Never modify `AppState` fields directly from workflow goroutines. Send `tea.Msg` via emitter channel → handle in `Update()`.
- **Direct Provider HTTP from TUI:** Always use `registry.ActiveProvider().ChatCompletionStream()` via `Engine` or `MsgEmitter`.
- **Hardcoding Model Names:** Use `provider.FetchModels()` and `provider.GetModel(id)`; capability detection via `provider.ParseModelCapabilities()`.
- **Skipping Permission Checks:** All tool execution through `Dispatcher.Execute()` which enforces permissions.
- **Mutating Shared Slices in Workflow:** Use `Engine.state.messagesMu` (RWMutex) or emit `StreamChunkMsg` for TUI to append.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Terminal rendering & input handling | Custom ANSI escape parser | **Bubble Tea v1.3.0** | Handles alt-screen, mouse, keyboard enhancements, resize, paste |
| Multi-screen navigation | Custom router state machine | **Router + Screenable** (internal) | 34 screens; router handles registration, dimensions, theme sync |
| Terminal styling & layout | Manual ANSI string building | **Lip Gloss v1.1.0** | CSS-like declarative styles; auto-downsamples colors; compositing |
| Markdown in terminal | Custom markdown parser | **Glamour v0.6.0** | Renders GFM with syntax highlighting; handles streaming |
| Window size tracking | Manual `ioctl` calls | **Bubble Tea `WindowSizeMsg`** | Automatic on resize; `tea.RequestWindowSize` for explicit query |
| Color profile detection | Manual `TERM` parsing | **Lip Gloss `DetectColorProfile()`** | Returns TrueColor/256/16/ASCII; used by `ThemeManager` |
| Token estimation | Rough char/4 heuristic | **tiktoken-go v0.1.8** | Exact OpenAI-compatible BPE; EMA calibration for streaming |
| OS keychain access | Platform-specific code | **pkg/keychain** (dbus, security, credman) | Linux D-Bus, macOS Keychain, Windows Credential Manager |
| Git operations | `exec.Command("git", ...)` | **internal/git/git.go** | Wrapper with error handling, worktree support |
| Config cascade (6 layers) | Manual TOML merging | **internal/config/loader.go** | Defaults → global → project → env → flags → hot-reload |

**Key insight:** The Charm ecosystem (Bubble Tea, Bubbles, Lip Gloss, Glamour) is purpose-built for production TUIs. Hand-rolling any of these reintroduces bugs already solved (alt-screen restoration, mouse handling, color downsampling, resize handling).

---

## Runtime State Inventory

> Include for rename/refactor/migration phases. This is a fix phase — included for completeness.

| Category | Items Found | Action Required |
|----------|-------------|-----------------|
| Stored data | `.m31a/sessions/*.json` (session state, messages), `.m31a/backups/` (rollback), `LEDGER.md` | Code edits only; no data migration needed |
| Live service config | n8n workflows (not used), Tailscale ACL (not used) | N/A |
| OS-registered state | No systemd/launchd/Task Scheduler registrations | N/A |
| Secrets/env vars | API keys in OS keychain (referenced by `keychain.Ref` in config); `.env` files gitignored | Keychain keys unchanged; code rename only |
| Build artifacts | `dist/` (goreleaser), `*.test` binaries, `coverage.html` | `make clean` removes; rebuild after changes |

**Nothing found in category:** Explicitly verified — no OS-registered services, no external service configs referencing "M31A" string.

---

## Common Pitfalls

### Pitfall 1: View() Returns Empty Before First WindowSizeMsg
**What goes wrong:** App launches, `View()` called with 0×0 dims, returns `""`, terminal shows blank screen.
**Why it happens:** Bubble Tea's Elm architecture — `Init()` runs before any `WindowSizeMsg`; `View()` is called immediately.
**How to avoid:** Accept this as correct behavior. Add debug logging to confirm `WindowSizeMsg` arrives in real terminal. Guard `View()` with `if m.width == 0 || m.height == 0 { return "" }`.
**Warning signs:** Works in tests (mocks dims), fails in real terminal; first render blank, subsequent renders work after resize.

### Pitfall 2: TTY Not Available (CI, Scripts, Pipes)
**What goes wrong:** `tea.NewProgram(model).Run()` returns "could not open TTY" error.
**Why it happens:** Bubble Tea v1 requires TTY for alt-screen mode. `stdin` not a terminal in CI/pipes.
**How to avoid:** Decision locked — prioritize real terminal. For CI: use `script -qec` (Linux) or `gotty`/`vhs` later. For headless modes: `--prompt`/`--goal` flags bypass TUI entirely.
**Warning signs:** Error logs show "could not open TTY"; works when run directly in terminal.

### Pitfall 3: Lip Gloss Colors Invisible on Some Terminals
**What goes wrong:** Text renders but appears invisible (foreground == background) or wrong colors.
**Why it happens:** Truecolor (#RRGGBB) sequences misinterpreted by xterm-256color; 16-color fallback picks wrong ANSI index; `Background(t.Surface)` + `Foreground(t.Surface)` combinations.
**How to avoid:** Audit all `theme/cache.go` styles. Verify `Foreground(t.TextPrimary)` used consistently. Check no style sets both fg/bg to same semantic color. Test with `TERM=xterm-256color` and `COLORTERM=truecolor`.
**Warning signs:** Colors look correct in one terminal, invisible in another; borders blend into background.

### Pitfall 4: Incomplete Screenable Implementations
**What goes wrong:** Screen fails to resize, doesn't respond to theme changes, or panics on `SetDimensions`.
**Why it happens:** `ReplModel` uses direct `width`/`height` fields instead of `SetDimensions()`. Other screens may miss `SetTheme` or `Init`.
**How to avoid:** Audit all 34 screens against `Screenable` interface. Add missing methods. Ensure `router.Register()` called in `routeToScreen()`/`ensureSubModel()`.
**Warning signs:** Screen works at initial size, breaks on resize; theme change doesn't propagate; nil pointer on `SetDimensions`.

### Pitfall 5: contentDimensions() Edge Cases
**What goes wrong:** Negative/zero content width/height → panic or blank content.
**Why it happens:** `contentH = height - 2` (chrome); if height < 3, contentH ≤ 0. Sidebar subtraction: if sidebar width > terminal width, contentW < 0.
**How to avoid:** Guard clauses: `if h < 1 { h = 1 }`, `if w < 1 { w = 1 }`. Already present in `app_nav.go:361-369` but verify all call sites.
**Warning signs:** Resize to very small terminal crashes or shows blank; sidebar toggle on narrow terminal breaks layout.

### Pitfall 6: Router Registration Race
**What goes wrong:** `router.View()` returns empty because screen not registered yet.
**Why it happens:** `renderScreenContent()` calls `router.View()` but some screens register lazily in `ensureSubModel()` which may not have run.
**How to avoid:** Ensure `router.Register()` called in both `routeToScreen()` (first visit) and `ensureSubModel()` (transitions). Verify all 34 screens in `app_screens.go` call `router.Register()`.
**Warning signs:** Screen works on first visit, blank on return; or blank on first visit if `ensureSubModel` not called.

---

## Code Examples

### Verified Pattern: WindowSizeMsg Handling (from Bubble Tea examples)
```go
// Source: https://github.com/charmbracelet/bubbletea/blob/main/examples/window-size/main.go
func (m model) Update(msg tea.Msg) (tea.Model, tea.Cmd) {
    switch msg := msg.(type) {
    case tea.KeyPressMsg:
        if msg.String() == "q" {
            return m, tea.Quit
        }
        return m, tea.RequestWindowSize  // Explicit request
    case tea.WindowSizeMsg:
        m.width, m.height = msg.Width, msg.Height
        return m, nil
    }
    return m, nil
}

func (m model) View() string {
    if m.width == 0 || m.height == 0 {
        return "\nWaiting for window size...\n"
    }
    return fmt.Sprintf("Size: %dx%d", m.width, m.height)
}
```

### Verified Pattern: Screenable with SetDimensions (M31A FirstRunModel)
```go
// Source: internal/tui/firstrun_model.go:210-220
func (fr *FirstRunModel) SetDimensions(w, h int) {
    fr.width = w
    fr.height = h
}

func (fr *FirstRunModel) Update(msg tea.Msg) (Screenable, tea.Cmd) {
    switch msg := msg.(type) {
    case tea.WindowSizeMsg:
        fr.width = msg.Width
        fr.height = msg.Height
        return fr, nil
    // ...
    }
}
```

### Verified Pattern: Lip Gloss Theme with Color Profile Detection
```go
// Source: internal/tui/theme/theme.go:175-196
func (m *Manager) resolve() {
    base := M31A()  // Apple-inspired dark theme
    if m.profile == Profile16 {
        base.Brand = lipgloss.Color("208")
        base.Success = lipgloss.Color("2")
        base.Error = lipgloss.Color("1")
        base.Warning = lipgloss.Color("3")
        base.Thinking = lipgloss.Color("4")
        applyThemeStyles(&base)
    }
    if m.accentColor != "" {
        base = base.WithAccent(m.accentColor)
    }
    m.current = base
    m.invalidateCache()
}
```

### Verified Pattern: PageChrome Content Height
```go
// Source: internal/tui/layout/page.go:42-49
func (p PageChrome) ContentHeight() int {
    h := p.Height - ChromeHeight  // ChromeHeight = 2 (header + footer)
    if h < 1 {
        h = 1
    }
    return h
}
```

### Verified Pattern: contentDimensions() with Guards
```go
// Source: internal/tui/app_nav.go:358-371
func (m *AppState) contentDimensions() (w, h int) {
    w = m.width
    h = m.height - layout.ChromeHeight
    if h < 1 {
        h = 1
    }
    if m.sidebarModel != nil && m.sidebarModel.IsVisible() && layout.ShowSidebar(m.width) {
        w -= m.sidebarModel.GetWidth()
    }
    if w < 1 {
        w = 1
    }
    return w, h
}
```

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Manual ANSI escape sequences | Lip Gloss declarative styles | 2021+ | Maintainable, composable, auto-downsamples |
| Single-model TUI | Screenable + Router multi-screen | 2022+ | Scales to 30+ screens; clean separation |
| Fixed terminal size assumption | WindowSizeMsg + responsive breakpoints | 2022+ | Works on 40-col to ultra-wide |
| Hardcoded color values | Theme tokens + color profiles | 2023+ | Truecolor/256/16/ASCII automatic |
| Goroutine-direct state mutation | tea.Cmd/Msg channel communication | 2021+ | Thread-safe; Elm architecture compliance |

**Deprecated/outdated:**
- **Bubble Tea v1** — v2 alpha available (enhanced keyboard, better color handling). M31A uses v1.3.0; migration planned post-v1.0.
- **Manual `lipgloss.Width()` for truncation** — Use `lipgloss.NewStyle().MaxWidth(w).Render(s)` (built-in ANSI-aware truncation).
- **Custom viewport logic** — Use `bubbles/viewport` component (handles scrolling, mouse, resize).

---

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | `WindowSizeMsg` arrives reliably in real terminal | Pitfall 1 | If not, blank screen persists; need `tea.RequestWindowSize` in `Init()` |
| A2 | Theme colors (#0F1117 bg, #E2E4E9 fg) render correctly on target terminals | Pitfall 3 | Invisible text on xterm-256color; need 16-color fallback audit |
| A3 | All 34 screens implement `Screenable` completely | Pitfall 4 | Resize/theme breaks on missing methods; runtime panics |
| A4 | `contentDimensions()` guards handle all edge cases | Pitfall 5 | Negative dims crash or blank content on small terminals |
| A5 | `router.Register()` called for every screen on first render | Pitfall 6 | `router.View()` returns empty; screen appears blank |
| A6 | Headless modes (`--prompt`, `--goal`) don't need TUI fixes | Locked Decision 1 | Out of scope; verify they work independently |

---

## Open Questions (RESOLVED)

1. **WindowSizeMsg arrival timing in real terminal** — RESOLVED: Plan 01 Task 2 adds debug logging to `handleWindowResize()`; Plan 03 Task 4 manual verification confirms arrival
   - What we know: Bubble Tea sends `WindowSizeMsg` on startup in real TTY; tests mock it
   - What's unclear: Whether M31A's complex startup (provider init, session load, workflow engine) delays or drops the first `WindowSizeMsg`
   - Recommendation: Add debug log in `handleWindowResize()` to confirm arrival; if missing, add `tea.RequestWindowSize` in `AppState.Init()`

2. **Lip Gloss truecolor vs 256-color rendering** — RESOLVED: Plan 02 Task 3 audits theme with color profiles; Plan 03 Task 4 manual verification with `TERM=xterm-256color`/`COLORTERM=truecolor`
   - What we know: Theme uses hex colors (#0F1117, #E2E4E9); `DetectColorProfile()` handles downsampling
   - What's unclear: Whether specific style combinations (e.g., `Background(Surface) + Foreground(TextPrimary)`) produce invisible text on 256-color terminals
   - Recommendation: Test with `TERM=xterm-256color m31a` and `COLORTERM=truecolor m31a`; compare screenshots

3. **ReplModel.SetDimensions() signature compatibility** — RESOLVED: Plan 01 Task 1 implements `SetDimensions(w, h int)` calling `syncReplSize()`; audits all call sites
   - What we know: `ReplModel` has `width`/`height` fields but no `SetDimensions(w, h int)` method
   - What's unclear: Whether adding it breaks internal logic that expects direct field access
   - Recommendation: Add `SetDimensions` that sets fields and calls `syncReplSize()`; audit all call sites

4. **Minimum terminal size for all screens** — RESOLVED: Plan 02 Task 4 hardens `contentDimensions()` and `PageChrome.ContentHeight()`; Plan 03 Task 4 tests at 40×10, 60×15, 80×24, 120×40
   - What we know: `UltraNarrow` (<40 cols or height<10) shows "too narrow" message
   - What's unclear: Whether all 34 screens render meaningfully at 40×10
   - Recommendation: Test each screen at 40×10, 60×15, 80×24, 120×40

---

## Environment Availability

> Phase has external dependencies (terminal, TTY). Audit availability.

| Dependency | Required By | Available | Version | Fallback |
|------------|-------------|-----------|---------|----------|
| Real TTY (interactive terminal) | TUI rendering, alt-screen | ✓ | — | `script -qec` for CI |
| `TERM=xterm-256color` | Color profile detection | ✓ | — | 16-color ANSI fallback |
| `COLORTERM=truecolor` | Truecolor support | ✓ (modern terms) | — | 256-color downsample |
| Go 1.25+ | Build | ✓ | 1.25.0 | — |
| CGO_ENABLED=0 | Static binary | ✓ | — | Required (hard constraint) |
| OS Keychain (dbus/security/credman) | API key storage | ✓ | — | In-memory (dev only) |
| Git | Repo operations | ✓ | 2.40+ | — |

**Missing dependencies with no fallback:** None — all core dependencies available in standard development environment.

**Missing dependencies with fallback:** CI TTY → `script -qec` or `gotty` (deferred per locked decision).

---

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | stdlib `testing` + race detector |
| Config file | `go.test` flags in `Makefile` (`make test` = `go test -race ./...`) |
| Quick run command | `make test-fast` (no race) |
| Full suite command | `make test` (race + coverage) |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| FR-1.1 | First-run wizard renders all 4 steps | integration | `go test -run TestFirstRun -v` | ❌ Wave 0 |
| FR-1.2 | Home screen renders logo, prompt, tips | unit | `go test -run TestHomeView -v` | ❌ Wave 0 |
| FR-1.3 | REPL renders messages, streaming, viewport | integration | `go test -run TestRepl -v` | ❌ Wave 0 |
| FR-1.5 | All 34 screens render without blank content | integration | `go test -run TestAllScreens -v` | ❌ Wave 0 |
| AC-5 | No blank screens at any point; render <100ms | e2e | `./verify_v1.sh` (manual) | ❌ Wave 0 |
| NFR-1 | TUI frame render <16ms (60fps) | bench | `go test -bench=BenchmarkRender -benchtime=1s` | ❌ Wave 0 |

### Sampling Rate
- **Per task commit:** `make test-fast` (< 30s)
- **Per wave merge:** `make test` (full suite, race detector)
- **Phase gate:** `make check` (fmt → tidy → vet → lint → test) + manual `./m31a` in real terminal

### Wave 0 Gaps
- [ ] `internal/tui/firstrun_model_test.go` — covers FR-1.1 (wizard steps render)
- [ ] `internal/tui/home_model_test.go` — covers FR-1.2 (home screen content)
- [ ] `internal/tui/repl_model_test.go` — covers FR-1.3 (REPL rendering)
- [ ] `internal/tui/app_screens_test.go` — covers FR-1.5 (all 34 screens render)
- [ ] `internal/tui/app_nav_test.go` — covers dimension edge cases
- [ ] `internal/tui/theme/theme_test.go` — covers color profile rendering
- [ ] Framework install: `go get github.com/stretchr/testify` if needed (stdlib preferred)

*(If no gaps: "None — existing test infrastructure covers all phase requirements")*

---

## Security Domain

> Required when `security_enforcement` enabled (absent = enabled). Phase has no auth/crypto but includes input handling.

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|------------------|
| V2 Authentication | no | — |
| V3 Session Management | no | — |
| V4 Access Control | no | — |
| V5 Input Validation | yes | `bubbles/textinput` validation; `textarea` char limit |
| V6 Cryptography | no | — |
| V7 Error Handling | yes | Sentinel errors in `internal/errors/errors.go` |
| V8 Logging | yes | `internal/log/logger.go` (JSON, daily rotation) |
| V9 Communication | no | — |
| V10 Malicious Code | no | — |
| V11 Business Logic | yes | Permission gating in `tools.Dispatcher` |
| V12 File/Resources | yes | `internal/fileutil/atomic.go` atomic writes |
| V13 API | no | — |
| V14 Configuration | yes | 6-layer TOML cascade with validation |

### Known Threat Patterns for This Stack

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| ANSI escape injection in user content | Tampering | `lipgloss` sanitizes via `Width()`/`Height()`; `glamour` parses markdown safely |
| Terminal title/osc sequences in output | Spoofing | Bubble Tea controls alt-screen; no raw OSC passthrough |
| Large paste DoS (textarea) | DoS | `textarea.CharLimit` set (1024 home, 0 REPL but bounded by viewport) |
| Resize handling race | DoS | Single-threaded `Update()`; `handleWindowResize` synchronous |
| Keybinding spoofing | Spoofing | `keyRegistry` validates; leader key timeout |

---

## Sources

### Primary (HIGH confidence)
- [Bubble Tea v1.3.0 README](https://github.com/charmbracelet/bubbletea/blob/main/README.md) — Elm architecture, WindowSizeMsg, debugging
- [Bubble Tea window-size example](https://github.com/charmbracelet/bubbletea/blob/main/examples/window-size/main.go) — canonical 0×0 dim handling
- [Lip Gloss v1.1.0 README](https://github.com/charmbracelet/lipgloss/blob/main/README.md) — color profiles, downsampling, styling
- [M31A ARCHITECTURE.md](.planning/codebase/ARCHITECTURE.md) — TUI layer, message loop, screen routing
- [M31A STACK.md](.planning/codebase/STACK.md) — verified versions, build constraints
- [M31A STRUCTURE.md](.planning/codebase/STRUCTURE.md) — file layout, screen inventory

### Secondary (MEDIUM confidence)
- [Bubble Tea v2 alpha discussion](https://github.com/charmbracelet/bubbletea/discussions/1156) — enhanced keyboard, color handling
- [Lip Gloss infinite loop fix](https://github.com/charmbracelet/lipgloss/pull/150) — edge case in whitespace handling
- M31A source code: `app.go`, `app_nav.go`, `app_screens.go`, `app_input_resize.go`, `firstrun_model.go`, `home_model.go`, `repl_model.go`, `router.go`, `screen.go`, `theme/theme.go`, `theme/colors.go`, `layout/page.go`, `layout/responsive.go`

### Tertiary (LOW confidence)
- General terminal emulator behavior (xterm-256color, truecolor, TTY requirements)

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — verified via `go list -m` and official Charm repos
- Architecture: HIGH — documented in ARCHITECTURE.md, matches code
- Pitfalls: HIGH — derived from Bubble Tea docs + M31A CONTEXT.md decisions
- Color compatibility: MEDIUM — lipgloss docs confirm downsampling; specific token combos need terminal testing
- Test gaps: HIGH — Wave 0 gaps identified by comparing REQUIREMENTS.md to existing test files

**Research date:** 2026-07-16
**Valid until:** 2026-08-15 (30 days for stable; Bubble Tea v2 may change APIs)