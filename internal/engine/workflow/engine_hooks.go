package workflow

import (
	"context"
	"log/slog"
	"sync"
	"time"

	"github.com/eshanized/M31A/internal/core/types"
	"github.com/eshanized/M31A/pkg/extensions"
)

// PhaseHookRegistry manages pre/post phase hooks for workflow extensibility.
type PhaseHookRegistry struct {
	mu        sync.RWMutex
	preHooks  map[types.WorkflowPhase][]extensions.PhaseHookHandler
	postHooks map[types.WorkflowPhase][]extensions.PhaseHookHandler
}

// NewPhaseHookRegistry creates a new PhaseHookRegistry.
func NewPhaseHookRegistry() *PhaseHookRegistry {
	return &PhaseHookRegistry{
		preHooks:  make(map[types.WorkflowPhase][]extensions.PhaseHookHandler),
		postHooks: make(map[types.WorkflowPhase][]extensions.PhaseHookHandler),
	}
}

// Register registers a hook handler for a specific phase.
func (r *PhaseHookRegistry) Register(phase types.WorkflowPhase, hook extensions.PhaseHookHandler) {
	r.mu.Lock()
	defer r.mu.Unlock()
	r.preHooks[phase] = append(r.preHooks[phase], hook)
	r.postHooks[phase] = append(r.postHooks[phase], hook)
}

// GetPreHooks returns all pre-phase hooks for a given phase.
func (r *PhaseHookRegistry) GetPreHooks(phase types.WorkflowPhase) []extensions.PhaseHookHandler {
	r.mu.RLock()
	defer r.mu.RUnlock()
	hooks := make([]extensions.PhaseHookHandler, len(r.preHooks[phase]))
	copy(hooks, r.preHooks[phase])
	return hooks
}

// GetPostHooks returns all post-phase hooks for a given phase.
func (r *PhaseHookRegistry) GetPostHooks(phase types.WorkflowPhase) []extensions.PhaseHookHandler {
	r.mu.RLock()
	defer r.mu.RUnlock()
	hooks := make([]extensions.PhaseHookHandler, len(r.postHooks[phase]))
	copy(hooks, r.postHooks[phase])
	return hooks
}

// toExtensionsPhaseResult converts a workflow PhaseResult to extensions PhaseResult.
func toExtensionsPhaseResult(r *PhaseResult) *extensions.PhaseResult {
	if r == nil {
		return nil
	}
	return &extensions.PhaseResult{
		Phase:   r.Phase,
		Success: r.Success,
		Error:   r.Error,
		Output:  nil, // TODO: add output if needed
	}
}

// RunPreHooks executes all pre-phase hooks for a given phase.
func (r *PhaseHookRegistry) RunPreHooks(ctx context.Context, phase types.WorkflowPhase, payload extensions.PhaseHookPayload) error {
	hooks := r.GetPreHooks(phase)

	for _, hook := range hooks {
		// Each hook runs with timeout; failure logs but doesn't block
		hookCtx, cancel := context.WithTimeout(ctx, 30*time.Second)
		err := hook.PrePhase(hookCtx, payload)
		cancel()
		if err != nil {
			slog.Error("pre-phase hook failed", "phase", phase, "error", err)
			// Continue to next hook — hooks are best-effort
		}
	}
	return nil
}

// RunPostHooks executes all post-phase hooks for a given phase.
func (r *PhaseHookRegistry) RunPostHooks(ctx context.Context, phase types.WorkflowPhase, payload extensions.PhaseHookPayload, result *PhaseResult) error {
	hooks := r.GetPostHooks(phase)

	extResult := toExtensionsPhaseResult(result)

	for _, hook := range hooks {
		// Each hook runs with timeout; failure logs but doesn't block
		hookCtx, cancel := context.WithTimeout(ctx, 30*time.Second)
		err := hook.PostPhase(hookCtx, payload, extResult)
		cancel()
		if err != nil {
			slog.Error("post-phase hook failed", "phase", phase, "error", err)
			// Continue to next hook — hooks are best-effort
		}
	}
	return nil
}
