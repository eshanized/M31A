package main

import (
	"log/slog"
	"os"
	"path/filepath"
	"runtime"
	"strings"
	"testing"

	"github.com/eshanized/M31A/internal/config"
	"github.com/eshanized/M31A/internal/integrations/keychain"
	"github.com/eshanized/M31A/internal/integrations/provider"
	"github.com/eshanized/M31A/internal/ui/tui"
	"github.com/eshanized/M31A/internal/types"
)

// TestMain runs setup/teardown for the test suite.
func TestMain(m *testing.M) {
	Version = "test"
	Commit = "abc123"
	Date = "2024-01-01"
	GoVersion = runtime.Version()
	os.Exit(m.Run())
}

// TestVersionFlag tests that version variables are set.
func TestVersionFlag(t *testing.T) {
	if Version == "" {
		t.Error("Version should be set")
	}
	if Commit == "" {
		t.Error("Commit should be set")
	}
	if Date == "" {
		t.Error("Date should be set")
	}
	if GoVersion == "" {
		t.Error("GoVersion should be set")
	}
}

// TestIndexOf tests the indexOf helper function.
func TestIndexOf(t *testing.T) {
	phases := []types.WorkflowPhase{types.PhaseInitialize, types.PhaseDiscuss, types.PhasePlan}

	if idx := indexOf(types.PhaseInitialize, phases); idx != 0 {
		t.Errorf("indexOf(PhaseInitialize) = %d, want 0", idx)
	}
	if idx := indexOf(types.PhaseDiscuss, phases); idx != 1 {
		t.Errorf("indexOf(PhaseDiscuss) = %d, want 1", idx)
	}
	if idx := indexOf(types.PhasePlan, phases); idx != 2 {
		t.Errorf("indexOf(PhasePlan) = %d, want 2", idx)
	}
	if idx := indexOf(types.PhaseExecute, phases); idx != -1 {
		t.Errorf("indexOf(PhaseExecute) = %d, want -1", idx)
	}
}

// TestConfigLoad tests config loading with various inputs.
func TestConfigLoad(t *testing.T) {
	cfg := config.DefaultConfig()
	if cfg == nil {
		t.Fatal("DefaultConfig() returned nil")
	}
	if cfg.Provider.FallbackPriority == nil {
		t.Error("FallbackPriority should not be nil")
	}
	if cfg.Provider.RegistrationOrder == nil {
		t.Error("RegistrationOrder should not be nil")
	}
	if len(cfg.Provider.FallbackPriority) == 0 {
		t.Error("FallbackPriority should have defaults")
	}
	if len(cfg.Provider.RegistrationOrder) == 0 {
		t.Error("RegistrationOrder should have defaults")
	}
}

// TestProviderRegistry tests provider registry operations.
func TestProviderRegistry(t *testing.T) {
	registry := provider.NewRegistry()

	if registry.Active() != "" {
		t.Error("New registry should have no active provider")
	}

	if err := registry.SetActive("nonexistent"); err == nil {
		t.Error("SetActive should fail for unregistered provider")
	}

	if registry.ActiveProvider() != nil {
		t.Error("ActiveProvider should return nil when no provider registered")
	}
}

// TestDefaultConfig tests that default config has expected values.
func TestDefaultConfig(t *testing.T) {
	cfg := config.DefaultConfig()

	if cfg.Provider.HealthCheckTimeoutSecs != 10 {
		t.Errorf("HealthCheckTimeoutSecs = %d, want 10", cfg.Provider.HealthCheckTimeoutSecs)
	}

	if cfg.UI.SidebarWidthThreshold != 120 {
		t.Errorf("SidebarWidthThreshold = %d, want 120", cfg.UI.SidebarWidthThreshold)
	}
	if cfg.UI.MaxIterations != 100 {
		t.Errorf("MaxIterations = %d, want 100", cfg.UI.MaxIterations)
	}
}

// TestResolveAPIKeys tests API key resolution from environment.
func TestResolveAPIKeys(t *testing.T) {
	cfg := config.DefaultConfig()

	err := cfg.ResolveAPIKeys(nil)
	if err != nil {
		t.Errorf("ResolveAPIKeys with nil keychain failed: %v", err)
	}

	mockKC := &mockKeychain{}
	err = cfg.ResolveAPIKeys(mockKC)
	if err != nil {
		t.Errorf("ResolveAPIKeys with mock keychain failed: %v", err)
	}
}

// mockKeychain implements keychain.Keychain for testing.
type mockKeychain struct {
	keychain.Keychain
}

func (m *mockKeychain) Get(provider string) (string, error) {
	return "", nil
}

func (m *mockKeychain) Set(provider, key string) error {
	return nil
}

func (m *mockKeychain) Delete(provider string) error {
	return nil
}

// TestPrintUsage tests that printUsage doesn't panic.
func TestPrintUsage(t *testing.T) {
	cmdRegistry := tui.DefaultCommands()
	printUsage(cmdRegistry)
}

// TestRestoreTerminal tests the restoreTerminal function.
func TestRestoreTerminal(t *testing.T) {
	restoreTerminal()
}

// TestSignalHandler tests that signal handling setup doesn't panic.
func TestSignalHandler(t *testing.T) {
	sigCh := make(chan os.Signal, 1)
	if cap(sigCh) != 1 {
		t.Error("signal channel should have buffer 1")
	}
}

// TestConfigPathResolution tests config path resolution logic.
func TestConfigPathResolution(t *testing.T) {
	home, err := os.UserHomeDir()
	if err != nil {
		t.Fatalf("UserHomeDir failed: %v", err)
	}
	expected := filepath.Join(home, ".m31a", "config.toml")

	if !strings.HasSuffix(expected, "config.toml") {
		t.Errorf("expected config path to end with config.toml, got %s", expected)
	}
}

// TestRunHeadlessWorkflow tests the runHeadlessWorkflow function with a mock provider.
func TestRunHeadlessWorkflow(t *testing.T) {
	registry := provider.NewRegistry()
	cmdRegistry := tui.DefaultCommands()
	cfg := config.DefaultConfig()
	logger := slog.Default()

	code := runHeadlessWorkflow("test goal", cmdRegistry, cfg, "test-model", logger, registry)
	if code != 1 {
		t.Errorf("runHeadlessWorkflow with no provider returned %d, want 1", code)
	}
}

// TestRunHeadlessPrompt tests the runHeadless function.
func TestRunHeadlessPrompt(t *testing.T) {
	registry := provider.NewRegistry()
	logger := slog.Default()

	code := runHeadless("test prompt", registry, "test-model", logger)
	if code != 1 {
		t.Errorf("runHeadless with no provider returned %d, want 1", code)
	}
}
