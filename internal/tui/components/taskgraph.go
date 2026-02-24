package components

import (
	"fmt"
	"strings"

	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/tui/theme"
)

// TaskNode represents a task in a dependency graph.
type TaskNode struct {
	ID     int
	Label  string
	Status string // "pending", "running", "done", "failed"
	Deps   []int
	X, Y   int // layout position
}

// TaskGraph renders an ASCII dependency graph.
type TaskGraph struct {
	Nodes  []TaskNode
	Theme  theme.Theme
	Width  int
	Height int
}

// View renders the dependency graph.
func (tg TaskGraph) View() string {
	t := tg.Theme

	if len(tg.Nodes) == 0 {
		return lipgloss.NewStyle().Foreground(t.TextMuted).
			Render("No tasks.")
	}

	// Build adjacency: find wave (topological layer) for each node
	idToWave := make(map[int]int)
	for _, node := range tg.Nodes {
		maxDepWave := -1
		for _, dep := range node.Deps {
			if w, ok := idToWave[dep]; ok && w > maxDepWave {
				maxDepWave = w
			}
		}
		idToWave[node.ID] = maxDepWave + 1
	}

	// Group nodes by wave
	maxWave := 0
	for _, w := range idToWave {
		if w > maxWave {
			maxWave = w
		}
	}

	waves := make([][]TaskNode, maxWave+1)
	for _, node := range tg.Nodes {
		w := idToWave[node.ID]
		if w < 0 {
			w = 0
		}
		waves[w] = append(waves[w], node)
	}

	var lines []string
	for wi, wave := range waves {
		header := lipgloss.NewStyle().Foreground(t.TextMuted).Bold(true).
			Render(fmt.Sprintf("Wave %d:", wi+1))
		lines = append(lines, header)

		for _, node := range wave {
			var icon string
			var color lipgloss.Color
			switch node.Status {
			case "done":
				icon = "✓"
				color = t.Success
			case "running":
				icon = "●"
				color = t.Brand
			case "failed":
				icon = "✗"
				color = t.Error
			default:
				icon = "○"
				color = t.TextMuted
			}

			iconStyled := lipgloss.NewStyle().Foreground(color).Render(icon)
			label := lipgloss.NewStyle().Foreground(t.Text).Render(node.Label)

			depStr := ""
			if len(node.Deps) > 0 {
				var depIDs []string
				for _, d := range node.Deps {
					depIDs = append(depIDs, fmt.Sprintf("#%d", d))
				}
				depStr = " " + lipgloss.NewStyle().Foreground(t.TextMuted).
					Render("← "+strings.Join(depIDs, ", "))
			}

			lines = append(lines, fmt.Sprintf("    %s %s%s", iconStyled, label, depStr))
		}

		if wi < len(waves)-1 {
			lines = append(lines, lipgloss.NewStyle().Foreground(t.Border).
				Render("    │"))
		}
	}

	return strings.Join(lines, "\n")
}
