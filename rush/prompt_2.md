# ROLE

You are the **Senior Go Engineer specializing in Terminal UI development with Bubble Tea**
for M31A. You are executing Phase 2 of a multi-phase build plan. Phases 0 and 1 are complete —
all interfaces, types, provider clients, and project scaffolding exist. Your job is to implement
the TUI Foundation.

Do not invent features. Do not add packages not listed. Do not write workflow logic, tool
implementations, or message rendering (those are Phase 3). You are building the TUI shell —
the application skeleton, screen routing, theme system, REPL view, and first-run flow.
Deviation = breakage downstream.

---

# PROJECT IDENTITY

M31A is a terminal-based AI coding assistant written in Go 1.22+.
- Module path: github.com/eshanized/M31A
- Binary: single static binary (CGO_ENABLED=0)
- UI: Bubble Tea + Lipgloss + Bubbles + Glamour (Charm stack)
- License: MIT — NO telemetry, NO vendor lock-in, NO paid tiers

**Phase 0 output:** All interfaces and types in `internal/` are defined.
**Phase 1 output:** Provider abstraction layer complete (OpenRouter + Zen clients, cache, SSE parser, registry, fallback, 40 tests).

Read these files before starting (they already exist):
- `internal/provider/interface.go` — LLMProvider interface, ChatRequest, ToolDefinition
- `internal/provider/registry.go` — Registry with active provider management
- `internal/provider/cache.go` — ModelCache
- `internal/provider/fallback.go` — Auto-fallback logic
- `internal/types/types.go` — All shared types
- `internal/types/constants.go` — All constants (HealthCheckInterval, etc.)
- `internal/errors/errors.go` — All sentinel errors
- `internal/log/log.go` — Structured logger

---

# PHASE 2 MISSION

Implement the TUI Foundation:
1. Bubble Tea application skeleton (`internal/tui/app.go`)
2. Theme system (`internal/tui/theme/theme.go`)
3. REPL screen (`internal/tui/repl.go`)
4. Header component with model badge, context bar, connection status
5. Health check ticker integrated with provider layer
6. First-run setup screen (`internal/tui/firstrun.go`)
7. Screen routing between REPL, FirstRun, and placeholder screens for future phases
8. Unit tests for all components

---

# DELIVERABLES

Create EXACTLY these files. No more, no less.

## 1. internal/tui/app.go

Top-level Bubble Tea application implementing `tea.Model`.

```go
package tui

import (
    "tea"
    "github.com/eshanized/M31A/internal/provider"
    "github.com/eshanized/M31A/internal/types"
)

// Screen identifies the active TUI view.
type Screen int

const (
    ScreenFirstRun Screen = iota
    ScreenREPL
    ScreenModelSelector   // Placeholder for Phase 7
    ScreenSettings         // Placeholder for Phase 7
    ScreenResume           // Placeholder for Phase 5
    ScreenPermission       // Placeholder for Phase 3
)

// AppMsg is the top-level message type for state transitions.
type AppMsg struct {
    Screen    Screen
    Health    *HealthUpdateMsg
    Provider  *ProviderSwitchMsg
    InitError error
}

// AppState holds the entire application state.
type AppState struct {
    // Core
    screen         Screen
    version        string
    initialized    bool

    // Provider System
    registry       *provider.Registry
    activeProvider string
    activeModel    *types.ModelInfo    // Currently selected model metadata
    contextUsed    int64               // Estimated tokens used in current session
    contextTotal   int64               // From active model's ContextLength

    // UI State
    width          int
    height         int
    focused        bool                // Is the TUI focused (vs. running in background)?
    lastActivity   time.Time           // For idle detection
    currentOperation string            // "Thinking...", "Connecting...", etc.

    // Health
    healthStatus   types.HealthStatus

    // Sub-models (each screen has its own Bubble Tea model)
    firstRunModel  *FirstRunModel
    replModel      *ReplModel

    // Configuration
    apiKey         string              // Resolved API key (from env, keychain, or config)
    configPath     string              // Path to config.toml
}

// NewApp creates the top-level AppState.
func NewApp(version string, registry *provider.Registry, apiKey string) *AppState

// Init is called when the Bubble Tea program starts.
func (m *AppState) Init() tea.Cmd

// Update processes all incoming messages.
func (m *AppState) Update(msg tea.Msg) (tea.Model, tea.Cmd)

// View renders the current screen.
func (m *AppState) View() string
```

### Implementation details:

**Update() message handling:**
- `tea.WindowSizeMsg`: Set `m.width`, `m.height`, propagate to sub-models
- `tea.KeyMsg`: Route to active screen's model. Handle global keys:
  - `ctrl+c`: If idle, quit. If in operation, cancel and return to idle.
  - `ctrl+l`: Clear screen (future — for now, no-op)
- `HealthUpdateMsg`: Update `m.healthStatus`, no screen change
- `ProviderSwitchMsg`: Update `m.activeProvider`, `m.activeModel`, propagate to sub-models
- `AppMsg{Screen: ...}`: Switch active screen, initialize new screen model

**View() rendering:**
- Route to active screen's View() method
- Add a 1-line status bar at the bottom (future — for now, empty)
- Handle terminal size too small: render "Terminal too small" message

**Key bindings (global):**
- `ctrl+c`: Quit (with confirmation if in active operation)
- `ctrl+d`: Quit (if input is empty)

**Bubble Tea model protocol:**
- Use `github.com/charmbracelet/bubbletea` with standard imports
- All imports must use the correct package path

---

## 2. internal/tui/theme/theme.go

Complete theme system with dark and light palettes.

```go
package theme

import (
    "github.com/charmbracelet/lipgloss"
)

// Mode identifies the active theme.
type Mode int

const (
    ModeDark Mode = iota
    ModeLight
    ModeAuto         // Detect from terminal background
)

// Theme holds all named Lipgloss styles.
type Theme struct {
    Mode Mode

    // Colors (resolved as lipgloss.Color values)
    Background       lipgloss.Color
    Surface          lipgloss.Color
    SurfaceElevated  lipgloss.Color
    Border           lipgloss.Color
    Brand            lipgloss.Color
    TextPrimary      lipgloss.Color
    TextSecondary    lipgloss.Color
    Thinking         lipgloss.Color
    Success          lipgloss.Color
    Error            lipgloss.Color
    Warning          lipgloss.Color
    CodeBG           lipgloss.Color

    // Pre-built styles
    Header       lipgloss.Style
    ModelBadge   lipgloss.Style
    ContextBar   lipgloss.Style
    StatusLive   lipgloss.Style
    StatusSlow   lipgloss.Style
    StatusOffline lipgloss.Style
    UserBubble   lipgloss.Style
    AssistantBubble lipgloss.Style
    InputArea    lipgloss.Style
    Spinner      lipgloss.Style
    ThinkingBlock lipgloss.Style
    ToolCard     lipgloss.Style
    ToolLabel    map[string]lipgloss.Style  // "bash" -> style, "fileread" -> style, etc.
    SuccessBadge lipgloss.Style
    ErrorBadge   lipgloss.Style
    WarningBadge lipgloss.Style
    ProgressBar  lipgloss.Style
    Modal        lipgloss.Style
    ModalTitle   lipgloss.Style
}

// Dark returns the dark mode theme.
func Dark() Theme

// Light returns the light mode theme.
func Light() Theme

// Auto detects the terminal background and returns the appropriate theme.
func Auto() Theme

// Default returns the default (dark) theme.
func Default() Theme
```

### Color palette (from spec §9.1):

**Dark mode:**
| Token | Value |
|-------|-------|
| Background | `#0D0D0D` |
| Surface | `#1A1A1A` |
| SurfaceElevated | `#242424` |
| Border | `#2E2E2E` |
| Brand | `#D77757` |
| TextPrimary | `#E8EAED` |
| TextSecondary | `#9AA0A6` |
| Thinking | `#8AB4F8` |
| Success | `#81C995` |
| Error | `#F28B82` |
| Warning | `#FDD663` |
| CodeBG | `#2D2D2D` |

**Light mode:**
| Token | Value |
|-------|-------|
| Background | `#FFFFFF` |
| Surface | `#F8F9FA` |
| SurfaceElevated | `#FFFFFF` |
| Border | `#E0E0E0` |
| Brand | `#C45C3A` |
| TextPrimary | `#1F1F1F` |
| TextSecondary | `#5F6368` |
| Thinking | `#1967D2` |
| Success | `#1E8E3E` |
| Error | `#D93025` |
| Warning | `#F9AB00` |
| CodeBG | `#F1F3F4` |

### Pre-built style guidelines:

- **Header**: Bold, Brand foreground, 1 line height
- **ModelBadge**: Small text, SurfaceElevated background, Border border, 2px padding
- **ContextBar**: TextSecondary foreground, updates color at 80% (Warning) and 95% (Error)
- **StatusLive/Slow/Offline**: Respective color foreground, bold
- **UserBubble**: Right-aligned, Surface background tint, 1px padding, 2px border-radius
- **AssistantBubble**: Left-aligned, full width, 2px padding, generous margins
- **InputArea**: Surface background, Border border, 1px padding
- **Spinner**: TextSecondary foreground
- **ThinkingBlock**: SurfaceElevated background, Thinking left border (3px), 1px padding
- **ToolCard**: Border border, 1px padding
- **ToolLabel**: Per-tool color map: Bash=`Warning`, FileRead=`Thinking`, FileWrite=`Brand`, Glob=`TextSecondary`, Grep=`Thinking`
- **SuccessBadge/ErrorBadge/WarningBadge**: Bold, respective color foreground, `[OK]` / `[ERR]` / `[WARN]` format
- **ProgressBar**: Block character fill (`█` for filled, `░` for empty), respective color
- **Modal**: SurfaceElevated background, Border border, centered
- **ModalTitle**: Bold, Brand foreground

---

## 3. internal/tui/repl.go

Main REPL screen implementing `tea.Model`.

```go
package tui

import (
    "tea"
    "github.com/eshanized/M31A/internal/provider"
    "github.com/eshanized/M31A/internal/types"
    "github.com/eshanized/M31A/internal/tui/theme"
)

// ReplModel is the Bubble Tea model for the main REPL screen.
type ReplModel struct {
    theme    theme.Theme
    registry *provider.Registry

    // Header state
    activeProvider string
    activeModel    *types.ModelInfo
    healthStatus   types.HealthStatus
    contextUsed    int64
    contextTotal   int64

    // Message history
    messages []types.Message
    scrollPos int            // Current scroll position (0 = bottom)

    // Input state
    input      string         // Current input buffer
    inputHistory []string     // Previous inputs (for up/down navigation)
    historyPos  int           // Current position in input history (-1 = not navigating)
    placeholder string        // Input placeholder text

    // Operation state
    streaming  bool           // Is the LLM currently streaming a response?
    thinking   bool           // Is the LLM in a thinking state?
    spinner    int            // Current spinner frame (0-9 for ⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏)
    lastStatus string         // Last status line text

    // Viewport
    width      int
    height     int
}

// NewReplModel creates the REPL screen model.
func NewReplModel(theme theme.Theme, registry *provider.Registry) *ReplModel

// Init is called when the REPL screen is activated.
func (m *ReplModel) Init() tea.Cmd

// Update handles REPL-specific messages.
func (m *ReplModel) Update(msg tea.Msg) (tea.Model, tea.Cmd)

// View renders the REPL: header + messages + input + status bar.
func (m *ReplModel) View() string
```

### Implementation details:

**Layout (4 regions):**
```
┌─────────────────────────────────────────┐
│ Header (1 line)                         │  ← Model badge, context bar, health
├─────────────────────────────────────────┤
│                                         │
│ Messages (flex height)                  │  ← Scrollable history
│                                         │
├─────────────────────────────────────────┤
│ Input (3-6 lines)                       │  ← Textarea with placeholder
├─────────────────────────────────────────┤
│ Status (1 line)                         │  ← Current operation / timestamp
└─────────────────────────────────────────┘
```

**Header (1 line):**
```
M31A  [OR] anthropic/claude-3.5-sonnet    12.4K/200K ctx  [LIVE]
```
- Brand: "M31A" in Brand color
- Provider badge: `[OR]` fg(Warning) or `[ZEN]` fg(Thinking)
- Model: active model name, truncated to fit
- Context bar: `used/total` with "ctx" suffix, color-shifts at 80%/95%
- Health: `[LIVE]` fg(Success), `[SLOW]` fg(Warning), `[OFFLINE]` fg(Error)

**Messages area:**
- For now: render a welcome message "Welcome to M31A. Type a goal or /help to begin."
- User messages: right-aligned, Surface background tint
- Assistant messages: left-aligned
- Empty state: center "No messages yet" with placeholder
- Auto-scroll to bottom on new messages
- `pgup`/`pgdn`: scroll up/down
- `up`/`down`: navigate input history (when input is empty)

**Input area:**
- Use `github.com/charmbracelet/bubbles/textarea` for multi-line input
- Placeholder: "Type a goal or /help to begin."
- Character count in bottom-right of input area
- `enter`: submit message (emit a message that app.go handles — for now, just echo it as an assistant message)
- `shift+enter`: newline
- `esc`: clear input

**Status bar (1 line):**
- When idle: last message timestamp
- When streaming: "Thinking..." with spinner (⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏ at 10fps)
- For now: static text "Ready"

**Key bindings:**
| Key | Action |
|-----|--------|
| `enter` | Send message (if input non-empty) |
| `shift+enter` | Newline in input |
| `ctrl+c` | Quit (or cancel streaming) |
| `ctrl+l` | Clear screen (future) |
| `ctrl+m` | Open model selector (future — no-op for now) |
| `ctrl+p` | Toggle plan mode (future — no-op for now) |
| `pgup/pgdn` | Scroll message history |
| `up/down` | Navigate input history (when input empty) |
| `esc` | Clear input |

---

## 4. internal/tui/header.go

Header component — renders the top line of the REPL screen.

```go
package tui

import (
    "github.com/charmbracelet/lipgloss"
    "github.com/eshanized/M31A/internal/types"
    "github.com/eshanized/M31A/internal/tui/theme"
)

// Header renders the top line: brand + model badge + context bar + health.
func RenderHeader(t theme.Theme, provider string, model *types.ModelInfo,
    health types.HealthStatus, contextUsed int64, contextTotal int64, width int) string
```

### Implementation details:

- Build the header as a single line, truncate with `...` if too wide
- Provider badge: `[OR]` or `[ZEN]` with respective color
- Model name: truncate to fit available space
- Context bar: format as `X.XK/YK ctx` (e.g., "12.4K/200K ctx")
  - < 80%: TextSecondary color
  - 80-95%: Warning color
  - > 95%: Error color
- Health: `[LIVE]` / `[SLOW]` / `[OFFLINE]` with respective color
- Right-align health and context bar, left-align brand + model
- Use `lipgloss.NewStyle().Width().Align().Render()` for layout

---

## 5. internal/tui/health.go

Health check ticker — background goroutine that polls provider health.

```go
package tui

import (
    "context"
    "time"

    tea "github.com/charmbracelet/bubbletea"
    "github.com/eshanized/M31A/internal/provider"
    "github.com/eshanized/M31A/internal/types"
)

// HealthUpdateMsg is emitted by the health check ticker.
type HealthUpdateMsg struct {
    Status types.HealthStatus
}

// ProviderSwitchMsg is emitted when auto-fallback occurs.
type ProviderSwitchMsg struct {
    Provider string
    Model    *types.ModelInfo
    Reason   string
}

// HealthCheckTicker starts a background health check loop.
// Returns a tea.Cmd that periodically emits HealthUpdateMsg.
func HealthCheckTicker(ctx context.Context, registry *provider.Registry,
    activeProvider string, interval time.Duration) tea.Cmd
```

### Implementation details:

- Background goroutine runs on `interval` (default 60s from `types.HealthCheckInterval`)
- Calls `registry.Get(activeProvider).HealthCheck(ctx)` each interval
- Adapts interval: if rate-limit detected, increase to 120s; if 401, stop polling
- Emits `HealthUpdateMsg` to Bubble Tea update loop via channel
- Non-blocking: never interrupt active streaming
- Returns immediately when context is cancelled

---

## 6. internal/tui/firstrun.go

First-run setup screen — shown when no API key is configured.

```go
package tui

import (
    tea "github.com/charmbracelet/bubbletea"
    "github.com/eshanized/M31A/internal/tui/theme"
)

// FirstRunState tracks the first-run flow.
type FirstRunState int

const (
    FirstRunWelcome FirstRunState = iota
    FirstRunProviderSelect
    FirstRunKeyInput
    FirstRunValidating
    FirstRunKeychainPrompt
    FirstRunComplete
)

// FirstRunModel is the Bubble Tea model for the first-run screen.
type FirstRunModel struct {
    theme       theme.Theme
    state       FirstRunState
    selectedProvider string  // "openrouter" | "zen" | "both" | "skip"
    apiKeyInput string
    validateError string
    keychainChoice bool      // true = store in OS keychain
    width         int
    height        int
}

// NewFirstRunModel creates the first-run screen model.
func NewFirstRunModel(theme theme.Theme) *FirstRunModel

// Init is called when the first-run screen is activated.
func (m *FirstRunModel) Init() tea.Cmd

// Update handles first-run-specific messages.
func (m *FirstRunModel) Update(msg tea.Msg) (tea.Model, tea.Cmd)

// View renders the first-run screen.
func (m *FirstRunModel) View() string
```

### Implementation details:

**Screen flow:**

```
┌─────────────────────────────────────────────────────────────┐
│  Welcome to M31A                                            │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  M31A needs an API key to connect to an LLM gateway.        │
│  Choose a provider:                                         │
│                                                             │
│  [1] OpenRouter  — 400+ models, pay-per-use                 │
│      Get a key: https://openrouter.ai/keys                  │
│                                                             │
│  [2] OpenCode Zen — Curated models, flat pricing            │
│      Get a key: https://opencode.ai/zen                     │
│                                                             │
│  [3] Both — Use OpenRouter primary, Zen as fallback         │
│                                                             │
│  [4] Skip — Run without a key (limited functionality)       │
│                                                             │
│  Enter your key(s):                                         │
│  > sk-or-...                                                │
│                                                             │
│  Store key securely in OS keychain? [Y/n]                   │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

- Provider selection: user types `1`, `2`, `3`, or `4`
- Key input: user types/pastes API key
- Validation: call `registry.Get(provider).HealthCheck(ctx)` — if 200, key is valid
- Invalid key: show inline error "Invalid API key. Try again."
- Keychain prompt: `Y` = store in keychain (for now, just set a flag — actual keychain implementation is Phase 5)
- Skip: set state to FirstRunComplete, transition to REPL with banner
- On complete: emit `AppMsg{Screen: ScreenREPL}` to switch to REPL

**For Phase 2:** Key validation should attempt a real health check against the provider.
If the user provides a key and it validates, store it in the `AppState.apiKey` field.
Keychain storage is deferred to Phase 5 — just set the flag and note it for later.

---

## 7. internal/tui/statusbar.go

Status bar component — renders the bottom line of the REPL screen.

```go
package tui

import (
    "time"

    "github.com/eshanized/M31A/internal/tui/theme"
)

// RenderStatusBar renders the 1-line status bar at the bottom of the REPL.
func RenderStatusBar(t theme.Theme, operation string, lastActivity time.Time, width int) string
```

### Implementation details:

- When idle: show "Ready · Last activity: HH:MM:SS"
- When streaming: show spinner + "Thinking..." or "Executing <tool>..."
- Right-align timestamp
- Left-align operation text
- Truncate with `...` if too wide

---

## 8. internal/tui/app_test.go

Tests for the top-level app state.

```go
package tui

import (
    "testing"
)

func TestNewApp_DefaultState(t *testing.T)
func TestAppState_UpdateWindowSize(t *testing.T)
func TestAppState_UpdateQuit(t *testing.T)
func TestAppState_View_TooSmall(t *testing.T)
```

Use `tea.NewProgram` with a test buffer, or test `Update()` directly by sending `tea.Msg` values.

---

## 9. internal/tui/theme/theme_test.go

Tests for the theme system.

```go
package theme

import (
    "testing"
)

func TestDark_AllColorsSet(t *testing.T)
func TestLight_AllColorsSet(t *testing.T)
func TestDark_BrandColor(t *testing.T)
func TestLight_BrandColor(t *testing.T)
func TestToolLabel_BashIsWarning(t *testing.T)
func TestToolLabel_FileReadIsThinking(t *testing.T)
```

Verify that all color values match the spec palette. Verify that pre-built styles
are non-zero (i.e., they were initialized).

---

## 10. internal/tui/repl_test.go

Tests for the REPL screen.

```go
package tui

import (
    "testing"
)

func TestNewReplModel_DefaultState(t *testing.T)
func TestReplModel_UpdateWindowSize(t *testing.T)
func TestReplModel_UpdateEnterSends(t *testing.T)
func TestReplModel_UpdateEscClearsInput(t *testing.T)
func TestReplModel_UpdatePageDown(t *testing.T)
func TestRenderHeader_FitsWidth(t *testing.T)
func TestRenderHeader_TruncatesModelName(t *testing.T)
func TestRenderHeader_ContextBarColors(t *testing.T)
func TestRenderStatusBar_Idle(t *testing.T)
```

---

## 11. internal/tui/firstrun_test.go

Tests for the first-run screen.

```go
package tui

import (
    "testing"
)

func TestNewFirstRunModel_DefaultState(t *testing.T)
func TestFirstRunModel_UpdateProviderSelect(t *testing.T)
func TestFirstRunModel_UpdateKeyInput(t *testing.T)
```

---

## 12. internal/tui/health_test.go

Tests for the health check ticker.

```go
package tui

import (
    "testing"
)

func TestHealthCheckTicker_EmitsHealthUpdate(t *testing.T)
func TestHealthCheckTicker_AdaptsOnRateLimit(t *testing.T)
func TestHealthCheckTicker_StopsOnAuthFail(t *testing.T)
```

These tests are tricky because they involve time-based polling. Use a short interval
(10ms) in tests and mock the provider's HealthCheck method.

---

# EXECUTION ORDER

Execute tasks in this strict order:

1.  Read existing files: `internal/provider/` (all), `internal/types/` (all), `internal/errors/errors.go`, `internal/log/log.go`
2.  Create `internal/tui/theme/theme.go`
3.  Create `internal/tui/header.go`
4.  Create `internal/tui/statusbar.go`
5.  Create `internal/tui/health.go`
6.  Create `internal/tui/firstrun.go`
7.  Create `internal/tui/repl.go`
8.  Create `internal/tui/app.go`
9.  Create all test files
10. Run: `go mod tidy` — will add Charm library dependencies
11. Run: `go build ./...` — MUST succeed
12. Run: `go vet ./...` — MUST pass
13. Run: `go test -race ./internal/tui/...` — MUST pass all tests
14. Run: `CGO_ENABLED=0 go build -o m31a ./cmd/m31a` — binary must build
15. Run: `./m31a` — must print version and exit (no TUI launch yet — app.go is not wired into main.go)
16. Create `walkthrough_2.md`

**Note:** Do NOT wire the TUI into `cmd/m31a/main.go` yet. Phase 2 only produces the TUI components.
The binary should still print version and exit as in Phase 0. The TUI will be wired in a later phase.

---

# HARD CONSTRAINTS

- `go build ./...` MUST succeed with zero errors
- `go vet ./...` MUST produce zero warnings
- `go test -race ./internal/tui/...` MUST pass all tests
- You MUST NOT implement message rendering pipeline (Phase 3)
- You MUST NOT implement workflow screens (Phase 6)
- You MUST NOT implement model selector UI (Phase 7)
- You MUST NOT implement settings screen (Phase 7)
- You MUST NOT wire the TUI into main.go — the binary should still print version and exit
- All Bubble Tea models must properly implement the tea.Model interface
- Theme colors MUST match the spec palette exactly
- Header MUST render as a single line (no wrapping)
- HTTP client in health check MUST have 30s dial timeout only
- `walkthrough_2.md` MUST contain actual test output, not placeholder text

---

# WHAT SUCCESS LOOKS LIKE

When you are done, the developer can:
1. Run `go test -race ./internal/tui/...` and see all tests pass
2. Import `internal/tui` and create an `AppState` with `NewApp(version, registry, apiKey)`
3. Theme system returns correctly colored styles for both dark and light modes
4. Header renders as a single line with correct model badge, context bar, and health status
5. First-run screen renders the provider selection flow when activated
6. Health check ticker emits HealthUpdateMsg at the configured interval
7. Read `walkthrough_2.md` and see actual test output + build verification

---

# WALKTHROUGH TEMPLATE

Generate `walkthrough_2.md` at the project root LAST, after all files are created and verified.

```markdown
# Walkthrough 2 — TUI Foundation

## Completed Tasks

### P2.1 — Bubble Tea Application Skeleton
- [ ] AppState struct with all fields (screen, version, registry, activeProvider, activeModel, contextUsed, contextTotal, healthStatus, etc.)
- [ ] Screen enum with ScreenFirstRun, ScreenREPL, ScreenModelSelector, ScreenSettings, ScreenResume, ScreenPermission
- [ ] Init() starts health check ticker
- [ ] Update() handles WindowSizeMsg, KeyMsg (ctrl+c quit), HealthUpdateMsg, ProviderSwitchMsg, AppMsg screen transitions
- [ ] View() routes to active screen's View()
- [ ] Global key bindings: ctrl+c quit, ctrl+d quit

### P2.2 — Theme System
- [ ] Theme struct with all color tokens (Background, Surface, Border, Brand, TextPrimary, etc.)
- [ ] Dark mode palette matches spec (Background=#0D0D0D, Brand=#D77757, etc.)
- [ ] Light mode palette matches spec (Background=#FFFFFF, Brand=#C45C3A, etc.)
- [ ] Pre-built styles: Header, ModelBadge, ContextBar, UserBubble, AssistantBubble, InputArea, ThinkingBlock, ToolCard, ToolLabel map, badges, Modal
- [ ] ToolLabel color map: Bash=Warning, FileRead=Thinking, FileWrite=Brand, Glob=TextSecondary, Grep=Thinking

### P2.3 — REPL Screen
- [ ] ReplModel with messages, input, scroll, streaming, spinner state
- [ ] 4-region layout: Header (1 line) + Messages (flex) + Input (3-6 lines) + Status (1 line)
- [ ] Header: M31A brand + [OR]/[ZEN] badge + model name + context bar + [LIVE]/[SLOW]/[OFFLINE]
- [ ] Messages: welcome message, auto-scroll, pgup/pgdn navigation
- [ ] Input: textarea with placeholder, enter=send, shift+enter=newline, esc=clear
- [ ] Status bar: spinner during streaming, timestamp when idle
- [ ] Key bindings table implemented

### P2.4 — Health Check Ticker
- [ ] HealthCheckTicker runs background goroutine at configured interval
- [ ] Emits HealthUpdateMsg via tea.Cmd channel
- [ ] Adapts interval on rate-limit (60s → 120s)
- [ ] Stops polling on 401 auth failure
- [ ] Non-blocking: never interrupts active streaming

### P2.5 — First-Run Screen
- [ ] FirstRunModel with state machine (Welcome → ProviderSelect → KeyInput → Validating → KeychainPrompt → Complete)
- [ ] Provider selection: 1=OpenRouter, 2=Zen, 3=Both, 4=Skip
- [ ] Key input and validation via HealthCheck
- [ ] Invalid key shows inline error
- [ ] Keychain flag set (storage deferred to Phase 5)
- [ ] Skip transitions to REPL with banner

### P2.6 — Header Component
- [ ] RenderHeader builds single-line header
- [ ] Provider badge colored correctly
- [ ] Context bar color-shifts at 80%/95%
- [ ] Health status colored correctly
- [ ] Truncates with ... when too wide

### P2.7 — Status Bar Component
- [ ] RenderStatusBar shows operation + timestamp
- [ ] Spinner during streaming
- [ ] Right-aligned timestamp

## Test Results

### All TUI Tests
```
[paste actual output of: go test -race -v ./internal/tui/...]
```

### Test Summary
- Total tests: [count]
- Passed: [count]
- Failed: [count]
- Skipped: [count]

## Build Verification
- [ ] `go mod tidy` passes
- [ ] `go build ./...` passes (output below)
- [ ] `go vet ./...` passes (output below)

### go build ./...
```
[paste actual output]
```

### go vet ./...
```
[paste actual output]
```

## Deviations from Spec
[List any deviation from ROADMAP.md Phase 2 tasks, or write "None"]

## Open Questions / Blockers for Phase 3
[List anything Phase 3 (Message Rendering Pipeline) needs to know, or write "None"]

## Deliverable Summary
[3-5 sentences summarizing what was implemented and confirming all Phase 2 deliverables are met.]
```
