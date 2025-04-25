package config

import (
	"errors"
	"os"
	"path/filepath"
	"testing"
)

// mockKeychain implements the keychain.Keychain interface for testing.
type mockKeychain struct {
	store map[string]string
}

func newMockKeychain() *mockKeychain {
	return &mockKeychain{store: make(map[string]string)}
}

func (m *mockKeychain) Get(service string) (string, error) {
	if v, ok := m.store[service]; ok {
		return v, nil
	}
	return "", errors.New("not found")
}

func (m *mockKeychain) Set(service, value string) error {
	m.store[service] = value
	return nil
}

func (m *mockKeychain) Delete(service string) error {
	delete(m.store, service)
	return nil
}

func TestConfig_DefaultConfig(t *testing.T) {
	cfg := DefaultConfig()
	if cfg == nil {
		t.Fatal("DefaultConfig() returned nil")
	}

	// Verify zero values
	if cfg.Provider.Default != "" {
		t.Errorf("expected empty Provider.Default, got %q", cfg.Provider.Default)
	}
	if cfg.UI.Theme != "" {
		t.Errorf("expected empty UI.Theme, got %q", cfg.UI.Theme)
	}
	if cfg.Model.Default != "" {
		t.Errorf("expected empty Model.Default, got %q", cfg.Model.Default)
	}
	if cfg.Permissions.DefaultMode != "" {
		t.Errorf("expected empty Permissions.DefaultMode, got %q", cfg.Permissions.DefaultMode)
	}
}

func TestConfig_LoadTOMLParse(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "config.toml")

	tomlContent := `
[provider]
default = "openrouter"
auto_fallback = true
[provider.openrouter]
api_key = "or-test-key"

[model]
default = "gpt-4o"
context_warning_threshold = 0.85

[ui]
theme = "dark"
compact_mode = true
`
	if err := os.WriteFile(path, []byte(tomlContent), 0644); err != nil {
		t.Fatal(err)
	}

	cfg, err := Load(path)
	if err != nil {
		t.Fatalf("Load failed: %v", err)
	}

	if cfg.Provider.Default != "openrouter" {
		t.Errorf("expected 'openrouter', got %q", cfg.Provider.Default)
	}
	if !cfg.Provider.AutoFallback {
		t.Error("expected AutoFallback true")
	}
	if cfg.Provider.OpenRouter.APIKey != "or-test-key" {
		t.Errorf("expected 'or-test-key', got %q", cfg.Provider.OpenRouter.APIKey)
	}
	if cfg.Model.Default != "gpt-4o" {
		t.Errorf("expected 'gpt-4o', got %q", cfg.Model.Default)
	}
	if cfg.Model.ContextWarningThreshold != 0.85 {
		t.Errorf("expected 0.85, got %f", cfg.Model.ContextWarningThreshold)
	}
	if cfg.UI.Theme != "dark" {
		t.Errorf("expected 'dark', got %q", cfg.UI.Theme)
	}
	if !cfg.UI.CompactMode {
		t.Error("expected CompactMode true")
	}
}

func TestConfig_LoadMissingFile(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "nonexistent.toml")

	cfg, err := Load(path)
	if err != nil {
		t.Fatalf("Load on missing file should not error: %v", err)
	}
	if cfg == nil {
		t.Fatal("Load on missing file returned nil config")
	}

	// Should be default config (zero values)
	if cfg.Provider.Default != "" {
		t.Errorf("expected empty provider on default config, got %q", cfg.Provider.Default)
	}
}

func TestConfig_LoadEnvThemeOverride(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "config.toml")

	tomlContent := `
[ui]
theme = "dark"
`
	if err := os.WriteFile(path, []byte(tomlContent), 0644); err != nil {
		t.Fatal(err)
	}

	// Set env var override
	t.Setenv("M31A_THEME", "light")

	cfg, err := Load(path)
	if err != nil {
		t.Fatalf("Load failed: %v", err)
	}

	if cfg.UI.Theme != "light" {
		t.Errorf("expected 'light' (env override), got %q", cfg.UI.Theme)
	}
}

func TestConfig_LoadEnvModelOverride(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "config.toml")

	tomlContent := `
[model]
default = "claude-3-sonnet"
`
	if err := os.WriteFile(path, []byte(tomlContent), 0644); err != nil {
		t.Fatal(err)
	}

	t.Setenv("M31A_DEFAULT_MODEL", "gpt-4o")

	cfg, err := Load(path)
	if err != nil {
		t.Fatalf("Load failed: %v", err)
	}

	if cfg.Model.Default != "gpt-4o" {
		t.Errorf("expected 'gpt-4o' (env override), got %q", cfg.Model.Default)
	}
}

func TestConfig_LoadConfigPathOverride(t *testing.T) {
	dir := t.TempDir()

	// Create two configs
	defaultPath := filepath.Join(dir, "default.toml")
	overridePath := filepath.Join(dir, "override.toml")

	defaultContent := `
[ui]
theme = "dark"
`
	overrideContent := `
[ui]
theme = "light"
`
	if err := os.WriteFile(defaultPath, []byte(defaultContent), 0644); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(overridePath, []byte(overrideContent), 0644); err != nil {
		t.Fatal(err)
	}

	// M31A_CONFIG should override the path argument
	t.Setenv("M31A_CONFIG", overridePath)

	cfg, err := Load(defaultPath)
	if err != nil {
		t.Fatalf("Load failed: %v", err)
	}

	// Should load from overridePath, not defaultPath
	if cfg.UI.Theme != "light" {
		t.Errorf("expected 'light' from M31A_CONFIG override, got %q", cfg.UI.Theme)
	}
}

func TestConfig_SaveAtomic(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "config.toml")

	cfg := &Config{}
	cfg.Provider.Default = "openrouter"
	cfg.UI.Theme = "dark"
	cfg.Model.Default = "gpt-4o"

	if err := cfg.Save(path); err != nil {
		t.Fatalf("Save failed: %v", err)
	}

	// Verify file exists and is valid TOML
	if _, err := os.Stat(path); os.IsNotExist(err) {
		t.Fatal("Save did not create config file")
	}

	// Read back and verify
	loaded, err := Load(path)
	if err != nil {
		t.Fatalf("Load of saved config failed: %v", err)
	}

	if loaded.Provider.Default != "openrouter" {
		t.Errorf("expected 'openrouter', got %q", loaded.Provider.Default)
	}
	if loaded.UI.Theme != "dark" {
		t.Errorf("expected 'dark', got %q", loaded.UI.Theme)
	}
	if loaded.Model.Default != "gpt-4o" {
		t.Errorf("expected 'gpt-4o', got %q", loaded.Model.Default)
	}
}

func TestConfig_SaveEnvOverride(t *testing.T) {
	dir := t.TempDir()
	originalPath := filepath.Join(dir, "original.toml")
	overridePath := filepath.Join(dir, "override.toml")

	cfg := &Config{}
	cfg.UI.Theme = "dark"

	// M31A_CONFIG env var overrides the path argument
	t.Setenv("M31A_CONFIG", overridePath)

	if err := cfg.Save(originalPath); err != nil {
		t.Fatalf("Save failed: %v", err)
	}

	// File should exist at overridePath, not originalPath
	if _, err := os.Stat(overridePath); os.IsNotExist(err) {
		t.Error("Save did not write to M31A_CONFIG path")
	}
	if _, err := os.Stat(originalPath); !os.IsNotExist(err) {
		t.Error("Save should not have written to the original path argument")
	}
}

func TestConfig_KeyResolutionEnvVar(t *testing.T) {
	cfg := &Config{}
	kc := newMockKeychain()

	// Set env var
	t.Setenv("M31A_OPENROUTER_API_KEY", "env-or-key")
	t.Setenv("M31A_ZEN_API_KEY", "env-zen-key")

	if err := cfg.ResolveAPIKeys(kc); err != nil {
		t.Fatalf("ResolveAPIKeys failed: %v", err)
	}

	if cfg.Provider.OpenRouter.APIKey != "env-or-key" {
		t.Errorf("expected 'env-or-key', got %q", cfg.Provider.OpenRouter.APIKey)
	}
	if cfg.Provider.Zen.APIKey != "env-zen-key" {
		t.Errorf("expected 'env-zen-key', got %q", cfg.Provider.Zen.APIKey)
	}
}

func TestConfig_KeyResolutionKeychain(t *testing.T) {
	cfg := &Config{}
	kc := newMockKeychain()
	kc.store["openrouter"] = "kc-or-key"
	kc.store["zen"] = "kc-zen-key"

	// No env vars set — should use keychain

	if err := cfg.ResolveAPIKeys(kc); err != nil {
		t.Fatalf("ResolveAPIKeys failed: %v", err)
	}

	if cfg.Provider.OpenRouter.APIKey != "kc-or-key" {
		t.Errorf("expected 'kc-or-key', got %q", cfg.Provider.OpenRouter.APIKey)
	}
	if cfg.Provider.Zen.APIKey != "kc-zen-key" {
		t.Errorf("expected 'kc-zen-key', got %q", cfg.Provider.Zen.APIKey)
	}
}

func TestConfig_KeyResolutionConfigFile(t *testing.T) {
	cfg := &Config{}
	cfg.Provider.OpenRouter.APIKey = "cfg-or-key"
	cfg.Provider.Zen.APIKey = "cfg-zen-key"

	// Empty keychain (no keys stored)
	kc := newMockKeychain()

	// No env vars — should fall back to config file

	if err := cfg.ResolveAPIKeys(kc); err != nil {
		t.Fatalf("ResolveAPIKeys failed: %v", err)
	}

	if cfg.Provider.OpenRouter.APIKey != "cfg-or-key" {
		t.Errorf("expected 'cfg-or-key', got %q", cfg.Provider.OpenRouter.APIKey)
	}
	if cfg.Provider.Zen.APIKey != "cfg-zen-key" {
		t.Errorf("expected 'cfg-zen-key', got %q", cfg.Provider.Zen.APIKey)
	}
}

func TestConfig_KeyResolutionOrder(t *testing.T) {
	cfg := &Config{}
	cfg.Provider.OpenRouter.APIKey = "cfg-or-key"

	kc := newMockKeychain()
	kc.store["openrouter"] = "kc-or-key"

	// Set env var (should win over keychain and config file)
	t.Setenv("M31A_OPENROUTER_API_KEY", "env-or-key")

	if err := cfg.ResolveAPIKeys(kc); err != nil {
		t.Fatalf("ResolveAPIKeys failed: %v", err)
	}

	// Env var should win
	if cfg.Provider.OpenRouter.APIKey != "env-or-key" {
		t.Errorf("expected 'env-or-key' (env var wins), got %q", cfg.Provider.OpenRouter.APIKey)
	}
}

func TestConfig_KeyResolutionNoKeys(t *testing.T) {
	cfg := &Config{}

	// No env vars, no keychain, no config file keys
	if err := cfg.ResolveAPIKeys(nil); err != nil {
		t.Fatalf("ResolveAPIKeys with nil keychain should not error: %v", err)
	}

	if cfg.Provider.OpenRouter.APIKey != "" {
		t.Errorf("expected empty, got %q", cfg.Provider.OpenRouter.APIKey)
	}
	if cfg.Provider.Zen.APIKey != "" {
		t.Errorf("expected empty, got %q", cfg.Provider.Zen.APIKey)
	}
}

func TestConfig_SaveCreatesParentDir(t *testing.T) {
	dir := t.TempDir()
	// Save to a nested directory that doesn't exist yet
	path := filepath.Join(dir, "subdir", "nested", "config.toml")

	cfg := &Config{}
	cfg.UI.Theme = "dark"

	if err := cfg.Save(path); err != nil {
		t.Fatalf("Save to nested dir failed: %v", err)
	}

	if _, err := os.Stat(path); os.IsNotExist(err) {
		t.Fatal("Save did not create file in nested directory")
	}

	// Read back
	loaded, err := Load(path)
	if err != nil {
		t.Fatalf("Load failed: %v", err)
	}
	if loaded.UI.Theme != "dark" {
		t.Errorf("expected 'dark', got %q", loaded.UI.Theme)
	}
}
