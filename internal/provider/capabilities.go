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

// nonChatModelPatterns match models that do not support chat completions at all
// (embedding, vision-only, safety classifiers, retrieval, utility endpoints).
var nonChatModelPatterns = []string{
	// Embedding models
	"bge-", "nv-embed", "embed-qa", "embedqa", "arctic-embed",
	"nv-embedcode", "llama-nemotron-embed", "llama-3.2-nemoretriever-1b-vlm-embed",
	// Vision-only / image-only models (no chat)
	"deplot", "kosmos-2", "nvclip", "neva-",
	// Safety / guard / content moderation classifiers
	"nemoguard", "content-safety", "safety-guard", "nemotron-safety",
	// Utility models (parsing, translation, retrieval, video, calibration)
	"nemoretriever-parse", "riva-translate", "ai-synthetic-video",
	"gliner-pii", "ising-calibration", "cosmos-reason",
}

// deprecatedNvidiaModels are known models that NVIDIA's /models endpoint still
// returns but that do not work with the standard chat completions API. This
// includes deprecated models, reward models, and other specialty endpoints.
// NOTE: nemotron-3-nano-omni and phi-4-multimodal were removed — they now
// support standard chat completions with the correct request parameters.
var deprecatedNvidiaModels = []string{
	"01-ai/yi-large",
	"phi-3-vision",
	"diffusiongemma",
	// Reward / scoring models (not chat-compatible)
	"nemotron-4-340b-reward",
	// Specialty models (parsing, vision-only, non-standard chat API)
	"nemotron-parse",
	"nvidia/vila",
	"adept/fuyu-8b",
	"google/recurrentgemma-2b",
	"bytedance/seed-oss-36b-instruct",
	"nvidia/llama3-chatqa-1.5-70b",
}

// brokenOnNvidiaNIM are models that appear in NVIDIA's /models endpoint but
// fail at runtime with 404, empty responses, or other errors. These are
// separate from deprecatedNvidiaModels because they ARE technically
// chat-capable but the hosted NIM endpoint does not serve them correctly.
// Reported on NVIDIA developer forums and confirmed via M31A testing.
var brokenOnNvidiaNIM = []string{
	"ibm/granite",
}

// IsNonChatModel reports whether a model ID belongs to a model that does not
// support chat completions (embeddings, vision-only, safety classifiers, etc.)
// or is a known deprecated model on NVIDIA NIM.
func IsNonChatModel(modelID string) bool {
	id := strings.ToLower(modelID)
	for _, p := range nonChatModelPatterns {
		if strings.Contains(id, p) {
			return true
		}
	}
	for _, p := range deprecatedNvidiaModels {
		if strings.Contains(id, p) {
			return true
		}
	}
	return false
}

// IsLikelyBrokenOnNvidia reports whether a model ID is known to fail at
// runtime on NVIDIA NIM despite appearing in the /models endpoint. These
// models return 404 or empty responses when used with /chat/completions.
func IsLikelyBrokenOnNvidia(modelID string) bool {
	id := strings.ToLower(modelID)
	for _, p := range brokenOnNvidiaNIM {
		if strings.Contains(id, p) {
			return true
		}
	}
	return false
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
