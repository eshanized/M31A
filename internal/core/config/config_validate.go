package config

import (
	"fmt"
	"log/slog"
	"os"
	"regexp"
	"strings"
	"sync"
	"time"

	m31types "github.com/eshanized/M31A/internal/core/types"
)

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
	if len(cfg.Provider.FallbackPriority) > 0 {
		validProviders := map[string]bool{
			m31types.ProviderOpenRouter: true,
			m31types.ProviderZen:        true,
			m31types.ProviderNvidia:     true,
		}
		for i, name := range cfg.Provider.FallbackPriority {
			if !validProviders[name] {
				errs = append(errs, ValidationError{
					Field:        fmt.Sprintf("provider.fallback_priority[%d]", i),
					ExpectedType: fmt.Sprintf("one of: %s, %s, %s", m31types.ProviderOpenRouter, m31types.ProviderZen, m31types.ProviderNvidia),
					ActualValue:  name,
				})
			}
		}
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
	// Validate extensions configuration
	if err := validateExtensionsConfig(cfg); err != nil {
		return err
	}
	return nil
}

// validateExtensionsConfig validates the extensions configuration section.
func validateExtensionsConfig(cfg *Config) error {
	var errs []error

	// Validate tool configurations
	for name, tool := range cfg.Extensions.Tools {
		if tool.Command == "" {
			errs = append(errs, ValidationError{
				Field:        fmt.Sprintf("extensions.tools.%s.command", name),
				ExpectedType: "non-empty string (executable path)",
				ActualValue:  "",
			})
		}
		if tool.Timeout != "" {
			if _, err := time.ParseDuration(tool.Timeout); err != nil {
				errs = append(errs, ValidationError{
					Field:        fmt.Sprintf("extensions.tools.%s.timeout", name),
					ExpectedType: "valid duration (e.g., \"30s\", \"5m\")",
					ActualValue:  tool.Timeout,
				})
			}
		}
		// Check if command is absolute or resolvable via PATH (basic check)
		if tool.Command != "" && !strings.HasPrefix(tool.Command, "/") && !strings.Contains(tool.Command, "/") {
			// Could be a command in PATH, that's acceptable
		}
	}

	// Validate provider configurations
	for name, provider := range cfg.Extensions.Providers {
		if provider.Command == "" {
			errs = append(errs, ValidationError{
				Field:        fmt.Sprintf("extensions.providers.%s.command", name),
				ExpectedType: "non-empty string (executable path)",
				ActualValue:  "",
			})
		}
		if provider.Timeout != "" {
			if _, err := time.ParseDuration(provider.Timeout); err != nil {
				errs = append(errs, ValidationError{
					Field:        fmt.Sprintf("extensions.providers.%s.timeout", name),
					ExpectedType: "valid duration (e.g., \"120s\", \"5m\")",
					ActualValue:  provider.Timeout,
				})
			}
		}
	}

	// Validate hook configurations
	validPhases := map[string]bool{
		"initialize": true, "discuss": true, "plan": true, "execute": true,
		"verify": true, "runtime": true, "ship": true,
	}
	validHookTypes := map[string]bool{
		"pre": true, "post": true,
	}
	for name, hook := range cfg.Extensions.Hooks {
		if hook.Command == "" {
			errs = append(errs, ValidationError{
				Field:        fmt.Sprintf("extensions.hooks.%s.command", name),
				ExpectedType: "non-empty string (executable path)",
				ActualValue:  "",
			})
		}
		if hook.Timeout != "" {
			if d, err := time.ParseDuration(hook.Timeout); err != nil {
				errs = append(errs, ValidationError{
					Field:        fmt.Sprintf("extensions.hooks.%s.timeout", name),
					ExpectedType: "valid duration (e.g., \"30s\", \"5m\")",
					ActualValue:  hook.Timeout,
				})
			} else if d > 5*time.Minute {
				errs = append(errs, ValidationError{
					Field:        fmt.Sprintf("extensions.hooks.%s.timeout", name),
					ExpectedType: "duration <= 5m",
					ActualValue:  hook.Timeout,
				})
			}
		}
		if len(hook.Phases) == 0 {
			errs = append(errs, ValidationError{
				Field:        fmt.Sprintf("extensions.hooks.%s.phases", name),
				ExpectedType: "non-empty array of phase names",
				ActualValue:  "empty",
			})
		} else {
			for _, phase := range hook.Phases {
				if !validPhases[phase] {
					errs = append(errs, ValidationError{
						Field:        fmt.Sprintf("extensions.hooks.%s.phases", name),
						ExpectedType: "one of: initialize, discuss, plan, execute, verify, runtime, ship",
						ActualValue:  phase,
					})
				}
			}
		}
		if len(hook.HookTypes) == 0 {
			errs = append(errs, ValidationError{
				Field:        fmt.Sprintf("extensions.hooks.%s.hook_types", name),
				ExpectedType: "non-empty array (\"pre\" and/or \"post\")",
				ActualValue:  "empty",
			})
		} else {
			for _, ht := range hook.HookTypes {
				if !validHookTypes[ht] {
					errs = append(errs, ValidationError{
						Field:        fmt.Sprintf("extensions.hooks.%s.hook_types", name),
						ExpectedType: "\"pre\" or \"post\"",
						ActualValue:  ht,
					})
				}
			}
		}
	}

	if len(errs) > 0 {
		var b strings.Builder
		b.WriteString("Invalid extensions configuration:\n")
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
			"model_capabilities": true, "prompts": true, "narrative": true, "templates": true,
			"extensions": true,
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

	// Git fields
	unresolved = append(unresolved, substituteVarsReport(&cfg.Git.UserName, "git.user_name")...)
	unresolved = append(unresolved, substituteVarsReport(&cfg.Git.UserEmail, "git.user_email")...)
	unresolved = append(unresolved, substituteVarsReport(&cfg.Git.CommitPrefix, "git.commit_prefix")...)
	unresolved = append(unresolved, substituteVarsReport(&cfg.Git.FixPrefix, "git.fix_prefix")...)
	unresolved = append(unresolved, substituteVarsReport(&cfg.Git.ShipPrefix, "git.ship_prefix")...)

	// Compaction fields
	unresolved = append(unresolved, substituteVarsReport(&cfg.Compaction.SummaryTemplate, "compaction.summary_template")...)
	unresolved = append(unresolved, substituteVarsReport(&cfg.Compaction.SummaryTemplateFile, "compaction.summary_template_file")...)

	// Prompt fields
	unresolved = append(unresolved, substituteVarsReport(&cfg.Prompts.SystemPromptFile, "prompts.system_prompt_file")...)
	unresolved = append(unresolved, substituteVarsReport(&cfg.Prompts.ProjectPromptDir, "prompts.project_prompt_dir")...)
	unresolved = append(unresolved, substituteVarsReport(&cfg.Prompts.GlobalPromptDir, "prompts.global_prompt_dir")...)

	// Template fields
	unresolved = append(unresolved, substituteVarsReport(&cfg.Templates.ExternalDir, "templates.external_dir")...)
	unresolved = append(unresolved, substituteVarsReport(&cfg.Templates.WebsiteFramework, "templates.website_framework")...)

	// Verify fields
	unresolved = append(unresolved, substituteVarsReport(&cfg.Verify.LintCommand, "verify.lint_command")...)

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
