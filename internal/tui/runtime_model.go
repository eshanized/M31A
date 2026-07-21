package tui

import (
	"fmt"
	"strings"
	"time"

	"github.com/charmbracelet/bubbles/viewport"
	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/tui/components"
	"github.com/eshanized/M31A/internal/tui/theme"
	"github.com/eshanized/M31A/internal/engine/workflow"
)

// RuntimeModel displays runtime verification results (dev server + smoke tests).
type RuntimeModel struct {
	summary   workflow.RuntimeSummary
	theme     theme.Theme
	width     int
	height    int
	viewport  viewport.Model
	testing   bool
	completed bool
	spinner   components.Spinner
}

// NewRuntimeModel creates a RuntimeModel.
func NewRuntimeModel(t theme.Theme, w, h int) *RuntimeModel {
	rm := &RuntimeModel{
		theme:   t,
		width:   w,
		height:  h,
		testing: true,
		spinner: components.NewSpinner(),
	}
	rm.initViewport()
	return rm
}

// Init implements Screenable.
func (rm *RuntimeModel) Init() tea.Cmd { return nil }

// SetDimensions implements Screenable.
func (rm *RuntimeModel) SetDimensions(w, h int) {
	rm.width = w
	rm.height = h
	rm.initViewport()
}

// SetTheme implements Screenable.
func (rm *RuntimeModel) SetTheme(t theme.Theme) {
	rm.theme = t
	rm.viewport.SetContent(rm.renderContent())
}

// View implements Screenable.
func (rm *RuntimeModel) View() string {
	return rm.renderContent()
}

func (rm *RuntimeModel) initViewport() {
	const chrome = 8
	h := rm.height - chrome
	if h < 5 {
		h = 5
	}
	rm.viewport = viewport.New(rm.width, h)
	rm.viewport.SetContent(rm.renderContent())
}

// SetSummary updates the model with runtime verification results.
func (rm *RuntimeModel) SetSummary(summary workflow.RuntimeSummary) {
	rm.summary = summary
	rm.testing = false
	rm.completed = true
	rm.viewport.SetContent(rm.renderContent())
}

// RuntimeTickMsg drives the spinner animation while tests are running.
type RuntimeTickMsg struct{}

// Update handles runtime screen key events.
func (rm *RuntimeModel) Update(msg tea.Msg) (tea.Model, tea.Cmd) {
	switch msg := msg.(type) {
	case tea.WindowSizeMsg:
		rm.width = msg.Width
		rm.height = msg.Height
		rm.initViewport()
	case tea.MouseMsg:
		if msg.Action == tea.MouseActionPress {
			switch msg.Button {
			case tea.MouseButtonWheelUp:
				rm.viewport.LineUp(3)
			case tea.MouseButtonWheelDown:
				rm.viewport.LineDown(3)
			}
		}
	case tea.KeyMsg:
		switch msg.String() {
		case "j", "down":
			rm.viewport.LineDown(1)
		case "k", "up":
			rm.viewport.LineUp(1)
		case "enter", "s":
			return rm, func() tea.Msg {
				return AppMsg{Action: "runtime_continue"}
			}
		case "esc", "q":
			return rm, func() tea.Msg {
				return PopScreenMsg{}
			}
		}
	case RuntimeTickMsg:
		if rm.testing {
			rm.spinner.Next()
			rm.viewport.SetContent(rm.renderContent())
			return rm, tea.Tick(components.SpinnerTickInterval, func(time.Time) tea.Msg {
				return RuntimeTickMsg{}
			})
		}
	}
	return rm, nil
}

func (rm *RuntimeModel) renderContent() string {
	t := rm.theme
	var sections []string

	titleStyle := lipgloss.NewStyle().Bold(true).Foreground(t.Accent)
	sections = append(sections, titleStyle.Render("Runtime Verification"))

	if rm.testing {
		frame := rm.spinner.Peek()
		sections = append(sections, fmt.Sprintf("\n%s Starting dev server and running smoke tests…", frame))
		return lipgloss.JoinVertical(lipgloss.Left, sections...)
	}

	s := rm.summary

	if s.ServerURL != "" {
		serverStatus := "not ready"
		if s.ServerReady {
			serverStatus = "ready"
		}
		sections = append(sections, fmt.Sprintf("\nServer: %s (%s)", s.ServerURL, serverStatus))
		sections = append(sections, fmt.Sprintf("Project: %s", s.ProjectType))
	}

	if len(s.Errors) > 0 {
		errStyle := lipgloss.NewStyle().Foreground(t.Error)
		sections = append(sections, errStyle.Render(fmt.Sprintf("\nErrors:\n  - %s", strings.Join(s.Errors, "\n  - "))))
	}

	if len(s.Tests) > 0 {
		sections = append(sections, fmt.Sprintf("\nSmoke Tests (%d/%d passed):", s.TotalPassed, s.TotalTests))

		for _, test := range s.Tests {
			icon := "PASS"
			style := lipgloss.NewStyle().Foreground(t.Success)
			if !test.Passed {
				icon = "FAIL"
				style = lipgloss.NewStyle().Foreground(t.Error)
			}

			line := fmt.Sprintf("  [%s] %s %s", icon, test.Method, test.Route)
			if test.Status > 0 {
				line += fmt.Sprintf(" -> %d (%d bytes, %dms)", test.Status, test.BodySize, test.DurationMs)
			}
			if test.Error != "" {
				line += fmt.Sprintf(" -- %s", test.Error)
			}
			sections = append(sections, style.Render(line))
		}
	}

	if s.TotalTests > 0 {
		passRate := 0
		if s.TotalTests > 0 {
			passRate = (s.TotalPassed * 100) / s.TotalTests
		}
		sections = append(sections, fmt.Sprintf("\nResult: %d/%d passed (%d%%) in %dms",
			s.TotalPassed, s.TotalTests, passRate, s.DurationMs))
	}

	sections = append(sections, "\n[enter] continue to ship  [esc] back")

	return lipgloss.JoinVertical(lipgloss.Left, sections...)
}
