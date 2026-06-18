package provider

import (
	"strings"

	"github.com/eshanized/M31A/internal/types"
)

// defaultReasoningPatterns are the default ID patterns that indicate reasoning/thinking models.
var defaultReasoningPatterns = []string{"/o1", "/o3", "/o4"}

// toolCapablePatterns are model ID patterns known to support function calling / tool use.
var toolCapablePatterns = []string{
	"claude", "gpt", "gemini", "deepseek", "qwen",
	"llama", "mistral", "command-r", "command-a",
}

// completionOnlyPatterns are model ID patterns for models that only support the
// /completions endpoint, not /chat/completions. These are hidden from the chat UI.
var completionOnlyPatterns = []string{
	"codellama", "code-llama",
	"starcoder", "starcoder2",
}

// ParseModelCapabilities infers capability flags from the model ID using heuristics.
// extraReasoningPatterns are additional patterns to check for reasoning detection
// (e.g., Zen uses "-r1" which OpenRouter does not).
// Tools capability defaults to false and is only set true for known tool-capable model families.
func ParseModelCapabilities(modelID string, extraReasoningPatterns ...string) types.CapFlags {
	id := strings.ToLower(modelID)
	caps := types.CapFlags{}

	// Check tool capability against known tool-capable model families
	for _, p := range toolCapablePatterns {
		if strings.Contains(id, p) {
			caps.Tools = true
			break
		}
	}

	// Build combined reasoning patterns
	patterns := defaultReasoningPatterns
	if len(extraReasoningPatterns) > 0 {
		patterns = make([]string, 0, len(defaultReasoningPatterns)+len(extraReasoningPatterns))
		patterns = append(patterns, defaultReasoningPatterns...)
		patterns = append(patterns, extraReasoningPatterns...)
	}

	// Detect reasoning/thinking models by ID patterns
	for _, p := range patterns {
		if strings.Contains(id, p) {
			caps.Reasoning = true
			break
		}
	}
	if !caps.Reasoning {
		if strings.Contains(id, "reason") || strings.Contains(id, "thinking") {
			caps.Reasoning = true
		}
	}

	// Detect vision/multimodal models by ID patterns
	if strings.Contains(id, "vision") || strings.Contains(id, "multimodal") {
		caps.Vision = true
	}

	// Most models support chat; completion-only models (e.g., codellama) do not.
	caps.Chat = true
	for _, p := range completionOnlyPatterns {
		if strings.Contains(id, p) {
			caps.Chat = false
			break
		}
	}

	return caps
}
