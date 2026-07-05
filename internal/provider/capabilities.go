package provider

import (
	"strings"
	"sync"

	"github.com/eshanized/M31A/internal/types"
)

// capabilityConfig holds the user-configured model capability overrides.
// Set once at startup via SetCapabilityConfig; all capability detection
// functions read from this package-level variable.
var (
	capabilityConfigMu sync.RWMutex
	capabilityConfig   capabilityConfigData
)

type capabilityConfigData struct {
	extraReasoningPatterns     []string
	extraToolCapablePatterns   []string
	extraCompletionOnlyPatterns []string
	extraNonChatPatterns       []string
	knownCapabilities          map[string]ModelCapabilities
}

// SetCapabilityConfig initializes the package-level capability configuration.
// Must be called once at startup before any capability detection functions
// are invoked. Pass nil or empty config to keep built-in defaults.
func SetCapabilityConfig(extraReasoning, extraToolCapable, extraCompletionOnly, extraNonChat []string, knownCaps map[string]ModelCapabilities) {
	capabilityConfigMu.Lock()
	defer capabilityConfigMu.Unlock()
	capabilityConfig = capabilityConfigData{
		extraReasoningPatterns:     extraReasoning,
		extraToolCapablePatterns:   extraToolCapable,
		extraCompletionOnlyPatterns: extraCompletionOnly,
		extraNonChatPatterns:       extraNonChat,
		knownCapabilities:          knownCaps,
	}
	// Clear the capabilities cache so new config takes effect immediately
	modelCapabilitiesCache = sync.Map{}
}

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

// ModelCapabilities describes what a model supports.
type ModelCapabilities struct {
	SupportsTools        bool
	SupportsImages       bool
	SupportsStreaming    bool
	SupportsJSON         bool
	SupportsSystemPrompt bool
	MaxContextWindow     int
	MaxOutputTokens      int
}

// modelCapabilitiesCache stores detected capabilities to avoid repeated lookups.
var modelCapabilitiesCache sync.Map

// knownModelCapabilities is a hardcoded fallback table for well-known models.
// This is used when runtime detection fails or is not available.
var knownModelCapabilities = map[string]ModelCapabilities{
	// OpenAI models
	"gpt-4o": {
		SupportsTools: true, SupportsImages: true, SupportsStreaming: true,
		SupportsJSON: true, SupportsSystemPrompt: true,
		MaxContextWindow: 128000, MaxOutputTokens: 16384,
	},
	"gpt-4-turbo": {
		SupportsTools: true, SupportsImages: true, SupportsStreaming: true,
		SupportsJSON: true, SupportsSystemPrompt: true,
		MaxContextWindow: 128000, MaxOutputTokens: 4096,
	},
	"o1-preview": {
		SupportsTools: false, SupportsImages: true, SupportsStreaming: false,
		SupportsJSON: false, SupportsSystemPrompt: false,
		MaxContextWindow: 128000, MaxOutputTokens: 32768,
	},
	// Anthropic models
	"claude-3-opus": {
		SupportsTools: true, SupportsImages: true, SupportsStreaming: true,
		SupportsJSON: true, SupportsSystemPrompt: true,
		MaxContextWindow: 200000, MaxOutputTokens: 4096,
	},
	"claude-3-5-sonnet": {
		SupportsTools: true, SupportsImages: true, SupportsStreaming: true,
		SupportsJSON: true, SupportsSystemPrompt: true,
		MaxContextWindow: 200000, MaxOutputTokens: 8192,
	},
	// Google models
	"gemini-pro": {
		SupportsTools: true, SupportsImages: true, SupportsStreaming: true,
		SupportsJSON: true, SupportsSystemPrompt: true,
		MaxContextWindow: 32760, MaxOutputTokens: 8192,
	},
	// Meta models
	"llama-3-70b": {
		SupportsTools: true, SupportsImages: false, SupportsStreaming: true,
		SupportsJSON: true, SupportsSystemPrompt: true,
		MaxContextWindow: 8192, MaxOutputTokens: 2048,
	},
	// Mistral models
	"mistral-large": {
		SupportsTools: true, SupportsImages: false, SupportsStreaming: true,
		SupportsJSON: true, SupportsSystemPrompt: true,
		MaxContextWindow: 32768, MaxOutputTokens: 4096,
	},
	// Qwen models
	"qwen-2.5": {
		SupportsTools: true, SupportsImages: false, SupportsStreaming: true,
		SupportsJSON: true, SupportsSystemPrompt: true,
		MaxContextWindow: 32768, MaxOutputTokens: 8192,
	},
	// DeepSeek models
	"deepseek-chat": {
		SupportsTools: true, SupportsImages: false, SupportsStreaming: true,
		SupportsJSON: true, SupportsSystemPrompt: true,
		MaxContextWindow: 32768, MaxOutputTokens: 4096,
	},
	// Cohere models
	"command-r-plus": {
		SupportsTools: true, SupportsImages: false, SupportsStreaming: true,
		SupportsJSON: true, SupportsSystemPrompt: true,
		MaxContextWindow: 128000, MaxOutputTokens: 4096,
	},
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
	capabilityConfigMu.RLock()
	extra := capabilityConfig.extraNonChatPatterns
	capabilityConfigMu.RUnlock()
	for _, p := range extra {
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
// Config-based extra patterns are also checked automatically.
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
	if !caps.Tools {
		capabilityConfigMu.RLock()
		extra := capabilityConfig.extraToolCapablePatterns
		capabilityConfigMu.RUnlock()
		for _, p := range extra {
			if strings.Contains(id, p) {
				caps.Tools = true
				break
			}
		}
	}

	// Build combined reasoning patterns (built-in + caller-provided + config)
	patterns := defaultReasoningPatterns
	capabilityConfigMu.RLock()
	configExtra := capabilityConfig.extraReasoningPatterns
	capabilityConfigMu.RUnlock()
	totalExtra := len(extraReasoningPatterns) + len(configExtra)
	if totalExtra > 0 {
		patterns = make([]string, 0, len(defaultReasoningPatterns)+totalExtra)
		patterns = append(patterns, defaultReasoningPatterns...)
		patterns = append(patterns, extraReasoningPatterns...)
		patterns = append(patterns, configExtra...)
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
	if caps.Chat {
		capabilityConfigMu.RLock()
		extra := capabilityConfig.extraCompletionOnlyPatterns
		capabilityConfigMu.RUnlock()
		for _, p := range extra {
			if strings.Contains(id, p) {
				caps.Chat = false
				break
			}
		}
	}

	return caps
}

// DetectCapabilities returns the capabilities for a model. It first checks the
// cache, then config overrides, then the hardcoded table. Returns nil if the
// model is not recognized.
func DetectCapabilities(provider, modelID string) (*ModelCapabilities, error) {
	// Check cache first
	cacheKey := provider + "/" + modelID
	if cached, ok := modelCapabilitiesCache.Load(cacheKey); ok {
		caps := cached.(*ModelCapabilities)
		return caps, nil
	}

	id := strings.ToLower(modelID)

	// Check config overrides (user-defined known capabilities)
	capabilityConfigMu.RLock()
	configCaps := capabilityConfig.knownCapabilities
	capabilityConfigMu.RUnlock()
	for knownID, caps := range configCaps {
		if strings.Contains(id, strings.ToLower(knownID)) {
			capsPtr := &caps
			modelCapabilitiesCache.Store(cacheKey, capsPtr)
			return capsPtr, nil
		}
	}

	// Check hardcoded table
	for knownID, caps := range knownModelCapabilities {
		if strings.Contains(id, strings.ToLower(knownID)) {
			// Cache the result (store pointer)
			capsPtr := &caps
			modelCapabilitiesCache.Store(cacheKey, capsPtr)
			return capsPtr, nil
		}
	}

	// Default capabilities for unknown models
	defaultCaps := &ModelCapabilities{
		SupportsTools:        false,
		SupportsImages:       false,
		SupportsStreaming:    true,
		SupportsJSON:         true,
		SupportsSystemPrompt: true,
		MaxContextWindow:     4096,
		MaxOutputTokens:      2048,
	}

	return defaultCaps, nil
}

// CheckModelHealth performs a lightweight health check for a model.
// Currently returns nil (always healthy) — can be extended to make actual
// API calls if needed.
func CheckModelHealth(provider, modelID string) error {
	// For now, just validate the model ID is not empty and not a known broken model
	if modelID == "" {
		return &ModelHealthError{ModelID: modelID, Reason: "empty model ID"}
	}
	if IsLikelyBrokenOnNvidia(modelID) {
		return &ModelHealthError{ModelID: modelID, Reason: "known broken on NVIDIA NIM"}
	}
	return nil
}

// ModelHealthError represents a model health check failure.
type ModelHealthError struct {
	ModelID string
	Reason  string
}

func (e *ModelHealthError) Error() string {
	return "model health check failed for " + e.ModelID + ": " + e.Reason
}
