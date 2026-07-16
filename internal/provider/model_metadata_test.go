package provider

import (
	"testing"

	"github.com/eshanized/M31A/pkg/types"
)

func TestNormalizeModelID(t *testing.T) {
	tests := []struct {
		input    string
		expected string
	}{
		{"anthropic/claude-fable-5", "claude-fable-5"},
		{"openai/gpt-5.4", "gpt-5.4"},
		{"google/gemini-3.5-flash", "gemini-3.5-flash"},
		{"claude-fable-5", "claude-fable-5"},
		{"gpt-5.4", "gpt-5.4"},
		{"meta-llama/llama-4-maverick", "llama-4-maverick"},
		{"deepseek/deepseek-r1", "deepseek-r1"},
	}
	for _, tt := range tests {
		t.Run(tt.input, func(t *testing.T) {
			got := normalizeModelID(tt.input)
			if got != tt.expected {
				t.Errorf("normalizeModelID(%q) = %q, want %q", tt.input, got, tt.expected)
			}
		})
	}
}

func TestParseFloatSafe(t *testing.T) {
	tests := []struct {
		input    string
		expected float64
	}{
		{"0.000005", 0.000005},
		{"0.00001", 0.00001},
		{"-1", 0},
		{"", 0},
		{"invalid", 0},
		{"2.5", 2.5},
	}
	for _, tt := range tests {
		t.Run(tt.input, func(t *testing.T) {
			got := parseFloatSafe(tt.input)
			if got != tt.expected {
				t.Errorf("parseFloatSafe(%q) = %v, want %v", tt.input, got, tt.expected)
			}
		})
	}
}

func TestLocalMetadataFallback(t *testing.T) {
	db := LocalMetadataFallback()
	if len(db) == 0 {
		t.Fatal("LocalMetadataFallback() returned empty map")
	}

	// Verify a few known models
	for _, tc := range []struct {
		id            string
		minCtxLength  int64
		expectPricing bool
	}{
		{"claude-fable-5", 100_000, true},
		{"gemini-3.5-flash", 100_000, true},
		{"gpt-5.4", 100_000, true},
		{"deepseek-r1", 10_000, true},
	} {
		meta, ok := db[tc.id]
		if !ok {
			t.Errorf("LocalMetadataFallback() missing model %q", tc.id)
			continue
		}
		if meta.ContextLength < tc.minCtxLength {
			t.Errorf("model %q context_length = %d, want >= %d", tc.id, meta.ContextLength, tc.minCtxLength)
		}
		if tc.expectPricing && meta.Pricing.InputPerMToken == 0 && meta.Pricing.OutputPerMToken == 0 {
			t.Errorf("model %q has zero pricing (expected non-zero)", tc.id)
		}
	}
}

func TestLookupMetadata_ExactMatch(t *testing.T) {
	openRouter := map[string]ModelMetadata{
		"anthropic/claude-fable-5": {ContextLength: 1_000_000, Pricing: types.Pricing{InputPerMToken: 10, OutputPerMToken: 50}},
	}

	meta, ok := lookupMetadata("anthropic/claude-fable-5", openRouter)
	if !ok {
		t.Fatal("lookupMetadata() failed for exact match")
	}
	if meta.ContextLength != 1_000_000 {
		t.Errorf("context_length = %d, want 1000000", meta.ContextLength)
	}
}

func TestLookupMetadata_NormalizedMatch(t *testing.T) {
	openRouter := map[string]ModelMetadata{
		"anthropic/claude-fable-5": {ContextLength: 1_000_000, Pricing: types.Pricing{InputPerMToken: 10, OutputPerMToken: 50}},
	}

	// Should match via prefix stripping
	meta, ok := lookupMetadata("claude-fable-5", openRouter)
	if !ok {
		t.Fatal("lookupMetadata() failed for normalized match")
	}
	if meta.ContextLength != 1_000_000 {
		t.Errorf("context_length = %d, want 1000000", meta.ContextLength)
	}
}

func TestLookupMetadata_LocalFallback(t *testing.T) {
	openRouter := map[string]ModelMetadata{}

	// Should fall back to local database
	meta, ok := lookupMetadata("claude-fable-5", openRouter)
	if !ok {
		t.Fatal("lookupMetadata() failed for local fallback")
	}
	if meta.ContextLength != 1_000_000 {
		t.Errorf("context_length = %d, want 1000000", meta.ContextLength)
	}
	if meta.Source != "local" {
		t.Errorf("source = %q, want %q", meta.Source, "local")
	}
}

func TestLookupMetadata_NotFound(t *testing.T) {
	openRouter := map[string]ModelMetadata{}

	_, ok := lookupMetadata("nonexistent-model-xyz", openRouter)
	if ok {
		t.Error("lookupMetadata() should return false for unknown model")
	}
}

func TestEnrichModelInfo_ZenProvider(t *testing.T) {
	models := []types.ModelInfo{
		{
			ID:            "claude-fable-5",
			ContextLength: types.DefaultContextLength, // 128K default
			Pricing:       types.Pricing{InputPerMToken: 0, OutputPerMToken: 0},
		},
		{
			ID:            "gemini-3.5-flash",
			ContextLength: types.DefaultContextLength,
			Pricing:       types.Pricing{InputPerMToken: 0, OutputPerMToken: 0},
		},
	}

	enriched := EnrichModelInfo(models, "zen")
	if len(enriched) != len(models) {
		t.Fatalf("EnrichModelInfo returned %d models, want %d", len(enriched), len(models))
	}

	for _, m := range enriched {
		switch m.ID {
		case "claude-fable-5":
			if m.ContextLength != 1_000_000 {
				t.Errorf("claude-fable-5 context_length = %d, want 1000000", m.ContextLength)
			}
			if m.Pricing.InputPerMToken == 0 {
				t.Error("claude-fable-5 pricing should be enriched (non-zero)")
			}
		case "gemini-3.5-flash":
			if m.ContextLength != 1_048_576 {
				t.Errorf("gemini-3.5-flash context_length = %d, want 1048576", m.ContextLength)
			}
			if m.Pricing.InputPerMToken == 0 {
				t.Error("gemini-3.5-flash pricing should be enriched (non-zero)")
			}
		}
	}
}

func TestEnrichModelInfo_NonZenProvider(t *testing.T) {
	models := []types.ModelInfo{
		{
			ID:            "claude-fable-5",
			ContextLength: types.DefaultContextLength,
			Pricing:       types.Pricing{InputPerMToken: 0, OutputPerMToken: 0},
		},
	}

	// Non-zen providers: context_length enriched, pricing NOT enriched
	enriched := EnrichModelInfo(models, "openrouter")
	if len(enriched) != 1 {
		t.Fatalf("EnrichModelInfo returned %d models, want 1", len(enriched))
	}

	m := enriched[0]
	if m.ContextLength != 1_000_000 {
		t.Errorf("context_length = %d, want 1000000", m.ContextLength)
	}
	// Pricing should NOT be enriched for non-zen providers
	if m.Pricing.InputPerMToken != 0 {
		t.Errorf("pricing should not be enriched for openrouter, got input=%f", m.Pricing.InputPerMToken)
	}
}

func TestEnrichModelInfo_PreservesNonDefaultValues(t *testing.T) {
	models := []types.ModelInfo{
		{
			ID:            "claude-fable-5",
			ContextLength: 500_000, // Already set by provider
			Pricing:       types.Pricing{InputPerMToken: 10, OutputPerMToken: 50},
		},
	}

	enriched := EnrichModelInfo(models, "zen")
	m := enriched[0]

	// Should NOT overwrite non-default values
	if m.ContextLength != 500_000 {
		t.Errorf("context_length = %d, want 500000 (should not overwrite)", m.ContextLength)
	}
	if m.Pricing.InputPerMToken != 10 {
		t.Errorf("pricing input = %f, want 10 (should not overwrite)", m.Pricing.InputPerMToken)
	}
}

func TestEnrichModelInfo_EmptySlice(t *testing.T) {
	enriched := EnrichModelInfo(nil, "zen")
	if enriched != nil {
		t.Errorf("EnrichModelInfo(nil) = %v, want nil", enriched)
	}

	enriched = EnrichModelInfo([]types.ModelInfo{}, "zen")
	if len(enriched) != 0 {
		t.Errorf("EnrichModelInfo(empty) returned %d models, want 0", len(enriched))
	}
}

func TestEnrichModelInfo_UnknownModel(t *testing.T) {
	models := []types.ModelInfo{
		{
			ID:            "unknown-model-xyz",
			ContextLength: types.DefaultContextLength,
			Pricing:       types.Pricing{InputPerMToken: 0, OutputPerMToken: 0},
		},
	}

	enriched := EnrichModelInfo(models, "zen")
	m := enriched[0]

	// Unknown model should keep defaults
	if m.ContextLength != types.DefaultContextLength {
		t.Errorf("unknown model context_length = %d, want %d (default)", m.ContextLength, types.DefaultContextLength)
	}
	if m.Pricing.InputPerMToken != 0 {
		t.Errorf("unknown model pricing = %f, want 0", m.Pricing.InputPerMToken)
	}
}
