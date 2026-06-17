package tui

import (
	"fmt"
	"log/slog"
	"strconv"
	"strings"
	"time"

	"github.com/charmbracelet/bubbles/textinput"
	"github.com/charmbracelet/bubbles/viewport"
	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/config"
	"github.com/eshanized/M31A/internal/tui/theme"
	"github.com/eshanized/M31A/pkg/keychain"
)

// ─── Field types ──────────────────────────────────────────────────────────────

type cfgFieldType int

const (
	cfgText     cfgFieldType = iota // free-form string
	cfgPassword                     // masked string
	cfgBool                         // yes/no toggle
	cfgNumber                       // integer
	cfgFloat                        // float64
	cfgChoice                       // one of a fixed set
	cfgReadOnly                     // display only (e.g. API key masked)
)

// cfgField describes one editable row.
type cfgField struct {
	key       string
	label     string
	fieldType cfgFieldType
	choices   []string // only for cfgChoice
	hint      string   // short help shown when selected
}

// cfgSection groups fields under a TOML section header.
type cfgSection struct {
	title  string
	fields []cfgField
}

// ─── ConfigSavedMsg ───────────────────────────────────────────────────────────

// ConfigSavedMsg is emitted when the config editor writes to disk.
type ConfigSavedMsg struct{}

// ─── ConfigModel ──────────────────────────────────────────────────────────────

// ConfigModel is the full interactive configuration editor for ScreenConfig.
// Every field from every section of config.Config is editable inline.
// Pressing 's' saves directly to the TOML file without leaving the screen.
type ConfigModel struct {
	theme    theme.Theme
	cfg      *config.Config
	cfgPath  string
	width    int
	height   int
	viewport viewport.Model
	keychain keychain.Keychain

	// Navigation
	sections   []cfgSection
	sectionIdx int // which section is active
	fieldIdx   int // which field inside the section

	// Editing
	editing   bool
	editInput textinput.Model

	// Status
	dirty      bool
	statusMsg  string
	statusTime time.Time
	saveErr    string
}

// NewConfigModel creates a ConfigModel.
func NewConfigModel(t theme.Theme, cfg *config.Config, cfgPath string, w, h int, kc keychain.Keychain) *ConfigModel {
	ti := textinput.New()
	ti.CharLimit = 512
	// Width set relative to terminal; will be corrected on first WindowSizeMsg
	ti.Width = max(20, min(50, w-12))

	vpH := h - 8
	if vpH < 4 {
		vpH = 4
	}
	vpW := max(10, w-4)
	vp := viewport.New(vpW, vpH)
	vp.Style = lipgloss.NewStyle().PaddingLeft(0)

	m := &ConfigModel{
		theme:     t,
		cfg:       cfg,
		cfgPath:   cfgPath,
		width:     w,
		height:    h,
		editInput: ti,
		viewport:  vp,
		keychain:  kc,
	}
	m.buildSections()
	return m
}

// buildSections defines all sections and their fields in the same order as config.toml.
func (m *ConfigModel) buildSections() {
	m.sections = []cfgSection{
		{
			title: "Provider",
			fields: []cfgField{
				{key: "provider.default", label: "Default provider", fieldType: cfgChoice, choices: []string{"zen", "openrouter"}, hint: "Which provider M31A uses by default"},
				{key: "provider.auto_fallback", label: "Auto fallback", fieldType: cfgBool, hint: "Automatically switch provider on failure"},
				{key: "provider.openrouter_base_url", label: "OpenRouter base URL", fieldType: cfgText, hint: "Custom base URL (empty = default)"},
				{key: "provider.zen_base_url", label: "Zen base URL", fieldType: cfgText, hint: "Custom base URL (empty = default)"},
				{key: "provider.openrouter_referer", label: "OpenRouter referer header", fieldType: cfgText, hint: "HTTP-Referer sent to OpenRouter"},
				{key: "provider.openrouter_title", label: "OpenRouter title header", fieldType: cfgText, hint: "X-Title sent to OpenRouter"},
				{key: "provider.openrouter.api_key", label: "OpenRouter API key", fieldType: cfgPassword, hint: "Saved to keychain, not written to disk"},
				{key: "provider.zen.api_key", label: "Zen API key", fieldType: cfgPassword, hint: "Saved to keychain, not written to disk"},
			},
		},
		{
			title: "Model",
			fields: []cfgField{
				{key: "model.default", label: "Default model", fieldType: cfgText, hint: "Model ID used when none is selected"},
				{key: "model.context_warning_threshold", label: "Context warning threshold", fieldType: cfgFloat, hint: "0.0–1.0, fraction of context window before warning"},
				{key: "model.show_thinking_by_default", label: "Show thinking by default", fieldType: cfgBool, hint: "Expand thinking blocks automatically"},
				{key: "model.auto_collapse_tools", label: "Auto-collapse tools", fieldType: cfgBool, hint: "Collapse tool cards after completion"},
				{key: "model.auto_arbitrage", label: "Auto-arbitrage", fieldType: cfgBool, hint: "Automatically suggest cheaper model alternatives"},
				{key: "model.arbitrage_threshold", label: "Arbitrage threshold", fieldType: cfgFloat, hint: "Cost ratio that triggers arbitrage (0.0–1.0)"},
				{key: "model.default_context_length", label: "Default context length", fieldType: cfgNumber, hint: "Fallback when provider doesn't return context size"},
				{key: "model.token_ema_alpha", label: "Token EMA alpha", fieldType: cfgFloat, hint: "EMA calibration rate 0.0–1.0 (lower = slower)"},
			},
		},
		{
			title: "UI",
			fields: []cfgField{
				{key: "ui.theme", label: "Theme", fieldType: cfgChoice, choices: []string{"dark", "light", "auto"}, hint: "Terminal color scheme"},
				{key: "ui.compact_mode", label: "Compact mode", fieldType: cfgBool, hint: "Reduce spacing for dense terminals"},
				{key: "ui.show_token_usage", label: "Show token usage", fieldType: cfgBool, hint: "Display token count in status bar"},
				{key: "ui.show_cost_estimate", label: "Show cost estimate", fieldType: cfgBool, hint: "Display inferred cost in status bar"},
				{key: "ui.max_iterations", label: "Max iterations", fieldType: cfgNumber, hint: "Tool call limit per workflow phase"},
				{key: "ui.leader_key", label: "Leader key", fieldType: cfgText, hint: "Chord prefix key (e.g. ctrl+x)"},
				{key: "ui.leader_timeout_ms", label: "Leader timeout (ms)", fieldType: cfgNumber, hint: "Time to wait for chord after leader key"},
				{key: "ui.sidebar_width_threshold", label: "Sidebar width threshold", fieldType: cfgNumber, hint: "Min terminal width before sidebar auto-shows"},
				{key: "ui.discuss_timeout", label: "Discuss timeout (s)", fieldType: cfgNumber, hint: "Q&A timeout in seconds"},
				{key: "ui.thinking_max_lines", label: "Thinking max lines", fieldType: cfgNumber, hint: "Max lines shown in thinking block"},
				{key: "ui.permission_modal_width", label: "Permission modal width", fieldType: cfgNumber, hint: "Column width of permission prompts"},
				{key: "ui.sidebar_width", label: "Sidebar width", fieldType: cfgNumber, hint: "Sidebar column width"},
				{key: "ui.max_message_history", label: "Max message history", fieldType: cfgNumber, hint: "Messages kept in memory per session"},
				{key: "ui.fallback_banner_timeout_secs", label: "Fallback banner timeout (s)", fieldType: cfgNumber, hint: "How long fallback banner stays visible"},
				{key: "ui.default_log_lines", label: "Default log lines", fieldType: cfgNumber, hint: "Lines shown by /log command"},
				{key: "ui.session_list_limit", label: "Session list limit", fieldType: cfgNumber, hint: "Sessions shown in resume screen"},
				{key: "ui.thinking_opacity", label: "Thinking opacity", fieldType: cfgFloat, hint: "Opacity of thinking blocks (0.0–1.0)"},
				{key: "ui.frecent_history_size", label: "Frecent history size", fieldType: cfgNumber, hint: "Max entries in frecent input history"},
			},
		},
		{
			title: "Permissions",
			fields: []cfgField{
				{key: "permissions.default_mode", label: "Default mode", fieldType: cfgChoice, choices: []string{"prompt", "allow", "deny"}, hint: "How tool permission requests are handled"},
				{key: "permissions.timeout_seconds", label: "Timeout (s)", fieldType: cfgNumber, hint: "Auto-deny after N seconds (0 = no timeout)"},
			},
		},
		{
			title: "Features",
			fields: []cfgField{
				{key: "features.auto_backup", label: "Auto backup", fieldType: cfgBool, hint: "Backup files before editing"},
				{key: "features.resume_on_startup", label: "Resume on startup", fieldType: cfgBool, hint: "Auto-resume last session on launch"},
				{key: "features.model_cache_ttl_minutes", label: "Model cache TTL (min)", fieldType: cfgNumber, hint: "How long to cache model lists"},
				{key: "features.model_cache_stale_hours", label: "Model cache stale (h)", fieldType: cfgNumber, hint: "Serve stale cache up to this age"},
				{key: "features.healthcheck_live_ms", label: "Health check live (ms)", fieldType: cfgNumber, hint: "Latency below which provider is 'live'"},
				{key: "features.healthcheck_slow_ms", label: "Health check slow (ms)", fieldType: cfgNumber, hint: "Latency below which provider is 'slow'"},
				{key: "features.session_id_length", label: "Session ID length", fieldType: cfgNumber, hint: "Hex chars in session IDs (4–16)"},
				{key: "features.max_recent_models", label: "Max recent models", fieldType: cfgNumber, hint: "Models remembered in recent list"},
				{key: "features.session_retention_days", label: "Session retention (days)", fieldType: cfgNumber, hint: "Sessions older than this are pruned"},
				{key: "features.health_check_timeout_secs", label: "Health check timeout (s)", fieldType: cfgNumber, hint: "Health check request timeout"},
				{key: "features.rate_limit_backoff_secs", label: "Rate limit backoff (s)", fieldType: cfgNumber, hint: "Wait time after 429 response"},
				{key: "features.budget_limit_usd", label: "Budget limit (USD)", fieldType: cfgFloat, hint: "Per-session spend cap (0 = unlimited)"},
			},
		},
		{
			title: "Ledger",
			fields: []cfgField{
				{key: "ledger.enabled", label: "Enabled", fieldType: cfgBool, hint: "Write cross-session learning ledger"},
				{key: "ledger.max_entries", label: "Max entries", fieldType: cfgNumber, hint: "Maximum ledger entries (0 = unlimited)"},
			},
		},
		{
			title: "Tools",
			fields: []cfgField{
				{key: "tools.max_glob_results", label: "Max glob results", fieldType: cfgNumber, hint: "Glob tool result cap"},
				{key: "tools.max_grep_results", label: "Max grep results", fieldType: cfgNumber, hint: "Grep tool result cap"},
				{key: "tools.bash_kill_grace_secs", label: "Bash kill grace (s)", fieldType: cfgNumber, hint: "Grace period before force-killing bash"},
				{key: "tools.max_backups_per_file", label: "Max backups per file", fieldType: cfgNumber, hint: "Backup rotation limit per file"},
				{key: "tools.webfetch_max_redirects", label: "Webfetch max redirects", fieldType: cfgNumber, hint: "HTTP redirect follow limit"},
				{key: "tools.webfetch_user_agent", label: "Webfetch user agent", fieldType: cfgText, hint: "User-Agent header for WebFetch tool"},
			},
		},
		{
			title: "Agents",
			fields: []cfgField{
				{key: "agents.default", label: "Default model", fieldType: cfgText, hint: "Model override for all phases"},
				{key: "agents.plan", label: "Plan model", fieldType: cfgText, hint: "Model override for Plan phase"},
				{key: "agents.execute", label: "Execute model", fieldType: cfgText, hint: "Model override for Execute phase"},
				{key: "agents.verify", label: "Verify model", fieldType: cfgText, hint: "Model override for Verify phase"},
				{key: "agents.ship", label: "Ship model", fieldType: cfgText, hint: "Model override for Ship phase"},
				{key: "agents.discuss", label: "Discuss model", fieldType: cfgText, hint: "Model override for Discuss phase"},
			},
		},
		{
			title: "Git",
			fields: []cfgField{
				{key: "git.commit_prefix", label: "Commit prefix", fieldType: cfgText, hint: "Conventional commit type (feat, fix, chore…)"},
				{key: "git.fix_prefix", label: "Fix prefix", fieldType: cfgText, hint: "Prefix used for fix-phase commits"},
				{key: "git.ship_prefix", label: "Ship prefix", fieldType: cfgText, hint: "Prefix used for ship-phase commits"},
				{key: "git.user_name", label: "Git user name", fieldType: cfgText, hint: "Name used in M31A commits"},
				{key: "git.user_email", label: "Git user email", fieldType: cfgText, hint: "Email used in M31A commits"},
			},
		},
		{
			title: "Verify",
			fields: []cfgField{
				{key: "verify.build_command", label: "Build command", fieldType: cfgText, hint: "Custom build command (empty = auto-detect)"},
				{key: "verify.test_command", label: "Test command", fieldType: cfgText, hint: "Custom test command (empty = auto-detect)"},
			},
		},
	}
}

// ─── Getters / Setters ────────────────────────────────────────────────────────

// getFieldValue reads the current value of a field from cfg.
func (m *ConfigModel) getFieldValue(f cfgField) string {
	if m.cfg == nil {
		return ""
	}
	c := m.cfg
	switch f.key {
	// Provider
	case "provider.default":
		return c.Provider.Default
	case "provider.auto_fallback":
		return boolStr(c.Provider.AutoFallback)
	case "provider.openrouter_base_url":
		return c.Provider.OpenRouterBaseURL
	case "provider.zen_base_url":
		return c.Provider.ZenBaseURL
	case "provider.openrouter_referer":
		return c.Provider.OpenRouterReferer
	case "provider.openrouter_title":
		return c.Provider.OpenRouterTitle
	case "provider.openrouter.api_key":
		return c.Provider.OpenRouter.APIKey
	case "provider.zen.api_key":
		return c.Provider.Zen.APIKey
	// Model
	case "model.default":
		return c.Model.Default
	case "model.context_warning_threshold":
		return fmt.Sprintf("%.2f", c.Model.ContextWarningThreshold)
	case "model.show_thinking_by_default":
		return boolStr(c.Model.ShowThinkingByDefault)
	case "model.auto_collapse_tools":
		return boolStr(c.Model.AutoCollapseTools)
	case "model.auto_arbitrage":
		return boolStr(c.Model.AutoArbitrage)
	case "model.arbitrage_threshold":
		return fmt.Sprintf("%.2f", c.Model.ArbitrageThreshold)
	case "model.default_context_length":
		return fmt.Sprintf("%d", c.Model.DefaultContextLength)
	case "model.token_ema_alpha":
		return fmt.Sprintf("%.2f", c.Model.TokenEMAAlpha)
	// UI
	case "ui.theme":
		if c.UI.Theme == "" {
			return "dark"
		}
		return c.UI.Theme
	case "ui.compact_mode":
		return boolStr(c.UI.CompactMode)
	case "ui.show_token_usage":
		return boolStr(c.UI.ShowTokenUsage)
	case "ui.show_cost_estimate":
		return boolStr(c.UI.ShowCostEstimate)
	case "ui.max_iterations":
		return fmt.Sprintf("%d", c.UI.MaxIterations)
	case "ui.leader_key":
		return c.UI.LeaderKey
	case "ui.leader_timeout_ms":
		return fmt.Sprintf("%d", c.UI.LeaderTimeoutMs)
	case "ui.sidebar_width_threshold":
		return fmt.Sprintf("%d", c.UI.SidebarWidthThreshold)
	case "ui.discuss_timeout":
		return fmt.Sprintf("%d", c.UI.DiscussTimeout)
	case "ui.thinking_max_lines":
		return fmt.Sprintf("%d", c.UI.ThinkingMaxLines)
	case "ui.permission_modal_width":
		return fmt.Sprintf("%d", c.UI.PermissionModalWidth)
	case "ui.sidebar_width":
		return fmt.Sprintf("%d", c.UI.SidebarWidth)
	case "ui.max_message_history":
		return fmt.Sprintf("%d", c.UI.MaxMessageHistory)
	case "ui.fallback_banner_timeout_secs":
		return fmt.Sprintf("%d", c.UI.FallbackBannerSecs)
	case "ui.default_log_lines":
		return fmt.Sprintf("%d", c.UI.DefaultLogLines)
	case "ui.session_list_limit":
		return fmt.Sprintf("%d", c.UI.SessionListLimit)
	case "ui.thinking_opacity":
		return fmt.Sprintf("%.1f", c.UI.ThinkingOpacity)
	case "ui.frecent_history_size":
		return fmt.Sprintf("%d", c.UI.FrecentHistorySize)
	// Permissions
	case "permissions.default_mode":
		return c.Permissions.DefaultMode
	case "permissions.timeout_seconds":
		return fmt.Sprintf("%d", c.Permissions.TimeoutSeconds)
	// Features
	case "features.auto_backup":
		return boolStr(c.Features.AutoBackup)
	case "features.resume_on_startup":
		return boolStr(c.Features.ResumeOnStartup)
	case "features.model_cache_ttl_minutes":
		return fmt.Sprintf("%d", c.Features.ModelCacheTTLMinutes)
	case "features.model_cache_stale_hours":
		return fmt.Sprintf("%d", c.Features.ModelCacheStaleHours)
	case "features.healthcheck_live_ms":
		return fmt.Sprintf("%d", c.Features.HealthCheckLiveMs)
	case "features.healthcheck_slow_ms":
		return fmt.Sprintf("%d", c.Features.HealthCheckSlowMs)
	case "features.session_id_length":
		return fmt.Sprintf("%d", c.Features.SessionIDLength)
	case "features.max_recent_models":
		return fmt.Sprintf("%d", c.Features.MaxRecentModels)
	case "features.session_retention_days":
		return fmt.Sprintf("%d", c.Features.SessionRetentionDays)
	case "features.health_check_timeout_secs":
		return fmt.Sprintf("%d", c.Features.HealthCheckTimeoutSecs)
	case "features.rate_limit_backoff_secs":
		return fmt.Sprintf("%d", c.Features.RateLimitBackoffSecs)
	case "features.budget_limit_usd":
		return fmt.Sprintf("%.2f", c.Features.BudgetLimitUSD)
	// Ledger
	case "ledger.enabled":
		return boolStr(c.Ledger.Enabled)
	case "ledger.max_entries":
		return fmt.Sprintf("%d", c.Ledger.MaxEntries)
	// Tools
	case "tools.max_glob_results":
		return fmt.Sprintf("%d", c.Tools.MaxGlobResults)
	case "tools.max_grep_results":
		return fmt.Sprintf("%d", c.Tools.MaxGrepResults)
	case "tools.bash_kill_grace_secs":
		return fmt.Sprintf("%d", c.Tools.BashKillGraceSecs)
	case "tools.max_backups_per_file":
		return fmt.Sprintf("%d", c.Tools.MaxBackupsPerFile)
	case "tools.webfetch_max_redirects":
		return fmt.Sprintf("%d", c.Tools.WebfetchMaxRedirects)
	case "tools.webfetch_user_agent":
		return c.Tools.WebfetchUserAgent
	// Agents
	case "agents.default":
		return c.Agents.Default
	case "agents.plan":
		return c.Agents.Plan
	case "agents.execute":
		return c.Agents.Execute
	case "agents.verify":
		return c.Agents.Verify
	case "agents.ship":
		return c.Agents.Ship
	case "agents.discuss":
		return c.Agents.Discuss
	// Git
	case "git.commit_prefix":
		return c.Git.CommitPrefix
	case "git.fix_prefix":
		return c.Git.FixPrefix
	case "git.ship_prefix":
		return c.Git.ShipPrefix
	case "git.user_name":
		return c.Git.UserName
	case "git.user_email":
		return c.Git.UserEmail
	// Verify
	case "verify.build_command":
		return c.Verify.BuildCommand
	case "verify.test_command":
		return c.Verify.TestCommand
	}
	return ""
}

// setFieldValue writes a new value into cfg.
func (m *ConfigModel) setFieldValue(f cfgField, val string) {
	if m.cfg == nil {
		m.cfg = config.DefaultConfig()
	}
	c := m.cfg
	switch f.key {
	case "provider.default":
		c.Provider.Default = val
	case "provider.auto_fallback":
		c.Provider.AutoFallback = val == "yes"
	case "provider.openrouter_base_url":
		c.Provider.OpenRouterBaseURL = val
	case "provider.zen_base_url":
		c.Provider.ZenBaseURL = val
	case "provider.openrouter_referer":
		c.Provider.OpenRouterReferer = val
	case "provider.openrouter_title":
		c.Provider.OpenRouterTitle = val
	case "provider.openrouter.api_key":
		c.Provider.OpenRouter.APIKey = val
	case "provider.zen.api_key":
		c.Provider.Zen.APIKey = val
	case "model.default":
		c.Model.Default = val
	case "model.context_warning_threshold":
		if v, err := strconv.ParseFloat(val, 64); err == nil {
			c.Model.ContextWarningThreshold = v
		}
	case "model.show_thinking_by_default":
		c.Model.ShowThinkingByDefault = val == "yes"
	case "model.auto_collapse_tools":
		c.Model.AutoCollapseTools = val == "yes"
	case "model.auto_arbitrage":
		c.Model.AutoArbitrage = val == "yes"
	case "model.arbitrage_threshold":
		if v, err := strconv.ParseFloat(val, 64); err == nil {
			c.Model.ArbitrageThreshold = v
		}
	case "model.default_context_length":
		if v, err := strconv.Atoi(val); err == nil {
			c.Model.DefaultContextLength = v
		}
	case "model.token_ema_alpha":
		if v, err := strconv.ParseFloat(val, 64); err == nil {
			c.Model.TokenEMAAlpha = v
		}
	case "ui.theme":
		c.UI.Theme = val
	case "ui.compact_mode":
		c.UI.CompactMode = val == "yes"
	case "ui.show_token_usage":
		c.UI.ShowTokenUsage = val == "yes"
	case "ui.show_cost_estimate":
		c.UI.ShowCostEstimate = val == "yes"
	case "ui.max_iterations":
		if v, err := strconv.Atoi(val); err == nil {
			c.UI.MaxIterations = v
		}
	case "ui.leader_key":
		c.UI.LeaderKey = val
	case "ui.leader_timeout_ms":
		if v, err := strconv.Atoi(val); err == nil {
			c.UI.LeaderTimeoutMs = v
		}
	case "ui.sidebar_width_threshold":
		if v, err := strconv.Atoi(val); err == nil {
			c.UI.SidebarWidthThreshold = v
		}
	case "ui.discuss_timeout":
		if v, err := strconv.Atoi(val); err == nil {
			c.UI.DiscussTimeout = v
		}
	case "ui.thinking_max_lines":
		if v, err := strconv.Atoi(val); err == nil {
			c.UI.ThinkingMaxLines = v
		}
	case "ui.permission_modal_width":
		if v, err := strconv.Atoi(val); err == nil {
			c.UI.PermissionModalWidth = v
		}
	case "ui.sidebar_width":
		if v, err := strconv.Atoi(val); err == nil {
			c.UI.SidebarWidth = v
		}
	case "ui.max_message_history":
		if v, err := strconv.Atoi(val); err == nil {
			c.UI.MaxMessageHistory = v
		}
	case "ui.fallback_banner_timeout_secs":
		if v, err := strconv.Atoi(val); err == nil {
			c.UI.FallbackBannerSecs = v
		}
	case "ui.default_log_lines":
		if v, err := strconv.Atoi(val); err == nil {
			c.UI.DefaultLogLines = v
		}
	case "ui.session_list_limit":
		if v, err := strconv.Atoi(val); err == nil {
			c.UI.SessionListLimit = v
		}
	case "ui.thinking_opacity":
		if v, err := strconv.ParseFloat(val, 64); err == nil {
			c.UI.ThinkingOpacity = v
		}
	case "ui.frecent_history_size":
		if v, err := strconv.Atoi(val); err == nil {
			c.UI.FrecentHistorySize = v
		}
	case "permissions.default_mode":
		c.Permissions.DefaultMode = val
	case "permissions.timeout_seconds":
		if v, err := strconv.Atoi(val); err == nil {
			c.Permissions.TimeoutSeconds = v
		}
	case "features.auto_backup":
		c.Features.AutoBackup = val == "yes"
	case "features.resume_on_startup":
		c.Features.ResumeOnStartup = val == "yes"
	case "features.model_cache_ttl_minutes":
		if v, err := strconv.Atoi(val); err == nil {
			c.Features.ModelCacheTTLMinutes = v
		}
	case "features.model_cache_stale_hours":
		if v, err := strconv.Atoi(val); err == nil {
			c.Features.ModelCacheStaleHours = v
		}
	case "features.healthcheck_live_ms":
		if v, err := strconv.Atoi(val); err == nil {
			c.Features.HealthCheckLiveMs = v
		}
	case "features.healthcheck_slow_ms":
		if v, err := strconv.Atoi(val); err == nil {
			c.Features.HealthCheckSlowMs = v
		}
	case "features.session_id_length":
		if v, err := strconv.Atoi(val); err == nil {
			c.Features.SessionIDLength = v
		}
	case "features.max_recent_models":
		if v, err := strconv.Atoi(val); err == nil {
			c.Features.MaxRecentModels = v
		}
	case "features.session_retention_days":
		if v, err := strconv.Atoi(val); err == nil {
			c.Features.SessionRetentionDays = v
		}
	case "features.health_check_timeout_secs":
		if v, err := strconv.Atoi(val); err == nil {
			c.Features.HealthCheckTimeoutSecs = v
		}
	case "features.rate_limit_backoff_secs":
		if v, err := strconv.Atoi(val); err == nil {
			c.Features.RateLimitBackoffSecs = v
		}
	case "features.budget_limit_usd":
		if v, err := strconv.ParseFloat(val, 64); err == nil {
			c.Features.BudgetLimitUSD = v
		}
	case "ledger.enabled":
		c.Ledger.Enabled = val == "yes"
	case "ledger.max_entries":
		if v, err := strconv.Atoi(val); err == nil {
			c.Ledger.MaxEntries = v
		}
	case "tools.max_glob_results":
		if v, err := strconv.Atoi(val); err == nil {
			c.Tools.MaxGlobResults = v
		}
	case "tools.max_grep_results":
		if v, err := strconv.Atoi(val); err == nil {
			c.Tools.MaxGrepResults = v
		}
	case "tools.bash_kill_grace_secs":
		if v, err := strconv.Atoi(val); err == nil {
			c.Tools.BashKillGraceSecs = v
		}
	case "tools.max_backups_per_file":
		if v, err := strconv.Atoi(val); err == nil {
			c.Tools.MaxBackupsPerFile = v
		}
	case "tools.webfetch_max_redirects":
		if v, err := strconv.Atoi(val); err == nil {
			c.Tools.WebfetchMaxRedirects = v
		}
	case "tools.webfetch_user_agent":
		c.Tools.WebfetchUserAgent = val
	case "agents.default":
		c.Agents.Default = val
	case "agents.plan":
		c.Agents.Plan = val
	case "agents.execute":
		c.Agents.Execute = val
	case "agents.verify":
		c.Agents.Verify = val
	case "agents.ship":
		c.Agents.Ship = val
	case "agents.discuss":
		c.Agents.Discuss = val
	case "git.commit_prefix":
		c.Git.CommitPrefix = val
	case "git.fix_prefix":
		c.Git.FixPrefix = val
	case "git.ship_prefix":
		c.Git.ShipPrefix = val
	case "git.user_name":
		c.Git.UserName = val
	case "git.user_email":
		c.Git.UserEmail = val
	case "verify.build_command":
		c.Verify.BuildCommand = val
	case "verify.test_command":
		c.Verify.TestCommand = val
	}
}

// ─── Bubble Tea interface ──────────────────────────────────────────────────────

func (m *ConfigModel) Init() tea.Cmd { return nil }

func (m *ConfigModel) Update(msg tea.Msg) (*ConfigModel, tea.Cmd) {
	switch msg := msg.(type) {
	case tea.KeyMsg:
		if m.editing {
			return m.updateEditing(msg)
		}
		return m.updateBrowsing(msg)

	case tea.WindowSizeMsg:
		m.width = msg.Width
		m.height = msg.Height
		m.viewport.Width = max(10, msg.Width-4)
		h := msg.Height - 8
		if h < 4 {
			h = 4
		}
		m.viewport.Height = h
		m.editInput.Width = max(20, min(50, msg.Width-12))
	}

	var cmd tea.Cmd
	m.viewport, cmd = m.viewport.Update(msg)
	return m, cmd
}

func (m *ConfigModel) updateBrowsing(msg tea.KeyMsg) (*ConfigModel, tea.Cmd) {
	switch msg.String() {
	case "esc", "q":
		return m, func() tea.Msg { return PopScreenMsg{} }

	case "tab", "]":
		m.sectionIdx = (m.sectionIdx + 1) % len(m.sections)
		m.fieldIdx = 0

	case "shift+tab", "[":
		m.sectionIdx = (m.sectionIdx - 1 + len(m.sections)) % len(m.sections)
		m.fieldIdx = 0

	case "up", "k":
		if m.fieldIdx > 0 {
			m.fieldIdx--
		} else if m.sectionIdx > 0 {
			m.sectionIdx--
			m.fieldIdx = len(m.sections[m.sectionIdx].fields) - 1
		}

	case "down", "j":
		sec := m.sections[m.sectionIdx]
		if m.fieldIdx < len(sec.fields)-1 {
			m.fieldIdx++
		} else if m.sectionIdx < len(m.sections)-1 {
			m.sectionIdx++
			m.fieldIdx = 0
		}

	case "enter", "e", " ":
		return m.activateField()

	case "s":
		return m.saveConfig()

	case "L":
		return m.saveLocalConfig()

	case "r":
		if m.cfgPath != "" {
			if cfg, err := config.Load(m.cfgPath); err == nil {
				m.cfg = cfg
				m.buildSections()
				m.statusMsg = "↺ Config reloaded from disk"
				m.statusTime = time.Now()
				return m, func() tea.Msg { return ConfigSavedMsg{} }
			}
		}
		m.buildSections()
		m.statusMsg = "↺ Config reloaded from memory"
		m.statusTime = time.Now()
	}

	m.scrollToField()
	return m, nil
}

func (m *ConfigModel) activateField() (*ConfigModel, tea.Cmd) {
	if len(m.sections) == 0 {
		return m, nil
	}
	sec := m.sections[m.sectionIdx]
	if m.fieldIdx >= len(sec.fields) {
		return m, nil
	}
	f := sec.fields[m.fieldIdx]

	switch f.fieldType {
	case cfgBool:
		cur := m.getFieldValue(f)
		newVal := "yes"
		if cur == "yes" {
			newVal = "no"
		}
		m.setFieldValue(f, newVal)
		m.dirty = true
		m.statusMsg = fmt.Sprintf("✎ %s → %s  (press s to save)", f.label, newVal)
		m.statusTime = time.Now()

	case cfgChoice:
		if len(f.choices) == 0 {
			return m, nil
		}
		cur := m.getFieldValue(f)
		idx := 0
		for i, c := range f.choices {
			if c == cur {
				idx = i
				break
			}
		}
		next := f.choices[(idx+1)%len(f.choices)]
		m.setFieldValue(f, next)
		m.dirty = true
		m.statusMsg = fmt.Sprintf("✎ %s → %s  (press s to save)", f.label, next)
		m.statusTime = time.Now()

	case cfgReadOnly:
		m.statusMsg = "(read-only field)"
		m.statusTime = time.Now()

	default: // text, password, number, float
		m.editing = true
		m.editInput.SetValue(m.getFieldValue(f))
		m.editInput.Focus()
		m.editInput.Placeholder = f.label
		if f.fieldType == cfgPassword {
			m.editInput.EchoMode = textinput.EchoPassword
		} else {
			m.editInput.EchoMode = textinput.EchoNormal
		}
		return m, textinput.Blink
	}
	return m, nil
}

func (m *ConfigModel) updateEditing(msg tea.KeyMsg) (*ConfigModel, tea.Cmd) {
	switch msg.String() {
	case "esc":
		m.editing = false
		m.editInput.EchoMode = textinput.EchoNormal
		return m, nil
	case "enter":
		val := strings.TrimSpace(m.editInput.Value())
		m.editing = false
		m.editInput.EchoMode = textinput.EchoNormal
		if val == "" {
			return m, nil
		}
		sec := m.sections[m.sectionIdx]
		f := sec.fields[m.fieldIdx]
		m.setFieldValue(f, val)
		m.dirty = true
		displayVal := val
		if f.fieldType == cfgPassword && len(val) > 4 {
			displayVal = "••••" + val[len(val)-4:]
		}
		m.statusMsg = fmt.Sprintf("✎ %s → %s  (press s to save)", f.label, displayVal)
		m.statusTime = time.Now()
		return m, nil
	}

	var cmd tea.Cmd
	m.editInput, cmd = m.editInput.Update(msg)
	return m, cmd
}

func (m *ConfigModel) saveConfig() (*ConfigModel, tea.Cmd) {
	if m.cfg == nil || m.cfgPath == "" {
		m.statusMsg = "✗ No config path set"
		m.statusTime = time.Now()
		return m, nil
	}
	if err := m.cfg.SaveWithKeychain(m.cfgPath, m.keychain); err != nil {
		m.saveErr = err.Error()
		m.statusMsg = "✗ Save failed: " + err.Error()
		m.statusTime = time.Now()
		slog.Warn("config editor save failed", "error", err)
		return m, nil
	}
	m.dirty = false
	m.saveErr = ""
	m.statusMsg = fmt.Sprintf("✓ Saved to %s", m.cfgPath)
	m.statusTime = time.Now()
	return m, func() tea.Msg { return ConfigSavedMsg{} }
}

func (m *ConfigModel) saveLocalConfig() (*ConfigModel, tea.Cmd) {
	if m.cfg == nil {
		m.statusMsg = "✗ No config loaded"
		m.statusTime = time.Now()
		return m, nil
	}
	localPath, err := config.LocalConfigPath()
	if err != nil {
		m.statusMsg = fmt.Sprintf("✗ Cannot determine working directory: %v", err)
		m.statusTime = time.Now()
		return m, nil
	}
	if err := m.cfg.SaveProject(localPath); err != nil {
		m.saveErr = err.Error()
		m.statusMsg = "✗ Local save failed: " + err.Error()
		m.statusTime = time.Now()
		return m, nil
	}
	m.dirty = false
	m.saveErr = ""
	m.statusMsg = fmt.Sprintf("✓ Project config saved to %s", localPath)
	m.statusTime = time.Now()
	return m, func() tea.Msg { return ConfigSavedMsg{} }
}

// ─── View ─────────────────────────────────────────────────────────────────────

func (m *ConfigModel) View() string {
	t := m.theme

	// ── Top: section tabs ────────────────────────────────────────────────────
	tabs := m.renderTabs()

	// ── Middle: field list ───────────────────────────────────────────────────
	fieldArea := m.renderFields()

	// ── Bottom: edit box or hint + status ────────────────────────────────────
	var bottom string
	if m.editing {
		bottom = m.renderEditBox()
	} else {
		bottom = m.renderHint()
	}

	// ── Status bar ───────────────────────────────────────────────────────────
	status := m.renderStatus()

	// ── Keybind footer ────────────────────────────────────────────────────────
	footerParts := []string{
		lipgloss.NewStyle().Foreground(t.Brand).Render("↑↓"),
		lipgloss.NewStyle().Foreground(t.TextMuted).Render(" navigate  "),
		lipgloss.NewStyle().Foreground(t.Brand).Render("↵/e"),
		lipgloss.NewStyle().Foreground(t.TextMuted).Render(" edit  "),
		lipgloss.NewStyle().Foreground(t.Brand).Render("tab"),
		lipgloss.NewStyle().Foreground(t.TextMuted).Render(" section  "),
		lipgloss.NewStyle().Foreground(t.Brand).Render("s"),
		lipgloss.NewStyle().Foreground(t.TextMuted).Render(" save  "),
		lipgloss.NewStyle().Foreground(t.Brand).Render("L"),
		lipgloss.NewStyle().Foreground(t.TextMuted).Render(" save local  "),
		lipgloss.NewStyle().Foreground(t.Brand).Render("q"),
		lipgloss.NewStyle().Foreground(t.TextMuted).Render(" close"),
	}
	if m.dirty {
		footerParts = append(footerParts,
			lipgloss.NewStyle().Foreground(t.Warning).Bold(true).Render("  ● unsaved changes"),
		)
	}
	footer := lipgloss.NewStyle().PaddingLeft(2).Render(strings.Join(footerParts, ""))

	return lipgloss.JoinVertical(lipgloss.Left,
		tabs,
		fieldArea,
		bottom,
		status,
		footer,
	)
}

func (m *ConfigModel) renderTabs() string {
	t := m.theme
	var parts []string
	for i, sec := range m.sections {
		if i == m.sectionIdx {
			parts = append(parts, lipgloss.NewStyle().
				Foreground(t.Background).Background(t.Brand).
				Bold(true).Padding(0, 2).
				Render(sec.title))
		} else {
			parts = append(parts, lipgloss.NewStyle().
				Foreground(t.TextMuted).Padding(0, 2).
				Render(sec.title))
		}
	}
	bar := strings.Join(parts, " ")
	return lipgloss.NewStyle().
		BorderBottom(true).
		BorderStyle(lipgloss.NormalBorder()).
		BorderForeground(t.Border).
		Width(max(10, m.width-4)).
		PaddingLeft(1).
		Render(bar) + "\n"
}

func (m *ConfigModel) renderFields() string {
	if len(m.sections) == 0 {
		return ""
	}
	sec := m.sections[m.sectionIdx]
	t := m.theme

	// Available height for the field viewport
	vpH := m.height - 10
	if vpH < 4 {
		vpH = 4
	}

	var rows []string
	for i, f := range sec.fields {
		rows = append(rows, m.renderFieldRow(f, i))
	}
	content := strings.Join(rows, "\n")

	cContentW := max(10, m.width-4)
	m.viewport.Width = cContentW
	m.viewport.Height = vpH
	m.viewport.SetContent(content)
	m.scrollToField()

	// Wrap viewport in a subtle border
	return lipgloss.NewStyle().
		Border(theme.ThinBorder).
		BorderForeground(t.Border).
		Width(max(10, m.width-2)).
		Margin(0, 1).
		Render(m.viewport.View())
}

func (m *ConfigModel) renderFieldRow(f cfgField, idx int) string {
	t := m.theme
	val := m.getFieldValue(f)
	selected := idx == m.fieldIdx

	cursor := "  "
	if selected {
		cursor = lipgloss.NewStyle().Foreground(t.Brand).Render("▸ ")
	}

	// Label width: ideal 34, scaled down on narrow terminals
	labelW := 34
	avail := m.width - 10 // cursor(2) + borders(4) + value margin(4)
	if avail < labelW {
		labelW = max(10, avail/2)
	}
	labelStyle := lipgloss.NewStyle().Foreground(t.TextMuted).Width(labelW)
	valStyle := lipgloss.NewStyle().Foreground(t.Text)
	if selected {
		labelStyle = labelStyle.Foreground(t.Text).Bold(true)
		valStyle = valStyle.Foreground(t.Brand).Bold(true)
	}

	var valDisplay string
	switch f.fieldType {
	case cfgBool:
		icon, col := "○ no", t.TextMuted
		if val == "yes" {
			icon, col = "● yes", t.Success
		}
		if selected {
			col = t.Brand
		}
		valDisplay = lipgloss.NewStyle().Foreground(col).Render(icon)

	case cfgChoice:
		var opts []string
		for _, c := range f.choices {
			if c == val {
				opts = append(opts, lipgloss.NewStyle().Foreground(t.Brand).Bold(true).Render("●"+c))
			} else {
				opts = append(opts, lipgloss.NewStyle().Foreground(t.TextMuted).Render("○"+c))
			}
		}
		valDisplay = strings.Join(opts, "  ")

	case cfgPassword:
		valDisplay = lipgloss.NewStyle().Foreground(t.TextMuted).Render(maskedKey(val))

	default:
		if val == "" {
			valDisplay = lipgloss.NewStyle().Foreground(t.TextMuted).Italic(true).Render("(not set)")
		} else {
			valDisplay = valStyle.Render(val)
		}
	}

	return cursor + labelStyle.Render(f.label) + valDisplay
}

func (m *ConfigModel) renderEditBox() string {
	t := m.theme
	sec := m.sections[m.sectionIdx]
	f := sec.fields[m.fieldIdx]

	typeHint := ""
	switch f.fieldType {
	case cfgNumber:
		typeHint = "integer"
	case cfgFloat:
		typeHint = "decimal"
	case cfgPassword:
		typeHint = "password"
	default:
		typeHint = "text"
	}

	inner := lipgloss.JoinVertical(lipgloss.Left,
		lipgloss.NewStyle().Foreground(t.TextMuted).Render("Editing: "+f.label+" ("+typeHint+")"),
		m.editInput.View(),
		lipgloss.NewStyle().Foreground(t.TextMuted).Render("↵ confirm   esc cancel"),
	)
	// Edit box width: clamped to terminal, max 60, min 30
	editW := min(60, max(30, m.width-8))
	return lipgloss.NewStyle().
		Border(lipgloss.RoundedBorder()).
		BorderForeground(t.Brand).
		Padding(0, 2).
		Width(editW).
		MarginLeft(2).
		Render(inner)
}

func (m *ConfigModel) renderHint() string {
	t := m.theme
	if len(m.sections) == 0 {
		return ""
	}
	sec := m.sections[m.sectionIdx]
	if m.fieldIdx >= len(sec.fields) {
		return ""
	}
	hint := sec.fields[m.fieldIdx].hint
	if hint == "" {
		return ""
	}
	return lipgloss.NewStyle().
		Foreground(t.TextMuted).
		Italic(true).
		PaddingLeft(4).
		Render("ℹ  " + hint)
}

func (m *ConfigModel) renderStatus() string {
	if m.statusMsg == "" || time.Since(m.statusTime) > 6*time.Second {
		return ""
	}
	t := m.theme
	col := t.Success
	if strings.HasPrefix(m.statusMsg, "✗") {
		col = t.Error
	} else if strings.HasPrefix(m.statusMsg, "✎") || strings.HasPrefix(m.statusMsg, "↺") {
		col = t.Warning
	}
	return lipgloss.NewStyle().
		Foreground(col).
		PaddingLeft(2).
		Bold(true).
		Render(m.statusMsg)
}

// scrollToField ensures the viewport is scrolled so the active field is visible.
func (m *ConfigModel) scrollToField() {
	// Each row is 1 line; estimate scroll offset
	offset := m.fieldIdx
	vpH := m.viewport.Height
	if vpH < 1 {
		return
	}
	if offset < m.viewport.YOffset {
		m.viewport.SetYOffset(offset)
	} else if offset >= m.viewport.YOffset+vpH {
		m.viewport.SetYOffset(offset - vpH + 1)
	}
}

// buildContent is kept for backward compatibility with AppState which calls it after SettingsSavedMsg.
func (m *ConfigModel) buildContent() {
	// No-op: content is now rendered dynamically from cfg in View().
}
