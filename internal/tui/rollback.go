package tui

import (
	"fmt"
	"strings"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/git"
	"github.com/eshanized/M31A/internal/tui/theme"
	"github.com/eshanized/M31A/pkg/rollback"
)

// RollbackModel provides a commit browser with rollback actions.
type RollbackModel struct {
	theme       theme.Theme
	width       int
	height      int
	commits     []git.CommitInfo
	selected    int
	showDiff    bool
	diffContent string
	confirming  bool
	confirmType string // "soft" or "hard"
	git         *git.Git
	rollback    *rollback.Rollback
	sessionID   string
}

// NewRollbackModel creates a RollbackModel with the given dependencies.
func NewRollbackModel(t theme.Theme, g *git.Git, rb *rollback.Rollback, width, height int) *RollbackModel {
	return &RollbackModel{
		theme:    t,
		git:      g,
		rollback: rb,
		width:    width,
		height:   height,
	}
}

// Init returns nil.
func (m *RollbackModel) Init() tea.Cmd {
	return nil
}

// LoadCommits populates the commit list.
func (m *RollbackModel) LoadCommits() error {
	if m.rollback != nil {
		entries, err := m.rollback.Chain(20)
		if err != nil {
			return err
		}
		m.commits = make([]git.CommitInfo, len(entries))
		for i, e := range entries {
			m.commits[i] = e.CommitInfo
		}
		return nil
	}
	if m.git != nil {
		commits, err := m.git.Log(false, "")
		if err != nil {
			return err
		}
		if len(commits) > 20 {
			commits = commits[:20]
		}
		m.commits = commits
		return nil
	}
	return nil
}

// Update handles messages for the rollback screen.
func (m *RollbackModel) Update(msg tea.Msg) ([]tea.Cmd, *AppMsg) {
	switch msg := msg.(type) {
	case tea.WindowSizeMsg:
		m.width = msg.Width
		m.height = msg.Height
		return nil, nil

	case tea.KeyMsg:
		if m.confirming {
			switch msg.String() {
			case "y", "Y", "enter":
				return m.executeRollback()
			case "n", "N", "esc":
				m.confirming = false
				m.confirmType = ""
				return nil, nil
			}
			return nil, nil
		}

		switch msg.String() {
		case "up", "k":
			if m.selected > 0 {
				m.selected--
				m.diffContent = ""
				m.showDiff = false
			}
			return nil, nil
		case "down", "j":
			if m.selected < len(m.commits)-1 {
				m.selected++
				m.diffContent = ""
				m.showDiff = false
			}
			return nil, nil
		case "d":
			if len(m.commits) > 0 {
				m.showDiff = !m.showDiff
				if m.showDiff && m.diffContent == "" {
					m.loadDiff()
				}
			}
			return nil, nil
		case "r":
			if len(m.commits) > 0 {
				m.confirming = true
				m.confirmType = "soft"
			}
			return nil, nil
		case "R":
			if len(m.commits) > 0 {
				m.confirming = true
				m.confirmType = "hard"
			}
			return nil, nil
		case "esc":
			return nil, &AppMsg{Screen: ScreenREPL}
		}
	}
	return nil, nil
}

// loadDiff fetches the diff for the selected commit.
func (m *RollbackModel) loadDiff() {
	if m.selected >= len(m.commits) {
		return
	}
	commit := m.commits[m.selected]
	if m.git != nil {
		diff, err := m.git.Diff(commit.Hash+"^", commit.Hash)
		if err != nil {
			m.diffContent = fmt.Sprintf("[diff unavailable: %v]", err)
		} else {
			m.diffContent = diff
		}
	}
}

// executeRollback performs the selected rollback action.
func (m *RollbackModel) executeRollback() ([]tea.Cmd, *AppMsg) {
	if m.selected >= len(m.commits) {
		m.confirming = false
		return nil, nil
	}
	commit := m.commits[m.selected]
	hash := commit.Hash

	if m.confirmType == "soft" && m.rollback != nil {
		result, err := m.rollback.SoftReset(hash, nil)
		if err != nil {
			m.confirming = false
			return nil, &AppMsg{Screen: ScreenREPL, Action: fmt.Sprintf("Rollback failed: %v", err)}
		}
		m.confirming = false
		return nil, &AppMsg{Screen: ScreenREPL, Action: result.Message}
	}
	if m.confirmType == "hard" && m.rollback != nil {
		result, err := m.rollback.HardReset(hash)
		if err != nil {
			m.confirming = false
			return nil, &AppMsg{Screen: ScreenREPL, Action: fmt.Sprintf("Rollback failed: %v", err)}
		}
		m.confirming = false
		return nil, &AppMsg{Screen: ScreenREPL, Action: result.Message}
	}

	m.confirming = false
	return nil, &AppMsg{Screen: ScreenREPL, Action: "Rollback not available"}
}

// View renders the rollback screen.
func (m *RollbackModel) View() string {
	if m.confirming {
		return m.renderConfirmation()
	}

	var parts []string

	// Header
	headerStyle := lipgloss.NewStyle().
		Foreground(m.theme.Brand).
		Bold(true).
		Padding(0, 1)
	parts = append(parts, headerStyle.Render("/rollback — Commit Time Machine"))

	// Commit list
	if len(m.commits) == 0 {
		emptyStyle := lipgloss.NewStyle().
			Foreground(m.theme.TextSecondary).
			Italic(true).
			Padding(2, 4)
		parts = append(parts, emptyStyle.Render("No commits found"))
	} else {
		for i, commit := range m.commits {
			card := m.renderCommit(i, commit)
			parts = append(parts, card)
		}
	}

	// Diff preview
	if m.showDiff && m.diffContent != "" {
		parts = append(parts, "")
		diffHeader := lipgloss.NewStyle().Foreground(m.theme.Brand).Bold(true).Render("Diff Preview:")
		parts = append(parts, diffHeader)
		diffStyle := lipgloss.NewStyle().
			Border(lipgloss.NormalBorder()).
			BorderForeground(m.theme.Border).
			Padding(0, 1).
			Width(m.width - 4)
		diff := m.diffContent
		if len(diff) > 2000 {
			diff = diff[:2000] + "\n... [truncated]"
		}
		parts = append(parts, diffStyle.Render(diff))
	}

	// Footer
	footerStyle := lipgloss.NewStyle().Foreground(m.theme.TextSecondary)
	footer := footerStyle.Render("j/k: navigate  |  d: diff  |  r: soft rollback  |  R: hard rollback  |  Esc: back")
	parts = append(parts, "", footer)

	return strings.Join(parts, "\n")
}

// renderCommit renders a single commit entry.
func (m *RollbackModel) renderCommit(idx int, commit git.CommitInfo) string {
	indicator := "  "
	if idx == m.selected {
		indicator = lipgloss.NewStyle().Foreground(m.theme.Brand).Render("▶ ")
	}

	shortHash := commit.ShortHash
	if shortHash == "" && len(commit.Hash) > 7 {
		shortHash = commit.Hash[:7]
	} else if shortHash == "" {
		shortHash = commit.Hash
	}

	marker := ""
	if idx == 0 {
		marker = lipgloss.NewStyle().Foreground(m.theme.Warning).Render(" [HEAD]")
	}

	line := fmt.Sprintf("%s%s %s%s",
		indicator,
		lipgloss.NewStyle().Foreground(m.theme.TextPrimary).Bold(true).Render(shortHash),
		lipgloss.NewStyle().Foreground(m.theme.TextPrimary).Render(commit.Message),
		marker,
	)

	if idx == m.selected {
		cardStyle := lipgloss.NewStyle().
			Border(lipgloss.RoundedBorder()).
			BorderForeground(m.theme.Brand).
			Padding(0, 1).
			Width(m.width - 4)
		return cardStyle.Render(line)
	}

	return line
}

// renderConfirmation renders the rollback confirmation dialog.
func (m *RollbackModel) renderConfirmation() string {
	commit := "unknown"
	if m.selected < len(m.commits) {
		shortHash := m.commits[m.selected].ShortHash
		if shortHash == "" {
			shortHash = m.commits[m.selected].Hash[:7]
		}
		commit = shortHash
	}

	prompt := fmt.Sprintf("Perform %s rollback to %s?", m.confirmType, commit)

	confirmStyle := lipgloss.NewStyle().
		Background(lipgloss.Color(m.theme.SurfaceElevated)).
		Foreground(lipgloss.Color(m.theme.TextPrimary)).
		Padding(1, 2).
		Border(theme.DoubleBorder).
		BorderForeground(lipgloss.Color(m.theme.Warning))

	hintStyle := lipgloss.NewStyle().
		Foreground(m.theme.TextSecondary).
		Padding(0, 2)

	content := lipgloss.JoinVertical(lipgloss.Left,
		confirmStyle.Render(prompt),
		hintStyle.Render("Y: confirm  |  N: cancel"),
	)

	return centerScreen(content, m.width, m.height)
}
