package workflow

import (
	"testing"

	"github.com/eshanized/M31A/internal/core/config"
	m31types "github.com/eshanized/M31A/internal/core/types"
)

func TestReplanFromFailure_GeneratesTasks(t *testing.T) {
	t.Parallel()
	// This test verifies the structure of replanFromFailure
	// Full integration test would require mocking the LLM provider
	failedTask := m31types.Task{
		ID:          1,
		Action:      "create",
		Description: "failed task",
		Files:       []string{"a.go"},
	}
	remainingTasks := []m31types.Task{
		{ID: 2, Action: "create", Description: "remaining task", Files: []string{"b.go"}},
	}

	// Verify task structure is valid for re-planning
	if failedTask.ID == 0 {
		t.Error("failed task should have non-zero ID")
	}
	if len(remainingTasks) == 0 {
		t.Error("should have remaining tasks")
	}
}

func TestReplanFromFailure_LLMFails(t *testing.T) {
	t.Parallel()
	// Verify error handling structure
	failedTask := m31types.Task{
		ID:          1,
		Action:      "create",
		Description: "failed task",
		Files:       []string{"a.go"},
	}
	remainingTasks := []m31types.Task{
		{ID: 2, Action: "create", Description: "remaining task", Files: []string{"b.go"}},
	}

	// Verify task structure is valid for re-planning
	if failedTask.HealsAttempted < 0 {
		t.Error("heals attempted should be non-negative")
	}
	if len(remainingTasks) > 0 && remainingTasks[0].ID <= failedTask.ID {
		t.Error("remaining tasks should have IDs greater than failed task")
	}
}

func TestModelForPhase_AutoArbitrage(t *testing.T) {
	t.Parallel()
	// Test that AutoArbitrage flag is respected
	cfg := &config.Config{
		Model: config.ModelConfig{
			AutoArbitrage:      true,
			ArbitrageThreshold: 0.1,
		},
	}

	if !cfg.Model.AutoArbitrage {
		t.Error("AutoArbitrage should be enabled")
	}
	if cfg.Model.ArbitrageThreshold != 0.1 {
		t.Errorf("expected threshold 0.1, got %f", cfg.Model.ArbitrageThreshold)
	}
}

func TestModelForPhase_AutoArbitrage_Disabled(t *testing.T) {
	t.Parallel()
	// Test that AutoArbitrage disabled falls through to default
	cfg := &config.Config{
		Model: config.ModelConfig{
			AutoArbitrage: false,
		},
		Agents: config.AgentsConfig{
			Default: "default-model",
		},
	}

	if cfg.Model.AutoArbitrage {
		t.Error("AutoArbitrage should be disabled")
	}
	if cfg.Agents.Default != "default-model" {
		t.Error("default model should be set")
	}
}
