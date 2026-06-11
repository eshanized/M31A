package tui

import (
	"encoding/json"
	"strings"

	tea "github.com/charmbracelet/bubbletea"
)

// repl_mouse.go — mouse event handling for the REPL.
//
// Supports:
//   - Scroll wheel over the viewport (pass-through to viewport)
//   - Left-click on the viewport to focus the textarea + mark user-scrolled
//   - Left-click on a slash-suggestion overlay item to select it
//   - Left-click on a mention-suggestion overlay item to complete it
//   - Left-click/drag on the scrollbar thumb to drag-scroll the viewport

// handleMouseMsg dispatches mouse events to the appropriate target.
func (m *ReplModel) handleMouseMsg(msg tea.MouseMsg) tea.Cmd {
	switch msg.Action {
	case tea.MouseActionPress:
		switch msg.Button {
		case tea.MouseButtonWheelUp, tea.MouseButtonWheelDown,
			tea.MouseButtonWheelLeft, tea.MouseButtonWheelRight:
			return m.handleWheel(msg)
		case tea.MouseButtonLeft:
			return m.handleLeftClick(msg)
		}
	case tea.MouseActionRelease:
		if msg.Button == tea.MouseButtonLeft {
			m.scrollbarDragging = false
		}
	case tea.MouseActionMotion:
		if m.scrollbarDragging && msg.Button == tea.MouseButtonNone {
			// Motion with button held (drag)
			return m.handleScrollbarDrag(msg)
		}
	}
	return nil
}

// handleWheel forwards scroll-wheel events to the viewport. The viewport
// already knows how to translate wheel up/down into LineUp/LineDown, so we
// delegate via its own Update().
func (m *ReplModel) handleWheel(msg tea.MouseMsg) tea.Cmd {
	// Only consume wheel events whose Y coordinate falls within the viewport.
	if !m.mouseInViewport(msg.Y) {
		return nil
	}
	newVP, cmd := m.viewport.Update(msg)
	m.viewport = newVP
	m.userScrolled = true
	return cmd
}

// handleLeftClick routes a left-click to the overlay hit-tests, the
// scrollbar thumb, or the viewport itself.
func (m *ReplModel) handleLeftClick(msg tea.MouseMsg) tea.Cmd {
	y := msg.Y

	// 1. Slash overlay — click selects and runs the command.
	if m.slashVisible && len(m.slashSuggestions) > 0 {
		if idx, ok := m.hitTestSlashOverlay(y); ok {
			if idx >= 0 && idx < len(m.slashSuggestions) {
				m.slashSelected = idx
				return m.handleSlashComplete()
			}
		}
	}

	// 2. Mention overlay — click completes the @-mention.
	if m.mentionVisible && len(m.mentionEntries) > 0 {
		if idx, ok := m.hitTestMentionOverlay(y); ok {
			if idx >= 0 && idx < len(m.mentionEntries) {
				m.mentionSelected = idx
				m.completeMention()
				m.updateMentionSuggestions()
				return nil
			}
		}
	}

	// 3. Scrollbar thumb — begin drag or jump to clicked position.
	if m.mouseOnScrollbar(msg.X, y) {
		m.scrollbarDragging = true
		return m.jumpScrollbarToY(y)
	}

	// 4. Viewport body — click on a tool card opens the detail screen;
	//    otherwise focus the textarea and mark user-scrolled so auto-scroll
	//    doesn't yank the viewport back to the bottom.
	if m.mouseInViewport(y) {
		if cmd := m.handleViewportClick(y); cmd != nil {
			return cmd
		}
		m.userScrolled = true
		if !m.textarea.Focused() {
			m.textarea.Focus()
		}
		return nil
	}

	// 5. Fallback: any click refocuses the textarea so typing still works.
	if !m.textarea.Focused() {
		m.textarea.Focus()
	}
	return nil
}

// ── Viewport / overlay geometry helpers ──────────────────────────────────────

// mouseInViewport returns true when y falls within the viewport rows.
// Viewport always occupies y ∈ [0, viewport.Height) inside ViewContent.
func (m *ReplModel) mouseInViewport(y int) bool {
	return y >= 0 && y < m.viewport.Height
}

// mouseOnScrollbar returns true when (x, y) lands on the rightmost scrollbar
// column inside the viewport region.
func (m *ReplModel) mouseOnScrollbar(x, y int) bool {
	if !m.mouseInViewport(y) {
		return false
	}
	scrollCol := m.viewport.Width - 1
	return x >= scrollCol && x <= scrollCol+1
}

// overlayStartY returns the Y coordinate where the first floating overlay
// row begins. Overlays are anchored to the bottom of the viewport, so the
// first overlay row sits (total-overlay-height) rows above the viewport's
// bottom edge.
func (m *ReplModel) overlayStartY() int {
	totalH := 0
	if m.mentionVisible && len(m.mentionEntries) > 0 {
		totalH += m.mentionOverlayHeight()
	}
	if m.slashVisible && len(m.slashSuggestions) > 0 {
		totalH += m.slashOverlayHeight()
	}
	if m.quickActionsVisible && !m.streaming && len(m.messages) > 0 {
		totalH += m.quickActionsOverlayHeight()
	}
	if totalH > m.viewport.Height {
		totalH = m.viewport.Height
	}
	return m.viewport.Height - totalH
}

// slashOverlayHeight returns the rendered height of the slash-command
// dropdown (top border + entries + bottom border).
func (m *ReplModel) slashOverlayHeight() int {
	if !m.slashVisible || len(m.slashSuggestions) == 0 {
		return 0
	}
	rendered := m.renderSlashSuggestions(m.replWidth())
	return strings.Count(rendered, "\n") + 1
}

// quickActionsOverlayHeight returns the rendered height of the quick-actions
// dropdown (top border + entries + bottom border).
func (m *ReplModel) quickActionsOverlayHeight() int {
	rendered := m.renderQuickActionsOverlay(m.replWidth())
	return strings.Count(rendered, "\n") + 1
}

// hitTestSlashOverlay returns the index of the slash-suggestion item under
// the given Y coordinate, or (-1, false) if Y is outside the overlay.
func (m *ReplModel) hitTestSlashOverlay(y int) (int, bool) {
	if !m.slashVisible || len(m.slashSuggestions) == 0 {
		return -1, false
	}
	// The mention overlay, if present, sits ABOVE the slash overlay in the
	// parts stack. Compute the slash overlay's Y offset accordingly.
	start := m.overlayStartY()
	if m.mentionVisible && len(m.mentionEntries) > 0 {
		mentionH := m.mentionOverlayHeight()
		start += mentionH
	}
	// The slash box is rendered with a rounded border: 1 (top) + N items + 1 (bottom).
	top := start + 1 // skip the top border line
	bottom := top + len(m.slashSuggestions)
	if y < top || y >= bottom {
		return -1, false
	}
	return y - top, true
}

// hitTestMentionOverlay returns the index of the mention entry under Y.
func (m *ReplModel) hitTestMentionOverlay(y int) (int, bool) {
	if !m.mentionVisible || len(m.mentionEntries) == 0 {
		return -1, false
	}
	start := m.overlayStartY()
	top := start + 1 // skip top border line
	bottom := top + len(m.mentionEntries)
	if y < top || y >= bottom {
		return -1, false
	}
	return y - top, true
}

// mentionOverlayHeight returns the rendered height of the mention overlay
// (top border + entries + bottom border).
func (m *ReplModel) mentionOverlayHeight() int {
	if !m.mentionVisible || len(m.mentionEntries) == 0 {
		return 0
	}
	// renderMentionSuggestions returns a rounded-border box with N entry rows.
	rendered := m.renderMentionSuggestions(m.replWidth())
	return strings.Count(rendered, "\n") + 1
}

// jumpScrollbarToY moves the viewport offset so the scrollbar thumb centers
// on the clicked row. This gives standard scrollbar "click-to-jump" behavior.
func (m *ReplModel) jumpScrollbarToY(y int) tea.Cmd {
	if y < 0 {
		y = 0
	}
	if y >= m.viewport.Height {
		y = m.viewport.Height - 1
	}
	totalLines := m.viewport.TotalLineCount()
	if totalLines <= m.viewport.Height {
		return nil
	}
	// Map [0, viewport.Height) → [0, totalLines - viewport.Height]
	maxScroll := totalLines - m.viewport.Height
	target := y * maxScroll / m.viewport.Height
	if target < 0 {
		target = 0
	}
	if target > maxScroll {
		target = maxScroll
	}
	m.viewport.SetYOffset(target)
	m.userScrolled = true
	return nil
}

// handleScrollbarDrag continues a scrollbar drag by reusing jump-to-Y logic.
func (m *ReplModel) handleScrollbarDrag(msg tea.MouseMsg) tea.Cmd {
	return m.jumpScrollbarToY(msg.Y)
}

// handleViewportClick maps a viewport-relative Y coordinate to a message
// index and returns a ToolClickMsg when that message contains a tool_use
// segment. Returns nil for plain content clicks.
func (m *ReplModel) handleViewportClick(y int) tea.Cmd {
	if len(m.messageLineOffsets) == 0 || len(m.messages) == 0 {
		return nil
	}

	// Translate viewport Y to absolute content line.
	contentLine := m.viewport.YOffset + y

	// Binary-search for the rightmost offset ≤ contentLine.
	idx := -1
	lo, hi := 0, len(m.messageLineOffsets)-1
	for lo <= hi {
		mid := (lo + hi) / 2
		if m.messageLineOffsets[mid] <= contentLine {
			idx = mid
			lo = mid + 1
		} else {
			hi = mid - 1
		}
	}
	if idx < 0 || idx >= len(m.messages) {
		return nil
	}

	// Look for the first tool_use segment in the clicked message.
	msg := m.messages[idx]
	for _, seg := range msg.Segments {
		if seg.Type == "tool_use" {
			return func() tea.Msg {
				return ToolClickMsg{MessageIndex: idx, ToolName: extractToolName(seg.Content)}
			}
		}
	}
	for _, tc := range msg.ToolCalls {
		return func() tea.Msg {
			return ToolClickMsg{MessageIndex: idx, ToolName: tc.Name}
		}
	}
	return nil
}

// extractToolName pulls the "name" field out of a JSON-encoded tool_use
// segment without fully unmarshaling it. Falls back to "" on any error.
func extractToolName(segContent string) string {
	type nameOnly struct {
		Name string `json:"name"`
	}
	var n nameOnly
	if err := json.Unmarshal([]byte(segContent), &n); err != nil {
		return ""
	}
	return n.Name
}
