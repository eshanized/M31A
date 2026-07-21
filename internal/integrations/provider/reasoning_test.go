package provider

import (
	"testing"
)

func TestGetReasoningConfig_DeepSeek(t *testing.T) {
	cfg, ok := GetReasoningConfig("deepseek/deepseek-r1")
	if !ok {
		t.Fatal("expected config for deepseek")
	}
	if cfg.ModelFamily != "deepseek" {
		t.Fatalf("expected family %q, got %q", "deepseek", cfg.ModelFamily)
	}
	if cfg.SSEField != "choices.0.delta.reasoning_content" {
		t.Fatalf("expected SSEField %q, got %q", "choices.0.delta.reasoning_content", cfg.SSEField)
	}
}

func TestGetReasoningConfig_OpenAI(t *testing.T) {
	cfg, ok := GetReasoningConfig("openai/o1-mini")
	if !ok {
		t.Fatal("expected config for openai o-series")
	}
	if cfg.ModelFamily != "openai" {
		t.Fatalf("expected family %q, got %q", "openai", cfg.ModelFamily)
	}
	effort, ok := cfg.RequestParams["reasoning_effort"]
	if !ok {
		t.Fatal("expected reasoning_effort param")
	}
	if effort != "medium" {
		t.Fatalf("expected reasoning_effort %q, got %v", "medium", effort)
	}
}

func TestGetReasoningConfig_Anthropic(t *testing.T) {
	cfg, ok := GetReasoningConfig("anthropic/claude-sonnet-4")
	if !ok {
		t.Fatal("expected config for anthropic")
	}
	if cfg.ModelFamily != "anthropic" {
		t.Fatalf("expected family %q, got %q", "anthropic", cfg.ModelFamily)
	}
	thinking, ok := cfg.RequestParams["thinking"]
	if !ok {
		t.Fatal("expected thinking param")
	}
	thinkingMap, ok := thinking.(map[string]any)
	if !ok {
		t.Fatal("expected thinking param to be map")
	}
	if thinkingMap["type"] != "enabled" {
		t.Fatalf("expected thinking type %q, got %v", "enabled", thinkingMap["type"])
	}
}

func TestGetReasoningConfig_Unknown(t *testing.T) {
	_, ok := GetReasoningConfig("unknown/model-that-does-not-exist")
	if ok {
		t.Fatal("expected no config for unknown model")
	}
}

func TestApplyReasoningParams_Anthropic(t *testing.T) {
	body := map[string]any{
		"model": "claude-sonnet-4",
	}
	result := ApplyReasoningParams("anthropic/claude-sonnet-4", body)
	thinking, ok := result["thinking"]
	if !ok {
		t.Fatal("expected thinking param in body")
	}
	thinkingMap, ok := thinking.(map[string]any)
	if !ok {
		t.Fatal("expected thinking to be map")
	}
	bt, ok := thinkingMap["budget_tokens"].(int)
	if !ok {
		btf, ok := thinkingMap["budget_tokens"].(float64)
		if !ok {
			t.Fatalf("expected budget_tokens to be numeric, got %T", thinkingMap["budget_tokens"])
		}
		bt = int(btf)
	}
	if bt != 1024 {
		t.Fatalf("expected budget_tokens 1024, got %d", bt)
	}
}

func TestParseSSEChunk_Content(t *testing.T) {
	data := `{"choices":[{"delta":{"content":"Hello"}}]}`
	chunk, err := ParseSSEChunk(data, "any/model")
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if chunk == nil {
		t.Fatal("expected chunk, got nil")
	}
	if chunk.Type != "content" {
		t.Fatalf("expected type %q, got %q", "content", chunk.Type)
	}
	if chunk.Delta != "Hello" {
		t.Fatalf("expected delta %q, got %q", "Hello", chunk.Delta)
	}
}

func TestParseSSEChunk_Thinking(t *testing.T) {
	data := `{"choices":[{"delta":{"reasoning_content":"thinking..."}}]}`
	chunk, err := ParseSSEChunk(data, "deepseek/deepseek-r1")
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if chunk == nil {
		t.Fatal("expected chunk, got nil")
	}
	if chunk.Type != "thinking" {
		t.Fatalf("expected type %q, got %q", "thinking", chunk.Type)
	}
	if chunk.Delta != "thinking..." {
		t.Fatalf("expected delta %q, got %q", "thinking...", chunk.Delta)
	}
}

func TestParseSSEChunk_Done(t *testing.T) {
	data := `{"choices":[{"delta":{"content":""},"finish_reason":"stop"}]}`
	chunk, err := ParseSSEChunk(data, "any/model")
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if chunk == nil {
		t.Fatal("expected chunk, got nil")
	}
	if chunk.Type != "done" {
		t.Fatalf("expected type %q, got %q", "done", chunk.Type)
	}
}
