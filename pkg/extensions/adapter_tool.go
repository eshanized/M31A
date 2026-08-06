package extensions

import (
	"context"
	"encoding/json"
	"fmt"
	"log/slog"
	"sync"
	"time"

	"github.com/eshanized/M31A/internal/core/types"
)

// ExternalToolAdapter wraps a SubprocessManager to implement ExternalTool.
type ExternalToolAdapter struct {
	name        string
	description string
	riskLevel   types.RiskLevel
	schema      string
	procManager *SubprocessManager
	initialized bool
	initMu      sync.Mutex
}

var (
	_ ExternalTool = (*ExternalToolAdapter)(nil)
)

// NewExternalToolAdapter creates a new ExternalToolAdapter.
func NewExternalToolAdapter(name string, proc *SubprocessManager) *ExternalToolAdapter {
	return &ExternalToolAdapter{
		name:        name,
		procManager: proc,
	}
}

// Name returns the tool name.
func (a *ExternalToolAdapter) Name() string {
	return a.name
}

// Description returns the tool description.
func (a *ExternalToolAdapter) Description() string {
	a.ensureInitialized()
	return a.description
}

// RiskLevel returns the tool risk level.
func (a *ExternalToolAdapter) RiskLevel() types.RiskLevel {
	a.ensureInitialized()
	return a.riskLevel
}

// ParameterSchema returns the tool parameter schema.
func (a *ExternalToolAdapter) ParameterSchema() string {
	a.ensureInitialized()
	return a.schema
}

// Execute runs the tool with the given input.
func (a *ExternalToolAdapter) Execute(ctx context.Context, input types.ToolInput) (types.ToolResult, error) {
	a.ensureInitialized()

	req := JSONRPCRequest{
		JSONRPC: "2.0",
		ID:      a.procManager.nextRequestID(),
		Method:  MethodToolExecute,
	}

	params := ToolExecuteParams{Input: input}
	paramsData, err := json.Marshal(params)
	if err != nil {
		return types.ToolResult{}, fmt.Errorf("marshal params: %w", err)
	}
	req.Params = paramsData

	// Use the configured timeout or default
	timeout := 30 * time.Second
	resp, err := a.procManager.Call(ctx, req, timeout)
	if err != nil {
		return types.ToolResult{}, err
	}

	if resp.Error != nil {
		return types.ToolResult{}, fmt.Errorf("tool execution error: %s", resp.Error.Message)
	}

	var result ToolExecuteResult
	if err := json.Unmarshal(resp.Result, &result); err != nil {
		return types.ToolResult{}, fmt.Errorf("unmarshal result: %w", err)
	}

	return result.Result, nil
}

// ensureInitialized fetches tool metadata on first access.
func (a *ExternalToolAdapter) ensureInitialized() {
	a.initMu.Lock()
	defer a.initMu.Unlock()

	if a.initialized {
		return
	}

	ctx, cancel := context.WithTimeout(context.Background(), 10*time.Second)
	defer cancel()

	// Fetch name
	if name, err := a.fetchName(ctx); err == nil {
		a.name = name
	}

	// Fetch description
	if desc, err := a.fetchDescription(ctx); err == nil {
		a.description = desc
	}

	// Fetch risk level
	if risk, err := a.fetchRiskLevel(ctx); err == nil {
		a.riskLevel = risk
	}

	// Fetch schema
	if schema, err := a.fetchSchema(ctx); err == nil {
		a.schema = schema
	}

	a.initialized = true
	slog.Debug("external tool adapter initialized", "name", a.name, "risk", a.riskLevel)
}

// fetchName calls tool.name method.
func (a *ExternalToolAdapter) fetchName(ctx context.Context) (string, error) {
	req := JSONRPCRequest{
		JSONRPC: "2.0",
		ID:      a.procManager.nextRequestID(),
		Method:  MethodToolName,
	}

	resp, err := a.procManager.Call(ctx, req, 5*time.Second)
	if err != nil {
		return "", err
	}

	if resp.Error != nil {
		return "", fmt.Errorf("tool.name error: %s", resp.Error.Message)
	}

	var result ToolNameResult
	if err := json.Unmarshal(resp.Result, &result); err != nil {
		return "", err
	}

	return result.Name, nil
}

// fetchDescription calls tool.description method.
func (a *ExternalToolAdapter) fetchDescription(ctx context.Context) (string, error) {
	req := JSONRPCRequest{
		JSONRPC: "2.0",
		ID:      a.procManager.nextRequestID(),
		Method:  MethodToolDescription,
	}

	resp, err := a.procManager.Call(ctx, req, 5*time.Second)
	if err != nil {
		return "", err
	}

	if resp.Error != nil {
		return "", fmt.Errorf("tool.description error: %s", resp.Error.Message)
	}

	var result ToolDescriptionResult
	if err := json.Unmarshal(resp.Result, &result); err != nil {
		return "", err
	}

	return result.Description, nil
}

// fetchRiskLevel calls tool.risk_level method.
func (a *ExternalToolAdapter) fetchRiskLevel(ctx context.Context) (types.RiskLevel, error) {
	req := JSONRPCRequest{
		JSONRPC: "2.0",
		ID:      a.procManager.nextRequestID(),
		Method:  MethodToolRiskLevel,
	}

	resp, err := a.procManager.Call(ctx, req, 5*time.Second)
	if err != nil {
		return types.RiskSafe, err
	}

	if resp.Error != nil {
		return types.RiskSafe, fmt.Errorf("tool.risk_level error: %s", resp.Error.Message)
	}

	var result ToolRiskLevelResult
	if err := json.Unmarshal(resp.Result, &result); err != nil {
		return types.RiskSafe, err
	}

	return result.RiskLevel, nil
}

// fetchSchema calls tool.schema method.
func (a *ExternalToolAdapter) fetchSchema(ctx context.Context) (string, error) {
	req := JSONRPCRequest{
		JSONRPC: "2.0",
		ID:      a.procManager.nextRequestID(),
		Method:  MethodToolSchema,
	}

	resp, err := a.procManager.Call(ctx, req, 5*time.Second)
	if err != nil {
		return "", err
	}

	if resp.Error != nil {
		return "", fmt.Errorf("tool.schema error: %s", resp.Error.Message)
	}

	var result ToolSchemaResult
	if err := json.Unmarshal(resp.Result, &result); err != nil {
		return "", err
	}

	return result.Schema, nil
}