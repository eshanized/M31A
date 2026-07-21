package workflow

import (
	"testing"

	"github.com/eshanized/M31A/internal/config"
	ctxsrc "github.com/eshanized/M31A/internal/integrations/context"
	"github.com/eshanized/M31A/internal/engine/tokens"
)

func newTestContextBuilder(t *testing.T) *ContextBuilder {
	t.Helper()
	pb, err := NewPromptBuilder(config.PromptConfig{}, "")
	if err != nil {
		t.Fatalf("NewPromptBuilder failed: %v", err)
	}
	state := &WorkflowState{}
	est := tokens.NewEstimator("test-model")
	cfg := &config.Config{}
	workDir := t.TempDir()
	registry := ctxsrc.NewRegistry(
		ctxsrc.DateTimeSource{},
		ctxsrc.EnvironmentSource{WorkDir: workDir},
	)
	return NewContextBuilder(pb, est, cfg, state, workDir, registry, func(phase string) string {
		return "test-model"
	})
}

func TestNewContextBuilder(t *testing.T) {
	cb := newTestContextBuilder(t)
	if cb == nil {
		t.Fatal("NewContextBuilder returned nil")
	}
}

func TestBuildSystemPrompt_ReturnsNonEmpty(t *testing.T) {
	cb := newTestContextBuilder(t)
	result := cb.BuildSystemPrompt("PhasePlan")
	if result == "" {
		t.Error("BuildSystemPrompt returned empty string")
	}
}

func TestBuildSystemPrompt_WithExtras(t *testing.T) {
	cb := newTestContextBuilder(t)
	result := cb.BuildSystemPrompt("PhasePlan", "extra1", "extra2")
	if result == "" {
		t.Error("BuildSystemPrompt with extras returned empty string")
	}
	// Result should contain the base prompt
	base, _ := cb.prompts.GetPrompt("base")
	if len(result) < len(base) {
		t.Error("BuildSystemPrompt with extras should be longer than base")
	}
}

func TestContextBuilder_BuildSystemPrompt_Caching(t *testing.T) {
	cb := newTestContextBuilder(t)
	result1 := cb.BuildSystemPrompt("PhasePlan", "extra1")
	result2 := cb.BuildSystemPrompt("PhasePlan", "extra1")
	if result1 != result2 {
		t.Error("BuildSystemPrompt should return cached result for same extras")
	}
}

func TestBuildSystemPrompt_DifferentExtras(t *testing.T) {
	cb := newTestContextBuilder(t)
	result1 := cb.BuildSystemPrompt("PhasePlan", "extra1")
	result2 := cb.BuildSystemPrompt("PhasePlan", "extra2")
	if result1 == result2 {
		t.Error("BuildSystemPrompt should return different results for different extras")
	}
}

func TestTokenCount(t *testing.T) {
	cb := newTestContextBuilder(t)
	count := cb.TokenCount("hello world")
	if count <= 0 {
		t.Errorf("TokenCount returned %d, want positive", count)
	}
}

func TestTokenCount_EmptyString(t *testing.T) {
	cb := newTestContextBuilder(t)
	count := cb.TokenCount("")
	if count < 0 {
		t.Errorf("TokenCount returned %d for empty string, want >= 0", count)
	}
}

func TestRenderDynamicContext_Empty(t *testing.T) {
	cb := newTestContextBuilder(t)
	result := cb.renderDynamicContext(nil)
	if result != "" {
		t.Errorf("renderDynamicContext(nil) = %q, want empty", result)
	}
}

func TestRenderDynamicContext_Values(t *testing.T) {
	cb := newTestContextBuilder(t)
	snapshot := map[string]string{
		"key1": "value1",
		"key2": "value2",
	}
	result := cb.renderDynamicContext(snapshot)
	if result == "" {
		t.Error("renderDynamicContext returned empty string")
	}
}
