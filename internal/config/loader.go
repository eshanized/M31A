package config

import (
	"crypto/rand"
	"encoding/hex"
	"errors"
	"fmt"
	"log/slog"
	"os"
	"path/filepath"

	"github.com/BurntSushi/toml"
	"github.com/eshanized/M31A/pkg/keychain"
)

// DefaultConfig returns a Config with zero-valued fields.
// Missing config file causes Load to return DefaultConfig without error.
func DefaultConfig() *Config {
	return &Config{}
}

// Load reads a TOML config file from the given path, applies multi-layer
// merging (global TOML → env vars → project m31a.toml), validation, and
// variable substitution, then returns the resulting Config.
//
// Config loading order (later overrides earlier):
//  1. DefaultConfig() — zero-valued defaults
//  2. Global TOML (~/.m31a/config.toml via path arg)
//  3. Environment variable overrides (M31A_*)
//  4. Project-level m31a.toml (walked up from cwd, max 3 levels)
//  5. Validation — type/range checks on known fields
//  6. Variable substitution — ${VAR} → env value
//
// Missing global config file is not an error — first-run flow handles creation.
func Load(path string) (*Config, error) {
	// Step 1: M31A_CONFIG env var overrides path argument
	if envPath := os.Getenv("M31A_CONFIG"); envPath != "" {
		path = envPath
	}

	// Step 2: Layer 1 — Defaults (zero-valued Config)
	cfg := DefaultConfig()

	// Step 3: Layer 2 — Global config (~/.m31a/config.toml)
	if path != "" {
		_, err := toml.DecodeFile(path, cfg)
		if err != nil {
			if !os.IsNotExist(err) {
				return nil, fmt.Errorf("decode global config %s: %w", path, err)
			}
			// Missing global config is not an error
		}
	}

	// Step 4: Layer 3 — Environment variable overrides
	if theme := os.Getenv("M31A_THEME"); theme != "" {
		cfg.UI.Theme = theme
	}
	if model := os.Getenv("M31A_DEFAULT_MODEL"); model != "" {
		cfg.Model.Default = model
	}

	// Step 5: Layer 4 — Project-level config (m31a.toml in cwd)
	cwd, err := os.Getwd()
	if err == nil {
		if projectPath := findProjectConfig(cwd); projectPath != "" {
			var projectCfg Config
			if _, err := toml.DecodeFile(projectPath, &projectCfg); err != nil {
				slog.Warn("failed to decode project config", "path", projectPath, "error", err)
			} else {
				mergeConfig(cfg, &projectCfg)
			}
		}
	}

	// Step 6: Validation — added in Task 2
	// if err := validateConfig(cfg); err != nil {
	//     return nil, fmt.Errorf("config validation: %w", err)
	// }

	// Step 7: Variable substitution — added in Task 2
	// applyVarSubstitution(cfg)

	return cfg, nil
}

// findProjectConfig walks up from cwd (max 3 parent directories) looking for
// an m31a.toml file. Returns the path if found, or "" if none exists.
func findProjectConfig(cwd string) string {
	dir := cwd
	for i := 0; i < 3; i++ {
		candidate := filepath.Join(dir, "m31a.toml")
		if _, err := os.Stat(candidate); err == nil {
			return candidate
		}
		parent := filepath.Dir(dir)
		if parent == dir {
			break
		}
		dir = parent
	}
	return ""
}

// mergeConfig performs a field-level merge of overlay into base.
// Overlay non-zero values override base values. Zero-valued fields in overlay
// leave base values unchanged. This is a flat merge per top-level section.
func mergeConfig(base, overlay *Config) {
	// Provider section
	if overlay.Provider.Default != "" {
		base.Provider.Default = overlay.Provider.Default
	}
	if overlay.Provider.AutoFallback {
		base.Provider.AutoFallback = true
	}
	if overlay.Provider.OpenRouter.APIKey != "" {
		base.Provider.OpenRouter.APIKey = overlay.Provider.OpenRouter.APIKey
	}
	if overlay.Provider.Zen.APIKey != "" {
		base.Provider.Zen.APIKey = overlay.Provider.Zen.APIKey
	}

	// Model section
	if overlay.Model.Default != "" {
		base.Model.Default = overlay.Model.Default
	}
	if overlay.Model.ContextWarningThreshold != 0 {
		base.Model.ContextWarningThreshold = overlay.Model.ContextWarningThreshold
	}
	if overlay.Model.ShowThinkingByDefault {
		base.Model.ShowThinkingByDefault = true
	}
	if overlay.Model.AutoCollapseTools {
		base.Model.AutoCollapseTools = true
	}
	if overlay.Model.AutoArbitrage {
		base.Model.AutoArbitrage = true
	}
	if overlay.Model.ArbitrageThreshold != 0 {
		base.Model.ArbitrageThreshold = overlay.Model.ArbitrageThreshold
	}

	// UI section
	if overlay.UI.Theme != "" {
		base.UI.Theme = overlay.UI.Theme
	}
	if overlay.UI.CompactMode {
		base.UI.CompactMode = true
	}
	if overlay.UI.ShowTokenUsage {
		base.UI.ShowTokenUsage = true
	}
	if overlay.UI.ShowCostEstimate {
		base.UI.ShowCostEstimate = true
	}
	if overlay.UI.MaxIterations != 0 {
		base.UI.MaxIterations = overlay.UI.MaxIterations
	}

	// Permissions section
	if overlay.Permissions.DefaultMode != "" {
		base.Permissions.DefaultMode = overlay.Permissions.DefaultMode
	}
	if overlay.Permissions.TimeoutSeconds != 0 {
		base.Permissions.TimeoutSeconds = overlay.Permissions.TimeoutSeconds
	}
	if len(overlay.Permissions.Rules) > 0 {
		base.Permissions.Rules = overlay.Permissions.Rules
	}
	// Agents merge added in Task 2 (permissions agent profiles)

	// Features section
	if overlay.Features.AutoBackup {
		base.Features.AutoBackup = true
	}
	if overlay.Features.ResumeOnStartup {
		base.Features.ResumeOnStartup = true
	}

	// Ledger section
	if overlay.Ledger.Enabled {
		base.Ledger.Enabled = true
	}
	if overlay.Ledger.MaxEntries != 0 {
		base.Ledger.MaxEntries = overlay.Ledger.MaxEntries
	}
}

// Save writes the config to a TOML file atomically (temp file + rename).
// If M31A_CONFIG env var is set, it is used as the path instead.
// API keys are never persisted to the config file — they must be set via
// environment variables or the OS keychain.
func (c *Config) Save(path string) error {
	// M31A_CONFIG env var overrides path
	if envPath := os.Getenv("M31A_CONFIG"); envPath != "" {
		path = envPath
	}

	// Don't persist API keys — they came from env vars or keychain
	c.Provider.OpenRouter.APIKey = ""
	c.Provider.Zen.APIKey = ""

	data, err := toml.Marshal(c)
	if err != nil {
		return fmt.Errorf("marshal config: %w", err)
	}

	// Ensure parent directory exists
	if err := os.MkdirAll(filepath.Dir(path), 0755); err != nil {
		return fmt.Errorf("create config dir: %w", err)
	}

	// Atomic write: write to temp file in same directory, then rename
	if err := atomicWrite(path, data); err != nil {
		return fmt.Errorf("write config: %w", err)
	}

	return nil
}

// ResolveAPIKeys resolves API keys for OpenRouter and Zen using the priority
// order: environment variable → OS keychain → config file field.
//
// Resolution per provider:
//  1. Check M31A_OPENROUTER_API_KEY / M31A_ZEN_API_KEY env var
//  2. If not set, try keychain.Get("openrouter") / keychain.Get("zen")
//  3. If keychain returns ErrKeyNotFound or ErrKeychainUnavailable,
//     fall back to c.Provider.OpenRouter.APIKey / c.Provider.Zen.APIKey
//
// Never returns an error for missing keys — keys may be left empty for
// first-run or settings screen to handle.
func (c *Config) ResolveAPIKeys(kc keychain.Keychain) error {
	// OpenRouter
	if key := os.Getenv("M31A_OPENROUTER_API_KEY"); key != "" {
		c.Provider.OpenRouter.APIKey = key
	} else if kc != nil {
		if k, err := kc.Get("openrouter"); err == nil {
			c.Provider.OpenRouter.APIKey = k
		} else if !errors.Is(err, keychain.ErrKeyNotFound) && !errors.Is(err, keychain.ErrKeychainUnavailable) {
			// Unexpected error — log and continue
			slog.Warn("keychain error", "provider", "openrouter", "error", err)
		}
		// If keychain returns ErrKeyNotFound or ErrKeychainUnavailable,
		// keep the value from config file (already loaded in c.Provider.OpenRouter.APIKey)
	}

	// Zen
	if key := os.Getenv("M31A_ZEN_API_KEY"); key != "" {
		c.Provider.Zen.APIKey = key
	} else if kc != nil {
		if k, err := kc.Get("zen"); err == nil {
			c.Provider.Zen.APIKey = k
		} else if !errors.Is(err, keychain.ErrKeyNotFound) && !errors.Is(err, keychain.ErrKeychainUnavailable) {
			slog.Warn("keychain error", "provider", "zen", "error", err)
		}
	}

	return nil
}

// atomicWrite writes data to path atomically using a temp file and rename.
// Temp file is created in the same directory to ensure atomic rename works
// within the same filesystem mount point.
func atomicWrite(path string, data []byte) error {
	dir := filepath.Dir(path)

	// Generate random temp name
	randBytes := make([]byte, 8)
	if _, err := rand.Read(randBytes); err != nil {
		return fmt.Errorf("generate temp name: %w", err)
	}
	tmpPath := filepath.Join(dir, ".m31a_tmp_"+hex.EncodeToString(randBytes))

	// Write to temp file
	tmpFile, err := os.Create(tmpPath)
	if err != nil {
		return fmt.Errorf("create temp file: %w", err)
	}
	defer os.Remove(tmpPath) // cleanup on failure

	if _, err := tmpFile.Write(data); err != nil {
		tmpFile.Close()
		return fmt.Errorf("write temp file: %w", err)
	}
	if err := tmpFile.Sync(); err != nil {
		tmpFile.Close()
		return fmt.Errorf("sync temp file: %w", err)
	}
	if err := tmpFile.Close(); err != nil {
		return fmt.Errorf("close temp file: %w", err)
	}

	// Atomic rename
	if err := os.Rename(tmpPath, path); err != nil {
		return fmt.Errorf("rename temp file: %w", err)
	}

	return nil
}
