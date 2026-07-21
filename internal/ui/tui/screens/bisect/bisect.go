package bisect

import (
	"fmt"
	"github.com/eshanized/M31A/internal/ui/tui/tuitypes"
	"strings"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/ui/tui/components"
	"github.com/eshanized/M31A/internal/ui/tui/theme"
)

// BisectModel provides an interactive git bisect interface.
type BisectModel struct {
	theme   theme.Theme
	commits []bisectCommit
	current int
	total   int
	status  string
	errMsg  string
	width   int
	height  int
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
	// Mark the initial midpoint as "testing" for visual feedback
	if bm.current < len(bm.commits) && bm.commits[bm.current].Status == "pending" {
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

// Update implements tuitypes.Screenable.
func (bm *BisectModel) Update(msg tea.Msg) (tea.Model, tea.Cmd) {
	switch msg := msg.(type) {
	case tea.WindowSizeMsg:
		bm.SetDimensions(msg.Width, msg.Height)
	case tea.KeyMsg:
		switch msg.String() {
		case "esc", "q":
			return bm, func() tea.Msg { return tuitypes.PopScreenMsg{} }
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
	if len(bm.commits) == 0 {
		bm.status = "done"
		return
	}

	// Find the range of untested commits between last known good and first known bad.
	// low = index of last "good" commit (or -1 if none)
	// high = index of first "bad" commit (or len if none)
	low := -1
	high := len(bm.commits)
	for i, c := range bm.commits {
		switch c.Status {
		case "good":
			low = i
		case "bad":
			if high == len(bm.commits) || i < high {
				high = i
			}
		}
	}

	// All commits tested or range is converged
	if low >= high-1 {
		bm.status = "done"
		return
	}

	// Compute midpoint
	mid := low + (high-low)/2

	// isUntested returns true for commits that haven't been judged (good/bad/skip)
	isUntested := func(c bisectCommit) bool {
		return c.Status != "good" && c.Status != "bad" && c.Status != "skip"
	}

	// Find the next untested commit near the midpoint
	found := -1
	// Search right from mid
	for i := mid; i < high; i++ {
		if isUntested(bm.commits[i]) {
			found = i
			break
		}
	}
	// Search left from mid if not found
	if found < 0 {
		for i := mid - 1; i > low; i-- {
			if isUntested(bm.commits[i]) {
				found = i
				break
			}
		}
	}

	if found < 0 {
		// No untested commits in range — all tested, we're done
		bm.status = "done"
		return
	}

	bm.current = found
	bm.commits[found].Status = "testing"
}

func (bm *BisectModel) reset() {
	for i := range bm.commits {
		bm.commits[i].Status = "pending"
	}
	bm.current = len(bm.commits) / 2
	bm.status = "testing"
	// Mark the initial midpoint as "testing" for visual feedback
	if bm.current < len(bm.commits) && bm.commits[bm.current].Status == "pending" {
		bm.commits[bm.current].Status = "testing"
	}
}

// View implements tea.Model.
func (bm *BisectModel) View() string {
	t := bm.theme
	w := bm.width
	if w <= 0 {
		w = 80 // only when uninitialized
	}

	title := components.ScreenTitle{Text: "Git Bisect", Theme: t}.Render()

	var lines []string
	lines = append(lines, "", title, "")

	if bm.errMsg != "" {
		lines = append(lines, components.ErrorBanner{Message: bm.errMsg, Theme: t}.Render())
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
			hash := c.Hash
			if len(hash) > 7 {
				hash = hash[:7]
			}
			hashStyled := lipgloss.NewStyle().Foreground(t.TextMuted).Render(hash)
			msgStyled := lipgloss.NewStyle().Foreground(t.Text).Render(
				tuitypes.TruncateWithEllipsis(c.Message, w-30))
			prefix := components.CursorIndicator{Selected: selected, Theme: t}.Render()
			lines = append(lines, fmt.Sprintf("%s%s  %s  %s", prefix, iconStyled, hashStyled, msgStyled))
		}
	}

	footer := components.HintBar{
		Hints: []string{"g Good", "b Bad", "s Skip", "r Reset", "esc Back"},
		Theme: t,
	}.Render()
	lines = append(lines, "", footer)

	return strings.Join(lines, "\n")
}
