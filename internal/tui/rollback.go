package tui

import (
	"fmt"
	"strings"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/bubbles/viewport"
	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/git"
	"github.com/eshanized/M31A/internal/tui/theme"
	"github.com/eshanized/M31A/pkg/rollback"
)

// RollbackModel shows the git commit timeline and allows resetting to any commit.
type RollbackModel struct {
	theme    theme.Theme
	git      *git.Git
	rollback *rollback.Rollback
	entries  []rollback.RollbackEntry
	cursor   int
	offset   int
	viewport viewport.Model // shows diff for selected commit
	showDiff bool
	errMsg   string
	width    int
	height   int
}

// NewRollbackModel creates a RollbackModel.
func NewRollbackModel(t theme.Theme, g *git.Git, rb *rollback.Rollback, w, h int) *RollbackModel {
	m := &RollbackModel{
		theme:    t,
		git:      g,
		rollback: rb,
		width:    w,
		height:   h,
	}
	return m
}

// SetTheme updates the theme.
func (rm *RollbackModel) SetTheme(t theme.Theme) {
	rm.theme = t
}

// SetDimensions updates the rollback model dimensions.
func (rm *RollbackModel) SetDimensions(w, h int) {
	rm.width = w
	rm.height = h
}

// LoadCommits fetches the commit chain from the rollback package.
func (rm *RollbackModel) LoadCommits() {
	if rm.rollback == nil {
		return
	}
	entries, err := rm.rollback.Chain(30)
	if err != nil {
		rm.errMsg = err.Error()
		return
	}
	rm.entries = entries
	rm.errMsg = ""
	rm.cursor = 0
	rm.offset = 0
}

// Init implements tea.Model.
func (rm *RollbackModel) Init() tea.Cmd { return nil }

// Update implements tea.Model.
func (rm *RollbackModel) Update(msg tea.Msg) (tea.Model, tea.Cmd) {
	switch msg := msg.(type) {
	case tea.WindowSizeMsg:
		rm.SetDimensions(msg.Width, msg.Height)
		return rm, nil
	case tea.KeyMsg:
		switch msg.String() {
		case "esc", "q":
			if rm.showDiff {
				rm.showDiff = false
				return rm, nil
			}
			return rm, func() tea.Msg {
				return AppMsg{Screen: ScreenREPL}
			}
		case "up", "k":
			if rm.cursor > 0 {
				rm.cursor--
				rm.clampScroll()
			}
		case "down", "j":
			if rm.cursor < len(rm.entries)-1 {
				rm.cursor++
				rm.clampScroll()
			}
		case "enter", " ":
			// Show diff for current entry
			if len(rm.entries) > 0 {
				e := rm.entries[rm.cursor]
				vpH := rm.height - 8
				if vpH < 3 {
					vpH = 3
				}
				rm.viewport = viewport.New(rm.width, vpH)
				rm.viewport.SetContent(rm.renderDiffContent(e))
				rm.showDiff = true
			}
		case "r":
			// Soft reset to current entry
			if len(rm.entries) > 0 && rm.rollback != nil {
				e := rm.entries[rm.cursor]
				_, err := rm.rollback.SoftReset(e.CommitInfo.Hash, nil)
				if err != nil {
					rm.errMsg = err.Error()
				} else {
					rm.errMsg = ""
					rm.LoadCommits()
				}
			}
		case "R":
			// Hard reset to current entry
			if len(rm.entries) > 0 && rm.rollback != nil {
				e := rm.entries[rm.cursor]
				_, err := rm.rollback.HardReset(e.CommitInfo.Hash)
				if err != nil {
					rm.errMsg = err.Error()
				} else {
					rm.errMsg = ""
					rm.LoadCommits()
				}
			}
		}
		if rm.showDiff {
			var cmd tea.Cmd
			rm.viewport, cmd = rm.viewport.Update(msg)
			return rm, cmd
		}
	}
	return rm, nil
}

// View implements tea.Model.
func (rm *RollbackModel) View() string {
	t := rm.theme
	w := rm.width
	if w < 30 {
		w = 80
	}

	title := lipgloss.NewStyle().Foreground(t.Brand).Bold(true).PaddingLeft(1).
		Render("  Rollback — Commit Time Machine")
	divider := lipgloss.NewStyle().Foreground(t.Border).Render(strings.Repeat("─", w))

	if rm.errMsg != "" {
		errLine := lipgloss.NewStyle().Foreground(t.Error).PaddingLeft(2).
			Render("! " + rm.errMsg)
		return lipgloss.JoinVertical(lipgloss.Left, title, divider, errLine)
	}

	if len(rm.entries) == 0 {
		empty := lipgloss.NewStyle().Foreground(t.TextMuted).PaddingLeft(2).
			Render("No commits found in this repository.")
		footer := lipgloss.NewStyle().Foreground(t.TextMuted).PaddingLeft(2).
			Render("esc back")
		return lipgloss.JoinVertical(lipgloss.Left, title, divider, "", empty, "", divider, footer)
	}

	if rm.showDiff {
		return rm.renderDiffView(title, divider)
	}
	return rm.renderCommitList(title, divider, w)
}

// renderCommitList renders the list of commits.
func (rm *RollbackModel) renderCommitList(title, divider string, w int) string {
	t := rm.theme
	listH := rm.listVisibleRows()
	end := rm.offset + listH
	if end > len(rm.entries) {
		end = len(rm.entries)
	}
	visible := rm.entries[rm.offset:end]

	var rows []string
	for i, e := range visible {
		globalIdx := rm.offset + i
		selected := globalIdx == rm.cursor
		rows = append(rows, rm.renderCommitRow(e, selected, w))
	}

	scrollInfo := lipgloss.NewStyle().Foreground(t.TextMuted).PaddingLeft(2).
		Render(fmt.Sprintf("%d/%d", rm.cursor+1, len(rm.entries)))

	footer := lipgloss.NewStyle().Foreground(t.TextMuted).PaddingLeft(2).
		Render("↵ diff  r soft-reset  R hard-reset  ↑↓ navigate  esc back")

	return lipgloss.JoinVertical(lipgloss.Left,
		title, divider,
		strings.Join(rows, "\n"),
		"", scrollInfo, divider, footer)
}

// renderCommitRow renders a single commit entry.
func (rm *RollbackModel) renderCommitRow(e rollback.RollbackEntry, selected bool, w int) string {
	t := rm.theme
	c := e.CommitInfo

	prefix := "  "
	hashStyle := lipgloss.NewStyle().Foreground(t.TextMuted)
	msgStyle := lipgloss.NewStyle().Foreground(t.Text)
	if selected {
		prefix = lipgloss.NewStyle().Foreground(t.Brand).Render("▶ ")
		hashStyle = hashStyle.Foreground(t.Brand).Bold(true)
		msgStyle = msgStyle.Bold(true)
	}
	if e.IsCurrent {
		hashStyle = hashStyle.Foreground(t.Success)
	}

	ts := c.Timestamp.Format("2006-01-02 15:04")
	msg := TruncateWithEllipsis(c.Message, w-30)

	return prefix +
		hashStyle.Render(c.ShortHash) + "  " +
		lipgloss.NewStyle().Foreground(t.TextMuted).Render(ts) + "  " +
		msgStyle.Render(msg)
}

// renderDiffView renders the diff viewport for the selected commit.
func (rm *RollbackModel) renderDiffView(title, divider string) string {
	t := rm.theme
	e := rm.entries[rm.cursor]
	subTitle := lipgloss.NewStyle().Foreground(t.TextMuted).PaddingLeft(2).
		Render(fmt.Sprintf("Diff for %s — %s", e.CommitInfo.ShortHash, TruncateWithEllipsis(e.CommitInfo.Message, 50)))
	footer := lipgloss.NewStyle().Foreground(t.TextMuted).PaddingLeft(2).
		Render("j/k scroll  esc close diff")
	return lipgloss.JoinVertical(lipgloss.Left,
		title, divider, subTitle, rm.viewport.View(), divider, footer)
}

// renderDiffContent produces the colored diff string for a commit.
func (rm *RollbackModel) renderDiffContent(e rollback.RollbackEntry) string {
	if e.Diff == "" {
		return lipgloss.NewStyle().Foreground(rm.theme.TextMuted).
			Render("  (current HEAD — no diff)")
	}
	return colorizeDiff(e.Diff, rm.theme)
}

// colorizeDiff applies +/- line colors to a diff string.
func colorizeDiff(diff string, t theme.Theme) string {
	lines := strings.Split(diff, "\n")
	var out []string
	for _, line := range lines {
		switch {
		case strings.HasPrefix(line, "+") && !strings.HasPrefix(line, "+++"):
			out = append(out, lipgloss.NewStyle().Foreground(t.DiffAdded).Render(line))
		case strings.HasPrefix(line, "-") && !strings.HasPrefix(line, "---"):
			out = append(out, lipgloss.NewStyle().Foreground(t.DiffRemoved).Render(line))
		case strings.HasPrefix(line, "@@"):
			out = append(out, lipgloss.NewStyle().Foreground(t.Brand).Render(line))
		default:
			out = append(out, lipgloss.NewStyle().Foreground(t.Text).Render(line))
		}
	}
	return strings.Join(out, "\n")
}

// clampScroll ensures the cursor is visible in the list.
func (rm *RollbackModel) clampScroll() {
	lh := rm.listVisibleRows()
	if rm.cursor < rm.offset {
		rm.offset = rm.cursor
	}
	if rm.cursor >= rm.offset+lh {
		rm.offset = rm.cursor - lh + 1
	}
}

// listVisibleRows returns how many commit rows fit in the terminal.
func (rm *RollbackModel) listVisibleRows() int {
	h := rm.height - 6
	if h < 3 {
		return 3
	}
	return h
}
