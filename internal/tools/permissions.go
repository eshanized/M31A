package tools

import (
	"context"
	"encoding/json"
	"fmt"
	"log/slog"
	"strings"
	"time"

	"github.com/bmatcuk/doublestar/v4"
	"github.com/eshanized/M31A/internal/core/config"
	m31errors "github.com/eshanized/M31A/internal/core/errors"
	"github.com/eshanized/M31A/internal/core/types"
)

// isRuleExpired returns true if the permission rule has expired based on its TTL.
func isRuleExpired(rule config.PermissionRule, now time.Time) bool {
	if rule.TTL == "" {
		return false // No TTL = permanent
	}
	if rule.CreatedAt == "" {
		return false // No creation time = treat as permanent
	}
	created, err := time.Parse(time.RFC3339, rule.CreatedAt)
	if err != nil {
		return false // Invalid timestamp = treat as permanent
	}

	// Parse TTL duration
	ttl, err := time.ParseDuration(rule.TTL)
	if err != nil {
		return false // Invalid TTL = treat as permanent
	}

	return now.After(created.Add(ttl))
}

// BatchApproval records a user's "approve all" decision for a tool+risk
// combination. Approvals are task-scoped: they expire when the current task
// completes or the workflow phase transitions.
type BatchApproval struct {
	ToolName  string
	RiskLevel types.RiskLevel
	ExpiresAt time.Time // zero = never expires (manual SetPermission)
}

func batchKey(toolName string, risk types.RiskLevel) string {
	return toolName + ":" + string(risk)
}

func (d *Dispatcher) ApprovePermission(requestID int64, allowed bool, remember bool) {
	resp := PermissionResponse{RequestID: requestID, Allowed: allowed, Remember: remember}
	if ch, ok := d.pendingResponses.Load(requestID); ok {
		respCh, ok := ch.(chan PermissionResponse)
		if !ok {
			slog.Warn("permission response dropped: invalid channel type",
				"request_id", requestID, "allowed", allowed)
			return
		}
		select {
		case respCh <- resp:
		default:
			slog.Warn("permission response dropped: per-request channel full",
				"request_id", requestID, "allowed", allowed)
		}
		return
	}
	// Fallback to shared channel for backwards compatibility
	select {
	case d.responseCh <- resp:
	default:
		slog.Warn("permission response dropped: shared channel full",
			"request_id", requestID, "allowed", allowed)
	}
}

// ApproveBatch records a batch approval for the given tool+risk combination.
// When active, all future permission requests matching this tool+risk are
// auto-approved without prompting the user. Approvals are task-scoped:
// they expire when RevokeBatchApprovals() is called (typically on task
// completion or phase transition).
func (d *Dispatcher) ApproveBatch(toolName string, risk types.RiskLevel) {
	key := batchKey(toolName, risk)
	d.batchMu.Lock()
	d.batchApprovals[key] = BatchApproval{
		ToolName:  toolName,
		RiskLevel: risk,
	}
	d.batchMu.Unlock()
	slog.Info("batch approval activated",
		"tool", toolName, "risk", risk)
}

// RevokeBatchApprovals clears all active batch approvals. Called on task
// completion or phase transition to reset session-scoped approvals.
func (d *Dispatcher) RevokeBatchApprovals() {
	d.batchMu.Lock()
	n := len(d.batchApprovals)
	d.batchApprovals = make(map[string]BatchApproval)
	d.batchMu.Unlock()
	if n > 0 {
		slog.Info("batch approvals revoked", "count", n)
	}
}

// checkBatchApproval returns true if a batch approval is active for the
// given tool+risk combination and has not expired.
func (d *Dispatcher) checkBatchApproval(toolName string, risk types.RiskLevel) bool {
	key := batchKey(toolName, risk)
	d.batchMu.RLock()
	ap, ok := d.batchApprovals[key]
	d.batchMu.RUnlock()
	if !ok {
		return false
	}
	if !ap.ExpiresAt.IsZero() && time.Now().After(ap.ExpiresAt) {
		d.batchMu.Lock()
		delete(d.batchApprovals, key)
		d.batchMu.Unlock()
		return false
	}
	return true
}

// BatchApprovalCount returns the number of active batch approvals.
func (d *Dispatcher) BatchApprovalCount() int {
	d.batchMu.RLock()
	defer d.batchMu.RUnlock()
	return len(d.batchApprovals)
}

// ActiveBatchToolNames returns a comma-separated list of tool names with
// active batch approvals (e.g. "bash,edit"). Empty string if none.
func (d *Dispatcher) ActiveBatchToolNames() string {
	d.batchMu.RLock()
	defer d.batchMu.RUnlock()
	var names []string
	for _, ap := range d.batchApprovals {
		names = append(names, ap.ToolName)
	}
	return strings.Join(names, ",")
}

// PermissionEntry represents a permission rule with metadata for display.
type PermissionEntry struct {
	Tool      string
	Pattern   string
	Action    string
	CreatedAt string
	TTL       string
	Expired   bool
	Source    string // "config", "persistent", "session"
}

// ListPermissions returns all active permission rules with metadata.
func (d *Dispatcher) ListPermissions() []PermissionEntry {
	d.mu.RLock()
	rules := make([]config.PermissionRule, len(d.rules))
	copy(rules, d.rules)
	d.mu.RUnlock()

	now := time.Now()
	entries := make([]PermissionEntry, 0, len(rules))

	// Track which rules came from original config vs persistent
	originalSet := make(map[string]struct{})
	d.mu.RLock()
	for _, rule := range d.originalRules {
		key := ruleKey(rule)
		originalSet[key] = struct{}{}
	}
	d.mu.RUnlock()

	for _, rule := range rules {
		key := ruleKey(rule)
		source := "session"
		if _, ok := originalSet[key]; ok {
			source = "config"
		} else if rule.CreatedAt != "" {
			source = "persistent"
		}

		expired := isRuleExpired(rule, now)
		entries = append(entries, PermissionEntry{
			Tool:      rule.Tool,
			Pattern:   rule.Pattern,
			Action:    rule.Action,
			CreatedAt: rule.CreatedAt,
			TTL:       rule.TTL,
			Expired:   expired,
			Source:    source,
		})
	}
	return entries
}

// RevokePermission removes a permission rule matching the given tool and pattern.
func (d *Dispatcher) RevokePermission(tool, pattern string) error {
	d.mu.Lock()
	defer d.mu.Unlock()

	// Remove from in-memory rules
	newRules := make([]config.PermissionRule, 0, len(d.rules))
	found := false
	for _, rule := range d.rules {
		if rule.Tool == tool && rule.Pattern == pattern {
			found = true
			continue
		}
		newRules = append(newRules, rule)
	}
	d.rules = newRules

	// Also remove from originalRules if present
	newOriginal := make([]config.PermissionRule, 0, len(d.originalRules))
	for _, rule := range d.originalRules {
		if rule.Tool == tool && rule.Pattern == pattern {
			found = true
			continue
		}
		newOriginal = append(newOriginal, rule)
	}
	d.originalRules = newOriginal

	if !found {
		return fmt.Errorf("permission rule not found: tool=%s pattern=%s", tool, pattern)
	}

	// If persistent perms enabled, update disk
	if d.persistentPerms != nil && d.workDir_ != "" {
		persistentRules := d.persistentPerms.Load(d.workDir_)
		newPersistent := make([]config.PermissionRule, 0, len(persistentRules))
		for _, rule := range persistentRules {
			if rule.Tool == tool && rule.Pattern == pattern {
				continue
			}
			newPersistent = append(newPersistent, rule)
		}
		if err := d.persistentPerms.Save(d.workDir_, newPersistent); err != nil {
			slog.Error("failed to persist permission revocation", "error", err)
		}
	}

	return nil
}

func ruleKey(rule config.PermissionRule) string {
	return rule.Tool + "|" + rule.Resource + "|" + rule.Pattern + "|" + string(rule.RiskLevel) + "|" + rule.Action
}

// PendingPermCount returns the number of permission requests currently
// waiting in the queue (between send and TUI response).
func (d *Dispatcher) PendingPermCount() int {
	return int(d.pendingPermCount.Load())
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
	// Last-match-wins evaluation: iterate all rules, the last matching rule
	// determines the outcome. This allows more specific rules to override
	// general ones by ordering them later in the list.
	var lastMatch *struct {
		allowed bool
		pctx    *PermissionContext
		err     error
	}

	for _, rule := range d.rules {
		if rule.Tool != "" && !matchToolName(rule.Tool, toolName) {
			continue
		}

		if rule.Resource != "" {
			if !matchAnyParamValue(rule.Resource, input.Params) {
				continue
			}
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
			lastMatch = &struct {
				allowed bool
				pctx    *PermissionContext
				err     error
			}{true, pctx, nil}
		case "deny":
			lastMatch = &struct {
				allowed bool
				pctx    *PermissionContext
				err     error
			}{false, pctx, m31errors.ErrPermissionDenied}
		case "ask":
			lastMatch = &struct {
				allowed bool
				pctx    *PermissionContext
				err     error
			}{false, pctx, nil}
		default:
			lastMatch = &struct {
				allowed bool
				pctx    *PermissionContext
				err     error
			}{false, pctx, nil}
		}
	}

	if lastMatch != nil {
		return lastMatch.allowed, lastMatch.pctx, lastMatch.err
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
	req := d.buildPermissionRequest(call, risk)
	req.RuleTool = pctx.RuleTool
	req.RulePattern = pctx.RulePattern
	req.RuleAction = pctx.RuleAction
	return d.sendAndWaitForPermission(ctx, req, call.Name)
}

func (d *Dispatcher) askPermissionWithAgentDefault(ctx context.Context, call types.ToolCall, risk types.RiskLevel) error {
	req := d.buildPermissionRequest(call, risk)
	req.RuleAction = "ask"
	return d.sendAndWaitForPermission(ctx, req, call.Name)
}

func (d *Dispatcher) askPermissionFallback(ctx context.Context, call types.ToolCall, risk types.RiskLevel) error {
	cmd := extractCommandString(call.Name, call.Input)
	workDir := d.workDir()
	cacheKey := workDir + ":" + call.Name + ":" + cmd

	// Use RLock for read-only check first (optimistic path)
	d.mu.RLock()
	allowed, remembered := d.permissions[cacheKey]
	if !remembered {
		allowed, remembered = d.permissions[call.Name]
	}
	d.mu.RUnlock()

	if remembered && allowed {
		return nil
	}

	req := d.buildPermissionRequest(call, risk)
	return d.sendAndWaitForPermission(ctx, req, call.Name)
}

// buildPermissionRequest constructs a PermissionRequest with common fields.
func (d *Dispatcher) buildPermissionRequest(call types.ToolCall, risk types.RiskLevel) PermissionRequest {
	cmd := extractCommandString(call.Name, call.Input)
	timeoutSecs := d.permissionTimeout
	// For Bash commands, extract the actual shell timeout from tool params
	// so the permission modal can show how long the command will hold the TTY.
	if call.Name == "Bash" {
		if t := extractBashTimeout(call.Input); t > 0 {
			timeoutSecs = t
		}
	}
	return PermissionRequest{
		ID:          nextPermissionRequestID(),
		ToolName:    call.Name,
		Command:     cmd,
		RiskLevel:   risk,
		TimeoutSecs: timeoutSecs,
	}
}

// extractBashTimeout parses the "timeout" parameter from a Bash tool call input.
func extractBashTimeout(input json.RawMessage) int {
	if len(input) == 0 {
		return 0
	}
	var params struct {
		Timeout float64 `json:"timeout"`
	}
	if err := json.Unmarshal(input, &params); err == nil && params.Timeout > 0 {
		return int(params.Timeout)
	}
	return 0
}

// sendAndWaitForPermission sends a permission request to the TUI and blocks
// until a matching response arrives, the timeout expires, or the context is
// cancelled. Uses per-request channels to avoid deadlock when multiple tools
// request permission concurrently. If the user selects "remember", the
// decision is cached for the tool name. If the user selects "approve all",
// all current and future requests for this tool+risk are batch-approved.
func (d *Dispatcher) sendAndWaitForPermission(ctx context.Context, req PermissionRequest, toolName string) error {
	// Create per-request response channel
	respCh := make(chan PermissionResponse, 1)
	d.pendingResponses.Store(req.ID, respCh)
	defer d.pendingResponses.Delete(req.ID)

	// Set queue depth before sending (how many are already waiting)
	req.QueueDepth = int(d.pendingPermCount.Load())
	d.pendingPermCount.Add(1)

	select {
	case d.requestCh <- req:
		defer d.pendingPermCount.Add(-1)
	default:
		d.pendingPermCount.Add(-1)
		return m31errors.ErrPermissionDenied
	}

	timeoutCtx, cancel := context.WithTimeout(ctx, time.Duration(req.TimeoutSecs)*time.Second)
	defer cancel()

	var resp PermissionResponse
	select {
	case resp = <-respCh:
	case <-timeoutCtx.Done():
		return m31errors.ErrPermissionDenied
	case <-ctx.Done():
		return ctx.Err()
	}

	if !resp.Allowed {
		return m31errors.ErrPermissionDenied
	}

	// Handle "approve all" — batch approve for this tool+risk
	if resp.ApproveAll {
		d.ApproveBatch(toolName, req.RiskLevel)
	}

	if resp.Remember {
		workDir := d.workDir()
		cacheKey := workDir + ":" + toolName + ":" + req.Command
		d.mu.Lock()
		d.permissions[cacheKey] = resp.Allowed
		d.mu.Unlock()

		// Persist to disk with TTL (default 24h)
		if d.persistentPerms != nil {
			rule := config.PermissionRule{
				Tool:      toolName,
				Pattern:   req.Command,
				Action:    "allow",
				TTL:       "24h", // default 24-hour TTL
				CreatedAt: time.Now().Format(time.RFC3339),
			}
			existingRules := d.persistentPerms.Load(workDir)
			existingRules = append(existingRules, rule)
			if err := d.persistentPerms.Save(workDir, existingRules); err != nil {
				slog.Error("failed to persist permission rule", "error", err)
			}
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
	case "TodoRead":
		if status, ok := params["status"].(string); ok && status != "" {
			return fmt.Sprintf("read todos (%s)", status)
		}
		return "read todos"
	case "WebSearch":
		if query, ok := params["query"].(string); ok {
			return fmt.Sprintf("search %s", query)
		}
	case "FileList":
		if path, ok := params["path"].(string); ok && path != "" {
			return fmt.Sprintf("list %s", path)
		}
		return "list files"
	case "FileDelete":
		if path, ok := params["path"].(string); ok {
			return fmt.Sprintf("delete %s", path)
		}
	case "FileMove":
		src, _ := params["source"].(string)
		dst, _ := params["destination"].(string)
		if src != "" && dst != "" {
			return fmt.Sprintf("move %s → %s", src, dst)
		}
		if src != "" {
			return fmt.Sprintf("move %s", src)
		}
	case "CodeMap":
		if query, ok := params["query"].(string); ok {
			return fmt.Sprintf("codemap %s", query)
		}
	case "CodeComplexity":
		return "analyze complexity"
	case "DevServer":
		if action, ok := params["action"].(string); ok {
			return fmt.Sprintf("devserver %s", action)
		}
	case "HTTPCheck":
		if u, ok := params["url"].(string); ok {
			return fmt.Sprintf("http check %s", u)
		}
	case "Agent":
		if desc, ok := params["description"].(string); ok && desc != "" {
			return fmt.Sprintf("agent: %s", desc)
		}
		return "spawn agent"
	case "MetricsTool":
		if mode, ok := params["mode"].(string); ok && mode != "" {
			return fmt.Sprintf("metrics (%s)", mode)
		}
		return "show metrics"
	}
	return ""
}
