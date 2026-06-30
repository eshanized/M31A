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
	styles  theme.SemanticStyles
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
		styles:  theme.BuildSemanticStyles(t),
		timeout: timeout,
	}
}

// Clear resets the modal to its inactive state.
func (m *PermissionModal) Clear() {
	m.elapsed = 0
}

func (m *PermissionModal) Render(width, height int) string {
	s := m.styles
	modalWidth := 60
	if width < modalWidth+4 {
		modalWidth = width - 4
	}
	if modalWidth < 20 {
		modalWidth = 20
	}

	// ── Title ─────────────────────────────────────────────────────────────
	titleLine := lipgloss.JoinHorizontal(lipgloss.Top,
		s.PermLock.Render("key"),
		" ",
		s.PermTitle.Render("Permission Required"),
	)

	// ── Tool & risk info ──────────────────────────────────────────────────
	riskBadge := m.riskStyle().Render(fmt.Sprintf(" %s ", riskLabel(m.request.RiskLevel)))

	toolLine := lipgloss.JoinHorizontal(lipgloss.Top,
		s.Caption.Render("Tool  "),
		s.Heading.Render(m.request.ToolName),
		"  ",
		s.Caption.Render("Risk  "),
		riskBadge,
	)

	// ── Command box ───────────────────────────────────────────────────────
	cmdContentW := modalWidth - 10
	if cmdContentW < 8 {
		cmdContentW = 8
	}
	highlighted := highlightCommand(m.request.Command, m.styles)
	highlighted = TruncateWithEllipsis(highlighted, cmdContentW)
	cmdBox := s.InputCode.
		Width(modalWidth - 6).
		Render(highlighted)

	// ── Keybindings ───────────────────────────────────────────────────────
	keys := lipgloss.JoinVertical(lipgloss.Left,
		lipgloss.JoinHorizontal(lipgloss.Top,
			s.PermKey.Render("[Y]"),
			s.PermHint.Render(" Allow once      "),
			s.PermKey.Render("[A]"),
			s.PermHint.Render(" Always allow"),
		),
		lipgloss.JoinHorizontal(lipgloss.Top,
			s.PermKey.Render("[B]"),
			s.PermHint.Render(" Approve all     "),
			s.PermKey.Render("[N]"),
			s.PermHint.Render(" Deny"),
		),
		lipgloss.JoinHorizontal(lipgloss.Top,
			s.PermKey.Render("[Esc]"),
			s.PermHint.Render(" Exit"),
		),
	)

	// ── Queue depth ────────────────────────────────────────────────────────
	var queueInfo string
	if m.request.QueueDepth > 0 {
		queueInfo = s.Caption.Render(fmt.Sprintf("  %d tool(s) queued behind this one", m.request.QueueDepth))
	}

	// ── Countdown ─────────────────────────────────────────────────────────
	remaining := m.Remaining()
	var countdown string
	if remaining <= 0 {
		countdown = s.PermCountdownErr.Render("Tool will be rejected")
	} else if remaining <= 30*time.Second {
		countdown = s.PermCountdownWarn.Render(fmt.Sprintf("Auto-deny in %s", formatDurationClock(remaining)))
	} else {
		countdown = s.PermCountdown.Render(fmt.Sprintf("Auto-deny in %s", formatDurationClock(remaining)))
	}

	// ── Rule context ──────────────────────────────────────────────────────
	var ruleInfo string
	if m.request.RuleTool != "" || m.request.RulePattern != "" {
		ruleInfo = s.PermRuleMatch.Render(
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
		s.Caption.Render("Command"),
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
		countdown,
	)
	if queueInfo != "" {
		modalContent = lipgloss.JoinVertical(lipgloss.Top,
			modalContent,
			queueInfo,
		)
	}

	modal := s.Dialog.
		Width(modalWidth).
		Render(modalContent)

	return lipgloss.Place(width, height,
		lipgloss.Center, lipgloss.Center,
		modal,
	)
}

func (m *PermissionModal) Allow() tools.PermissionResponse {
	return tools.PermissionResponse{Allowed: true, Remember: false}
}

func (m *PermissionModal) AllowAlways() tools.PermissionResponse {
	return tools.PermissionResponse{Allowed: true, Remember: true}
}

func (m *PermissionModal) AllowApproveAll() tools.PermissionResponse {
	return tools.PermissionResponse{Allowed: true, ApproveAll: true}
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
		return m.styles.PermRiskDanger
	case types.RiskDestructive:
		return m.styles.PermRiskDestruct
	case types.RiskMedium:
		return m.styles.PermRiskMedium
	case types.RiskSafe:
		return m.styles.PermRiskSafe
	default:
		return m.styles.PermRiskSafe
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

func highlightCommand(cmd string, s theme.SemanticStyles) string {
	if cmd == "" {
		return cmd
	}
	parts := strings.Fields(cmd)
	if len(parts) == 0 {
		return cmd
	}

	var result strings.Builder
	result.WriteString(s.BrandBold.Render(parts[0]))
	for _, part := range parts[1:] {
		result.WriteString(" ")
		if part == "|" || part == "&&" || part == "||" || part == ">" || part == ">>" || part == "<" {
			result.WriteString(s.WarningText.Render(part))
		} else {
			result.WriteString(s.Body.Render(part))
		}
	}
	return result.String()
}
