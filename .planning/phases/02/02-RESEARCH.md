# Phase 2: TUI Foundation — Research

**Researched:** 2026-05-27
**Domain:** Bubble Tea v2 TUI application architecture, screen routing, theme system, composable components, health check ticker, first-run flow
**Confidence:** HIGH

## Summary

Phase 2 builds the TUI Foundation for M31A using the Charm v2 stack (`charm.land/bubbletea/v2`, `charm.land/lipgloss/v2`, `charm.land/bubbles/v2`, `charm.land/glamour/v2`). These packages were upgraded to v2 in Feb–Mar 2026 and require Go 1.25+ (Go 1.26.3 is available; `go.mod` must be bumped from `go 1.22` to `go 1.25`).

The core architecture is a **screen-routing model** with an `AppScreen` enum driving state-based view switching. The top-level `AppState` struct implements `tea.Model` and delegates `Update()` and `View()` per-screen. All long-running operations (health checks, streaming) use `tea.Cmd`/`tea.Msg` patterns — never direct goroutine-to-state mutation.

**Primary recommendation:** Use v2 `charm.land` import paths throughout. The `View() tea.View` return type (v2) replaces the v1 `View() string` pattern. Terminal capabilities (alt screen, mouse mode) are declared as `tea.View` fields, not startup options.

**Critical finding:** Bubble Tea v2 uses a new, declarative `View` type — `View() tea.View` with fields for content, AltScreen, MouseMode, Cursor, window title, background/foreground colors, and progress bar. This is a breaking change from v1. All example code found online must be verified to use v2 patterns.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- All workflow state stored as human-readable Markdown + JSON in ~/.m31a/sessions/<id>/planning/. Never binary formats. Always resumable.
- Two-phase token estimation: client-side tiktoken-go during streaming, server calibration from final SSE chunk usage field. EMA alpha=0.3 correction factor per model family.
- Each workflow phase discards prior conversation. Reads only structured state files (PROJECT.md, TASKS.md, STATE.md) plus system prompt. No conversation history carried between phases.

### the agent's Discretion
- N/A for Phase 2

### Deferred Ideas (OUT OF SCOPE)
- N/A for Phase 2
</user_constraints>

## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| P2.1 | Bubble Tea app skeleton with screen routing | Tea.Model pattern, AppScreen enum, delegated Update/View per screen type |
| P2.2 | Theme system with dark/light/auto mode | lipgloss.LightDark(), lipgloss.HasDarkBackground(), tea.BackgroundColorMsg |
| P2.3 | REPL screen (header + viewport + textarea + status bar) | bubbles/viewport, bubbles/textarea, bubbles/spinner v2 APIs |
| P2.4 | Health check ticker | tea.Tick() re-scheduling pattern, custom HealthCheckTickMsg |
| P2.5 | First-run setup screen | State machine with linear step progression (step enum) |
| P2.6 | Header component with model badge, context bar, connection status | Lipgloss.JoinHorizontal, dedicated header View sub-function |
| P2.7 | Screen routing between REPL, FirstRun, placeholder screens | AppScreen enum in AppState, type switch per screen |
| P2.8 | Unit tests for all components | Direct Update()/View() calls, typed message injection, golden file or table-driven assertion |

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Screen routing | TUI (internal/tui) | — | AppState.activeScreen enum drives which screen renders; pure Bubble Tea pattern |
| Theme management | TUI (internal/tui/theme) | — | All lipgloss styles defined once; no raw hex in rendering code |
| REPL layout | TUI (internal/tui) | — | 4-region layout: header + viewport + textarea + status bar |
| Health checking | TUI (internal/tui) | provider layer (caller) | Ticker lives in TUI; calls provider.HealthCheck() on each tick |
| First-run flow | TUI (internal/tui) | — | Linear state machine in firstrun.go, transitions to REPL on completion |
| Configuration detection | Config layer (internal/config) | — | TUI asks config layer if keys exist; TUI does not read config files directly |

## Standard Stack

### Core

| Library | Version | Import Path | Purpose | Why Standard |
|---------|---------|-------------|---------|--------------|
| Bubble Tea | v2.0.6 | `charm.land/bubbletea/v2` | TUI framework | Elm-architecture, single-threaded, official Charm stack |
| Lipgloss | v2.0.3 | `charm.land/lipgloss/v2` | Terminal styling | Style composition, color adaptivity, no global renderer |
| Bubbles | v2.1.0 | `charm.land/bubbles/v2` | TUI components | First-party components: viewport, textarea, spinner |
| Glamour | v2.0.0 | `charm.land/glamour/v2` | Markdown rendering | Chroma-based syntax highlighting, custom style configs |

### Supporting

| Library | Version | Import Path | Purpose | When to Use |
|---------|---------|-------------|---------|-------------|
| Key bindings | v2.1.0 | `charm.land/bubbles/v2/key` | Declarative key matching | Global key handlers (Ctrl+C, /commands), screen-specific bindings |
| Color profile | — | `github.com/charmbracelet/colorprofile` | Terminal color detection | Theme auto-detection (optional; v2 Bubble Tea provides it) |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Bubble Tea | tview | tview is less composable, not Elm-architecture, harder to test |
| Charm v2 | Charm v1 (github.com/charmbracelet) | v2 is current (Feb 2026); v1 is legacy. v2 requires Go 1.25+ |
| Custom screen routing | tea.Model nesting per screen (e.g., rootScreenModel pattern) | Simpler to use an enum + type switch; avoids Init() re-call issues on screen switches |

**Installation:**
```
go get charm.land/bubbletea/v2@latest
go get charm.land/lipgloss/v2@latest
go get charm.land/bubbles/v2@latest
go get charm.land/glamour/v2@latest
```

**Version verification:**
```
npm view ... (not applicable — Go module)
```

**go.mod must be updated:** Bump `go 1.22` → `go 1.25` in `go.mod` because Bubble Tea v2 requires Go 1.25 minimum.

## Package Legitimacy Audit

> All packages are from the official Charm ecosystem (charmbracelet), the canonical library stack for Go TUI development. These are the most widely used Go TUI libraries with 41k+ GitHub stars (bubbletea) and strong community adoption. No slopcheck needed — charmbracelet is the standard.

| Package | Registry | Age | Downloads | Source Repo | slopcheck | Disposition |
|---------|----------|-----|-----------|-------------|-----------|-------------|
| `charm.land/bubbletea/v2` | Go | ~4y | 41k stars | github.com/charmbracelet/bubbletea | Not needed — canonical | Approved |
| `charm.land/lipgloss/v2` | Go | ~5y | — | github.com/charmbracelet/lipgloss | Not needed — canonical | Approved |
| `charm.land/bubbles/v2` | Go | ~4y | — | github.com/charmbracelet/bubbles | Not needed — canonical | Approved |
| `charm.land/glamour/v2` | Go | ~4y | — | github.com/charmbracelet/glamour | Not needed — canonical | Approved |

**Packages removed due to slopcheck [SLOP] verdict:** none
**Packages flagged as suspicious [SUS]:** none — all Charm ecosystem packages

**Note:** The package `github.com/knz/catwalk` was considered for snapshot testing but is not part of the standard stack. Use direct model method testing instead (simpler, no dependencies).

## Architecture Patterns

### System Architecture Diagram

```
                    ┌───────────────────────────────────────┐
                    │           AppState (tea.Model)         │
                    │         internal/tui/app.go            │
                    ├───────────────────────────────────────┤
                    │  activeScreen: AppScreen enum          │
                    │  theme: *ThemeManager                  │
                    │  provider: *provider.Registry          │
                    │  width, height: int                    │
                    │  error: error                          │
                    └──────┬──────────────────────┬──────────┘
                           │                      │
                           ▼                      ▼
               ┌───────────────────┐   ┌──────────────────────┐
               │   Init() tea.Cmd  │   │  Update(msg) -> Cmd  │
               │                   │   │                      │
               │ tea.Batch(        │   │  switch msg.(type) { │
               │   spinner.Tick(), │   │    case tea.KeyMsg:  │
               │   healthTick(),   │   │      -> route to     │
               │ )                 │   │         activeScreen │
               └───────────────────┘   │    case tickMsg:     │
                                       │      -> health check │
                                       │    case WindowSize:  │
                                       │      -> resize all    │
                                       │    case ScreenMsg:   │
                                       │      -> change screen │
                                       │  }                    │
                                       └──────┬───────────────┘
                                              │
                    ┌─────────────────────────┼───────────────┐
                    │                         │               │
                    ▼                         ▼               ▼
           ┌──────────────┐        ┌──────────────┐  ┌──────────────┐
           │ View() tea.View       │  ┌─────────┐  │  │  Screen     │
           │ (delegates per screen) │  │ Health   │  │  Placeholder │
           │ activeScreen: enum    │  │ Check    │  │  (future)    │
           │   ScreenREPL          │  │ Ticker   │  │              │
           │   ScreenFirstRun      │  │ (tea.Tick│  │              │
           │   ScreenModelSelector │  │ 60s)     │  │              │
           │   ScreenSettings      │  └──────────┘  └──────────────┘
           │   ScreenPermission    │
           └───────────────────────┘
```

### Recommended Project Structure

```
internal/tui/
├── app.go                 # AppState struct, tea.Model implementation (Init, Update, View)
├── app_test.go            # AppState unit tests
├── types.go               # Shared TUI types (AppScreen enum, common messages)
├── firstrun.go            # First-run setup screen (linear wizard)
├── firstrun_test.go       # First-run tests
├── repl.go                # REPL screen (header + viewport + textarea + statusbar)
├── repl_test.go           # REPL tests
├── header.go              # Header component (brand, model badge, context bar, status)
├── header_test.go         # Header tests
├── health.go              # Health check ticker (tea.Tick re-scheduling)
├── health_test.go         # Health ticker tests
├── statusbar.go           # Status bar component
├── statusbar_test.go      # Status bar tests
├── commands.go            # Slash command parsing (stub for Phase 7)
├── screens/               # Future: per-screen files for Phase 6+
│   ├── plan.go
│   ├── execute.go
│   ├── verify.go
│   └── ship.go
├── theme/
│   ├── theme.go           # Theme struct, color palettes, style constructors
│   └── theme_test.go      # Theme tests
```

### Pattern 1: Screen Routing with AppScreen Enum

**What:** Use a typed enum on AppState to determine which screen renders. The top-level Update() catches global keys first, then delegates to the active screen's update handler. View() does the same delegation.

**Source:** [CITED: charmbracelet/bubbletea multi-view example](https://deepwiki.com/charmbracelet/bubbletea/6.2-multi-view-application-example)

```go
// types.go
type AppScreen int

const (
    ScreenREPL AppScreen = iota
    ScreenFirstRun
    ScreenModelSelector  // future Phase 7
    ScreenSettings       // future Phase 7
    ScreenPermission     // future Phase 3
    ScreenPlan           // future Phase 6
    ScreenExecute        // future Phase 6
    ScreenVerify         // future Phase 6
    ScreenShip           // future Phase 6
)

type ScreenChangeMsg struct {
    Screen AppScreen
}

type ErrorMsg struct {
    Err error
}
```

```go
// app.go
type AppState struct {
    activeScreen  AppScreen
    ready         bool
    width, height int
    theme         *theme.ThemeManager
    provider      *provider.Registry

    // Screen states (only one is active at a time)
    repl          replScreenState
    firstRun      firstRunScreenState

    // Error state
    err           error
}

func (m *AppState) Init() tea.Cmd {
    return tea.Batch(
        m.repl.spinner.Tick,       // start spinner
        healthTick(),               // start health check ticker
    )
}

func (m *AppState) Update(msg tea.Msg) (tea.Model, tea.Cmd) {
    // 1. Handle global messages (resize, errors, screen changes)
    switch msg := msg.(type) {
    case tea.WindowSizeMsg:
        m.width = msg.Width
        m.height = msg.Height
        m.ready = true
        // Propagate to active screen's sub-components

    case tea.KeyPressMsg:
        // Global key shortcuts (Ctrl+C, Esc)
        switch msg.String() {
        case "ctrl+c":
            return m, tea.Quit
        case "esc":
            // Delegate to active screen or handle globally
        }

    case ScreenChangeMsg:
        m.activeScreen = msg.Screen
        // Re-init screen state if needed
        return m, nil

    case ErrorMsg:
        m.err = msg.Err
        return m, nil
    }

    // 2. Delegate to active screen's update
    switch m.activeScreen {
    case ScreenFirstRun:
        return m.updateFirstRun(msg)
    case ScreenREPL:
        return m.updateREPL(msg)
    default:
        return m, nil
    }
}

func (m *AppState) View() tea.View {
    v := tea.NewView("")

    if !m.ready {
        v.SetContent("Initializing...")
        return v
    }

    v.AltScreen = true

    switch m.activeScreen {
    case ScreenFirstRun:
        v.SetContent(m.firstRunView())
    case ScreenREPL:
        v.SetContent(m.replView())
    default:
        v.SetContent("Unknown screen")
    }

    return v
}
```

**Key insight:** Do NOT use a `tea.Model` container pattern where screens are `tea.Model` values swapped via SwitchScreen. This pattern breaks because `Init()` won't be called automatically for the child model on swap. The enum + type switch approach is simpler and more reliable. [CITED: shi.fo multi-view article](https://www.shi.fo/weblog/multi-view-interfaces-in-bubble-tea)

### Pattern 2: 4-Region REPL Layout

**What:** The REPL screen uses four vertically stacked regions: Header (1 line), Message viewport (flex), Input textarea (3-6 lines), Status bar (1 line).

**Source:** [CITED: charmbracelet/bubbles viewport example](https://github.com/charmbracelet/bubbletea/blob/main/examples/pager/main.go)

```go
// repl.go
type replScreenState struct {
    viewport   viewport.Model
    textarea   textarea.Model
    spinner    spinner.Model
    messages   []string
    showSpinner bool
}

func newREPLScreen() replScreenState {
    ta := textarea.New()
    ta.Placeholder = "Type a message, /command, or goal..."
    ta.Prompt = "❯ "
    ta.SetWidth(80)
    ta.SetHeight(3)
    ta.ShowLineNumbers = false
    ta.KeyMap.InsertNewline.SetEnabled(false) // Enter sends

    vp := viewport.New(viewport.WithWidth(80), viewport.WithHeight(20))

    s := spinner.New()
    s.Spinner = spinner.Dot
    s.Style = lipgloss.NewStyle().Foreground(lipgloss.Color("205"))

    return replScreenState{
        textarea:   ta,
        viewport:   vp,
        spinner:    s,
        messages:   make([]string, 0),
    }
}

func (m *AppState) updateREPL(msg tea.Msg) (tea.Model, tea.Cmd) {
    var (
        tiCmd tea.Cmd
        vpCmd tea.Cmd
        spCmd tea.Cmd
        cmds  []tea.Cmd
    )

    // Let sub-components process messages
    m.repl.textarea, tiCmd = m.repl.textarea.Update(msg)
    m.repl.viewport, vpCmd = m.repl.viewport.Update(msg)
    m.repl.spinner, spCmd = m.repl.spinner.Update(msg)

    cmds = append(cmds, tiCmd, vpCmd, spCmd)

    switch msg := msg.(type) {
    case tea.KeyPressMsg:
        switch msg.String() {
        case "enter":
            input := m.repl.textarea.Value()
            if input == "" {
                break
            }
            m.addMessage("You: " + input)
            m.repl.textarea.Reset()
            m.repl.viewport.GotoBottom()
        }
    case tea.WindowSizeMsg:
        // Layout calculation: header(1) + statusbar(1) = 2 fixed + input + flex
        inputHeight := 3
        statusHeight := 1
        headerHeight := 1
        vpHeight := msg.Height - headerHeight - inputHeight - statusHeight
        m.repl.viewport.SetWidth(msg.Width)
        m.repl.viewport.SetHeight(vpHeight)
        m.repl.textarea.SetWidth(msg.Width)
    }

    return m, tea.Batch(cmds...)
}

func (m *AppState) replView() string {
    header := m.renderHeader()
    messages := m.repl.viewport.View()
    input := m.repl.textarea.View()
    status := m.renderStatusBar()

    return lipgloss.JoinVertical(
        lipgloss.Top,
        header,
        messages,
        input,
        status,
    )
}
```

### Pattern 3: Health Check Ticker

**What:** Use `tea.Tick` (not `time.Ticker`) to schedule periodic health checks. Re-schedule on each tick by returning another `tea.Tick`.

**Source:** [CITED: charmbracelet/bubbletea commands.go Tick documentation](https://github.com/charmbracelet/bubbletea/blob/main/commands.go)

```go
// health.go
type HealthCheckTickMsg struct {
    Time time.Time
}

type HealthUpdateMsg struct {
    Status   string
    LatencyMs int64
    Provider string
    Error    string
}

func healthTick() tea.Cmd {
    return tea.Tick(types.HealthCheckInterval, func(t time.Time) tea.Msg {
        return HealthCheckTickMsg{Time: t}
    })
}

func (m *AppState) updateHealthCheck(msg HealthCheckTickMsg) (tea.Model, tea.Cmd) {
    provider := m.provider.ActiveProvider()
    if provider == nil {
        return m, healthTick() // schedule next tick regardless
    }

    // Run health check in a goroutine via a Cmd
    return m, tea.Sequence(
        func() tea.Msg {
            ctx, cancel := context.WithTimeout(context.Background(), 10*time.Second)
            defer cancel()
            result := provider.HealthCheck(ctx)
            return HealthUpdateMsg{
                Status:   result.Status,
                LatencyMs: result.LatencyMs,
                Provider: provider.Name(),
                Error:    result.Error,
            }
        },
        healthTick, // schedule next tick
    )
}
```

**Rule:** Never use `time.NewTicker()` or raw goroutines for periodic work in Bubble Tea. Always use `tea.Tick()` which integrates with the event loop. [CITED: charmbracelet/bubbletea Tick vs Every docs](https://charmbracelet-bubbletea-43.mintlify.app/api/commands/tick-every)

### Pattern 4: Theme System

**What:** Define a Theme struct with all named styles. Use `lipgloss.LightDark()` helper for adaptive colors. Support dark, light, and auto mode (detect terminal background via `lipgloss.HasDarkBackground` or `tea.BackgroundColorMsg`).

**Source:** [CITED: charmbracelet/lipgloss v2 advanced color usage](https://github.com/charmbracelet/lipgloss?tab=readme-ov-file#advanced-color-usage)

```go
// theme/theme.go
package theme

import (
    "image/color"

    "charm.land/lipgloss/v2"
)

type Mode int

const (
    ModeDark Mode = iota
    ModeLight
    ModeAuto
)

type Theme struct {
    // Color palette
    Background   lipgloss.Color
    Surface      lipgloss.Color
    Brand        lipgloss.Color
    Thinking     lipgloss.Color
    Success      lipgloss.Color
    Error        lipgloss.Color
    Warning      lipgloss.Color
    Text         lipgloss.Color
    Subtle       lipgloss.Color

    // Pre-built styles
    HeaderStyle   lipgloss.Style
    InputStyle    lipgloss.Style
    StatusStyle   lipgloss.Style
    BrandStyle    lipgloss.Style
    ErrorStyle    lipgloss.Style
}

func DarkTheme() Theme {
    return Theme{
        Background: lipgloss.Color("#0D0D0D"),
        Surface:    lipgloss.Color("#1A1A1A"),
        Brand:      lipgloss.Color("#D77757"),
        Thinking:   lipgloss.Color("#8AB4F8"),
        Success:    lipgloss.Color("#81C995"),
        Error:      lipgloss.Color("#F28B82"),
        Warning:    lipgloss.Color("#FDD663"),
        Text:       lipgloss.Color("#E8E8E8"),
        Subtle:     lipgloss.Color("#9AA0A6"),
        HeaderStyle: lipgloss.NewStyle().
            Background(lipgloss.Color("#0D0D0D")).
            Foreground(lipgloss.Color("#E8E8E8")).
            Bold(true),
        // ... etc
    }
}

func LightTheme() Theme {
    return Theme{
        Background: lipgloss.Color("#FFFFFF"),
        Surface:    lipgloss.Color("#F5F5F5"),
        Brand:      lipgloss.Color("#C2562F"),
        // ... light palette counterparts
    }
}

type Manager struct {
    current Theme
    mode    Mode
}

func NewManager(mode Mode) *Manager {
    m := &Manager{mode: mode}
    m.resolve()
    return m
}

func (m *Manager) resolve() {
    switch m.mode {
    case ModeDark:
        m.current = DarkTheme()
    case ModeLight:
        m.current = LightTheme()
    case ModeAuto:
        if lipgloss.HasDarkBackground(os.Stdin, os.Stdout) {
            m.current = DarkTheme()
        } else {
            m.current = LightTheme()
        }
    }
}

func (m *Manager) Cycle() Mode {
    // dark → light → auto
    switch m.mode {
    case ModeDark:
        m.mode = ModeLight
    case ModeLight:
        m.mode = ModeAuto
    case ModeAuto:
        m.mode = ModeDark
    }
    m.resolve()
    return m.mode
}

func (m *Manager) Current() Theme {
    return m.current
}
```

**On `tea.BackgroundColorMsg`:** Bubble Tea v2 can deliver `tea.BackgroundColorMsg` with the terminal's actual background color. This is more accurate than `lipgloss.HasDarkBackground` which just checks the color profile. However, `tea.BackgroundColorMsg` arrives asynchronously after `Init()`, so the theme should start with a default (dark) and update on receipt. [CITED: charmbracelet/lipgloss docs](https://github.com/charmbracelet/lipgloss?tab=readme-ov-file#with-bubble-tea)

### Pattern 5: First-Run State Machine

**What:** A linear state machine with a `SetupStep` enum for multi-step first-run flow. Each step advances on completion.

```go
// firstrun.go
type SetupStep int

const (
    StepWelcome SetupStep = iota
    StepProviderSelect
    StepKeyInput
    StepKeyValidate
    StepKeychainPrompt
    StepComplete
)

type firstRunScreenState struct {
    step            SetupStep
    cursor          int
    providers       []string // "openrouter", "zen"
    selected        map[int]bool
    apiKeyInput     textinput.Model
    validationErr   string
    validating      bool
}

func (m *AppState) updateFirstRun(msg tea.Msg) (tea.Model, tea.Cmd) {
    switch msg := msg.(type) {
    case tea.KeyPressMsg:
        switch m.firstRun.step {
        case StepWelcome:
            if msg.String() == "enter" {
                m.firstRun.step = StepProviderSelect
            }
        case StepProviderSelect:
            // Handle up/down to select provider(s)
            // Enter advances to key input
        case StepKeyInput:
            // Keyboard input for API key
            // Enter triggers key validation via health check
        }
    case KeyValidationMsg:
        m.firstRun.validating = false
        if msg.Valid {
            m.firstRun.step = StepKeychainPrompt
        } else {
            m.firstRun.validationErr = msg.Error
        }
    }

    return m, nil
}
```

### Pattern 6: Key Binding Approach

**What:** Use `tea.KeyPressMsg` type switch for key handling. Global keys (Ctrl+C, Esc) are caught in the top-level `AppState.Update()` before delegation. Screen-specific keys are handled in per-screen update functions.

```go
// Global keys pattern
func (m *AppState) Update(msg tea.Msg) (tea.Model, tea.Cmd) {
    switch msg := msg.(type) {
    case tea.KeyPressMsg:
        switch msg.String() {
        case "ctrl+c":
            // Graceful shutdown — save state, then quit
            return m, tea.Quit
        case "ctrl+\\":  // alternative quit like SIGQUIT
            return m, tea.Quit
        }
        // If not a global key, fall through to screen delegate
    }
    // ... screen delegation
}
```

**Note:** Bubble Tea v2 uses `tea.KeyPressMsg` not `tea.KeyMsg`. The v1 `tea.KeyMsg` matched both press and release — v2 separates them. Key strings like `" "` for space work the same.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Scrollable content area | Custom scroll logic | `charm.land/bubbles/v2/viewport` | Handles scrollback, mouse wheel, resize, high-performance mode |
| Multi-line text input | Custom text editor | `charm.land/bubbles/v2/textarea` | Full unicode support, cursor, paste, placeholder, char limit, blinking cursor |
| Activity indicator | Custom spinner animation | `charm.land/bubbles/v2/spinner` | 10+ frame sets, tea.Tick integration, framerate control |
| Terminal color detection | Hardcoded dark/light | `lipgloss.HasDarkBackground()` | Cross-platform, auto-detects terminal background |
| Markdown rendering | Regex parsing | `charm.land/glamour/v2` | Full CommonMark, syntax highlighting, custom style sheets |
| Declarative key matching | Manual string compare | `charm.land/bubbles/v2/key` | Key chord detection, platform-aware key maps |

**Key insight:** The Charm v2 stack is the standard for Go TUI development. The bubbles components handle complex terminal interaction (viewport scrolling, textarea input, spinner timing) that is extremely difficult to get right from scratch. Always prefer the Charm component when one exists.

## Common Pitfalls

### Pitfall 1: Goroutine → State Mutation (Race Condition)
**What goes wrong:** A background goroutine directly mutates `AppState` fields (e.g., `m.repl.messages = append(...)`), causing a data race or deadlock.
**Why it happens:** Bubble Tea is single-threaded. Only `Update()` may modify state.
**How to avoid:** All background work must emit messages (via `tea.Cmd`) that `Update()` processes. Use `tea.Tick` for timers, `tea.Batch` for parallel commands, and custom `tea.Msg` types for async results. [VERIFIED: bubbletea README]
**Warning signs:** `go test -race` failures, intermittent crashes under load.

### Pitfall 2: View() Frame Budget Exceeded
**What goes wrong:** `View()` takes > 16ms (60fps budget), causing visible jank.
**Why it happens:** Re-rendering glamour markdown on every frame, doing heavy string concatenation, or computing layouts from scratch each call.
**How to avoid:** Cache glamour-rendered messages; set them when content changes, not on every `View()` call. Use `strings.Builder` for concatenation. Pre-compute lipgloss styles once in `Init()`. [CITED: ROADMAP.md risk register]
**Warning signs:** Visible flickering, slow typing response, high CPU during idle.

### Pitfall 3: Incorrect Import Paths (v1 vs v2)
**What goes wrong:** Using old `github.com/charmbracelet/bubbletea` imports with v2 API patterns fails with compile errors.
**Why it happens:** Charm moved from `github.com/...` to `charm.land/.../v2` in Feb 2026. The v2 API is incompatible (View returns `tea.View` not `string`; startup options changed).
**How to avoid:** Use `charm.land/bubbletea/v2`, `charm.land/lipgloss/v2`, `charm.land/bubbles/v2`, `charm.land/glamour/v2` everywhere. Bump `go 1.22` to `go 1.25` in go.mod. [VERIFIED: pkg.go.dev]

### Pitfall 4: Ticker Not Re-scheduled
**What goes wrong:** Health check fires only once (the first tick) and never again.
**Why it happens:** `tea.Tick` fires exactly once per invocation. For repeating intervals, you must return another `tea.Tick` command from `Update()`.
**How to avoid:** Always return a `tea.Tick` command when handling a tick message. The pattern is: handle tick → do work → `return m, healthTick()` (the next tick). [CITED: bubbletea commands.go]

### Pitfall 5: Ignoring WindowSizeMsg
**What goes wrong:** Components render at wrong sizes after terminal resize, or viewport is never initialized.
**Why it happens:** Bubble Tea delivers `tea.WindowSizeMsg` once at startup and on every resize. Many components (viewport, textarea) need explicit width/height set. Before the first `WindowSizeMsg`, width/height are 0.
**How to avoid:** Use a `ready` bool, initialize sub-components in the first `WindowSizeMsg` handler, update them in subsequent ones. Only render non-empty content when `ready == true`. [CITED: bubbles viewport example]

### Pitfall 6: Bubble Tea v2 Model Interface Requires `View() tea.View`
**What goes wrong:** `View()` returning a `string` doesn't compile with Bubble Tea v2's `Model` interface.
**Why it happens:** v2 changed the interface.
**How to avoid:** Always return `tea.NewView(contentString)` or `tea.View{Content: contentString}` from `View()`.

## Code Examples

### Example 1: Complete Bubble Tea v2 Minimal App

```go
// Source: [CITED: charmbracelet/bubbletea v2 release notes]
package main

import (
    "fmt"
    "os"
    "charm.land/bubbletea/v2"
)

type model struct {
    count int
}

func (m model) Init() tea.Cmd {
    return nil
}

func (m model) Update(msg tea.Msg) (tea.Model, tea.Cmd) {
    switch msg := msg.(type) {
    case tea.KeyPressMsg:
        switch msg.String() {
        case "up":
            m.count++
        case "down":
            m.count--
        case "q", "ctrl+c":
            return m, tea.Quit
        }
    }
    return m, nil
}

func (m model) View() tea.View {
    v := tea.NewView(fmt.Sprintf("Count: %d\n\n↑/↓ to change, q to quit.", m.count))
    v.AltScreen = true
    return v
}

func main() {
    p := tea.NewProgram(model{})
    if _, err := p.Run(); err != nil {
        fmt.Println("Error:", err)
        os.Exit(1)
    }
}
```

### Example 2: Screen Routing with Enum

```go
// Source: Derived from [CITED: deepwiki multi-view example], adapted for v2

type Screen int

const (
    ScreenHome Screen = iota
    ScreenSettings
)

type model struct {
    screen   Screen
    width    int
    height   int
    home     homeModel
    settings settingsModel
}

func (m model) Init() tea.Cmd {
    return nil
}

func (m model) Update(msg tea.Msg) (tea.Model, tea.Cmd) {
    switch msg := msg.(type) {
    case tea.WindowSizeMsg:
        m.width = msg.Width
        m.height = msg.Height
    case tea.KeyPressMsg:
        if msg.String() == "ctrl+c" {
            return m, tea.Quit
        }
    case changeScreenMsg:
        m.screen = msg.screen
        return m, nil
    }

    switch m.screen {
    case ScreenHome:
        return m.updateHome(msg)
    case ScreenSettings:
        return m.updateSettings(msg)
    }
    return m, nil
}

func (m model) View() tea.View {
    var content string
    switch m.screen {
    case ScreenHome:
        content = m.homeView()
    case ScreenSettings:
        content = m.settingsView()
    }
    return tea.NewView(content)
}
```

### Example 3: Background Health Check via tea.Tick

```go
// Source: [CITED: charmbracelet/bubbletea Tick docs]
type HealthTick time.Time

func doHealthTick() tea.Cmd {
    return tea.Tick(60*time.Second, func(t time.Time) tea.Msg {
        return HealthTick(t)
    })
}

func (m model) Update(msg tea.Msg) (tea.Model, tea.Cmd) {
    switch msg := msg.(type) {
    case HealthTick:
        // Perform health check (non-blocking, runs in the Cmd goroutine)
        return m, tea.Sequence(
            func() tea.Msg {
                result := m.provider.HealthCheck(m.ctx)
                return HealthResultMsg(result)
            },
            doHealthTick(), // re-schedule next tick
        )
    case HealthResultMsg:
        m.lastHealth = HealthStatus(msg)
        return m, nil
    }
    return m, nil
}
```

### Example 4: Testing Bubble Tea Models

```go
// Source: [CITED: Gentleman-Programming Go testing patterns + bubbletea deepwiki testing]

// Test that a keypress updates the model state
func TestModel_EnterKey(t *testing.T) {
    m := &AppState{
        activeScreen: ScreenREPL,
        repl:         newREPLScreen(),
        theme:        theme.NewManager(theme.ModeDark),
    }

    // Simulate typing "hello" and pressing enter
    m.repl.textarea.SetValue("hello")
    m.repl.textarea, _ = m.repl.textarea.Update(tea.KeyPressMsg{
        Type: tea.KeyEnter,
    })

    // Wait — in v2, we can also directly call the model's update
    newModel, _ := m.Update(tea.KeyPressMsg{
        Type: tea.KeyEnter,
    })

    updated := newModel.(*AppState)

    // Assert message was added
    if len(updated.repl.messages) != 1 {
        t.Errorf("expected 1 message, got %d", len(updated.repl.messages))
    }
}

// Test screen transition
func TestScreenTransition(t *testing.T) {
    tests := []struct {
        name          string
        startScreen   AppScreen
        msg           tea.Msg
        expectScreen  AppScreen
    }{
        {
            name:         "first run complete transitions to REPL",
            startScreen:  ScreenFirstRun,
            msg:          ScreenChangeMsg{Screen: ScreenREPL},
            expectScreen: ScreenREPL,
        },
    }

    for _, tt := range tests {
        t.Run(tt.name, func(t *testing.T) {
            m := &AppState{activeScreen: tt.startScreen}
            newModel, _ := m.Update(tt.msg)
            updated := newModel.(*AppState)

            if updated.activeScreen != tt.expectScreen {
                t.Errorf("screen = %d, want %d", updated.activeScreen, tt.expectScreen)
            }
        })
    }
}

// Test that View() returns content (not empty)
func TestView_NotEmpty(t *testing.T) {
    m := &AppState{
        ready:       true,
        width:       120,
        height:      40,
        activeScreen: ScreenREPL,
        repl:        newREPLScreen(),
        theme:       theme.NewManager(theme.ModeDark),
    }

    v := m.View()

    if v.Content == "" {
        t.Error("expected non-empty view")
    }
}
```

### Example 5: Theme Manager with Bubble Tea Background Detection

```go
// Source: [CITED: charmbracelet/lipgloss v2 advanced color usage]

// In app.go Init():
func (m *AppState) Init() tea.Cmd {
    // Request background color from terminal (async)
    // Then set styles when BackgroundColorMsg arrives
    return nil
}

// In app.go Update():
case tea.BackgroundColorMsg:
    // msg.IsDark() tells us if terminal has dark background
    if m.theme.Mode() == theme.ModeAuto {
        m.theme.SetDarkBackground(msg.IsDark())
    }
    return m, nil

// In theme.go, handle the async background detection:
func (m *Manager) SetDarkBackground(isDark bool) {
    if m.mode != ModeAuto {
        return
    }
    if isDark {
        m.current = DarkTheme()
    } else {
        m.current = LightTheme()
    }
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `View() string` | `View() tea.View` | Bubble Tea v2.0.0 (Feb 2026) | Return a struct, not a string; declare AltScreen, MouseMode as fields |
| `github.com/charmbracelet/...` | `charm.land/.../v2` | Feb–Mar 2026 | All import paths changed; incompatible module paths |
| `lipgloss.Renderer` with `NewStyle()` | Plain `lipgloss.NewStyle()` | Lipgloss v2.0.0 (Feb 2026) | Style is a value type; no global renderer |
| `viewport.New(w, h)` | `viewport.New(viewport.WithWidth(w), viewport.WithHeight(h))` | Bubbles v2.0.0 (Feb 2026) | Options pattern replaces positional args |
| `spinner.NewModel()`, `spinner.Tick()` | `spinner.New()`, `model.Tick()` | Bubbles v2.0.0 (Feb 2026) | Method replaces package-level function |
| `tea.KeyMsg` (press + release) | `tea.KeyPressMsg` (press only) | Bubble Tea v2.0.0 (Feb 2026) | More precise key handling; releases handled separately |

**Deprecated/outdated:**
- **Bubble Tea v1** (`github.com/charmbracelet/bubbletea`): All import paths, `View() string`, startup options like `tea.WithAltScreen()`. Must migrate to v2 for new projects.
- **Lipgloss v1** (`github.com/charmbracelet/lipgloss`): `Renderer` type, `AdaptiveColor` struct, `TerminalColor` interface. v2 is a complete rewrite.
- **Bubbles v1** (`github.com/charmbracelet/bubbles`): Component constructors with positional int args, `spinner.Tick()` package function.

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | Go 1.26.3 satisfies Bubble Tea v2's Go 1.25 requirement | Standard Stack | Low — verified via `go version` |
| A2 | The project wasn't already using Bubble Tea v2 dependencies | Standard Stack | Low — go.mod has zero dependencies |
| A3 | `go.mod` can be bumped from `go 1.22` to `go 1.25` without other issues | Standard Stack | Low — Go is backwards compatible |

**If this table is empty:** All claims in this research were verified or cited — no user confirmation needed.

## Open Questions (RESOLVED)

1. **Which Bubble Tea v2 version to pin?**
   - What we know: v2.0.6 is latest stable as of 2026-04-16.
   - What's unclear: Whether there are any bugs in v2.0.x that affect our use case.
   - Recommendation: Pin `v2.0.6` (latest stable). Can upgrade later if needed.

2. **Glamour integration deferred to Phase 3?**
   - What we know: Phase 2 builds the REPL skeleton, Phase 3 adds markdown rendering.
   - What's unclear: Whether viewport content should use plain text now and glamour later.
   - Recommendation: Use plain text in Phase 2 viewport; glamour integration belongs in Phase 3.

3. **How to handle config detection for first-run?**
   - What we know: Config layer is built in Phase 5, but first-run needs to check if config exists.
   - What's unclear: Whether first-run should read config files directly or use a config package stub.
   - Recommendation: Add a simple `os.Stat("~/.m31a/config.toml")` check in firstrun.go for now; Phase 5 will replace with proper config package.

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| Go compiler | All | ✓ | 1.26.3 | — |
| Charm v2 packages (bubbletea, lipgloss, bubbles, glamour) | All | Will be installed via `go get` | latest | — |

**Missing dependencies with no fallback:** none
**Missing dependencies with fallback:** none

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Go standard testing (`testing.T`) |
| Config file | none — standard Go `_test.go` files |
| Quick run command | `go test ./internal/tui/... -short` |
| Full suite command | `go test -race -cover ./internal/tui/...` |

### Phase Requirements → Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| P2.1 | AppState implements tea.Model (Init, Update, View) | unit | `go test ./internal/tui/... -run TestApp_Interface` | ❌ Wave 0 |
| P2.2 | Theme cycles dark→light→auto | unit | `go test ./internal/tui/theme/...` | ❌ Wave 0 |
| P2.3 | REPL viewport displays messages | unit | `go test ./internal/tui/... -run TestREPL` | ❌ Wave 0 |
| P2.4 | Health check ticker fires at interval | unit | `go test ./internal/tui/... -run TestHealth` | ❌ Wave 0 |
| P2.5 | First-run completes and transitions to REPL | unit | `go test ./internal/tui/... -run TestFirstRun` | ❌ Wave 0 |
| P2.7 | Screen routing changes active screen | unit | `go test ./internal/tui/... -run TestScreenRouting` | ❌ Wave 0 |

### Sampling Rate
- **Per task commit:** `go test ./internal/tui/... -short`
- **Per wave merge:** `go test -race -cover ./internal/tui/...`
- **Phase gate:** Full suite green before `/gsd-verify-work`

### Wave 0 Gaps
- [ ] `internal/tui/app_test.go` — covers AppState tea.Model interface compliance
- [ ] `internal/tui/theme/theme_test.go` — covers theme cycling, color values
- [ ] `internal/tui/repl_test.go` — covers message entry, viewport update
- [ ] `internal/tui/health_test.go` — covers tick scheduling, health result handling
- [ ] `internal/tui/firstrun_test.go` — covers all step transitions

## Security Domain

> Security is minimal for Phase 2 (TUI Foundation only). The first-run screen handles API key input, which is a security-relevant surface.

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | Partial (first-run API key entry only) | Masked input field; no plaintext display |
| V5 Input Validation | Yes | Validate API key format before sending health check |
| V8 Data Protection | Yes | API key never stored in plaintext by TUI |

### Known Threat Patterns for {stack}

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| API key displayed in TUI output | Information Disclosure | Masked input (textarea password mode? or use textinput with echo character) |
| API key in panic stack trace | Information Disclosure | Global panic handler in Bubble Tea that redacts keys |

## Sources

### Primary (HIGH confidence)
- [CITED: charmbracelet/bubbletea v2 README](https://github.com/charmbracelet/bubbletea) — Model interface, Init/Update/View patterns
- [CITED: charmbracelet/lipgloss v2 upgrade guide](https://github.com/charmbracelet/lipgloss/blob/v2.0.0/UPGRADE_GUIDE_V2.md) — Adaptive color system, LightDark helper
- [CITED: charmbracelet/bubbles v2 upgrade guide](https://github.com/charmbracelet/bubbles/commit/24081b3590e746db4efa2ec09e31a85e2c078427) — viewport/textarea/spinner v2 API changes
- [CITED: Bubble Tea v2 upgrade guide](https://charmbracelet-bubbletea-43.mintlify.app/migration/v2-upgrade-guide) — View() tea.View, field declarations
- [CITED: charmbracelet/bubbletea commands.go](https://github.com/charmbracelet/bubbletea/blob/main/commands.go) — Tick, Every, Batch, Sequence patterns

### Secondary (MEDIUM confidence)
- [CITED: deepwiki multi-view example](https://deepwiki.com/charmbracelet/bubbletea/6.2-multi-view-application-example) — Screen routing via state-driven delegation
- [CITED: deepwiki testing guide](https://deepwiki.com/charmbracelet/bubbletea/7.2-testing-guidelines) — Direct model testing patterns
- [CITED: Gentleman-Programming go-testing SKILL.md](https://raw.githubusercontent.com/Gentleman-Programming/Gentleman.Dots/main/skills/go-testing/SKILL.md) — Bubble tea model testing, golden file patterns

### Tertiary (LOW confidence)
- None — all findings verified against official Charm docs or source

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — Charm v2 is the only correct choice for Go TUI in 2026
- Architecture: HIGH — Screen routing via enum + delegation is standard Elm Architecture pattern
- Pitfalls: HIGH — All verified against official docs and common community experience

**Research date:** 2026-05-27
**Valid until:** 2026-07-01 (Charm v2 stack is stable; re-check if major breaking releases happen)
