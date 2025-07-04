package config

import "github.com/eshanized/M31A/internal/types"

type Config struct {
	Provider    ProviderConfig    `toml:"provider"`
	Model       ModelConfig       `toml:"model"`
	UI          UIConfig          `toml:"ui"`
	Permissions PermissionsConfig `toml:"permissions"`
	Features    FeaturesConfig    `toml:"features"`
	Ledger      LedgerConfig      `toml:"ledger"`
	Agents      AgentsConfig      `toml:"agents"`
}

type ProviderConfig struct {
	Default      string                   `toml:"default"`
	AutoFallback bool                     `toml:"auto_fallback"`
	OpenRouter   ProviderCredentialConfig `toml:"openrouter"`
	Zen          ProviderCredentialConfig `toml:"zen"`
	// Custom base URLs for self-hosted or proxied gateways.
	// Empty means use the default provider URLs.
	OpenRouterBaseURL string `toml:"openrouter_base_url"`
	ZenBaseURL        string `toml:"zen_base_url"`
	// HTTP-Referer and X-Title headers sent to OpenRouter.
	// Empty means use defaults ("https://github.com/eshanized/M31A", "M31A").
	OpenRouterReferer string `toml:"openrouter_referer"`
	OpenRouterTitle   string `toml:"openrouter_title"`
}

type ProviderCredentialConfig struct {
	APIKey string `toml:"api_key"`
}

type ModelConfig struct {
	Default                 string  `toml:"default"`
	ContextWarningThreshold float64 `toml:"context_warning_threshold"`
	ShowThinkingByDefault   bool    `toml:"show_thinking_by_default"`
	AutoCollapseTools       bool    `toml:"auto_collapse_tools"`
	AutoArbitrage           bool    `toml:"auto_arbitrage"`
	ArbitrageThreshold      float64 `toml:"arbitrage_threshold"`
	// Default context length used when provider doesn't return one.
	DefaultContextLength int `toml:"default_context_length"`
	// Token estimator EMA calibration rate (0.0–1.0). Lower = slower calibration.
	TokenEMAAlpha float64 `toml:"token_ema_alpha"`
}

type UIConfig struct {
	Theme            string `toml:"theme"`
	CompactMode      bool   `toml:"compact_mode"`
	ShowTokenUsage   bool   `toml:"show_token_usage"`
	ShowCostEstimate bool   `toml:"show_cost_estimate"`
	MaxIterations    int    `toml:"max_iterations"`
	// Leader key for chord shortcuts. Default "ctrl+x".
	LeaderKey string `toml:"leader_key"`
	// Leader key timeout duration in milliseconds. Default 1000.
	LeaderTimeoutMs int `toml:"leader_timeout_ms"`
	// Minimum terminal width before sidebar auto-shows. Default 120.
	SidebarWidthThreshold int `toml:"sidebar_width_threshold"`
}

type PermissionsConfig struct {
	DefaultMode    string                            `toml:"default_mode"`
	TimeoutSeconds int                               `toml:"timeout_seconds"`
	Rules          []PermissionRule                  `toml:"rules"`
	Agents         map[string]PermissionsAgentConfig `toml:"agents,omitempty"`
}

type PermissionRule struct {
	Tool      string          `toml:"tool"`
	Pattern   string          `toml:"pattern"`
	RiskLevel types.RiskLevel `toml:"risk_level"`
	Action    string          `toml:"action"`
}

// PermissionsAgentConfig defines per-agent permission profiles.
// Each agent (e.g., "build", "plan", "default") can have its own
// default action and ruleset.
type PermissionsAgentConfig struct {
	DefaultAction string           `toml:"default_action"`
	Rules         []PermissionRule `toml:"rules"`
}

type FeaturesConfig struct {
	AutoBackup      bool `toml:"auto_backup"`
	ResumeOnStartup bool `toml:"resume_on_startup"`
	// Model cache TTL in minutes. Default 5.
	ModelCacheTTLMinutes int `toml:"model_cache_ttl_minutes"`
	// Stale cache TTL in hours. Default 24.
	ModelCacheStaleHours int `toml:"model_cache_stale_hours"`
	// Health check latency thresholds in milliseconds.
	HealthCheckLiveMs int `toml:"healthcheck_live_ms"`
	HealthCheckSlowMs int `toml:"healthcheck_slow_ms"`
	// Session ID length in hex chars. Default 8.
	SessionIDLength int `toml:"session_id_length"`
	// Max recent models to remember. Default 10.
	MaxRecentModels int `toml:"max_recent_models"`
}

type LedgerConfig struct {
	Enabled    bool `toml:"enabled"`
	MaxEntries int  `toml:"max_entries"`
}

// AgentsConfig defines per-workflow-phase model assignments.
// Each field names a workflow phase and holds a model ID string.
// Allows users to assign different models to different workflow phases
// (e.g., cheap model for Plan, powerful model for Execute).
type AgentsConfig struct {
	Default string `toml:"default"`
	Plan    string `toml:"plan"`
	Execute string `toml:"execute"`
	Verify  string `toml:"verify"`
	Ship    string `toml:"ship"`
	Discuss string `toml:"discuss"`
}
