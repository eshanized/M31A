package config

import (
	"errors"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/BurntSushi/toml"
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

// ── Multi-Layer Config Tests ─────────────────────────────────────────────────

func TestFindProjectConfig(t *testing.T) {
	// Create a temp dir tree with m31a.toml at root
	dir := t.TempDir()

	// Create root/m31a.toml
	rootCfg := filepath.Join(dir, "m31a.toml")
	if err := os.WriteFile(rootCfg, []byte("[ui]\ntheme=\"dark\"\n"), 0644); err != nil {
		t.Fatal(err)
	}

	// Create subdirectories
	sub1 := filepath.Join(dir, "sub1")
	sub2 := filepath.Join(dir, "sub1", "sub2")
	for _, d := range []string{sub1, sub2} {
		if err := os.MkdirAll(d, 0755); err != nil {
			t.Fatal(err)
		}
	}

	// Separate dir for no-config test (no m31a.toml in ancestry)
	noConfigDir := t.TempDir()
	deepEmpty := filepath.Join(noConfigDir, "a", "b", "c")
	if err := os.MkdirAll(deepEmpty, 0755); err != nil {
		t.Fatal(err)
	}

	tests := []struct {
		name string
		cwd  string
		want string
	}{
		{"same dir", dir, rootCfg},
		{"1 level up", sub1, rootCfg},
		{"2 levels up", sub2, rootCfg},
		{"no config", deepEmpty, ""},
	}

	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			got := findProjectConfig(tc.cwd)
			if got != tc.want {
				t.Errorf("findProjectConfig(%q) = %q, want %q", tc.cwd, got, tc.want)
			}
		})
	}
}

func TestProjectConfigOverrides(t *testing.T) {
	dir := t.TempDir()

	// Create "global" config
	globalPath := filepath.Join(dir, "config.toml")
	globalContent := `
[ui]
theme = "dark"

[model]
default = "gpt-4o"
`
	if err := os.WriteFile(globalPath, []byte(globalContent), 0644); err != nil {
		t.Fatal(err)
	}

	// Create project config in subdirectory
	projectDir := filepath.Join(dir, "project")
	if err := os.MkdirAll(projectDir, 0755); err != nil {
		t.Fatal(err)
	}
	projectCfg := filepath.Join(projectDir, "m31a.toml")
	projectContent := `
[model]
default = "claude-3"
`
	if err := os.WriteFile(projectCfg, []byte(projectContent), 0644); err != nil {
		t.Fatal(err)
	}

	// Change to project dir, load global config
	origWd, _ := os.Getwd()
	if err := os.Chdir(projectDir); err != nil {
		t.Fatal(err)
	}
	defer os.Chdir(origWd)

	cfg, err := Load(globalPath)
	if err != nil {
		t.Fatalf("Load failed: %v", err)
	}

	if cfg.Model.Default != "claude-3" {
		t.Errorf("expected 'claude-3' (project override), got %q", cfg.Model.Default)
	}
	if cfg.UI.Theme != "dark" {
		t.Errorf("expected 'dark' (from global, not overridden), got %q", cfg.UI.Theme)
	}
}

func TestMergeConfig(t *testing.T) {
	base := &Config{}
	base.UI.Theme = "dark"
	base.Model.Default = "gpt-4o"
	base.Provider.Default = "openrouter"
	base.Ledger.MaxEntries = 100

	overlay := &Config{}
	overlay.Model.Default = "claude-3-opus"
	overlay.UI.CompactMode = true
	overlay.Ledger.Enabled = true

	mergeConfig(base, overlay)

	if base.Model.Default != "claude-3-opus" {
		t.Errorf("expected 'claude-3-opus', got %q", base.Model.Default)
	}
	if base.UI.Theme != "dark" {
		t.Errorf("expected 'dark' (preserved), got %q", base.UI.Theme)
	}
	if !base.UI.CompactMode {
		t.Error("expected CompactMode true (from overlay)")
	}
	if base.Provider.Default != "openrouter" {
		t.Errorf("expected 'openrouter' (preserved), got %q", base.Provider.Default)
	}
	if !base.Ledger.Enabled {
		t.Error("expected Ledger.Enabled true (from overlay)")
	}
	if base.Ledger.MaxEntries != 100 {
		t.Errorf("expected MaxEntries 100 (preserved), got %d", base.Ledger.MaxEntries)
	}
}

// ── Validation Tests ─────────────────────────────────────────────────────────

func TestValidateConfig_Valid(t *testing.T) {
	cfg := &Config{
		Provider: ProviderConfig{
			Default: "openrouter",
		},
		Model: ModelConfig{
			ContextWarningThreshold: 0.8,
			ArbitrageThreshold:      0.5,
		},
		UI: UIConfig{
			Theme:         "dark",
			MaxIterations: 100,
		},
		Permissions: PermissionsConfig{
			DefaultMode:    "prompt",
			TimeoutSeconds: 300,
		},
		Ledger: LedgerConfig{
			MaxEntries: 50,
		},
	}

	if err := validateConfig(cfg); err != nil {
		t.Errorf("expected nil error for valid config, got: %v", err)
	}
}

func TestValidateConfig_InvalidTheme(t *testing.T) {
	cfg := &Config{
		UI: UIConfig{
			Theme: "neon",
		},
	}

	err := validateConfig(cfg)
	if err == nil {
		t.Fatal("expected error for invalid theme, got nil")
	}
	if !strings.Contains(err.Error(), "ui.theme") {
		t.Errorf("expected error to mention 'ui.theme', got: %v", err)
	}
	if !errors.Is(err, ErrValidation) {
		t.Errorf("expected error to wrap ErrValidation")
	}
}

func TestValidateConfig_InvalidThreshold(t *testing.T) {
	cfg := &Config{
		Model: ModelConfig{
			ContextWarningThreshold: 1.5,
		},
	}

	err := validateConfig(cfg)
	if err == nil {
		t.Fatal("expected error for invalid threshold, got nil")
	}
	if !strings.Contains(err.Error(), "context_warning_threshold") {
		t.Errorf("expected error to mention 'context_warning_threshold', got: %v", err)
	}
}

func TestValidateConfig_InvalidPermissionsMode(t *testing.T) {
	cfg := &Config{
		Permissions: PermissionsConfig{
			DefaultMode: "auto",
		},
	}

	err := validateConfig(cfg)
	if err == nil {
		t.Fatal("expected error for invalid permissions mode, got nil")
	}
	if !strings.Contains(err.Error(), "default_mode") {
		t.Errorf("expected error to mention 'default_mode', got: %v", err)
	}
}

func TestValidateConfig_InvalidRuleAction(t *testing.T) {
	cfg := &Config{
		Permissions: PermissionsConfig{
			Rules: []PermissionRule{
				{Tool: "Bash", Action: "maybe"},
			},
		},
	}

	err := validateConfig(cfg)
	if err == nil {
		t.Fatal("expected error for invalid rule action, got nil")
	}
	if !strings.Contains(err.Error(), "rules[0].action") {
		t.Errorf("expected error to mention 'rules[0].action', got: %v", err)
	}
}

// ── Variable Substitution Tests ──────────────────────────────────────────────

func TestVarSubstitution(t *testing.T) {
	t.Setenv("TEST_MODEL", "claude-opus")

	cfg := &Config{
		Model: ModelConfig{
			Default: "${TEST_MODEL}",
		},
	}

	applyVarSubstitution(cfg)

	if cfg.Model.Default != "claude-opus" {
		t.Errorf("expected 'claude-opus', got %q", cfg.Model.Default)
	}
}

func TestVarSubstitution_UnsetVar(t *testing.T) {
	cfg := &Config{
		Model: ModelConfig{
			Default: "${UNSET_VAR}",
		},
	}

	applyVarSubstitution(cfg)

	if cfg.Model.Default != "" {
		t.Errorf("expected empty string (unset var replaced), got %q", cfg.Model.Default)
	}
}

func TestVarSubstitution_NoVars(t *testing.T) {
	cfg := &Config{
		Model: ModelConfig{
			Default: "gpt-4o",
		},
	}

	applyVarSubstitution(cfg)

	if cfg.Model.Default != "gpt-4o" {
		t.Errorf("expected 'gpt-4o', got %q", cfg.Model.Default)
	}
}

func TestVarSubstitution_APIKey(t *testing.T) {
	t.Setenv("OPENROUTER_KEY", "sk-or-v1-test123")

	cfg := &Config{
		Provider: ProviderConfig{
			OpenRouter: ProviderCredentialConfig{
				APIKey: "${OPENROUTER_KEY}",
			},
		},
	}

	applyVarSubstitution(cfg)

	if cfg.Provider.OpenRouter.APIKey != "sk-or-v1-test123" {
		t.Errorf("expected resolved API key, got %q", cfg.Provider.OpenRouter.APIKey)
	}
}

// ── Env Override Test ────────────────────────────────────────────────────────

func TestEnvVarOverrides(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "config.toml")

	tomlContent := `
[ui]
theme = "dark"

[model]
default = "claude-sonnet"
`
	if err := os.WriteFile(path, []byte(tomlContent), 0644); err != nil {
		t.Fatal(err)
	}

	t.Setenv("M31A_THEME", "light")
	t.Setenv("M31A_DEFAULT_MODEL", "custom-model")

	cfg, err := Load(path)
	if err != nil {
		t.Fatalf("Load failed: %v", err)
	}

	if cfg.UI.Theme != "light" {
		t.Errorf("expected 'light' (env override), got %q", cfg.UI.Theme)
	}
	if cfg.Model.Default != "custom-model" {
		t.Errorf("expected 'custom-model' (env override), got %q", cfg.Model.Default)
	}
}

// ── PermissionsAgentConfig Test ──────────────────────────────────────────────

func TestPermissionsAgentConfig(t *testing.T) {
	cfg := &Config{
		Permissions: PermissionsConfig{
			Agents: map[string]PermissionsAgentConfig{
				"build": {
					DefaultAction: "allow",
					Rules: []PermissionRule{
						{Tool: "Bash", Action: "ask"},
					},
				},
			},
		},
	}

	// Marshal to TOML and back
	data, err := toml.Marshal(cfg)
	if err != nil {
		t.Fatalf("Marshal failed: %v", err)
	}

	var reloaded Config
	if _, err := toml.Decode(string(data), &reloaded); err != nil {
		t.Fatalf("Decode failed: %v", err)
	}

	if reloaded.Permissions.Agents == nil {
		t.Fatal("expected Agents to be non-nil after round-trip")
	}
	buildAgent, ok := reloaded.Permissions.Agents["build"]
	if !ok {
		t.Fatal("expected 'build' agent in reloaded config")
	}
	if buildAgent.DefaultAction != "allow" {
		t.Errorf("expected 'allow', got %q", buildAgent.DefaultAction)
	}
	if len(buildAgent.Rules) != 1 {
		t.Fatalf("expected 1 rule, got %d", len(buildAgent.Rules))
	}
	if buildAgent.Rules[0].Tool != "Bash" || buildAgent.Rules[0].Action != "ask" {
		t.Errorf("expected Tool=Bash Action=ask, got Tool=%q Action=%q", buildAgent.Rules[0].Tool, buildAgent.Rules[0].Action)
	}
}

func TestDefaultConfig_SidebarWidthThresholdIs120(t *testing.T) {
	t.Parallel()
	c := DefaultConfig()
	if c.UI.SidebarWidthThreshold != 120 {
		t.Errorf("expected SidebarWidthThreshold=120, got %d", c.UI.SidebarWidthThreshold)
	}
}
