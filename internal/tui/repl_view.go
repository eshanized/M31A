package tui

import (
	"strings"

	"github.com/charmbracelet/lipgloss"
)

// ─── Layout constants ─────────────────────────────────────────────────────────

// viewportTopChrome is the height of everything above the viewport in the REPL.
// This is: header (1) + divider (1) = 2 rows.
const viewportTopChrome = 2

// viewportBottomChrome is the height of everything below the viewport.
// This is: top-border (1) + metadata (1) + textarea (inputHeight) + bottom-border (1) + status (1) = inputHeight + 4.
func viewportBottomChrome() int {
	return inputHeight + 4
}

// viewportHeight computes the viewport height given terminal height.
// It ensures there's always at least 4 rows for the viewport.
func viewportHeight(termHeight int) int {
	h := termHeight - viewportTopChrome - viewportBottomChrome()
	if h < 4 {
		h = 4
	}
	return h
}

// ─── View ─────────────────────────────────────────────────────────────────────

// View renders the REPL screen.
//
// Layout (from top to bottom):
//  1. [optional sidebar] | [viewport: messages OR welcome content]
//  2. ─── input border ───────────────────────────────────────────
//  3. M31A · model · provider  (metadata row)
//  4. [textarea: user input]
//  5. ╹▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀  (bottom border)
//  6. status bar
//  7. [slash suggestion dropdown overlay]
//
// IMPORTANT: The welcome screen (logo + provider card + hints) is set as
// the viewport's content, NOT rendered outside the viewport. This prevents
// the double-input visual bug seen in the previous version.
func (m *ReplModel) View() string {
	t := m.theme
	rw := m.replWidth()

	// ── Welcome mode: set viewport content to welcome screen ─────────────────
	if len(m.messages) == 0 && !m.streaming {
		welcomeContent := m.renderWelcome()
		m.viewport.SetContent(welcomeContent)
	}

	// ── Viewport ──────────────────────────────────────────────────────────────
	viewportContent := m.viewport.View()

	// ── Input border (top) ────────────────────────────────────────────────────
	inputBorder := lipgloss.NewStyle().
		Foreground(t.Brand).
		Render(strings.Repeat("─", rw))

	// ── Metadata row: M31A · model · [provider] ───────────────────────────────
	agentName := "M31A"
	modelName := ""
	providerName := ""
	if m.activeModel != nil {
		modelName = m.activeModel.Name
	}
	if m.activeProvider != "" {
		providerName = m.activeProvider
	}
	metaRow := RenderPromptMetadata(agentName, modelName, providerName, t, rw)

	// ── Textarea ──────────────────────────────────────────────────────────────
	textareaView := m.textarea.View()

	// ── Bottom border ─────────────────────────────────────────────────────────
	bottomBorder := RenderPromptBottomBorder(t.Brand, rw)

	// ── Status bar ────────────────────────────────────────────────────────────
	info := &StatusBarInfo{
		IsStreaming:  m.streaming,
		IsThinking:   m.thinking,
		KeyboardHints: []string{"ctrl+p commands", "ctrl+b sidebar", "ctrl+x leader"},
	}
	if m.lastUsage != nil && m.cfg != nil && m.cfg.UI.ShowCostEstimate {
		info.TotalTokens = m.lastUsage.TotalTokens
		info.Cost = m.lastCost
		info.ShowCost = true
	}
	if m.keyRegistry != nil && m.keyRegistry.IsLeaderActive() {
		info.LeaderActive = true
	}
	statusBar := RenderStatusBar(t, m.GetStatusText(), m.lastActivity, rw, info)

	// ── Slash suggestions overlay (above input) ───────────────────────────────
	slashOverlay := ""
	if m.slashVisible && len(m.slashSuggestions) > 0 {
		slashOverlay = m.renderSlashSuggestions(rw) + "\n"
	}

	// ── Assemble all parts ────────────────────────────────────────────────────
	parts := []string{
		viewportContent,
		inputBorder,
		metaRow,
		textareaView,
		bottomBorder,
		statusBar,
	}
	if slashOverlay != "" {
		// Prepend the overlay before the input border
		parts = []string{
			viewportContent,
			slashOverlay,
			inputBorder,
			metaRow,
			textareaView,
			bottomBorder,
			statusBar,
		}
	}

	return lipgloss.JoinVertical(lipgloss.Left, parts...)
}

// renderSlashSuggestions renders the slash command autocomplete dropdown.
func (m *ReplModel) renderSlashSuggestions(width int) string {
	t := m.theme
	var lines []string

	for i, cmd := range m.slashSuggestions {
		slashStyle := lipgloss.NewStyle().Foreground(t.Brand)
		nameStyle := lipgloss.NewStyle().Foreground(t.Text)
		descStyle := lipgloss.NewStyle().Foreground(t.TextMuted)

		if i == m.slashSelected {
			nameStyle = nameStyle.Background(t.Brand).Foreground(t.Background)
			slashStyle = slashStyle.Background(t.Brand).Foreground(t.Background)
			descStyle = descStyle.Background(t.Brand).Foreground(t.Background)
		}

		slash := slashStyle.Render(cmd.Slash)
		desc := descStyle.Render("  " + cmd.Description)
		line := "  " + nameStyle.Render(slash) + desc
		if lipgloss.Width(line) > width {
			line = TruncateWithEllipsis(line, width)
		}
		lines = append(lines, line)
	}

	box := lipgloss.NewStyle().
		Border(lipgloss.RoundedBorder()).
		BorderForeground(t.Brand).
		Width(width - 2).
		Render(strings.Join(lines, "\n"))

	return box
}
