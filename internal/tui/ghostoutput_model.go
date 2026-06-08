package tui

import (
	"fmt"
	"strings"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/tui/theme"
	"github.com/eshanized/M31A/internal/tui/tuitypes"
)

// GhostOutputModel shows the results of a ghost write operation.
type GhostOutputModel struct {
	theme  theme.Theme
	result *tuitypes.GhostResult
	cursor int
	scroll int
	width  int
	height int
}

// NewGhostOutputModel creates a GhostOutputModel.
func NewGhostOutputModel(t theme.Theme, w, h int) *GhostOutputModel {
	return &GhostOutputModel{
		theme:  t,
		width:  w,
		height: h,
	}
}

// SetResult sets the ghost write result to display.
func (go_ *GhostOutputModel) SetResult(r *tuitypes.GhostResult) {
	go_.result = r
	go_.cursor = 0
	go_.scroll = 0
}

// SetTheme updates the theme.
func (go_ *GhostOutputModel) SetTheme(t theme.Theme) { go_.theme = t }

// SetDimensions updates dimensions.
func (go_ *GhostOutputModel) SetDimensions(w, h int) {
	go_.width = w
	go_.height = h
}

// Init implements tea.Model.
func (go_ *GhostOutputModel) Init() tea.Cmd { return nil }

// Update implements tea.Model.
func (go_ *GhostOutputModel) Update(msg tea.Msg) (tea.Model, tea.Cmd) {
	switch msg := msg.(type) {
	case tea.WindowSizeMsg:
		go_.SetDimensions(msg.Width, msg.Height)
	case tea.KeyMsg:
		switch msg.String() {
		case "esc", "q":
			return go_, func() tea.Msg { return PopScreenMsg{} }
		case "up", "k":
			if go_.cursor > 0 {
				go_.cursor--
			}
		case "down", "j":
			if go_.result != nil && go_.cursor < len(go_.result.Files)-1 {
				go_.cursor++
			}
		}
	}
	return go_, nil
}

// View implements tea.Model.
func (go_ *GhostOutputModel) View() string {
	t := go_.theme
	w := go_.width
	if w < 30 {
		w = 80
	}

	title := lipgloss.NewStyle().Foreground(t.Brand).Bold(true).PaddingLeft(2).
		Render("Ghost Write Output")

	var lines []string
	lines = append(lines, "", title, "")

	if go_.result == nil {
		lines = append(lines, lipgloss.NewStyle().Foreground(t.TextMuted).PaddingLeft(2).
			Render("No ghost write results yet."))
		return strings.Join(lines, "\n")
	}

	// Files created
	if len(go_.result.Files) > 0 {
		header := lipgloss.NewStyle().Foreground(t.TextSecondary).Bold(true).PaddingLeft(2).
			Render(fmt.Sprintf("Files Created (%d):", len(go_.result.Files)))
		lines = append(lines, header, "")

		visible := go_.height - 10
		if visible < 1 {
			visible = 1
		}
		start := go_.scroll
		end := start + visible
		if end > len(go_.result.Files) {
			end = len(go_.result.Files)
		}

		for i := start; i < end; i++ {
			f := go_.result.Files[i]
			selected := i == go_.cursor

			prefix := "    "
			if selected {
				prefix = lipgloss.NewStyle().Foreground(t.Brand).Render("  ▶ ")
			}

			icon := lipgloss.NewStyle().Foreground(t.Success).Render("✓")
			path := lipgloss.NewStyle().Foreground(t.Text).Render(f.Path)

			lines = append(lines, fmt.Sprintf("%s%s  %s", prefix, icon, path))

			// Show content preview for selected file
			if selected && f.Content != "" {
				preview := f.Content
				if len(preview) > 120 {
					preview = preview[:117] + "..."
				}
				previewLines := strings.Split(preview, "\n")
				if len(previewLines) > 5 {
					previewLines = previewLines[:5]
					previewLines = append(previewLines, "  ...")
				}
				for _, pl := range previewLines {
					lines = append(lines, lipgloss.NewStyle().Foreground(t.TextMuted).PaddingLeft(6).Render(pl))
				}
			}
		}

		if len(go_.result.Files) > visible {
			lines = append(lines, lipgloss.NewStyle().Foreground(t.TextMuted).PaddingLeft(4).
				Render(fmt.Sprintf("  ... %d more files", len(go_.result.Files)-visible)))
		}
	}

	// Warnings
	if len(go_.result.Warnings) > 0 {
		lines = append(lines, "")
		warnHeader := lipgloss.NewStyle().Foreground(t.Warning).Bold(true).PaddingLeft(2).
			Render(fmt.Sprintf("Warnings (%d):", len(go_.result.Warnings)))
		lines = append(lines, warnHeader)
		for _, w := range go_.result.Warnings {
			lines = append(lines, lipgloss.NewStyle().Foreground(t.Warning).PaddingLeft(4).Render("⚠ "+w))
		}
	}

	footer := lipgloss.NewStyle().Foreground(t.TextMuted).PaddingLeft(2).
		Render("[j/k] Navigate   [esc] Back")
	lines = append(lines, "", footer)

	return strings.Join(lines, "\n")
}
