package tools

import (
	"context"
	"encoding/json"
	"fmt"
	"log/slog"
	"sort"
	"strings"
	"sync"
	"sync/atomic"
	"time"

	"github.com/eshanized/M31A/internal/core/config"
	m31errors "github.com/eshanized/M31A/internal/core/errors"
	"github.com/eshanized/M31A/internal/core/types"
	"github.com/eshanized/M31A/internal/integrations/metrics"
	"github.com/eshanized/M31A/internal/tools/exec"
	"github.com/eshanized/M31A/internal/tools/fileops"
	"github.com/eshanized/M31A/internal/tools/subagent"
	"github.com/eshanized/M31A/internal/tools/todo"
	"golang.org/x/time/rate"
)

type Dispatcher struct {
	mu                sync.RWMutex
	tools             map[string]types.Tool
	permissions       map[string]bool
	requestCh         chan PermissionRequest
	responseCh        chan PermissionResponse
	pendingResponses  sync.Map // map[int64]chan PermissionResponse — per-request routing
	todoWrite         *todo.TodoWrite
	todoRead          *todo.TodoRead
	questionReqCh     chan types.QuestionRequest
	questionRespCh    chan types.QuestionResponse
	pendingQuestions  sync.Map // map[int64]chan types.QuestionResponse — per-request routing
	rules             []config.PermissionRule
	originalRules     []config.PermissionRule
	agents            map[string]config.PermissionsAgentConfig
	activeAgent       string
	permissionTimeout int
	workDir_          string // working directory for cwd-aware permission caching
	// Batch approval: user can approve all pending + future calls for a
	// tool+risk combination. Approvals are task-scoped (expire on task
	// completion or phase transition).
	batchMu        sync.RWMutex
	batchApprovals map[string]BatchApproval // key: "toolName:riskLevel"
	// pendingPermCount tracks how many permission requests are waiting in the
	// queue for display in the permission modal ("3 tools behind this one").
	pendingPermCount atomic.Int64
	// Rate limiter: zero-goroutine token bucket via golang.org/x/time/rate (WP-S04).
	rateLimiter *rate.Limiter
	// Per-risk-level rate limiter for dangerous/destructive tools (M6).
	dangerousLimiter *rate.Limiter
	// Concurrency limiter: semaphore limiting concurrent tool executions (M5).
	concurrencySem chan struct{}
	// C-12: sync.Once prevents TOCTOU race in Stop().
	stopOnce sync.Once
	// outputStore bounds tool output to prevent context window exhaustion.
	outputStore *exec.OutputStore
	// collector captures tool execution metrics (call count, success/fail, duration).
	collector *metrics.Collector
	// persistentPerms handles saving permission rules to disk.
	persistentPerms *PersistentPermissions
	// permissionDecider defines how permission decisions are made.
	// Allows different behaviors for TUI, headless, CI, etc.
	permissionDecider PermissionDecider
}

// NewDispatcher creates a new Dispatcher with zero-goroutine rate limiting.
// The caller MUST call Stop() when the Dispatcher is no longer needed to prevent
// goroutine leaks from pending sends (e.g., during session restart or app shutdown).
func newDispatcher(cfg *config.PermissionsConfig, policy PermissionDecider) *Dispatcher {
	d := &Dispatcher{
		tools:             make(map[string]types.Tool),
		permissions:       make(map[string]bool),
		requestCh:         make(chan PermissionRequest, PermissionChannelBuffer),
		responseCh:        make(chan PermissionResponse, PermissionChannelBuffer),
		questionReqCh:     make(chan types.QuestionRequest, QuestionChannelBuffer),
		questionRespCh:    make(chan types.QuestionResponse, QuestionChannelBuffer),
		rules:             []config.PermissionRule{},
		originalRules:     []config.PermissionRule{},
		agents:            make(map[string]config.PermissionsAgentConfig),
		activeAgent:       DefaultAgentName,
		permissionTimeout: types.DefaultPermissionTimeout,
		batchApprovals:    make(map[string]BatchApproval),
		rateLimiter:       rate.NewLimiter(rate.Limit(ToolRateLimitPerSec), ToolRateLimitBurst),
		dangerousLimiter:  rate.NewLimiter(rate.Limit(DangerousRateLimitPerSec), DangerousRateLimitBurst),
		concurrencySem:    make(chan struct{}, MaxConcurrentTools),
		persistentPerms:   NewPersistentPermissions(),
		permissionDecider: policy,
	}
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
	// Default to headless deny policy if no policy provided
	if !d.HasPermissionDecider() {
		d.permissionDecider = NewHeadlessDenyDecider()
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

// SetCollector attaches a metrics collector for recording tool execution metrics.
// Also propagates the collector to tools that support it (e.g., Edit for strategy tracking).
func (d *Dispatcher) SetCollector(c *metrics.Collector) {
	d.mu.Lock()
	d.collector = c
	d.mu.Unlock()
	// Propagate collector to tools that support metric recording
	d.mu.RLock()
	defer d.mu.RUnlock()
	for _, tool := range d.tools {
		if edit, ok := tool.(*fileops.Edit); ok {
			edit.SetCollector(c)
		}
	}
}

// Unregister removes a tool by name. Used by profile-based filtering to
// strip tools that a subagent profile should not access.
func (d *Dispatcher) Unregister(name string) {
	d.mu.Lock()
	defer d.mu.Unlock()
	delete(d.tools, name)
}

// UnregisterTool is an alias for Unregister to satisfy ai.ToolDispatcher interface.
func (d *Dispatcher) UnregisterTool(name string) {
	d.Unregister(name)
}

// SetPermissionDecider updates the permission decider used by the dispatcher.
// This allows switching between different policies (e.g., headless deny, interactive, CI).
func (d *Dispatcher) SetPermissionDecider(policy PermissionDecider) {
	d.mu.Lock()
	defer d.mu.Unlock()
	d.permissionDecider = policy
	if d.permissionDecider == nil {
		d.permissionDecider = NewHeadlessDenyDecider()
	}
}

// HasPermissionDecider returns true if a permission decider is set.
func (d *Dispatcher) HasPermissionDecider() bool {
	return d.permissionDecider != nil
}

// SetPermission sets the permission for a tool (always allow/deny).
func (d *Dispatcher) SetPermission(name string, allowed bool) {
	d.mu.Lock()
	defer d.mu.Unlock()
	d.permissions[name] = allowed
}

func (d *Dispatcher) Execute(ctx context.Context, call types.ToolCall) (types.ToolResult, error) {
	start := time.Now()

	// Concurrency limiter (M5): limit concurrent tool executions.
	select {
	case d.concurrencySem <- struct{}{}:
		defer func() { <-d.concurrencySem }()
	case <-ctx.Done():
		return types.ToolResult{}, ctx.Err()
	}

	// Rate limit tool execution to prevent resource exhaustion from
	// malicious or buggy LLMs generating thousands of tool calls per second.
	if err := d.rateLimiter.Wait(ctx); err != nil {
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

	// Per-risk-level rate limiting (M6): dangerous/destructive tools get stricter limits.
	risk := tool.RiskLevel()
	if riskLevelValue(risk) >= riskLevelValue(types.RiskDangerous) {
		if err := d.dangerousLimiter.Wait(ctx); err != nil {
			return types.ToolResult{}, ctx.Err()
		}
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

	// Record tool execution metrics (B24 fix: protect collector read with lock)
	d.mu.RLock()
	collector := d.collector
	d.mu.RUnlock()
	if collector != nil {
		collector.RecordToolCall(call.Name, err == nil, elapsed)
	}

	slog.Debug("tool executed", "tool", call.Name, "duration_ms", elapsed, "error", err)

	output := result.Output
	truncated := result.Truncated
	if d.outputStore != nil && err == nil {
		bounded, _, wasBounded := d.outputStore.Bound(output)
		if wasBounded {
			output = bounded
			truncated = true
		}
	}

	res := types.ToolResult{
		ToolCallID: call.ID,
		Output:     wrapToolOutput(output),
		DurationMs: elapsed,
		Truncated:  truncated,
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

func (d *Dispatcher) ListTools() []subagent.ToolDescriptor {
	d.mu.RLock()
	defer d.mu.RUnlock()
	out := make([]subagent.ToolDescriptor, 0, len(d.tools))
	for _, t := range d.tools {
		desc := subagent.ToolDescriptor{
			Name:        t.Name(),
			Description: t.Description(),
		}
		if sp, ok := t.(types.SchemaProvider); ok {
			desc.ParameterSchema = sp.ParameterSchema()
		}
		out = append(out, desc)
	}
	return out
}

func (d *Dispatcher) RequestCh() chan PermissionRequest {
	return d.requestCh
}

func (d *Dispatcher) ResponseCh() chan PermissionResponse {
	return d.responseCh
}

func (d *Dispatcher) PermissionTimeout() int {
	return d.permissionTimeout
}

func (d *Dispatcher) QuestionRequestCh() chan types.QuestionRequest {
	return d.questionReqCh
}

func (d *Dispatcher) QuestionResponseCh() chan types.QuestionResponse {
	return d.questionRespCh
}

func (d *Dispatcher) SetSessionID(id string) {
	if d.todoWrite != nil {
		d.todoWrite.SetSessionID(id)
	}
	if d.todoRead != nil {
		d.todoRead.SetSessionID(id)
	}
}

// SetTodoWriteCallback sets the callback invoked after successful TodoWrite tool executions.
func (d *Dispatcher) SetTodoWriteCallback(fn func(items []todo.TodoItem)) {
	if d.todoWrite != nil {
		d.todoWrite.SetOnUpdate(fn)
	}
}

// SyncTodoFromTasks updates TODO.md from the task runner's current state.
// Called automatically after each execution group to keep the TODO in sync.
func (d *Dispatcher) SyncTodoFromTasks(tasks []types.Task) error {
	if d.todoWrite == nil {
		return nil
	}
	return d.todoWrite.SyncTodoFromTasks(tasks)
}

// SetOutputStore configures the output store for bounding tool output.
func (d *Dispatcher) SetOutputStore(store *exec.OutputStore) {
	d.mu.Lock()
	defer d.mu.Unlock()
	d.outputStore = store
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
		// rate.Limiter has no goroutines to stop — drain pending channels only.
		d.drainChannels()
	})
}

// DrainChannels is exported for testing. It non-blockingly drains requestCh,
// questionReqCh, and responseCh so that any goroutines blocked on sends
// can unblock and exit.
func (d *Dispatcher) DrainChannels() {
	d.drainChannels()
}

// drainChannels non-blockingly drains requestCh, questionReqCh, and responseCh
// so that any goroutines blocked on sends can unblock and exit.
func (d *Dispatcher) drainChannels() {
	for {
		select {
		case <-d.requestCh:
		case <-d.questionReqCh:
		case <-d.responseCh:
		default:
			return
		}
	}
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

	// Check batch approval before prompting (between rule and agent_default)
	if d.checkBatchApproval(call.Name, risk) {
		return nil
	}

	// If rules explicitly say "ask", use the permission policy to decide
	if pctx != nil && (pctx.Source == "rule" || pctx.Source == "agent_default") && pctx.RuleAction == "ask" {
		decision := d.permissionDecider.Decide(ctx, call.Name, risk, input)
		if !decision.Allowed {
			return toolResultError{result: types.ToolResult{Error: decision.ErrorMsg}}
		}
		return nil
	}

	// For dangerous tools without explicit rule, use permission policy
	if riskLevelValue(risk) >= riskLevelValue(types.RiskDangerous) {
		decision := d.permissionDecider.Decide(ctx, call.Name, risk, input)
		if !decision.Allowed {
			return toolResultError{result: types.ToolResult{Error: decision.ErrorMsg}}
		}
		return nil
	}

	return nil
}

// RespondQuestion routes a question response to the per-request channel
// for the given request ID. Falls back to the shared channel if no per-request
// channel exists. This prevents cross-caller response routing (H-2).
func (d *Dispatcher) RespondQuestion(requestID int64, answer string) {
	resp := types.QuestionResponse{Answer: answer}
	if rawCh, ok := d.pendingQuestions.Load(requestID); ok {
		if qCh, ok := rawCh.(chan types.QuestionResponse); ok {
			select {
			case qCh <- resp:
			default:
				slog.Warn("question response dropped: per-request channel full", "request_id", requestID)
			}
		}
		d.pendingQuestions.Delete(requestID)
		return
	}
	// Fallback to shared channel
	select {
	case d.questionRespCh <- resp:
	default:
		slog.Warn("question response dropped: shared channel full", "request_id", requestID)
	}
}

// wrapToolOutput wraps tool output in <tool_output> delimiters to help the LLM
// distinguish between tool results and its own reasoning.
func wrapToolOutput(output string) string {
	if output == "" {
		return output
	}
	return "<tool_output>\n" + output + "\n</tool_output>"
}
