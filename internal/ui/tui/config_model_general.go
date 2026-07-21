package tui

import (
	"fmt"
	"strconv"

	"github.com/eshanized/M31A/internal/core/config"
)

// getGeneralFieldValue reads general section values (model, permissions, agents, git, verify) from config.
func getGeneralFieldValue(c *config.Config, key string) (string, bool) {
	switch key {
	// Model
	case "model.default":
		return c.Model.Default, true
	case "model.context_warning_threshold":
		return fmt.Sprintf("%.2f", c.Model.ContextWarningThreshold), true
	case "model.show_thinking_by_default":
		return boolStr(c.Model.ShowThinkingByDefault), true
	case "model.auto_collapse_tools":
		return boolStr(c.Model.AutoCollapseTools), true
	case "model.auto_arbitrage":
		return boolStr(c.Model.AutoArbitrage), true
	case "model.arbitrage_threshold":
		return fmt.Sprintf("%.2f", c.Model.ArbitrageThreshold), true
	case "model.default_context_length":
		return fmt.Sprintf("%d", c.Model.DefaultContextLength), true
	case "model.token_ema_alpha":
		return fmt.Sprintf("%.2f", c.Model.TokenEMAAlpha), true
	// Permissions
	case "permissions.default_mode":
		return c.Permissions.DefaultMode, true
	case "permissions.timeout_seconds":
		return fmt.Sprintf("%d", c.Permissions.TimeoutSeconds), true
	// Agents
	case "agents.default":
		return c.Agents.Default, true
	case "agents.plan":
		return c.Agents.Plan, true
	case "agents.execute":
		return c.Agents.Execute, true
	case "agents.verify":
		return c.Agents.Verify, true
	case "agents.ship":
		return c.Agents.Ship, true
	case "agents.discuss":
		return c.Agents.Discuss, true
	// Git
	case "git.commit_prefix":
		return c.Git.CommitPrefix, true
	case "git.fix_prefix":
		return c.Git.FixPrefix, true
	case "git.ship_prefix":
		return c.Git.ShipPrefix, true
	case "git.user_name":
		return c.Git.UserName, true
	case "git.user_email":
		return c.Git.UserEmail, true
	// Verify
	case "verify.build_command":
		return c.Verify.BuildCommand, true
	case "verify.test_command":
		return c.Verify.TestCommand, true
	}
	return "", false
}

// setGeneralFieldValue writes general section values to config.
func setGeneralFieldValue(c *config.Config, key string, val string) bool {
	switch key {
	// Model
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
	// Permissions
	case "permissions.default_mode":
		c.Permissions.DefaultMode = val
	case "permissions.timeout_seconds":
		if v, err := strconv.Atoi(val); err == nil {
			c.Permissions.TimeoutSeconds = v
		}
	// Agents
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
	// Git
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
	// Verify
	case "verify.build_command":
		c.Verify.BuildCommand = val
	case "verify.test_command":
		c.Verify.TestCommand = val
	default:
		return false
	}
	return true
}
