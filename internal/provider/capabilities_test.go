package provider

import "testing"

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
		id       string
		extra    []string
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
