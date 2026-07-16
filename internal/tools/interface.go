package tools

import (
	"sync/atomic"

	"github.com/eshanized/M31A/internal/types"
)

// permissionRequestID is a monotonically increasing counter for correlating
// permission requests with responses. This prevents response mix-ups when
// multiple permission requests are in flight (V1.1 concurrent subagents).
var permissionRequestID atomic.Int64

func nextPermissionRequestID() int64 {
	return permissionRequestID.Add(1)
}

type PermissionRequest struct {
	ID          int64           `json:"id"`
	ToolName    string          `json:"tool_name"`
	Command     string          `json:"command"`
	RiskLevel   types.RiskLevel `json:"risk_level"`
	TimeoutSecs int             `json:"timeout_secs"`
	// QueueDepth is the number of pending permission requests behind this one.
	QueueDepth int `json:"queue_depth"`
	// Rule context (populated when a permission rule matched)
	RuleTool    string `json:"rule_tool,omitempty"`
	RulePattern string `json:"rule_pattern,omitempty"`
	RuleAction  string `json:"rule_action,omitempty"`
	// Policy indicates how permissions should be handled (AllowSafe/DenyAll/Prompt)
	Policy PermissionPolicy `json:"policy,omitempty"`
}

// PermissionPolicy defines how permissions are handled in headless mode
type PermissionPolicy string

const (
	PolicyAllowSafe PermissionPolicy = "allow-safe"
	PolicyDenyAll   PermissionPolicy = "deny-all"
	PolicyPrompt    PermissionPolicy = "prompt"
)

func (p PermissionPolicy) IsValid() bool {
	switch p {
	case PolicyAllowSafe, PolicyDenyAll, PolicyPrompt:
		return true
	default:
		return false
	}
}

// ShouldAutoAllow determines if a tool is automatically allowed based on policy
func (p PermissionPolicy) ShouldAutoAllow(toolName string, risk types.RiskLevel) bool {
	switch p {
	case PolicyAllowSafe:
		return risk != types.RiskDangerous && risk != types.RiskDestructive
	case PolicyDenyAll:
		return false
	default:
		return false
	}
}

type PermissionResponse struct {
	RequestID  int64 `json:"request_id"`
	Allowed    bool  `json:"allowed"`
	Remember   bool  `json:"remember"`
	ApproveAll bool  `json:"approve_all"` // approve all pending + future for this tool+risk
}

// PermissionContext carries additional info about a matched rule
// for display in the permission modal.
type PermissionContext struct {
	RuleTool    string // Tool name from the matched rule
	RulePattern string // Pattern from the matched rule
	RuleAction  string // The action that triggered (allow/deny/ask)
	Source      string // Source of the decision: "rule", "agent_default", or "risk_level"
}
