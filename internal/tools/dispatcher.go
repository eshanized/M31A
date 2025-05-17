package tools

import (
	"context"
	"encoding/json"
	"fmt"
	"sort"
	"strings"
	"sync"
	"time"

	"github.com/bmatcuk/doublestar/v4"
	"github.com/eshanized/M31A/internal/config"
	m31errors "github.com/eshanized/M31A/internal/errors"
	"github.com/eshanized/M31A/internal/types"
)

type Dispatcher struct {
	mu             sync.RWMutex
	tools          map[string]types.Tool
	permissions    map[string]bool
	requestCh      chan PermissionRequest
	responseCh     chan PermissionResponse
	todoWrite      *TodoWrite
	questionReqCh  chan QuestionRequest
	questionRespCh chan QuestionResponse
	// Permission rule system
	rules         []config.PermissionRule                  // active rules
	originalRules []config.PermissionRule                  // preserved for "default" agent reset
	agents        map[string]config.PermissionsAgentConfig // per-agent profiles
	activeAgent   string                                   // "default" = built-in rules
}

func NewDispatcher(cfg *config.PermissionsConfig) *Dispatcher {
	d := &Dispatcher{
		tools:          make(map[string]types.Tool),
		permissions:    make(map[string]bool),
		requestCh:      make(chan PermissionRequest, 8),
		responseCh:     make(chan PermissionResponse),
		questionReqCh:  make(chan QuestionRequest, 4),
		questionRespCh: make(chan QuestionResponse),
		rules:          []config.PermissionRule{},
		originalRules:  []config.PermissionRule{},
		agents:         make(map[string]config.PermissionsAgentConfig),
		activeAgent:    "default",
	}
	if cfg != nil {
		if cfg.Rules != nil {
			d.rules = make([]config.PermissionRule, len(cfg.Rules))
			copy(d.rules, cfg.Rules)
			d.originalRules = make([]config.PermissionRule, len(cfg.Rules))
			copy(d.originalRules, cfg.Rules)
		}
		if cfg.Agents != nil {
			d.agents = cfg.Agents
		}
	}
	return d
}

func (d *Dispatcher) Register(tool types.Tool) {
	d.mu.Lock()
	defer d.mu.Unlock()
	name := tool.Name()
	if _, ok := d.tools[name]; ok {
		panic(fmt.Sprintf("tool already registered: %s", name))
	}
	d.tools[name] = tool
}

func (d *Dispatcher) Execute(ctx context.Context, call types.ToolCall) (types.ToolResult, error) {
	start := time.Now()

	d.mu.RLock()
	tool, ok := d.tools[call.Name]
	d.mu.RUnlock()

	if !ok {
		return types.ToolResult{}, fmt.Errorf("%w: unknown tool: %s", m31errors.ErrToolExecution, call.Name)
	}

	var input types.ToolInput
	if err := json.Unmarshal(call.Input, &input); err != nil {
		return types.ToolResult{}, fmt.Errorf("tool %s: invalid input JSON: %w", call.Name, err)
	}
	input.Name = call.Name

	// Step 1: Check permission rules (allow/deny/ask)
	d.mu.RLock()
	allowedByRule, pctx, ruleErr := d.checkPermission(call.Name, input)
	d.mu.RUnlock()

	if ruleErr != nil {
		// Rule denied execution
		return types.ToolResult{Error: ruleErr.Error()}, nil
	}

	risk := tool.RiskLevel()

	if !allowedByRule {
		// Check if a rule triggered "ask" — must prompt user even for safe tools
		if pctx != nil && pctx.Source == "rule" && pctx.RuleAction == "ask" {
			req := PermissionRequest{
				ToolName:    call.Name,
				Command:     extractCommandString(call.Name, call.Input),
				RiskLevel:   risk,
				TimeoutSecs: 300,
			}

			select {
			case d.requestCh <- req:
			default:
				return types.ToolResult{}, m31errors.ErrPermissionDenied
			}

			var resp PermissionResponse
			select {
			case resp = <-d.responseCh:
			case <-ctx.Done():
				return types.ToolResult{}, ctx.Err()
			}

			if !resp.Allowed {
				return types.ToolResult{}, m31errors.ErrPermissionDenied
			}

			if resp.Remember {
				d.mu.Lock()
				d.permissions[call.Name] = resp.Allowed
				d.mu.Unlock()
			}
		} else {
			// No matching rule — fall through to RiskLevel default behavior
			if risk == types.RiskDangerous || risk == types.RiskDestructive {
				d.mu.RLock()
				allowed, remembered := d.permissions[call.Name]
				d.mu.RUnlock()

				if !remembered || !allowed {
					req := PermissionRequest{
						ToolName:    call.Name,
						Command:     extractCommandString(call.Name, call.Input),
						RiskLevel:   risk,
						TimeoutSecs: 300,
					}

					select {
					case d.requestCh <- req:
					default:
						return types.ToolResult{}, m31errors.ErrPermissionDenied
					}

					var resp PermissionResponse
					select {
					case resp = <-d.responseCh:
					case <-ctx.Done():
						return types.ToolResult{}, ctx.Err()
					}

					if !resp.Allowed {
						return types.ToolResult{}, m31errors.ErrPermissionDenied
					}

					if resp.Remember {
						d.mu.Lock()
						d.permissions[call.Name] = resp.Allowed
						d.mu.Unlock()
					}
				}
			}
			// RiskSafe and RiskMedium tools execute without permission (existing behavior)
		}
	}

	result, err := tool.Execute(ctx, input)
	elapsed := time.Since(start).Milliseconds()

	res := types.ToolResult{
		ToolCallID: call.ID,
		Output:     result.Output,
		DurationMs: elapsed,
		Truncated:  result.Truncated,
	}
	if err != nil {
		res.Error = err.Error()
	}

	return res, nil
}

func (d *Dispatcher) ApprovePermission(allowed bool, remember bool) {
	d.responseCh <- PermissionResponse{Allowed: allowed, Remember: remember}
}

// SetPermission sets a remembered permission directly (useful for tests).
func (d *Dispatcher) SetPermission(toolName string, allowed bool) {
	d.mu.Lock()
	defer d.mu.Unlock()
	d.permissions[toolName] = allowed
}

func (d *Dispatcher) List() []string {
	d.mu.RLock()
	defer d.mu.RUnlock()
	names := make([]string, 0, len(d.tools))
	for name := range d.tools {
		names = append(names, name)
	}
	sort.Strings(names)
	return names
}

func (d *Dispatcher) GetTool(name string) (types.Tool, bool) {
	d.mu.RLock()
	defer d.mu.RUnlock()
	t, ok := d.tools[name]
	return t, ok
}

// SelectAgent switches the dispatcher to use a named agent's permission profile.
// The "default" agent restores the global rules from the initial config.
// Returns an error if the agent name is unknown.
func (d *Dispatcher) SelectAgent(agent string) error {
	d.mu.Lock()
	defer d.mu.Unlock()

	if agent == "default" {
		d.activeAgent = "default"
		// Restore original global rules
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

// checkPermission evaluates rules against tool name and input params.
// Returns allowed=true if the tool is permitted, false if blocked or needs ask.
// When err is non-nil (ErrPermissionDenied), the tool is denied immediately.
// When err is nil and allowed=false, the caller must check PermissionContext
// to determine if "ask" was triggered or no rule matched (fall through to RiskLevel).
func (d *Dispatcher) checkPermission(toolName string, input types.ToolInput) (bool, *PermissionContext, error) {
	// Step 1: Check rules first (rules take priority)
	for _, rule := range d.rules {
		// Tool filter: if rule.Tool is set, only match that tool name
		if rule.Tool != "" && !matchToolName(rule.Tool, toolName) {
			continue
		}

		// Pattern filter: if rule.Pattern is set, check if it matches
		// any string value in ToolInput.Params
		if rule.Pattern != "" {
			matched := matchAnyParamValue(rule.Pattern, input.Params)
			if !matched {
				continue
			}
		}

		// Rule matched
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
			return false, pctx, nil // nil error means "ask the user"
		}
	}

	// Step 2: No rule matched — use agent default action if applicable
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

	// Step 3: No matching rule — fall through to RiskLevel default behavior
	return false, &PermissionContext{Source: "risk_level"}, nil
}

// matchToolName checks if a tool name matches the given glob pattern.
func matchToolName(pattern, name string) bool {
	if pattern == name || pattern == "*" {
		return true
	}
	matched, _ := doublestar.Match(pattern, name)
	return matched
}

// matchAnyParamValue checks if the given glob pattern matches any string
// value in the params map.
func matchAnyParamValue(pattern string, params map[string]any) bool {
	for _, v := range params {
		str, ok := v.(string)
		if !ok {
			continue
		}
		matched, err := doublestar.Match(pattern, str)
		if err != nil {
			continue
		}
		if matched {
			return true
		}
	}
	return false
}

// extractCommandString extracts a human-readable command string from tool input
// for display in the permission prompt.
func extractCommandString(toolName string, input json.RawMessage) string {
	if len(input) == 0 {
		return ""
	}

	// Normalize tool name to canonical form (first char uppercase)
	if len(toolName) > 0 {
		toolName = strings.ToUpper(toolName[:1]) + toolName[1:]
	}

	// Try unmarshaling as ToolInput first (has Name and Params)
	var toolInput struct {
		Name   string         `json:"name"`
		Params map[string]any `json:"params"`
	}
	if err := json.Unmarshal(input, &toolInput); err == nil && len(toolInput.Params) > 0 {
		switch toolName {
		case "Bash":
			if cmd, ok := toolInput.Params["command"].(string); ok {
				return cmd
			}
		case "FileRead":
			if path, ok := toolInput.Params["path"].(string); ok {
				return fmt.Sprintf("read %s", path)
			}
		case "FileWrite":
			if path, ok := toolInput.Params["path"].(string); ok {
				return fmt.Sprintf("write %s", path)
			}
		case "Glob":
			if pattern, ok := toolInput.Params["pattern"].(string); ok {
				return fmt.Sprintf("glob %s", pattern)
			}
		case "Grep":
			if pattern, ok := toolInput.Params["pattern"].(string); ok {
				return fmt.Sprintf("grep %s", pattern)
			}
		case "Edit":
			if path, ok := toolInput.Params["path"].(string); ok {
				return fmt.Sprintf("edit %s", path)
			}
		case "WebFetch":
			if u, ok := toolInput.Params["url"].(string); ok {
				return u
			}
		case "TodoWrite":
			if todos, ok := toolInput.Params["todos"].([]any); ok {
				return fmt.Sprintf("todos: %d items", len(todos))
			}
		case "AskUserQuestion":
			if q, ok := toolInput.Params["question"].(string); ok {
				return q
			}
		}
	}

	// Fallback: try as raw map
	var params map[string]any
	if err := json.Unmarshal(input, &params); err == nil {
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
	}

	return string(input)
}

func (d *Dispatcher) RequestCh() chan PermissionRequest {
	return d.requestCh
}

func (d *Dispatcher) QuestionRequestCh() chan QuestionRequest {
	return d.questionReqCh
}

func (d *Dispatcher) QuestionResponseCh() chan QuestionResponse {
	return d.questionRespCh
}

func (d *Dispatcher) SetSessionID(id string) {
	if d.todoWrite != nil {
		d.todoWrite.SetSessionID(id)
	}
}

func DefaultDispatcher(workDir, backupDir, sessionsDir string) *Dispatcher {
	d := NewDispatcher(nil)
	d.Register(NewBash(workDir))
	d.Register(NewFileRead(workDir))
	d.Register(NewFileWrite(workDir, backupDir))
	d.Register(NewEdit(workDir, backupDir))
	todo := NewTodoWrite(sessionsDir, "")
	d.todoWrite = todo
	d.Register(todo)
	d.Register(NewWebFetch(sessionsDir))
	d.Register(NewAskUserQuestion(d.questionReqCh, d.questionRespCh))
	d.Register(NewGlob(workDir))
	d.Register(NewGrep(workDir))
	return d
}
