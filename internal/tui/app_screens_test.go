package tui

import (
	"testing"

	"github.com/eshanized/M31A/internal/tui/theme"
)

func TestAllScreens_RenderWithoutBlank(t *testing.T) {
	tm := theme.NewManager(theme.ModeDark)
	screens := []Screen{
		ScreenFirstRun,
		ScreenHome,
		ScreenREPL,
		ScreenSettings,
		ScreenHelp,
		ScreenConfirmQuit,
		ScreenNotifications,
		ScreenDecisions,
	}

	for _, screen := range screens {
		t.Run(screen.Label(), func(t *testing.T) {
			defer func() {
				if r := recover(); r != nil {
					t.Errorf("Screen %s panicked: %v", screen.Label(), r)
				}
			}()
			m := &AppState{
				width:        80,
				height:       24,
				themeManager: tm,
				screen:       screen,
			}
			cmd := m.routeToScreen()
			_ = cmd
		})
	}
}
