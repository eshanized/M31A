package tui

import (
	"time"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/tui/theme"
)

// KeyContext defines the scope in which keybindings are active.
type KeyContext string

const (
	CtxGlobal    KeyContext = "global"
	CtxREPL      KeyContext = "repl"
	CtxPalette   KeyContext = "palette"
	CtxSidebar   KeyContext = "sidebar"
	CtxSettings  KeyContext = "settings"
	CtxModelSel  KeyContext = "modelselector"
	CtxResume    KeyContext = "resume"
	CtxPermModal KeyContext = "permission"
	CtxPlan      KeyContext = "plan"
	CtxExecute   KeyContext = "execute"
	CtxVerify    KeyContext = "verify"
	CtxShip      KeyContext = "ship"
	CtxFirstRun  KeyContext = "firstrun"
)

// KeyAction is a function executed when a keybinding is triggered.
type KeyAction func() tea.Cmd

// KeyBinding represents a single key binding.
type KeyBinding struct {
	Key         string
	Description string
	Action      KeyAction
	Context     KeyContext
}

// LeaderTimeoutMsg is emitted when the leader key timeout fires.
type LeaderTimeoutMsg struct{}

// KeyActionMsg is emitted by keybinding actions for app-level handling.
type KeyActionMsg struct {
	Action string // "toggle_sidebar", "open_settings", "new_session", "session_list", "cycle_model", "toggle_theme"
}

// KeyRegistry manages keybindings with context-aware dispatch and leader key support.
type KeyRegistry struct {
	bindings map[KeyContext][]KeyBinding

	// Leader key state
	leaderActive  bool
	leaderKey     string
	leaderTimeout time.Duration
	leaderTimer   *time.Timer

	// Which-key display
	whichKeyActive bool
}

// NewKeyRegistry creates a new key registry with default leader key configuration.
func NewKeyRegistry() *KeyRegistry {
	return &KeyRegistry{
		bindings:      make(map[KeyContext][]KeyBinding),
		leaderKey:     "ctrl+x",
		leaderTimeout: 1 * time.Second,
	}
}

// Register adds a keybinding for a specific context.
func (r *KeyRegistry) Register(ctx KeyContext, key, description string, action KeyAction) {
	r.bindings[ctx] = append(r.bindings[ctx], KeyBinding{
		Key:         key,
		Description: description,
		Action:      action,
		Context:     ctx,
	})
}

// Handle processes a key message for the given context. Returns (handled, cmd).
// If the leader key is active, it tries to match a chord binding.
// If the key is the leader key, it activates the leader state.
func (r *KeyRegistry) Handle(key string, ctx KeyContext) (bool, tea.Cmd) {
	// If leader is active, try to match a chord
	if r.leaderActive {
		r.cancelLeaderTimer()
		r.leaderActive = false

		// Look for chord binding: leaderKey + key
		chordKey := r.leaderKey + " " + key
		for _, b := range r.bindings[ctx] {
			if b.Key == chordKey {
				if b.Action == nil {
					return true, nil
				}
				return true, b.Action()
			}
		}
		// Also check global context for chords
		for _, b := range r.bindings[CtxGlobal] {
			if b.Key == chordKey {
				if b.Action == nil {
					return true, nil
				}
				return true, b.Action()
			}
		}
		// No chord matched, treat key as normal (fallthrough)
	}

	// Check if this is the leader key
	if key == r.leaderKey {
		r.leaderActive = true
		return true, tea.Tick(r.leaderTimeout, func(time.Time) tea.Msg {
			return LeaderTimeoutMsg{}
		})
	}

	// Normal key handling: check context bindings first, then global
	for _, b := range r.bindings[ctx] {
		if b.Key == key {
			if b.Action == nil {
				return true, nil
			}
			return true, b.Action()
		}
	}
	for _, b := range r.bindings[CtxGlobal] {
		if b.Key == key {
			if b.Action == nil {
				return true, nil
			}
			return true, b.Action()
		}
	}

	return false, nil
}

// IsLeaderActive returns whether the leader key sequence is pending.
func (r *KeyRegistry) IsLeaderActive() bool {
	return r.leaderActive
}

// DeactivateLeader cancels the leader key state.
func (r *KeyRegistry) DeactivateLeader() {
	r.leaderActive = false
	r.cancelLeaderTimer()
}

func (r *KeyRegistry) cancelLeaderTimer() {
	if r.leaderTimer != nil {
		r.leaderTimer.Stop()
		r.leaderTimer = nil
	}
}

// GetContextBindings returns all bindings for a context (including global).
func (r *KeyRegistry) GetContextBindings(ctx KeyContext) []KeyBinding {
	var result []KeyBinding
	result = append(result, r.bindings[CtxGlobal]...)
	result = append(result, r.bindings[ctx]...)
	return result
}

// RenderWhichKey renders a which-key hint line for the given context.
func (r *KeyRegistry) RenderWhichKey(ctx KeyContext, t theme.Theme, maxWidth int) string {
	bindings := r.GetContextBindings(ctx)
	if len(bindings) == 0 {
		return ""
	}

	var parts []string
	for _, b := range bindings {
		displayKey := b.Key
		// Shorten common keys
		switch displayKey {
		case "ctrl+c":
			displayKey = "C-c"
		case "ctrl+p":
			displayKey = "C-p"
		case "ctrl+b":
			displayKey = "C-b"
		case "ctrl+x":
			displayKey = "C-x"
		case "esc":
			displayKey = "esc"
		case "tab":
			displayKey = "tab"
		case "shift+tab":
			displayKey = "S-tab"
		case "enter":
			displayKey = "enter"
		case "up":
			displayKey = "↑"
		case "down":
			displayKey = "↓"
		case "pgup":
			displayKey = "PgUp"
		case "pgdown":
			displayKey = "PgDn"
		}

		parts = append(parts, lipgloss.JoinHorizontal(lipgloss.Top,
			lipgloss.NewStyle().Foreground(t.Brand).Bold(true).Render(displayKey),
			lipgloss.NewStyle().Foreground(t.TextSecondary).Render(":"+b.Description),
		))
	}

	result := lipgloss.JoinHorizontal(lipgloss.Left, parts...)
	if lipgloss.Width(result) > maxWidth {
		// Truncate safely without breaking UTF-8 or ANSI sequences
		truncateWidth := maxWidth - 3
		if truncateWidth < 10 {
			return ""
		}
		result = truncateWithANSI(result, truncateWidth) + "..."
	}
	return result
}

// RenderLeaderPrompt renders the leader key waiting prompt.
func (r *KeyRegistry) RenderLeaderPrompt(t theme.Theme) string {
	if !r.leaderActive {
		return ""
	}
	return lipgloss.JoinHorizontal(lipgloss.Top,
		lipgloss.NewStyle().Foreground(t.Brand).Bold(true).Render("ctrl+x"),
		lipgloss.NewStyle().Foreground(t.TextSecondary).Render(" ─ waiting ─"),
	)
}

// RegisterDefaultBindings populates the registry with standard M31A keybindings.
func (r *KeyRegistry) RegisterDefaultBindings() {
	// Global bindings
	r.Register(CtxGlobal, "ctrl+p", "command palette", nil) // handled directly in app.go
	r.Register(CtxGlobal, "ctrl+b", "toggle sidebar", nil)  // handled directly in app.go
	r.Register(CtxGlobal, "ctrl+x b", "toggle sidebar", func() tea.Cmd {
		return func() tea.Msg { return KeyActionMsg{Action: "toggle_sidebar"} }
	})
	r.Register(CtxGlobal, "ctrl+x s", "settings", func() tea.Cmd {
		return func() tea.Msg { return KeyActionMsg{Action: "open_settings"} }
	})
	r.Register(CtxGlobal, "ctrl+x n", "new session", func() tea.Cmd {
		return func() tea.Msg { return KeyActionMsg{Action: "new_session"} }
	})
	r.Register(CtxGlobal, "ctrl+x l", "session list", func() tea.Cmd {
		return func() tea.Msg { return KeyActionMsg{Action: "session_list"} }
	})
	r.Register(CtxGlobal, "ctrl+x m", "cycle model", func() tea.Cmd {
		return func() tea.Msg { return KeyActionMsg{Action: "cycle_model"} }
	})

	// Direct Ctrl+M model cycling (REPL context only — textarea-focused)
	r.Register(CtxREPL, "ctrl+m", "cycle model forward", func() tea.Cmd {
		return func() tea.Msg { return KeyActionMsg{Action: "cycle_model_forward"} }
	})
	r.Register(CtxREPL, "ctrl+shift+m", "cycle model backward", func() tea.Cmd {
		return func() tea.Msg { return KeyActionMsg{Action: "cycle_model_backward"} }
	})
	r.Register(CtxGlobal, "ctrl+x t", "toggle theme", func() tea.Cmd {
		return func() tea.Msg { return KeyActionMsg{Action: "toggle_theme"} }
	})

	// REPL bindings
	r.Register(CtxREPL, "t", "toggle thinking (focused)", nil)
	r.Register(CtxREPL, "T", "toggle all thinking", nil)
	r.Register(CtxREPL, "tab", "next thinking block", nil)
	r.Register(CtxREPL, "shift+tab", "prev thinking block", nil)
	r.Register(CtxREPL, "pgup", "scroll up", nil)
	r.Register(CtxREPL, "pgdown", "scroll down", nil)
	r.Register(CtxREPL, "x", "dismiss banner", nil)
	r.Register(CtxREPL, "esc", "clear input", nil)

	// Settings bindings
	r.Register(CtxSettings, "ctrl+s", "save config", nil)
	r.Register(CtxSettings, "tab", "next tab", nil)
	r.Register(CtxSettings, "shift+tab", "prev tab", nil)

	// Model selector bindings
	r.Register(CtxModelSel, "tab", "toggle detail", nil)
	r.Register(CtxModelSel, "p", "cycle provider", nil)
}
