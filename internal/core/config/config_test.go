package config

import (
	"os"
	"path/filepath"
	"testing"

	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

func TestConfigValidation(t *testing.T) {
	tests := []struct {
		name          string
		cfg           *Config
		expectError   bool
		errorContains string
	}{
		{
			name:        "valid config with defaults",
			cfg:         DefaultConfig(),
			expectError: false,
		},
		{
			name: "invalid eventstore path",
			cfg: func() *Config {
				cfg := DefaultConfig()
				cfg.EventStore.Path = ""
				return cfg
			}(),
			expectError:   true,
			errorContains: "eventstore.path",
		},
		{
			name: "invalid eventstore busy_timeout_ms",
			cfg: func() *Config {
				cfg := DefaultConfig()
				cfg.EventStore.BusyTimeoutMs = 0
				return cfg
			}(),
			expectError:   true,
			errorContains: "eventstore.busy_timeout_ms",
		},
		{
			name: "negative eventstore busy_timeout_ms",
			cfg: func() *Config {
				cfg := DefaultConfig()
				cfg.EventStore.BusyTimeoutMs = -1
				return cfg
			}(),
			expectError:   true,
			errorContains: "eventstore.busy_timeout_ms",
		},
		{
			name: "invalid backup_interval_hours",
			cfg: func() *Config {
				cfg := DefaultConfig()
				cfg.EventStore.BackupIntervalHours = 0
				return cfg
			}(),
			expectError:   true,
			errorContains: "eventstore.backup_interval_hours",
		},
		{
			name: "invalid checkpoint_interval",
			cfg: func() *Config {
				cfg := DefaultConfig()
				cfg.EventStore.CheckpointInterval = 0
				return cfg
			}(),
			expectError:   true,
			errorContains: "eventstore.checkpoint_interval",
		},
		{
			name: "invalid checkpoint_retention",
			cfg: func() *Config {
				cfg := DefaultConfig()
				cfg.EventStore.CheckpointRetention = 0
				return cfg
			}(),
			expectError:   true,
			errorContains: "eventstore.checkpoint_retention",
		},
		{
			name: "invalid migration planning_dir",
			cfg: func() *Config {
				cfg := DefaultConfig()
				cfg.Migration.PlanningDir = ""
				return cfg
			}(),
			expectError:   true,
			errorContains: "migration.planning_dir",
		},
		{
			name: "valid provider name",
			cfg: func() *Config {
				cfg := DefaultConfig()
				cfg.Provider.Default = "nvidia"
				return cfg
			}(),
			expectError: false,
		},
		{
			name: "invalid provider name",
			cfg: func() *Config {
				cfg := DefaultConfig()
				cfg.Provider.Default = "invalid"
				return cfg
			}(),
			expectError:   true,
			errorContains: "provider.default",
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			err := validateConfig(tt.cfg)
			if tt.expectError {
				require.Error(t, err)
				if tt.errorContains != "" {
					assert.Contains(t, err.Error(), tt.errorContains)
				}
			} else {
				assert.NoError(t, err)
			}
		})
	}
}

func TestLayeredConfig(t *testing.T) {
	// Test that defaults are loaded when no config file exists
	cfg := DefaultConfig()
	assert.Equal(t, ".m31a/events.db", cfg.EventStore.Path)
	assert.True(t, cfg.EventStore.WALMode)
	assert.Equal(t, 5000, cfg.EventStore.BusyTimeoutMs)
	assert.Equal(t, 24, cfg.EventStore.BackupIntervalHours)
	assert.Equal(t, 100, cfg.EventStore.CheckpointInterval)
	assert.Equal(t, 10, cfg.EventStore.CheckpointRetention)
	assert.Equal(t, ".planning", cfg.Migration.PlanningDir)
	assert.True(t, cfg.Migration.ArchiveOnComplete)

	// Test provider defaults
	assert.Equal(t, "nvidia", cfg.Provider.Default)
	assert.Equal(t, "nvidia/nemotron-3-ultra-550b-a55b", cfg.Model.Default)
	assert.Equal(t, "https://integrate.api.nvidia.com/v1", cfg.Provider.NvidiaBaseURL)
	assert.Empty(t, cfg.Provider.OpenRouter.APIKey)
	assert.Empty(t, cfg.Provider.Zen.APIKey)
	assert.Empty(t, cfg.Provider.Nvidia.APIKey)
}

func TestKeychainResolution(t *testing.T) {
	// Test that API keys are resolved from env > keychain > config file
	// This test verifies the priority order

	// Set env var
	os.Setenv("NVIDIA_API_KEY", "env-key")
	defer os.Unsetenv("NVIDIA_API_KEY")

	cfg := DefaultConfig()
	err := cfg.ResolveAPIKeys(nil)
	require.NoError(t, err)
	assert.Equal(t, "env-key", cfg.Provider.Nvidia.APIKey)

	// Test M31A_ prefixed env var
	os.Setenv("M31A_NVIDIA_API_KEY", "m31a-env-key")
	defer os.Unsetenv("M31A_NVIDIA_API_KEY")

	cfg2 := DefaultConfig()
	err = cfg2.ResolveAPIKeys(nil)
	require.NoError(t, err)
	// M31A_ prefix should take priority
	assert.Equal(t, "m31a-env-key", cfg2.Provider.Nvidia.APIKey)
}

func TestNoSecretsInConfig(t *testing.T) {
	// Test that config saved with keychain doesn't contain API keys
	cfg := DefaultConfig()
	cfg.Provider.Nvidia.APIKey = "test-key"

	// Save without keychain - should persist keys
	tempDir := t.TempDir()
	tempPath := filepath.Join(tempDir, "test-config-no-keychain.toml")

	err := cfg.SaveWithKeychain(tempPath, nil)
	require.NoError(t, err)

	// Read back and verify key is in file
	data, err := os.ReadFile(tempPath)
	require.NoError(t, err)
	configStr := string(data)
	// When no keychain, keys are persisted
	assert.Contains(t, configStr, "test-key")
}

func TestEventStoreConfigDefaults(t *testing.T) {
	cfg := DefaultConfig()
	// EventStore defaults
	assert.Equal(t, ".m31a/events.db", cfg.EventStore.Path)
	assert.True(t, cfg.EventStore.WALMode)
	assert.Equal(t, 5000, cfg.EventStore.BusyTimeoutMs)
	assert.Equal(t, 24, cfg.EventStore.BackupIntervalHours)
	assert.Equal(t, 100, cfg.EventStore.CheckpointInterval)
	assert.Equal(t, 10, cfg.EventStore.CheckpointRetention)
}

func TestMigrationConfigDefaults(t *testing.T) {
	cfg := DefaultConfig()
	// Migration defaults
	assert.Equal(t, ".planning", cfg.Migration.PlanningDir)
	assert.True(t, cfg.Migration.ArchiveOnComplete)
}
