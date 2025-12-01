package tui

import (
	"strings"
	"time"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/lipgloss"
)

// KeyContext identifies which screen or layer is active for key binding lookups.
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
	CtxFirstRun  KeyContext = "firstrun"
)

// KeyAction is a callback that produces a tea.Cmd when a key chord fires.
type KeyAction func() tea.Cmd

// KeyBinding associates a key (or chord) with an action and description.
type KeyBinding struct {
	Key         string
	Description string
	Action      KeyAction
	Context     KeyContext
}

// LeaderTimeoutMsg is emitted after the leader key timeout expires.
type LeaderTimeoutMsg struct{}

// KeyActionMsg is emitted by leader-key bindings to communicate an action name
// to AppState.handleKeyAction.
type KeyActionMsg struct {
	Action string
}

// KeyRegistryOpts configures the key registry.
type KeyRegistryOpts struct {
	LeaderKey     string
	LeaderTimeout time.Duration
}

// KeyRegistry holds all key bindings indexed by KeyContext.
// It supports simple key bindings and two-key leader-chord sequences.
type KeyRegistry struct {
	bindings map[KeyContext][]KeyBinding

	leaderActive  bool
	leaderKey     string
	leaderTimeout time.Duration
}

// NewKeyRegistry creates a KeyRegistry with the given options.
func NewKeyRegistry(opts KeyRegistryOpts) *KeyRegistry {
	if opts.LeaderKey == "" {
		opts.LeaderKey = "ctrl+x"
	}
	if opts.LeaderTimeout == 0 {
		opts.LeaderTimeout = 1 * time.Second
	}
	return &KeyRegistry{
		bindings:      make(map[KeyContext][]KeyBinding),
		leaderKey:     opts.LeaderKey,
		leaderTimeout: opts.LeaderTimeout,
	}
}

// Register adds a key binding to the registry.
func (r *KeyRegistry) Register(ctx KeyContext, key, description string, action KeyAction) {
	r.bindings[ctx] = append(r.bindings[ctx], KeyBinding{
		Key:         key,
		Description: description,
		Action:      action,
		Context:     ctx,
	})
}

// Handle processes a key event and returns (handled, cmd).
// If the leader key is active it looks for chord bindings first.
// If the leader key itself is pressed it activates leader mode and
// returns a timeout tick.
func (r *KeyRegistry) Handle(key string, ctx KeyContext) (bool, tea.Cmd) {
	if r.leaderActive {
		r.leaderActive = false
		chordKey := r.leaderKey + " " + key
		// Check context-specific chord bindings first
		for _, b := range r.bindings[ctx] {
			if b.Key == chordKey {
				if b.Action != nil {
					return true, b.Action()
				}
				return true, nil
			}
		}
		// Fall through to global chord bindings
		for _, b := range r.bindings[CtxGlobal] {
			if b.Key == chordKey {
				if b.Action != nil {
					return true, b.Action()
				}
				return true, nil
			}
		}
		// Chord not found — just consumed the key sequence
		return true, nil
	}

	// Activate leader mode
	if key == r.leaderKey {
		r.leaderActive = true
		return true, tea.Tick(r.leaderTimeout, func(time.Time) tea.Msg {
			return LeaderTimeoutMsg{}
		})
	}

	// Simple bindings — context then global
	for _, b := range r.bindings[ctx] {
		if b.Key == key {
			if b.Action != nil {
				return true, b.Action()
			}
			return true, nil
		}
	}
	for _, b := range r.bindings[CtxGlobal] {
		if b.Key == key {
			if b.Action != nil {
				return true, b.Action()
			}
			return true, nil
		}
	}

	return false, nil
}

// IsLeaderActive returns true if the leader key was just pressed.
func (r *KeyRegistry) IsLeaderActive() bool {
	return r.leaderActive
}

// DeactivateLeader clears leader mode (called from LeaderTimeoutMsg handler).
func (r *KeyRegistry) DeactivateLeader() {
	r.leaderActive = false
}

// GetContextBindings returns all bindings active for a context (global + context-specific).
func (r *KeyRegistry) GetContextBindings(ctx KeyContext) []KeyBinding {
	var result []KeyBinding
	result = append(result, r.bindings[CtxGlobal]...)
	result = append(result, r.bindings[ctx]...)
	return result
}

// RenderWhichKey returns a formatted which-key overlay showing available leader
// key bindings for the current context. Returns empty string if no bindings.
func (r *KeyRegistry) RenderWhichKey(ctx KeyContext, maxWidth int, brand, textSecondary, textMuted lipgloss.Color) string {
	if !r.leaderActive {
		return ""
	}

	// Collect chord bindings for this context + global
	var bindings []KeyBinding
	seen := make(map[string]bool)

	// Context-specific chords
	for _, b := range r.bindings[ctx] {
		if strings.HasPrefix(b.Key, r.leaderKey+" ") {
			shortKey := strings.TrimPrefix(b.Key, r.leaderKey+" ")
			if !seen[shortKey] {
				seen[shortKey] = true
				bindings = append(bindings, KeyBinding{
					Key:         shortKey,
					Description: b.Description,
				})
			}
		}
	}

	// Global chords
	for _, b := range r.bindings[CtxGlobal] {
		if strings.HasPrefix(b.Key, r.leaderKey+" ") {
			shortKey := strings.TrimPrefix(b.Key, r.leaderKey+" ")
			if !seen[shortKey] {
				seen[shortKey] = true
				bindings = append(bindings, KeyBinding{
					Key:         shortKey,
					Description: b.Description,
				})
			}
		}
	}

	if len(bindings) == 0 {
		return ""
	}

	// Build which-key display
	var lines []string
	for _, b := range bindings {
		key := lipgloss.NewStyle().Foreground(brand).Bold(true).Render(b.Key)
		desc := lipgloss.NewStyle().Foreground(textSecondary).Render(" → " + b.Description)
		lines = append(lines, "  "+key+desc)
	}

	content := strings.Join(lines, "\n")

	// Wrap in a bordered box
	box := lipgloss.NewStyle().
		Border(lipgloss.RoundedBorder()).
		BorderForeground(brand).
		Padding(0, 1).
		Width(maxWidth / 3).
		Render(content)

	return box
}
