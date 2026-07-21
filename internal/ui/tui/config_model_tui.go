package tui

import (
	"fmt"
	"strconv"

	"github.com/eshanized/M31A/internal/config"
)

// getTUIFieldValue reads UI section values from config.
func getTUIFieldValue(c *config.Config, key string) (string, bool) {
	switch key {
	case "ui.compact_mode":
		return boolStr(c.UI.CompactMode), true
	case "ui.show_token_usage":
		return boolStr(c.UI.ShowTokenUsage), true
	case "ui.show_cost_estimate":
		return boolStr(c.UI.ShowCostEstimate), true
	case "ui.max_iterations":
		return fmt.Sprintf("%d", c.UI.MaxIterations), true
	case "ui.leader_key":
		return c.UI.LeaderKey, true
	case "ui.leader_timeout_ms":
		return fmt.Sprintf("%d", c.UI.LeaderTimeoutMs), true
	case "ui.sidebar_width_threshold":
		return fmt.Sprintf("%d", c.UI.SidebarWidthThreshold), true
	case "ui.discuss_timeout":
		return fmt.Sprintf("%d", c.UI.DiscussTimeout), true
	case "ui.thinking_max_lines":
		return fmt.Sprintf("%d", c.UI.ThinkingMaxLines), true
	case "ui.permission_modal_width":
		return fmt.Sprintf("%d", c.UI.PermissionModalWidth), true
	case "ui.sidebar_width":
		return fmt.Sprintf("%d", c.UI.SidebarWidth), true
	case "ui.max_message_history":
		return fmt.Sprintf("%d", c.UI.MaxMessageHistory), true
	case "ui.fallback_banner_timeout_secs":
		return fmt.Sprintf("%d", c.UI.FallbackBannerSecs), true
	case "ui.default_log_lines":
		return fmt.Sprintf("%d", c.UI.DefaultLogLines), true
	case "ui.session_list_limit":
		return fmt.Sprintf("%d", c.UI.SessionListLimit), true
	case "ui.thinking_opacity":
		return fmt.Sprintf("%.1f", c.UI.ThinkingOpacity), true
	case "ui.frecent_history_size":
		return fmt.Sprintf("%d", c.UI.FrecentHistorySize), true
	}
	return "", false
}

// setTUIFieldValue writes UI section values to config.
func setTUIFieldValue(c *config.Config, key string, val string) bool {
	switch key {
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
	default:
		return false
	}
	return true
}
