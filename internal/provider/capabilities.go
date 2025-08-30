package provider

import (
	"strings"

	"github.com/eshanized/M31A/internal/types"
)

// defaultReasoningPatterns are the default ID patterns that indicate reasoning/thinking models.
var defaultReasoningPatterns = []string{"/o1", "/o3", "/o4"}

// ParseModelCapabilities infers capability flags from the model ID using heuristics.
// extraReasoningPatterns are additional patterns to check for reasoning detection
// (e.g., Zen uses "-r1" which OpenRouter does not).
// Tools capability defaults to true for all models since most modern LLMs support function calling.
func ParseModelCapabilities(modelID string, extraReasoningPatterns ...string) types.CapFlags {
	id := strings.ToLower(modelID)
	caps := types.CapFlags{
		Tools: true,
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

	return caps
}
