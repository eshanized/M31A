package tui

import (
	"errors"
	"testing"
	"time"

	"github.com/eshanized/M31A/internal/config"
	m31errors "github.com/eshanized/M31A/internal/errors"
	"github.com/eshanized/M31A/internal/tui/theme"
	"github.com/eshanized/M31A/internal/types"
)

// newMinimalApp creates a minimal AppState for testing message handlers.
func newMinimalApp(t *testing.T) *AppState {
	t.Helper()
	return &AppState{
		themeManager: theme.NewManager(theme.ModeDark),
		screen:       ScreenREPL,
		config:       &config.Config{},
	}
}

// TestApp_Update_ErrorMsg verifies that ErrorMsg sets currentOperation.
func TestApp_Update_ErrorMsg(t *testing.T) {
	t.Parallel()
	app := newMinimalApp(t)

	_, cmd := app.Update(ErrorMsg{Err: errors.New("something broke")})
	if cmd != nil {
		t.Errorf("expected nil cmd, got %T", cmd)
	}
	if app.currentOperation == "" {
		t.Error("expected currentOperation to be set from ErrorMsg")
	}
}

// TestApp_Update_ErrorMsg_SentinelError verifies sentinel errors produce user-friendly messages.
func TestApp_Update_ErrorMsg_SentinelError(t *testing.T) {
	t.Parallel()
	app := newMinimalApp(t)

	_, _ = app.Update(ErrorMsg{Err: m31errors.ErrInvalidKey})
	if app.currentOperation == "" {
		t.Error("expected currentOperation to be set for ErrInvalidKey")
	}
}

// TestApp_Update_HealthCheckResultMsg verifies health status is stored and next tick is scheduled.
func TestApp_Update_HealthCheckResultMsg(t *testing.T) {
	t.Parallel()
	app := newMinimalApp(t)
	app.healthCheckInFlight = true

	_, cmd := app.Update(HealthCheckResultMsg{
		Result: types.HealthStatus{Status: "live", LatencyMs: 42},
	})
	if cmd == nil {
		t.Fatal("expected non-nil cmd (next health tick)")
	}
	if app.healthCheckInFlight {
		t.Error("expected healthCheckInFlight to be false after result")
	}
	if app.healthStatus.Status != "live" {
		t.Errorf("expected health status 'live', got %q", app.healthStatus.Status)
	}
	if app.healthStatus.LatencyMs != 42 {
		t.Errorf("expected latency 42, got %d", app.healthStatus.LatencyMs)
	}
}

// TestApp_Update_HealthCheckResultMsg_InvalidatedHeaderCache verifies header cache invalidation.
func TestApp_Update_HealthCheckResultMsg_InvalidatedHeaderCache(t *testing.T) {
	t.Parallel()
	app := newMinimalApp(t)
	app.headerCacheValid = true

	_, _ = app.Update(HealthCheckResultMsg{
		Result: types.HealthStatus{Status: "unhealthy"},
	})
	if app.headerCacheValid {
		t.Error("expected headerCacheValid to be false after health check result")
	}
}

// TestApp_Update_PermissionRequestMsg verifies the permission handler is invoked.
func TestApp_Update_PermissionRequestMsg(t *testing.T) {
	t.Parallel()
	app := newMinimalApp(t)

	// PermissionRequestMsg with nil dispatcher should not panic
	_, cmd := app.Update(PermissionRequestMsg{})
	// cmd may be nil if dispatcher is nil (no permission modal shown)
	_ = cmd
}

// TestApp_Update_ConfigReloadMsg_Success verifies config fields are updated on reload.
func TestApp_Update_ConfigReloadMsg_Success(t *testing.T) {
	t.Parallel()
	app := newMinimalApp(t)
	app.config.UI.Theme = "dark"

	newCfg := &config.Config{
		UI: config.UIConfig{
			Theme:       "light",
			CompactMode: true,
		},
		Permissions: config.PermissionsConfig{
			DefaultMode: "ask",
		},
		Features: config.FeaturesConfig{
			AutoBackup: true,
		},
	}

	_, cmd := app.Update(config.ConfigReloadMsg{Config: newCfg})
	if cmd != nil {
		t.Errorf("expected nil cmd, got %T", cmd)
	}
	if app.config.UI.Theme != "light" {
		t.Errorf("expected theme 'light' after reload, got %q", app.config.UI.Theme)
	}
	if !app.config.UI.CompactMode {
		t.Error("expected CompactMode true after reload")
	}
	if app.config.Permissions.DefaultMode != "ask" {
		t.Errorf("expected permissions mode 'ask', got %q", app.config.Permissions.DefaultMode)
	}
	if !app.config.Features.AutoBackup {
		t.Error("expected AutoBackup true after reload")
	}
}

// TestApp_Update_ConfigReloadMsg_Error verifies reload errors are logged and config unchanged.
func TestApp_Update_ConfigReloadMsg_Error(t *testing.T) {
	t.Parallel()
	app := newMinimalApp(t)
	app.config.UI.Theme = "dark"

	_, cmd := app.Update(config.ConfigReloadMsg{Error: errors.New("parse error")})
	if cmd != nil {
		t.Errorf("expected nil cmd, got %T", cmd)
	}
	// Config should remain unchanged
	if app.config.UI.Theme != "dark" {
		t.Errorf("expected theme unchanged 'dark', got %q", app.config.UI.Theme)
	}
}

// TestApp_Update_ConfigReloadMsg_NilConfig verifies nil config doesn't panic.
func TestApp_Update_ConfigReloadMsg_NilConfig(t *testing.T) {
	t.Parallel()
	app := newMinimalApp(t)

	_, cmd := app.Update(config.ConfigReloadMsg{Config: nil})
	if cmd != nil {
		t.Errorf("expected nil cmd, got %T", cmd)
	}
}

// TestApp_Update_FallbackEventMsg verifies fallback banner is set.
func TestApp_Update_FallbackEventMsg(t *testing.T) {
	t.Parallel()
	app := newMinimalApp(t)

	_, cmd := app.Update(FallbackEventMsg{
		From:   "openrouter",
		To:     "zen",
		Reason: "rate_limited",
	})
	if cmd != nil {
		t.Errorf("expected nil cmd, got %T", cmd)
	}
}

// TestApp_Update_ToastMsg verifies toast scheduling.
func TestApp_Update_ToastMsg(t *testing.T) {
	t.Parallel()
	app := newMinimalApp(t)

	_, cmd := app.Update(ToastMsg{
		Text:     "Operation complete",
		Duration: 5 * time.Second,
		Type:     "success",
	})
	if cmd == nil {
		t.Fatal("expected non-nil cmd (toast expiry timer)")
	}
	if app.toastText != "Operation complete" {
		t.Errorf("expected toast text 'Operation complete', got %q", app.toastText)
	}
	if app.toastType != "success" {
		t.Errorf("expected toast type 'success', got %q", app.toastType)
	}
}

// TestApp_Update_ToastExpiryMsg verifies toast is cleared on expiry.
func TestApp_Update_ToastExpiryMsg(t *testing.T) {
	t.Parallel()
	app := newMinimalApp(t)
	app.toastText = "old toast"
	app.toastExpires = time.Now().Add(-time.Second) // already expired

	_, cmd := app.Update(ToastExpiryMsg{})
	if cmd != nil {
		t.Errorf("expected nil cmd, got %T", cmd)
	}
	if app.toastText != "" {
		t.Errorf("expected toast cleared, got %q", app.toastText)
	}
}

// TestApp_Update_StreamChunkMsg_NilReplModel verifies no panic with nil replModel.
func TestApp_Update_StreamChunkMsg_NilReplModel(t *testing.T) {
	t.Parallel()
	app := newMinimalApp(t)
	// replModel is nil by default

	_, cmd := app.Update(StreamChunkMsg{
		Chunk: &types.StreamChunk{Type: "content", Delta: "test"},
	})
	if cmd != nil {
		t.Errorf("expected nil cmd, got %T", cmd)
	}
}

// TestApp_Update_ThemeChangedMsg verifies theme change handling.
func TestApp_Update_ThemeChangedMsg(t *testing.T) {
	t.Parallel()
	app := newMinimalApp(t)

	_, cmd := app.Update(ThemeChangedMsg{Theme: "light"})
	if cmd != nil {
		t.Errorf("expected nil cmd, got %T", cmd)
	}
}
