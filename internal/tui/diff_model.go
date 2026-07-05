package tui

import (
	"strings"

	"github.com/charmbracelet/bubbles/viewport"
	tea "github.com/charmbracelet/bubbletea"
	"github.com/eshanized/M31A/internal/tui/theme"
)

// DiffModel shows a git diff with syntax coloring.
type DiffModel struct {
	theme    theme.Theme
	diff     string
	title    string
	filePath string
	viewport viewport.Model
	width    int
	height   int

	// Display options
	showLineNumbers bool
	showSideBySide  bool // for future use

	// Stats
	additions int
	deletions int
}

// NewDiffModel creates a DiffModel.
func NewDiffModel(t theme.Theme) *DiffModel {
	return &DiffModel{
		theme:           t,
		showLineNumbers: true,
		showSideBySide:  false,
	}
}

// SetTheme updates the theme.
func (dm *DiffModel) SetTheme(t theme.Theme) {
	dm.theme = t
}

// diffChromeHeight is the total height of non-viewport chrome in the diff view.
// Accounts for: title(1) + filePath(1) + stats(1) + divider(1) + hints(1) + margins(3)
const diffChromeHeight = 8

// SetDimensions updates the diff model dimensions and refreshes the viewport.
func (dm *DiffModel) SetDimensions(w, h int) {
	dm.width = w
	dm.height = h
	vpH := h - diffChromeHeight
	if vpH < 3 {
		vpH = 3
	}
	dm.viewport = viewport.New(w, vpH)
	if dm.diff != "" {
		dm.viewport.SetContent(colorizeDiff(dm.diff, dm.theme))
	}
}

// SetDiff loads a new diff string into the model and computes stats.
func (dm *DiffModel) SetDiff(diff string) {
	dm.diff = diff
	dm.additions = 0
	dm.deletions = 0
	for _, line := range strings.Split(diff, "\n") {
		if strings.HasPrefix(line, "+") && !strings.HasPrefix(line, "+++") {
			dm.additions++
		}
		if strings.HasPrefix(line, "-") && !strings.HasPrefix(line, "---") {
			dm.deletions++
		}
	}
	// Only initialize the viewport if dimensions have already been set via
	// SetDimensions. If not, SetDimensions will apply the content when called.
	if dm.width > 0 && dm.height > 0 {
		vpH := dm.height - diffChromeHeight
		if vpH < 3 {
			vpH = 3
		}
		dm.viewport = viewport.New(dm.width, vpH)
		dm.viewport.SetContent(colorizeDiff(diff, dm.theme))
	}
}

// SetTitle sets the optional title line shown above the diff.
func (dm *DiffModel) SetTitle(title string) {
	dm.title = title
	if strings.Contains(title, " — ") {
		parts := strings.SplitN(title, " — ", 2)
		if len(parts) == 2 {
			dm.filePath = parts[1]
		}
	}
}

// Init implements tea.Model.
func (dm *DiffModel) Init() tea.Cmd { return nil }

// Update implements tea.Model.
func (dm *DiffModel) Update(msg tea.Msg) (tea.Model, tea.Cmd) {
	switch msg := msg.(type) {
	case tea.WindowSizeMsg:
		dm.SetDimensions(msg.Width, msg.Height)
		return dm, nil
	case DiffCloseMsg:
		return dm, func() tea.Msg {
			return PopScreenMsg{}
		}
	case tea.KeyMsg:
		switch msg.String() {
		case "esc", "q":
			return dm, func() tea.Msg {
				return DiffCloseMsg{}
			}
		}
	}
	var cmd tea.Cmd
	dm.viewport, cmd = dm.viewport.Update(msg)
	return dm, cmd
}

// View implements tea.Model — delegates to diff_view.go.
func (dm *DiffModel) View() string {
	return renderDiffView(dm)
}
