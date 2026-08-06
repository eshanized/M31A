package workflow

// Model selection and configuration for per-phase model routing.

import (
	"context"

	m31types "github.com/eshanized/M31A/internal/core/types"
	"github.com/eshanized/M31A/internal/integrations/arbitrage"
	"github.com/eshanized/M31A/internal/integrations/provider"
)

// modelForPhase returns the per-phase model ID, checked in priority order:
//  1. perPhaseModels (set interactively by the TUI at workflow start)
//  2. AgentsConfig from config.toml
//  3. cfg.Model.Default
//  4. the engine's active modelID
func (e *Engine) modelForPhase(phase m31types.WorkflowPhase) string {
	// 1. Interactive per-phase override (highest priority)
	e.perPhaseModelsMu.RLock()
	if id, ok := e.perPhaseModels[phase]; ok && id != "" {
		e.perPhaseModelsMu.RUnlock()
		e.logger.Debug("model selected", "phase", phase, "model", id, "source", "per-phase override")
		return id
	}
	e.perPhaseModelsMu.RUnlock()
	if e.cfg == nil {
		return e.modelID
	}
	// 2. AgentsConfig from config.toml
	var override string
	switch phase {
	case m31types.PhaseInitialize:
		override = e.cfg.Agents.Initialize
	case m31types.PhasePlan:
		override = e.cfg.Agents.Plan
	case m31types.PhaseExecute:
		override = e.cfg.Agents.Execute
	case m31types.PhaseVerify:
		override = e.cfg.Agents.Verify
	case m31types.PhaseRuntime:
		override = e.cfg.Agents.Runtime
	case m31types.PhaseShip:
		override = e.cfg.Agents.Ship
	case m31types.PhaseDiscuss:
		override = e.cfg.Agents.Discuss
	}
	if override != "" {
		e.logger.Debug("model selected", "phase", phase, "model", override, "source", "agents config")
		return override
	}
	// 3. AutoArbitrage layer (when enabled, for PhaseExecute only)
	if e.cfg.Model.AutoArbitrage && phase == m31types.PhaseExecute && e.provider != nil {
		if rec, err := e.arbitrageRecommend(context.Background()); err == nil && rec != nil {
			e.logger.Debug("model selected via arbitrage",
				"phase", phase,
				"model", rec.RecommendedModel.ModelID,
				"complexity", rec.Complexity,
				"savings", rec.Savings,
				"reason", rec.Reason)
			return rec.RecommendedModel.ModelID
		}
	}
	// 4. Global agent default
	if e.cfg.Agents.Default != "" {
		e.logger.Debug("model selected", "phase", phase, "model", e.cfg.Agents.Default, "source", "default")
		return e.cfg.Agents.Default
	}
	// 5. Engine model ID
	return e.modelID
}

// SetPhaseModel assigns a model ID to a specific workflow phase.
// This takes the highest priority over AgentsConfig and cfg.Model.Default.
// Called by the TUI after the user selects Planning/Coding models in the picker.
func (e *Engine) SetPhaseModel(phase m31types.WorkflowPhase, modelID string) {
	e.perPhaseModelsMu.Lock()
	defer e.perPhaseModelsMu.Unlock()
	if e.perPhaseModels == nil {
		e.perPhaseModels = make(map[m31types.WorkflowPhase]string)
	}
	if modelID != "" {
		e.perPhaseModels[phase] = modelID
	}
}

// SetWorkflowMode sets the mode that controls phase-skipping behaviour.
func (e *Engine) SetWorkflowMode(mode m31types.WorkflowMode) {
	e.workflowModeMu.Lock()
	e.workflowMode = mode
	e.workflowModeMu.Unlock()
}

// WorkflowMode returns the current workflow mode.
func (e *Engine) WorkflowMode() m31types.WorkflowMode {
	e.workflowModeMu.RLock()
	defer e.workflowModeMu.RUnlock()
	return e.workflowMode
}

// SetModel updates the active model ID and provider for the engine.
func (e *Engine) SetModel(modelID string, p provider.LLMProvider) {
	e.modelIDMu.Lock()
	defer e.modelIDMu.Unlock()
	e.modelID = modelID
	if p != nil {
		e.provider = p
	}
}

// providerAndModel returns the current provider and modelID atomically.
func (e *Engine) providerAndModel() (provider.LLMProvider, string) {
	e.modelIDMu.RLock()
	defer e.modelIDMu.RUnlock()
	return e.provider, e.modelID
}

// arbitrageRecommend fetches available models and calls arbitrage.Recommend
// to find the best model for the current task context.
func (e *Engine) arbitrageRecommend(ctx context.Context) (*arbitrage.ArbitrageRecommendation, error) {
	if e.provider == nil {
		return nil, nil
	}

	// Fetch available models from the provider
	models, err := e.provider.FetchModels(ctx)
	if err != nil {
		e.logger.Debug("failed to fetch models for arbitrage", "error", err)
		return nil, err
	}

	if len(models) == 0 {
		return nil, nil
	}

	// Create a synthetic task for the current phase context
	// This is a placeholder — in a real implementation, we'd pass the actual task
	task := m31types.Task{
		Action:      "execute",
		Description: "current execution task",
		Files:       []string{},
	}

	// Get threshold from config (default 0.1 = 10%)
	threshold := e.cfg.Model.ArbitrageThreshold
	if threshold <= 0 {
		threshold = 0.1
	}

	return arbitrage.Recommend(models, task, threshold)
}
