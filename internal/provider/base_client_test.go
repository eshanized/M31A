package provider

import (
	"testing"
	"time"

	"github.com/eshanized/M31A/pkg/types"
)

func TestNewBaseClient_AllFields(t *testing.T) {
	t.Parallel()
	bc := NewBaseClient("test-key", "https://api.example.com", "1.0", 10*time.Minute, 20*time.Minute, 500, 2000)

	if bc.APIKeyField != "test-key" {
		t.Errorf("expected APIKeyField 'test-key', got %q", bc.APIKeyField)
	}
	if bc.BaseURLField != "https://api.example.com" {
		t.Errorf("expected BaseURLField, got %q", bc.BaseURLField)
	}
	if bc.Version != "1.0" {
		t.Errorf("expected Version '1.0', got %q", bc.Version)
	}
	if bc.HTTPClient == nil {
		t.Error("expected non-nil HTTPClient")
	}
	if bc.CatalogClient == nil {
		t.Error("expected non-nil CatalogClient")
	}
	if bc.Cache == nil {
		t.Error("expected non-nil Cache")
	}
	if bc.HealthLiveMs != 500 {
		t.Errorf("expected HealthLiveMs 500, got %d", bc.HealthLiveMs)
	}
	if bc.HealthSlowMs != 2000 {
		t.Errorf("expected HealthSlowMs 2000, got %d", bc.HealthSlowMs)
	}
}

func TestBaseClient_CatalogClientHasTimeout(t *testing.T) {
	t.Parallel()
	bc := NewBaseClient("key", "url", "1.0", 0, 0, 0, 0)
	if bc.CatalogClient.Timeout == 0 {
		t.Error("expected CatalogClient to have a non-zero timeout")
	}
}

func TestBaseClient_HTTPClientNoTimeout(t *testing.T) {
	t.Parallel()
	bc := NewBaseClient("key", "url", "1.0", 0, 0, 0, 0)
	if bc.HTTPClient.Timeout != 0 {
		t.Error("expected HTTPClient to have zero timeout for SSE streaming")
	}
}

func TestChatRequest_AllFields(t *testing.T) {
	t.Parallel()
	req := ChatRequest{
		Model:     "gpt-4",
		Messages:  []types.Message{{Role: "user", Content: "hello"}},
		MaxTokens: 100,
		Tools: []ToolDefinition{
			{Name: "Bash", Description: "Execute bash", Parameters: "{}"},
		},
		ReasoningEnabled: true,
	}
	if req.Model != "gpt-4" {
		t.Errorf("expected 'gpt-4', got %q", req.Model)
	}
	if req.MaxTokens != 100 {
		t.Errorf("expected 100, got %d", req.MaxTokens)
	}
	if !req.ReasoningEnabled {
		t.Error("expected ReasoningEnabled to be true")
	}
}

func TestToolDefinition_AllFields(t *testing.T) {
	t.Parallel()
	td := ToolDefinition{
		Name:        "Bash",
		Description: "Execute bash commands",
		Parameters:  `{"type":"object"}`,
	}
	if td.Name != "Bash" {
		t.Errorf("expected 'Bash', got %q", td.Name)
	}
	if td.Parameters != `{"type":"object"}` {
		t.Errorf("expected parameters, got %q", td.Parameters)
	}
}

func TestChatRequest_ZeroValue(t *testing.T) {
	t.Parallel()
	var req ChatRequest
	if req.Model != "" {
		t.Errorf("expected empty model, got %q", req.Model)
	}
	if req.MaxTokens != 0 {
		t.Errorf("expected 0 max tokens, got %d", req.MaxTokens)
	}
}

func TestToolDefinition_ZeroValue(t *testing.T) {
	t.Parallel()
	var td ToolDefinition
	if td.Name != "" {
		t.Errorf("expected empty name, got %q", td.Name)
	}
}
