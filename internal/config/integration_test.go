package config

import (
	"os"
	"path/filepath"
	"testing"

	"github.com/eshanized/M31A/internal/testutil"
)

func TestIntegration_ResolveAPIKeys_FromEnv(t *testing.T) {
	testutil.LoadTestDotEnv(t)

	orKey := testutil.RequireAnyAPIKey(t, "M31A_OPENROUTER_API_KEY", "OPENROUTER_API_KEY")
	zenKey := os.Getenv("M31A_ZEN_API_KEY")
	if zenKey == "" {
		zenKey = os.Getenv("ZEN_API_KEY")
	}

	cfg := &Config{}
	kc := newMockKeychain()

	if err := cfg.ResolveAPIKeys(kc); err != nil {
		t.Fatalf("ResolveAPIKeys failed: %v", err)
	}

	if cfg.Provider.OpenRouter.APIKey != orKey {
		t.Errorf("OpenRouter key mismatch: got %q, want %q", cfg.Provider.OpenRouter.APIKey, orKey)
	}
	if zenKey != "" && cfg.Provider.Zen.APIKey != zenKey {
		t.Errorf("Zen key mismatch: got %q, want %q", cfg.Provider.Zen.APIKey, zenKey)
	}
}

func TestIntegration_LoadConfigWithAPIKey(t *testing.T) {
	testutil.LoadTestDotEnv(t)

	apiKey := testutil.RequireAnyAPIKey(t, "OPENROUTER_API_KEY", "M31A_OPENROUTER_API_KEY")
	if apiKey == "" {
		return
	}

	dir := t.TempDir()
	path := filepath.Join(dir, "config.toml")

	tomlContent := `
[provider]
default = "openrouter"
[provider.openrouter]
api_key = "` + apiKey + `"
`
	if err := os.WriteFile(path, []byte(tomlContent), 0644); err != nil {
		t.Fatal(err)
	}

	cfg, err := Load(path)
	if err != nil {
		t.Fatalf("Load failed: %v", err)
	}

	if cfg.Provider.OpenRouter.APIKey != apiKey {
		t.Errorf("expected API key to match, got %q", cfg.Provider.OpenRouter.APIKey)
	}
	if cfg.Provider.Default != "openrouter" {
		t.Errorf("expected provider 'openrouter', got %q", cfg.Provider.Default)
	}
}

func TestIntegration_DotEnvLoadsAPIKeys(t *testing.T) {
	testutil.LoadTestDotEnv(t)

	// Verify that LoadTestDotEnv populated the env vars
	if v := os.Getenv("OPENROUTER_API_KEY"); v == "" {
		t.Log("OPENROUTER_API_KEY not set in .env.test — skipping verification")
		return
	}

	cfg := &Config{}
	kc := newMockKeychain()

	if err := cfg.ResolveAPIKeys(kc); err != nil {
		t.Fatalf("ResolveAPIKeys failed: %v", err)
	}

	if cfg.Provider.OpenRouter.APIKey == "" {
		t.Error("expected OpenRouter API key to be resolved from env")
	}
}
