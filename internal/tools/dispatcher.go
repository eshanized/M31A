package tools

import (
	"context"
	"encoding/json"
	"fmt"
	"log/slog"
	"sort"
	"strings"
	"sync"
	"time"

	"github.com/eshanized/M31A/internal/config"
	m31errors "github.com/eshanized/M31A/internal/errors"
	"github.com/eshanized/M31A/internal/types"
)

type Dispatcher struct {
	mu                sync.RWMutex
	tools             map[string]types.Tool
	permissions       map[string]bool
	requestCh         chan PermissionRequest
	responseCh        chan PermissionResponse
	pendingResponses  sync.Map // map[int64]chan PermissionResponse — per-request routing
	todoWrite         *TodoWrite
	questionReqCh     chan QuestionRequest
	questionRespCh    chan QuestionResponse
	pendingQuestions  sync.Map // map[int64]chan QuestionResponse — per-request routing
	rules             []config.PermissionRule
	originalRules     []config.PermissionRule
	agents            map[string]config.PermissionsAgentConfig
	activeAgent       string
	permissionTimeout int
	workDir_          string // working directory for cwd-aware permission caching
	// Rate limiter: token bucket for tool execution (WP-S04).
	rateTokens chan struct{}
	rateTicker *time.Ticker
	rateDone   chan struct{}
	// C-12: sync.Once prevents TOCTOU race in Stop().
	stopOnce sync.Once
}

// NewDispatcher creates a new Dispatcher with a background rate-limiter goroutine.
// The caller MUST call Stop() when the Dispatcher is no longer needed to prevent
// goroutine leaks (e.g., during session restart or app shutdown).
func NewDispatcher(cfg *config.PermissionsConfig) *Dispatcher {
	d := &Dispatcher{
		tools:             make(map[string]types.Tool),
		permissions:       make(map[string]bool),
		requestCh:         make(chan PermissionRequest, PermissionChannelBuffer),
		responseCh:        make(chan PermissionResponse, PermissionChannelBuffer),
		questionReqCh:     make(chan QuestionRequest, QuestionChannelBuffer),
		questionRespCh:    make(chan QuestionResponse, QuestionChannelBuffer),
		rules:             []config.PermissionRule{},
		originalRules:     []config.PermissionRule{},
		agents:            make(map[string]config.PermissionsAgentConfig),
		activeAgent:       DefaultAgentName,
		permissionTimeout: types.DefaultPermissionTimeout,
		rateTokens:        make(chan struct{}, ToolRateLimitBurst),
		rateDone:          make(chan struct{}),
	}
	// Initialize token bucket for rate limiting.
	for i := 0; i < ToolRateLimitBurst; i++ {
		d.rateTokens <- struct{}{}
	}
	d.rateTicker = time.NewTicker(time.Second / ToolRateLimitPerSec)
	go func() {
		for {
			select {
			case <-d.rateDone:
				return
			case <-d.rateTicker.C:
				select {
				case d.rateTokens <- struct{}{}:
				default:
				}
			}
		}
	}()
	if cfg != nil {
		if cfg.Rules != nil {
			d.rules = make([]config.PermissionRule, len(cfg.Rules))
			copy(d.rules, cfg.Rules)
			d.originalRules = make([]config.PermissionRule, len(cfg.Rules))
			copy(d.originalRules, cfg.Rules)
		}
		if cfg.Agents != nil {
			newAgents := make(map[string]config.PermissionsAgentConfig, len(cfg.Agents))
			for k, v := range cfg.Agents {
				newAgents[k] = v
			}
			d.agents = newAgents
		}
		if cfg.TimeoutSeconds > 0 {
			d.permissionTimeout = cfg.TimeoutSeconds
		}
	}
	return d
}

// UpdatePermissions hot-reloads the permission configuration.
func (d *Dispatcher) UpdatePermissions(cfg *config.PermissionsConfig) {
	d.mu.Lock()
	defer d.mu.Unlock()
	if cfg == nil {
		return
	}
	if cfg.Rules != nil {
		d.rules = make([]config.PermissionRule, len(cfg.Rules))
		copy(d.rules, cfg.Rules)
	}
	if cfg.Agents != nil {
		newAgents := make(map[string]config.PermissionsAgentConfig, len(cfg.Agents))
		for k, v := range cfg.Agents {
			newAgents[k] = v
		}
		d.agents = newAgents
	}
	if cfg.TimeoutSeconds > 0 {
		d.permissionTimeout = cfg.TimeoutSeconds
	}
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

	// Rate limit tool execution to prevent resource exhaustion from
	// malicious or buggy LLMs generating thousands of tool calls per second.
	select {
	case <-d.rateTokens:
	case <-ctx.Done():
		return types.ToolResult{}, ctx.Err()
	}

	// Look up the tool BEFORE parsing input — unknown tools should be
	// reported immediately regardless of whether Input is valid JSON.
	d.mu.RLock()
	tool, ok := d.tools[call.Name]
	d.mu.RUnlock()

	if !ok {
		available := d.List()
		return types.ToolResult{}, fmt.Errorf("%w: unknown tool: %s. Available tools: %s", m31errors.ErrToolExecution, call.Name, strings.Join(available, ", "))
	}

	// Tolerate nil or empty Input by treating it as an empty JSON object.
	inputBytes := call.Input
	if len(inputBytes) == 0 {
		inputBytes = []byte("{}")
	}

	var input types.ToolInput
	if err := json.Unmarshal(inputBytes, &input); err != nil {
		rawInput := string(inputBytes)
		if len(rawInput) > 200 {
			rawInput = rawInput[:200] + "…"
		}
		return types.ToolResult{}, fmt.Errorf("tool %s: invalid input JSON: %w. Raw input: %s", call.Name, err, rawInput)
	}
	// Normalize direct args vs nested params: if the model sent
	// {"path":"..."} (direct) instead of {"params":{"path":"..."}} (nested),
	// treat the whole input object as the params map.
	if len(input.Params) == 0 {
		var direct map[string]any
		if err := json.Unmarshal(inputBytes, &direct); err == nil {
			delete(direct, "name")
			delete(direct, "params")
			if len(direct) > 0 {
				input.Params = direct
			}
		}
	}
	input.Name = call.Name

	if len(input.Params) > 1000 {
		return types.ToolResult{}, fmt.Errorf("tool %s: too many parameters (%d > 1000)", call.Name, len(input.Params))
	}

	if err := d.ensurePermission(ctx, call, tool, input); err != nil {
		if errResult, ok := err.(toolResultError); ok {
			return errResult.result, nil
		}
		return types.ToolResult{}, err
	}

	result, err := tool.Execute(ctx, input)
	elapsed := time.Since(start).Milliseconds()

	slog.Debug("tool executed", "tool", call.Name, "duration_ms", elapsed, "error", err)

	res := types.ToolResult{
		ToolCallID: call.ID,
		Output:     result.Output,
		DurationMs: elapsed,
		Truncated:  result.Truncated,
	}
	if err != nil {
		res.Error = err.Error()
		return res, fmt.Errorf("tool %s: %w", call.Name, err)
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

// workDir returns the working directory for cwd-aware permission caching.
func (d *Dispatcher) workDir() string {
	return d.workDir_
}

// Stop shuts down the rate limiter goroutine and ticker.
// Should be called when the Dispatcher is no longer needed (e.g., during app shutdown).
// C-12: Uses sync.Once to prevent TOCTOU race on concurrent calls.
func (d *Dispatcher) Stop() {
	d.stopOnce.Do(func() {
		close(d.rateDone)
		d.rateTicker.Stop()
	})
}

// toolResultError wraps a ToolResult to distinguish "permission rule error
// returns a ToolResult with Error field" from "real Go error that should propagate".
type toolResultError struct {
	result types.ToolResult
}

func (e toolResultError) Error() string { return e.result.Error }

// ensurePermission evaluates permission rules and prompts the user if needed.
// Returns nil if the tool is allowed to proceed.
// Returns toolResultError for rule-level errors (logged in ToolResult.Error).
// Returns a regular error for permission denials or prompt failures.
func (d *Dispatcher) ensurePermission(ctx context.Context, call types.ToolCall, tool types.Tool, input types.ToolInput) error {
	d.mu.RLock()
	allowedByRule, pctx, ruleErr := d.checkPermission(call.Name, input)
	d.mu.RUnlock()

	if ruleErr != nil {
		return toolResultError{result: types.ToolResult{Error: ruleErr.Error()}}
	}

	if allowedByRule {
		return nil
	}

	risk := tool.RiskLevel()

	interactive := true
	if val, ok := input.Params["interactive"]; ok {
		if iv, isBool := val.(bool); isBool {
			interactive = iv
		}
	}

	if !interactive {
		if riskLevelValue(risk) >= riskLevelValue(types.RiskDangerous) {
			return fmt.Errorf("tool %s (risk: %s) blocked in shell mode: %w", call.Name, risk, m31errors.ErrPermissionDenied)
		}
		return nil
	}

	if pctx != nil && pctx.Source == "rule" && pctx.RuleAction == "ask" {
		return d.askPermission(ctx, call, risk, pctx)
	}
	if pctx != nil && pctx.Source == "agent_default" && pctx.RuleAction == "ask" {
		return d.askPermissionWithAgentDefault(ctx, call, risk)
	}
	if riskLevelValue(risk) >= riskLevelValue(types.RiskDangerous) {
		return d.askPermissionFallback(ctx, call, risk)
	}
	return nil
}

// RespondQuestion routes a question response to the per-request channel
// for the given request ID. Falls back to the shared channel if no per-request
// channel exists. This prevents cross-caller response routing (H-2).
func (d *Dispatcher) RespondQuestion(requestID int64, answer string) {
	resp := QuestionResponse{Answer: answer}
	if ch, ok := d.pendingQuestions.Load(requestID); ok {
		select {
		case ch.(chan QuestionResponse) <- resp:
		default:
			slog.Warn("question response dropped: per-request channel full", "request_id", requestID)
		}
		d.pendingQuestions.Delete(requestID)
		return
	}
	// Fallback to shared channel
	select {
	case d.questionRespCh <- resp:
	case <-time.After(30 * time.Second):
		slog.Warn("question response dropped: shared channel full")
	}
}
