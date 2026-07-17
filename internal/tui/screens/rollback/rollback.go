package rollback

import (
	"fmt"
	"strings"

	"github.com/charmbracelet/bubbles/viewport"
	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/git"
	"github.com/eshanized/M31A/internal/rollback"
	"github.com/eshanized/M31A/internal/tui/theme"
	"github.com/eshanized/M31A/internal/tui/tuitypes"
)

// RollbackModel shows the git commit timeline and allows resetting to any commit.
type RollbackModel struct {
	theme        theme.Theme
	git          *git.Git
	rollback     *rollback.Rollback
	entries      []rollback.RollbackEntry
	cursor       int
	offset       int
	viewport     viewport.Model // shows diff for selected commit
	showDiff     bool
	errMsg       string
	confirmReset string // "soft" or "hard" when awaiting confirmation
	width        int
	height       int
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

// Update implements tuitypes.Screenable.
func (rm *RollbackModel) Update(msg tea.Msg) (tea.Model, tea.Cmd) {
	switch msg := msg.(type) {
	case tea.WindowSizeMsg:
		rm.SetDimensions(msg.Width, msg.Height)
		return rm, nil
	case tea.KeyMsg:
		switch msg.String() {
		case "esc", "q":
			if rm.confirmReset != "" {
				rm.confirmReset = ""
				return rm, nil
			}
			if rm.showDiff {
				rm.showDiff = false
				return rm, nil
			}
			return rm, func() tea.Msg {
				return tuitypes.PopScreenMsg{}
			}
		case "up", "k":
			rm.confirmReset = ""
			if rm.cursor > 0 {
				rm.cursor--
				rm.clampScroll()
			}
		case "down", "j":
			rm.confirmReset = ""
			if rm.cursor < len(rm.entries)-1 {
				rm.cursor++
				rm.clampScroll()
			}
		case "enter", " ":
			rm.confirmReset = ""
			if len(rm.entries) > 0 {
				e := rm.entries[rm.cursor]
				// diffChromeHeight accounts for: title + filepath + stats + divider + hints + margins
				const rollbackDiffChromeH = 8
				vpH := rm.height - rollbackDiffChromeH
				if vpH < 3 {
					vpH = 3
				}
				rm.viewport = viewport.New(rm.width, vpH)
				rm.viewport.SetContent(rm.renderDiffContent(e))
				rm.showDiff = true
			}
		case "r":
			if len(rm.entries) > 0 && rm.rollback != nil {
				if rm.confirmReset == "soft" {
					e := rm.entries[rm.cursor]
					_, err := rm.rollback.SoftReset(e.CommitInfo.Hash, nil)
					if err != nil {
						rm.errMsg = err.Error()
					} else {
						rm.errMsg = ""
						rm.LoadCommits()
					}
					rm.confirmReset = ""
				} else {
					rm.confirmReset = "soft"
				}
			}
		case "R":
			if len(rm.entries) > 0 && rm.rollback != nil {
				if rm.confirmReset == "hard" {
					e := rm.entries[rm.cursor]
					_, err := rm.rollback.HardReset(e.CommitInfo.Hash)
					if err != nil {
						rm.errMsg = err.Error()
					} else {
						rm.errMsg = ""
						rm.LoadCommits()
					}
					rm.confirmReset = ""
				} else {
					rm.confirmReset = "hard"
				}
			}
		default:
			rm.confirmReset = ""
		}
		if rm.showDiff {
			var cmd tea.Cmd
			rm.viewport, cmd = rm.viewport.Update(msg)
			return rm, cmd
		}
	}
	return rm, nil
}

// View implements tea.Model — content only, chrome handled by PageLayout.
func (rm *RollbackModel) View() string {
	t := rm.theme
	w := rm.width
	if w <= 0 {
		w = 80 // only when uninitialized
	}

	if rm.errMsg != "" {
		return lipgloss.NewStyle().Foreground(t.Error).PaddingLeft(2).
			Render("! " + rm.errMsg)
	}

	if len(rm.entries) == 0 {
		return lipgloss.NewStyle().Foreground(t.TextMuted).PaddingLeft(2).
			Render("No commits found in this repository.")
	}

	if rm.showDiff {
		return rm.renderDiffView()
	}

	content := rm.renderCommitList(w)
	if rm.confirmReset != "" {
		action := "soft reset"
		if rm.confirmReset == "hard" {
			action = "HARD reset"
		}
		prompt := lipgloss.NewStyle().Foreground(t.Warning).Bold(true).PaddingLeft(2).
			Render(fmt.Sprintf("Press %s again to confirm %s (any other key to cancel)", rm.confirmReset, action))
		content = lipgloss.JoinVertical(lipgloss.Left, content, "", prompt)
	}
	return content
}

// renderCommitList renders the list of commits.
func (rm *RollbackModel) renderCommitList(w int) string {
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

	return lipgloss.JoinVertical(lipgloss.Left,
		strings.Join(rows, "\n"),
		"", scrollInfo)
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
	msg := tuitypes.TruncateWithEllipsis(c.Message, w-30)

	return prefix +
		hashStyle.Render(c.ShortHash) + "  " +
		lipgloss.NewStyle().Foreground(t.TextMuted).Render(ts) + "  " +
		msgStyle.Render(msg)
}

// renderDiffView renders the diff viewport for the selected commit.
func (rm *RollbackModel) renderDiffView() string {
	t := rm.theme
	e := rm.entries[rm.cursor]
	subTitle := lipgloss.NewStyle().Foreground(t.TextMuted).PaddingLeft(2).
		Render(fmt.Sprintf("Diff for %s — %s", e.CommitInfo.ShortHash, tuitypes.TruncateWithEllipsis(e.CommitInfo.Message, 50)))
	return lipgloss.JoinVertical(lipgloss.Left,
		subTitle, rm.viewport.View())
}

// renderDiffContent produces the colored diff string for a commit.
func (rm *RollbackModel) renderDiffContent(e rollback.RollbackEntry) string {
	if e.Diff == "" {
		return lipgloss.NewStyle().Foreground(rm.theme.TextMuted).
			Render("  (current HEAD — no diff)")
	}
	return colorizeDiff(e.Diff, rm.theme)
}

// colorizeDiff applies basic syntax highlighting to a diff string.
func colorizeDiff(diff string, t theme.Theme) string {
	var b strings.Builder
	lines := strings.Split(diff, "\n")
	for _, line := range lines {
		switch {
		case strings.HasPrefix(line, "+++") || strings.HasPrefix(line, "---"):
			b.WriteString(lipgloss.NewStyle().Foreground(t.Brand).Render(line))
		case strings.HasPrefix(line, "+"):
			b.WriteString(lipgloss.NewStyle().Foreground(t.Success).Render(line))
		case strings.HasPrefix(line, "-"):
			b.WriteString(lipgloss.NewStyle().Foreground(t.Error).Render(line))
		case strings.HasPrefix(line, "@@"):
			b.WriteString(lipgloss.NewStyle().Foreground(t.TextMuted).Render(line))
		default:
			b.WriteString(line)
		}
		b.WriteString("\n")
	}
	return b.String()
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

// listVisibleRows returns the number of rows available for the commit list.
func (rm *RollbackModel) listVisibleRows() int {
	// chrome: title(1) + header row(1) + footer/hints(2) + margins(2)
	const rollbackListChromeH = 6
	h := rm.height - rollbackListChromeH
	if h < 3 {
		return 3
	}
	return h
}
