package tui

import (
	"context"
	"strings"
	"testing"
	"time"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/eshanized/M31A/internal/provider"
	"github.com/eshanized/M31A/internal/types"
)

func TestNewApp_NoKey_CreatesFirstRun(t *testing.T) {
	app := NewApp("test", nil, "", "/tmp/config")
	if app.screen != ScreenFirstRun {
		t.Errorf("Expected ScreenFirstRun, got %d", app.screen)
	}
	if app.firstRunModel == nil {
		t.Error("Expected firstRunModel to be non-nil")
	}
	if app.replModel != nil {
		t.Error("Expected replModel to be nil")
	}
}

func TestNewApp_WithKey_CreatesREPL(t *testing.T) {
	app := NewApp("test", nil, "sk-or-v1-key", "/tmp/config")
	if app.screen != ScreenREPL {
		t.Errorf("Expected ScreenREPL, got %d", app.screen)
	}
	if app.replModel == nil {
		t.Error("Expected replModel to be non-nil")
	}
	if app.firstRunModel != nil {
		t.Error("Expected firstRunModel to be nil")
	}
}

func TestNewApp_Version(t *testing.T) {
	app := NewApp("v1.0.0", nil, "key", "/tmp/config")
	if app.version != "v1.0.0" {
		t.Errorf("Expected version v1.0.0, got %q", app.version)
	}
}

func TestNewApp_RegistryActive(t *testing.T) {
	reg := provider.NewRegistry()
	app := NewApp("test", reg, "key", "/tmp/config")
	if app.activeProvider != "" {
		t.Errorf("Expected empty active provider with empty registry, got %q", app.activeProvider)
	}
}

func TestApp_Init_ReturnsCmd(t *testing.T) {
	reg := provider.NewRegistry()
	reg.Register("openrouter", &mockProvider{})
	app := NewApp("test", reg, "key", "/tmp/config")
	cmd := app.Init()
	if cmd == nil {
		t.Error("Init() should return non-nil command when registry has active provider")
	}
}

func TestApp_Init_NoKey_ReturnsNil(t *testing.T) {
	app := NewApp("test", nil, "", "/tmp/config")
	cmd := app.Init()
	if cmd != nil {
		t.Error("Init() without key should return nil")
	}
}

func TestApp_CtrlC_Quits(t *testing.T) {
	app := NewApp("test", nil, "key", "/tmp/config")
	_, cmd := app.Update(tea.KeyMsg{Type: tea.KeyCtrlC})
	if cmd == nil {
		t.Fatal("Expected non-nil command for ctrl+c")
	}

	quitMsg := cmd()
	if _, ok := quitMsg.(tea.QuitMsg); !ok {
		t.Errorf("Expected tea.QuitMsg, got %T", quitMsg)
	}
}

func TestApp_ScreenTransition(t *testing.T) {
	app := NewApp("test", nil, "key", "/tmp/config")
	newModel, _ := app.Update(AppMsg{Screen: ScreenFirstRun})
	updated := newModel.(*AppState)
	if updated.screen != ScreenFirstRun {
		t.Errorf("Expected ScreenFirstRun, got %d", updated.screen)
	}
}

func TestApp_FirstRunToREPL(t *testing.T) {
	app := NewApp("test", nil, "", "/tmp/config")
	if app.screen != ScreenFirstRun {
		t.Fatalf("Expected ScreenFirstRun initially, got %d", app.screen)
	}

	newModel, _ := app.Update(AppMsg{Screen: ScreenREPL})
	updated := newModel.(*AppState)
	if updated.screen != ScreenREPL {
		t.Errorf("Expected ScreenREPL after AppMsg, got %d", updated.screen)
	}
	if updated.replModel == nil {
		t.Error("Expected replModel to be created after transition")
	}
}

func TestApp_HealthTick_Reschedules(t *testing.T) {
	reg := provider.NewRegistry()
	app := NewApp("test", reg, "key", "/tmp/config")
	newModel, cmd := app.Update(HealthCheckTickMsg{Time: testTime})
	if cmd == nil {
		t.Fatal("Expected non-nil command for health tick")
	}
	_ = newModel
}

func TestApp_View_NotEmpty(t *testing.T) {
	app := NewApp("test", nil, "key", "/tmp/config")
	app.Update(tea.WindowSizeMsg{Width: 80, Height: 24})
	v := app.View()
	if v == "" {
		t.Error("View() should not be empty after resize")
	}
}

func TestApp_TerminalTooSmall(t *testing.T) {
	app := NewApp("test", nil, "key", "/tmp/config")
	app.Update(tea.WindowSizeMsg{Width: 30, Height: 5})
	v := app.View()
	if !strings.Contains(v, "Terminal too small") {
		t.Errorf("Expected 'Terminal too small' message, got %q", v)
	}
	if !strings.Contains(v, "30x5") {
		t.Errorf("Expected dimensions in message, got %q", v)
	}
}

func TestApp_ViewFirstRun(t *testing.T) {
	app := NewApp("test", nil, "", "/tmp/config")
	app.Update(tea.WindowSizeMsg{Width: 80, Height: 24})
	v := app.View()
	if v == "" {
		t.Error("First-run View() should not be empty")
	}
}

func TestApp_ErrorMsg(t *testing.T) {
	app := NewApp("test", nil, "key", "/tmp/config")
	newModel, _ := app.Update(ErrorMsg{Err: testErr})
	updated := newModel.(*AppState)
	if updated.currentOperation == "" {
		t.Error("Error should set currentOperation")
	}
	_ = newModel
}

func TestApp_AppMsgWithHealth(t *testing.T) {
	app := NewApp("test", nil, "key", "/tmp/config")
	healthMsg := &HealthUpdateMsg{Status: "offline"}
	newModel, _ := app.Update(AppMsg{Screen: ScreenREPL, Health: healthMsg})
	updated := newModel.(*AppState)
	if updated.healthStatus.Status != "offline" {
		t.Errorf("Expected health status 'offline', got %q", updated.healthStatus.Status)
	}
}

func TestApp_AppMsgWithProvider(t *testing.T) {
	app := NewApp("test", nil, "key", "/tmp/config")
	providerMsg := &ProviderSwitchMsg{Provider: "zen"}
	newModel, _ := app.Update(AppMsg{Screen: ScreenREPL, Provider: providerMsg})
	updated := newModel.(*AppState)
	if updated.activeProvider != "zen" {
		t.Errorf("Expected activeProvider 'zen', got %q", updated.activeProvider)
	}
}

func TestApp_InitFirstRunNoHealth(t *testing.T) {
	app := NewApp("test", nil, "", "/tmp/config")
	cmd := app.Init()
	if cmd != nil {
		t.Error("First-run Init should return nil")
	}
}

func TestApp_ScreenREPLWithNilRepl(t *testing.T) {
	app := NewApp("test", nil, "key", "/tmp/config")
	app.replModel = nil
	newModel, _ := app.Update(tea.WindowSizeMsg{Width: 80, Height: 24})
	_ = newModel
}

func TestApp_HealthTickNoRegistry(t *testing.T) {
	app := NewApp("test", nil, "key", "/tmp/config")
	app.registry = nil
	newModel, cmd := app.Update(HealthCheckTickMsg{Time: testTime})
	if cmd == nil {
		t.Fatal("Expected non-nil command even without registry")
	}
	_ = newModel
}

func TestApp_InitWithRegistryReturnsCmd(t *testing.T) {
	reg := provider.NewRegistry()
	reg.Register("openrouter", &mockProvider{})
	app := NewApp("test", reg, "key", "/tmp/config")
	cmd := app.Init()
	if cmd == nil {
		t.Error("Init with registry should return health ticker cmd")
	}
}

type mockProvider struct{}

func (m *mockProvider) Name() string {
	return "mock"
}

func (m *mockProvider) FetchModels(ctx context.Context) ([]types.ModelInfo, error) {
	return nil, nil
}

func (m *mockProvider) ChatCompletionStream(ctx context.Context, req provider.ChatRequest) (*types.StreamIterator, error) {
	return nil, nil
}

func (m *mockProvider) EstimateCost(modelID string, usage types.Usage) float64 {
	return 0
}

func (m *mockProvider) HealthCheck(ctx context.Context) types.HealthStatus {
	return types.HealthStatus{Status: "live", LatencyMs: 42}
}

func (m *mockProvider) GetModel(id string) (*types.ModelInfo, error) {
	return nil, nil
}

var testTime = time.Date(2026, 5, 27, 12, 0, 0, 0, time.UTC)
var testErr = testError{}

type testError struct{}

func (e testError) Error() string {
	return "test error"
}
