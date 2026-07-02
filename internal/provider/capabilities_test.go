package provider

import (
	"strings"
	"testing"
)

func TestParseModelCapabilities_ToolCapable(t *testing.T) {
	t.Parallel()

	toolCapable := []string{
		"anthropic/claude-3-opus",
		"openai/gpt-4o",
		"google/gemini-pro",
		"deepseek/deepseek-chat",
		"qwen/qwen-2.5",
		"meta-llama/llama-3",
		"mistralai/mistral-large",
		"cohere/command-r-plus",
		"cohere/command-a",
	}

	for _, id := range toolCapable {
		t.Run(id, func(t *testing.T) {
			caps := ParseModelCapabilities(id)
			if !caps.Tools {
				t.Errorf("expected Tools=true for %q", id)
			}
		})
	}
}

func TestParseModelCapabilities_NotToolCapable(t *testing.T) {
	t.Parallel()

	notCapable := []string{
		"custom/my-model",
		"local/llm",
		"unknown/provider",
	}

	for _, id := range notCapable {
		t.Run(id, func(t *testing.T) {
			caps := ParseModelCapabilities(id)
			if caps.Tools {
				t.Errorf("expected Tools=false for %q", id)
			}
		})
	}
}

func TestParseModelCapabilities_Reasoning(t *testing.T) {
	t.Parallel()

	tests := []struct {
		id        string
		extra     []string
		reasoning bool
	}{
		{"openai/o1-preview", nil, true},
		{"openai/o3-mini", nil, true},
		{"openai/o4-mini", nil, true},
		{"deepseek/deepseek-r1", []string{"-r1"}, true},
		{"deepseek/deepseek-chat", []string{"-r1"}, false},
		{"custom/reasoning-model", nil, true},
		{"custom/thinking-model", nil, true},
		{"custom/normal-model", nil, false},
	}

	for _, tt := range tests {
		t.Run(tt.id, func(t *testing.T) {
			caps := ParseModelCapabilities(tt.id, tt.extra...)
			if caps.Reasoning != tt.reasoning {
				t.Errorf("ParseModelCapabilities(%q, %v).Reasoning = %v, want %v", tt.id, tt.extra, caps.Reasoning, tt.reasoning)
			}
		})
	}
}

func TestParseModelCapabilities_Vision(t *testing.T) {
	t.Parallel()

	tests := []struct {
		id     string
		vision bool
	}{
		{"openai/gpt-4o-vision", true},
		{"google/gemini-multimodal", true},
		{"anthropic/claude-3-opus", false},
	}

	for _, tt := range tests {
		t.Run(tt.id, func(t *testing.T) {
			caps := ParseModelCapabilities(tt.id)
			if caps.Vision != tt.vision {
				t.Errorf("ParseModelCapabilities(%q).Vision = %v, want %v", tt.id, caps.Vision, tt.vision)
			}
		})
	}
}

func TestParseModelCapabilities_Combined(t *testing.T) {
	t.Parallel()

	// A model that is tool-capable, has reasoning, and vision
	caps := ParseModelCapabilities("openai/gpt-4o-vision")
	if !caps.Tools {
		t.Error("expected Tools=true")
	}
	if caps.Reasoning {
		t.Error("expected Reasoning=false for gpt-4o-vision")
	}
	if !caps.Vision {
		t.Error("expected Vision=true")
	}
}

func TestParseModelCapabilities_CaseInsensitive(t *testing.T) {
	t.Parallel()

	caps := ParseModelCapabilities("ANTHROPIC/CLAUDE-3-OPUS")
	if !caps.Tools {
		t.Error("expected Tools=true for uppercase model ID")
	}
}

func TestParseModelCapabilities_EmptyID(t *testing.T) {
	t.Parallel()

	caps := ParseModelCapabilities("")
	if caps.Tools || caps.Reasoning || caps.Vision {
		t.Error("expected all capabilities false for empty ID")
	}
}

func TestParseModelCapabilities_ChatDefault(t *testing.T) {
	t.Parallel()

	chatModels := []string{
		"anthropic/claude-3-opus",
		"openai/gpt-4o",
		"google/gemini-pro",
		"meta-llama/llama-3-70b",
		"deepseek/deepseek-chat",
	}

	for _, id := range chatModels {
		t.Run(id, func(t *testing.T) {
			caps := ParseModelCapabilities(id)
			if !caps.Chat {
				t.Errorf("expected Chat=true for %q", id)
			}
		})
	}
}

func TestParseModelCapabilities_ChatCompletionOnly(t *testing.T) {
	t.Parallel()

	completionOnly := []string{
		"meta/codellama-70b",
		"meta/code-llama-70b",
		"bigcode/starcoder2-15b",
		"bigcode/starcoder",
	}

	for _, id := range completionOnly {
		t.Run(id, func(t *testing.T) {
			caps := ParseModelCapabilities(id)
			if caps.Chat {
				t.Errorf("expected Chat=false for %q", id)
			}
		})
	}
}

func TestDetectCapabilities_KnownModels(t *testing.T) {
	t.Parallel()

	tests := []struct {
		provider string
		modelID  string
		wantCaps bool
	}{
		{"openai", "gpt-4o", true},
		{"anthropic", "claude-3-opus", true},
		{"google", "gemini-pro", true},
		{"meta", "llama-3-70b", true},
		{"mistral", "mistral-large", true},
		{"qwen", "qwen-2.5", true},
		{"deepseek", "deepseek-chat", true},
		{"cohere", "command-r-plus", true},
		{"custom", "unknown-model", false},
	}

	for _, tt := range tests {
		t.Run(tt.modelID, func(t *testing.T) {
			caps, err := DetectCapabilities(tt.provider, tt.modelID)
			if err != nil {
				t.Fatalf("unexpected error: %v", err)
			}
			if caps == nil {
				t.Fatal("expected non-nil capabilities")
			}

			// Verify known models have reasonable defaults
			if tt.wantCaps {
				if caps.MaxContextWindow <= 0 {
					t.Errorf("expected positive MaxContextWindow for %q", tt.modelID)
				}
				if caps.MaxOutputTokens <= 0 {
					t.Errorf("expected positive MaxOutputTokens for %q", tt.modelID)
				}
			}
		})
	}
}

func TestDetectCapabilities_Caching(t *testing.T) {
	t.Parallel()

	// First call
	caps1, err := DetectCapabilities("openai", "gpt-4o")
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	// Second call should return cached result
	caps2, err := DetectCapabilities("openai", "gpt-4o")
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	// Should be the same pointer (cached)
	if caps1 != caps2 {
		t.Error("expected same pointer from cache")
	}
}

func TestCheckModelHealth_EmptyModelID(t *testing.T) {
	t.Parallel()

	err := CheckModelHealth("openai", "")
	if err == nil {
		t.Error("expected error for empty model ID")
	}

	var healthErr *ModelHealthError
	if !strings.Contains(err.Error(), "empty model ID") {
		t.Errorf("expected 'empty model ID' in error, got: %v", err)
	}
	_ = healthErr
}

func TestCheckModelHealth_BrokenModel(t *testing.T) {
	t.Parallel()

	err := CheckModelHealth("nvidia", "ibm/granite")
	if err == nil {
		t.Error("expected error for broken model")
	}

	if !strings.Contains(err.Error(), "known broken on NVIDIA NIM") {
		t.Errorf("expected 'known broken on NVIDIA NIM' in error, got: %v", err)
	}
}

func TestCheckModelHealth_HealthyModel(t *testing.T) {
	t.Parallel()

	err := CheckModelHealth("openai", "gpt-4o")
	if err != nil {
		t.Errorf("expected no error for healthy model, got: %v", err)
	}
}

func TestModelCapabilities_Fields(t *testing.T) {
	t.Parallel()

	caps, err := DetectCapabilities("openai", "gpt-4o")
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	// Verify all fields are set
	if !caps.SupportsTools {
		t.Error("expected SupportsTools=true for gpt-4o")
	}
	if !caps.SupportsImages {
		t.Error("expected SupportsImages=true for gpt-4o")
	}
	if !caps.SupportsStreaming {
		t.Error("expected SupportsStreaming=true for gpt-4o")
	}
	if !caps.SupportsJSON {
		t.Error("expected SupportsJSON=true for gpt-4o")
	}
	if !caps.SupportsSystemPrompt {
		t.Error("expected SupportsSystemPrompt=true for gpt-4o")
	}
}
