package components

import (
	"strings"

	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/tui/theme"
)

// TourStep represents a single step in the getting-started tour.
type TourStep struct {
	Title       string
	Description string
	Illustration string // optional decorative text
}

// TourModel holds the state of the getting-started tour.
type TourModel struct {
	steps     []TourStep
	current   int
	completed bool
	width     int
	height    int
	theme     theme.Theme
}

// NewTourModel creates a new tour with the standard getting-started steps.
func NewTourModel(t theme.Theme, w, h int) *TourModel {
	steps := []TourStep{
		{
			Title:       "Welcome to M31A",
			Description: "M31A is an AI-powered terminal assistant.\nIt plans and executes tasks autonomously so you can focus on building.",
			Illustration: "◆  M 3 1 A",
		},
		{
			Title:       "How it works",
			Description: "M31A follows a structured workflow:\n\n  Discuss  →  Plan  →  Execute  →  Verify  →  Ship\n\nEach step is automated. You review and approve key decisions.",
			Illustration: "",
		},
		{
			Title:       "Quick start",
			Description: "Type a task description to begin:\n\n  \"Fix the failing tests\"\n  \"Add error handling to the API\"\n  \"Explain this codebase architecture\"\n\nM31A will discuss, plan, and execute it.",
			Illustration: "",
		},
		{
			Title:       "Navigation",
			Description: "  Esc        Go back / close overlay\n  j / k      Scroll up / down\n  Enter      Confirm / submit\n  ?          Show help\n  ctrl+p     Command palette\n  ctrl+b     Toggle sidebar",
			Illustration: "",
		},
		{
			Title:       "Ready!",
			Description: "Type your first task to get started.\n\nYou can always restart this tour with:\n  /help getting-started",
			Illustration: "",
		},
	}

	return &TourModel{
		steps:  steps,
		width:  w,
		height: h,
		theme:  t,
	}
}

// SetDimensions updates the tour's available space.
func (tm *TourModel) SetDimensions(w, h int) {
	tm.width = w
	tm.height = h
}

// SetTheme updates the tour's theme.
func (tm *TourModel) SetTheme(t theme.Theme) {
	tm.theme = t
}

// Current returns the current step index (0-based).
func (tm *TourModel) Current() int {
	return tm.current
}

// IsCompleted returns whether the tour has been finished or skipped.
func (tm *TourModel) IsCompleted() bool {
	return tm.completed
}

// Total returns the total number of steps.
func (tm *TourModel) Total() int {
	return len(tm.steps)
}

// Next advances to the next step. Returns true if the tour is now complete.
func (tm *TourModel) Next() bool {
	if tm.current < len(tm.steps)-1 {
		tm.current++
		return false
	}
	tm.completed = true
	return true
}

// Skip marks the tour as completed without finishing all steps.
func (tm *TourModel) Skip() {
	tm.completed = true
}

// Render renders the current tour step as a styled string.
func (tm *TourModel) Render() string {
	t := tm.theme
	w := tm.width
	if w <= 0 {
		w = 80
	}

	s := theme.BuildSemanticStyles(t)

	step := tm.steps[tm.current]
	totalSteps := len(tm.steps)

	// Title
	title := s.BrandBold.Render(step.Title)

	// Step indicator: "Step 2 of 5"
	indicator := s.Muted.Render(
		"Step " + itoa(tm.current+1) + " of " + itoa(totalSteps),
	)

	// Illustration (if any)
	var illustration string
	if step.Illustration != "" {
		illustration = s.BrandText.Render(step.Illustration)
	}

	// Description (preserves newlines)
	description := s.Body.Render(step.Description)

	// Build content
	var parts []string
	parts = append(parts, title, "", indicator)
	if illustration != "" {
		parts = append(parts, "", illustration)
	}
	parts = append(parts, "", description)

	// Progress bar
	progress := tm.renderProgressBar(t, w)
	parts = append(parts, "", progress)

	// Hints
	hintStyle := s.Muted
	keyStyle := s.BrandBold
	hints := lipgloss.JoinHorizontal(lipgloss.Center,
		keyStyle.Render("Enter"), hintStyle.Render(" next  "),
		keyStyle.Render("Esc"), hintStyle.Render(" skip"),
	)
	parts = append(parts, "", hints)

	content := lipgloss.JoinVertical(lipgloss.Left, parts...)

	// Wrap in a bordered card
	innerW := w - 6
	if innerW < 20 {
		innerW = 20
	}
	card := lipgloss.NewStyle().
		Border(lipgloss.RoundedBorder()).
		BorderForeground(t.Brand).
		Padding(1, 2).
		Width(innerW).
		Render(content)

	return card
}

// renderProgressBar renders a visual progress indicator.
func (tm *TourModel) renderProgressBar(t theme.Theme, w int) string {
	total := len(tm.steps)
	current := tm.current + 1

	s := theme.BuildSemanticStyles(t)
	var parts []string
	for i := 0; i < total; i++ {
		if i < current {
			parts = append(parts, s.BrandBold.Render("━"))
		} else {
			parts = append(parts, s.Muted.Render("─"))
		}
	}
	return strings.Join(parts, "")
}

// itoa is a simple int-to-string converter to avoid importing strconv.
func itoa(n int) string {
	if n == 0 {
		return "0"
	}
	var buf [20]byte
	i := len(buf)
	for n > 0 {
		i--
		buf[i] = byte('0' + n%10)
		n /= 10
	}
	return string(buf[i:])
}
