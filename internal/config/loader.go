package config

import (
	"context"
	"crypto/rand"
	"encoding/hex"
	"errors"
	"fmt"
	"log/slog"
	"os"
	"path/filepath"
	"reflect"
	"regexp"
	"strings"
	"time"

	"github.com/BurntSushi/toml"
	"github.com/eshanized/M31A/internal/types"
	"github.com/eshanized/M31A/pkg/keychain"
)

// ErrValidation is returned when config validation fails.
var ErrValidation = errors.New("config validation")

// DefaultConfig returns a Config with sane defaults. Missing config file
// causes Load to return DefaultConfig without error.
func DefaultConfig() *Config {
	return &Config{
		UI: UIConfig{
			SidebarWidthThreshold: 120,
			MaxIterations:         100,
			DiscussTimeout:        300,
			LeaderTimeoutMs:       1000,
			ThinkingMaxLines:      20,
			PermissionModalWidth:  60,
			SidebarWidth:          42,
			MaxMessageHistory:     1000,
			FallbackBannerSecs:    15,
			DefaultLogLines:       20,
			SessionListLimit:      20,
			ThinkingOpacity:       0.6,
			FrecentHistorySize:    100,
		},
		Model: ModelConfig{
			ContextWarningThreshold: types.ContextWarningThreshold,
			TokenEMAAlpha:           0.3,
			DefaultContextLength:    types.DefaultContextLength,
		},
		Features: FeaturesConfig{
			ModelCacheTTLMinutes:   5,
			ModelCacheStaleHours:   24,
			SessionIDLength:        types.SessionIDLength,
			MaxRecentModels:        types.DefaultMaxRecentModels,
			HealthCheckLiveMs:      types.DefaultHealthLiveMs,
			HealthCheckSlowMs:      types.DefaultHealthSlowMs,
			SessionRetentionDays:   30,
			HealthCheckTimeoutSecs: 10,
			RateLimitBackoffSecs:   120,
		},
		Tools: ToolsConfig{
			MaxGlobResults:       types.DefaultMaxGlobResults,
			MaxGrepResults:       types.DefaultMaxGrepResults,
			BashKillGraceSecs:    types.DefaultBashKillGraceSecs,
			MaxBackupsPerFile:    types.DefaultMaxBackupsPerFile,
			WebfetchMaxRedirects: types.DefaultWebfetchMaxRedirects,
			WebfetchUserAgent:    "M31A/dev",
			SkipDirs:             []string{"node_modules", "vendor", ".next", "dist", "build", "target", ".venv", "venv", "__pycache__"},
		},
		Git: GitConfig{
			CommitPrefix: "feat",
			FixPrefix:    "fix",
			ShipPrefix:   "chore",
			UserName:     "M31A",
			UserEmail:    "m31a@local",
		},
	}
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
//  5. Variable substitution — ${VAR} → env value
//  6. Validation — type/range checks on known fields
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
	if err != nil {
		slog.Warn("cannot determine working directory, skipping project config", "error", err)
	} else {
		if projectPath := findProjectConfig(cwd); projectPath != "" {
			var projectCfg Config
			meta, err := toml.DecodeFile(projectPath, &projectCfg)
			if err != nil {
				slog.Warn("failed to decode project config", "path", projectPath, "error", err)
			} else {
				// Build set of explicitly defined keys to distinguish
				// "not set" from "explicitly set to false" for bool fields.
				defined := make(map[string]bool)
				for _, key := range meta.Undecoded() {
					defined[key.String()] = true
				}
				mergeConfig(cfg, &projectCfg, defined)
			}
		}
	}

	// Step 6: Variable substitution (before validation so ${VAR} in
	// enum fields like theme or permissions.default_mode resolves first)
	applyVarSubstitution(cfg)

	// Step 7: Validation
	if err := validateConfig(cfg); err != nil {
		return nil, fmt.Errorf("config validation: %w", err)
	}

	// M-12: TokenEMAAlpha=0 silently disables EMA. Apply default when unset.
	if cfg.Model.TokenEMAAlpha == 0 {
		slog.Warn("token_ema_alpha is 0 (disabled), applying default 0.3")
		cfg.Model.TokenEMAAlpha = 0.3
	}

	return cfg, nil
}

// findProjectConfig walks up from cwd (max 3 parent directories) looking for
// an m31a.toml file. Returns the path if found, or "" if none exists.
func findProjectConfig(cwd string) string {
	dir := cwd
	for i := 0; i < types.MaxProjectConfigDepth; i++ {
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

// mergeConfig performs a reflection-based merge of overlay into base.
// Overlay non-zero values override base values. Zero-valued fields in overlay
// leave base values unchanged. Handles nested structs recursively.
// The defined set tracks which TOML keys were explicitly set, enabling
// bool fields to be overridden with false.
func mergeConfig(base, overlay *Config, defined map[string]bool) {
	mergeStructs(reflect.ValueOf(base).Elem(), reflect.ValueOf(overlay).Elem(), defined, "")
}

// mergeStructs recursively merges overlay fields into base using reflection.
func mergeStructs(base, overlay reflect.Value, defined map[string]bool, prefix string) {
	overlayType := overlay.Type()
	for i := 0; i < overlay.NumField(); i++ {
		field := overlayType.Field(i)
		if !field.IsExported() {
			continue
		}
		baseField := base.FieldByName(field.Name)
		overlayField := overlay.FieldByName(field.Name)
		if !baseField.IsValid() || !overlayField.IsValid() {
			continue
		}
		fieldPrefix := prefix
		if fieldPrefix != "" {
			fieldPrefix += "."
		}
		fieldPrefix += toTOMLKey(field.Name)
		mergeField(baseField, overlayField, field.Type, defined, fieldPrefix)
	}
}

// toTOMLKey converts a Go field name to a TOML key (snake_case).
func toTOMLKey(name string) string {
	var result []byte
	for i, ch := range name {
		if ch >= 'A' && ch <= 'Z' {
			if i > 0 {
				result = append(result, '_')
			}
			result = append(result, byte(ch+32))
		} else {
			result = append(result, byte(ch))
		}
	}
	return string(result)
}

// mergeField copies a single field from overlay to base if non-zero.
func mergeField(base, overlay reflect.Value, typ reflect.Type, defined map[string]bool, key string) {
	switch typ.Kind() {
	case reflect.String:
		if overlay.String() != "" {
			base.SetString(overlay.String())
		}
	case reflect.Bool:
		// G-4 fix: Check if the bool was explicitly defined in the TOML.
		// If explicitly set, always overwrite (even with false).
		// If not in the defined set, only overwrite if overlay is true
		// (to preserve base value for unset fields).
		if defined[key] || overlay.Bool() {
			base.SetBool(overlay.Bool())
		}
	case reflect.Int, reflect.Int8, reflect.Int16, reflect.Int32, reflect.Int64:
		if overlay.Int() != 0 {
			base.SetInt(overlay.Int())
		}
	case reflect.Float32, reflect.Float64:
		if overlay.Float() != 0 {
			base.SetFloat(overlay.Float())
		}
	case reflect.Slice:
		if !overlay.IsNil() && overlay.Len() > 0 {
			newSlice := reflect.MakeSlice(typ, overlay.Len(), overlay.Len())
			reflect.Copy(newSlice, overlay)
			base.Set(newSlice)
		}
	case reflect.Map:
		if !overlay.IsNil() {
			if base.IsNil() {
				base.Set(reflect.MakeMap(typ))
			}
			iter := overlay.MapRange()
			for iter.Next() {
				base.SetMapIndex(iter.Key(), iter.Value())
			}
		}
	case reflect.Struct:
		mergeStructs(base, overlay, defined, key)
	}
}

// ValidationError describes a single field-level config validation failure.
type ValidationError struct {
	Field        string
	ExpectedType string
	ActualValue  string
}

func (e ValidationError) Error() string {
	return fmt.Sprintf("config: field %q expected %s, got %q", e.Field, e.ExpectedType, e.ActualValue)
}

// validateConfig checks all known Config fields for type/range correctness.
// Collects all errors and returns them as a joined error.
func validateConfig(cfg *Config) error {
	var errs []error

	// Provider
	if cfg.Provider.AutoFallback && cfg.Provider.Default == "" {
		errs = append(errs, ValidationError{
			Field:        "provider.default",
			ExpectedType: "non-empty string (when auto_fallback is true)",
			ActualValue:  "",
		})
	}

	// Model
	if cfg.Model.ContextWarningThreshold < 0 || cfg.Model.ContextWarningThreshold > 1 {
		errs = append(errs, ValidationError{
			Field:        "model.context_warning_threshold",
			ExpectedType: "float64 between 0 and 1",
			ActualValue:  fmt.Sprintf("%v", cfg.Model.ContextWarningThreshold),
		})
	}
	if cfg.Model.ArbitrageThreshold < 0 || cfg.Model.ArbitrageThreshold > 1 {
		errs = append(errs, ValidationError{
			Field:        "model.arbitrage_threshold",
			ExpectedType: "float64 between 0 and 1",
			ActualValue:  fmt.Sprintf("%v", cfg.Model.ArbitrageThreshold),
		})
	}
	if cfg.Model.DefaultContextLength < 0 {
		errs = append(errs, ValidationError{
			Field:        "model.default_context_length",
			ExpectedType: "non-negative integer",
			ActualValue:  fmt.Sprintf("%d", cfg.Model.DefaultContextLength),
		})
	}
	if cfg.Model.TokenEMAAlpha < 0 || cfg.Model.TokenEMAAlpha > 1 {
		errs = append(errs, ValidationError{
			Field:        "model.token_ema_alpha",
			ExpectedType: "float64 between 0 and 1",
			ActualValue:  fmt.Sprintf("%v", cfg.Model.TokenEMAAlpha),
		})
	}

	// UI
	if cfg.UI.Theme != "" && cfg.UI.Theme != "dark" && cfg.UI.Theme != "light" && cfg.UI.Theme != "auto" {
		errs = append(errs, ValidationError{
			Field:        "ui.theme",
			ExpectedType: "\"dark\", \"light\", or \"auto\"",
			ActualValue:  cfg.UI.Theme,
		})
	}
	if cfg.UI.MaxIterations < 0 {
		errs = append(errs, ValidationError{
			Field:        "ui.max_iterations",
			ExpectedType: "non-negative integer",
			ActualValue:  fmt.Sprintf("%d", cfg.UI.MaxIterations),
		})
	}
	if cfg.UI.LeaderTimeoutMs < 0 {
		errs = append(errs, ValidationError{
			Field:        "ui.leader_timeout_ms",
			ExpectedType: "non-negative integer",
			ActualValue:  fmt.Sprintf("%d", cfg.UI.LeaderTimeoutMs),
		})
	}

	// Permissions
	if cfg.Permissions.DefaultMode != "" &&
		cfg.Permissions.DefaultMode != "prompt" &&
		cfg.Permissions.DefaultMode != "allow" &&
		cfg.Permissions.DefaultMode != "deny" {
		errs = append(errs, ValidationError{
			Field:        "permissions.default_mode",
			ExpectedType: "\"prompt\", \"allow\", or \"deny\"",
			ActualValue:  cfg.Permissions.DefaultMode,
		})
	}
	if cfg.Permissions.TimeoutSeconds < 0 {
		errs = append(errs, ValidationError{
			Field:        "permissions.timeout_seconds",
			ExpectedType: "non-negative integer",
			ActualValue:  fmt.Sprintf("%d", cfg.Permissions.TimeoutSeconds),
		})
	}

	// Rules validation
	for i, rule := range cfg.Permissions.Rules {
		if rule.Tool == "" {
			errs = append(errs, ValidationError{
				Field:        fmt.Sprintf("permissions.rules[%d].tool", i),
				ExpectedType: "non-empty string",
				ActualValue:  "",
			})
		}
		if rule.Action != "" &&
			rule.Action != "allow" && rule.Action != "deny" && rule.Action != "ask" {
			errs = append(errs, ValidationError{
				Field:        fmt.Sprintf("permissions.rules[%d].action", i),
				ExpectedType: "\"allow\", \"deny\", or \"ask\"",
				ActualValue:  rule.Action,
			})
		}
	}

	// Ledger
	if cfg.Ledger.MaxEntries < 0 {
		errs = append(errs, ValidationError{
			Field:        "ledger.max_entries",
			ExpectedType: "non-negative integer",
			ActualValue:  fmt.Sprintf("%d", cfg.Ledger.MaxEntries),
		})
	}

	// Features
	if cfg.Features.ModelCacheTTLMinutes < 0 {
		errs = append(errs, ValidationError{
			Field:        "features.model_cache_ttl_minutes",
			ExpectedType: "non-negative integer",
			ActualValue:  fmt.Sprintf("%d", cfg.Features.ModelCacheTTLMinutes),
		})
	}
	if cfg.Features.ModelCacheStaleHours < 0 {
		errs = append(errs, ValidationError{
			Field:        "features.model_cache_stale_hours",
			ExpectedType: "non-negative integer",
			ActualValue:  fmt.Sprintf("%d", cfg.Features.ModelCacheStaleHours),
		})
	}
	if cfg.Features.HealthCheckLiveMs < 0 {
		errs = append(errs, ValidationError{
			Field:        "features.healthcheck_live_ms",
			ExpectedType: "non-negative integer",
			ActualValue:  fmt.Sprintf("%d", cfg.Features.HealthCheckLiveMs),
		})
	}
	if cfg.Features.HealthCheckSlowMs < 0 || cfg.Features.HealthCheckSlowMs < cfg.Features.HealthCheckLiveMs {
		errs = append(errs, ValidationError{
			Field:        "features.healthcheck_slow_ms",
			ExpectedType: "non-negative integer >= healthcheck_live_ms",
			ActualValue:  fmt.Sprintf("%d", cfg.Features.HealthCheckSlowMs),
		})
	}
	if cfg.Features.SessionIDLength != 0 && (cfg.Features.SessionIDLength < 4 || cfg.Features.SessionIDLength > 16) {
		errs = append(errs, ValidationError{
			Field:        "features.session_id_length",
			ExpectedType: "integer between 4 and 16",
			ActualValue:  fmt.Sprintf("%d", cfg.Features.SessionIDLength),
		})
	}
	if cfg.Features.MaxRecentModels < 0 {
		errs = append(errs, ValidationError{
			Field:        "features.max_recent_models",
			ExpectedType: "non-negative integer",
			ActualValue:  fmt.Sprintf("%d", cfg.Features.MaxRecentModels),
		})
	}

	// Tools
	if cfg.Tools.MaxGlobResults < 0 {
		errs = append(errs, ValidationError{
			Field:        "tools.max_glob_results",
			ExpectedType: "non-negative integer",
			ActualValue:  fmt.Sprintf("%d", cfg.Tools.MaxGlobResults),
		})
	}
	if cfg.Tools.MaxGrepResults < 0 {
		errs = append(errs, ValidationError{
			Field:        "tools.max_grep_results",
			ExpectedType: "non-negative integer",
			ActualValue:  fmt.Sprintf("%d", cfg.Tools.MaxGrepResults),
		})
	}
	if cfg.Tools.BashKillGraceSecs < 0 {
		errs = append(errs, ValidationError{
			Field:        "tools.bash_kill_grace_secs",
			ExpectedType: "non-negative integer",
			ActualValue:  fmt.Sprintf("%d", cfg.Tools.BashKillGraceSecs),
		})
	}
	if cfg.Tools.MaxBackupsPerFile < 0 {
		errs = append(errs, ValidationError{
			Field:        "tools.max_backups_per_file",
			ExpectedType: "non-negative integer",
			ActualValue:  fmt.Sprintf("%d", cfg.Tools.MaxBackupsPerFile),
		})
	}
	if cfg.Tools.WebfetchMaxRedirects < 0 {
		errs = append(errs, ValidationError{
			Field:        "tools.webfetch_max_redirects",
			ExpectedType: "non-negative integer",
			ActualValue:  fmt.Sprintf("%d", cfg.Tools.WebfetchMaxRedirects),
		})
	}

	if len(errs) > 0 {
		var b strings.Builder
		b.WriteString("Invalid configuration:\n")
		for _, err := range errs {
			b.WriteString("- ")
			b.WriteString(err.Error())
			b.WriteString("\n")
		}
		return fmt.Errorf("%w\n%s", ErrValidation, b.String())
	}
	return nil
}

var varRe = regexp.MustCompile(`\$\{([^}]+)\}`)

// applyVarSubstitution walks all string fields in cfg and replaces ${VAR}
// patterns with their corresponding environment variable values.
func applyVarSubstitution(cfg *Config) {
	cfg.Provider.Default = substituteVars(cfg.Provider.Default)
	cfg.Provider.OpenRouter.APIKey = substituteVars(cfg.Provider.OpenRouter.APIKey)
	cfg.Provider.Zen.APIKey = substituteVars(cfg.Provider.Zen.APIKey)
	cfg.Model.Default = substituteVars(cfg.Model.Default)
	cfg.UI.Theme = substituteVars(cfg.UI.Theme)
	cfg.Permissions.DefaultMode = substituteVars(cfg.Permissions.DefaultMode)

	for i := range cfg.Permissions.Rules {
		cfg.Permissions.Rules[i].Tool = substituteVars(cfg.Permissions.Rules[i].Tool)
		cfg.Permissions.Rules[i].Pattern = substituteVars(cfg.Permissions.Rules[i].Pattern)
		cfg.Permissions.Rules[i].Action = substituteVars(cfg.Permissions.Rules[i].Action)
	}

	for name := range cfg.Permissions.Agents {
		agent := cfg.Permissions.Agents[name]
		agent.DefaultAction = substituteVars(agent.DefaultAction)
		for j := range agent.Rules {
			agent.Rules[j].Tool = substituteVars(agent.Rules[j].Tool)
			agent.Rules[j].Pattern = substituteVars(agent.Rules[j].Pattern)
			agent.Rules[j].Action = substituteVars(agent.Rules[j].Action)
		}
		cfg.Permissions.Agents[name] = agent
	}
}

// substituteVars replaces ${VAR} patterns in s with the value of the
// environment variable VAR. If the variable is not set, the original ${VAR}
// pattern is preserved as-is.
func substituteVars(s string) string {
	if s == "" || !strings.Contains(s, "${") {
		return s
	}
	return varRe.ReplaceAllStringFunc(s, func(match string) string {
		name := match[2 : len(match)-1]
		if val, ok := os.LookupEnv(name); ok {
			return val
		}
		return ""
	})
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

	// Copy the config to avoid mutating the original
	cfgCopy := *c

	// Don't persist API keys — they came from env vars or keychain
	cfgCopy.Provider.OpenRouter.APIKey = ""
	cfgCopy.Provider.Zen.APIKey = ""

	data, err := toml.Marshal(&cfgCopy)
	if err != nil {
		return fmt.Errorf("marshal config: %w", err)
	}

	// Ensure parent directory exists
	if err := os.MkdirAll(filepath.Dir(path), types.DirPermission); err != nil {
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

// ConfigReloadMsg is emitted when the config file changes on disk.
type ConfigReloadMsg struct {
	Config *Config
	Error  error
}

// WatchConfig polls the config file for changes and sends ConfigReloadMsg
// to the provided channel when a change is detected. Runs until ctx is cancelled.
func WatchConfig(ctx context.Context, path string, ch chan<- ConfigReloadMsg) {
	var lastModTime time.Time
	if info, err := os.Stat(path); err == nil {
		lastModTime = info.ModTime()
	}
	ticker := time.NewTicker(types.ConfigWatchInterval)
	defer ticker.Stop()
	for {
		select {
		case <-ctx.Done():
			return
		case <-ticker.C:
			info, err := os.Stat(path)
			if err != nil {
				continue
			}
			if info.ModTime().After(lastModTime) {
				lastModTime = info.ModTime()
				cfg, err := Load(path)
				ch <- ConfigReloadMsg{Config: cfg, Error: err}
			}
		}
	}
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

	// Write to temp file with secure permissions
	tmpFile, err := os.OpenFile(tmpPath, os.O_CREATE|os.O_WRONLY, 0600)
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
