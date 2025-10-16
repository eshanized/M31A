package tui

import (
	"strings"

	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/tui/components"
	"github.com/eshanized/M31A/internal/tui/theme"
	"github.com/eshanized/M31A/internal/types"
)

func (m *ReplModel) View() string {
	if m.width == 0 {
		return "Initializing..."
	}

	viewportStr := m.viewport.View()

	// Render model context line above textarea: │ M31A · model-name · provider
	modelContextLine := m.renderModelContextLine()

	// Bottom border with Enter-to-send hint
	bottomBorder := m.renderInputBottomBorder()

	// Assemble input frame: model context + textarea + bottom border
	inputStr := m.textarea.View()
	inputFrame := lipgloss.JoinVertical(lipgloss.Top, modelContextLine, inputStr, bottomBorder)

	// Determine border highlight color
	borderColor := m.theme.Border
	if m.streaming {
		borderColor = m.theme.Brand
	} else if m.thinking {
		borderColor = m.theme.Thinking
	}

	// Apply left border + background to the input frame
	replWidth := m.replWidth()
	borderStyle := lipgloss.NewStyle().
		Border(theme.SplitBorder, true, false, false, false).
		BorderForeground(borderColor).
		Background(m.theme.BackgroundElement).
		Padding(0, 2, 0, 2).
		Width(replWidth)
	borderedInput := borderStyle.Render(inputFrame)

	// Render slash command suggestions dropdown
	var suggestionStr string
	if m.slashVisible && len(m.slashSuggestions) > 0 {
		suggestionStr = m.renderSlashSuggestions()
	}

	// Git status strip (above input, when sidebar is hidden)
	gitStrip := m.renderGitStatusStrip()

	// Status bar below the prompt
	var statusInfo *StatusBarInfo
	if m.lastUsage != nil && m.cfg != nil && m.cfg.UI.ShowCostEstimate {
		statusInfo = &StatusBarInfo{
			PromptTokens: m.lastUsage.PromptTokens,
			TotalTokens:  m.lastUsage.TotalTokens,
			Cost:         m.lastCost,
			ShowCost:     true,
		}
	} else {
		statusInfo = &StatusBarInfo{}
	}
	if m.keyRegistry != nil {
		statusInfo.LeaderActive = m.keyRegistry.IsLeaderActive()
		if !statusInfo.LeaderActive {
			statusInfo.WhichKey = m.keyRegistry.RenderWhichKey(CtxREPL, m.theme, (m.width-m.sidebarWidth)/2)
		}
	}
	statusInfo.IsStreaming = m.streaming
	statusInfo.IsThinking = m.thinking

	status := RenderStatusBar(m.theme, m.GetStatusText(), m.lastActivity, m.width-m.sidebarWidth, statusInfo)

	// Assemble the REPL content
	var contentParts []string
	contentParts = append(contentParts, viewportStr)
	if gitStrip != "" {
		contentParts = append(contentParts, gitStrip)
	}
	contentParts = append(contentParts, borderedInput)
	if suggestionStr != "" {
		contentParts = append(contentParts, suggestionStr)
	}
	contentParts = append(contentParts, status)

	replContent := lipgloss.JoinVertical(lipgloss.Top, contentParts...)

	// Render fallback banner if active
	if m.fallbackBanner != "" {
		bannerStyle := lipgloss.NewStyle().
			Background(m.theme.Warning).
			Foreground(m.theme.Background).
			Padding(0, 1).
			Bold(true).
			Width(m.width - m.sidebarWidth)
		banner := bannerStyle.Render("[!] " + m.fallbackBanner + "  [x] Dismiss")
		replContent = lipgloss.JoinVertical(lipgloss.Top, banner, replContent)
	}

	return replContent
}

// renderModelContextLine renders the input frame's model context line:
// │ M31A · model-name · provider
func (m *ReplModel) renderModelContextLine() string {
	t := m.theme
	width := m.width - m.sidebarWidth

	var parts []string
	parts = append(parts, "M31A")

	if m.activeModel != nil && m.activeModel.Name != "" {
		parts = append(parts, m.activeModel.Name)
	}
	if m.activeProvider != "" {
		parts = append(parts, m.activeProvider)
	}

	contextText := strings.Join(parts, " · ")
	gutterStyle := lipgloss.NewStyle().Foreground(t.Brand)

	return lipgloss.NewStyle().Width(width).Render(
		lipgloss.JoinHorizontal(lipgloss.Top,
			gutterStyle.Render("│"),
			lipgloss.NewStyle().Foreground(t.TextMuted).PaddingLeft(1).Render(contextText),
		),
	)
}

// renderInputBottomBorder renders the bottom border of the input frame:
// ╹▀▀▀───────────────── Enter to send · Ctrl+C cancel ────────────────
func (m *ReplModel) renderInputBottomBorder() string {
	t := m.theme
	width := m.width - m.sidebarWidth

	return RenderPromptBottomBorder(t.Border, width)
}

// renderGitStatusStrip shows modified files in a thin row above the input.
// Only shown when sidebar is hidden and there are modified files.
func (m *ReplModel) renderGitStatusStrip() string {
	// Only show when sidebar is hidden
	if m.sidebarWidth > 0 {
		return ""
	}

	// Check for modified files via git status (read-only, non-blocking)
	// We'll use a simple approach: check if we have cwd set and run git status
	if m.cwd == "" {
		return ""
	}

	// For now, return empty — the git strip is populated externally
	// via SetGitStatusStrip() or rendered from session state.
	// This is the structural hook for the feature.
	return ""
}

func (m *ReplModel) renderMessages() {
	var b strings.Builder
	availableWidth := m.width - m.sidebarWidth

	for i, msg := range m.messages {
		// Insert timestamp bar between messages (except before the first one)
		if i > 0 && !msg.CreatedAt.IsZero() {
			tsBar := components.RenderTimestampBar(m.theme, msg.CreatedAt, availableWidth)
			if tsBar != "" {
				b.WriteString(tsBar)
				b.WriteString("\n")
			}
		}

		if m.msgRenderer != nil {
			rendered := m.msgRenderer.RenderMessage(msg, availableWidth)
			b.WriteString(rendered)
			b.WriteString("\n")
		} else {
			// Fallback: simple rendering with gutter
			role := "M31A"
			gutterColor := m.theme.Brand
			if msg.Role == "user" {
				role = "USER"
				gutterColor = m.theme.TextSecondary
			}
			gutter := lipgloss.NewStyle().Foreground(gutterColor).Render("│ " + role)
			b.WriteString(gutter)
			b.WriteString("\n  ")
			b.WriteString(msg.Content)
			b.WriteString("\n")
		}
	}

	// BUG-06 fix: show quick actions welcome screen when no messages
	if len(m.messages) == 0 && !m.streaming {
		b.WriteString(m.renderQuickActions())
	}

	if m.streaming {
		b.WriteString(m.renderStreamingContent())
	}

	// Add scroll indicator if not at bottom
	if !m.atBottom() && len(m.messages) > 0 {
		scrollIndicator := lipgloss.NewStyle().Foreground(m.theme.Brand).Render("▼")
		b.WriteString(scrollIndicator)
		b.WriteString("\n")
	}

	m.viewport.SetContent(b.String())
}

func (m *ReplModel) renderStreamingContent() string {
	if m.msgRenderer == nil {
		content := m.streamContent.String()
		if content != "" {
			gutter := lipgloss.NewStyle().Foreground(m.theme.Brand).Render("│ M31A")
			return gutter + "\n  " + content + "\n"
		}
		gutter := lipgloss.NewStyle().Foreground(m.theme.Brand).Render("│ M31A")
		return gutter + "\n  ...\n"
	}

	var segments []types.MessageSegment
	segments = append(segments, m.streamSegments...)

	if m.streamContent.Len() > 0 {
		partial := m.streamContent.String()
		if m.activeSegmentType == "thinking" {
			segments = append(segments, types.MessageSegment{
				Type:      "thinking",
				Content:   partial,
				Visible:   true,
				StartedAt: m.thinkingStartAt,
			})
		} else {
			segments = append(segments, types.MessageSegment{
				Type:    "content",
				Content: partial,
				Visible: true,
			})
		}
	}

	msg := types.Message{
		Role:     "assistant",
		Content:  m.streamContent.String(),
		Segments: segments,
	}

	availableWidth := m.width - m.sidebarWidth
	return m.msgRenderer.RenderMessage(msg, availableWidth)
}

// renderWelcome, renderProviderCard, renderLogo, renderInputBox,
// renderKeyboardHints, and renderBottomBar are defined in repl_welcome.go

// renderSlashSuggestions renders the autocomplete dropdown for slash commands.
func (m *ReplModel) renderSlashSuggestions() string {
	if len(m.slashSuggestions) == 0 {
		return ""
	}

	t := m.theme
	maxWidth := m.width - m.sidebarWidth - 4 // account for padding
	if maxWidth < 20 {
		return ""
	}

	var lines []string
	for i, cmd := range m.slashSuggestions {
		if i >= 5 {
			break // limit to 5 visible rows
		}

		// Build the display text: /command  description
		display := cmd.Slash
		if cmd.Name != "" {
			display = display + "  " + cmd.Name
		}
		if cmd.Description != "" {
			display = display + "    " + cmd.Description
		}

		// Truncate if too wide
		runes := []rune(display)
		if lipgloss.Width(display) > maxWidth {
			display = string(runes[:maxWidth-3]) + "..."
		}

		var style lipgloss.Style
		if i == m.slashSelected {
			style = lipgloss.NewStyle().
				Background(t.SurfaceElevated).
				Foreground(t.Text).
				Padding(0, 1)
			display = "> " + display
		} else {
			style = lipgloss.NewStyle().
				Foreground(t.TextMuted).
				PaddingLeft(3)
		}

		lines = append(lines, style.Render(display))
	}

	// Wrap in a bordered box
	suggestionStyle := lipgloss.NewStyle().
		Border(lipgloss.NormalBorder(), true, false, false, false).
		BorderForeground(t.Border).
		Background(t.BackgroundElement).
		PaddingTop(0).
		PaddingBottom(0).
		Width(maxWidth + 2)

	return suggestionStyle.Render(lipgloss.JoinVertical(lipgloss.Top, lines...))
}
