package tui

import (
	"github.com/eshanized/M31A/internal/types"
)

// buildSections defines all sections and their fields in the same order as config.toml.
func (m *ConfigModel) buildSections() {
	m.sections = []cfgSection{
		{
			title: "Provider",
			fields: []cfgField{
				{key: "provider.default", label: "Default provider", fieldType: cfgChoice, choices: []string{types.ProviderOpenRouter, types.ProviderZen, types.ProviderNvidia}, hint: "Which provider M31A uses by default"},
				{key: "provider.auto_fallback", label: "Auto fallback", fieldType: cfgBool, hint: "Automatically switch provider on failure"},
				{key: "provider.openrouter_base_url", label: "OpenRouter base URL", fieldType: cfgText, hint: "Custom base URL (empty = default)"},
				{key: "provider.zen_base_url", label: "Zen base URL", fieldType: cfgText, hint: "Custom base URL (empty = default)"},
				{key: "provider.nvidia_base_url", label: "NVIDIA base URL", fieldType: cfgText, hint: "Custom base URL (empty = default)"},
				{key: "provider.openrouter_referer", label: "OpenRouter referer header", fieldType: cfgText, hint: "HTTP-Referer sent to OpenRouter"},
				{key: "provider.openrouter_title", label: "OpenRouter title header", fieldType: cfgText, hint: "X-Title sent to OpenRouter"},
				{key: "provider.openrouter.api_key", label: "OpenRouter API key", fieldType: cfgPassword, hint: "Saved to keychain, not written to disk"},
				{key: "provider.zen.api_key", label: "Zen API key", fieldType: cfgPassword, hint: "Saved to keychain, not written to disk"},
				{key: "provider.nvidia.api_key", label: "NVIDIA API key", fieldType: cfgPassword, hint: "Saved to keychain, not written to disk"},
			},
		},
		{
			title: "Model",
			fields: []cfgField{
				{key: "model.default", label: "Default model", fieldType: cfgText, hint: "Model ID used when none is selected"},
				{key: "model.context_warning_threshold", label: "Context warning threshold", fieldType: cfgFloat, hint: "0.0-1.0, fraction of context window before warning"},
				{key: "model.show_thinking_by_default", label: "Show thinking by default", fieldType: cfgBool, hint: "Expand thinking blocks automatically"},
				{key: "model.auto_collapse_tools", label: "Auto-collapse tools", fieldType: cfgBool, hint: "Collapse tool cards after completion"},
				{key: "model.auto_arbitrage", label: "Auto-arbitrage", fieldType: cfgBool, hint: "Automatically suggest cheaper model alternatives"},
				{key: "model.arbitrage_threshold", label: "Arbitrage threshold", fieldType: cfgFloat, hint: "Cost ratio that triggers arbitrage (0.0-1.0)"},
				{key: "model.default_context_length", label: "Default context length", fieldType: cfgNumber, hint: "Fallback when provider doesn't return context size"},
				{key: "model.token_ema_alpha", label: "Token EMA alpha", fieldType: cfgFloat, hint: "EMA calibration rate 0.0-1.0 (lower = slower)"},
			},
		},
		{
			title: "UI",
			fields: []cfgField{
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
				{key: "ui.thinking_opacity", label: "Thinking opacity", fieldType: cfgFloat, hint: "Opacity of thinking blocks (0.0-1.0)"},
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
				{key: "features.session_id_length", label: "Session ID length", fieldType: cfgNumber, hint: "Hex chars in session IDs (4-16)"},
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
				{key: "git.commit_prefix", label: "Commit prefix", fieldType: cfgText, hint: "Conventional commit type (feat, fix, chore)"},
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
