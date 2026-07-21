package workflow

import (
	"strings"
	"testing"

	"github.com/eshanized/M31A/internal/core/config"
)

func TestNewPromptBuilder(t *testing.T) {
	pb, err := NewPromptBuilder(config.PromptConfig{}, "")
	if err != nil {
		t.Fatalf("NewPromptBuilder failed: %v", err)
	}
	if pb == nil {
		t.Fatal("NewPromptBuilder returned nil")
	}
}

func TestGetPrompt_Base(t *testing.T) {
	pb, err := NewPromptBuilder(config.PromptConfig{}, "")
	if err != nil {
		t.Fatalf("NewPromptBuilder failed: %v", err)
	}
	s, err := pb.GetPrompt("base")
	if err != nil {
		t.Fatalf("GetPrompt(base) error: %v", err)
	}
	if s == "" {
		t.Error("GetPrompt(base) returned empty string")
	}
}

func TestGetPrompt_ToolUse(t *testing.T) {
	pb, err := NewPromptBuilder(config.PromptConfig{}, "")
	if err != nil {
		t.Fatalf("NewPromptBuilder failed: %v", err)
	}
	s, err := pb.GetPrompt("tool-use")
	if err != nil {
		t.Fatalf("GetPrompt(tool-use) error: %v", err)
	}
	if s == "" {
		t.Error("GetPrompt(tool-use) returned empty string")
	}
}

func TestGetPrompt_AllPrompts(t *testing.T) {
	pb, err := NewPromptBuilder(config.PromptConfig{}, "")
	if err != nil {
		t.Fatalf("NewPromptBuilder failed: %v", err)
	}
	names := []string{
		"base", "tool-use", "plan-format", "execute-task", "discuss",
		"self-heal", "demonstration", "autonomous", "context-awareness",
		"code-quality", "code-intelligence", "research", "plan-check",
		"plan-revise", "plan-outline", "discuss-followup", "intent-classify",
		"website-build",
	}
	for _, name := range names {
		s, err := pb.GetPrompt(name)
		if err != nil {
			t.Errorf("GetPrompt(%q) error: %v", name, err)
		}
		if s == "" {
			t.Errorf("GetPrompt(%q) returned empty string", name)
		}
	}
}

func TestGetPrompt_Nonexistent(t *testing.T) {
	pb, err := NewPromptBuilder(config.PromptConfig{}, "")
	if err != nil {
		t.Fatalf("NewPromptBuilder failed: %v", err)
	}
	_, err = pb.GetPrompt("nonexistent")
	if err == nil {
		t.Error("GetPrompt(nonexistent) should return error")
	}
}

func TestPrompt(t *testing.T) {
	pb, err := NewPromptBuilder(config.PromptConfig{}, "")
	if err != nil {
		t.Fatalf("NewPromptBuilder failed: %v", err)
	}
	s, err := pb.Prompt("base")
	if err != nil {
		t.Fatalf("Prompt(base) error: %v", err)
	}
	if s == "" {
		t.Error("Prompt(base) returned empty string")
	}
}

func TestPrompt_ReturnsErrorOnInvalidName(t *testing.T) {
	pb, err := NewPromptBuilder(config.PromptConfig{}, "")
	if err != nil {
		t.Fatalf("NewPromptBuilder failed: %v", err)
	}
	_, err = pb.Prompt("nonexistent")
	if err == nil {
		t.Error("Prompt(nonexistent) should return error")
	}
}

func TestBuildSystemPrompt(t *testing.T) {
	pb, err := NewPromptBuilder(config.PromptConfig{}, "")
	if err != nil {
		t.Fatalf("NewPromptBuilder failed: %v", err)
	}
	result := pb.BuildSystemPrompt("base prompt", "extra1", "extra2")
	if !strings.Contains(result, "base prompt") {
		t.Error("BuildSystemPrompt missing base")
	}
	if !strings.Contains(result, "extra1") {
		t.Error("BuildSystemPrompt missing extra1")
	}
	if !strings.Contains(result, "extra2") {
		t.Error("BuildSystemPrompt missing extra2")
	}
}

func TestBuildSystemPrompt_EmptyExtras(t *testing.T) {
	pb, err := NewPromptBuilder(config.PromptConfig{}, "")
	if err != nil {
		t.Fatalf("NewPromptBuilder failed: %v", err)
	}
	result := pb.BuildSystemPrompt("base prompt")
	if result != "base prompt" {
		t.Errorf("BuildSystemPrompt with no extras should return base only, got: %q", result)
	}
}

func TestBuildSystemPrompt_EmptyStringsIgnored(t *testing.T) {
	pb, err := NewPromptBuilder(config.PromptConfig{}, "")
	if err != nil {
		t.Fatalf("NewPromptBuilder failed: %v", err)
	}
	result := pb.BuildSystemPrompt("base", "", "extra")
	if !strings.Contains(result, "base") {
		t.Error("BuildSystemPrompt missing base")
	}
	if !strings.Contains(result, "extra") {
		t.Error("BuildSystemPrompt missing extra")
	}
	// Empty strings should not add separator
	if strings.Count(result, "---") > 1 {
		t.Error("BuildSystemPrompt should not add separator for empty strings")
	}
}

func TestPromptBuilder_NilGetPrompt(t *testing.T) {
	var pb *PromptBuilder
	_, err := pb.GetPrompt("base")
	if err == nil {
		t.Error("GetPrompt on nil PromptBuilder should return error")
	}
}
