package workflow

import (
	"context"
	"sort"
	"strings"

	"github.com/eshanized/M31A/internal/config"
	ctxsrc "github.com/eshanized/M31A/internal/context"
	"github.com/eshanized/M31A/internal/tokens"
)

// ContextBuilder assembles system prompts and context for workflow phases.
// It owns the prompt composition logic that was previously embedded in Engine,
// including base prompt caching, model-specific template injection, AGENTS.md
// instructions, and dynamic context source reconciliation.
type ContextBuilder struct {
	prompts         *PromptBuilder
	tokens          *tokens.Estimator
	cfg             *config.Config
	state           *WorkflowState
	workDir         string
	contextRegistry *ctxsrc.Registry
	// modelForPhase returns the model ID for a given phase (delegated from Engine).
	modelForPhase func(phase string) string
}

// NewContextBuilder creates a ContextBuilder with the given dependencies.
// The modelForPhase function is a callback to the Engine's model resolution.
func NewContextBuilder(
	prompts *PromptBuilder,
	tokEst *tokens.Estimator,
	cfg *config.Config,
	state *WorkflowState,
	workDir string,
	contextRegistry *ctxsrc.Registry,
	modelForPhase func(phase string) string,
) *ContextBuilder {
	return &ContextBuilder{
		prompts:         prompts,
		tokens:          tokEst,
		cfg:             cfg,
		state:           state,
		workDir:         workDir,
		contextRegistry: contextRegistry,
		modelForPhase:   modelForPhase,
	}
}

// BuildSystemPrompt composes the system prompt from base + optional extras.
// The base prompt is cached since it doesn't change during a session (PERF-24).
// Full assembled prompts are cached per extras signature to avoid repeated string building.
func (cb *ContextBuilder) BuildSystemPrompt(activePhase string, extras ...string) string {
	cb.state.cachedBasePromptOnce.Do(func() {
		s, err := cb.prompts.Prompt("base")
		if err != nil {
			s = ""
		}
		cb.state.cachedBasePrompt = s
	})

	// Build cache key from extras
	key := strings.Join(extras, "|")

	cb.state.cachedFullPromptsMu.Lock()
	if cb.state.cachedFullPrompts == nil {
		cb.state.cachedFullPrompts = make(map[string]string)
	}
	if cached, ok := cb.state.cachedFullPrompts[key]; ok {
		cb.state.cachedFullPromptsMu.Unlock()
		return cached
	}
	cb.state.cachedFullPromptsMu.Unlock()

	parts := []string{cb.state.cachedBasePrompt}

	// Inject model-specific template
	if cb.modelForPhase != nil {
		var promptCfg config.PromptConfig
		if cb.cfg != nil {
			promptCfg = cb.cfg.Prompts
		}
		if modelTemplate := SelectTemplate(cb.modelForPhase(activePhase), promptCfg); modelTemplate != "" {
			parts = append(parts, modelTemplate)
		}
	}

	// Inject AGENTS.md instructions if enabled
	if cb.cfg != nil && cb.cfg.Instructions.Enabled {
		files := config.DiscoverInstructions(cb.workDir, cb.workDir)
		if rendered := config.RenderInstructions(files); rendered != "" {
			parts = append(parts, rendered)
		}
	}

	// Reconcile dynamic context sources and include current state
	if cb.contextRegistry != nil {
		ctx := context.Background()
		changes := cb.contextRegistry.Reconcile(ctx, cb.state.contextSnapshot)
		snapshot := cb.contextRegistry.LoadAll(ctx)

		if cb.state.contextSnapshot == nil || len(changes) > 0 {
			cb.state.contextSnapshot = snapshot
			cb.state.cachedDynamicContext = cb.renderDynamicContext(snapshot)
		}

		if cb.state.cachedDynamicContext != "" {
			parts = append(parts, cb.state.cachedDynamicContext)
		}
	}

	for _, p := range extras {
		if p != "" {
			parts = append(parts, p)
		}
	}
	result := strings.Join(parts, "\n\n---\n\n")

	cb.state.cachedFullPromptsMu.Lock()
	cb.state.cachedFullPrompts[key] = result
	cb.state.cachedFullPromptsMu.Unlock()

	return result
}

// renderDynamicContext formats a context snapshot map into a prompt section.
// Values are joined in key-sorted order for deterministic output.
func (cb *ContextBuilder) renderDynamicContext(snapshot map[string]string) string {
	if len(snapshot) == 0 {
		return ""
	}
	keys := make([]string, 0, len(snapshot))
	for k := range snapshot {
		keys = append(keys, k)
	}
	sort.Strings(keys)
	var sb strings.Builder
	for _, k := range keys {
		if v := snapshot[k]; v != "" {
			sb.WriteString(v)
			sb.WriteString("\n\n")
		}
	}
	return strings.TrimSpace(sb.String())
}

// TokenCount returns the estimated token count for the given text.
func (cb *ContextBuilder) TokenCount(text string) int {
	if cb.tokens == nil {
		return 0
	}
	return cb.tokens.Estimate(text)
}
