package components

import (
	"fmt"
	"time"

	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/tools"
	"github.com/eshanized/M31A/internal/tui/theme"
	"github.com/eshanized/M31A/internal/types"
)

type PermissionModal struct {
	request   tools.PermissionRequest
	theme     theme.Theme
	elapsed   time.Duration
	timeout   time.Duration
	responded bool
	response  tools.PermissionResponse
}

func NewPermissionModal(request tools.PermissionRequest, t theme.Theme, timeout time.Duration) *PermissionModal {
	return &PermissionModal{
		request: request,
		theme:   t,
		timeout: timeout,
	}
}

// Clear resets the modal to its inactive state, clearing rule context.
func (m *PermissionModal) Clear() {
	m.responded = false
	m.response = tools.PermissionResponse{}
	m.elapsed = 0
}

func (m *PermissionModal) Render(width, height int) string {
	modalWidth := 60
	if width < modalWidth+4 {
		modalWidth = width - 4
	}
	if modalWidth < 40 {
		modalWidth = 40
	}

	lockBadge := lipgloss.NewStyle().
		Foreground(m.theme.Warning).
		Bold(true).
		Render("[LOCK]")

	title := lipgloss.NewStyle().
		Foreground(m.theme.Warning).
		Bold(true).
		Render("Permission Required")

	titleLine := lipgloss.JoinHorizontal(lipgloss.Top, lockBadge, lipgloss.NewStyle().Render(" "), title)

	riskStyle := m.riskStyle()
	riskLabel := riskStyle.Render(fmt.Sprintf(" [%s] ", string(m.request.RiskLevel)))

	toolInfo := lipgloss.NewStyle().
		Foreground(m.theme.TextPrimary).
		Render(fmt.Sprintf("Tool:     %s", m.request.ToolName))

	riskInfo := lipgloss.JoinHorizontal(lipgloss.Top,
		lipgloss.NewStyle().Foreground(m.theme.TextPrimary).Render("Risk:     "),
		riskLabel,
	)

	cmdBox := lipgloss.NewStyle().
		Border(lipgloss.RoundedBorder()).
		BorderForeground(m.theme.Border).
		Padding(0, 1).
		Width(modalWidth - 6).
		Render(m.request.Command)

	keys := lipgloss.NewStyle().
		Foreground(m.theme.TextSecondary).
		Render("[Y] Allow Once    [A] Always Allow\n[N] Deny          [E] Exit M31A")

	countdown := lipgloss.NewStyle().
		Foreground(m.theme.Warning).
		Render(fmt.Sprintf("Auto-deny in %s...", formatDuration(m.Remaining())))

	// Rule context section (displayed when a permission rule matched)
	var ruleInfo string
	if m.request.RuleTool != "" || m.request.RulePattern != "" {
		ruleInfo = lipgloss.NewStyle().Faint(true).Render(
			fmt.Sprintf("  Matched rule: tool=%q pattern=%q action=%q",
				m.request.RuleTool, m.request.RulePattern, m.request.RuleAction),
		)
	}

	modalContent := lipgloss.JoinVertical(lipgloss.Top,
		titleLine,
		"",
		toolInfo,
		riskInfo,
		"",
		lipgloss.NewStyle().Foreground(m.theme.TextPrimary).Render("Command:"),
		cmdBox,
		"",
	)
	if ruleInfo != "" {
		modalContent = lipgloss.JoinVertical(lipgloss.Top,
			modalContent,
			ruleInfo,
			"",
		)
	}
	modalContent = lipgloss.JoinVertical(lipgloss.Top,
		modalContent,
		keys,
		"",
		countdown,
	)

	modal := lipgloss.NewStyle().
		Background(m.theme.SurfaceElevated).
		Foreground(m.theme.TextPrimary).
		Padding(1, 2).
		Border(lipgloss.DoubleBorder()).
		BorderForeground(m.theme.Brand).
		Width(modalWidth).
		Render(modalContent)

	return lipgloss.Place(width, height,
		lipgloss.Center, lipgloss.Center,
		modal,
	)
}

func (m *PermissionModal) Allow() tools.PermissionResponse {
	m.responded = true
	m.response = tools.PermissionResponse{Allowed: true, Remember: false}
	return m.response
}

func (m *PermissionModal) AllowAlways() tools.PermissionResponse {
	m.responded = true
	m.response = tools.PermissionResponse{Allowed: true, Remember: true}
	return m.response
}

func (m *PermissionModal) Deny() tools.PermissionResponse {
	m.responded = true
	m.response = tools.PermissionResponse{Allowed: false, Remember: false}
	return m.response
}

func (m *PermissionModal) IsResponded() bool {
	return m.responded
}

func (m *PermissionModal) Tick() {
	m.elapsed += time.Second / 10
}

func (m *PermissionModal) Remaining() time.Duration {
	remaining := m.timeout - m.elapsed
	if remaining < 0 {
		return 0
	}
	return remaining
}

func (m *PermissionModal) riskStyle() lipgloss.Style {
	switch m.request.RiskLevel {
	case types.RiskDangerous:
		return lipgloss.NewStyle().
			Background(m.theme.Error).
			Foreground(lipgloss.Color("#000000")).
			Bold(true).
			Padding(0, 1)
	case types.RiskDestructive:
		return lipgloss.NewStyle().
			Background(m.theme.Error).
			Foreground(lipgloss.Color("#000000")).
			Bold(true).
			Padding(0, 1)
	case types.RiskMedium:
		return lipgloss.NewStyle().
			Background(m.theme.Warning).
			Foreground(lipgloss.Color("#000000")).
			Padding(0, 1)
	case types.RiskSafe:
		return lipgloss.NewStyle().
			Background(m.theme.TextSecondary).
			Foreground(lipgloss.Color("#000000")).
			Padding(0, 1)
	default:
		return lipgloss.NewStyle().
			Background(m.theme.TextSecondary).
			Foreground(lipgloss.Color("#000000")).
			Padding(0, 1)
	}
}

func formatDuration(d time.Duration) string {
	totalSecs := int(d.Seconds())
	mins := totalSecs / 60
	secs := totalSecs % 60
	return fmt.Sprintf("%d:%02d", mins, secs)
}
