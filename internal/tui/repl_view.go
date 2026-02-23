package tui

import (
	"fmt"
	"strings"
	"time"

	"github.com/charmbracelet/lipgloss"
)

// ─── Layout constants ─────────────────────────────────────────────────────────

// inputSeparatorHeight is the height of the half-block separator above the textarea.
const inputSeparatorHeight = 1

// replBottomChrome is the height of the separator + textarea below the viewport.
func replBottomChrome() int {
	return inputSeparatorHeight + inputHeight
}

// contentViewportHeight computes the viewport height given the content area height.
// Content area = contentHeight (terminal - unified chrome).
// Viewport = contentHeight - separator(1) - textarea(inputHeight).
func contentViewportHeight(contentHeight int) int {
	h := contentHeight - replBottomChrome()
	if h < 4 {
		h = 4
	}
	return h
}

// viewportTopChrome is the height of everything above the viewport in the REPL.
// With unified chrome, the header/footer are handled by PageLayout. The viewport
// sits at the top of the content area with no additional top chrome.
const viewportTopChrome = 0

// viewportBottomChrome is the height of everything below the viewport in the REPL.
// This is: separator (1) + textarea (inputHeight) = inputHeight + 1.
func viewportBottomChrome() int {
	return inputSeparatorHeight + inputHeight
}

// ─── View ─────────────────────────────────────────────────────────────────────

// View renders the REPL screen.
//
// Layout (from top to bottom):
//  1. [optional sidebar] | [viewport: messages OR welcome content]
//  2. ▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁  (bottom half-block input separator — opencode style)
//  3. M31A · model [provider]  (metadata row with badge)
//  4. [textarea: user input]
//  5. status bar (cwd ⎇ branch  hints  cost)
//  6. [slash suggestion dropdown overlay]
//
// NOTE: Bottom border (╹▀▀▀) was removed — status bar sits flush below textarea.
//
// IMPORTANT: The welcome screen (logo + provider card + hints) is set as
// the viewport's content, NOT rendered outside the viewport. This prevents
// the double-input visual bug seen in the previous version.
func (m *ReplModel) View() string {
	t := m.theme
	rw := m.replWidth()

	// ── Welcome mode: viewport content is set by renderMessages() ────────────
	// Welcome content is handled via renderMessages() → renderWelcome()

	// ── Viewport ──────────────────────────────────────────────────────────────────
	viewportContent := m.viewport.View()

	// ── Quick actions panel (below messages when idle) ───────────────────────────
	quickActions := ""
	if len(m.messages) > 0 && !m.streaming {
		quickActions = m.renderQuickActionsPanel(rw)
	}

	// ── Input separator (opencode half-block style) ──────────────────────────────
	// Top half-block row gives a visual "shelf" effect above the input area
	shelfLeft := lipgloss.NewStyle().Foreground(t.Brand).Render("▁")
	shelfFill := lipgloss.NewStyle().Foreground(t.Surface).Render(strings.Repeat("▁", rw-1))
	inputBorder := shelfLeft + shelfFill

	// ── Metadata row: M31A · model [provider] ───────────────────────────────────
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

	// ── Textarea ─────────────────────────────────────────────────────────────────
	textareaView := m.textarea.View()

	// ── Status bar ──────────────────────────────────────────────────────────────
	var thinkingDur int64
	if m.thinking && !m.thinkingStartAt.IsZero() {
		thinkingDur = time.Since(m.thinkingStartAt).Milliseconds()
	}
	info := &StatusBarInfo{
		IsStreaming:      m.streaming,
		IsThinking:       m.thinking,
		ThinkingDuration: thinkingDur,
		SpinnerFrame:     m.spinner.Peek(),
		KeyboardHints:    []string{"ctrl+p commands", "ctrl+b sidebar", "@ files", "ctrl+x leader"},
	}
	if m.streaming || m.thinking {
		info.KeyboardHints = append([]string{"ctrl+c cancel"}, info.KeyboardHints...)
	}
	// Add cwd and git branch if available
	if m.cwd != "" {
		info.CwdName = pathBase(m.cwd)
	}
	if m.sidebarBranch != "" {
		info.GitBranch = m.sidebarBranch
	}
	if m.lastUsage != nil && m.cfg != nil && m.cfg.UI.ShowCostEstimate {
		info.TotalTokens = m.lastUsage.TotalTokens
		info.Cost = m.lastCost
		info.ShowCost = true
	}
	if m.keyRegistry != nil && m.keyRegistry.IsLeaderActive() {
		info.LeaderActive = true
	}
	statusBar := RenderStatusBar(t, rw, info)

	// ── Mention suggestions overlay (above slash overlay) ─────────────────────
	mentionOverlay := ""
	if m.mentionVisible && len(m.mentionEntries) > 0 {
		mentionOverlay = m.renderMentionSuggestions(rw) + "\n"
	}

	// ── Slash suggestions overlay (above input) ───────────────────────────────
	slashOverlay := ""
	if m.slashVisible && len(m.slashSuggestions) > 0 {
		slashOverlay = m.renderSlashSuggestions(rw) + "\n"
	}

	// ── Which-key overlay (leader key active) ─────────────────────────────────
	whichKeyOverlay := ""
	if m.keyRegistry != nil && m.keyRegistry.IsLeaderActive() {
		whichKeyOverlay = m.keyRegistry.RenderWhichKey(CtxREPL, rw, t.Brand, t.TextSecondary, t.TextMuted)
		if whichKeyOverlay != "" {
			whichKeyOverlay += "\n"
		}
	}

	// ── "New messages" indicator ────────────────────────────────────────────────
	newMessagesIndicator := ""
	if m.newMessagesWhileScrolled > 0 && m.userScrolled {
		newMessagesIndicator = lipgloss.NewStyle().
			Foreground(t.Background).
			Background(t.Brand).
			Bold(true).
			Align(lipgloss.Center).
			Width(rw).
			Render(fmt.Sprintf(" ↓ %d new message(s) — ctrl+l to scroll ", m.newMessagesWhileScrolled))
	}

	// ── Assemble all parts ────────────────────────────────────────────────────
	parts := []string{viewportContent}
	if quickActions != "" {
		parts = append(parts, quickActions)
	}
	if mentionOverlay != "" {
		parts = append(parts, mentionOverlay)
	}
	if slashOverlay != "" {
		parts = append(parts, slashOverlay)
	}
	if whichKeyOverlay != "" {
		parts = append(parts, whichKeyOverlay)
	}
	if newMessagesIndicator != "" {
		parts = append(parts, newMessagesIndicator)
	}
	parts = append(parts, inputBorder, metaRow, textareaView, statusBar)

	return lipgloss.JoinVertical(lipgloss.Left, parts...)
}

// ViewContent renders the REPL content area for the unified PageLayout system.
// It returns ONLY the content: viewport + separator + textarea.
// Header, footer, metadata row, and status bar are handled by PageChrome.
func (m *ReplModel) ViewContent(contentHeight, contentWidth int) string {
	t := m.theme
	rw := contentWidth
	if rw < 20 {
		rw = 20
	}

	// Resize viewport to fit content area
	vpH := contentViewportHeight(contentHeight)
	if m.viewport.Width != rw || m.viewport.Height != vpH {
		m.viewport.Width = rw
		m.viewport.Height = vpH
		m.renderMessages()
	}

	// Viewport (messages or welcome content)
	viewportContent := m.viewport.View()

	// Quick actions panel (below messages when idle)
	quickActions := ""
	if len(m.messages) > 0 && !m.streaming {
		quickActions = m.renderQuickActionsPanel(rw)
	}

	// Input separator (opencode half-block style)
	shelfLeft := lipgloss.NewStyle().Foreground(t.Brand).Render("▁")
	shelfFill := lipgloss.NewStyle().Foreground(t.Surface).Render(strings.Repeat("▁", rw-1))
	inputBorder := shelfLeft + shelfFill

	// Textarea
	textareaView := m.textarea.View()

	// Overlays (mention, slash, which-key, new messages)
	mentionOverlay := ""
	if m.mentionVisible && len(m.mentionEntries) > 0 {
		mentionOverlay = m.renderMentionSuggestions(rw) + "\n"
	}

	slashOverlay := ""
	if m.slashVisible && len(m.slashSuggestions) > 0 {
		slashOverlay = m.renderSlashSuggestions(rw) + "\n"
	}

	whichKeyOverlay := ""
	if m.keyRegistry != nil && m.keyRegistry.IsLeaderActive() {
		whichKeyOverlay = m.keyRegistry.RenderWhichKey(CtxREPL, rw, t.Brand, t.TextSecondary, t.TextMuted)
		if whichKeyOverlay != "" {
			whichKeyOverlay += "\n"
		}
	}

	newMessagesIndicator := ""
	if m.newMessagesWhileScrolled > 0 && m.userScrolled {
		newMessagesIndicator = lipgloss.NewStyle().
			Foreground(t.Background).
			Background(t.Brand).
			Bold(true).
			Align(lipgloss.Center).
			Width(rw).
			Render(fmt.Sprintf(" ↓ %d new message(s) — ctrl+l to scroll ", m.newMessagesWhileScrolled))
	}

	// Assemble content parts
	parts := []string{viewportContent}
	if quickActions != "" {
		parts = append(parts, quickActions)
	}
	if mentionOverlay != "" {
		parts = append(parts, mentionOverlay)
	}
	if slashOverlay != "" {
		parts = append(parts, slashOverlay)
	}
	if whichKeyOverlay != "" {
		parts = append(parts, whichKeyOverlay)
	}
	if newMessagesIndicator != "" {
		parts = append(parts, newMessagesIndicator)
	}
	parts = append(parts, inputBorder, textareaView)

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

// pathBase returns the last path component of a file path.
func pathBase(p string) string {
	if p == "" {
		return ""
	}
	// Trim trailing slashes
	for len(p) > 0 && (p[len(p)-1] == '/' || p[len(p)-1] == '\\') {
		p = p[:len(p)-1]
	}
	for i := len(p) - 1; i >= 0; i-- {
		if p[i] == '/' || p[i] == '\\' {
			return p[i+1:]
		}
	}
	return p
}
