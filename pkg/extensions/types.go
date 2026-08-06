package extensions

import (
	"context"

	"github.com/eshanized/M31A/internal/core/types"
)

// ExternalTool is the interface that external tool extensions must implement.
// Tools are registered via config and invoked through the Dispatcher.
type ExternalTool interface {
	// Name returns the unique name of the tool.
	Name() string
	// Description returns a human-readable description of the tool.
	Description() string
	// RiskLevel returns the risk level of the tool for permission gating.
	RiskLevel() types.RiskLevel
	// ParameterSchema returns the JSON Schema for the tool's parameters.
	ParameterSchema() string
	// Execute runs the tool with the given input and returns the result.
	Execute(ctx context.Context, input types.ToolInput) (types.ToolResult, error)
}

// ExternalProvider is the interface that external LLM provider extensions must implement.
// Providers are registered via config and managed by the Provider Registry.
type ExternalProvider interface {
	// Name returns the unique name of the provider.
	Name() string
	// APIKey returns the API key (empty for external providers).
	APIKey() string
	// FetchModels returns the list of models available from this provider.
	FetchModels(ctx context.Context) ([]types.ModelInfo, error)
	// CachedModels returns the cached models without fetching.
	CachedModels() []types.ModelInfo
	// ChatCompletionStream performs a streaming chat completion request.
	ChatCompletionStream(ctx context.Context, req types.ChatRequest) (*types.StreamIterator, error)
	// EstimateCost estimates the cost of a request in USD.
	EstimateCost(modelID string, usage types.Usage) float64
	// HealthCheck verifies the provider is reachable and healthy.
	HealthCheck(ctx context.Context) types.HealthStatus
	// GetModel returns a specific model by ID.
	GetModel(id string) (*types.ModelInfo, error)
}

// PhaseHookHandler is the interface that workflow phase hook extensions must implement.
// Hooks are invoked at pre/post phase transitions via the MsgEmitter.
type PhaseHookHandler interface {
	// PrePhase is called before a workflow phase begins.
	PrePhase(ctx context.Context, payload PhaseHookPayload) error
	// PostPhase is called after a workflow phase completes.
	PostPhase(ctx context.Context, payload PhaseHookPayload, result *PhaseResult) error
}

// PhaseHookPayload carries context to phase hook handlers.
type PhaseHookPayload struct {
	// PhaseName is the name of the workflow phase.
	PhaseName types.WorkflowPhase `json:"phase_name"`
	// WorkflowState is a snapshot of the workflow state.
	WorkflowState WorkflowStateSnapshot `json:"workflow_state"`
	// Context provides cancellation and timeout for the hook.
	Context context.Context `json:"-"`
	// ExtensionConfig is optional extension-specific configuration.
	ExtensionConfig []byte `json:"extension_config,omitempty"`
}

// WorkflowStateSnapshot captures the workflow state for hooks.
type WorkflowStateSnapshot struct {
	// CurrentPhase is the current workflow phase.
	CurrentPhase types.WorkflowPhase `json:"current_phase"`
	// Goal is the user's high-level goal.
	Goal string `json:"goal"`
	// Tasks is the list of tasks in the current plan.
	Tasks []types.Task `json:"tasks"`
	// Messages is the conversation history (truncated for size).
	Messages []types.Message `json:"messages,omitempty"`
	// SessionID is the unique session identifier.
	SessionID string `json:"session_id"`
	// BudgetSpentUSD is the cumulative cost in USD.
	BudgetSpentUSD float64 `json:"budget_spent_usd"`
}

// PhaseResult represents the result of a workflow phase execution.
type PhaseResult struct {
	// Phase is the phase that completed.
	Phase types.WorkflowPhase `json:"phase"`
	// Success indicates if the phase completed successfully.
	Success bool `json:"success"`
	// Error is the error message if the phase failed.
	Error string `json:"error,omitempty"`
	// Output is any structured output from the phase.
	Output any `json:"output,omitempty"`
}
