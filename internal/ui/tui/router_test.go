package tui

import (
	"testing"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/eshanized/M31A/internal/ui/tui/theme"
	"github.com/eshanized/M31A/internal/ui/tui/tuitypes"
)

// mockScreen implements Screenable for testing
type mockScreen struct {
	viewOutput string
}

func newMockScreen(viewOutput string) *mockScreen {
	return &mockScreen{viewOutput: viewOutput}
}

func (m *mockScreen) Init() tea.Cmd {
	return nil
}

func (m *mockScreen) Update(msg tea.Msg) (tea.Model, tea.Cmd) {
	return m, nil
}

func (m *mockScreen) View() string {
	return m.viewOutput
}

func (m *mockScreen) SetDimensions(w, h int) {}

func (m *mockScreen) SetTheme(t theme.Theme) {}

const screenTestA = tuitypes.Screen(100)

// TestRouter_SwitchToBeforeRegister verifies that SwitchTo() sets activeID
// even when the screen is not yet registered, and that Register() auto-activates
// a screen that was SwitchTo'd before registration.
func TestRouter_SwitchToBeforeRegister(t *testing.T) {
	router := NewRouter()

	// Call SwitchTo BEFORE Register
	router.SwitchTo(screenTestA)

	// The bug was: ActiveID() returned 0 because activeID was only set inside
	// the registered-screen guard. The fix ensures activeID is set unconditionally.
	if router.ActiveID() != screenTestA {
		t.Errorf("Expected ActiveID() == %d after SwitchTo before Register, got %d", screenTestA, router.ActiveID())
	}

	// Now register the screen - this should auto-activate it
	mock := newMockScreen("test-content")
	router.Register(screenTestA, mock)

	// Verify auto-activate worked: View() should return the mock's content
	if router.View() != "test-content" {
		t.Errorf("Expected View() == 'test-content' after auto-activate, got %q", router.View())
	}
}

// TestRouter_SwitchToZeroNoOp verifies that SwitchTo(0) is a no-op.
func TestRouter_SwitchToZeroNoOp(t *testing.T) {
	router := NewRouter()

	router.SwitchTo(0)

	if router.ActiveID() != 0 {
		t.Errorf("Expected ActiveID() == 0 after SwitchTo(0), got %d", router.ActiveID())
	}
}

// TestRouter_SwitchToRegisteredScreen verifies SwitchTo works normally
// when the screen is already registered.
func TestRouter_SwitchToRegisteredScreen(t *testing.T) {
	router := NewRouter()

	mock := newMockScreen("registered-content")
	router.Register(screenTestA, mock)

	router.SwitchTo(screenTestA)

	if router.View() != "registered-content" {
		t.Errorf("Expected View() == 'registered-content', got %q", router.View())
	}
}