package config

// mergeHelper provides type-safe merge operations that replicate the
// reflection-based merge behavior from the original mergeConfig.
// The defined map tracks which TOML keys were explicitly set in the overlay,
// enabling bool fields to be overridden with false (the "defined" pattern).
type mergeHelper struct {
	defined map[string]bool
}

func newMergeHelper(defined map[string]bool) mergeHelper {
	if defined == nil {
		defined = make(map[string]bool)
	}
	return mergeHelper{defined: defined}
}

// hasKey reports whether the given TOML key was explicitly set in the overlay.
func (m mergeHelper) hasKey(key string) bool {
	return m.defined[key]
}

// stringField copies overlay to base if non-empty.
func (m mergeHelper) stringField(base, overlay *string, key string) {
	if *overlay != "" {
		*base = *overlay
	}
}

// boolField copies overlay to base if explicitly defined or overlay is true.
// This preserves the original behavior: a bool set to false in the overlay
// only overrides the base if the key was explicitly present in the TOML.
func (m mergeHelper) boolField(base, overlay *bool, key string) {
	if m.hasKey(key) || *overlay {
		*base = *overlay
	}
}

// intField copies overlay to base if explicitly defined in the overlay.
// This allows zero-value overrides (e.g., setting max_iterations = 0).
func (m mergeHelper) intField(base, overlay *int, key string) {
	if m.hasKey(key) || *overlay != 0 {
		*base = *overlay
	}
}

// float64Field copies overlay to base if explicitly defined in the overlay.
// This allows zero-value overrides.
func (m mergeHelper) float64Field(base, overlay *float64, key string) {
	if m.hasKey(key) || *overlay != 0 {
		*base = *overlay
	}
}

// sliceField copies overlay to base if non-nil and non-empty.
func (m mergeHelper) sliceField(base, overlay *[]string, key string) {
	if len(*overlay) > 0 {
		dst := make([]string, len(*overlay))
		copy(dst, *overlay)
		*base = dst
	}
}

// stringMapField copies overlay entries into base map. Non-empty overlay keys
// override corresponding base keys; base keys not in overlay are preserved.
func (m mergeHelper) stringMapField(base, overlay *map[string]string, key string) {
	if len(*overlay) > 0 {
		if *base == nil {
			*base = make(map[string]string)
		}
		for k, v := range *overlay {
			(*base)[k] = v
		}
	}
}

// mergeSlice copies overlay to base if non-nil and non-empty.
// Used for PermissionRule slices and similar typed slices.
func mergeSlice[T any](base, overlay *[]T, key string) {
	if len(*overlay) > 0 {
		dst := make([]T, len(*overlay))
		copy(dst, *overlay)
		*base = dst
	}
}

// MergeConfig performs a type-safe merge of overlay into base.
// Overlay non-zero values override base values. Zero-valued fields in overlay
// leave base values unchanged. Bool fields only override when explicitly
// defined in the overlay or when overlay is true.
func MergeConfig(base, overlay *Config, defined map[string]bool) {
	if overlay == nil {
		return
	}
	h := newMergeHelper(defined)

	h.mergeProviderConfig(&base.Provider, &overlay.Provider, "provider")
	h.mergeModelConfig(&base.Model, &overlay.Model, "model")
	h.mergeUIConfig(&base.UI, &overlay.UI, "ui")
	h.mergePermissionsConfig(&base.Permissions, &overlay.Permissions, "permissions")
	h.mergeFeaturesConfig(&base.Features, &overlay.Features, "features")
	h.mergeLedgerConfig(&base.Ledger, &overlay.Ledger, "ledger")
	h.mergeToolsConfig(&base.Tools, &overlay.Tools, "tools")
	h.mergeAgentsConfig(&base.Agents, &overlay.Agents, "agents")
	h.mergeGitConfig(&base.Git, &overlay.Git, "git")
	h.mergeVerifyConfig(&base.Verify, &overlay.Verify, "verify")
	h.mergeIntelligenceConfig(&base.Intelligence, &overlay.Intelligence, "intelligence")
	h.mergeCompactionConfig(&base.Compaction, &overlay.Compaction, "compaction")
	h.mergeInstructionsConfig(&base.Instructions, &overlay.Instructions, "instructions")
	h.mergeSkillsConfig(&base.Skills, &overlay.Skills, "skills")
	h.mergeNarrativeConfig(&base.Narrative, &overlay.Narrative, "narrative")
	h.mergeModelCapabilitiesConfig(&base.ModelCapabilities, &overlay.ModelCapabilities, "model_capabilities")
	h.mergePromptConfig(&base.Prompts, &overlay.Prompts, "prompts")
	h.mergeTemplateConfig(&base.Templates, &overlay.Templates, "templates")
	h.mergeExtensionsConfig(&base.Extensions, &overlay.Extensions, "extensions")
}

func (h mergeHelper) mergeProviderConfig(base, overlay *ProviderConfig, prefix string) {
	h.stringField(&base.Default, &overlay.Default, prefix+".default")
	h.boolField(&base.AutoFallback, &overlay.AutoFallback, prefix+".auto_fallback")
	h.mergeCredentialConfig(&base.OpenRouter, &overlay.OpenRouter, prefix+".openrouter")
	h.mergeCredentialConfig(&base.Zen, &overlay.Zen, prefix+".zen")
	h.mergeCredentialConfig(&base.Nvidia, &overlay.Nvidia, prefix+".nvidia")
	h.stringField(&base.OpenRouterBaseURL, &overlay.OpenRouterBaseURL, prefix+".openrouter_base_url")
	h.stringField(&base.ZenBaseURL, &overlay.ZenBaseURL, prefix+".zen_base_url")
	h.stringField(&base.NvidiaBaseURL, &overlay.NvidiaBaseURL, prefix+".nvidia_base_url")
	h.stringField(&base.OpenRouterReferer, &overlay.OpenRouterReferer, prefix+".openrouter_referer")
	h.stringField(&base.OpenRouterTitle, &overlay.OpenRouterTitle, prefix+".openrouter_title")
	h.sliceField(&base.FallbackPriority, &overlay.FallbackPriority, prefix+".fallback_priority")
	h.intField(&base.HealthCheckTimeoutSecs, &overlay.HealthCheckTimeoutSecs, prefix+".health_check_timeout_secs")
	h.sliceField(&base.RegistrationOrder, &overlay.RegistrationOrder, prefix+".registration_order")
}

func (h mergeHelper) mergeCredentialConfig(base, overlay *ProviderCredentialConfig, prefix string) {
	h.stringField(&base.APIKey, &overlay.APIKey, prefix+".api_key")
}

func (h mergeHelper) mergeModelConfig(base, overlay *ModelConfig, prefix string) {
	h.stringField(&base.Default, &overlay.Default, prefix+".default")
	h.float64Field(&base.ContextWarningThreshold, &overlay.ContextWarningThreshold, prefix+".context_warning_threshold")
	h.boolField(&base.ShowThinkingByDefault, &overlay.ShowThinkingByDefault, prefix+".show_thinking_by_default")
	h.boolField(&base.AutoCollapseTools, &overlay.AutoCollapseTools, prefix+".auto_collapse_tools")
	h.boolField(&base.AutoArbitrage, &overlay.AutoArbitrage, prefix+".auto_arbitrage")
	h.float64Field(&base.ArbitrageThreshold, &overlay.ArbitrageThreshold, prefix+".arbitrage_threshold")
	h.intField(&base.DefaultContextLength, &overlay.DefaultContextLength, prefix+".default_context_length")
	h.float64Field(&base.TokenEMAAlpha, &overlay.TokenEMAAlpha, prefix+".token_ema_alpha")
}

func (h mergeHelper) mergeUIConfig(base, overlay *UIConfig, prefix string) {
	h.stringField(&base.Theme, &overlay.Theme, prefix+".theme")
	h.boolField(&base.CompactMode, &overlay.CompactMode, prefix+".compact_mode")
	h.boolField(&base.ShowTokenUsage, &overlay.ShowTokenUsage, prefix+".show_token_usage")
	h.boolField(&base.ShowCostEstimate, &overlay.ShowCostEstimate, prefix+".show_cost_estimate")
	h.intField(&base.MaxIterations, &overlay.MaxIterations, prefix+".max_iterations")
	h.stringField(&base.LeaderKey, &overlay.LeaderKey, prefix+".leader_key")
	h.intField(&base.LeaderTimeoutMs, &overlay.LeaderTimeoutMs, prefix+".leader_timeout_ms")
	h.intField(&base.SidebarWidthThreshold, &overlay.SidebarWidthThreshold, prefix+".sidebar_width_threshold")
	h.intField(&base.DiscussTimeout, &overlay.DiscussTimeout, prefix+".discuss_timeout")
	h.intField(&base.ThinkingMaxLines, &overlay.ThinkingMaxLines, prefix+".thinking_max_lines")
	h.intField(&base.PermissionModalWidth, &overlay.PermissionModalWidth, prefix+".permission_modal_width")
	h.intField(&base.SidebarWidth, &overlay.SidebarWidth, prefix+".sidebar_width")
	h.intField(&base.MaxMessageHistory, &overlay.MaxMessageHistory, prefix+".max_message_history")
	h.intField(&base.FallbackBannerSecs, &overlay.FallbackBannerSecs, prefix+".fallback_banner_timeout_secs")
	h.intField(&base.DefaultLogLines, &overlay.DefaultLogLines, prefix+".default_log_lines")
	h.intField(&base.SessionListLimit, &overlay.SessionListLimit, prefix+".session_list_limit")
	h.float64Field(&base.ThinkingOpacity, &overlay.ThinkingOpacity, prefix+".thinking_opacity")
	h.intField(&base.FrecentHistorySize, &overlay.FrecentHistorySize, prefix+".frecent_history_size")

	// Theme & Colors
	h.stringField(&base.AccentColor, &overlay.AccentColor, prefix+".accent_color")
	h.stringField(&base.CustomBackground, &overlay.CustomBackground, prefix+".custom_background")
	h.stringField(&base.BorderStyle, &overlay.BorderStyle, prefix+".border_style")

	// Typography
	h.boolField(&base.BoldHeaders, &overlay.BoldHeaders, prefix+".bold_headers")
	h.boolField(&base.ItalicThinking, &overlay.ItalicThinking, prefix+".italic_thinking")
	h.intField(&base.TabWidth, &overlay.TabWidth, prefix+".tab_width")

	// Accessibility
	h.boolField(&base.ReducedMotion, &overlay.ReducedMotion, prefix+".reduced_motion")

	// Layout
	h.stringField(&base.SidebarPosition, &overlay.SidebarPosition, prefix+".sidebar_position")
	h.boolField(&base.SidebarAutoShow, &overlay.SidebarAutoShow, prefix+".sidebar_auto_show")
	h.intField(&base.CardPadding, &overlay.CardPadding, prefix+".card_padding")
	h.boolField(&base.WelcomeScreen, &overlay.WelcomeScreen, prefix+".welcome_screen")
	h.stringField(&base.ZenModeKey, &overlay.ZenModeKey, prefix+".zen_mode_key")

	// Animation
	h.stringField(&base.AnimationSpeed, &overlay.AnimationSpeed, prefix+".animation_speed")
	h.stringField(&base.SpinnerStyle, &overlay.SpinnerStyle, prefix+".spinner_style")
	h.stringField(&base.TransitionStyle, &overlay.TransitionStyle, prefix+".transition_style")
	h.boolField(&base.BreathingEffects, &overlay.BreathingEffects, prefix+".breathing_effects")
	h.boolField(&base.LogoAnimation, &overlay.LogoAnimation, prefix+".logo_animation")

	// Status Bar
	h.stringField(&base.StatusBarStyle, &overlay.StatusBarStyle, prefix+".status_bar_style")
	h.stringField(&base.StatusBarPosition, &overlay.StatusBarPosition, prefix+".status_bar_position")
	h.boolField(&base.ShowSpinnerInStatus, &overlay.ShowSpinnerInStatus, prefix+".show_spinner_in_status")

	// Tool Cards
	h.stringField(&base.ToolCardStyle, &overlay.ToolCardStyle, prefix+".tool_card_style")
	h.intField(&base.ToolOutputMaxLines, &overlay.ToolOutputMaxLines, prefix+".tool_output_max_lines")
	h.boolField(&base.SyntaxHighlight, &overlay.SyntaxHighlight, prefix+".syntax_highlight")

	// Toasts
	h.stringField(&base.ToastPosition, &overlay.ToastPosition, prefix+".toast_position")
	h.intField(&base.ToastDurationSecs, &overlay.ToastDurationSecs, prefix+".toast_duration_secs")
	h.intField(&base.ToastMaxVisible, &overlay.ToastMaxVisible, prefix+".toast_max_visible")
}

func (h mergeHelper) mergePermissionsConfig(base, overlay *PermissionsConfig, prefix string) {
	h.stringField(&base.DefaultMode, &overlay.DefaultMode, prefix+".default_mode")
	h.intField(&base.TimeoutSeconds, &overlay.TimeoutSeconds, prefix+".timeout_seconds")
	mergeSlice(&base.Rules, &overlay.Rules, prefix+".rules")

	if overlay.Agents != nil {
		if base.Agents == nil {
			base.Agents = make(map[string]PermissionsAgentConfig)
		}
		for name, overlayAgent := range overlay.Agents {
			baseAgent, exists := base.Agents[name]
			if !exists {
				base.Agents[name] = overlayAgent
				continue
			}
			agentPrefix := prefix + ".agents." + name
			h.stringField(&baseAgent.DefaultAction, &overlayAgent.DefaultAction, agentPrefix+".default_action")
			mergeSlice(&baseAgent.Rules, &overlayAgent.Rules, agentPrefix+".rules")
			base.Agents[name] = baseAgent
		}
	}
}

func (h mergeHelper) mergeFeaturesConfig(base, overlay *FeaturesConfig, prefix string) {
	h.boolField(&base.AutoBackup, &overlay.AutoBackup, prefix+".auto_backup")
	h.boolField(&base.ResumeOnStartup, &overlay.ResumeOnStartup, prefix+".resume_on_startup")
	h.stringField(&base.WorkflowMode, &overlay.WorkflowMode, prefix+".workflow_mode")
	h.intField(&base.ModelCacheTTLMinutes, &overlay.ModelCacheTTLMinutes, prefix+".model_cache_ttl_minutes")
	h.intField(&base.ModelCacheStaleHours, &overlay.ModelCacheStaleHours, prefix+".model_cache_stale_hours")
	h.intField(&base.HealthCheckLiveMs, &overlay.HealthCheckLiveMs, prefix+".healthcheck_live_ms")
	h.intField(&base.HealthCheckSlowMs, &overlay.HealthCheckSlowMs, prefix+".healthcheck_slow_ms")
	h.intField(&base.SessionIDLength, &overlay.SessionIDLength, prefix+".session_id_length")
	h.intField(&base.MaxRecentModels, &overlay.MaxRecentModels, prefix+".max_recent_models")
	h.intField(&base.SessionRetentionDays, &overlay.SessionRetentionDays, prefix+".session_retention_days")
	h.intField(&base.HealthCheckTimeoutSecs, &overlay.HealthCheckTimeoutSecs, prefix+".health_check_timeout_secs")
	h.intField(&base.RateLimitBackoffSecs, &overlay.RateLimitBackoffSecs, prefix+".rate_limit_backoff_secs")
	h.float64Field(&base.BudgetLimitUSD, &overlay.BudgetLimitUSD, prefix+".budget_limit_usd")
	h.boolField(&base.MetricsEnabled, &overlay.MetricsEnabled, prefix+".metrics_enabled")

	// Plan enhancements
	h.boolField(&base.PlanResearch, &overlay.PlanResearch, prefix+".plan_research")
	h.boolField(&base.PlanCheck, &overlay.PlanCheck, prefix+".plan_check")
	h.intField(&base.PlanCheckMaxIter, &overlay.PlanCheckMaxIter, prefix+".plan_check_max_iter")
	h.boolField(&base.PlanSecurityGate, &overlay.PlanSecurityGate, prefix+".plan_security_gate")
	h.boolField(&base.PlanCoverageGate, &overlay.PlanCoverageGate, prefix+".plan_coverage_gate")
	h.boolField(&base.PlanGapAnalysis, &overlay.PlanGapAnalysis, prefix+".plan_gap_analysis")
	h.boolField(&base.PlanChunked, &overlay.PlanChunked, prefix+".plan_chunked")
	h.intField(&base.PlanChunkThreshold, &overlay.PlanChunkThreshold, prefix+".plan_chunk_threshold")

	// Discuss phase enhancements
	h.boolField(&base.DiscussQualityCheck, &overlay.DiscussQualityCheck, prefix+".discuss_quality_check")
	h.boolField(&base.DiscussCompleteness, &overlay.DiscussCompleteness, prefix+".discuss_completeness")
	h.boolField(&base.DiscussFollowUps, &overlay.DiscussFollowUps, prefix+".discuss_follow_ups")

	// Execute phase enhancements
	h.boolField(&base.ExecutePreflight, &overlay.ExecutePreflight, prefix+".execute_preflight")
	h.boolField(&base.ExecuteQualityGate, &overlay.ExecuteQualityGate, prefix+".execute_quality_gate")
	h.boolField(&base.ExecuteLoopDetect, &overlay.ExecuteLoopDetect, prefix+".execute_loop_detect")

	// Verify + Ship phase enhancements
	h.boolField(&base.VerifyReport, &overlay.VerifyReport, prefix+".verify_report")
	h.boolField(&base.VerifySecurity, &overlay.VerifySecurity, prefix+".verify_security")
	h.boolField(&base.ShipPreflight, &overlay.ShipPreflight, prefix+".ship_preflight")
	h.boolField(&base.ShipChangelog, &overlay.ShipChangelog, prefix+".ship_changelog")

	// Initialize phase enhancements
	h.boolField(&base.InitDeepAnalysis, &overlay.InitDeepAnalysis, prefix+".init_deep_analysis")
	h.boolField(&base.InitPreflight, &overlay.InitPreflight, prefix+".init_preflight")

	// Intent classification
	h.boolField(&base.IntentClassification, &overlay.IntentClassification, prefix+".intent_classification")
	h.intField(&base.IntentClassifyTimeoutSecs, &overlay.IntentClassifyTimeoutSecs, prefix+".intent_classify_timeout_secs")

	// Workflow thresholds (F-030, F-031)
	h.intField(&base.MaxHealAttempts, &overlay.MaxHealAttempts, prefix+".max_heal_attempts")
	h.intField(&base.MaxPlanRetries, &overlay.MaxPlanRetries, prefix+".max_plan_retries")

	// Context (F-033)
	h.float64Field(&base.ContextTruncationThreshold, &overlay.ContextTruncationThreshold, prefix+".context_truncation_threshold")

	// Retry policy (F-061)
	h.intField(&base.RetryMaxAttempts, &overlay.RetryMaxAttempts, prefix+".retry_max_attempts")
	h.intField(&base.RetryBaseDelayMs, &overlay.RetryBaseDelayMs, prefix+".retry_base_delay_ms")
	h.intField(&base.RetryMaxDelayMs, &overlay.RetryMaxDelayMs, prefix+".retry_max_delay_ms")
	h.float64Field(&base.RetryBackoffMultiplier, &overlay.RetryBackoffMultiplier, prefix+".retry_backoff_multiplier")

	// Retry-after (F-062)
	h.intField(&base.MaxRetryAfterSecs, &overlay.MaxRetryAfterSecs, prefix+".max_retry_after_secs")

	// Task runner (F-076)
	h.intField(&base.MaxParallelTasks, &overlay.MaxParallelTasks, prefix+".max_parallel_tasks")

	// Coordinator (F-078)
	h.intField(&base.CoordinatorTimeoutSecs, &overlay.CoordinatorTimeoutSecs, prefix+".coordinator_timeout_secs")
}

func (h mergeHelper) mergeLedgerConfig(base, overlay *LedgerConfig, prefix string) {
	h.boolField(&base.Enabled, &overlay.Enabled, prefix+".enabled")
	h.intField(&base.MaxEntries, &overlay.MaxEntries, prefix+".max_entries")
}

func (h mergeHelper) mergeToolsConfig(base, overlay *ToolsConfig, prefix string) {
	h.intField(&base.MaxGlobResults, &overlay.MaxGlobResults, prefix+".max_glob_results")
	h.intField(&base.MaxGrepResults, &overlay.MaxGrepResults, prefix+".max_grep_results")
	h.intField(&base.BashKillGraceSecs, &overlay.BashKillGraceSecs, prefix+".bash_kill_grace_secs")
	h.intField(&base.MaxBackupsPerFile, &overlay.MaxBackupsPerFile, prefix+".max_backups_per_file")
	h.intField(&base.WebfetchMaxRedirects, &overlay.WebfetchMaxRedirects, prefix+".webfetch_max_redirects")
	h.stringField(&base.WebfetchUserAgent, &overlay.WebfetchUserAgent, prefix+".webfetch_user_agent")
	h.sliceField(&base.SkipDirs, &overlay.SkipDirs, prefix+".skip_dirs")
	h.stringField(&base.WebSearchBaseURL, &overlay.WebSearchBaseURL, prefix+".websearch_base_url")
	h.boolField(&base.WebSearchEnabled, &overlay.WebSearchEnabled, prefix+".websearch_enabled")
	h.intField(&base.OutputMaxLines, &overlay.OutputMaxLines, prefix+".output_max_lines")
	h.intField(&base.OutputMaxBytes, &overlay.OutputMaxBytes, prefix+".output_max_bytes")

	// Rate limiting (F-018)
	h.intField(&base.RateLimitBurst, &overlay.RateLimitBurst, prefix+".rate_limit_burst")
	h.intField(&base.RateLimitPerSec, &overlay.RateLimitPerSec, prefix+".rate_limit_per_sec")
	h.intField(&base.DangerousRateLimitBurst, &overlay.DangerousRateLimitBurst, prefix+".dangerous_rate_limit_burst")
	h.intField(&base.DangerousRateLimitPerSec, &overlay.DangerousRateLimitPerSec, prefix+".dangerous_rate_limit_per_sec")
	h.intField(&base.MaxConcurrent, &overlay.MaxConcurrent, prefix+".max_concurrent")

	// Output bounds (F-019)
	h.intField(&base.OutputRetentionDays, &overlay.OutputRetentionDays, prefix+".output_retention_days")

	// DNS (F-023)
	h.intField(&base.DnsCacheTTLSecs, &overlay.DnsCacheTTLSecs, prefix+".dns_cache_ttl_secs")

	// Edit tool (F-024)
	h.float64Field(&base.FuzzyThreshold, &overlay.FuzzyThreshold, prefix+".fuzzy_threshold")
	h.intField(&base.MinLinesForFuzzy, &overlay.MinLinesForFuzzy, prefix+".min_lines_for_fuzzy")

	// Bash (F-020)
	h.intField(&base.BashMaxTimeoutSecs, &overlay.BashMaxTimeoutSecs, prefix+".bash_max_timeout_secs")

	// WebFetch (F-021)
	h.intField(&base.WebfetchMaxRetries, &overlay.WebfetchMaxRetries, prefix+".webfetch_max_retries")
	h.intField(&base.WebfetchRetryDelayMs, &overlay.WebfetchRetryDelayMs, prefix+".webfetch_retry_delay_ms")

	// Execute phase (F-086, F-087)
	h.intField(&base.MaxToolConcurrency, &overlay.MaxToolConcurrency, prefix+".max_tool_concurrency")
	h.intField(&base.LoopDetectWindow, &overlay.LoopDetectWindow, prefix+".loop_detect_window")

	// Dangerous command extensions (F-017)
	h.sliceField(&base.AdditionalBlockedCommands, &overlay.AdditionalBlockedCommands, prefix+".additional_blocked_commands")
	h.sliceField(&base.AdditionalObfuscationPatterns, &overlay.AdditionalObfuscationPatterns, prefix+".additional_obfuscation_patterns")
}

func (h mergeHelper) mergeAgentsConfig(base, overlay *AgentsConfig, prefix string) {
	h.stringField(&base.Default, &overlay.Default, prefix+".default")
	h.stringField(&base.Initialize, &overlay.Initialize, prefix+".initialize")
	h.stringField(&base.Research, &overlay.Research, prefix+".research")
	h.stringField(&base.Plan, &overlay.Plan, prefix+".plan")
	h.stringField(&base.Execute, &overlay.Execute, prefix+".execute")
	h.stringField(&base.Verify, &overlay.Verify, prefix+".verify")
	h.stringField(&base.Ship, &overlay.Ship, prefix+".ship")
	h.stringField(&base.Discuss, &overlay.Discuss, prefix+".discuss")

	if overlay.Profiles != nil {
		if base.Profiles == nil {
			base.Profiles = make(map[string]SubagentProfileConfig)
		}
		for name, overlayProfile := range overlay.Profiles {
			baseProfile, exists := base.Profiles[name]
			if !exists {
				base.Profiles[name] = overlayProfile
				continue
			}
			profilePrefix := prefix + ".profiles." + name
			h.stringField(&baseProfile.Description, &overlayProfile.Description, profilePrefix+".description")
			h.stringField(&baseProfile.Mode, &overlayProfile.Mode, profilePrefix+".mode")
			h.stringField(&baseProfile.SystemPrompt, &overlayProfile.SystemPrompt, profilePrefix+".system_prompt")
			h.stringField(&baseProfile.Model, &overlayProfile.Model, profilePrefix+".model")
			if overlayProfile.Hidden != nil {
				baseProfile.Hidden = overlayProfile.Hidden
			}
			h.sliceField(&baseProfile.AllowedTools, &overlayProfile.AllowedTools, profilePrefix+".allowed_tools")
			h.sliceField(&baseProfile.DeniedTools, &overlayProfile.DeniedTools, profilePrefix+".denied_tools")
			h.intField(&baseProfile.MaxTools, &overlayProfile.MaxTools, profilePrefix+".max_tools")
			h.intField(&baseProfile.MaxTokens, &overlayProfile.MaxTokens, profilePrefix+".max_tokens")
			h.intField(&baseProfile.MaxTurns, &overlayProfile.MaxTurns, profilePrefix+".max_turns")
			h.boolField(&baseProfile.Disabled, &overlayProfile.Disabled, profilePrefix+".disabled")
			base.Profiles[name] = baseProfile
		}
	}
}

func (h mergeHelper) mergeGitConfig(base, overlay *GitConfig, prefix string) {
	h.stringField(&base.CommitPrefix, &overlay.CommitPrefix, prefix+".commit_prefix")
	h.stringField(&base.FixPrefix, &overlay.FixPrefix, prefix+".fix_prefix")
	h.stringField(&base.ShipPrefix, &overlay.ShipPrefix, prefix+".ship_prefix")
	h.stringField(&base.UserName, &overlay.UserName, prefix+".user_name")
	h.stringField(&base.UserEmail, &overlay.UserEmail, prefix+".user_email")
}

func (h mergeHelper) mergeVerifyConfig(base, overlay *VerifyConfig, prefix string) {
	h.stringField(&base.BuildCommand, &overlay.BuildCommand, prefix+".build_command")
	h.stringField(&base.TestCommand, &overlay.TestCommand, prefix+".test_command")
	h.stringField(&base.LintCommand, &overlay.LintCommand, prefix+".lint_command")
}

func (h mergeHelper) mergeIntelligenceConfig(base, overlay *IntelligenceConfig, prefix string) {
	h.stringField(&base.ReproCommand, &overlay.ReproCommand, prefix+".repro_command")
	h.intField(&base.BisectMaxCommits, &overlay.BisectMaxCommits, prefix+".bisect_max_commits")
	h.mergeDepsRiskConfig(&base.DepsRisk, &overlay.DepsRisk, prefix+".deps_risk")
	h.mergeDepsPolicyConfig(&base.DepsPolicy, &overlay.DepsPolicy, prefix+".deps_policy")
}

func (h mergeHelper) mergeDepsRiskConfig(base, overlay *DepsRiskConfig, prefix string) {
	h.intField(&base.StaleMonths, &overlay.StaleMonths, prefix+".stale_months")
	h.sliceField(&base.LicenseAllowlist, &overlay.LicenseAllowlist, prefix+".license_allowlist")
	h.intField(&base.YoungMonths, &overlay.YoungMonths, prefix+".young_months")
}

func (h mergeHelper) mergeDepsPolicyConfig(base, overlay *DepsPolicyConfig, prefix string) {
	h.boolField(&base.RequireApprovalHighRisk, &overlay.RequireApprovalHighRisk, prefix+".require_approval_high_risk")
}

func (h mergeHelper) mergeCompactionConfig(base, overlay *CompactionConfig, prefix string) {
	h.boolField(&base.Auto, &overlay.Auto, prefix+".auto")
	h.intField(&base.Buffer, &overlay.Buffer, prefix+".buffer")
	h.intField(&base.KeepTokens, &overlay.KeepTokens, prefix+".keep_tokens")
	h.boolField(&base.Proactive, &overlay.Proactive, prefix+".proactive")
	h.intField(&base.ToolCallsThreshold, &overlay.ToolCallsThreshold, prefix+".tool_calls_threshold")
	h.intField(&base.PhaseTransitionPct, &overlay.PhaseTransitionPct, prefix+".phase_transition_pct")
	h.stringField(&base.SummaryTemplate, &overlay.SummaryTemplate, prefix+".summary_template")
	h.stringField(&base.SummaryTemplateFile, &overlay.SummaryTemplateFile, prefix+".summary_template_file")
}

func (h mergeHelper) mergeInstructionsConfig(base, overlay *InstructionsConfig, prefix string) {
	h.boolField(&base.Enabled, &overlay.Enabled, prefix+".enabled")
	h.boolField(&base.DisableProject, &overlay.DisableProject, prefix+".disable_project")
}

func (h mergeHelper) mergeSkillsConfig(base, overlay *SkillsConfig, prefix string) {
	h.sliceField(&base.Sources, &overlay.Sources, prefix+".sources")
}

func (h mergeHelper) mergeNarrativeConfig(base, overlay *NarrativeConfig, prefix string) {
	h.stringMapField(&base.TemplateOverrides, &overlay.TemplateOverrides, prefix+".template_overrides")
	h.stringMapField(&base.ClassificationOverrides, &overlay.ClassificationOverrides, prefix+".classification_overrides")
}

func (h mergeHelper) mergeModelCapabilitiesConfig(base, overlay *ModelCapabilitiesConfig, prefix string) {
	h.sliceField(&base.ExtraReasoningPatterns, &overlay.ExtraReasoningPatterns, prefix+".extra_reasoning_patterns")
	h.sliceField(&base.ExtraToolCapablePatterns, &overlay.ExtraToolCapablePatterns, prefix+".extra_tool_capable_patterns")
	h.sliceField(&base.ExtraCompletionOnlyPatterns, &overlay.ExtraCompletionOnlyPatterns, prefix+".extra_completion_only_patterns")
	h.sliceField(&base.ExtraNonChatPatterns, &overlay.ExtraNonChatPatterns, prefix+".extra_non_chat_patterns")

	if overlay.KnownCapabilities != nil {
		if base.KnownCapabilities == nil {
			base.KnownCapabilities = make(map[string]ModelCapabilityOverride)
		}
		for name, overlayCap := range overlay.KnownCapabilities {
			base.KnownCapabilities[name] = overlayCap
		}
	}
}

func (h mergeHelper) mergePromptConfig(base, overlay *PromptConfig, prefix string) {
	h.stringField(&base.SystemPromptFile, &overlay.SystemPromptFile, prefix+".system_prompt_file")
	h.stringField(&base.ProjectPromptDir, &overlay.ProjectPromptDir, prefix+".project_prompt_dir")
	h.stringField(&base.GlobalPromptDir, &overlay.GlobalPromptDir, prefix+".global_prompt_dir")
	h.stringMapField(&base.Overrides, &overlay.Overrides, prefix+".overrides")
	h.stringMapField(&base.ModelTemplateOverrides, &overlay.ModelTemplateOverrides, prefix+".model_template_overrides")
}

func (h mergeHelper) mergeTemplateConfig(base, overlay *TemplateConfig, prefix string) {
	h.stringField(&base.ExternalDir, &overlay.ExternalDir, prefix+".external_dir")
	h.stringField(&base.WebsiteFramework, &overlay.WebsiteFramework, prefix+".website_framework")

	if overlay.CustomPalettes != nil {
		if base.CustomPalettes == nil {
			base.CustomPalettes = make(map[string]map[string]string)
		}
		for paletteName, overlayColors := range overlay.CustomPalettes {
			if base.CustomPalettes[paletteName] == nil {
				base.CustomPalettes[paletteName] = make(map[string]string)
			}
			for k, v := range overlayColors {
				base.CustomPalettes[paletteName][k] = v
			}
		}
	}
}

func (h mergeHelper) mergeExtensionsConfig(base, overlay *ExtensionsConfig, prefix string) {
	h.mergeToolConfigs(&base.Tools, &overlay.Tools, prefix+".tools")
	h.mergeProviderConfigs(&base.Providers, &overlay.Providers, prefix+".providers")
	h.mergeHookConfigs(&base.Hooks, &overlay.Hooks, prefix+".hooks")
}

func (h mergeHelper) mergeToolConfigs(base, overlay *map[string]ExternalToolConfig, prefix string) {
	if overlay == nil || len(*overlay) == 0 {
		return
	}
	if *base == nil {
		*base = make(map[string]ExternalToolConfig)
	}
	for name, overlayTool := range *overlay {
		baseTool, exists := (*base)[name]
		if !exists {
			(*base)[name] = overlayTool
			continue
		}
		toolPrefix := prefix + "." + name
		h.stringField(&baseTool.Command, &overlayTool.Command, toolPrefix+".command")
		h.sliceField(&baseTool.Args, &overlayTool.Args, toolPrefix+".args")
		h.stringMapField(&baseTool.Env, &overlayTool.Env, toolPrefix+".env")
		h.stringField(&baseTool.Timeout, &overlayTool.Timeout, toolPrefix+".timeout")
		(*base)[name] = baseTool
	}
}

func (h mergeHelper) mergeProviderConfigs(base, overlay *map[string]ExternalProviderConfig, prefix string) {
	if overlay == nil || len(*overlay) == 0 {
		return
	}
	if *base == nil {
		*base = make(map[string]ExternalProviderConfig)
	}
	for name, overlayProvider := range *overlay {
		baseProvider, exists := (*base)[name]
		if !exists {
			(*base)[name] = overlayProvider
			continue
		}
		providerPrefix := prefix + "." + name
		h.stringField(&baseProvider.Command, &overlayProvider.Command, providerPrefix+".command")
		h.sliceField(&baseProvider.Args, &overlayProvider.Args, providerPrefix+".args")
		h.stringMapField(&baseProvider.Env, &overlayProvider.Env, providerPrefix+".env")
		h.stringField(&baseProvider.Timeout, &overlayProvider.Timeout, providerPrefix+".timeout")
		(*base)[name] = baseProvider
	}
}

func (h mergeHelper) mergeHookConfigs(base, overlay *map[string]PhaseHookConfig, prefix string) {
	if overlay == nil || len(*overlay) == 0 {
		return
	}
	if *base == nil {
		*base = make(map[string]PhaseHookConfig)
	}
	for name, overlayHook := range *overlay {
		baseHook, exists := (*base)[name]
		if !exists {
			(*base)[name] = overlayHook
			continue
		}
		hookPrefix := prefix + "." + name
		h.stringField(&baseHook.Command, &overlayHook.Command, hookPrefix+".command")
		h.sliceField(&baseHook.Args, &overlayHook.Args, hookPrefix+".args")
		h.stringMapField(&baseHook.Env, &overlayHook.Env, hookPrefix+".env")
		h.sliceField(&baseHook.Phases, &overlayHook.Phases, hookPrefix+".phases")
		h.sliceField(&baseHook.HookTypes, &overlayHook.HookTypes, hookPrefix+".hook_types")
		h.stringField(&baseHook.Timeout, &overlayHook.Timeout, hookPrefix+".timeout")
		(*base)[name] = baseHook
	}
}
