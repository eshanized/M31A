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
}

type PermissionResponse struct {
	Allowed  bool `json:"allowed"`
	Remember bool `json:"remember"`
}

type Dispatcher struct {
	Register func(name string, tool types.Tool)
	Execute  func(ctx context.Context, call types.ToolCall) (types.ToolResult, error)
	List     func() []string
}
