package tui

import (
	"fmt"
	"path/filepath"
	"strings"
	"time"

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

	// Render prompt metadata row: agent · model · provider
	var agentName, modelName, providerName string
	if m.activeModel != nil {
		modelName = m.activeModel.Name
	}
	if m.activeProvider != "" {
		providerName = m.activeProvider
	}
	metadataRow := RenderPromptMetadata(agentName, modelName, providerName, m.theme, m.width-m.sidebarWidth)

	// Determine border highlight color
	borderColor := m.theme.Border
	if m.streaming {
		borderColor = m.theme.Brand
	} else if m.thinking {
		borderColor = m.theme.Thinking
	}

	// Build the input container: textarea + metadata
	inputContainer := lipgloss.JoinVertical(lipgloss.Top, inputStr, metadataRow)

	// Apply left border + background to the container
	borderStyle := lipgloss.NewStyle().
		Border(theme.SplitBorder, true, false, false, false).
		BorderForeground(borderColor).
		Background(m.theme.BackgroundElement).
		Padding(0, 2, 0, 2)
	borderedInput := borderStyle.Render(inputContainer)

	// Render slash command suggestions dropdown
	var suggestionStr string
	if m.slashVisible && len(m.slashSuggestions) > 0 {
		suggestionStr = m.renderSlashSuggestions()
	}

	// Bottom border continuation line
	bottomBorder := RenderPromptBottomBorder(borderColor, m.width-m.sidebarWidth)

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
	// KeyboardHints removed - WhichKey provides the same info

	status := RenderStatusBar(m.theme, m.GetStatusText(), m.lastActivity, m.width-m.sidebarWidth, statusInfo)

	// Assemble the REPL content
	var contentParts []string
	contentParts = append(contentParts, viewportStr)
	contentParts = append(contentParts, borderedInput)
	if suggestionStr != "" {
		contentParts = append(contentParts, suggestionStr)
	}
	contentParts = append(contentParts, bottomBorder)
	contentParts = append(contentParts, status)

	replContent := lipgloss.JoinVertical(lipgloss.Top, contentParts...)

	// Render fallback banner if active
	if m.fallbackBanner != "" {
		bannerStyle := lipgloss.NewStyle().
			Background(m.theme.Warning).
			Foreground(lipgloss.Color("#000000")).
			Padding(0, 1).
			Bold(true).
			Width(m.width - m.sidebarWidth)
		banner := bannerStyle.Render("[!] " + m.fallbackBanner)
		replContent = lipgloss.JoinVertical(lipgloss.Top, banner, replContent)
	}

	return replContent
}

func (m *ReplModel) renderMessages() {
	var b strings.Builder

	for _, msg := range m.messages {
		if m.msgRenderer != nil {
			rendered := m.msgRenderer.RenderMessage(msg, m.width)
			b.WriteString(rendered)
			b.WriteString("\n")
		} else {
			role := msg.Role
			if role == "user" {
				role = "You"
			} else if role == "assistant" {
				role = "Assistant"
			}
			b.WriteString(fmt.Sprintf("%s: %s\n", role, msg.Content))
		}
	}

	if m.streaming {
		b.WriteString(m.renderStreamingContent())
	}

	m.viewport.SetContent(b.String())
}

func (m *ReplModel) renderStreamingContent() string {
	if m.msgRenderer == nil {
		content := m.streamContent.String()
		if content != "" {
			return fmt.Sprintf("Assistant: %s\n", content)
		}
		return "Assistant: ...\n"
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

	return m.msgRenderer.RenderMessage(msg, m.width)
}

func (m *ReplModel) renderWelcome() string {
	if m.width == 0 || m.height == 0 {
		return "Welcome to M31A"
	}

	// ASCII art banner logo: M31A rendered in block characters
	logo := m.renderBlockLogo("M31A")

	// Provider status card
	providerCard := m.renderProviderCard()

	// Quick actions panel
	quickActions := m.renderQuickActions()

	// Session stats bar
	statsBar := m.renderSessionStats()

	// Input area — matches the screenshot: grey box with blue left border accent
	inputBoxStyle := lipgloss.NewStyle().
		Border(lipgloss.NormalBorder(), true, false, false, false).
		BorderForeground(m.theme.Brand).
		Background(lipgloss.Color("#2A2A2A")).
		Padding(0, 2, 0, 2)

	// Build the placeholder line
	placeholderText := lipgloss.NewStyle().
		Foreground(m.theme.TextMuted).
		Render("What would you like to build?")

	// Context sub-line (model · provider)
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
			Foreground(m.theme.TextMuted).
			Render(strings.Join(contextParts, " · "))
	} else {
		contextLine = lipgloss.NewStyle().
			Foreground(m.theme.TextMuted).
			Render("M31A")
	}

	// Assemble the input box content
	inputLines := []string{placeholderText}
	if contextLine != "" {
		inputLines = append(inputLines, contextLine)
	}
	inputBox := inputBoxStyle.Render(lipgloss.JoinVertical(lipgloss.Top, inputLines...))

	// Keyboard hints
	hints := lipgloss.NewStyle().
		Foreground(m.theme.TextMuted).
		Render("ctrl+p commands  ctrl+b sidebar  ctrl+x for more")

	// Tip
	tips := []string{
		"Run /config to set up your AI provider",
		"Use @filepath to include file context",
		"Press /help to see all commands",
		"Use !prefix for shell commands",
		"Press ctrl+x m to cycle models",
	}
	tipIdx := int(time.Now().Unix()) % len(tips)
	tipDot := lipgloss.NewStyle().Foreground(m.theme.Brand).Render("•")
	tipText := lipgloss.NewStyle().
		Foreground(m.theme.TextMuted).
		Render("Tip: " + tips[tipIdx])
	tip := tipDot + " " + tipText

	// Bottom corners: cwd and version
	cwdLabel := lipgloss.NewStyle().
		Foreground(m.theme.TextMuted).
		Render(filepath.Base(m.cwd))
	versionLabel := lipgloss.NewStyle().
		Foreground(m.theme.TextMuted).
		Render("v0.1.0")

	logoPadded := lipgloss.Place(m.width, lipgloss.Height(logo)+1, lipgloss.Center, lipgloss.Top, logo)

	// Build the main layout
	// Stack: Logo → Provider Card → Quick Actions → Input Box → Stats → Hints
	mainContent := lipgloss.JoinVertical(lipgloss.Center,
		logoPadded,
		"",
		providerCard,
		"",
		quickActions,
		"",
		inputBox,
		"",
		statsBar,
		"",
		hints,
	)

	// Use Place to center the group in the available space, pushing tip to bottom
	availableHeight := m.viewport.Height - 4 // reserve space for tip and bottom bar
	content := lipgloss.Place(m.width, availableHeight, lipgloss.Center, lipgloss.Center, mainContent)

	tipArea := lipgloss.Place(m.width-4, 2, lipgloss.Left, lipgloss.Top, tip)
	bottomBar := lipgloss.JoinHorizontal(lipgloss.Left,
		cwdLabel,
		lipgloss.Place(m.width-lipgloss.Width(cwdLabel)-lipgloss.Width(versionLabel)-4, 1, lipgloss.Right, lipgloss.Top, versionLabel),
	)
	bottomArea := lipgloss.Place(m.width, 3, lipgloss.Center, lipgloss.Bottom,
		lipgloss.JoinVertical(lipgloss.Top, tipArea, bottomBar),
	)

	return lipgloss.JoinVertical(lipgloss.Top, content, bottomArea)
}

// renderProviderCard shows current model/provider status or setup prompt.
func (m *ReplModel) renderProviderCard() string {
	t := theme.Default()

	if m.activeModel == nil || m.activeProvider == "" {
		// Not configured - show setup prompt
		style := lipgloss.NewStyle().
			Background(t.Surface).
			Border(lipgloss.RoundedBorder()).
			BorderForeground(t.Warning).
			Padding(0, 2)

		warningDot := lipgloss.NewStyle().Foreground(t.Warning).Render("●")
		text := lipgloss.NewStyle().Foreground(t.TextSecondary).Render("No provider configured — run /settings to get started")
		return style.Render(warningDot + " " + text)
	}

	// Configured - show provider info
	style := lipgloss.NewStyle().
		Background(t.Surface).
		Border(lipgloss.RoundedBorder()).
		BorderForeground(t.Success).
		Padding(0, 2)

	// Model name
	modelStyle := lipgloss.NewStyle().Foreground(t.TextPrimary).Bold(true)
	modelBadge := modelStyle.Render(m.activeModel.Name)

	// Provider badge
	providerBadge := components.NewBadge(m.activeProvider, components.BadgeBrand).Render()

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

	return style.Render(lipgloss.JoinVertical(lipgloss.Left, parts...))
}

// renderSessionStats shows basic session statistics.
func (m *ReplModel) renderSessionStats() string {
	t := m.theme

	stats := []struct {
		label  string
		value  string
		symbol string
	}{
		{label: "Msgs", value: fmt.Sprintf("%d", len(m.messages)), symbol: "#"},
		{label: "Tokens", value: "0", symbol: "T"},
		{label: "Cost", value: "$0.00", symbol: "$"},
		{label: "Session", value: "New", symbol: "S"},
	}

	if m.lastUsage != nil {
		stats[1].value = components.FormatMetric(m.lastUsage.TotalTokens)
		stats[2].value = components.FormatCost(m.lastCost)
	}

	parts := make([]string, len(stats))
	for i, s := range stats {
		symbolStyle := lipgloss.NewStyle().Foreground(t.Brand).Bold(true)
		labelStyle := lipgloss.NewStyle().Foreground(t.TextSecondary)
		valueStyle := lipgloss.NewStyle().Foreground(t.TextPrimary)

		parts[i] = symbolStyle.Render(s.symbol) + " " + labelStyle.Render(s.label) + ": " + valueStyle.Render(s.value)
	}

	return lipgloss.JoinHorizontal(lipgloss.Left, parts...)
}

// renderBlockLogo renders a string in a 5x5 block-letter pixel art style.
// Each character is composed of '#' and '.' characters.
func (m *ReplModel) renderBlockLogo(s string) string {
	// ASCII art banner for M31A
	banner := `░███     ░███  ░██████    ░██      ░███    
░████   ░████ ░██   ░██ ░████     ░██░██   
░██░██ ░██░██       ░██   ░██    ░██  ░██  
░██ ░████ ░██   ░█████    ░██   ░█████████ 
░██  ░██  ░██       ░██   ░██   ░██    ░██ 
░██       ░██ ░██   ░██   ░██   ░██    ░██ 
░██       ░██  ░██████  ░██████ ░██    ░██ 
                                           
                                           
                                           `

	// Color the banner with the brand color
	lines := strings.Split(banner, "\n")
	styled := make([]string, len(lines))
	for i, line := range lines {
		styled[i] = lipgloss.NewStyle().Foreground(m.theme.Brand).Render(line)
	}
	return lipgloss.JoinVertical(lipgloss.Top, styled...)
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
