package tui

import (
	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/tui/theme"
)

// RenderWhichKey renders a which-key hint line for the given context.
func (r *KeyRegistry) RenderWhichKey(ctx KeyContext, t theme.Theme, maxWidth int) string {
	bindings := r.GetContextBindings(ctx)
	if len(bindings) == 0 {
		return ""
	}

	var parts []string
	for _, b := range bindings {
		displayKey := b.Key
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
		truncateWidth := maxWidth - 3
		if truncateWidth < 10 {
			return ""
		}
		result = TruncateWithEllipsis(result, truncateWidth)
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
	r.Register(CtxGlobal, "ctrl+p", "command palette", nil)
	r.Register(CtxGlobal, "ctrl+b", "toggle sidebar", nil)
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

	r.Register(CtxREPL, "ctrl+m", "cycle model forward", func() tea.Cmd {
		return func() tea.Msg { return KeyActionMsg{Action: "cycle_model_forward"} }
	})
	r.Register(CtxREPL, "ctrl+shift+m", "cycle model backward", func() tea.Cmd {
		return func() tea.Msg { return KeyActionMsg{Action: "cycle_model_backward"} }
	})
	r.Register(CtxGlobal, "ctrl+x t", "toggle theme", func() tea.Cmd {
		return func() tea.Msg { return KeyActionMsg{Action: "toggle_theme"} }
	})

	// Single-character keys (t, T, x, esc, tab) are handled directly in ReplModel.Update()
	// with conditional logic (only when textarea is empty). Do NOT register them here,
	// as the KeyRegistry would consume them before they reach the textarea, breaking
	// normal text input (e.g., typing "/settings" would lose the "t").

	r.Register(CtxSettings, "ctrl+s", "save config", nil)
	r.Register(CtxSettings, "tab", "next tab", nil)
	r.Register(CtxSettings, "shift+tab", "prev tab", nil)

	r.Register(CtxModelSel, "tab", "toggle detail", nil)
	r.Register(CtxModelSel, "p", "cycle provider", nil)
}
