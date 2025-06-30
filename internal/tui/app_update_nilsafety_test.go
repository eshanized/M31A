package tui

import (
	"errors"
	"testing"
	"time"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/eshanized/M31A/internal/tui/theme"
	"github.com/eshanized/M31A/internal/types"
)

// TestApp_Update_ReplModelNil_NoPanic verifies that AppState.Update()
// does not panic when m.replModel is nil and any representative tea.Msg
// type is delivered. This is the regression test for audit finding C-1
// (CRITICAL — TUI panic on nil m.replModel).
//
// The test constructs a minimal AppState with m.replModel explicitly set
// to nil, then calls Update() for each message type that previously
// dereferenced replModel without a guard.
func TestApp_Update_ReplModelNil_NoPanic(t *testing.T) {
	t.Parallel()

	// Helper to create a minimal AppState with nil replModel.
	newNilReplApp := func(t *testing.T) *AppState {
		t.Helper()
		app := &AppState{
			themeManager: theme.NewManager(theme.ModeDark),
			screen:       ScreenREPL,
			// replModel intentionally nil — this is the C-1 trigger condition.
		}
		return app
	}

	// Each sub-test sends one message type and asserts no panic.
	t.Run("WindowSizeMsg", func(t *testing.T) {
		t.Parallel()
		app := newNilReplApp(t)
		defer func() {
			if r := recover(); r != nil {
				t.Fatalf("panic on WindowSizeMsg with nil replModel: %v", r)
			}
		}()
		_, cmd := app.Update(tea.WindowSizeMsg{Width: 80, Height: 24})
		if cmd != nil {
			t.Errorf("expected nil cmd, got %T", cmd)
		}
	})

	t.Run("KeyMsg_ctrl_c", func(t *testing.T) {
		t.Parallel()
		app := newNilReplApp(t)
		defer func() {
			if r := recover(); r != nil {
				t.Fatalf("panic on ctrl+c with nil replModel: %v", r)
			}
		}()
		_, cmd := app.Update(tea.KeyMsg{Type: tea.KeyCtrlC})
		// ctrl+c returns tea.Quit when not streaming
		if cmd == nil {
			t.Fatal("expected non-nil cmd (quit)")
		}
	})

	t.Run("KeyMsg普通的key", func(t *testing.T) {
		t.Parallel()
		app := newNilReplApp(t)
		defer func() {
			if r := recover(); r != nil {
				t.Fatalf("panic on regular key with nil replModel: %v", r)
			}
		}()
		// A regular key that doesn't match any special case
		_, _ = app.Update(tea.KeyMsg{Type: tea.KeyRunes, Runes: []rune("a")})
	})

	t.Run("StreamChunkMsg", func(t *testing.T) {
		t.Parallel()
		app := newNilReplApp(t)
		defer func() {
			if r := recover(); r != nil {
				t.Fatalf("panic on StreamChunkMsg with nil replModel: %v", r)
			}
		}()
		_, cmd := app.Update(StreamChunkMsg{
			Chunk:  &types.StreamChunk{Type: "content", Delta: "test"},
			Source: "discuss",
		})
		if cmd != nil {
			t.Errorf("expected nil cmd, got %T", cmd)
		}
	})

	t.Run("StreamErrorMsg", func(t *testing.T) {
		t.Parallel()
		app := newNilReplApp(t)
		defer func() {
			if r := recover(); r != nil {
				t.Fatalf("panic on StreamErrorMsg with nil replModel: %v", r)
			}
		}()
		_, cmd := app.Update(StreamErrorMsg{Err: errors.New("stream failed")})
		if cmd != nil {
			t.Errorf("expected nil cmd, got %T", cmd)
		}
	})

	t.Run("PhaseResultMsg_no_success", func(t *testing.T) {
		t.Parallel()
		app := newNilReplApp(t)
		defer func() {
			if r := recover(); r != nil {
				t.Fatalf("panic on PhaseResultMsg (error) with nil replModel: %v", r)
			}
		}()
		_, cmd := app.Update(PhaseResultMsg{
			Phase: types.PhaseInitialize,
			Error: "something failed",
		})
		if cmd != nil {
			t.Errorf("expected nil cmd, got %T", cmd)
		}
	})

	t.Run("QuestionResponseMsg", func(t *testing.T) {
		t.Parallel()
		app := newNilReplApp(t)
		defer func() {
			if r := recover(); r != nil {
				t.Fatalf("panic on QuestionResponseMsg with nil replModel: %v", r)
			}
		}()
		// QuestionResponseMsg routes through dispatcher, not replModel directly
		_, _ = app.Update(QuestionResponseMsg{Answer: "test"})
	})

	t.Run("SettingsSavedMsg", func(t *testing.T) {
		t.Parallel()
		app := newNilReplApp(t)
		defer func() {
			if r := recover(); r != nil {
				t.Fatalf("panic on SettingsSavedMsg with nil replModel: %v", r)
			}
		}()
		_, cmd := app.Update(SettingsSavedMsg{})
		if cmd == nil {
			t.Error("expected non-nil cmd (health tick + cache refresh)")
		}
	})

	t.Run("ThemeChangedMsg", func(t *testing.T) {
		t.Parallel()
		app := newNilReplApp(t)
		defer func() {
			if r := recover(); r != nil {
				t.Fatalf("panic on ThemeChangedMsg with nil replModel: %v", r)
			}
		}()
		_, cmd := app.Update(ThemeChangedMsg{Theme: "light"})
		if cmd != nil {
			t.Errorf("expected nil cmd, got %T", cmd)
		}
	})

	t.Run("ErrorMsg", func(t *testing.T) {
		t.Parallel()
		app := newNilReplApp(t)
		defer func() {
			if r := recover(); r != nil {
				t.Fatalf("panic on ErrorMsg with nil replModel: %v", r)
			}
		}()
		_, cmd := app.Update(ErrorMsg{Err: errors.New("test error")})
		if cmd != nil {
			t.Errorf("expected nil cmd, got %T", cmd)
		}
	})

	t.Run("SidebarRefreshMsg", func(t *testing.T) {
		t.Parallel()
		app := newNilReplApp(t)
		defer func() {
			if r := recover(); r != nil {
				t.Fatalf("panic on SidebarRefreshMsg with nil replModel: %v", r)
			}
		}()
		_, cmd := app.Update(SidebarRefreshMsg{})
		if cmd != nil {
			t.Errorf("expected nil cmd, got %T", cmd)
		}
	})

	t.Run("ToastMsg", func(t *testing.T) {
		t.Parallel()
		app := newNilReplApp(t)
		defer func() {
			if r := recover(); r != nil {
				t.Fatalf("panic on ToastMsg with nil replModel: %v", r)
			}
		}()
		_, cmd := app.Update(ToastMsg{Text: "hello", Duration: time.Second, Type: "info"})
		if cmd != nil {
			t.Errorf("expected nil cmd, got %T", cmd)
		}
	})

	t.Run("HealthCheckTickMsg", func(t *testing.T) {
		t.Parallel()
		app := newNilReplApp(t)
		defer func() {
			if r := recover(); r != nil {
				t.Fatalf("panic on HealthCheckTickMsg with nil replModel: %v", r)
			}
		}()
		_, cmd := app.Update(HealthCheckTickMsg{Time: time.Now()})
		if cmd == nil {
			t.Error("expected non-nil cmd (reschedule)")
		}
	})

	t.Run("FallbackEventMsg", func(t *testing.T) {
		t.Parallel()
		app := newNilReplApp(t)
		defer func() {
			if r := recover(); r != nil {
				t.Fatalf("panic on FallbackEventMsg with nil replModel: %v", r)
			}
		}()
		_, cmd := app.Update(FallbackEventMsg{From: "openrouter", To: "zen", Reason: "rate_limited"})
		if cmd != nil {
			t.Errorf("expected nil cmd, got %T", cmd)
		}
	})

	t.Run("PermissionRequestMsg", func(t *testing.T) {
		t.Parallel()
		app := newNilReplApp(t)
		defer func() {
			if r := recover(); r != nil {
				t.Fatalf("panic on PermissionRequestMsg with nil replModel: %v", r)
			}
		}()
		// PermissionRequestMsg doesn't touch replModel, but should not panic
		_, _ = app.Update(PermissionRequestMsg{})
	})
}
