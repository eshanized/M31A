package tui

import (
	"fmt"
	"strconv"

	"github.com/eshanized/M31A/internal/config"
)

// getAdvancedFieldValue reads advanced section values (features, ledger, tools) from config.
func getAdvancedFieldValue(c *config.Config, key string) (string, bool) {
	switch key {
	// Features
	case "features.auto_backup":
		return boolStr(c.Features.AutoBackup), true
	case "features.resume_on_startup":
		return boolStr(c.Features.ResumeOnStartup), true
	case "features.model_cache_ttl_minutes":
		return fmt.Sprintf("%d", c.Features.ModelCacheTTLMinutes), true
	case "features.model_cache_stale_hours":
		return fmt.Sprintf("%d", c.Features.ModelCacheStaleHours), true
	case "features.healthcheck_live_ms":
		return fmt.Sprintf("%d", c.Features.HealthCheckLiveMs), true
	case "features.healthcheck_slow_ms":
		return fmt.Sprintf("%d", c.Features.HealthCheckSlowMs), true
	case "features.session_id_length":
		return fmt.Sprintf("%d", c.Features.SessionIDLength), true
	case "features.max_recent_models":
		return fmt.Sprintf("%d", c.Features.MaxRecentModels), true
	case "features.session_retention_days":
		return fmt.Sprintf("%d", c.Features.SessionRetentionDays), true
	case "features.health_check_timeout_secs":
		return fmt.Sprintf("%d", c.Features.HealthCheckTimeoutSecs), true
	case "features.rate_limit_backoff_secs":
		return fmt.Sprintf("%d", c.Features.RateLimitBackoffSecs), true
	case "features.budget_limit_usd":
		return fmt.Sprintf("%.2f", c.Features.BudgetLimitUSD), true
	// Ledger
	case "ledger.enabled":
		return boolStr(c.Ledger.Enabled), true
	case "ledger.max_entries":
		return fmt.Sprintf("%d", c.Ledger.MaxEntries), true
	// Tools
	case "tools.max_glob_results":
		return fmt.Sprintf("%d", c.Tools.MaxGlobResults), true
	case "tools.max_grep_results":
		return fmt.Sprintf("%d", c.Tools.MaxGrepResults), true
	case "tools.bash_kill_grace_secs":
		return fmt.Sprintf("%d", c.Tools.BashKillGraceSecs), true
	case "tools.max_backups_per_file":
		return fmt.Sprintf("%d", c.Tools.MaxBackupsPerFile), true
	case "tools.webfetch_max_redirects":
		return fmt.Sprintf("%d", c.Tools.WebfetchMaxRedirects), true
	case "tools.webfetch_user_agent":
		return c.Tools.WebfetchUserAgent, true
	}
	return "", false
}

// setAdvancedFieldValue writes advanced section values to config.
func setAdvancedFieldValue(c *config.Config, key string, val string) bool {
	switch key {
	// Features
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
	// Ledger
	case "ledger.enabled":
		c.Ledger.Enabled = val == "yes"
	case "ledger.max_entries":
		if v, err := strconv.Atoi(val); err == nil {
			c.Ledger.MaxEntries = v
		}
	// Tools
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
	default:
		return false
	}
	return true
}
