package tools

import (
	"context"
	"encoding/json"
	"fmt"
	"sort"
	"strings"
	"sync"
	"time"

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
	rules          []config.PermissionRule
	originalRules  []config.PermissionRule
	agents         map[string]config.PermissionsAgentConfig
	activeAgent    string
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

func (d *Dispatcher) Register(tool types.Tool) error {
	d.mu.Lock()
	defer d.mu.Unlock()
	name := tool.Name()
	if _, ok := d.tools[name]; ok {
		return fmt.Errorf("tool already registered: %s", name)
	}
	d.tools[name] = tool
	return nil
}

func (d *Dispatcher) Execute(ctx context.Context, call types.ToolCall) (types.ToolResult, error) {
	start := time.Now()

	d.mu.RLock()
	tool, ok := d.tools[call.Name]
	d.mu.RUnlock()

	if !ok {
		available := d.List()
		return types.ToolResult{}, fmt.Errorf("%w: unknown tool: %s. Available tools: %s", m31errors.ErrToolExecution, call.Name, strings.Join(available, ", "))
	}

	var input types.ToolInput
	if err := json.Unmarshal(call.Input, &input); err != nil {
		rawInput := string(call.Input)
		if len(rawInput) > 200 {
			rawInput = rawInput[:200] + "…"
		}
		return types.ToolResult{}, fmt.Errorf("tool %s: invalid input JSON: %s. Raw input: %s", call.Name, err, rawInput)
	}
	input.Name = call.Name

	d.mu.RLock()
	allowedByRule, pctx, ruleErr := d.checkPermission(call.Name, input)
	d.mu.RUnlock()

	if ruleErr != nil {
		return types.ToolResult{Error: ruleErr.Error()}, nil
	}

	risk := tool.RiskLevel()

	if !allowedByRule {
		interactive := true
		if val, ok := input.Params["interactive"]; ok {
			if iv, isBool := val.(bool); isBool {
				interactive = iv
			}
		}

		if !interactive {
			if risk == types.RiskDangerous || risk == types.RiskDestructive {
				return types.ToolResult{}, fmt.Errorf("tool %s (risk: %s) blocked in shell mode: %w", call.Name, risk, m31errors.ErrPermissionDenied)
			}
		} else if pctx != nil && pctx.Source == "rule" && pctx.RuleAction == "ask" {
			if err := d.askPermission(ctx, call, risk, pctx); err != nil {
				return types.ToolResult{}, err
			}
		} else if pctx != nil && pctx.Source == "agent_default" && pctx.RuleAction == "ask" {
			if err := d.askPermissionWithAgentDefault(ctx, call, risk); err != nil {
				return types.ToolResult{}, err
			}
		} else {
			if risk == types.RiskDangerous || risk == types.RiskDestructive {
				if err := d.askPermissionFallback(ctx, call, risk); err != nil {
					return types.ToolResult{}, err
				}
			}
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
