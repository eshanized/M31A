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
}

type UIConfig struct {
	Theme           string `toml:"theme"`
	CompactMode     bool   `toml:"compact_mode"`
	ShowTokenUsage  bool   `toml:"show_token_usage"`
	ShowCostEstimate bool  `toml:"show_cost_estimate"`
	MaxIterations   int    `toml:"max_iterations"`
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
	AutoBackup       bool `toml:"auto_backup"`
	ResumeOnStartup  bool `toml:"resume_on_startup"`
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
