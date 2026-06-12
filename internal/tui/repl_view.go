package tui

import (
	"fmt"
	"strings"
	"time"

	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/tui/theme"
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

// chromeHeight computes the total number of rows ViewContent reserves for
// non-viewport chrome: the input separator (1) + the textarea's current
// height (which grows with multi-line input).
//
// Overlays (slash, mention, quick-actions, which-key, new-messages) float on
// the viewport and do NOT count toward chromeHeight.
func (m *ReplModel) chromeHeight() int {
	taH := m.textarea.Height()
	if taH < inputHeight {
		taH = inputHeight
	}
	return inputSeparatorHeight + taH
}

// compositeOverlays anchors overlay lines to the bottom of the viewport
// content, replacing the last len(overlayLines) rows. The underlying message
// text in those rows is occluded; the user can scroll to reveal it.
//
// When overlayLines is empty the viewport content is returned unchanged.
func compositeOverlays(viewportContent string, overlayLines []string, width, vpHeight int) string {
	if len(overlayLines) == 0 || vpHeight <= 0 {
		return viewportContent
	}
	n := len(overlayLines)
	if n > vpHeight {
		n = vpHeight
		overlayLines = overlayLines[len(overlayLines)-vpHeight:]
	}

	lines := strings.Split(viewportContent, "\n")
	for len(lines) < vpHeight {
		lines = append(lines, "")
	}
	if len(lines) > vpHeight {
		lines = lines[:vpHeight]
	}

	start := vpHeight - n
	for i, ov := range overlayLines {
		base := lines[start+i]
		baseW := lipgloss.Width(base)
		if baseW < width {
			base = base + strings.Repeat(" ", width-baseW)
		}
		ovW := lipgloss.Width(ov)
		if ovW >= width {
			lines[start+i] = ov
			continue
		}
		prefix := truncateStyledToWidth(base, width-ovW)
		gap := width - lipgloss.Width(prefix) - ovW
		if gap < 0 {
			gap = 0
		}
		lines[start+i] = prefix + strings.Repeat(" ", gap) + ov
	}
	return strings.Join(lines, "\n")
}

// ─── View ─────────────────────────────────────────────────────────────────────

// View renders the REPL screen (standalone mode, not used by PageLayout).
func (m *ReplModel) View() string {
	t := m.theme
	rw := m.replWidth()

	// ── Viewport (messages or welcome) with scrollbar overlay ───────────────
	viewportContent := overlayScrollbar(m.viewport.View(), m.viewport, t, rw)

	// ── Floating overlays (anchored to viewport's bottom rows) ─────────────
	var overlays []string
	if m.mentionVisible && len(m.mentionEntries) > 0 {
		overlays = append(overlays, m.renderMentionSuggestions(rw))
	}
	if m.slashVisible && len(m.slashSuggestions) > 0 {
		overlays = append(overlays, m.renderSlashSuggestions(rw))
	}
	if m.quickActionsVisible && !m.streaming && len(m.messages) > 0 {
		overlays = append(overlays, m.renderQuickActionsOverlay(rw))
	}
	if m.keyRegistry != nil && m.keyRegistry.IsLeaderActive() {
		if wk := m.keyRegistry.RenderWhichKey(CtxREPL, rw, t.Brand, t.TextSecondary, t.TextMuted); wk != "" {
			overlays = append(overlays, wk)
		}
	}
	viewportContent = compositeOverlays(viewportContent, overlays, rw, m.viewport.Height)

	// Floating new-messages pill anchored to the TOP of the viewport.
	if m.newMessagesWhileScrolled > 0 && m.userScrolled {
		pill := lipgloss.NewStyle().
			Foreground(t.Background).
			Background(t.Brand).
			Bold(true).
			Align(lipgloss.Center).
			Width(rw).
			Render(fmt.Sprintf(" ↓ %d new message(s) — ctrl+l to scroll ", m.newMessagesWhileScrolled))
		viewportContent = compositeOverlaysTop(viewportContent, []string{pill}, rw, m.viewport.Height)
	}

	// ── Input separator: clean half-block line ──────────────────────────────
	inputBorder := lipgloss.NewStyle().Foreground(t.Border).Render(strings.Repeat("▁", rw))

	// ── Textarea ───────────────────────────────────────────────────────────
	textareaView := m.textarea.View()

	// ── Status bar ─────────────────────────────────────────────────────────
	var thinkingDur int64
	if m.thinking && !m.thinkingStartAt.IsZero() {
		thinkingDur = time.Since(m.thinkingStartAt).Milliseconds()
	}
	info := &StatusBarInfo{
		IsStreaming:      m.streaming,
		IsThinking:       m.thinking,
		ThinkingDuration: thinkingDur,
		SpinnerFrame:     m.spinner.Peek(),
		KeyboardHints:    []string{"ctrl+p cmds", "ctrl+b sidebar"},
	}
	if m.streaming || m.thinking {
		info.KeyboardHints = append([]string{"ctrl+c cancel"}, info.KeyboardHints...)
	}
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

	// ── Assemble all parts ─────────────────────────────────────────────────
	parts := []string{viewportContent, inputBorder, textareaView, statusBar}
	return lipgloss.JoinVertical(lipgloss.Left, parts...)
}

// renderMergedMetadata renders a single line combining the usage row (ctx
// meter · last-request tokens · cost) and the agent · model · provider line.
// The two are separated by a " · " so everything fits on one row.
func (m *ReplModel) renderMergedMetadata(agentName, modelName, providerName string, width int) string {
	t := m.theme
	var parts []string

	// Usage prefix (context meter + tokens + cost), if any data is available.
	if m.lastUsage != nil && m.lastUsage.TotalTokens > 0 {
		if m.activeModel != nil && m.activeModel.ContextLength > 0 {
			total := int(m.activeModel.ContextLength)
			used := m.lastUsage.TotalTokens
			if used > total {
				used = total
			}
			parts = append(parts, renderContextMeter(used, total, t))
		}
		parts = append(parts,
			lipgloss.NewStyle().Foreground(t.TextMuted).Render(formatTokenCount(m.lastUsage.TotalTokens)))
		if m.cfg != nil && m.cfg.UI.ShowCostEstimate && m.lastCost > 0 {
			var costStr string
			if m.lastCost < 0.01 {
				costStr = "<$0.01"
			} else {
				costStr = fmt.Sprintf("$%.2f", m.lastCost)
			}
			parts = append(parts, lipgloss.NewStyle().Foreground(t.Warning).Render(costStr))
		}
	}

	// Agent · model · provider suffix.
	if agentName != "" {
		parts = append(parts, lipgloss.NewStyle().Foreground(t.Text).Render(agentName))
	}
	if modelName != "" {
		parts = append(parts, lipgloss.NewStyle().Foreground(t.TextMuted).Render(modelName))
	}
	if providerName != "" {
		providerShort := ProviderShortName(providerName)
		parts = append(parts, lipgloss.NewStyle().Foreground(t.TextMuted).Render("["+providerShort+"]"))
	}
	if len(parts) == 0 {
		return ""
	}

	sep := lipgloss.NewStyle().Foreground(t.TextMuted).Render(" · ")
	row := strings.Join(parts, sep)
	if lipgloss.Width(row) > width {
		row = TruncateWithEllipsis(row, width)
	}
	return lipgloss.NewStyle().PaddingTop(1).Width(width).Render(row)
}

// ViewContent renders the REPL content area for the unified PageLayout system.
// It returns ONLY the content: viewport + separator + textarea.
// Header, footer, metadata row, and status bar are handled by PageChrome.
//
// Layout (exactly contentHeight rows):
//
//	viewport  ← contentHeight - chromeHeight rows
//	▁[quick actions ctrl+q]▁▁▁▁▁▁▁▁▁▁▁  ← input separator (1 row)
//	[textarea]                ← textarea.Height() rows
//
// Overlays (slash, mention, quick-actions dropdown, which-key, new-messages)
// float on the viewport — they are composited onto its bottom rows so they
// do not consume extra vertical space.
func (m *ReplModel) ViewContent(contentHeight, contentWidth int) string {
	t := m.theme
	rw := contentWidth
	if rw < 20 {
		rw = 20
	}

	// Auto-expand textarea height based on current input lines, then compute
	// the chrome budget and resize the viewport to fill the remaining rows.
	m.updateAutoExpandHeight()
	chromeH := m.chromeHeight()
	vpH := contentHeight - chromeH
	if vpH < 4 {
		vpH = 4
	}
	if m.viewport.Width != rw || m.viewport.Height != vpH {
		m.viewport.Width = rw
		m.viewport.Height = vpH
		m.autoScrollConditionally()
	}

	// Base viewport (messages or welcome content) with scrollbar overlay.
	viewportContent := overlayScrollbar(m.viewport.View(), m.viewport, t, rw)

	// Input separator: clean half-block line
	inputBorder := lipgloss.NewStyle().Foreground(t.Border).Render(strings.Repeat("▁", rw))

	// ── Floating overlays — composited onto the viewport's bottom rows ────
	var overlays []string

	// @-mention dropdown.
	if m.mentionVisible && len(m.mentionEntries) > 0 {
		overlays = append(overlays, m.renderMentionSuggestions(rw))
	}
	// Slash-command dropdown.
	if m.slashVisible && len(m.slashSuggestions) > 0 {
		overlays = append(overlays, m.renderSlashSuggestions(rw))
	}
	// Quick actions dropdown (ctrl+q).
	if m.quickActionsVisible && !m.streaming && len(m.messages) > 0 {
		overlays = append(overlays, m.renderQuickActionsOverlay(rw))
	}
	// Which-key overlay (leader key active).
	if m.keyRegistry != nil && m.keyRegistry.IsLeaderActive() {
		if wk := m.keyRegistry.RenderWhichKey(CtxREPL, rw, t.Brand, t.TextSecondary, t.TextMuted); wk != "" {
			overlays = append(overlays, wk)
		}
	}

	viewportContent = compositeOverlays(viewportContent, overlays, rw, vpH)

	// ── Floating new-messages indicator — anchored to the TOP of viewport ─
	if m.newMessagesWhileScrolled > 0 && m.userScrolled {
		pill := lipgloss.NewStyle().
			Foreground(t.Background).
			Background(t.Brand).
			Bold(true).
			Align(lipgloss.Center).
			Width(rw).
			Render(fmt.Sprintf(" ↓ %d new message(s) — ctrl+l to scroll ", m.newMessagesWhileScrolled))
		viewportContent = compositeOverlaysTop(viewportContent, []string{pill}, rw, vpH)
	}

	// Textarea
	textareaView := m.textarea.View()

	// Assemble exactly contentHeight rows: viewport + input border + textarea.
	parts := []string{viewportContent, inputBorder, textareaView}
	return lipgloss.JoinVertical(lipgloss.Left, parts...)
}

// compositeOverlaysTop anchors overlay lines to the TOP of the viewport
// content, replacing the first len(overlayLines) rows. Used for transient
// indicators like the "N new messages" pill.
func compositeOverlaysTop(viewportContent string, overlayLines []string, width, vpHeight int) string {
	if len(overlayLines) == 0 || vpHeight <= 0 {
		return viewportContent
	}
	n := len(overlayLines)
	if n > vpHeight {
		n = vpHeight
		overlayLines = overlayLines[:vpHeight]
	}

	lines := strings.Split(viewportContent, "\n")
	for len(lines) < vpHeight {
		lines = append(lines, "")
	}
	if len(lines) > vpHeight {
		lines = lines[:vpHeight]
	}

	for i, ov := range overlayLines {
		if i >= n {
			break
		}
		base := lines[i]
		baseW := lipgloss.Width(base)
		if baseW < width {
			base = base + strings.Repeat(" ", width-baseW)
		}
		ovW := lipgloss.Width(ov)
		if ovW >= width {
			lines[i] = ov
			continue
		}
		prefix := truncateStyledToWidth(base, width-ovW)
		gap := width - lipgloss.Width(prefix) - ovW
		if gap < 0 {
			gap = 0
		}
		lines[i] = prefix + strings.Repeat(" ", gap) + ov
	}
	return strings.Join(lines, "\n")
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

// renderUsageRow renders an optional row above the metadata:
//
//	ctx [████░░░░] 42%  ·  3.6K ctx  ·  $0.12
//
// Returns "" when there is no usage data, so callers may append its output
// unconditionally inside a vertical stack.
func (m *ReplModel) renderUsageRow(width int) string {
	if m.lastUsage == nil || m.lastUsage.TotalTokens == 0 {
		return ""
	}

	t := m.theme
	var parts []string

	// Context meter (visual bar) — only when model's context length is known.
	if m.activeModel != nil && m.activeModel.ContextLength > 0 {
		total := int(m.activeModel.ContextLength)
		used := m.lastUsage.TotalTokens
		if used > total {
			used = total
		}
		parts = append(parts, renderContextMeter(used, total, t))
	}

	// Last-request token count.
	parts = append(parts,
		lipgloss.NewStyle().Foreground(t.TextMuted).Render(formatTokenCount(m.lastUsage.TotalTokens)))

	// Session cost (honors UI.ShowCostEstimate config).
	if m.cfg != nil && m.cfg.UI.ShowCostEstimate && m.lastCost > 0 {
		var costStr string
		if m.lastCost < 0.01 {
			costStr = "<$0.01"
		} else {
			costStr = fmt.Sprintf("$%.2f", m.lastCost)
		}
		parts = append(parts,
			lipgloss.NewStyle().Foreground(t.Warning).Render(costStr))
	}

	sep := lipgloss.NewStyle().Foreground(t.TextMuted).Render(" · ")
	row := strings.Join(parts, sep)
	if lipgloss.Width(row) > width {
		row = TruncateWithEllipsis(row, width)
	}
	return lipgloss.NewStyle().Width(width).Render(row)
}

// renderContextMeter renders a compact visual context usage bar:
//
//	ctx [████░░░░] 42%
func renderContextMeter(used, total int, t theme.Theme) string {
	if total <= 0 {
		return ""
	}
	pct := float64(used) / float64(total)
	var ctxColor lipgloss.Color
	switch {
	case pct >= 0.9:
		ctxColor = t.Error
	case pct >= 0.7:
		ctxColor = t.Warning
	default:
		ctxColor = t.TextMuted
	}

	const barSegments = 8
	filled := int(pct * barSegments)
	if filled > barSegments {
		filled = barSegments
	}
	if filled < 0 {
		filled = 0
	}

	bar := "["
	bar += strings.Repeat("█", filled)
	bar += strings.Repeat("░", barSegments-filled)
	bar += "]"

	return lipgloss.NewStyle().Foreground(ctxColor).Render(
		fmt.Sprintf("ctx %s %d%%", bar, int(pct*100)),
	)
}
