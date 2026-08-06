package extensions

import (
	"context"
	"encoding/json"
	"fmt"
	"testing"
	"time"

	"github.com/eshanized/M31A/internal/core/types"
)

// TestExternalToolAdapterSchemaProvider tests that ExternalToolAdapter implements SchemaProvider.
func TestExternalToolAdapterSchemaProvider(t *testing.T) {
	var _ types.SchemaProvider = (*ExternalToolAdapter)(nil)
}

// mockSubprocessManager implements a mock SubprocessManager for testing.
type mockSubprocessManager struct {
	callFunc func(ctx context.Context, req JSONRPCRequest, timeout time.Duration) (JSONRPCResponse, error)
	nextID   int64
}

func (m *mockSubprocessManager) nextRequestID() json.RawMessage {
	m.nextID++
	return json.RawMessage(fmt.Sprintf("%d", m.nextID))
}

func (m *mockSubprocessManager) Call(ctx context.Context, req JSONRPCRequest, timeout time.Duration) (JSONRPCResponse, error) {
	if m.callFunc != nil {
		return m.callFunc(ctx, req, timeout)
	}
	return JSONRPCResponse{JSONRPC: "2.0", ID: req.ID, Result: json.RawMessage("{}")}, nil
}

func (m *mockSubprocessManager) Start(ctx context.Context) error { return nil }
func (m *mockSubprocessManager) Stop() error                     { return nil }
func (m *mockSubprocessManager) IsRunning() bool                 { return true }
func (m *mockSubprocessManager) ProtocolVersion() string         { return "1.0" }
func (m *mockSubprocessManager) SupportedMethods() []string      { return []string{} }
func (m *mockSubprocessManager) SetWorkDir(dir string)           {}

// TestExternalToolAdapterMetadataFetch tests that metadata is fetched on construction.
func TestExternalToolAdapterMetadataFetch(t *testing.T) {
	callCount := 0
	mockProc := &mockSubprocessManager{
		callFunc: func(ctx context.Context, req JSONRPCRequest, timeout time.Duration) (JSONRPCResponse, error) {
			callCount++
			switch req.Method {
			case MethodToolName:
				return JSONRPCResponse{
					JSONRPC: "2.0",
					ID:      req.ID,
					Result:  json.RawMessage(`{"name":"test-tool"}`),
				}, nil
			case MethodToolDescription:
				return JSONRPCResponse{
					JSONRPC: "2.0",
					ID:      req.ID,
					Result:  json.RawMessage(`{"description":"A test tool"}`),
				}, nil
			case MethodToolRiskLevel:
				return JSONRPCResponse{
					JSONRPC: "2.0",
					ID:      req.ID,
					Result:  json.RawMessage(`{"risk_level":"safe"}`),
				}, nil
			case MethodToolSchema:
				return JSONRPCResponse{
					JSONRPC: "2.0",
					ID:      req.ID,
					Result:  json.RawMessage(`{"schema":"{\"type\":\"object\"}"}`),
				}, nil
			default:
				return JSONRPCResponse{}, fmt.Errorf("unexpected method: %s", req.Method)
			}
		},
	}

	adapter := NewExternalToolAdapter("test-tool", mockProc)

	// Access properties to trigger initialization
	name := adapter.Name()
	desc := adapter.Description()
	risk := adapter.RiskLevel()
	schema := adapter.ParameterSchema()

	if name != "test-tool" {
		t.Errorf("expected name 'test-tool', got %s", name)
	}
	if desc != "A test tool" {
		t.Errorf("expected description 'A test tool', got %s", desc)
	}
	if risk != types.RiskSafe {
		t.Errorf("expected risk level 'safe', got %s", risk)
	}
	if schema != "{\"type\":\"object\"}" {
		t.Errorf("expected schema '{\"type\":\"object\"}', got %s", schema)
	}

	// Should have made 4 calls (one for each metadata fetch)
	if callCount != 4 {
		t.Errorf("expected 4 metadata calls, got %d", callCount)
	}
}

// TestExternalToolAdapterExecute tests tool execution via JSON-RPC.
func TestExternalToolAdapterExecute(t *testing.T) {
	mockProc := &mockSubprocessManager{
		callFunc: func(ctx context.Context, req JSONRPCRequest, timeout time.Duration) (JSONRPCResponse, error) {
			switch req.Method {
			case MethodToolName:
				return JSONRPCResponse{
					JSONRPC: "2.0",
					ID:      req.ID,
					Result:  json.RawMessage(`{"name":"test-tool"}`),
				}, nil
			case MethodToolDescription:
				return JSONRPCResponse{
					JSONRPC: "2.0",
					ID:      req.ID,
					Result:  json.RawMessage(`{"description":"A test tool"}`),
				}, nil
			case MethodToolRiskLevel:
				return JSONRPCResponse{
					JSONRPC: "2.0",
					ID:      req.ID,
					Result:  json.RawMessage(`{"risk_level":"safe"}`),
				}, nil
			case MethodToolSchema:
				return JSONRPCResponse{
					JSONRPC: "2.0",
					ID:      req.ID,
					Result:  json.RawMessage(`{"schema":"{}"}`),
				}, nil
			case MethodToolExecute:
				var params ToolExecuteParams
				if err := json.Unmarshal(req.Params, &params); err != nil {
					return JSONRPCResponse{}, err
				}
				return JSONRPCResponse{
					JSONRPC: "2.0",
					ID:      req.ID,
					Result:  json.RawMessage(fmt.Sprintf(`{"result":{"tool_call_id":"%s","output":"executed: %v","duration_ms":10}}`, params.Input.Params["key"], params.Input.Params)),
				}, nil
			default:
				return JSONRPCResponse{}, fmt.Errorf("unexpected method: %s", req.Method)
			}
		},
	}

	adapter := NewExternalToolAdapter("test-tool", mockProc)

	ctx := context.Background()
	input := types.ToolInput{
		Name: "test-tool",
		Params: map[string]any{
			"key": "value",
		},
	}

	result, err := adapter.Execute(ctx, input)
	if err != nil {
		t.Fatalf("Execute failed: %v", err)
	}

	if result.Output == "" {
		t.Error("expected non-empty output")
	}
	if result.Error != "" {
		t.Errorf("unexpected error in result: %s", result.Error)
	}
}

// TestExternalToolAdapterExecuteError tests error handling during execution.
func TestExternalToolAdapterExecuteError(t *testing.T) {
	mockProc := &mockSubprocessManager{
		callFunc: func(ctx context.Context, req JSONRPCRequest, timeout time.Duration) (JSONRPCResponse, error) {
			switch req.Method {
			case MethodToolName:
				return JSONRPCResponse{
					JSONRPC: "2.0",
					ID:      req.ID,
					Result:  json.RawMessage(`{"name":"test-tool"}`),
				}, nil
			case MethodToolDescription:
				return JSONRPCResponse{
					JSONRPC: "2.0",
					ID:      req.ID,
					Result:  json.RawMessage(`{"description":"A test tool"}`),
				}, nil
			case MethodToolRiskLevel:
				return JSONRPCResponse{
					JSONRPC: "2.0",
					ID:      req.ID,
					Result:  json.RawMessage(`{"risk_level":"safe"}`),
				}, nil
			case MethodToolSchema:
				return JSONRPCResponse{
					JSONRPC: "2.0",
					ID:      req.ID,
					Result:  json.RawMessage(`{"schema":"{}"}`),
				}, nil
			case MethodToolExecute:
				return JSONRPCResponse{
					JSONRPC: "2.0",
					ID:      req.ID,
					Error:   &JSONRPCError{Code: -32603, Message: "execution failed"},
				}, nil
			default:
				return JSONRPCResponse{}, fmt.Errorf("unexpected method: %s", req.Method)
			}
		},
	}

	adapter := NewExternalToolAdapter("test-tool", mockProc)

	ctx := context.Background()
	input := types.ToolInput{
		Name:   "test-tool",
		Params: map[string]any{},
	}

	_, err := adapter.Execute(ctx, input)
	if err == nil {
		t.Error("expected error from tool execution")
	}
	if err.Error() != "tool execution error: execution failed" {
		t.Errorf("unexpected error message: %s", err.Error())
	}
}

// TestExternalToolAdapterExecuteTimeout tests timeout handling.
func TestExternalToolAdapterExecuteTimeout(t *testing.T) {
	mockProc := &mockSubprocessManager{
		callFunc: func(ctx context.Context, req JSONRPCRequest, timeout time.Duration) (JSONRPCResponse, error) {
			switch req.Method {
			case MethodToolName:
				return JSONRPCResponse{
					JSONRPC: "2.0",
					ID:      req.ID,
					Result:  json.RawMessage(`{"name":"test-tool"}`),
				}, nil
			case MethodToolDescription:
				return JSONRPCResponse{
					JSONRPC: "2.0",
					ID:      req.ID,
					Result:  json.RawMessage(`{"description":"A test tool"}`),
				}, nil
			case MethodToolRiskLevel:
				return JSONRPCResponse{
					JSONRPC: "2.0",
					ID:      req.ID,
					Result:  json.RawMessage(`{"risk_level":"safe"}`),
				}, nil
			case MethodToolSchema:
				return JSONRPCResponse{
					JSONRPC: "2.0",
					ID:      req.ID,
					Result:  json.RawMessage(`{"schema":"{}"}`),
				}, nil
			case MethodToolExecute:
				// Simulate timeout by not responding
				<-ctx.Done()
				return JSONRPCResponse{}, ctx.Err()
			default:
				return JSONRPCResponse{}, fmt.Errorf("unexpected method: %s", req.Method)
			}
		},
	}

	adapter := NewExternalToolAdapter("test-tool", mockProc)

	ctx, cancel := context.WithTimeout(context.Background(), 10*time.Millisecond)
	defer cancel()

	input := types.ToolInput{
		Name:   "test-tool",
		Params: map[string]any{},
	}

	_, err := adapter.Execute(ctx, input)
	if err == nil {
		t.Error("expected timeout error")
	}
	if err != context.DeadlineExceeded {
		t.Errorf("expected DeadlineExceeded, got %v", err)
	}
}

// TestExternalToolAdapterInvalidResponse tests handling of invalid JSON-RPC response.
func TestExternalToolAdapterInvalidResponse(t *testing.T) {
	mockProc := &mockSubprocessManager{
		callFunc: func(ctx context.Context, req JSONRPCRequest, timeout time.Duration) (JSONRPCResponse, error) {
			switch req.Method {
			case MethodToolName:
				return JSONRPCResponse{
					JSONRPC: "2.0",
					ID:      req.ID,
					Result:  json.RawMessage(`{"name":"test-tool"}`),
				}, nil
			case MethodToolDescription:
				return JSONRPCResponse{
					JSONRPC: "2.0",
					ID:      req.ID,
					Result:  json.RawMessage(`{"description":"A test tool"}`),
				}, nil
			case MethodToolRiskLevel:
				return JSONRPCResponse{
					JSONRPC: "2.0",
					ID:      req.ID,
					Result:  json.RawMessage(`{"risk_level":"safe"}`),
				}, nil
			case MethodToolSchema:
				return JSONRPCResponse{
					JSONRPC: "2.0",
					ID:      req.ID,
					Result:  json.RawMessage(`{"schema":"{}"}`),
				}, nil
			case MethodToolExecute:
				// Return invalid JSON
				return JSONRPCResponse{
					JSONRPC: "2.0",
					ID:      req.ID,
					Result:  json.RawMessage(`invalid json`),
				}, nil
			default:
				return JSONRPCResponse{}, fmt.Errorf("unexpected method: %s", req.Method)
			}
		},
	}

	adapter := NewExternalToolAdapter("test-tool", mockProc)

	ctx := context.Background()
	input := types.ToolInput{
		Name:   "test-tool",
		Params: map[string]any{},
	}

	_, err := adapter.Execute(ctx, input)
	if err == nil {
		t.Error("expected error from invalid response")
	}
}

// TestExternalToolAdapterSchemaProviderInterface tests SchemaProvider interface.
func TestExternalToolAdapterSchemaProviderInterface(t *testing.T) {
	mockProc := &mockSubprocessManager{
		callFunc: func(ctx context.Context, req JSONRPCRequest, timeout time.Duration) (JSONRPCResponse, error) {
			switch req.Method {
			case MethodToolName:
				return JSONRPCResponse{
					JSONRPC: "2.0",
					ID:      req.ID,
					Result:  json.RawMessage(`{"name":"test-tool"}`),
				}, nil
			case MethodToolDescription:
				return JSONRPCResponse{
					JSONRPC: "2.0",
					ID:      req.ID,
					Result:  json.RawMessage(`{"description":"A test tool"}`),
				}, nil
			case MethodToolRiskLevel:
				return JSONRPCResponse{
					JSONRPC: "2.0",
					ID:      req.ID,
					Result:  json.RawMessage(`{"risk_level":"safe"}`),
				}, nil
			case MethodToolSchema:
				return JSONRPCResponse{
					JSONRPC: "2.0",
					ID:      req.ID,
					Result:  json.RawMessage(`{"schema":"{\"type\":\"object\",\"properties\":{\"name\":{\"type\":\"string\"}}}"}`),
				}, nil
			default:
				return JSONRPCResponse{}, fmt.Errorf("unexpected method: %s", req.Method)
			}
		},
	}

	adapter := NewExternalToolAdapter("test-tool", mockProc)

	schema := adapter.ParameterSchema()
	expected := "{\"type\":\"object\",\"properties\":{\"name\":{\"type\":\"string\"}}}"
	if schema != expected {
		t.Errorf("expected schema %s, got %s", expected, schema)
	}
}

// TestExternalToolAdapterNotRunning tests calling Execute on stopped process.
func TestExternalToolAdapterNotRunning(t *testing.T) {
	mockProc := &mockSubprocessManager{
		callFunc: func(ctx context.Context, req JSONRPCRequest, timeout time.Duration) (JSONRPCResponse, error) {
			return JSONRPCResponse{}, fmt.Errorf("subprocess not running")
		},
	}

	adapter := NewExternalToolAdapter("test-tool", mockProc)

	ctx := context.Background()
	input := types.ToolInput{
		Name:   "test-tool",
		Params: map[string]any{},
	}

	_, err := adapter.Execute(ctx, input)
	if err == nil {
		t.Error("expected error when subprocess not running")
	}
}
