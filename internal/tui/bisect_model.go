package tui

import (
	"fmt"
	"strings"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/bubbles/viewport"
	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/tui/theme"
)

// BisectModel provides an interactive git bisect interface.
type BisectModel struct {
	theme    theme.Theme
	commits  []bisectCommit
	current  int
	good     int
	bad      int
	total    int
	status   string
	viewport viewport.Model
	errMsg   string
	width    int
	height   int
}

type bisectCommit struct {
	Hash    string
	Message string
	Status  string // "pending", "good", "bad", "skip", "testing"
}

// NewBisectModel creates a BisectModel.
func NewBisectModel(t theme.Theme, w, h int) *BisectModel {
	return &BisectModel{
		theme:  t,
		status: "idle",
		width:  w,
		height: h,
	}
}

// SetCommits sets the bisect range.
func (bm *BisectModel) SetCommits(commits []bisectCommit) {
	bm.commits = commits
	bm.total = len(commits)
	bm.current = len(commits) / 2
	bm.status = "testing"
	if bm.current < len(bm.commits) {
		bm.commits[bm.current].Status = "testing"
	}
}

// SetTheme updates the theme.
func (bm *BisectModel) SetTheme(t theme.Theme) { bm.theme = t }

// SetDimensions updates dimensions.
func (bm *BisectModel) SetDimensions(w, h int) {
	bm.width = w
	bm.height = h
}

// Init implements tea.Model.
func (bm *BisectModel) Init() tea.Cmd { return nil }

// Update implements tea.Model.
func (bm *BisectModel) Update(msg tea.Msg) (tea.Model, tea.Cmd) {
	switch msg := msg.(type) {
	case tea.WindowSizeMsg:
		bm.SetDimensions(msg.Width, msg.Height)
	case tea.KeyMsg:
		switch msg.String() {
		case "esc", "q":
			return bm, func() tea.Msg { return PopScreenMsg{} }
		case "g":
			bm.markCurrent("good")
		case "b":
			bm.markCurrent("bad")
		case "s":
			bm.markCurrent("skip")
		case "r":
			bm.reset()
		}
	}
	return bm, nil
}

func (bm *BisectModel) markCurrent(status string) {
	if bm.current < len(bm.commits) {
		bm.commits[bm.current].Status = status
		bm.advance()
	}
}

func (bm *BisectModel) advance() {
	// Simple binary search: find midpoint of remaining range
	low := 0
	high := len(bm.commits) - 1
	for i, c := range bm.commits {
		if c.Status == "good" && i > low {
			low = i
		}
		if c.Status == "bad" && i < high {
			high = i
		}
	}
	bm.current = low + (high-low)/2
	if bm.current < len(bm.commits) && bm.commits[bm.current].Status == "pending" {
		bm.commits[bm.current].Status = "testing"
	}
	if low >= high-1 {
		bm.status = "done"
	}
}

func (bm *BisectModel) reset() {
	for i := range bm.commits {
		bm.commits[i].Status = "pending"
	}
	bm.current = len(bm.commits) / 2
	bm.status = "testing"
	if bm.current < len(bm.commits) {
		bm.commits[bm.current].Status = "testing"
	}
}

// View implements tea.Model.
func (bm *BisectModel) View() string {
	t := bm.theme
	w := bm.width
	if w < 30 {
		w = 80
	}

	title := lipgloss.NewStyle().Foreground(t.Brand).Bold(true).PaddingLeft(2).
		Render("Git Bisect")

	var lines []string
	lines = append(lines, "", title, "")

	if bm.errMsg != "" {
		lines = append(lines, lipgloss.NewStyle().Foreground(t.Error).PaddingLeft(2).
			Render("! "+bm.errMsg))
	}

	if len(bm.commits) == 0 {
		lines = append(lines, lipgloss.NewStyle().Foreground(t.TextMuted).PaddingLeft(2).
			Render("No bisect range set. Use /bisect start <good> <bad> to begin."))
	} else {
		info := fmt.Sprintf("  Range: %d commits   Current: %d/%d   Status: %s",
			bm.total, bm.current+1, bm.total, bm.status)
		lines = append(lines, lipgloss.NewStyle().Foreground(t.TextSecondary).Render(info), "")

		start := 0
		if bm.current > 5 {
			start = bm.current - 5
		}
		end := start + 12
		if end > len(bm.commits) {
			end = len(bm.commits)
		}

		for i := start; i < end; i++ {
			c := bm.commits[i]
			selected := i == bm.current
			var icon string
			var color lipgloss.Color
			switch c.Status {
			case "good":
				icon = "✓"
				color = t.Success
			case "bad":
				icon = "✗"
				color = t.Error
			case "skip":
				icon = "⊘"
				color = t.TextMuted
			case "testing":
				icon = "◐"
				color = t.Brand
			default:
				icon = "○"
				color = t.TextMuted
			}
			iconStyled := lipgloss.NewStyle().Foreground(color).Render(icon)
			hashStyled := lipgloss.NewStyle().Foreground(t.TextMuted).Render(c.Hash[:7])
			msgStyled := lipgloss.NewStyle().Foreground(t.Text).Render(
				TruncateWithEllipsis(c.Message, w-30))
			prefix := "    "
			if selected {
				prefix = lipgloss.NewStyle().Foreground(t.Brand).Render("  ▶ ")
			}
			lines = append(lines, fmt.Sprintf("%s%s  %s  %s", prefix, iconStyled, hashStyled, msgStyled))
		}
	}

	footer := lipgloss.NewStyle().Foreground(t.TextMuted).PaddingLeft(2).
		Render("[g] Good   [b] Bad   [s] Skip   [r] Reset   [esc] Back")
	lines = append(lines, "", footer)

	return strings.Join(lines, "\n")
}
