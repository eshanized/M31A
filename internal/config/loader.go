package config

import (
	"context"
	"errors"
	"fmt"
	"log/slog"
	"os"
	"path/filepath"
	"regexp"
	"strings"
	"sync"
	"time"

	"github.com/BurntSushi/toml"
	"github.com/eshanized/M31A/internal/fileutil"
	"github.com/eshanized/M31A/internal/types"
	"github.com/eshanized/M31A/pkg/keychain"
	"github.com/fsnotify/fsnotify"
)

// ErrValidation is returned when config validation fails.
var ErrValidation = errors.New("config validation")

// DefaultConfig returns a Config with sane defaults. Missing config file
// causes Load to return DefaultConfig without error.
func DefaultConfig() *Config {
	return &Config{
		Provider: ProviderConfig{
			FallbackPriority:       []string{"nvidia", "zen", "openrouter"},
			HealthCheckTimeoutSecs: 10,
			RegistrationOrder:      []string{"openrouter", "zen", "nvidia"},
		},
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
			// Layout thresholds (F-046)
			WidthUltraCompact: 40,
			WidthCompact:      60,
			WidthFull:         80,
			// Toast (F-047)
			ToastMaxVisible: 3,
			// History (F-048, F-049)
			MaxMessages:  500,
			MaxTodoItems: 50,
			// Sidebar (F-050)
			SidebarRefreshSecs: 5,
			// Welcome screen (F-081, F-082)
			WelcomeTwoColThreshold: 88,
			WelcomeCardMinWidth:    20,
			WelcomeCardMaxWidth:    60,
		},
		Model: ModelConfig{
			ContextWarningThreshold: types.ContextWarningThreshold,
			TokenEMAAlpha:           types.EMACorrectionAlpha,
			DefaultContextLength:    types.DefaultContextLength,
		},
		Features: FeaturesConfig{
			MetricsEnabled:            true,
			ModelCacheTTLMinutes:      5,
			ModelCacheStaleHours:      24,
			SessionIDLength:           types.SessionIDLength,
			MaxRecentModels:           types.DefaultMaxRecentModels,
			HealthCheckLiveMs:         types.DefaultHealthLiveMs,
			HealthCheckSlowMs:         types.DefaultHealthSlowMs,
			SessionRetentionDays:      30,
			HealthCheckTimeoutSecs:    10,
			RateLimitBackoffSecs:      120,
			PlanCheckMaxIter:          3,
			PlanChunkThreshold:        10,
			IntentClassification:      true,
			IntentClassifyTimeoutSecs: 25,
			// Enable all quality gates and checks by default
			PlanResearch:        true,
			PlanCheck:           true,
			PlanSecurityGate:    true,
			PlanCoverageGate:    true,
			PlanGapAnalysis:     true,
			PlanChunked:         true,
			DiscussQualityCheck: true,
			DiscussCompleteness: true,
			ExecutePreflight:    true,
			ExecuteQualityGate:  true,
			ExecuteLoopDetect:   true,
			VerifyReport:        true,
			ShipPreflight:       true,
			ShipChangelog:       true,
			InitDeepAnalysis:    true,
			InitPreflight:       true,
			// Workflow thresholds (F-030, F-031)
			MaxHealAttempts: types.MaxHealAttempts,
			MaxPlanRetries:  types.MaxPlanRetries,
			// Context (F-033)
			ContextTruncationThreshold: types.ContextWarningThreshold,
			// Retry policy (F-061)
			RetryMaxAttempts:       3,
			RetryBaseDelayMs:       1000,
			RetryMaxDelayMs:        30000,
			RetryBackoffMultiplier: 2.0,
			// Retry-after (F-062)
			MaxRetryAfterSecs: int(types.MaxRetryAfterWait / time.Second),
			// Task runner (F-076)
			MaxParallelTasks: types.DefaultMaxParallelTasks,
			// Coordinator (F-078)
			CoordinatorTimeoutSecs: 300,
		},
		Tools: ToolsConfig{
			MaxGlobResults:       types.DefaultMaxGlobResults,
			MaxGrepResults:       types.DefaultMaxGrepResults,
			BashKillGraceSecs:    types.DefaultBashKillGraceSecs,
			MaxBackupsPerFile:    types.DefaultMaxBackupsPerFile,
			WebfetchMaxRedirects: types.DefaultWebfetchMaxRedirects,
			WebfetchUserAgent:    "M31A/dev",
			SkipDirs:             types.SkipDirs,
			WebSearchBaseURL:     "https://search.sagibo.net",
			WebSearchEnabled:     true,
			OutputMaxLines:       types.DefaultOutputMaxLines,
			OutputMaxBytes:       types.DefaultOutputMaxBytes,
			// Rate limiting (F-018)
			RateLimitBurst:           20,
			RateLimitPerSec:          10,
			DangerousRateLimitBurst:  5,
			DangerousRateLimitPerSec: 2,
			MaxConcurrent:            8,
			// Output bounds (F-019)
			OutputRetentionDays: 7,
			// DNS (F-023)
			DnsCacheTTLSecs: 300,
			// Edit tool (F-024)
			FuzzyThreshold:   0.7,
			MinLinesForFuzzy: 3,
			// Bash (F-020)
			BashMaxTimeoutSecs: 1800,
			// WebFetch (F-021)
			WebfetchMaxRetries:   3,
			WebfetchRetryDelayMs: 500,
			// Execute phase (F-086, F-087)
			MaxToolConcurrency: 4,
			LoopDetectWindow:   3,
		},
		Git: GitConfig{
			CommitPrefix: "feat",
			FixPrefix:    "fix",
			ShipPrefix:   "chore",
			UserName:     "M31A",
			UserEmail:    "m31a@local",
		},
		Compaction: CompactionConfig{
			Auto:               true,
			Buffer:             20000,
			KeepTokens:         8000,
			Proactive:          true,
			ToolCallsThreshold: 15,
			PhaseTransitionPct: 60,
		},
		Instructions: InstructionsConfig{
			Enabled: true,
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
		meta, err := toml.DecodeFile(path, cfg)
		if err != nil {
			if !os.IsNotExist(err) {
				return nil, fmt.Errorf("decode global config %s: %w", path, err)
			}
			// Missing global config is not an error
		} else {
			// Warn on unknown TOML keys so typos like [providr] don't silently fail.
			// meta.Keys() returns fully-qualified dotted paths (e.g. "provider.default"),
			// so we check only the top-level section name against the known set.
			knownKeys := knownConfigKeys()
			for _, key := range meta.Keys() {
				k := key.String()
				topLevel := k
				if dotIdx := strings.IndexByte(k, '.'); dotIdx >= 0 {
					topLevel = k[:dotIdx]
				}
				if !knownKeys[topLevel] {
					slog.Warn("unknown config key, check for typos", "key", k, "file", path)
				}
			}
		}
	}

	// Step 3.5: Auto-load .env file from cwd (I1)
	LoadDotEnv()

	// Step 4: Layer 3 — Environment variable overrides (C2)
	if theme := os.Getenv("M31A_THEME"); theme != "" {
		cfg.UI.Theme = theme
	}
	if model := os.Getenv("M31A_DEFAULT_MODEL"); model != "" {
		cfg.Model.Default = model
	}
	if provider := os.Getenv("M31A_PROVIDER"); provider != "" {
		cfg.Provider.Default = provider
	}
	if mode := os.Getenv("M31A_PERMISSION_MODE"); mode != "" {
		cfg.Permissions.DefaultMode = mode
	}
	if compact := os.Getenv("M31A_COMPACT"); compact == "true" || compact == "1" {
		cfg.UI.CompactMode = true
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
				for _, key := range meta.Keys() {
					defined[key.String()] = true
				}
				mergeConfig(cfg, &projectCfg, defined)
			}
		}
	}

	// Step 6: Variable substitution (before validation so ${VAR} in
	// enum fields like theme or permissions.default_mode resolves first)
	unresolvedVars := applyVarSubstitution(cfg)

	// Step 7: Validation
	if err := validateConfig(cfg); err != nil {
		return nil, fmt.Errorf("config validation: %w", err)
	}

	// Check for unresolved ${VAR} patterns — these are almost certainly
	// user mistakes (typo in env var name, forgot to export, etc.).
	// Log warnings but don't block startup; the unresolved patterns are
	// preserved as-is so the user can see them in the running config.
	if len(unresolvedVars) > 0 {
		for _, msg := range unresolvedVars {
			slog.Warn(msg)
		}
	}

	return cfg, nil
}

// findProjectConfig walks up from cwd (max 3 parent directories) looking for
// an m31a.toml file. Returns the path if found, or "" if none exists.
// Note: this may load config from a parent project directory when running
// in a nested subdirectory of another project.
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

// LocalConfigPath returns the path where a new project-level m31a.toml should
// be created (cwd/m31a.toml).
func LocalConfigPath() (string, error) {
	cwd, err := os.Getwd()
	if err != nil {
		return "", err
	}
	return filepath.Join(cwd, "m31a.toml"), nil
}

// SaveProject writes the config to a project-level m31a.toml in the working
// directory. API keys are always cleared from project config for security —
// they belong in the global config or keychain.
func (c *Config) SaveProject(path string) error {
	cfgCopy := *c
	cfgCopy.Provider.OpenRouter.APIKey = ""
	cfgCopy.Provider.Zen.APIKey = ""

	if c.Permissions.Rules != nil {
		rulesCopy := make([]PermissionRule, len(c.Permissions.Rules))
		copy(rulesCopy, c.Permissions.Rules)
		cfgCopy.Permissions.Rules = rulesCopy
	}
	if c.Permissions.Agents != nil {
		agentsCopy := make(map[string]PermissionsAgentConfig, len(c.Permissions.Agents))
		for k, v := range c.Permissions.Agents {
			agentRulesCopy := make([]PermissionRule, len(v.Rules))
			copy(agentRulesCopy, v.Rules)
			v.Rules = agentRulesCopy
			agentsCopy[k] = v
		}
		cfgCopy.Permissions.Agents = agentsCopy
	}
	if c.Tools.SkipDirs != nil {
		skipDirsCopy := make([]string, len(c.Tools.SkipDirs))
		copy(skipDirsCopy, c.Tools.SkipDirs)
		cfgCopy.Tools.SkipDirs = skipDirsCopy
	}

	data, err := toml.Marshal(&cfgCopy)
	if err != nil {
		return fmt.Errorf("marshal project config: %w", err)
	}
	if err := fileutil.AtomicWrite(path, data); err != nil {
		return fmt.Errorf("write project config: %w", err)
	}
	return nil
}

// mergeConfig performs a type-safe merge of overlay into base.
// Overlay non-zero values override base values. Zero-valued fields in overlay
// leave base values unchanged. Handles nested structs recursively.
// The defined set tracks which TOML keys were explicitly set, enabling
// bool fields to be overridden with false.
func mergeConfig(base, overlay *Config, defined map[string]bool) {
	MergeConfig(base, overlay, defined)
}

// toTOMLKey converts a Go field name to a TOML key (snake_case).
// Handles acronyms correctly: APIKey → api_key, BaseURL → base_url,
// HTTPSProxy → https_proxy.
// Since Go struct field names are always ASCII, iterates over bytes directly
// instead of converting to []rune.
func toTOMLKey(name string) string {
	var result []byte
	for i := 0; i < len(name); i++ {
		ch := name[i]
		if ch >= 'A' && ch <= 'Z' {
			if i > 0 {
				prev := name[i-1]
				prevIsLower := prev >= 'a' && prev <= 'z'
				prevIsDigit := prev >= '0' && prev <= '9'
				nextIsLower := i+1 < len(name) && name[i+1] >= 'a' && name[i+1] <= 'z'
				if prevIsLower || prevIsDigit || nextIsLower {
					result = append(result, '_')
				}
			}
			result = append(result, ch+32)
		} else {
			result = append(result, ch)
		}
	}
	return string(result)
}

// ValidationError describes a single field-level config validation failure.
type ValidationError struct {
	Field        string
	ExpectedType string
	ActualValue  string
}

func (e ValidationError) Error() string {
	return fmt.Sprintf("Invalid config value for '%s': expected %s, but got '%s'. Check your config file.", e.Field, e.ExpectedType, e.ActualValue)
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
	if cfg.UI.Theme == "" {
		// Treat empty theme as "dark" (the documented default). The first-run
		// wizard writes theme = "" which would otherwise fail validation.
		cfg.UI.Theme = "dark"
	}
	if cfg.UI.Theme != "dark" && cfg.UI.Theme != "light" && cfg.UI.Theme != "auto" {
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

	// T-07-01: Rate limit bounds — prevent unlimited rate limits via config.
	if cfg.Tools.RateLimitBurst < 0 || cfg.Tools.RateLimitBurst > 100 {
		errs = append(errs, ValidationError{
			Field:        "tools.rate_limit_burst",
			ExpectedType: "integer between 0 and 100",
			ActualValue:  fmt.Sprintf("%d", cfg.Tools.RateLimitBurst),
		})
	}
	if cfg.Tools.RateLimitPerSec < 0 || cfg.Tools.RateLimitPerSec > 50 {
		errs = append(errs, ValidationError{
			Field:        "tools.rate_limit_per_sec",
			ExpectedType: "integer between 0 and 50",
			ActualValue:  fmt.Sprintf("%d", cfg.Tools.RateLimitPerSec),
		})
	}

	// T-07-02: Concurrency bounds — prevent resource exhaustion via config.
	if cfg.Tools.MaxConcurrent < 0 || cfg.Tools.MaxConcurrent > 32 {
		errs = append(errs, ValidationError{
			Field:        "tools.max_concurrent",
			ExpectedType: "integer between 0 and 32",
			ActualValue:  fmt.Sprintf("%d", cfg.Tools.MaxConcurrent),
		})
	}
	if cfg.Tools.MaxToolConcurrency < 0 || cfg.Tools.MaxToolConcurrency > 16 {
		errs = append(errs, ValidationError{
			Field:        "tools.max_tool_concurrency",
			ExpectedType: "integer between 0 and 16",
			ActualValue:  fmt.Sprintf("%d", cfg.Tools.MaxToolConcurrency),
		})
	}

	// T-07-04: Retry bounds — prevent infinite retry loops via config.
	if cfg.Features.RetryMaxAttempts < 0 || cfg.Features.RetryMaxAttempts > 10 {
		errs = append(errs, ValidationError{
			Field:        "features.retry_max_attempts",
			ExpectedType: "integer between 0 and 10",
			ActualValue:  fmt.Sprintf("%d", cfg.Features.RetryMaxAttempts),
		})
	}
	if cfg.Features.RetryMaxDelayMs < 0 || (cfg.Features.RetryMaxDelayMs > 0 && cfg.Features.RetryMaxDelayMs < 1000) || cfg.Features.RetryMaxDelayMs > 300000 {
		errs = append(errs, ValidationError{
			Field:        "features.retry_max_delay_ms",
			ExpectedType: "integer between 0, or 1000 and 300000",
			ActualValue:  fmt.Sprintf("%d", cfg.Features.RetryMaxDelayMs),
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

// knownConfigKeys returns the set of known top-level TOML keys for warning
// on unknown keys. This is a best-effort set; sub-keys are not checked.
// Cached with sync.Once to avoid repeated map allocations.
var (
	knownKeysOnce sync.Once
	knownKeysMap  map[string]bool
)

func knownConfigKeys() map[string]bool {
	knownKeysOnce.Do(func() {
		knownKeysMap = map[string]bool{
			"provider": true, "model": true, "ui": true, "permissions": true,
			"features": true, "tools": true, "git": true, "ledger": true,
			"agents": true, "verify": true, "compaction": true, "instructions": true, "skills": true,
			"model_capabilities": true,
			// Common typos / sub-tables that appear in user configs
			"openrouter": true, "zen": true, "nvidia": true,
		}
	})
	return knownKeysMap
}

var varRe = regexp.MustCompile(`\$\{([^}]+)\}`)

// applyVarSubstitution walks all string fields in cfg and replaces ${VAR}
// patterns with their corresponding environment variable values.
// Returns a list of unresolved variable names (still containing ${...} patterns).
func applyVarSubstitution(cfg *Config) []string {
	var unresolved []string
	unresolved = append(unresolved, substituteVarsReport(&cfg.Provider.Default, "provider.default")...)
	unresolved = append(unresolved, substituteVarsReport(&cfg.Provider.OpenRouter.APIKey, "provider.openrouter.api_key")...)
	unresolved = append(unresolved, substituteVarsReport(&cfg.Provider.Zen.APIKey, "provider.zen.api_key")...)
	unresolved = append(unresolved, substituteVarsReport(&cfg.Model.Default, "model.default")...)
	unresolved = append(unresolved, substituteVarsReport(&cfg.UI.Theme, "ui.theme")...)
	unresolved = append(unresolved, substituteVarsReport(&cfg.Permissions.DefaultMode, "permissions.default_mode")...)
	unresolved = append(unresolved, substituteVarsReport(&cfg.Tools.WebSearchBaseURL, "tools.websearch_base_url")...)

	for i := range cfg.Permissions.Rules {
		unresolved = append(unresolved, substituteVarsReport(&cfg.Permissions.Rules[i].Tool, fmt.Sprintf("permissions.rules[%d].tool", i))...)
		unresolved = append(unresolved, substituteVarsReport(&cfg.Permissions.Rules[i].Pattern, fmt.Sprintf("permissions.rules[%d].pattern", i))...)
		unresolved = append(unresolved, substituteVarsReport(&cfg.Permissions.Rules[i].Action, fmt.Sprintf("permissions.rules[%d].action", i))...)
	}

	for name := range cfg.Permissions.Agents {
		agent := cfg.Permissions.Agents[name]
		unresolved = append(unresolved, substituteVarsReport(&agent.DefaultAction, fmt.Sprintf("permissions.agents.%s.default_action", name))...)
		for j := range agent.Rules {
			unresolved = append(unresolved, substituteVarsReport(&agent.Rules[j].Tool, fmt.Sprintf("permissions.agents.%s.rules[%d].tool", name, j))...)
			unresolved = append(unresolved, substituteVarsReport(&agent.Rules[j].Pattern, fmt.Sprintf("permissions.agents.%s.rules[%d].pattern", name, j))...)
			unresolved = append(unresolved, substituteVarsReport(&agent.Rules[j].Action, fmt.Sprintf("permissions.agents.%s.rules[%d].action", name, j))...)
		}
		cfg.Permissions.Agents[name] = agent
	}
	return unresolved
}

// substituteVarsReport replaces ${VAR} patterns and returns a list of
// unresolved variable names for the given field.
func substituteVarsReport(field *string, fieldName string) []string {
	if *field == "" || !strings.Contains(*field, "${") {
		return nil
	}
	var unresolved []string
	*field = varRe.ReplaceAllStringFunc(*field, func(match string) string {
		name := match[2 : len(match)-1]
		if val, ok := os.LookupEnv(name); ok {
			return val
		}
		slog.Warn("unresolved variable in config, preserving pattern", "variable", name, "field", fieldName)
		unresolved = append(unresolved, fmt.Sprintf("%s references unset variable ${%s}", fieldName, name))
		return match
	})
	return unresolved
}

// substituteVars replaces ${VAR} patterns in s with the value of the
// environment variable VAR. If the variable is not set, the original ${VAR}
// pattern is preserved as-is and a warning is logged.
func substituteVars(s string) string {
	if s == "" || !strings.Contains(s, "${") {
		return s
	}
	return varRe.ReplaceAllStringFunc(s, func(match string) string {
		name := match[2 : len(match)-1]
		if val, ok := os.LookupEnv(name); ok {
			return val
		}
		slog.Warn("unresolved variable in config, preserving pattern", "variable", name, "hint", "set the environment variable or replace the ${...} with a literal value")
		return match
	})
}

// Save writes the config to a TOML file atomically (temp file + rename).
// If M31A_CONFIG env var is set, it is used as the path instead.
// API keys are never persisted to the config file — they must be set via
// environment variables or the OS keychain.
func (c *Config) Save(path string) error {
	return c.SaveWithKeychain(path, nil)
}

// SaveWithKeychain writes the config to a TOML file atomically.
// If a keychain is provided and available, API keys are saved to the keychain
// and cleared from the config file. If the keychain is unavailable or nil,
// API keys are persisted to the config file as a fallback (with a warning).
func (c *Config) SaveWithKeychain(path string, kc keychain.Keychain) error {
	// M31A_CONFIG env var overrides path
	if envPath := os.Getenv("M31A_CONFIG"); envPath != "" {
		path = envPath
	}

	// Copy the config to avoid mutating the original
	cfgCopy := *c

	// H-12: Deep-copy slice and map fields to prevent data races with WatchConfig
	if c.Permissions.Rules != nil {
		rulesCopy := make([]PermissionRule, len(c.Permissions.Rules))
		copy(rulesCopy, c.Permissions.Rules)
		cfgCopy.Permissions.Rules = rulesCopy
	}
	if c.Permissions.Agents != nil {
		agentsCopy := make(map[string]PermissionsAgentConfig, len(c.Permissions.Agents))
		for k, v := range c.Permissions.Agents {
			agentRulesCopy := make([]PermissionRule, len(v.Rules))
			copy(agentRulesCopy, v.Rules)
			v.Rules = agentRulesCopy
			agentsCopy[k] = v
		}
		cfgCopy.Permissions.Agents = agentsCopy
	}
	if c.Tools.SkipDirs != nil {
		skipDirsCopy := make([]string, len(c.Tools.SkipDirs))
		copy(skipDirsCopy, c.Tools.SkipDirs)
		cfgCopy.Tools.SkipDirs = skipDirsCopy
	}

	// Determine if we should persist API keys to the config file.
	// We persist keys if:
	// 1. No keychain is provided, OR
	// 2. Keychain is provided but unavailable (ErrKeychainUnavailable)
	// We clear keys from the config file only if keychain is available and saves succeed.
	persistKeys := true
	if kc != nil {
		// Try to save keys to keychain to test availability
		openRouterKey := c.Provider.OpenRouter.APIKey
		zenKey := c.Provider.Zen.APIKey
		nvidiaKey := c.Provider.Nvidia.APIKey
		openRouterSaved := openRouterKey == ""
		zenSaved := zenKey == ""
		nvidiaSaved := nvidiaKey == ""

		if openRouterKey != "" {
			if err := kc.Set("openrouter", openRouterKey); err == nil {
				openRouterSaved = true
			} else if errors.Is(err, keychain.ErrKeychainUnavailable) {
				// Keychain unavailable - will persist to config file
			} else {
				slog.Warn("failed to save OpenRouter key to keychain", "error", err)
			}
		}
		if zenKey != "" {
			if err := kc.Set("zen", zenKey); err == nil {
				zenSaved = true
			} else if errors.Is(err, keychain.ErrKeychainUnavailable) {
				// Keychain unavailable - will persist to config file
			} else {
				slog.Warn("failed to save Zen key to keychain", "error", err)
			}
		}
		if nvidiaKey != "" {
			if err := kc.Set("nvidia", nvidiaKey); err == nil {
				nvidiaSaved = true
			} else if errors.Is(err, keychain.ErrKeychainUnavailable) {
				// Keychain unavailable - will persist to config file
			} else {
				slog.Warn("failed to save NVIDIA key to keychain", "error", err)
			}
		}

		// Clear keys from config file only for providers that were successfully
		// saved to keychain. Providers that failed keychain storage keep their
		// keys in the config file as fallback.
		if openRouterSaved {
			cfgCopy.Provider.OpenRouter.APIKey = ""
		}
		if zenSaved {
			cfgCopy.Provider.Zen.APIKey = ""
		}
		if nvidiaSaved {
			cfgCopy.Provider.Nvidia.APIKey = ""
		}
		if openRouterSaved && zenSaved && nvidiaSaved {
			persistKeys = false
		} else {
			slog.Warn("keychain unavailable or save failed for some providers; persisting API keys to config file as fallback")
		}
	}

	if persistKeys {
		// Keys will be persisted to config file as fallback
		slog.Info("API keys will be stored in config file (keychain unavailable)")
	}

	data, err := toml.Marshal(&cfgCopy)
	if err != nil {
		return fmt.Errorf("marshal config: %w", err)
	}

	// Ensure parent directory exists
	if err := os.MkdirAll(filepath.Dir(path), types.DirPermission); err != nil {
		return fmt.Errorf("create config dir: %w", err)
	}

	// Atomic write: write to temp file in same directory, then rename
	if err := fileutil.AtomicWrite(path, data); err != nil {
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
	} else if key := os.Getenv("OPENROUTER_API_KEY"); key != "" {
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
	} else if key := os.Getenv("ZEN_API_KEY"); key != "" {
		c.Provider.Zen.APIKey = key
	} else if kc != nil {
		if k, err := kc.Get("zen"); err == nil {
			c.Provider.Zen.APIKey = k
		} else if !errors.Is(err, keychain.ErrKeyNotFound) && !errors.Is(err, keychain.ErrKeychainUnavailable) {
			slog.Warn("keychain error", "provider", "zen", "error", err)
		}
	}

	// NVIDIA NIM
	if key := os.Getenv("M31A_NVIDIA_API_KEY"); key != "" {
		c.Provider.Nvidia.APIKey = key
	} else if key := os.Getenv("NVIDIA_API_KEY"); key != "" {
		c.Provider.Nvidia.APIKey = key
	} else if kc != nil {
		if k, err := kc.Get("nvidia"); err == nil {
			c.Provider.Nvidia.APIKey = k
		} else if !errors.Is(err, keychain.ErrKeyNotFound) && !errors.Is(err, keychain.ErrKeychainUnavailable) {
			slog.Warn("keychain error", "provider", "nvidia", "error", err)
		}
	}

	return nil
}

// ConfigReloadMsg is emitted when the config file changes on disk.
type ConfigReloadMsg struct {
	Config *Config
	Error  error
}

// sendReload loads the config and sends it on ch, retrying briefly if the
// receiver is busy. Blocks until delivered or ctx is cancelled — never drops
// the message silently (BUG-18).
func sendReload(ctx context.Context, ch chan<- ConfigReloadMsg, path string) {
	cfg, err := Load(path)
	msg := ConfigReloadMsg{Config: cfg, Error: err}
	select {
	case ch <- msg:
		return
	case <-ctx.Done():
		return
	case <-time.After(100 * time.Millisecond):
		// Receiver didn't take it within 100ms — block until it does or
		// the watcher is shut down. No silent drop.
		select {
		case ch <- msg:
		case <-ctx.Done():
			slog.Warn("config reload message not delivered: watcher cancelled")
		}
	}
}

// WatchConfig watches the config file for changes using fsnotify and sends
// ConfigReloadMsg to the provided channel when a change is detected.
// Falls back to polling with ConfigWatchInterval if fsnotify is unavailable.
// Runs until ctx is cancelled.
func WatchConfig(ctx context.Context, path string, ch chan<- ConfigReloadMsg) {
	watcher, err := fsnotify.NewWatcher()
	if err != nil {
		slog.Warn("fsnotify unavailable, falling back to config polling", "error", err)
		watchConfigPolling(ctx, path, ch)
		return
	}
	defer watcher.Close() //nolint:errcheck

	dir := filepath.Dir(path)
	base := filepath.Base(path)
	if err := watcher.Add(dir); err != nil {
		slog.Warn("fsnotify watch failed, falling back to config polling", "error", err)
		watchConfigPolling(ctx, path, ch)
		return
	}

	var debounce *time.Timer
	for {
		select {
		case <-ctx.Done():
			if debounce != nil {
				debounce.Stop()
			}
			return
		case event, ok := <-watcher.Events:
			if !ok {
				return
			}
			if filepath.Base(event.Name) != base {
				continue
			}
			if event.Op&(fsnotify.Write|fsnotify.Create|fsnotify.Rename) == 0 {
				continue
			}
			if debounce != nil {
				debounce.Stop()
			}
			debounce = time.AfterFunc(50*time.Millisecond, func() {
				sendReload(ctx, ch, path)
			})
		case err, ok := <-watcher.Errors:
			if !ok {
				return
			}
			slog.Warn("config watcher error", "error", err)
		}
	}
}

// watchConfigPolling is the fallback config watcher that polls the file's
// modification time every ConfigWatchInterval.
func watchConfigPolling(ctx context.Context, path string, ch chan<- ConfigReloadMsg) {
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
				sendReload(ctx, ch, path)
			}
		}
	}
}

// DefaultGitConfig returns the default git configuration.
func DefaultGitConfig() GitConfig {
	return GitConfig{
		CommitPrefix: "feat",
		FixPrefix:    "fix",
		ShipPrefix:   "chore",
		UserName:     "M31A",
		UserEmail:    "m31a@local",
	}
}

// LoadDotEnv reads a .env file from the current working directory and sets
// environment variables. Does not override already-set variables.
//
// This must be called before any goroutines that read os.Environ() are started
// (e.g., before logger initialization) because os.Setenv is not goroutine-safe.
// Guarded with sync.Once to prevent duplicate calls from main.go and Load().
var loadDotEnvOnce sync.Once

func LoadDotEnv() {
	loadDotEnvOnce.Do(func() {
		cwd, err := os.Getwd()
		if err != nil {
			return
		}
		envPath := filepath.Join(cwd, ".env")
		info, err := os.Stat(envPath)
		if err != nil {
			return
		}
		if info.Mode().Perm()&0o022 != 0 {
			slog.Warn("skipping group/world-writable .env file", "path", envPath)
			return
		}
		data, err := os.ReadFile(envPath)
		if err != nil {
			return
		}
		for _, line := range strings.Split(string(data), "\n") {
			line = strings.TrimSpace(line)
			if len(line) > 4096 {
				slog.Warn("skipping overly long .env line", "path", envPath)
				continue
			}
			if line == "" || strings.HasPrefix(line, "#") {
				continue
			}
			parts := strings.SplitN(line, "=", 2)
			if len(parts) != 2 {
				continue
			}
			key := strings.TrimSpace(parts[0])
			value := strings.TrimSpace(parts[1])
			// Remove surrounding quotes
			if len(value) >= 2 {
				if (value[0] == '"' && value[len(value)-1] == '"') ||
					(value[0] == '\'' && value[len(value)-1] == '\'') {
					value = value[1 : len(value)-1]
				}
			}
			if _, exists := os.LookupEnv(key); !exists {
				_ = os.Setenv(key, value)
			}
		}
	})
}
