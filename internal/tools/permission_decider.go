package tools

import (
	"context"
	"sync"
	"time"

	"github.com/eshanized/M31A/internal/core/types"
)

type PermissionDecision struct {
	Allowed  bool
	ErrorMsg string
}

type PermissionDecider interface {
	Decide(ctx context.Context, toolName string, risk types.RiskLevel, input types.ToolInput) PermissionDecision
}

type InteractiveDecider struct {
	requestCh  chan PermissionRequest
	responseCh chan PermissionResponse
	pending    sync.Map
	timeout    int
}

var _ PermissionDecider = (*InteractiveDecider)(nil)

func NewInteractiveDecider(requestCh chan PermissionRequest, responseCh chan PermissionResponse, timeout int) *InteractiveDecider {
	return &InteractiveDecider{
		requestCh:  requestCh,
		responseCh: responseCh,
		timeout:    timeout,
	}
}

func (p *InteractiveDecider) Decide(ctx context.Context, toolName string, risk types.RiskLevel, input types.ToolInput) PermissionDecision {
	req := PermissionRequest{
		ID:          nextPermissionRequestID(),
		ToolName:    toolName,
		RiskLevel:   risk,
		TimeoutSecs: p.timeout,
	}

	respCh := make(chan PermissionResponse, 1)
	p.pending.Store(req.ID, respCh)
	defer func() {
		go func() {
			time.Sleep(200 * time.Millisecond)
			p.pending.Delete(req.ID)
		}()
	}()

	select {
	case p.requestCh <- req:
		select {
		case resp := <-respCh:
			if !resp.Allowed {
				return PermissionDecision{Allowed: false, ErrorMsg: "Permission denied by user"}
			}
			return PermissionDecision{Allowed: true}
		case <-time.After(time.Duration(p.timeout) * time.Second):
			return PermissionDecision{Allowed: false, ErrorMsg: "Permission request timed out"}
		}
	default:
		return PermissionDecision{Allowed: false, ErrorMsg: "Permission request queue full"}
	}
}

type HeadlessDenyDecider struct{}

var _ PermissionDecider = (*HeadlessDenyDecider)(nil)

func (p *HeadlessDenyDecider) Decide(ctx context.Context, toolName string, risk types.RiskLevel, input types.ToolInput) PermissionDecision {
	return PermissionDecision{
		Allowed:  false,
		ErrorMsg: "Permission denied: headless mode denies interactive requests by default. Use --permission-mode=allow to override.",
	}
}

type HeadlessAllowDecider struct{}

var _ PermissionDecider = (*HeadlessAllowDecider)(nil)

func (p *HeadlessAllowDecider) Decide(ctx context.Context, toolName string, risk types.RiskLevel, input types.ToolInput) PermissionDecision {
	return PermissionDecision{Allowed: true}
}

func NewHeadlessDenyDecider() *HeadlessDenyDecider {
	return &HeadlessDenyDecider{}
}

func NewHeadlessAllowDecider() *HeadlessAllowDecider {
	return &HeadlessAllowDecider{}
}

type CIPolicy struct {
	allowedTools map[string]bool
}

var _ PermissionDecider = (*CIPolicy)(nil)

func NewCIPolicy() *CIPolicy {
	return &CIPolicy{
		allowedTools: map[string]bool{
			"Bash":       true,
			"FileRead":   true,
			"FileWrite":  true,
			"FileList":   true,
			"Glob":       true,
			"Grep":       true,
			"FileEdit":   true,
			"FileDelete": false,
			"FileMove":   false,
		},
	}
}

func (p *CIPolicy) Decide(ctx context.Context, toolName string, risk types.RiskLevel, input types.ToolInput) PermissionDecision {
	if p.allowedTools[toolName] {
		return PermissionDecision{Allowed: true}
	}
	return PermissionDecision{
		Allowed:  false,
		ErrorMsg: "Tool not allowed in CI policy: " + toolName,
	}
}

type TestDecider struct {
	requestCh  chan PermissionRequest
	responseCh chan PermissionResponse
	pending    sync.Map
	timeout    int
}

var _ PermissionDecider = (*TestDecider)(nil)

func NewTestDecider(requestCh chan PermissionRequest, responseCh chan PermissionResponse, timeout int) *TestDecider {
	return &TestDecider{
		requestCh:  requestCh,
		responseCh: responseCh,
		timeout:    timeout,
	}
}

func (p *TestDecider) Decide(ctx context.Context, toolName string, risk types.RiskLevel, input types.ToolInput) PermissionDecision {
	switch risk {
	case types.RiskSafe, types.RiskMedium:
		return PermissionDecision{Allowed: true}
	case types.RiskDangerous:
		// For testing, send request to channel but auto-approve immediately
		req := PermissionRequest{
			ID:          nextPermissionRequestID(),
			ToolName:    toolName,
			RiskLevel:   risk,
			TimeoutSecs: p.timeout,
		}

		select {
		case p.requestCh <- req:
			// Auto-approve immediately
			return PermissionDecision{Allowed: true}
		case <-time.After(100 * time.Millisecond):
			return PermissionDecision{Allowed: false, ErrorMsg: "Permission request queue full"}
		}
	case types.RiskDestructive:
		return PermissionDecision{
			Allowed:  false,
			ErrorMsg: "Destructive tools not allowed in tests",
		}
	}
	return PermissionDecision{Allowed: false, ErrorMsg: "Unknown risk level"}
}
