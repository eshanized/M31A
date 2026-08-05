package components

import (
	"fmt"
	"strings"
	"time"

	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/core/types"
	"github.com/eshanized/M31A/internal/tools"
	"github.com/eshanized/M31A/internal/ui/tui/theme"
)

// DefaultPermissionTimeout is the default permission modal timeout (10 minutes).
// The permission modal waits for user decision without auto-deny.
var DefaultPermissionTimeout = 10 * time.Minute

type PermissionModal struct {
	request tools.PermissionRequest
	theme   theme.Theme
	styles  theme.SemanticStyles
	elapsed time.Duration
	timeout time.Duration
	// phase is the current workflow phase (e.g. "execute", "plan")
	phase string
	// goal is the current workflow goal
	goal string
	// batchAvailable indicates batch approval is available for this request
	batchAvailable bool
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

// SetContext sets the workflow phase and goal for richer descriptions.
func (m *PermissionModal) SetContext(phase, goal string) {
	m.phase = phase
	m.goal = goal
}

// SetBatchAvailable indicates that batch approval is available for this request.
func (m *PermissionModal) SetBatchAvailable(available bool) {
	m.batchAvailable = available
}

// Clear resets the modal to its inactive state.
func (m *PermissionModal) Clear() {
	m.elapsed = 0
}

func (m *PermissionModal) Render(width, height int) string {
	s := m.styles
	modalWidth := 60
	// Use full command text width if wider than minimum
	cmdWidth := len(m.request.Command) + 10
	if cmdWidth > modalWidth {
		modalWidth = cmdWidth
	}
	// Cap at maximum 80 columns
	if modalWidth > 80 {
		modalWidth = 80
	}
	// Ensure it fits in terminal
	if width < modalWidth+4 {
		modalWidth = width - 4
	}
	if modalWidth < 20 {
		modalWidth = 20
	}

	// Generate plain English description
	desc := GenerateDescription(m.request, m.phase, m.goal)

	// ── Title: "M31A wants to [action]" ──────────────────────────────────
	actionText := desc.Action
	if actionText == "" {
		actionText = "use a tool"
	}
	titleLine := s.PermTitle.Render(fmt.Sprintf("M31A wants to %s", actionText))

	// ── Risk text label (D-10) ──────────────────────────────────────────
	riskBadge := riskTextLabel(m.request.RiskLevel, s)

	// ── Consequence explanation ───────────────────────────────────────────
	var consequenceLine string
	if desc.Consequence != "" {
		consequenceLine = s.Body.Render(desc.Consequence)
	}

	// ── Target (if different from action) ─────────────────────────────────
	var targetLine string
	if desc.Target != "" {
		targetLine = s.Caption.Render("Target: ") + s.Body.Render(desc.Target)
	}

	// ── Risk via border color accent (not text label) ────────────────────
	// Risk is communicated via the border style, not via a text badge

	// ── Command box (secondary, below divider) ───────────────────────────
	cmdContentW := modalWidth - 6
	if cmdContentW < 8 {
		cmdContentW = 8
	}
	highlighted := highlightCommand(m.request.Command, m.styles)
	// Show full command when possible, only truncate if extremely long
	if len(m.request.Command) > cmdContentW {
		highlighted = TruncateWithEllipsis(highlighted, cmdContentW)
	}
	cmdBox := s.InputCode.
		Width(modalWidth - 6).
		Render(highlighted)

	// ── Keybindings ───────────────────────────────────────────────────────
	keys := lipgloss.JoinVertical(lipgloss.Left,
		lipgloss.JoinHorizontal(lipgloss.Top,
			s.PermKey.Render("[Y]"),
			s.PermHint.Render(" Allow once        "),
			s.PermKey.Render("[A]"),
			s.PermHint.Render(" Allow for session"),
		),
		lipgloss.JoinHorizontal(lipgloss.Top,
			s.PermKey.Render("[N]"),
			s.PermHint.Render(" Deny              "),
			s.PermKey.Render("[Esc]"),
			s.PermHint.Render(" Deny (safe default)"),
		),
	)

	// ── Batch approval keybinding (D-11) ─────────────────────────────────
	if m.batchAvailable && m.request.QueueDepth > 0 {
		batchLine := lipgloss.JoinHorizontal(lipgloss.Top,
			s.PermKey.Render("[B]"),
			s.PermHint.Render(fmt.Sprintf(" Approve all %d    ", m.request.QueueDepth+1)),
		)
		keys = lipgloss.JoinVertical(lipgloss.Top,
			lipgloss.JoinVertical(lipgloss.Top,
				lipgloss.JoinHorizontal(lipgloss.Top,
					s.PermKey.Render("[Y]"),
					s.PermHint.Render(" Allow once        "),
					s.PermKey.Render("[A]"),
					s.PermHint.Render(" Allow for session"),
				),
				batchLine,
			),
			lipgloss.JoinHorizontal(lipgloss.Top,
				s.PermKey.Render("[N]"),
				s.PermHint.Render(" Deny              "),
				s.PermKey.Render("[Esc]"),
				s.PermHint.Render(" Deny (safe default)"),
			),
		)
	}

	// ── Queue depth ────────────────────────────────────────────────────────
	var queueInfo string
	if m.request.QueueDepth > 0 {
		queueInfo = s.Caption.Render(fmt.Sprintf("  %d tool(s) queued behind this one", m.request.QueueDepth))
	}

	// ── Timeout info (non-urgent, just informational) ─────────────────────
	remaining := m.Remaining()
	var timeoutInfo string
	if remaining > 0 {
		timeoutInfo = s.Caption.Render(fmt.Sprintf("  Timeout in %s", formatDurationClock(remaining)))
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
	titleWithRisk := titleLine
	if riskBadge != "" {
		titleWithRisk = lipgloss.JoinHorizontal(lipgloss.Top,
			titleLine,
			"  ",
			riskBadge,
		)
	}
	modalContent := lipgloss.JoinVertical(lipgloss.Top,
		titleWithRisk,
	)
	if consequenceLine != "" {
		modalContent = lipgloss.JoinVertical(lipgloss.Top,
			modalContent,
			consequenceLine,
		)
	}
	if targetLine != "" {
		modalContent = lipgloss.JoinVertical(lipgloss.Top,
			modalContent,
			targetLine,
		)
	}
	modalContent = lipgloss.JoinVertical(lipgloss.Top,
		modalContent,
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
	)
	if timeoutInfo != "" {
		modalContent = lipgloss.JoinVertical(lipgloss.Top,
			modalContent,
			"",
			timeoutInfo,
		)
	}
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

// HighlightCommand is the exported version of highlightCommand that takes a theme.
func HighlightCommand(cmd string, t theme.Theme) string {
	s := theme.BuildSemanticStyles(t)
	return highlightCommand(cmd, s)
}

// riskTextLabel returns a styled risk badge for the given risk level.
func riskTextLabel(risk types.RiskLevel, s theme.SemanticStyles) string {
	switch risk {
	case types.RiskSafe:
		return s.PermRiskSafe.Render("SAFE")
	case types.RiskMedium:
		return s.PermRiskMedium.Render("CAUTION")
	case types.RiskDestructive:
		return s.PermRiskDestruct.Render("DESTRUCTIVE")
	case types.RiskDangerous:
		return s.PermRiskDanger.Render("DANGER")
	default:
		return ""
	}
}
