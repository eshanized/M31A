package components

import (
	"fmt"
	"strings"
	"time"

	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/tools"
	"github.com/eshanized/M31A/internal/tui/theme"
	"github.com/eshanized/M31A/internal/types"
)

// DefaultPermissionTimeout is the default permission modal timeout, derived from the shared constant.
var DefaultPermissionTimeout = time.Duration(types.DefaultPermissionTimeout) * time.Second

type PermissionModal struct {
	request tools.PermissionRequest
	theme   theme.Theme
	elapsed time.Duration
	timeout time.Duration
}

func NewPermissionModal(request tools.PermissionRequest, t theme.Theme, timeout time.Duration) *PermissionModal {
	if timeout <= 0 {
		timeout = DefaultPermissionTimeout
	}
	return &PermissionModal{
		request: request,
		theme:   t,
		timeout: timeout,
	}
}

// Clear resets the modal to its inactive state.
func (m *PermissionModal) Clear() {
	m.elapsed = 0
}

func (m *PermissionModal) Render(width, height int) string {
	modalWidth := 60
	if width < modalWidth+4 {
		modalWidth = width - 4
	}
	if modalWidth < 20 {
		modalWidth = 20
	}

	// ── Title ─────────────────────────────────────────────────────────────
	lockBadge := lipgloss.NewStyle().
		Foreground(m.theme.Warning).
		Bold(true).
		Render("🔒")

	title := lipgloss.NewStyle().
		Foreground(m.theme.Warning).
		Bold(true).
		Render("Permission Required")

	titleLine := lipgloss.JoinHorizontal(lipgloss.Top, lockBadge, " ", title)

	// ── Tool & risk info ──────────────────────────────────────────────────
	riskBadge := m.riskStyle().Render(fmt.Sprintf(" %s ", riskLabel(m.request.RiskLevel)))

	toolLine := lipgloss.JoinHorizontal(lipgloss.Top,
		lipgloss.NewStyle().Foreground(m.theme.TextMuted).Render("Tool  "),
		lipgloss.NewStyle().Foreground(m.theme.TextPrimary).Bold(true).Render(m.request.ToolName),
		lipgloss.NewStyle().Render("  "),
		lipgloss.NewStyle().Foreground(m.theme.TextMuted).Render("Risk  "),
		riskBadge,
	)

	// ── Command box ───────────────────────────────────────────────────────
	cmdContentW := modalWidth - 10
	if cmdContentW < 8 {
		cmdContentW = 8
	}
	highlighted := highlightCommand(m.request.Command, m.theme)
	highlighted = TruncateWithEllipsis(highlighted, cmdContentW)
	cmdBox := lipgloss.NewStyle().
		Border(theme.DoubleBorder).
		BorderForeground(m.theme.Border).
		Padding(0, 1).
		Width(modalWidth - 6).
		Render(highlighted)

	// ── Keybindings ───────────────────────────────────────────────────────
	keyStyle := lipgloss.NewStyle().Foreground(m.theme.TextSecondary)
	keys := lipgloss.JoinVertical(lipgloss.Left,
		lipgloss.JoinHorizontal(lipgloss.Top,
			keyStyle.Render("[Y]"),
			lipgloss.NewStyle().Foreground(m.theme.TextMuted).Render(" Allow once    "),
			keyStyle.Render("[A]"),
			lipgloss.NewStyle().Foreground(m.theme.TextMuted).Render(" Always allow"),
		),
		lipgloss.JoinHorizontal(lipgloss.Top,
			keyStyle.Render("[N]"),
			lipgloss.NewStyle().Foreground(m.theme.TextMuted).Render(" Deny          "),
			keyStyle.Render("[Esc]"),
			lipgloss.NewStyle().Foreground(m.theme.TextMuted).Render(" Exit"),
		),
	)

	// ── Countdown ─────────────────────────────────────────────────────────
	remaining := m.Remaining()
	var countdown string
	if remaining <= 0 {
		countdown = lipgloss.NewStyle().
			Foreground(m.theme.Error).
			Bold(true).
			Render("Tool will be rejected")
	} else if remaining <= 30*time.Second {
		countdown = lipgloss.NewStyle().
			Foreground(m.theme.Error).
			Render(fmt.Sprintf("Auto-deny in %s", formatDurationClock(remaining)))
	} else {
		countdown = lipgloss.NewStyle().
			Foreground(m.theme.Warning).
			Render(fmt.Sprintf("Auto-deny in %s", formatDurationClock(remaining)))
	}

	countdownBar := m.renderCountdownBar(modalWidth - 6)

	// ── Rule context ──────────────────────────────────────────────────────
	var ruleInfo string
	if m.request.RuleTool != "" || m.request.RulePattern != "" {
		ruleInfo = lipgloss.NewStyle().Faint(true).Render(
			fmt.Sprintf("  Matched rule: tool=%q pattern=%q action=%q",
				m.request.RuleTool, m.request.RulePattern, m.request.RuleAction),
		)
	}

	// ── Assemble ──────────────────────────────────────────────────────────
	modalContent := lipgloss.JoinVertical(lipgloss.Top,
		titleLine,
		"",
		toolLine,
		"",
		lipgloss.NewStyle().Foreground(m.theme.TextMuted).Render("Command"),
		cmdBox,
	)
	if ruleInfo != "" {
		modalContent = lipgloss.JoinVertical(lipgloss.Top,
			modalContent,
			"",
			ruleInfo,
		)
	}
	modalContent = lipgloss.JoinVertical(lipgloss.Top,
		modalContent,
		"",
		keys,
		"",
		countdownBar,
		countdown,
	)

	modal := lipgloss.NewStyle().
		Background(m.theme.SurfaceElevated).
		Foreground(m.theme.TextPrimary).
		Padding(1, 2).
		Border(theme.DoubleBorder).
		BorderForeground(m.theme.Brand).
		Width(modalWidth).
		Render(modalContent)

	return lipgloss.Place(width, height,
		lipgloss.Center, lipgloss.Center,
		modal,
	)
}

// renderCountdownBar renders a progress bar using half-block characters (▀▄).
// Fills from left to right based on remaining time ratio.
func (m *PermissionModal) renderCountdownBar(maxWidth int) string {
	if maxWidth <= 0 {
		return ""
	}

	total := m.timeout.Seconds()
	remaining := m.Remaining().Seconds()
	if total <= 0 {
		return ""
	}

	ratio := remaining / total
	filledWidth := int(ratio * float64(maxWidth))
	if filledWidth > maxWidth {
		filledWidth = maxWidth
	}
	emptyWidth := maxWidth - filledWidth

	filled := lipgloss.NewStyle().Foreground(m.theme.Warning).Render(strings.Repeat("█", filledWidth))
	empty := lipgloss.NewStyle().Foreground(m.theme.Border).Render(strings.Repeat("░", emptyWidth))

	return filled + empty
}

func (m *PermissionModal) Allow() tools.PermissionResponse {
	return tools.PermissionResponse{Allowed: true, Remember: false}
}

func (m *PermissionModal) AllowAlways() tools.PermissionResponse {
	return tools.PermissionResponse{Allowed: true, Remember: true}
}

func (m *PermissionModal) Deny() tools.PermissionResponse {
	return tools.PermissionResponse{Allowed: false, Remember: false}
}

func (m *PermissionModal) Tick() {
	m.elapsed += time.Second
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
			Background(m.theme.Warning).
			Foreground(m.theme.BadgeForeground).
			Bold(true).
			Padding(0, 1)
	case types.RiskDestructive:
		return lipgloss.NewStyle().
			Background(m.theme.Error).
			Foreground(m.theme.BadgeForeground).
			Bold(true).
			Padding(0, 1)
	case types.RiskMedium:
		return lipgloss.NewStyle().
			Background(m.theme.Warning).
			Foreground(m.theme.BadgeForeground).
			Padding(0, 1)
	case types.RiskSafe:
		return lipgloss.NewStyle().
			Background(m.theme.TextSecondary).
			Foreground(m.theme.BadgeForeground).
			Padding(0, 1)
	default:
		return lipgloss.NewStyle().
			Background(m.theme.TextSecondary).
			Foreground(m.theme.BadgeForeground).
			Padding(0, 1)
	}
}

// riskLabel returns a human-readable risk label with icon for accessibility.
func riskLabel(level types.RiskLevel) string {
	switch level {
	case types.RiskDangerous:
		return "⚠ DANGEROUS"
	case types.RiskDestructive:
		return "✖ DESTRUCTIVE"
	case types.RiskMedium:
		return "● MEDIUM"
	case types.RiskSafe:
		return "✓ SAFE"
	default:
		return "UNKNOWN"
	}
}

func formatDurationClock(d time.Duration) string {
	totalSecs := int(d.Seconds())
	mins := totalSecs / 60
	secs := totalSecs % 60
	return fmt.Sprintf("%d:%02d", mins, secs)
}

func highlightCommand(cmd string, t theme.Theme) string {
	if cmd == "" {
		return cmd
	}
	parts := strings.Fields(cmd)
	if len(parts) == 0 {
		return cmd
	}

	var result strings.Builder
	cmdStyle := lipgloss.NewStyle().Foreground(t.Brand).Bold(true)
	argStyle := lipgloss.NewStyle().Foreground(t.TextPrimary)
	pipeStyle := lipgloss.NewStyle().Foreground(t.Warning)

	result.WriteString(cmdStyle.Render(parts[0]))
	for _, part := range parts[1:] {
		result.WriteString(" ")
		if part == "|" || part == "&&" || part == "||" || part == ">" || part == ">>" || part == "<" {
			result.WriteString(pipeStyle.Render(part))
		} else if strings.HasPrefix(part, "-") {
			result.WriteString(argStyle.Render(part))
		} else {
			result.WriteString(argStyle.Render(part))
		}
	}
	return result.String()
}
