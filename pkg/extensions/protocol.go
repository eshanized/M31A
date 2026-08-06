package extensions

import (
	"context"
	"encoding/json"
	"time"

	"github.com/eshanized/M31A/internal/core/types"
)

// JSON-RPC 2.0 Protocol Types

// JSONRPCRequest represents a JSON-RPC 2.0 request.
type JSONRPCRequest struct {
	JSONRPC string          `json:"jsonrpc"`
	ID      json.RawMessage `json:"id"`
	Method  string          `json:"method"`
	Params  json.RawMessage `json:"params,omitempty"`
}

// JSONRPCResponse represents a JSON-RPC 2.0 response.
type JSONRPCResponse struct {
	JSONRPC string          `json:"jsonrpc"`
	ID      json.RawMessage `json:"id"`
	Result  json.RawMessage `json:"result,omitempty"`
	Error   *JSONRPCError   `json:"error,omitempty"`
}

// JSONRPCError represents a JSON-RPC 2.0 error object.
type JSONRPCError struct {
	Code    int         `json:"code"`
	Message string      `json:"message"`
	Data    interface{} `json:"data,omitempty"`
}

// JSONRPCNotification represents a JSON-RPC 2.0 notification (no ID).
type JSONRPCNotification struct {
	JSONRPC string          `json:"jsonrpc"`
	Method  string          `json:"method"`
	Params  json.RawMessage `json:"params,omitempty"`
}

// Standard JSON-RPC error codes
const (
	// ParseError indicates invalid JSON was received.
	ParseError = -32700
	// InvalidRequest indicates the JSON sent is not a valid Request object.
	InvalidRequest = -32600
	// MethodNotFound indicates the method does not exist / is not available.
	MethodNotFound = -32601
	// InvalidParams indicates invalid method parameter(s).
	InvalidParams = -32602
	// InternalError indicates internal JSON-RPC error.
	InternalError = -32603
)

// Method constants for ExternalTool
const (
	// MethodToolName returns the tool's name.
	MethodToolName = "tool.name"
	// MethodToolDescription returns the tool's description.
	MethodToolDescription = "tool.description"
	// MethodToolRiskLevel returns the tool's risk level.
	MethodToolRiskLevel = "tool.risk_level"
	// MethodToolSchema returns the tool's parameter schema.
	MethodToolSchema = "tool.schema"
	// MethodToolExecute executes the tool with given input.
	MethodToolExecute = "tool.execute"
)

// MethodProviderName returns the provider's name.
const (
	MethodProviderName            = "provider.name"
	MethodProviderFetchModels     = "provider.fetch_models"
	MethodProviderChatCompletion  = "provider.chat_completion_stream"
	MethodProviderEstimateCost    = "provider.estimate_cost"
	MethodProviderHealthCheck     = "provider.health_check"
	MethodProviderGetModel        = "provider.get_model"
)

// Method constants for PhaseHookHandler
const (
	// MethodHookPrePhase is called before a phase begins.
	MethodHookPrePhase = "hook.pre_phase"
	// MethodHookPostPhase is called after a phase completes.
	MethodHookPostPhase = "hook.post_phase"
)

// Method constants for handshake and lifecycle
const (
	// MethodHandshake performs protocol version negotiation.
	MethodHandshake = "handshake"
	// MethodShutdown notifies the extension to shut down gracefully.
	MethodShutdown = "shutdown"
)

// HandshakeResult is the response to a handshake request.
type HandshakeResult struct {
	// ProtocolVersion is the negotiated protocol version (e.g., "1.0").
	ProtocolVersion string `json:"protocol_version"`
	// SupportedMethods lists all methods the extension implements.
	SupportedMethods []string `json:"supported_methods"`
}

// ToolExecuteParams are the parameters for tool.execute.
type ToolExecuteParams struct {
	// Input is the tool input parameters.
	Input types.ToolInput `json:"input"`
}

// ToolExecuteResult is the result of tool.execute.
type ToolExecuteResult struct {
	// Result is the tool execution result.
	Result types.ToolResult `json:"result"`
}

// ProviderChatCompletionParams are the parameters for provider.chat_completion_stream.
type ProviderChatCompletionParams struct {
	// Request is the chat completion request.
	Request types.ChatRequest `json:"request"`
}

// ProviderChatCompletionResult is the initial response for streaming chat completion.
// The actual stream chunks are sent as JSON-RPC notifications.
type ProviderChatCompletionResult struct {
	// StreamID identifies the stream for subsequent notifications.
	StreamID string `json:"stream_id"`
}

// ProviderFetchModelsResult is the result of provider.fetch_models.
type ProviderFetchModelsResult struct {
	// Models is the list of available models.
	Models []types.ModelInfo `json:"models"`
}

// ProviderEstimateCostResult is the result of provider.estimate_cost.
type ProviderEstimateCostResult struct {
	// CostUSD is the estimated cost in USD.
	CostUSD float64 `json:"cost_usd"`
}

// ProviderHealthCheckResult is the result of provider.health_check.
type ProviderHealthCheckResult struct {
	// Status is the health status string (e.g., "healthy", "degraded", "unhealthy").
	Status string `json:"status"`
	// LatencyMs is the health check latency in milliseconds.
	LatencyMs int64 `json:"latency_ms"`
	// Error is any error message if unhealthy.
	Error string `json:"error,omitempty"`
}

// ProviderGetModelResult is the result of provider.get_model.
type ProviderGetModelResult struct {
	// Model is the model info if found.
	Model *types.ModelInfo `json:"model,omitempty"`
	// Error is any error message if not found.
	Error string `json:"error,omitempty"`
}

// HookPrePhaseParams are the parameters for hook.pre_phase.
type HookPrePhaseParams struct {
	// Payload is the phase hook payload.
	Payload PhaseHookPayload `json:"payload"`
}

// HookPostPhaseParams are the parameters for hook.post_phase.
type HookPostPhaseParams struct {
	// Payload is the phase hook payload.
	Payload PhaseHookPayload `json:"payload"`
	// Result is the phase result.
	Result PhaseResult `json:"result"`
}

// ToolNameResult is the result of tool.name.
type ToolNameResult struct {
	Name string `json:"name"`
}

// ToolDescriptionResult is the result of tool.description.
type ToolDescriptionResult struct {
	Description string `json:"description"`
}

// ToolRiskLevelResult is the result of tool.risk_level.
type ToolRiskLevelResult struct {
	RiskLevel types.RiskLevel `json:"risk_level"`
}

// ToolSchemaResult is the result of tool.schema.
type ToolSchemaResult struct {
	Schema string `json:"schema"`
}

// ProviderNameResult is the result of provider.name.
type ProviderNameResult struct {
	Name string `json:"name"`
}

// SubprocessManagerInterface defines the methods required by adapters.
// This allows mocking in tests.
type SubprocessManagerInterface interface {
	Call(ctx context.Context, req JSONRPCRequest, timeout time.Duration) (JSONRPCResponse, error)
	nextRequestID() json.RawMessage
	Start(ctx context.Context) error
	Stop() error
	IsRunning() bool
	ProtocolVersion() string
	SupportedMethods() []string
	SetWorkDir(dir string)
}