package tui

import (
	"fmt"
	"path/filepath"
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
	inputStr := m.textarea.View()

	// Render model context line above textarea: │ M31A · model-name · provider
	modelContextLine := m.renderModelContextLine()

	// Bottom border with Enter-to-send hint
	bottomBorder := m.renderInputBottomBorder()

	// Assemble input frame: model context + textarea + bottom border
	inputFrame := lipgloss.JoinVertical(lipgloss.Top, modelContextLine, inputStr, bottomBorder)

	// Determine border highlight color
	borderColor := m.theme.Border
	if m.streaming {
		borderColor = m.theme.Brand
	} else if m.thinking {
		borderColor = m.theme.Thinking
	}

	// Apply left border + background to the input frame
	borderStyle := lipgloss.NewStyle().
		Border(theme.SplitBorder, true, false, false, false).
		BorderForeground(borderColor).
		Background(m.theme.BackgroundElement).
		Padding(0, 2, 0, 2)
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

	return lipgloss.JoinHorizontal(lipgloss.Top,
		gutterStyle.Render("│"),
		lipgloss.NewStyle().Foreground(t.TextMuted).PaddingLeft(1).Render(contextText),
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

func (m *ReplModel) renderWelcome() string {
	if m.width == 0 || m.height == 0 {
		return "Welcome to M31A"
	}

	// 1. Logo (compact, 4 lines)
	logo := m.renderLogo()

	// 2. Provider status card
	providerCard := m.renderProviderCard()

	// 3. Input area with placeholder
	inputBox := m.renderInputBox()

	// 4. Keyboard hints
	hints := renderKeyboardHints(m.theme)

	// 5. Bottom bar (cwd + version)
	bottomBar := m.renderBottomBar()

	// Stack vertically, centered
	content := lipgloss.JoinVertical(lipgloss.Center,
		logo,
		"",
		providerCard,
		"",
		inputBox,
		"",
		hints,
	)

	// Center in available space
	availableHeight := m.height - 4 // reserve for bottom bar
	centered := lipgloss.Place(m.width, availableHeight, lipgloss.Center, lipgloss.Center, content)

	return lipgloss.JoinVertical(lipgloss.Top, centered, bottomBar)
}

// renderProviderCard shows current model/provider status or setup prompt.
func (m *ReplModel) renderProviderCard() string {
	t := m.theme

	if m.activeModel == nil || m.activeProvider == "" {
		// Not configured - show setup prompt
		style := lipgloss.NewStyle().
			Border(lipgloss.RoundedBorder()).
			BorderForeground(t.Warning).
			Padding(0, 2).
			Width(40)

		warningDot := lipgloss.NewStyle().Foreground(t.Warning).Render("●")
		title := lipgloss.NewStyle().Foreground(t.TextPrimary).Bold(true).Render("No provider configured")
		subtitle := lipgloss.NewStyle().Foreground(t.TextSecondary).Render("Run /settings to get started")

		content := lipgloss.JoinVertical(lipgloss.Left,
			warningDot+" "+title,
			subtitle,
		)

		return style.Render(content)
	}

	// Configured - show provider info
	style := lipgloss.NewStyle().
		Border(lipgloss.RoundedBorder()).
		BorderForeground(t.Success).
		Padding(0, 2).
		Width(40)

	// Model name
	modelStyle := lipgloss.NewStyle().Foreground(t.TextPrimary).Bold(true)
	modelBadge := modelStyle.Render(m.activeModel.Name)

	// Provider badge
	providerBadge := components.NewBadge(m.activeProvider, components.BadgeBrand, m.theme).Render()

	// Pricing info
	pricingText := ""
	if m.activeModel.Pricing.InputPerMToken > 0 || m.activeModel.Pricing.OutputPerMToken > 0 {
		pricingText = lipgloss.NewStyle().Foreground(t.TextMuted).Render(
			fmt.Sprintf("in $%.2f/M  out $%.2f/M",
				m.activeModel.Pricing.InputPerMToken,
				m.activeModel.Pricing.OutputPerMToken))
	}

	// Context window
	contextText := ""
	if m.activeModel.ContextLength > 0 {
		contextText = lipgloss.NewStyle().Foreground(t.TextMuted).Render(
			fmt.Sprintf("ctx %s", components.FormatMetric(int(m.activeModel.ContextLength))))
	}

	parts := []string{modelBadge + " " + providerBadge}
	if pricingText != "" {
		parts = append(parts, pricingText)
	}
	if contextText != "" {
		parts = append(parts, contextText)
	}

	// Recent session activity sparkline (optional, shown when we have history)
	if m.sessionSparkline != "" {
		sparkStyle := lipgloss.NewStyle().Foreground(t.TextSecondary)
		parts = append(parts, sparkStyle.Render(m.sessionSparkline))
	}

	return style.Render(lipgloss.JoinVertical(lipgloss.Left, parts...))
}

// renderLogo renders a clean ASCII art logo for M31A.
func (m *ReplModel) renderLogo() string {
	version := m.version
	if version == "" {
		version = "dev"
	}
	logo := `  __  _______  __
 /  |/  / __ \/ _/
 / /|_/ / /_/ / _/
 /_/  /_/\____/_/ ` + version

	lines := strings.Split(logo, "\n")
	styled := make([]string, len(lines))
	for i, line := range lines {
		styled[i] = lipgloss.NewStyle().Foreground(m.theme.Brand).Render(line)
	}
	return lipgloss.JoinVertical(lipgloss.Top, styled...)
}

// renderInputBox renders the input area with placeholder text.
func (m *ReplModel) renderInputBox() string {
	t := m.theme

	style := lipgloss.NewStyle().
		Border(lipgloss.NormalBorder(), true, false, false, false).
		BorderForeground(t.Brand).
		Background(t.Surface).
		Padding(0, 2).
		Width(50)

	placeholder := lipgloss.NewStyle().
		Foreground(t.TextMuted).
		Render("Type a message, /command, or goal...")

	// Context line (model · provider)
	var contextParts []string
	if m.activeModel != nil {
		contextParts = append(contextParts, m.activeModel.Name)
	}
	if m.activeProvider != "" {
		contextParts = append(contextParts, m.activeProvider)
	}
	contextLine := ""
	if len(contextParts) > 0 {
		contextLine = lipgloss.NewStyle().
			Foreground(t.TextMuted).
			Render(strings.Join(contextParts, " · "))
	} else {
		contextLine = lipgloss.NewStyle().
			Foreground(t.TextMuted).
			Render("M31A")
	}

	content := lipgloss.JoinVertical(lipgloss.Top, placeholder, contextLine)
	return style.Render(content)
}

// renderKeyboardHints renders keyboard shortcut hints.
func renderKeyboardHints(t theme.Theme) string {
	hints := []struct {
		key   string
		label string
	}{
		{"ctrl+p", "commands"},
		{"ctrl+b", "sidebar"},
		{"ctrl+x", "leader"},
	}

	parts := make([]string, len(hints))
	for i, h := range hints {
		keyStyle := lipgloss.NewStyle().
			Foreground(t.Brand).
			Bold(true)
		labelStyle := lipgloss.NewStyle().
			Foreground(t.TextMuted)

		parts[i] = keyStyle.Render(h.key) + " " + labelStyle.Render(h.label)
	}

	return lipgloss.JoinHorizontal(lipgloss.Center, parts...)
}

// renderBottomBar renders the bottom bar with cwd and version.
func (m *ReplModel) renderBottomBar() string {
	t := m.theme

	cwdLabel := lipgloss.NewStyle().
		Foreground(t.TextMuted).
		Render(filepath.Base(m.cwd))

	version := m.version
	if version == "" {
		version = "dev"
	}
	versionLabel := lipgloss.NewStyle().
		Foreground(t.TextMuted).
		Render(version)

	// Right-align version
	spacer := m.width - lipgloss.Width(cwdLabel) - lipgloss.Width(versionLabel) - 4
	if spacer < 0 {
		spacer = 0
	}

	return lipgloss.JoinHorizontal(lipgloss.Left,
		"  "+cwdLabel,
		strings.Repeat(" ", spacer),
		versionLabel+"  ",
	)
}

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
