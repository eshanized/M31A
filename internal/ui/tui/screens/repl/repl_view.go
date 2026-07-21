package repl

import (
	"fmt"
	"github.com/eshanized/M31A/internal/ui/tui/tuitypes"
	"path/filepath"
	"strings"
	"time"

	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/ui/tui/components"
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
	m.updatePlaceholder()
	t := m.theme
	rw := m.replWidth()

	// ── Viewport (messages or welcome) with scrollbar overlay ───────────────
	viewportContent := overlayScrollbar(m.viewport.View(), m.viewport, m.styleCache.S, rw)

	// ── Floating overlays (anchored to viewport's bottom rows) ─────────────
	var overlays []string
	if m.mentionVisible && len(m.mentionEntries) > 0 {
		overlays = append(overlays, RenderMentionSuggestions(m, rw))
	}
	if m.slashVisible && len(m.slashSuggestions) > 0 {
		overlays = append(overlays, m.renderSlashSuggestions(rw))
	}
	if m.quickActionsVisible && !m.streaming && len(m.messages) > 0 {
		overlays = append(overlays, m.renderQuickActionsOverlay(rw))
	}
	if m.keyRegistry != nil && m.keyRegistry.IsLeaderActive() {
		if wk := m.keyRegistry.RenderWhichKey(tuitypes.CtxREPL, rw, t.Brand, t.TextSecondary, t.TextMuted); wk != "" {
			overlays = append(overlays, wk)
		}
	}
	viewportContent = compositeOverlays(viewportContent, overlays, rw, m.viewport.Height)

	// ── Inline search bar (Ctrl+F) ──────────────────────────────────────────
	var searchBar string
	if m.search.visible {
		searchBar = m.renderSearchBar(rw)
	}

	// Floating new-messages pill anchored to the TOP of the viewport.
	if m.newMessagesWhileScrolled > 0 && m.userScrolled {
		s := m.styleCache.S
		pill := s.BrandBold.
			Align(lipgloss.Center).
			Width(rw).
			Render(fmt.Sprintf("● %d new message(s) — ctrl+l or end to jump", m.newMessagesWhileScrolled))
		viewportContent = compositeOverlaysTop(viewportContent, []string{pill}, rw, m.viewport.Height)
	}

	// ── Input separator: animated wave during streaming, clean line when idle ──
	inputBorder := m.renderWaveSeparator(rw)

	// ── Textarea ───────────────────────────────────────────────────────────
	textareaView := m.textarea.View()
	// Apply visible focus ring when the REPL input has keyboard focus.
	// The input loses focus when modals/overlays are open (slash, mention, etc.).
	hasFocus := !m.slashVisible && !m.mentionVisible && !m.quickActionsVisible
	textareaView = components.RenderFocusRing(textareaView, hasFocus, t, rw)

	// ── Status bar ─────────────────────────────────────────────────────────
	var thinkingDur int64
	if m.thinking && !m.thinkingStartAt.IsZero() {
		thinkingDur = time.Since(m.thinkingStartAt).Milliseconds()
	}
	info := &StatusBarInfo{
		IsStreaming:        m.streaming,
		IsThinking:         m.thinking,
		ThinkingDuration:   thinkingDur,
		SpinnerFrame:       m.spinner.Peek(),
		KeyboardHints:      []string{"ctrl+p cmds", "ctrl+b sidebar"},
		WorkflowPhase:      m.workflowPhase,
		WorkflowPhaseIndex: m.workflowPhaseIndex,
		TotalPhases:        m.totalPhases,
	}
	if m.streaming || m.thinking {
		info.KeyboardHints = append([]string{"ctrl+c cancel"}, info.KeyboardHints...)
	}
	if m.cwd != "" {
		info.CwdName = filepath.Base(m.cwd)
	}
	if m.sidebarBranch != "" {
		info.GitBranch = m.sidebarBranch
	}
	if m.lastUsage != nil && m.cfg != nil && m.cfg.UI.ShowCostEstimate {
		info.TotalTokens = m.lastUsage.TotalTokens
		info.Cost = m.lastCost
		info.ShowCost = true
	}
	if m.lastUsage != nil {
		info.ContextUsed = m.lastUsage.TotalTokens
	}
	if m.activeModel != nil && m.activeModel.ContextLength > 0 {
		info.ContextMax = int(m.activeModel.ContextLength)
	}
	if m.keyRegistry != nil && m.keyRegistry.IsLeaderActive() {
		info.LeaderActive = true
	}
	if m.dispatcher != nil {
		info.BatchApprovalTools = m.dispatcher.ActiveBatchToolNames()
	}
	statusBar := RenderStatusBar(m.styleCache.S, rw, info)

	// ── Assemble all parts ─────────────────────────────────────────────────
	parts := []string{viewportContent, inputBorder, textareaView}
	if searchBar != "" {
		parts = append(parts, searchBar)
	}
	parts = append(parts, statusBar)
	return lipgloss.JoinVertical(lipgloss.Left, parts...)
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
	m.updatePlaceholder()
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
	viewportContent := overlayScrollbar(m.viewport.View(), m.viewport, m.styleCache.S, rw)

	// Input separator: animated wave during streaming, clean line when idle
	inputBorder := m.renderWaveSeparator(rw)

	// ── Floating overlays — composited onto the viewport's bottom rows ────
	var overlays []string

	// @-mention dropdown.
	if m.mentionVisible && len(m.mentionEntries) > 0 {
		overlays = append(overlays, RenderMentionSuggestions(m, rw))
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
		if wk := m.keyRegistry.RenderWhichKey(tuitypes.CtxREPL, rw, t.Brand, t.TextSecondary, t.TextMuted); wk != "" {
			overlays = append(overlays, wk)
		}
	}

	viewportContent = compositeOverlays(viewportContent, overlays, rw, vpH)

	// ── Floating new-messages indicator — anchored to the TOP of viewport ─
	if m.newMessagesWhileScrolled > 0 && m.userScrolled {
		s := m.styleCache.S
		pill := s.BrandBold.
			Align(lipgloss.Center).
			Width(rw).
			Render(fmt.Sprintf("● %d new message(s) — ctrl+l or end to jump", m.newMessagesWhileScrolled))
		viewportContent = compositeOverlaysTop(viewportContent, []string{pill}, rw, vpH)
	}

	// Textarea
	textareaView := m.textarea.View()
	// Apply visible focus ring when the REPL input has keyboard focus.
	hasFocus := !m.slashVisible && !m.mentionVisible && !m.quickActionsVisible
	textareaView = components.RenderFocusRing(textareaView, hasFocus, t, rw)

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

// renderWaveSeparator renders the input area separator.
// When the agent is idle: a plain ▁▁▁ line in border color.
// When streaming or thinking: a travelling ▁▂▃▄▃▂▁ wave in brand color.
func (m *ReplModel) renderWaveSeparator(width int) string {
	if width <= 0 {
		return ""
	}
	s := m.styleCache.S

	if !m.streaming && !m.thinking {
		// Idle: clean, subtle line
		return s.SeparatorH.Render(strings.Repeat("▁", width))
	}

	// Active: travelling sine-wave using block chars
	waveChars := []rune{'▁', '▂', '▃', '▄', '▃', '▂'}
	waveLen := len(waveChars)

	var raw strings.Builder
	raw.Grow(width)
	for i := 0; i < width; i++ {
		phase := (i + m.waveOffset) % waveLen
		if phase < 0 {
			phase += waveLen
		}
		raw.WriteRune(waveChars[phase])
	}
	return s.BrandText.Render(raw.String())
}

func (m *ReplModel) renderSlashSuggestions(width int) string {
	s := m.styleCache.S
	var lines []string

	for i, cmd := range m.slashSuggestions {
		slashStyle := s.BrandText
		nameStyle := s.Body
		descStyle := s.Muted

		if i == m.slashSelected {
			nameStyle = nameStyle.Background(m.theme.Brand).Foreground(m.theme.Background)
			slashStyle = slashStyle.Background(m.theme.Brand).Foreground(m.theme.Background)
			descStyle = descStyle.Background(m.theme.Brand).Foreground(m.theme.Background)
		}

		slash := slashStyle.Render(cmd.Slash)
		desc := descStyle.Render("  " + cmd.Description)
		line := "  " + nameStyle.Render(slash) + desc
		if lipgloss.Width(line) > width {
			line = tuitypes.TruncateWithEllipsis(line, width)
		}
		lines = append(lines, line)
	}

	box := lipgloss.NewStyle().
		Border(lipgloss.RoundedBorder()).
		BorderForeground(m.theme.Brand).
		Width(width - 2).
		Render(strings.Join(lines, "\n"))

	return box
}
