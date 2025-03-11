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

type PermissionGate interface {
	RequestPermission(ctx context.Context, req PermissionRequest) (PermissionResponse, error)
}
