package config

import (
	"testing"
)

func TestConfig_DefaultZeroValues(t *testing.T) {
	t.Parallel()
	c := Config{}
	if c.Provider.Default != "" {
		t.Errorf("expected empty provider default, got %q", c.Provider.Default)
	}
	if c.Model.Default != "" {
		t.Errorf("expected empty model default, got %q", c.Model.Default)
	}
	if c.UI.Theme != "" {
		t.Errorf("expected empty theme, got %q", c.UI.Theme)
	}
}

func TestProviderConfig_AllFields(t *testing.T) {
	t.Parallel()
	pc := ProviderConfig{
		Default:      "openrouter",
		AutoFallback: true,
		OpenRouter: ProviderCredentialConfig{
			APIKey: "test-key",
		},
		OpenRouterBaseURL: "https://custom.api.com",
	}
	if pc.Default != "openrouter" {
		t.Errorf("expected 'openrouter', got %q", pc.Default)
	}
	if !pc.AutoFallback {
		t.Error("expected AutoFallback to be true")
	}
	if pc.OpenRouter.APIKey != "test-key" {
		t.Errorf("expected 'test-key', got %q", pc.OpenRouter.APIKey)
	}
	if pc.OpenRouterBaseURL != "https://custom.api.com" {
		t.Errorf("expected custom URL, got %q", pc.OpenRouterBaseURL)
	}
}

func TestModelConfig_AllFields(t *testing.T) {
	t.Parallel()
	mc := ModelConfig{
		Default:                 "gpt-4",
		ContextWarningThreshold: 0.8,
		AutoArbitrage:           true,
		ArbitrageThreshold:      0.1,
		DefaultContextLength:    128000,
	}
	if mc.Default != "gpt-4" {
		t.Errorf("expected 'gpt-4', got %q", mc.Default)
	}
	if mc.ContextWarningThreshold != 0.8 {
		t.Errorf("expected 0.8, got %f", mc.ContextWarningThreshold)
	}
}

func TestUIConfig_AllFields(t *testing.T) {
	t.Parallel()
	ui := UIConfig{
		Theme:            "dark",
		CompactMode:      true,
		ShowCostEstimate: true,
		MaxIterations:    50,
	}
	if ui.Theme != "dark" {
		t.Errorf("expected 'dark', got %q", ui.Theme)
	}
	if !ui.CompactMode {
		t.Error("expected CompactMode to be true")
	}
}

func TestPermissionsConfig_AllFields(t *testing.T) {
	t.Parallel()
	pc := PermissionsConfig{
		DefaultMode:    "ask",
		TimeoutSeconds: 300,
	}
	if pc.DefaultMode != "ask" {
		t.Errorf("expected 'ask', got %q", pc.DefaultMode)
	}
	if pc.TimeoutSeconds != 300 {
		t.Errorf("expected 300, got %d", pc.TimeoutSeconds)
	}
}

func TestFeaturesConfig_AllFields(t *testing.T) {
	t.Parallel()
	fc := FeaturesConfig{
		AutoBackup:      true,
		ResumeOnStartup: true,
		WorkflowMode:    "full",
	}
	if !fc.AutoBackup {
		t.Error("expected AutoBackup to be true")
	}
	if fc.WorkflowMode != "full" {
		t.Errorf("expected 'full', got %q", fc.WorkflowMode)
	}
}

func TestGitConfig_AllFields(t *testing.T) {
	t.Parallel()
	gc := GitConfig{
		CommitPrefix: "feat:",
		UserName:     "test",
		UserEmail:    "test@test.com",
	}
	if gc.CommitPrefix != "feat:" {
		t.Errorf("expected 'feat:', got %q", gc.CommitPrefix)
	}
	if gc.UserName != "test" {
		t.Errorf("expected 'test', got %q", gc.UserName)
	}
}

func TestVerifyConfig_AllFields(t *testing.T) {
	t.Parallel()
	vc := VerifyConfig{
		BuildCommand: "make build",
		TestCommand:  "make test",
	}
	if vc.BuildCommand != "make build" {
		t.Errorf("expected 'make build', got %q", vc.BuildCommand)
	}
	if vc.TestCommand != "make test" {
		t.Errorf("expected 'make test', got %q", vc.TestCommand)
	}
}

func TestToolsConfig_AllFields(t *testing.T) {
	t.Parallel()
	tc := ToolsConfig{
		MaxGlobResults:    100,
		MaxGrepResults:    100,
		BashKillGraceSecs: 5,
		MaxBackupsPerFile: 10,
	}
	if tc.MaxGlobResults != 100 {
		t.Errorf("expected 100, got %d", tc.MaxGlobResults)
	}
	if tc.BashKillGraceSecs != 5 {
		t.Errorf("expected 5, got %d", tc.BashKillGraceSecs)
	}
}

func TestAgentsConfig_AllFields(t *testing.T) {
	t.Parallel()
	ac := AgentsConfig{
		Default: "gpt-4",
		Plan:    "gpt-4",
		Execute: "gpt-4",
	}
	if ac.Default != "gpt-4" {
		t.Errorf("expected 'gpt-4', got %q", ac.Default)
	}
	if ac.Plan != "gpt-4" {
		t.Errorf("expected 'gpt-4', got %q", ac.Plan)
	}
}

func TestLedgerConfig_AllFields(t *testing.T) {
	t.Parallel()
	lc := LedgerConfig{
		Enabled:    true,
		MaxEntries: 100,
	}
	if !lc.Enabled {
		t.Error("expected Enabled to be true")
	}
	if lc.MaxEntries != 100 {
		t.Errorf("expected 100, got %d", lc.MaxEntries)
	}
}
