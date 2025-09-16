package tools

import (
	"context"
	"encoding/json"
	"fmt"
	"strings"
	"time"

	"github.com/bmatcuk/doublestar/v4"
	"github.com/eshanized/M31A/internal/config"
	m31errors "github.com/eshanized/M31A/internal/errors"
	"github.com/eshanized/M31A/internal/types"
)

func (d *Dispatcher) ApprovePermission(requestID int64, allowed bool, remember bool) {
	d.responseCh <- PermissionResponse{RequestID: requestID, Allowed: allowed, Remember: remember}
}

func (d *Dispatcher) SetPermission(toolName string, allowed bool) {
	d.mu.Lock()
	defer d.mu.Unlock()
	d.permissions[toolName] = allowed
}

func (d *Dispatcher) SelectAgent(agent string) error {
	d.mu.Lock()
	defer d.mu.Unlock()

	if agent == "default" {
		d.activeAgent = "default"
		d.rules = make([]config.PermissionRule, len(d.originalRules))
		copy(d.rules, d.originalRules)
		return nil
	}

	profile, ok := d.agents[agent]
	if !ok {
		return fmt.Errorf("unknown agent profile: %s", agent)
	}

	d.activeAgent = agent
	d.rules = make([]config.PermissionRule, len(profile.Rules))
	copy(d.rules, profile.Rules)
	return nil
}

func (d *Dispatcher) checkPermission(toolName string, input types.ToolInput) (bool, *PermissionContext, error) {
	for _, rule := range d.rules {
		if rule.Tool != "" && !matchToolName(rule.Tool, toolName) {
			continue
		}

		if rule.Pattern != "" {
			matched := matchAnyParamValue(rule.Pattern, input.Params)
			if !matched {
				continue
			}
		}

		pctx := &PermissionContext{
			RuleTool:    rule.Tool,
			RulePattern: rule.Pattern,
			RuleAction:  rule.Action,
			Source:      "rule",
		}

		switch rule.Action {
		case "allow":
			return true, pctx, nil
		case "deny":
			return false, pctx, m31errors.ErrPermissionDenied
		case "ask":
			return false, pctx, nil
		}
	}

	if d.activeAgent != "default" {
		if profile, ok := d.agents[d.activeAgent]; ok && profile.DefaultAction != "" {
			switch profile.DefaultAction {
			case "allow":
				return true, &PermissionContext{Source: "agent_default"}, nil
			case "deny":
				return false, &PermissionContext{Source: "agent_default"}, m31errors.ErrPermissionDenied
			}
		}
	}

	return false, &PermissionContext{Source: "risk_level"}, nil
}

func matchToolName(pattern, name string) bool {
	if pattern == name || pattern == "*" {
		return true
	}
	matched, _ := doublestar.Match(pattern, name)
	return matched
}

func riskLevelValue(r types.RiskLevel) int {
	switch r {
	case types.RiskSafe:
		return 0
	case types.RiskMedium:
		return 1
	case types.RiskDangerous:
		return 2
	case types.RiskDestructive:
		return 3
	default:
		return -1
	}
}

func matchAnyParamValue(pattern string, params map[string]any) bool {
	paramKeys := []string{"path", "url", "command", "pattern"}
	for _, key := range paramKeys {
		v, ok := params[key]
		if !ok {
			continue
		}
		if matchValue(v, pattern) {
			return true
		}
	}
	return false
}

func matchValue(v any, pattern string) bool {
	switch val := v.(type) {
	case string:
		matched, _ := doublestar.Match(pattern, val)
		return matched
	case map[string]any:
		for _, inner := range val {
			if matchValue(inner, pattern) {
				return true
			}
		}
	case []any:
		for _, item := range val {
			if matchValue(item, pattern) {
				return true
			}
		}
	default:
		// int, bool, etc. — stringify for matching
		s := fmt.Sprintf("%v", val)
		matched, _ := doublestar.Match(pattern, s)
		return matched
	}
	return false
}

func (d *Dispatcher) askPermission(ctx context.Context, call types.ToolCall, risk types.RiskLevel, pctx *PermissionContext) error {
	reqID := nextPermissionRequestID()
	req := PermissionRequest{
		ID:          reqID,
		ToolName:    call.Name,
		Command:     extractCommandString(call.Name, call.Input),
		RiskLevel:   risk,
		TimeoutSecs: d.permissionTimeout,
		RuleTool:    pctx.RuleTool,
		RulePattern: pctx.RulePattern,
		RuleAction:  pctx.RuleAction,
	}

	select {
	case d.requestCh <- req:
	default:
		return m31errors.ErrPermissionDenied
	}

	// Create a timeout context for the permission request
	timeoutCtx, cancel := context.WithTimeout(ctx, time.Duration(req.TimeoutSecs)*time.Second)
	defer cancel()

	// RC-2 fix: match responses by request ID to prevent mix-ups
	var resp PermissionResponse
	for {
		select {
		case r := <-d.responseCh:
			if r.RequestID == reqID {
				resp = r
				goto done
			}
			// Not ours — put it back and keep looking
			select {
			case d.responseCh <- r:
			default:
			}
		case <-timeoutCtx.Done():
			return m31errors.ErrPermissionDenied
		case <-ctx.Done():
			return ctx.Err()
		}
	}
done:

	if !resp.Allowed {
		return m31errors.ErrPermissionDenied
	}

	if resp.Remember {
		d.mu.Lock()
		d.permissions[call.Name] = resp.Allowed
		d.mu.Unlock()
	}

	return nil
}

func (d *Dispatcher) askPermissionWithAgentDefault(ctx context.Context, call types.ToolCall, risk types.RiskLevel) error {
	reqID := nextPermissionRequestID()
	req := PermissionRequest{
		ID:          reqID,
		ToolName:    call.Name,
		Command:     extractCommandString(call.Name, call.Input),
		RiskLevel:   risk,
		TimeoutSecs: d.permissionTimeout,
		RuleAction:  "ask",
	}

	select {
	case d.requestCh <- req:
	default:
		return m31errors.ErrPermissionDenied
	}

	// Create a timeout context for the permission request
	timeoutCtx, cancel := context.WithTimeout(ctx, time.Duration(req.TimeoutSecs)*time.Second)
	defer cancel()

	var resp PermissionResponse
	for {
		select {
		case r := <-d.responseCh:
			if r.RequestID == reqID {
				resp = r
				goto done
			}
			select {
			case d.responseCh <- r:
			default:
			}
		case <-timeoutCtx.Done():
			return m31errors.ErrPermissionDenied
		case <-ctx.Done():
			return ctx.Err()
		}
	}
done:

	if !resp.Allowed {
		return m31errors.ErrPermissionDenied
	}

	if resp.Remember {
		d.mu.Lock()
		d.permissions[call.Name] = resp.Allowed
		d.mu.Unlock()
	}

	return nil
}

func (d *Dispatcher) askPermissionFallback(ctx context.Context, call types.ToolCall, risk types.RiskLevel) error {
	d.mu.RLock()
	allowed, remembered := d.permissions[call.Name]
	d.mu.RUnlock()

	if !remembered || !allowed {
		reqID := nextPermissionRequestID()
		req := PermissionRequest{
			ID:          reqID,
			ToolName:    call.Name,
			Command:     extractCommandString(call.Name, call.Input),
			RiskLevel:   risk,
			TimeoutSecs: d.permissionTimeout,
		}

		select {
		case d.requestCh <- req:
		default:
			return m31errors.ErrPermissionDenied
		}

		// Create a timeout context for the permission request
		timeoutCtx, cancel := context.WithTimeout(ctx, time.Duration(req.TimeoutSecs)*time.Second)
		defer cancel()

		var resp PermissionResponse
		for {
			select {
			case r := <-d.responseCh:
				if r.RequestID == reqID {
					resp = r
					goto done
				}
				select {
				case d.responseCh <- r:
				default:
				}
			case <-timeoutCtx.Done():
				return m31errors.ErrPermissionDenied
			case <-ctx.Done():
				return ctx.Err()
			}
		}
	done:

		if !resp.Allowed {
			return m31errors.ErrPermissionDenied
		}

		if resp.Remember {
			d.mu.Lock()
			d.permissions[call.Name] = resp.Allowed
			d.mu.Unlock()
		}
	}

	return nil
}

func extractCommandString(toolName string, input json.RawMessage) string {
	if len(input) == 0 {
		return ""
	}

	if len(toolName) > 0 {
		toolName = strings.ToUpper(toolName[:1]) + toolName[1:]
	}

	var toolInput struct {
		Name   string         `json:"name"`
		Params map[string]any `json:"params"`
	}
	if err := json.Unmarshal(input, &toolInput); err == nil && len(toolInput.Params) > 0 {
		if s := extractFromParams(toolName, toolInput.Params); s != "" {
			return s
		}
	}

	var params map[string]any
	if err := json.Unmarshal(input, &params); err == nil {
		if s := extractFromParams(toolName, params); s != "" {
			return s
		}
	}

	return string(input)
}

func extractFromParams(toolName string, params map[string]any) string {
	switch toolName {
	case "Bash":
		if cmd, ok := params["command"].(string); ok {
			return cmd
		}
	case "FileRead":
		if path, ok := params["path"].(string); ok {
			return fmt.Sprintf("read %s", path)
		}
	case "FileWrite":
		if path, ok := params["path"].(string); ok {
			return fmt.Sprintf("write %s", path)
		}
	case "Glob":
		if pattern, ok := params["pattern"].(string); ok {
			return fmt.Sprintf("glob %s", pattern)
		}
	case "Grep":
		if pattern, ok := params["pattern"].(string); ok {
			return fmt.Sprintf("grep %s", pattern)
		}
	case "Edit":
		if path, ok := params["path"].(string); ok {
			return fmt.Sprintf("edit %s", path)
		}
	case "WebFetch":
		if u, ok := params["url"].(string); ok {
			return u
		}
	case "TodoWrite":
		if todos, ok := params["todos"].([]any); ok {
			return fmt.Sprintf("todos: %d items", len(todos))
		}
	case "AskUserQuestion":
		if q, ok := params["question"].(string); ok {
			return q
		}
	}
	return ""
}
