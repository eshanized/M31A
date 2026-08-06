package extensions

import (
	"context"
	"encoding/json"
	"testing"
	"time"

	"github.com/eshanized/M31A/internal/core/types"
)

// TestExternalToolInterfaceCompile ensures ExternalTool interface is implementable.
func TestExternalToolInterfaceCompile(t *testing.T) {
	var _ ExternalTool = (*mockExternalTool)(nil)
}

// TestExternalProviderInterfaceCompile ensures ExternalProvider interface is implementable.
func TestExternalProviderInterfaceCompile(t *testing.T) {
	var _ ExternalProvider = (*mockExternalProvider)(nil)
}

// TestPhaseHookHandlerInterfaceCompile ensures PhaseHookHandler interface is implementable.
func TestPhaseHookHandlerInterfaceCompile(t *testing.T) {
	var _ PhaseHookHandler = (*mockPhaseHookHandler)(nil)
}

// mockExternalTool is a minimal implementation for compile-time checking.
type mockExternalTool struct{}

func (m *mockExternalTool) Name() string               { return "mock" }
func (m *mockExternalTool) Description() string        { return "mock tool" }
func (m *mockExternalTool) RiskLevel() types.RiskLevel { return types.RiskSafe }
func (m *mockExternalTool) ParameterSchema() string    { return "{}" }
func (m *mockExternalTool) Execute(ctx context.Context, input types.ToolInput) (types.ToolResult, error) {
	return types.ToolResult{Output: "ok"}, nil
}

// mockExternalProvider is a minimal implementation for compile-time checking.
type mockExternalProvider struct{}

func (m *mockExternalProvider) Name() string   { return "mock" }
func (m *mockExternalProvider) APIKey() string { return "" }
func (m *mockExternalProvider) FetchModels(ctx context.Context) ([]types.ModelInfo, error) {
	return nil, nil
}
func (m *mockExternalProvider) CachedModels() []types.ModelInfo { return nil }
func (m *mockExternalProvider) ChatCompletionStream(ctx context.Context, req types.ChatRequest) (*types.StreamIterator, error) {
	return nil, nil
}
func (m *mockExternalProvider) EstimateCost(modelID string, usage types.Usage) float64 { return 0 }
func (m *mockExternalProvider) HealthCheck(ctx context.Context) types.HealthStatus {
	return types.HealthStatus{Status: "healthy"}
}
func (m *mockExternalProvider) GetModel(id string) (*types.ModelInfo, error) { return nil, nil }

// mockPhaseHookHandler is a minimal implementation for compile-time checking.
type mockPhaseHookHandler struct{}

func (m *mockPhaseHookHandler) PrePhase(ctx context.Context, payload PhaseHookPayload) error {
	return nil
}
func (m *mockPhaseHookHandler) PostPhase(ctx context.Context, payload PhaseHookPayload, result *PhaseResult) error {
	return nil
}

// TestPhaseHookPayloadSerialization tests JSON serialization of PhaseHookPayload.
func TestPhaseHookPayloadSerialization(t *testing.T) {
	payload := PhaseHookPayload{
		PhaseName: types.PhaseExecute,
		WorkflowState: WorkflowStateSnapshot{
			CurrentPhase:   types.PhaseExecute,
			Goal:           "test goal",
			Tasks:          []types.Task{{ID: 1, Description: "test task"}},
			SessionID:      "test-session",
			BudgetSpentUSD: 0.01,
		},
		ExtensionConfig: []byte(`{"timeout":"30s"}`),
	}

	// This tests that the struct can be marshaled without error
	// (Context field is excluded via json:"-")
	data, err := json.Marshal(payload)
	if err != nil {
		t.Fatalf("marshal failed: %v", err)
	}

	var decoded PhaseHookPayload
	if err := json.Unmarshal(data, &decoded); err != nil {
		t.Fatalf("unmarshal failed: %v", err)
	}

	if decoded.PhaseName != payload.PhaseName {
		t.Errorf("phase name mismatch: %s != %s", decoded.PhaseName, payload.PhaseName)
	}
	if decoded.WorkflowState.Goal != payload.WorkflowState.Goal {
		t.Errorf("goal mismatch: %s != %s", decoded.WorkflowState.Goal, payload.WorkflowState.Goal)
	}
}

// TestExternalToolConfigSerialization tests JSON/TOML serialization.
func TestExternalToolConfigSerialization(t *testing.T) {
	cfg := ExternalToolConfig{
		Command: "/usr/bin/tool",
		Args:    []string{"--flag"},
		Env:     map[string]string{"KEY": "value"},
		Timeout: "60s",
	}

	data, err := json.Marshal(cfg)
	if err != nil {
		t.Fatalf("json marshal failed: %v", err)
	}

	var decoded ExternalToolConfig
	if err := json.Unmarshal(data, &decoded); err != nil {
		t.Fatalf("json unmarshal failed: %v", err)
	}

	if decoded.Command != cfg.Command {
		t.Errorf("command mismatch")
	}
	if len(decoded.Args) != len(cfg.Args) {
		t.Errorf("args length mismatch")
	}
	if decoded.Timeout != cfg.Timeout {
		t.Errorf("timeout mismatch")
	}
}

// TestExternalProviderConfigSerialization tests JSON/TOML serialization.
func TestExternalProviderConfigSerialization(t *testing.T) {
	cfg := ExternalProviderConfig{
		Command: "/usr/bin/provider",
		Args:    []string{"--model", "gpt-4"},
		Env:     map[string]string{"API_KEY": "secret"},
		Timeout: "120s",
	}

	data, err := json.Marshal(cfg)
	if err != nil {
		t.Fatalf("json marshal failed: %v", err)
	}

	var decoded ExternalProviderConfig
	if err := json.Unmarshal(data, &decoded); err != nil {
		t.Fatalf("json unmarshal failed: %v", err)
	}

	if decoded.Command != cfg.Command {
		t.Errorf("command mismatch")
	}
	if decoded.Timeout != cfg.Timeout {
		t.Errorf("timeout mismatch")
	}
}

// TestPhaseHookConfigSerialization tests JSON/TOML serialization.
func TestPhaseHookConfigSerialization(t *testing.T) {
	cfg := PhaseHookConfig{
		Command:   "/usr/bin/hook",
		Args:      []string{"--check"},
		Env:       map[string]string{"CONFIG": "file.yaml"},
		Phases:    []string{string(types.PhaseExecute), string(types.PhaseVerify)},
		HookTypes: []string{"pre", "post"},
		Timeout:   "30s",
	}

	data, err := json.Marshal(cfg)
	if err != nil {
		t.Fatalf("json marshal failed: %v", err)
	}

	var decoded PhaseHookConfig
	if err := json.Unmarshal(data, &decoded); err != nil {
		t.Fatalf("json unmarshal failed: %v", err)
	}

	if decoded.Command != cfg.Command {
		t.Errorf("command mismatch")
	}
	if len(decoded.Phases) != len(cfg.Phases) {
		t.Errorf("phases length mismatch")
	}
	if len(decoded.HookTypes) != len(cfg.HookTypes) {
		t.Errorf("hook_types length mismatch")
	}
}

// TestExtensionsConfigSerialization tests the top-level config.
func TestExtensionsConfigSerialization(t *testing.T) {
	cfg := ExtensionsConfig{
		Tools: map[string]ExternalToolConfig{
			"mytool": {Command: "/bin/tool", Timeout: "30s"},
		},
		Providers: map[string]ExternalProviderConfig{
			"myprovider": {Command: "/bin/provider", Timeout: "60s"},
		},
		Hooks: map[string]PhaseHookConfig{
			"myhook": {Command: "/bin/hook", Phases: []string{string(types.PhaseExecute)}, HookTypes: []string{"pre"}},
		},
	}

	data, err := json.Marshal(cfg)
	if err != nil {
		t.Fatalf("json marshal failed: %v", err)
	}

	var decoded ExtensionsConfig
	if err := json.Unmarshal(data, &decoded); err != nil {
		t.Fatalf("json unmarshal failed: %v", err)
	}

	if len(decoded.Tools) != 1 {
		t.Errorf("tools count mismatch")
	}
	if len(decoded.Providers) != 1 {
		t.Errorf("providers count mismatch")
	}
	if len(decoded.Hooks) != 1 {
		t.Errorf("hooks count mismatch")
	}
}

// TestParsedTimeout tests timeout parsing.
func TestParsedTimeout(t *testing.T) {
	tests := []struct {
		name     string
		timeout  string
		expected time.Duration
		wantErr  bool
	}{
		{"empty_default_tool", "", 30 * time.Second, false},
		{"valid_seconds", "30s", 30 * time.Second, false},
		{"valid_minutes", "2m", 2 * time.Minute, false},
		{"valid_hours", "1h", 1 * time.Hour, false},
		{"invalid", "invalid", 0, true},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			cfg := &ExternalToolConfig{Timeout: tt.timeout}
			d, err := cfg.ParsedTimeout()
			if tt.wantErr {
				if err == nil {
					t.Errorf("expected error, got nil")
				}
			} else {
				if err != nil {
					t.Errorf("unexpected error: %v", err)
				}
				if d != tt.expected {
					t.Errorf("expected %v, got %v", tt.expected, d)
				}
			}
		})
	}
}
