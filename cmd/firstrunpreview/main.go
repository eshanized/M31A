package main

import (
	"fmt"
	"os"
	"strings"

	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/tui"
	"github.com/eshanized/M31A/internal/tui/theme"
)

func main() {
	tm := theme.NewManager(theme.ModeDark)
	t := tm.Current()

	widths := []int{40, 60, 100, 140}
	height := 28

	// Panel-only preview: show the panel's actual rendered width at each
	// effective terminal width, so we can verify the border math before the
	// starfield overlay is composed.
	for _, w := range widths {
		fmt.Printf("\n--- panel only @ terminal width=%d ---\n", w)
		fr := tui.NewFirstRunModel(t, nil)
		fr.SetDimensions(w, height)
		v := fr.View()
		viewW := 0
		maxRunes := 0
		for _, line := range strings.Split(v, "\n") {
			if lw := lipgloss.Width(line); lw > viewW {
				viewW = lw
			}
			if rl := len([]rune(line)); rl > maxRunes {
				maxRunes = rl
			}
		}
		viewH := strings.Count(v, "\n") + 1
		fmt.Printf("view size: %d cols (lw) x %d rows, maxRunes=%d\n", viewW, viewH, maxRunes)
		fmt.Println(v)
	}

	os.Exit(0)
}
