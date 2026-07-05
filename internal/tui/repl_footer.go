package tui

import (
	"fmt"
	"strings"
	"time"

	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/tui/components"
	"github.com/eshanized/M31A/internal/tui/theme"
)

// StatusBarInfo carries optional info to render in the status bar.
type StatusBarInfo struct {
	PromptTokens       int
	TotalTokens        int
	Cost               float64
	ShowCost           bool
	WhichKey           string
	LeaderActive       bool
	AgentName          string
	ModelName          string
	ProviderName       string
	IsStreaming        bool
	IsThinking         bool
	ThinkingDuration   int64 // milliseconds of current thinking session
	KeyboardHints      []string
	WorkflowPhase      string
	WorkflowPhaseIndex int    // numeric phase index (0-based)
	TotalPhases        int    // total number of workflow phases
	QuestionProgress   string
	CwdName            string // basename of working directory
	GitBranch          string // current git branch
	SpinnerFrame       string // animated spinner frame (empty = use static char)
	ContextUsed        int    // tokens used in context window
	ContextMax         int    // model's max context length (0 = unknown)
	BatchApprovalTools string // comma-separated tool names with active batch approvals
}

// RenderStatusBar renders the status bar line at the bottom of the terminal.
// Uses a clean 3-zone layout:
//
//	left (cwd + branch) · center (operation) · right (hints + cost)
//
// No background fill — inherits terminal background.
func RenderStatusBar(s theme.SemanticStyles, width int, info *StatusBarInfo) string {
	if width < 10 {
		return ""
	}

	if info == nil {
		info = &StatusBarInfo{}
	}

	// Clone info to avoid mutating the original (narrow terminal adaptations)
	cloned := *info
	info = &cloned

	// ── Compact mode ──────────────────────────────────────────────────────────
	// Narrow terminal (< 80 cols): hide hints, hide cost, shorten cwd.
	if width < 80 {
		info.KeyboardHints = nil
		info.ShowCost = false
		if info.CwdName != "" {
			parts := strings.Split(info.CwdName, "/")
			info.CwdName = parts[len(parts)-1]
		}
	}

	// Ultra-narrow (< 60 cols): hide git branch.
	if width < 60 {
		info.GitBranch = ""
	}

	// ── Left zone: cwd + branch ───────────────────────────────────────────────
	var leftParts []string
	if info.CwdName != "" {
		leftParts = append(leftParts, s.FooterCwd.Render("⌂ "+info.CwdName))
	}
	if info.GitBranch != "" {
		leftParts = append(leftParts, s.FooterBranch.Render("⎇ "+info.GitBranch))
	}
	leftText := strings.Join(leftParts, "  ")

	// ── Center zone: operation status ─────────────────────────────────────────
	var centerText string
	spinnerChar := info.SpinnerFrame
	if spinnerChar == "" {
		spinnerChar = "⋯"
	}
	switch {
	case info.LeaderActive:
		centerText = s.FooterLeader.Render("ctrl+x") +
			s.FooterOp.Render(" ─ waiting ─")
	case info.IsThinking:
		thinkingLabel := "Thinking…"
		if info.ThinkingDuration > 0 {
			// Use intent-based label instead of timer
			elapsed := time.Duration(info.ThinkingDuration) * time.Millisecond
			thinkingLabel = components.ThinkingLabel(info.WorkflowPhase, "", elapsed)
		}
		centerText = s.SpinnerBrand.Render(spinnerChar) + " " +
			s.Thinking.Render(thinkingLabel)
	case info.IsStreaming:
		centerText = s.SpinnerBrand.Render(spinnerChar) + " " +
			s.FooterOp.Render("streaming…")
	case info.WorkflowPhase != "":
		phaseText := "▸ " + info.WorkflowPhase
		if info.QuestionProgress != "" {
			phaseText += " · " + info.QuestionProgress
		}
		// Add phase progress indicator when phase index is available
		if info.TotalPhases > 0 {
			progressBar := components.RenderWorkflowProgressBar(info.WorkflowPhaseIndex+1, info.TotalPhases, 8)
			centerText = s.BrandText.Render(phaseText) + " " + s.ProgressLabel.Render(progressBar)
		} else {
			centerText = s.BrandText.Render(phaseText)
		}
	case info.WhichKey != "":
		centerText = s.FooterOp.Render(info.WhichKey)
	}

	// ── Batch approval badge ────────────────────────────────────────────────
	if info.BatchApprovalTools != "" {
		batchBadge := s.WarningText.Render(fmt.Sprintf("Batch: %s", info.BatchApprovalTools))
		if centerText != "" {
			centerText += "  " + batchBadge
		} else {
			centerText = batchBadge
		}
	}

	// ── Right zone: hints only (no cost, no context ring) ───────────────────
	var rightParts []string
	for _, hint := range info.KeyboardHints {
		rightParts = append(rightParts, s.FooterHint.Render(hint))
	}
	rightText := strings.Join(rightParts, "  ")

	// ── Assemble with separators ─────────────────────────────────────────────
	// Only show separators between non-empty zones.
	var displayParts []string
	if leftText != "" {
		displayParts = append(displayParts, leftText)
	}
	if centerText != "" {
		displayParts = append(displayParts, centerText)
	}
	if rightText != "" {
		displayParts = append(displayParts, rightText)
	}

	result := strings.Join(displayParts, " · ")
	resultWidth := lipgloss.Width(result)

	// ── Overflow: drop right zone first, then center, then truncate left ─────
	if resultWidth > width && rightText != "" {
		displayParts = nil
		if leftText != "" {
			displayParts = append(displayParts, leftText)
		}
		if centerText != "" {
			displayParts = append(displayParts, centerText)
		}
		result = strings.Join(displayParts, " · ")
		resultWidth = lipgloss.Width(result)
	}

	if resultWidth > width && centerText != "" {
		displayParts = nil
		if leftText != "" {
			displayParts = append(displayParts, leftText)
		}
		result = strings.Join(displayParts, " · ")
		resultWidth = lipgloss.Width(result)
	}

	if resultWidth > width && leftText != "" {
		maxLeft := width - 1
		if maxLeft < 1 {
			maxLeft = 1
		}
		result = TruncateWithEllipsis(leftText, maxLeft)
		resultWidth = lipgloss.Width(result)
	}

	padding := width - resultWidth
	if padding < 0 {
		padding = 0
	}

	return result + strings.Repeat(" ", padding)
}
