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

// Load reads a TOML config file from the given path, applies env var overrides,
// and returns the resulting Config. If the file does not exist, DefaultConfig
// is returned with nil error (first-run flow handles file creation).
//
// Env var overrides (applied after TOML parsing):
//   - M31A_CONFIG  overrides the config file path
//   - M31A_THEME   overrides cfg.UI.Theme
//   - M31A_DEFAULT_MODEL overrides cfg.Model.Default
func Load(path string) (*Config, error) {
	// M31A_CONFIG env var overrides path
	if envPath := os.Getenv("M31A_CONFIG"); envPath != "" {
		path = envPath
	}

	cfg := DefaultConfig()

	_, err := toml.DecodeFile(path, cfg)
	if err != nil {
		if os.IsNotExist(err) {
			// Missing config file is not an error — first-run handles it
			return cfg, nil
		}
		return nil, fmt.Errorf("decode config: %w", err)
	}

	// Apply env var overrides after TOML parsing
	if theme := os.Getenv("M31A_THEME"); theme != "" {
		cfg.UI.Theme = theme
	}
	if model := os.Getenv("M31A_DEFAULT_MODEL"); model != "" {
		cfg.Model.Default = model
	}

	return cfg, nil
}

// Save writes the config to a TOML file atomically (temp file + rename).
// If M31A_CONFIG env var is set, it is used as the path instead.
func (c *Config) Save(path string) error {
	// M31A_CONFIG env var overrides path
	if envPath := os.Getenv("M31A_CONFIG"); envPath != "" {
		path = envPath
	}

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
