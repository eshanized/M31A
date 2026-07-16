package ghost_picker


import (
	"github.com/eshanized/M31A/internal/tui/tuitypes"
	"fmt"
	"strings"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/tui/components"
	"github.com/eshanized/M31A/internal/tui/theme"
)

// GhostPickerModel shows a file selector for ghost write operations.
type GhostPickerModel struct {
	theme  theme.Theme
	files  []ghostFileEntry
	cursor int
	scroll int
	width  int
	height int
}

type ghostFileEntry struct {
	Path     string
	Selected bool
}

// NewGhostPickerModel creates a GhostPickerModel.
func NewGhostPickerModel(t theme.Theme, w, h int) *GhostPickerModel {
	return &GhostPickerModel{
		theme:  t,
		width:  w,
		height: h,
	}
}

// SetFiles sets the list of files available for ghost write.
func (gp *GhostPickerModel) SetFiles(paths []string) {
	gp.files = make([]ghostFileEntry, len(paths))
	for i, p := range paths {
		gp.files[i] = ghostFileEntry{Path: p, Selected: false}
	}
	gp.cursor = 0
	gp.scroll = 0
}

// SetTheme updates the theme.
func (gp *GhostPickerModel) SetTheme(t theme.Theme) { gp.theme = t }

// SetDimensions updates dimensions.
func (gp *GhostPickerModel) SetDimensions(w, h int) {
	gp.width = w
	gp.height = h
}

// SelectedFiles returns the paths of all selected files.
func (gp *GhostPickerModel) SelectedFiles() []string {
	var selected []string
	for _, f := range gp.files {
		if f.Selected {
			selected = append(selected, f.Path)
		}
	}
	return selected
}

// Init implements tea.Model.
func (gp *GhostPickerModel) Init() tea.Cmd { return nil }

// Update implements tuitypes.Screenable.
func (gp *GhostPickerModel) Update(msg tea.Msg) (tea.Model, tea.Cmd) {
	switch msg := msg.(type) {
	case tea.WindowSizeMsg:
		gp.SetDimensions(msg.Width, msg.Height)
	case tea.KeyMsg:
		switch msg.String() {
		case "esc", "q":
			return gp, func() tea.Msg { return tuitypes.PopScreenMsg{} }
		case "up", "k":
			if gp.cursor > 0 {
				gp.cursor--
				gp.clampScroll()
			}
		case "down", "j":
			if gp.cursor < len(gp.files)-1 {
				gp.cursor++
				gp.clampScroll()
			}
		case " ":
			if gp.cursor < len(gp.files) {
				gp.files[gp.cursor].Selected = !gp.files[gp.cursor].Selected
			}
		case "a":
			// Select all
			for i := range gp.files {
				gp.files[i].Selected = true
			}
		case "A":
			// Deselect all
			for i := range gp.files {
				gp.files[i].Selected = false
			}
		case "enter":
			selected := gp.SelectedFiles()
			if len(selected) == 0 {
				return gp, nil
			}
			return gp, func() tea.Msg {
				return tuitypes.GhostWriteRequestMsg{Files: selected}
			}
		}
	}
	return gp, nil
}

func (gp *GhostPickerModel) clampScroll() {
	visible := gp.height - 8
	if visible < 1 {
		visible = 1
	}
	if gp.cursor < gp.scroll {
		gp.scroll = gp.cursor
	}
	if gp.cursor >= gp.scroll+visible {
		gp.scroll = gp.cursor - visible + 1
	}
}

// View implements tea.Model.
func (gp *GhostPickerModel) View() string {
	t := gp.theme

	title := components.ScreenTitle{Text: "Ghost Write Files", Theme: t}.Render()

	subtitle := lipgloss.NewStyle().Foreground(t.TextMuted).PaddingLeft(2).
		Render("Select files to generate:")

	var lines []string
	lines = append(lines, "", title, subtitle, "")

	if len(gp.files) == 0 {
		lines = append(lines, lipgloss.NewStyle().Foreground(t.TextMuted).PaddingLeft(4).
			Render("No files available for ghost write."))
	} else {
		visible := gp.height - 8
		if visible < 1 {
			visible = 1
		}

		start := gp.scroll
		end := start + visible
		if end > len(gp.files) {
			end = len(gp.files)
		}

		for i := start; i < end; i++ {
			f := gp.files[i]
			selected := i == gp.cursor
			checkbox := "[ ]"
			if f.Selected {
				checkbox = "[x]"
			}

			prefix := "    "
			if selected {
				prefix = components.CursorIndicator{Selected: true, Theme: t}.Render()
			}

			pathStyle := lipgloss.NewStyle().Foreground(t.Text)
			if f.Selected {
				pathStyle = lipgloss.NewStyle().Foreground(t.Success)
			}

			lines = append(lines, fmt.Sprintf("%s%s  %s", prefix, checkbox, pathStyle.Render(f.Path)))
		}

		if len(gp.files) > visible {
			lines = append(lines, lipgloss.NewStyle().Foreground(t.TextMuted).PaddingLeft(4).
				Render(fmt.Sprintf("  ... %d more files (scroll with j/k)", len(gp.files)-visible)))
		}
	}

	selectedCount := len(gp.SelectedFiles())
	summary := lipgloss.NewStyle().Foreground(t.TextSecondary).PaddingLeft(2).
		Render(fmt.Sprintf("  %d file(s) selected", selectedCount))
	lines = append(lines, "", summary)

	footer := lipgloss.NewStyle().Foreground(t.TextMuted).PaddingLeft(2).
		Render("[j/k] Navigate   [space] Toggle   [a/A] Select/Deselect all   [enter] Write   [esc] Back")
	lines = append(lines, "", footer)

	return strings.Join(lines, "\n")
}
