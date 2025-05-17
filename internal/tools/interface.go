package tools

import (
	"context"

	"github.com/eshanized/M31A/internal/types"
)

type PermissionRequest struct {
	ToolName    string          `json:"tool_name"`
	Command     string          `json:"command"`
	RiskLevel   types.RiskLevel `json:"risk_level"`
	TimeoutSecs int             `json:"timeout_secs"`
	// Rule context (populated when a permission rule matched)
	RuleTool    string `json:"rule_tool,omitempty"`
	RulePattern string `json:"rule_pattern,omitempty"`
	RuleAction  string `json:"rule_action,omitempty"`
}

type PermissionResponse struct {
	Allowed  bool `json:"allowed"`
	Remember bool `json:"remember"`
}

// PermissionContext carries additional info about a matched rule
// for display in the permission modal.
type PermissionContext struct {
	RuleTool    string // Tool name from the matched rule
	RulePattern string // Pattern from the matched rule
	RuleAction  string // The action that triggered (allow/deny/ask)
	Source      string // Source of the decision: "rule", "agent_default", or "risk_level"
}

type PermissionGate interface {
	RequestPermission(ctx context.Context, req PermissionRequest) (PermissionResponse, error)
}
