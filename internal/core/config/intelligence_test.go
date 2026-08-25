package config

import (
	"os"
	"path/filepath"
	"testing"

	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

// writeAndLoad writes tomlContent to a temp config file and loads it.
func writeAndLoad(t *testing.T, tomlContent string) *Config {
	t.Helper()
	dir := t.TempDir()
	path := filepath.Join(dir, "config.toml")
	if err := os.WriteFile(path, []byte(tomlContent), 0644); err != nil {
		t.Fatal(err)
	}
	cfg, err := Load(path)
	require.NoError(t, err)
	return cfg
}

func TestIntelligenceLoadSection(t *testing.T) {
	cfg := writeAndLoad(t, `
[intelligence]
repro_command = "go test ./..."
bisect_max_commits = 25

[intelligence.deps_risk]
stale_months = 9
young_months = 3
license_allowlist = ["MIT", "Apache-2.0"]

[intelligence.deps_policy]
require_approval_high_risk = false
`)

	assert.Equal(t, "go test ./...", cfg.Intelligence.ReproCommand)
	assert.Equal(t, 25, cfg.Intelligence.BisectMaxCommits)
	assert.Equal(t, 9, cfg.Intelligence.DepsRisk.StaleMonths)
	assert.Equal(t, 3, cfg.Intelligence.DepsRisk.YoungMonths)
	assert.Equal(t, []string{"MIT", "Apache-2.0"}, cfg.Intelligence.DepsRisk.LicenseAllowlist)
	assert.False(t, cfg.Intelligence.DepsPolicy.RequireApprovalHighRisk)
}

func TestIntelligenceDefaultsNoSection(t *testing.T) {
	cfg := writeAndLoad(t, `
[ui]
theme = "dark"
`)

	// Pitfall 10: absence normalizes to safe defaults after layered merge.
	assert.Equal(t, 50, cfg.Intelligence.BisectMaxCommits)
	assert.Equal(t, 12, cfg.Intelligence.DepsRisk.StaleMonths)
	assert.Equal(t, 6, cfg.Intelligence.DepsRisk.YoungMonths)
	assert.Equal(t, DefaultLicenseAllowlist(), cfg.Intelligence.DepsRisk.LicenseAllowlist)
	assert.True(t, cfg.Intelligence.DepsPolicy.RequireApprovalHighRisk)
}

func TestIntelligenceZeroNormalizes(t *testing.T) {
	cfg := writeAndLoad(t, `
[intelligence]
bisect_max_commits = 0

[intelligence.deps_risk]
stale_months = 0
young_months = 0
`)

	// Absence and explicit zero are indistinguishable post-merge.
	assert.Equal(t, 50, cfg.Intelligence.BisectMaxCommits)
	assert.Equal(t, 12, cfg.Intelligence.DepsRisk.StaleMonths)
	assert.Equal(t, 6, cfg.Intelligence.DepsRisk.YoungMonths)
}

func TestValidateConfig_IntelligenceRejectsInvalidRanges(t *testing.T) {
	tests := []struct {
		name   string
		mutate func(*Config)
		field  string
	}{
		{
			name:   "negative bisect_max_commits",
			mutate: func(c *Config) { c.Intelligence.BisectMaxCommits = -3 },
			field:  "intelligence.bisect_max_commits",
		},
		{
			name:   "zero bisect_max_commits",
			mutate: func(c *Config) { c.Intelligence.BisectMaxCommits = 0 },
			field:  "intelligence.bisect_max_commits",
		},
		{
			name:   "zero stale_months",
			mutate: func(c *Config) { c.Intelligence.DepsRisk.StaleMonths = 0 },
			field:  "intelligence.deps_risk.stale_months",
		},
		{
			name:   "zero young_months",
			mutate: func(c *Config) { c.Intelligence.DepsRisk.YoungMonths = 0 },
			field:  "intelligence.deps_risk.young_months",
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			cfg := DefaultConfig()
			tt.mutate(cfg)

			err := validateConfig(cfg)
			require.Error(t, err)
			assert.Contains(t, err.Error(), tt.field)
		})
	}
}

func TestValidateConfig_IntelligenceDefaultsPass(t *testing.T) {
	cfg := DefaultConfig()
	normalizeIntelligence(&cfg.Intelligence)
	assert.NoError(t, validateConfig(cfg))
}

func TestIntelligenceMergeWorkspaceOverridesGlobal(t *testing.T) {
	dir := t.TempDir()

	globalPath := filepath.Join(dir, "config.toml")
	globalContent := `
[intelligence]
repro_command = "make test"
bisect_max_commits = 30
`
	if err := os.WriteFile(globalPath, []byte(globalContent), 0644); err != nil {
		t.Fatal(err)
	}

	workspaceDir := filepath.Join(dir, "workspace")
	if err := os.MkdirAll(filepath.Join(workspaceDir, ".m31a"), 0755); err != nil {
		t.Fatal(err)
	}
	workspaceCfg := filepath.Join(workspaceDir, ".m31a", "workspace.toml")
	workspaceContent := `
[intelligence]
repro_command = "go test ./..."
`
	if err := os.WriteFile(workspaceCfg, []byte(workspaceContent), 0644); err != nil {
		t.Fatal(err)
	}

	origWd, _ := os.Getwd()
	require.NoError(t, os.Chdir(workspaceDir))
	defer os.Chdir(origWd)

	cfg, err := Load(globalPath)
	require.NoError(t, err)

	// Workspace-level repro_command overrides global.
	assert.Equal(t, "go test ./...", cfg.Intelligence.ReproCommand)
	// Global value preserved where workspace did not override.
	assert.Equal(t, 30, cfg.Intelligence.BisectMaxCommits)
}

func TestNormalizeIntelligence_NegativeValuesClampToDefaults(t *testing.T) {
	cfg := &IntelligenceConfig{
		BisectMaxCommits: -5,
		DepsRisk: DepsRiskConfig{
			StaleMonths: -1,
			YoungMonths: -2,
		},
	}
	normalizeIntelligence(cfg)

	assert.Equal(t, 50, cfg.BisectMaxCommits)
	assert.Equal(t, 12, cfg.DepsRisk.StaleMonths)
	assert.Equal(t, 6, cfg.DepsRisk.YoungMonths)
	assert.NotNil(t, cfg.DepsRisk.LicenseAllowlist)
}
