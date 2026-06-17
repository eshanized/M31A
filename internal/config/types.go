package config

import "github.com/eshanized/M31A/internal/types"

type Config struct {
	Provider    ProviderConfig    `toml:"provider"`
	Model       ModelConfig       `toml:"model"`
	UI          UIConfig          `toml:"ui"`
	Permissions PermissionsConfig `toml:"permissions"`
	Features    FeaturesConfig    `toml:"features"`
	Ledger      LedgerConfig      `toml:"ledger"`
	Tools       ToolsConfig       `toml:"tools"`
	Agents      AgentsConfig      `toml:"agents"`
	Git         GitConfig         `toml:"git"`
	Verify      VerifyConfig      `toml:"verify"`
}

// GitConfig holds configurable git commit message prefixes and user identity.
type GitConfig struct {
	CommitPrefix string `toml:"commit_prefix"`
	FixPrefix    string `toml:"fix_prefix"`
	ShipPrefix   string `toml:"ship_prefix"`
	UserName     string `toml:"user_name"`
	UserEmail    string `toml:"user_email"`
}

// VerifyConfig holds configurable verification commands.
// Empty values trigger auto-detection (existing behavior).
type VerifyConfig struct {
	BuildCommand string `toml:"build_command"`
	TestCommand  string `toml:"test_command"`
}

type ProviderConfig struct {
	Default      string                   `toml:"default"`
	AutoFallback bool                     `toml:"auto_fallback"`
	OpenRouter   ProviderCredentialConfig `toml:"openrouter"`
	Zen          ProviderCredentialConfig `toml:"zen"`
	Nvidia       ProviderCredentialConfig `toml:"nvidia"`
	// Custom base URLs for self-hosted or proxied gateways.
	// Empty means use the default provider URLs.
	OpenRouterBaseURL string `toml:"openrouter_base_url"`
	ZenBaseURL        string `toml:"zen_base_url"`
	NvidiaBaseURL     string `toml:"nvidia_base_url"`
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
	// Discuss Q&A timeout in seconds. Default 300 (5 minutes).
	DiscussTimeout int `toml:"discuss_timeout"`
	// Max lines for thinking block content. Default 20.
	ThinkingMaxLines int `toml:"thinking_max_lines"`
	// Permission modal width in columns. Default 60.
	PermissionModalWidth int `toml:"permission_modal_width"`
	// Sidebar width in columns. Default 42.
	SidebarWidth int `toml:"sidebar_width"`
	// Max message history entries. Default 1000.
	MaxMessageHistory int `toml:"max_message_history"`
	// Fallback banner display duration in seconds. Default 15.
	FallbackBannerSecs int `toml:"fallback_banner_timeout_secs"`
	// Default number of log lines to show. Default 20.
	DefaultLogLines int `toml:"default_log_lines"`
	// Max sessions to list in resume screen. Default 10.
	SessionListLimit int `toml:"session_list_limit"`
	// Thinking block opacity. Default 0.6.
	ThinkingOpacity float64 `toml:"thinking_opacity"`
	// Frecent history max entries. Default 100.
	FrecentHistorySize int `toml:"frecent_history_size"`

	// New - Theme & Colors
	AccentColor      string `toml:"accent_color"`
	CustomBackground string `toml:"custom_background"`
	BorderStyle      string `toml:"border_style"`

	// New - Typography
	BoldHeaders    bool `toml:"bold_headers"`
	ItalicThinking bool `toml:"italic_thinking"`
	TabWidth       int  `toml:"tab_width"`

	// New - Layout
	SidebarPosition string `toml:"sidebar_position"`
	SidebarAutoShow bool   `toml:"sidebar_auto_show"`
	CardPadding     int    `toml:"card_padding"`
	WelcomeScreen   bool   `toml:"welcome_screen"`
	ZenModeKey      string `toml:"zen_mode_key"`

	// New - Animation
	AnimationSpeed   string `toml:"animation_speed"`
	SpinnerStyle     string `toml:"spinner_style"`
	TransitionStyle  string `toml:"transition_style"`
	BreathingEffects bool   `toml:"breathing_effects"`
	LogoAnimation    bool   `toml:"logo_animation"`

	// New - Status Bar
	StatusBarStyle      string `toml:"status_bar_style"`
	StatusBarPosition   string `toml:"status_bar_position"`
	ShowSpinnerInStatus bool   `toml:"show_spinner_in_status"`

	// New - Tool Cards
	ToolCardStyle      string `toml:"tool_card_style"`
	ToolOutputMaxLines int    `toml:"tool_output_max_lines"`
	SyntaxHighlight    bool   `toml:"syntax_highlight"`

	// New - Toasts
	ToastPosition     string `toml:"toast_position"`
	ToastDurationSecs int    `toml:"toast_duration_secs"`
	ToastMaxVisible   int    `toml:"toast_max_visible"`
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
	// WorkflowMode controls phase-skipping behaviour:
	//   "auto"   — classify and adapt automatically (default)
	//   "full"   — always run all 6 phases
	//   "fast"   — skip Plan phase (Discuss→Execute)
	//   "direct" — skip Discuss, Plan, Verify (only Initialize→Execute→Ship)
	// Empty string means auto.
	WorkflowMode string `toml:"workflow_mode"`
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
	// Session retention in days. Default 30.
	SessionRetentionDays int `toml:"session_retention_days"`
	// Health check timeout in seconds. Default 10.
	HealthCheckTimeoutSecs int `toml:"health_check_timeout_secs"`
	// Rate limit backoff in seconds. Default 120.
	RateLimitBackoffSecs int `toml:"rate_limit_backoff_secs"`
	// Optional per-session budget limit in USD. 0 means no limit.
	// When set, workflow phases check cumulative cost before proceeding.
	BudgetLimitUSD float64 `toml:"budget_limit_usd"`
}

type LedgerConfig struct {
	Enabled    bool `toml:"enabled"`
	MaxEntries int  `toml:"max_entries"`
}

// AgentsConfig defines per-workflow-phase model assignments.
// Each field names a workflow phase and holds a model ID string.
// Allows users to assign different models to different workflow phases
// (e.g., cheap model for Plan, powerful model for Execute).
// ToolsConfig defines configurable limits for tool execution.
type ToolsConfig struct {
	MaxGlobResults       int      `toml:"max_glob_results"`
	MaxGrepResults       int      `toml:"max_grep_results"`
	BashKillGraceSecs    int      `toml:"bash_kill_grace_secs"`
	MaxBackupsPerFile    int      `toml:"max_backups_per_file"`
	WebfetchMaxRedirects int      `toml:"webfetch_max_redirects"`
	WebfetchUserAgent    string   `toml:"webfetch_user_agent"`
	SkipDirs             []string `toml:"skip_dirs"`
	WebSearchBaseURL     string   `toml:"websearch_base_url"`
	WebSearchEnabled     bool     `toml:"websearch_enabled"`
}

type AgentsConfig struct {
	Default string `toml:"default"`
	Plan    string `toml:"plan"`
	Execute string `toml:"execute"`
	Verify  string `toml:"verify"`
	Ship    string `toml:"ship"`
	Discuss string `toml:"discuss"`
}
