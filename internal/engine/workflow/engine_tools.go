package workflow

// Tool definition building, system prompt composition, and code intelligence.

import (
	"context"
	"encoding/json"
	"time"

	m31types "github.com/eshanized/M31A/internal/core/types"
	"github.com/eshanized/M31A/internal/integrations/codeintel"
	"github.com/eshanized/M31A/internal/integrations/provider"
)

// buildToolDefinitions returns the tool definitions for the LLM.
// Results are cached after the first call since tool definitions
// don't change during a session (PERF-23). Each definition's
// ParametersParsed field is populated once to avoid repeated
// json.Unmarshal in BuildChatBody (PERF-25).
func (e *Engine) buildToolDefinitions() []provider.ToolDefinition {
	e.cacheMu.RLock()
	cache := e.cache
	e.cacheMu.RUnlock()
	return cache.GetToolDefs(func() []provider.ToolDefinition {
		var defs []provider.ToolDefinition
		for _, name := range e.dispatcher.List() {
			tool, ok := e.dispatcher.GetTool(name)
			if !ok {
				continue
			}
			def := provider.ToolDefinition{
				Name:        tool.Name(),
				Description: tool.Description(),
				Parameters:  "{}",
			}
			if sp, ok := tool.(m31types.SchemaProvider); ok {
				def.Parameters = sp.ParameterSchema()
			}
			// Pre-parse JSON to avoid repeated Unmarshal in BuildChatBody
			if def.Parameters != "" {
				var parsed any
				if err := json.Unmarshal([]byte(def.Parameters), &parsed); err == nil {
					def.ParametersParsed = parsed
				}
			}
			defs = append(defs, def)
		}
		return defs
	})
}

// buildSystemPrompt composes the system prompt from base + optional extras.
// Delegates to ContextBuilder for prompt composition logic.
func (e *Engine) buildSystemPrompt(extra ...string) string {
	return e.contextBuilder.BuildSystemPrompt(string(e.stateMachine.CurrentPhase()), extra...)
}

// getCodeIntel lazily builds the codebase intelligence indexer.
// Returns nil if building fails or the workDir is empty.
// The caller's context is used for the build, so cancellation propagates.
func (e *Engine) getCodeIntel(ctx context.Context) *codeintel.Indexer {
	e.codeIntelMu.Lock()
	defer e.codeIntelMu.Unlock()
	if e.codeIntelBuilt {
		return e.codeIntel
	}
	e.codeIntelBuilt = true
	idx := codeintel.NewIndexer(e.workDir)
	buildCtx, cancel := context.WithTimeout(ctx, 30*time.Second)
	defer cancel()
	if err := idx.Build(buildCtx); err != nil {
		e.logger.Warn("codeintel build failed", "error", err)
		return nil
	}
	e.codeIntel = idx
	return e.codeIntel
}
