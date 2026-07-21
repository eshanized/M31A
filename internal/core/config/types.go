package config

import (
	"time"

	"github.com/eshanized/M31A/internal/core/types"
)

type Config struct {
	Provider          ProviderConfig          `toml:"provider"`
	Model             ModelConfig             `toml:"model"`
	UI                UIConfig                `toml:"ui"`
	Permissions       PermissionsConfig       `toml:"permissions"`
	Features          FeaturesConfig          `toml:"features"`
	Ledger            LedgerConfig            `toml:"ledger"`
	Tools             ToolsConfig             `toml:"tools"`
	Agents            AgentsConfig            `toml:"agents"`
	Git               GitConfig               `toml:"git"`
	Verify            VerifyConfig            `toml:"verify"`
	Compaction        CompactionConfig        `toml:"compaction"`
	Instructions      InstructionsConfig      `toml:"instructions"`
	Skills            SkillsConfig            `toml:"skills"`
	ModelCapabilities ModelCapabilitiesConfig `toml:"model_capabilities"`
	Prompts           PromptConfig            `toml:"prompts"`
	Narrative         NarrativeConfig         `toml:"narrative"`
	Templates         TemplateConfig          `toml:"templates"`
}

// TemplateConfig holds template directory and website framework configuration.
type TemplateConfig struct {
	// External template directory (F-056): path to a directory containing
	// user-defined templates. Templates in this directory are merged with
	// embedded templates. If a template has the same name, the external one wins.
	ExternalDir string `toml:"external_dir"`

	// Website framework (F-005, F-057): which framework to use for website builds.
	// Options: "nextjs" (default), "vue", "svelte", "astro", "html"
	WebsiteFramework string `toml:"website_framework"`

	// Custom design palettes (F-058): map of palette name to color definitions.
	// Example: {"custom": {"primary": "#FF5733", "secondary": "#33FF57"}}
	CustomPalettes map[string]map[string]string `toml:"custom_palettes"`
}

// PromptConfig holds configurable prompt override settings.
// Prompt loading follows a 4-level priority chain:
//
//  1. Config override (prompts.overrides[name])
//  2. Project-level override (.m31a/prompts/<name>.md)
//  3. Global override (~/.m31a/prompts/<name>.md)
//  4. Embedded default (prompts/<name>.md from go:embed)
type PromptConfig struct {
	// SystemPromptFile is the path to a file that replaces the base system prompt.
	// If empty, uses embedded prompts/base.md.
	SystemPromptFile string `toml:"system_prompt_file"`

	// ProjectPromptDir is the path to a directory containing project-level prompt overrides.
	// Default: ".m31a/prompts/" relative to the project root.
	// If a file like "execute-task.md" exists in this directory, it overrides
	// the embedded version of that prompt.
	ProjectPromptDir string `toml:"project_prompt_dir"`

	// GlobalPromptDir is the path to a directory for user-wide prompt overrides.
	// Default: "" (resolved at runtime to ~/.m31a/prompts/).
	GlobalPromptDir string `toml:"global_prompt_dir"`

	// Overrides is a map of prompt name to file path for per-prompt overrides.
	// Example: {"execute-task": "/path/to/custom-execute.md"}
	Overrides map[string]string `toml:"overrides"`

	// ModelTemplateOverrides is a map of model ID prefix to template file path.
	// Example: {"mistral": "/path/to/mistral.txt", "deepseek": "/path/to/deepseek.txt"}
	ModelTemplateOverrides map[string]string `toml:"model_template_overrides"`
}

// CompactionConfig holds automatic session compaction settings.
type CompactionConfig struct {
	Auto       bool `toml:"auto"`
	Buffer     int  `toml:"buffer"`
	KeepTokens int  `toml:"keep_tokens"`

	// Proactive compaction settings (Wave 2B)
	Proactive          bool `toml:"proactive"`            // trigger compaction before phase transitions
	ToolCallsThreshold int  `toml:"tool_calls_threshold"` // check compaction every N tool calls during Execute
	PhaseTransitionPct int  `toml:"phase_transition_pct"` // trigger compaction at this % before phase transition

	// Custom summary template (F-063): inline template text that overrides
	// the embedded compaction summary template. Takes precedence over file.
	SummaryTemplate string `toml:"summary_template"`

	// Custom summary template file path: if set and SummaryTemplate is empty,
	// reads the template from this file. Falls back to embedded default on error.
	SummaryTemplateFile string `toml:"summary_template_file"`
}

// NarrativeConfig holds narrative system configuration (F-059, F-060).
type NarrativeConfig struct {
	// Template overrides: map of NarrativeType string to template text.
	// Overrides the embedded template for that type.
	// Example: {"task_complete": "Done! {description} completed successfully."}
	TemplateOverrides map[string]string `toml:"template_overrides"`

	// Classification overrides: map of EventType string to classification string.
	// Overrides the embedded classification for that event type.
	// Valid values: "narrative", "grouped", "hidden", "expanded"
	// Example: {"tool_call": "expanded"}
	ClassificationOverrides map[string]string `toml:"classification_overrides"`
}

// InstructionsConfig controls AGENTS.md file discovery for project-aware context.
type InstructionsConfig struct {
	Enabled        bool `toml:"enabled"`
	DisableProject bool `toml:"disable_project"`
}

// SkillsConfig controls skill discovery directories for composable slash commands.
type SkillsConfig struct {
	Sources []string `toml:"sources"`
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
	LintCommand  string `toml:"lint_command"`
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

	// Fallback priority (F-015): ordered list of provider names to try on failure.
	// Default: ["nvidia", "zen", "openrouter"] (matches alphabetical sort).
	FallbackPriority []string `toml:"fallback_priority"`

	// Health check timeout in seconds (F-016).
	// Default: 10 (current hardcoded value).
	HealthCheckTimeoutSecs int `toml:"health_check_timeout_secs"`

	// Provider registration order (F-013).
	// If set, providers are registered in this order.
	// Default: ["openrouter", "zen", "nvidia"] (current behavior).
	RegistrationOrder []string `toml:"registration_order"`
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

	// Accessibility
	ReducedMotion bool `toml:"reduced_motion"`

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

	// Layout thresholds (F-046)
	WidthUltraCompact int `toml:"width_ultra_compact"`
	WidthCompact      int `toml:"width_compact"`
	WidthFull         int `toml:"width_full"`

	// History (F-048, F-049)
	MaxMessages  int `toml:"max_messages"`
	MaxTodoItems int `toml:"max_todo_items"`

	// Sidebar (F-050)
	SidebarRefreshSecs int `toml:"sidebar_refresh_secs"`

	// Welcome screen (F-081, F-082)
	WelcomeTwoColThreshold int `toml:"welcome_two_col_threshold"`
	WelcomeCardMinWidth    int `toml:"welcome_card_min_width"`
	WelcomeCardMaxWidth    int `toml:"welcome_card_max_width"`

	// Logo (F-042): path to a custom logo file. If empty, uses embedded ASCII art.
	LogoFile string `toml:"logo_file"`

	// Logo text override: inline logo text. Takes precedence over LogoFile.
	LogoText string `toml:"logo_text"`

	// Welcome suggestions (F-043): custom welcome prompt suggestions.
	// If empty, uses the built-in defaults.
	WelcomeSuggestions []string `toml:"welcome_suggestions"`

	// Keyboard hints (F-044): custom keyboard hint strings.
	// If empty, uses the built-in defaults.
	KeyboardHints []string `toml:"keyboard_hints"`

	// Unicode symbol overrides (F-053): map of symbol name to replacement character.
	// Example: {"check": "✓", "cross": "✗", "warning": "⚠"}
	SymbolOverrides map[string]string `toml:"symbol_overrides"`

	// ASCII fallback mode: when true, uses ASCII characters instead of Unicode.
	// Useful for terminals with poor Unicode support.
	ASCIIFallback bool `toml:"ascii_fallback"`

	// Theme file (F-052): path to a custom theme file.
	// If empty, uses built-in dark/light/auto themes.
	ThemeFile string `toml:"theme_file"`

	// Toast type overrides (F-045): map of toast type to icon/title.
	// Example: {"success": {"icon": "OK", "title": "Done"}}
	ToastTypeOverrides map[string]ToastTypeConfig `toml:"toast_type_overrides"`
}

// ToastTypeConfig holds custom icon and title for a toast type.
type ToastTypeConfig struct {
	Icon  string `toml:"icon"`
	Title string `toml:"title"`
}

type PermissionsConfig struct {
	DefaultMode    string                            `toml:"default_mode"`
	TimeoutSeconds int                               `toml:"timeout_seconds"`
	Rules          []PermissionRule                  `toml:"rules"`
	Agents         map[string]PermissionsAgentConfig `toml:"agents,omitempty"`
}

type PermissionRule struct {
	Tool      string          `toml:"tool"`
	Resource  string          `toml:"resource"`
	Pattern   string          `toml:"pattern"`
	RiskLevel types.RiskLevel `toml:"risk_level"`
	Action    string          `toml:"action"`
	TTL       string          `toml:"ttl,omitempty"`        // e.g., "24h", "7d" (empty = permanent)
	CreatedAt string          `toml:"created_at,omitempty"` // RFC3339 timestamp
}

// IsExpired returns true if the permission rule has exceeded its TTL.
func (r PermissionRule) IsExpired() bool {
	if r.TTL == "" || r.CreatedAt == "" {
		return false // No TTL or no creation time = permanent
	}
	duration, err := time.ParseDuration(r.TTL)
	if err != nil {
		return false // Invalid TTL = treat as permanent
	}
	created, err := time.Parse(time.RFC3339, r.CreatedAt)
	if err != nil {
		return false // Invalid timestamp = treat as permanent
	}
	return time.Now().After(created.Add(duration))
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

	// Metrics collection (observability pipeline). Enabled by default.
	// When true, the session records tool execution, LLM token usage,
	// and workflow phase metrics to METRICS.json in the session directory.
	MetricsEnabled bool `toml:"metrics_enabled"`

	// Plan enhancements (GSD-inspired sub-steps within the Plan phase)
	PlanResearch       bool `toml:"plan_research"`        // Pre-plan research step
	PlanCheck          bool `toml:"plan_check"`           // Plan quality checker + revision loop
	PlanCheckMaxIter   int  `toml:"plan_check_max_iter"`  // Max revision iterations (default 3)
	PlanSecurityGate   bool `toml:"plan_security_gate"`   // Security heuristic gate
	PlanCoverageGate   bool `toml:"plan_coverage_gate"`   // Requirements coverage gate
	PlanGapAnalysis    bool `toml:"plan_gap_analysis"`    // Post-plan gap analysis
	PlanChunked        bool `toml:"plan_chunked"`         // Chunked plan generation
	PlanChunkThreshold int  `toml:"plan_chunk_threshold"` // Tasks threshold for auto-chunking (default 10)

	// Discuss phase enhancements
	DiscussQualityCheck bool `toml:"discuss_quality_check"` // Question quality checker
	DiscussCompleteness bool `toml:"discuss_completeness"`  // Answer completeness check
	DiscussFollowUps    bool `toml:"discuss_follow_ups"`    // Follow-up question generation

	// Execute phase enhancements
	ExecutePreflight   bool `toml:"execute_preflight"`    // Pre-execution validation
	ExecuteQualityGate bool `toml:"execute_quality_gate"` // Per-task acceptance criteria checks
	ExecuteLoopDetect  bool `toml:"execute_loop_detect"`  // Tool call loop detection

	// Verify + Ship phase enhancements
	VerifyReport   bool `toml:"verify_report"`   // Generate verification report
	VerifySecurity bool `toml:"verify_security"` // Security file scanning
	ShipPreflight  bool `toml:"ship_preflight"`  // Pre-ship checklist
	ShipChangelog  bool `toml:"ship_changelog"`  // Changelog generation

	// Initialize phase enhancements
	InitDeepAnalysis bool `toml:"init_deep_analysis"` // Deep project analysis
	InitPreflight    bool `toml:"init_preflight"`     // Environment pre-flight checks

	// Intent classification (LLM-based prompt routing)
	IntentClassification      bool `toml:"intent_classification"`        // Pre-classify REPL input for routing
	IntentClassifyTimeoutSecs int  `toml:"intent_classify_timeout_secs"` // Timeout for LLM classification (seconds). Default 25.

	// Workflow thresholds (F-030, F-031)
	MaxHealAttempts int `toml:"max_heal_attempts"`
	MaxPlanRetries  int `toml:"max_plan_retries"`

	// Context (F-033)
	ContextTruncationThreshold float64 `toml:"context_truncation_threshold"`

	// Retry policy (F-061)
	RetryMaxAttempts       int     `toml:"retry_max_attempts"`
	RetryBaseDelayMs       int     `toml:"retry_base_delay_ms"`
	RetryMaxDelayMs        int     `toml:"retry_max_delay_ms"`
	RetryBackoffMultiplier float64 `toml:"retry_backoff_multiplier"`

	// Retry-after (F-062)
	MaxRetryAfterSecs int `toml:"max_retry_after_secs"`

	// Task runner (F-076)
	MaxParallelTasks int `toml:"max_parallel_tasks"`

	// Coordinator (F-078)
	CoordinatorTimeoutSecs int `toml:"coordinator_timeout_secs"`
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
	OutputMaxLines       int      `toml:"output_max_lines"`
	OutputMaxBytes       int      `toml:"output_max_bytes"`

	// Rate limiting (F-018)
	RateLimitBurst           int `toml:"rate_limit_burst"`
	RateLimitPerSec          int `toml:"rate_limit_per_sec"`
	DangerousRateLimitBurst  int `toml:"dangerous_rate_limit_burst"`
	DangerousRateLimitPerSec int `toml:"dangerous_rate_limit_per_sec"`
	MaxConcurrent            int `toml:"max_concurrent"`

	// Output bounds (F-019)
	OutputRetentionDays int `toml:"output_retention_days"`

	// DNS (F-023)
	DnsCacheTTLSecs int `toml:"dns_cache_ttl_secs"`

	// Edit tool (F-024)
	FuzzyThreshold   float64 `toml:"fuzzy_threshold"`
	MinLinesForFuzzy int     `toml:"min_lines_for_fuzzy"`

	// Bash (F-020)
	BashMaxTimeoutSecs int `toml:"bash_max_timeout_secs"`

	// WebFetch (F-021)
	WebfetchMaxRetries   int `toml:"webfetch_max_retries"`
	WebfetchRetryDelayMs int `toml:"webfetch_retry_delay_ms"`

	// Execute phase (F-086, F-087)
	MaxToolConcurrency int `toml:"max_tool_concurrency"`
	LoopDetectWindow   int `toml:"loop_detect_window"`

	// Dangerous command extensions (F-017): user-defined patterns appended to the
	// compiled security baseline. These are regex patterns matched against bash input.
	// The compiled baseline is never bypassable via config.
	// Example: ["docker rm", "kubectl delete", "terraform destroy"]
	AdditionalBlockedCommands []string `toml:"additional_blocked_commands"`

	// Additional obfuscation patterns: user-defined patterns appended to the
	// compiled obfuscation detection list.
	AdditionalObfuscationPatterns []string `toml:"additional_obfuscation_patterns"`
}

type AgentsConfig struct {
	Default    string `toml:"default"`
	Initialize string `toml:"initialize"`
	Research   string `toml:"research"`
	Plan       string `toml:"plan"`
	Execute    string `toml:"execute"`
	Verify     string `toml:"verify"`
	Runtime    string `toml:"runtime"`
	Ship       string `toml:"ship"`
	Discuss    string `toml:"discuss"`

	// Profiles holds user-configurable overrides for subagent profiles.
	// Keys are profile names (e.g., "explore", "general", "security").
	// Built-in profiles can be overridden; new profiles can be added.
	Profiles map[string]SubagentProfileConfig `toml:"profiles,omitempty"`
}

// SubagentProfileConfig defines user-configurable overrides for a subagent
// profile. Zero-value fields inherit from the built-in default.
type SubagentProfileConfig struct {
	Description  string   `toml:"description,omitempty"`
	Mode         string   `toml:"mode,omitempty"`
	SystemPrompt string   `toml:"system_prompt,omitempty"`
	Model        string   `toml:"model,omitempty"`
	Hidden       *bool    `toml:"hidden,omitempty"`
	AllowedTools []string `toml:"allowed_tools,omitempty"`
	DeniedTools  []string `toml:"denied_tools,omitempty"`
	MaxTools     int      `toml:"max_tools,omitempty"`
	MaxTokens    int      `toml:"max_tokens,omitempty"`
	MaxTurns     int      `toml:"max_turns,omitempty"`
	Disabled     bool     `toml:"disabled,omitempty"`
}

// ModelCapabilitiesConfig holds user-configurable overrides for model capability
// detection. All fields are additive — they extend the built-in patterns, never
// replace them. When empty, the built-in behavior is preserved.
type ModelCapabilitiesConfig struct {
	// Extra reasoning patterns appended to the built-in list (F-011).
	ExtraReasoningPatterns []string `toml:"extra_reasoning_patterns"`

	// Extra tool-capable patterns appended to the built-in list (F-011).
	ExtraToolCapablePatterns []string `toml:"extra_tool_capable_patterns"`

	// Extra completion-only patterns appended to the built-in list (F-011).
	ExtraCompletionOnlyPatterns []string `toml:"extra_completion_only_patterns"`

	// Extra non-chat patterns appended to the built-in list (F-011).
	ExtraNonChatPatterns []string `toml:"extra_non_chat_patterns"`

	// Known model capability overrides merged with the built-in map (F-012).
	// Key: model ID pattern, Value: capability settings.
	KnownCapabilities map[string]ModelCapabilityOverride `toml:"known_capabilities"`
}

// ModelCapabilityOverride defines a single model capability entry that overrides
// or extends the built-in knownModelCapabilities table.
type ModelCapabilityOverride struct {
	ContextLength     int  `toml:"context_length"`
	MaxOutput         int  `toml:"max_output"`
	SupportsTools     bool `toml:"supports_tools"`
	SupportsReasoning bool `toml:"supports_reasoning"`
}
