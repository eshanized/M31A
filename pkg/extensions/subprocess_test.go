package extensions

import (
	"context"
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"runtime"
	"testing"
	"time"

	"github.com/eshanized/M31A/internal/core/types"
)

// TestSubprocessManager tests the SubprocessManager with a simple echo script.
func TestSubprocessManager(t *testing.T) {
	// Create a test extension script
	scriptPath := createTestExtensionScript(t)
	defer os.Remove(scriptPath)

	ctx := context.Background()

	cmd := scriptPath
	args := []string{}
	if runtime.GOOS == "windows" {
		cmd = "bash"
		args = []string{scriptPath}
	}

	// Create subprocess manager
	proc := NewSubprocessManager(cmd, args, nil, 10*time.Second)
	defer proc.Stop()

	// Start the subprocess
	if err := proc.Start(ctx); err != nil {
		t.Fatalf("Start failed: %v", err)
	}

	// Test handshake
	if proc.ProtocolVersion() != "1.0" {
		t.Errorf("expected protocol version 1.0, got %s", proc.ProtocolVersion())
	}

	// Test tool.name
	req := JSONRPCRequest{
		JSONRPC: "2.0",
		ID:      json.RawMessage("1"),
		Method:  MethodToolName,
	}

	resp, err := proc.Call(ctx, req, 5*time.Second)
	if err != nil {
		t.Fatalf("Call failed: %v", err)
	}

	if resp.Error != nil {
		t.Fatalf("RPC error: %s", resp.Error.Message)
	}

	var result ToolNameResult
	unmarshalErr := json.Unmarshal(resp.Result, &result)
	if unmarshalErr != nil {
		t.Fatalf("unmarshal failed: %v", unmarshalErr)
	}

	if result.Name != "test-tool" {
		t.Errorf("expected name 'test-tool', got '%s'", result.Name)
	}

	// Test tool.description
	req = JSONRPCRequest{
		JSONRPC: "2.0",
		ID:      json.RawMessage("2"),
		Method:  MethodToolDescription,
	}

	resp, err = proc.Call(ctx, req, 5*time.Second)
	if err != nil {
		t.Fatalf("Call failed: %v", err)
	}

	var descResult ToolDescriptionResult
	unmarshalErr = json.Unmarshal(resp.Result, &descResult)
	if unmarshalErr != nil {
		t.Fatalf("unmarshal failed: %v", unmarshalErr)
	}

	if descResult.Description != "A test tool for unit testing" {
		t.Errorf("unexpected description: %s", descResult.Description)
	}

	// Test tool.risk_level
	req = JSONRPCRequest{
		JSONRPC: "2.0",
		ID:      json.RawMessage("3"),
		Method:  MethodToolRiskLevel,
	}

	resp, err = proc.Call(ctx, req, 5*time.Second)
	if err != nil {
		t.Fatalf("Call failed: %v", err)
	}

	var riskResult ToolRiskLevelResult
	unmarshalErr = json.Unmarshal(resp.Result, &riskResult)
	if unmarshalErr != nil {
		t.Fatalf("unmarshal failed: %v", unmarshalErr)
	}

	if riskResult.RiskLevel != "safe" {
		t.Errorf("expected risk level 'safe', got '%s'", riskResult.RiskLevel)
	}

	// Test tool.schema
	req = JSONRPCRequest{
		JSONRPC: "2.0",
		ID:      json.RawMessage("4"),
		Method:  MethodToolSchema,
	}

	resp, err = proc.Call(ctx, req, 5*time.Second)
	if err != nil {
		t.Fatalf("Call failed: %v", err)
	}

	var schemaResult ToolSchemaResult
	unmarshalErr = json.Unmarshal(resp.Result, &schemaResult)
	if unmarshalErr != nil {
		t.Fatalf("unmarshal failed: %v", unmarshalErr)
	}

	if schemaResult.Schema == "" {
		t.Error("expected non-empty schema")
	}

	// Test tool.execute
	req = JSONRPCRequest{
		JSONRPC: "2.0",
		ID:      json.RawMessage("5"),
		Method:  MethodToolExecute,
	}

	params := ToolExecuteParams{
		Input: types.ToolInput{
			Name:   "test-tool",
			Params: map[string]any{"message": "hello"},
		},
	}
	paramsData, _ := json.Marshal(params)
	req.Params = paramsData

	resp, err = proc.Call(ctx, req, 5*time.Second)
	if err != nil {
		t.Fatalf("Call failed: %v", err)
	}

	var execResult ToolExecuteResult
	if err := json.Unmarshal(resp.Result, &execResult); err != nil {
		t.Fatalf("unmarshal failed: %v", err)
	}

	if execResult.Result.Output == "" {
		t.Error("expected non-empty output")
	}

	// Test shutdown
	if err := proc.Stop(); err != nil {
		t.Logf("Stop returned error (may be expected): %v", err)
	}
}

// TestSubprocessManagerMultipleCalls tests multiple sequential calls.
func TestSubprocessManagerMultipleCalls(t *testing.T) {
	scriptPath := createTestExtensionScript(t)
	defer os.Remove(scriptPath)

	ctx := context.Background()
	cmd := scriptPath
	args := []string{}
	if runtime.GOOS == "windows" {
		cmd = "bash"
		args = []string{scriptPath}
	}
	proc := NewSubprocessManager(cmd, args, nil, 10*time.Second)
	defer proc.Stop()

	if err := proc.Start(ctx); err != nil {
		t.Fatalf("Start failed: %v", err)
	}

	// Make multiple calls
	for i := 0; i < 10; i++ {
		req := JSONRPCRequest{
			JSONRPC: "2.0",
			ID:      json.RawMessage(fmt.Sprintf("%d", i+10)),
			Method:  MethodToolExecute,
		}

		params := ToolExecuteParams{
			Input: types.ToolInput{
				Name:   "test-tool",
				Params: map[string]any{"iteration": i},
			},
		}
		paramsData, _ := json.Marshal(params)
		req.Params = paramsData

		resp, err := proc.Call(ctx, req, 5*time.Second)
		if err != nil {
			t.Fatalf("Call %d failed: %v", i, err)
		}

		if resp.Error != nil {
			t.Fatalf("Call %d RPC error: %s", i, resp.Error.Message)
		}
	}

	if err := proc.Stop(); err != nil {
		t.Logf("Stop error: %v", err)
	}
}

// TestSubprocessManagerTimeout tests timeout handling.
func TestSubprocessManagerTimeout(t *testing.T) {
	// Create a slow test extension script
	scriptPath := createSlowTestExtensionScript(t)
	defer os.Remove(scriptPath)

	ctx := context.Background()
	cmd := scriptPath
	args := []string{}
	if runtime.GOOS == "windows" {
		cmd = "bash"
		args = []string{scriptPath}
	}
	proc := NewSubprocessManager(cmd, args, nil, 1*time.Second)
	defer proc.Stop()

	if err := proc.Start(ctx); err != nil {
		t.Fatalf("Start failed: %v", err)
	}

	// This should timeout
	req := JSONRPCRequest{
		JSONRPC: "2.0",
		ID:      json.RawMessage("100"),
		Method:  MethodToolExecute,
	}

	params := ToolExecuteParams{
		Input: types.ToolInput{
			Name:   "test-tool",
			Params: map[string]any{"delay": "2s"},
		},
	}
	paramsData, _ := json.Marshal(params)
	req.Params = paramsData

	_, err := proc.Call(ctx, req, 500*time.Millisecond)
	if err == nil {
		t.Error("expected timeout error")
	}

	// Process should still be usable after timeout
	req2 := JSONRPCRequest{
		JSONRPC: "2.0",
		ID:      json.RawMessage("101"),
		Method:  MethodToolName,
	}

	resp, err := proc.Call(ctx, req2, 2*time.Second)
	if err != nil {
		t.Fatalf("Call after timeout failed: %v", err)
	}

	var result ToolNameResult
	_ = json.Unmarshal(resp.Result, &result)
	if result.Name == "" {
		t.Error("expected name after timeout recovery")
	}
}

// TestSubprocessManagerStopIdempotent tests that Stop can be called multiple times.
func TestSubprocessManagerStopIdempotent(t *testing.T) {
	scriptPath := createTestExtensionScript(t)
	defer os.Remove(scriptPath)

	ctx := context.Background()
	cmd := scriptPath
	args := []string{}
	if runtime.GOOS == "windows" {
		cmd = "bash"
		args = []string{scriptPath}
	}
	proc := NewSubprocessManager(cmd, args, nil, 10*time.Second)

	if err := proc.Start(ctx); err != nil {
		t.Fatalf("Start failed: %v", err)
	}

	// Call Stop multiple times
	for i := 0; i < 3; i++ {
		if err := proc.Stop(); err != nil {
			t.Logf("Stop %d error (may be expected): %v", i, err)
		}
	}

	// Should not panic
	proc.Stop()
}

// TestSubprocessManagerNotRunning tests calling Call on stopped process.
func TestSubprocessManagerNotRunning(t *testing.T) {
	scriptPath := createTestExtensionScript(t)
	defer os.Remove(scriptPath)

	ctx := context.Background()
	cmd := scriptPath
	args := []string{}
	if runtime.GOOS == "windows" {
		cmd = "bash"
		args = []string{scriptPath}
	}
	proc := NewSubprocessManager(cmd, args, nil, 10*time.Second)

	// Call without Start
	req := JSONRPCRequest{
		JSONRPC: "2.0",
		ID:      json.RawMessage("1"),
		Method:  MethodToolName,
	}

	_, err := proc.Call(ctx, req, 5*time.Second)
	if err == nil {
		t.Error("expected error when calling on stopped process")
	}

	// Start then stop, then call
	startErr := proc.Start(ctx)
	if startErr != nil {
		t.Fatalf("Start failed: %v", startErr)
	}
	_ = proc.Stop()

	_, err = proc.Call(ctx, req, 5*time.Second)
	if err == nil {
		t.Error("expected error when calling on stopped process")
	}
}

// createTestExtensionScript creates a simple test extension script.
func createTestExtensionScript(t *testing.T) string {
	script := `#!/usr/bin/env bash
# Test extension that implements JSON-RPC for a simple tool

read_request() {
    local request
    read -r request
    echo "$request"
}

send_response() {
    local id="$1"
    local result="$2"
    cat <<EOF
{"jsonrpc":"2.0","id":$id,"result":$result}
EOF
}

send_error() {
    local id="$1"
    local code="$2"
    local message="$3"
    cat <<EOF
{"jsonrpc":"2.0","id":$id,"error":{"code":$code,"message":"$message"}}
EOF
}

while request=$(read_request); do
    if [ -z "$request" ]; then
        continue
    fi

    method=$(echo "$request" | jq -r '.method // ""')
    id=$(echo "$request" | jq -r '.id // ""')

    case "$method" in
        "handshake")
            send_response "$id" '{"protocol_version":"1.0","supported_methods":["tool.name","tool.description","tool.risk_level","tool.schema","tool.execute","handshake","shutdown"]}'
            ;;
        "tool.name")
            send_response "$id" '{"name":"test-tool"}'
            ;;
        "tool.description")
            send_response "$id" '{"description":"A test tool for unit testing"}'
            ;;
        "tool.risk_level")
            send_response "$id" '{"risk_level":"safe"}'
            ;;
        "tool.schema")
            send_response "$id" '{"schema":"{\"type\":\"object\",\"properties\":{\"message\":{\"type\":\"string\"}}"}'
            ;;
        "tool.execute")
            params=$(echo "$request" | jq -c '.params // {}')
            input=$(echo "$params" | jq -c '.input // {}')
            message=$(echo "$input" | jq -r '.params.message // "default"')
            iteration=$(echo "$input" | jq -r '.params.iteration // 0')
            delay=$(echo "$input" | jq -r '.params.delay // "0"')

            if [ "$delay" != "0" ]; then
                sleep "$delay"
            fi

            output="Test tool executed: message=$message, iteration=$iteration"
            send_response "$id" "{\"result\":{\"tool_call_id\":\"test-$iteration\",\"output\":\"$output\",\"duration_ms\":10}}"
            ;;
        "shutdown")
            send_response "$id" '{}'
            exit 0
            ;;
        *)
            send_error "$id" -32601 "Method not found: $method"
            ;;
    esac
done
`

	tmpDir := t.TempDir()
	scriptPath := filepath.Join(tmpDir, "test_extension.sh")
	if err := os.WriteFile(scriptPath, []byte(script), 0755); err != nil {
		t.Fatalf("write script: %v", err)
	}

	return scriptPath
}

// createSlowTestExtensionScript creates a test extension with configurable delay.
func createSlowTestExtensionScript(t *testing.T) string {
	script := `#!/usr/bin/env bash
# Slow test extension for timeout testing

read_request() {
    local request
    read -r request
    echo "$request"
}

send_response() {
    local id="$1"
    local result="$2"
    cat <<EOF
{"jsonrpc":"2.0","id":$id,"result":$result}
EOF
}

send_error() {
    local id="$1"
    local code="$2"
    local message="$3"
    cat <<EOF
{"jsonrpc":"2.0","id":$id,"error":{"code":$code,"message":"$message"}}
EOF
}

while request=$(read_request); do
    if [ -z "$request" ]; then
        continue
    fi

    method=$(echo "$request" | jq -r '.method // ""')
    id=$(echo "$request" | jq -r '.id // ""')

    case "$method" in
        "handshake")
            send_response "$id" '{"protocol_version":"1.0","supported_methods":["tool.name","tool.description","tool.risk_level","tool.schema","tool.execute","handshake","shutdown"]}'
            ;;
        "tool.name")
            send_response "$id" '{"name":"test-tool"}'
            ;;
        "tool.description")
            send_response "$id" '{"description":"A slow test tool"}'
            ;;
        "tool.risk_level")
            send_response "$id" '{"risk_level":"safe"}'
            ;;
        "tool.schema")
            send_response "$id" '{"schema":"{\"type\":\"object\",\"properties\":{\"delay\":{\"type\":\"string\"}}"}'
            ;;
        "tool.execute")
            params=$(echo "$request" | jq -c '.params // {}')
            input=$(echo "$params" | jq -c '.input // {}')
            delay=$(echo "$input" | jq -r '.params.delay // "0"')

            if [ "$delay" != "0" ] && [ "$delay" != "null" ]; then
                sleep "$delay"
            fi

            output="Slow tool executed after $delay"
            send_response "$id" "{\"result\":{\"tool_call_id\":\"slow-1\",\"output\":\"$output\",\"duration_ms\":10}}"
            ;;
        "shutdown")
            send_response "$id" '{}'
            exit 0
            ;;
        *)
            send_error "$id" -32601 "Method not found: $method"
            ;;
    esac
done
`

	tmpDir := t.TempDir()
	scriptPath := filepath.Join(tmpDir, "slow_extension.sh")
	if err := os.WriteFile(scriptPath, []byte(script), 0755); err != nil {
		t.Fatalf("write script: %v", err)
	}

	return scriptPath
}

// TestProtocol tests JSON-RPC protocol types.
func TestProtocol(t *testing.T) {
	// Test JSONRPCRequest marshaling
	req := JSONRPCRequest{
		JSONRPC: "2.0",
		ID:      json.RawMessage("1"),
		Method:  "tool.execute",
		Params:  json.RawMessage(`{"input":{"name":"test","params":{}}}`),
	}

	data, err := json.Marshal(req)
	if err != nil {
		t.Fatalf("marshal request: %v", err)
	}

	var decoded JSONRPCRequest
	unmarshalErr := json.Unmarshal(data, &decoded)
	if unmarshalErr != nil {
		t.Fatalf("unmarshal request: %v", unmarshalErr)
	}

	if decoded.Method != req.Method {
		t.Errorf("method mismatch")
	}

	// Test JSONRPCResponse with result
	resp := JSONRPCResponse{
		JSONRPC: "2.0",
		ID:      json.RawMessage("1"),
		Result:  json.RawMessage(`{"output":"success"}`),
	}

	data, err = json.Marshal(resp)
	if err != nil {
		t.Fatalf("marshal response: %v", err)
	}

	var decodedResp JSONRPCResponse
	unmarshalErr = json.Unmarshal(data, &decodedResp)
	if unmarshalErr != nil {
		t.Fatalf("unmarshal response: %v", unmarshalErr)
	}

	if decodedResp.Error != nil {
		t.Errorf("expected no error")
	}

	// Test JSONRPCResponse with error
	errResp := JSONRPCResponse{
		JSONRPC: "2.0",
		ID:      json.RawMessage("1"),
		Error: &JSONRPCError{
			Code:    -32601,
			Message: "Method not found",
		},
	}

	data, err = json.Marshal(errResp)
	if err != nil {
		t.Fatalf("marshal error response: %v", err)
	}

	var decodedErrResp JSONRPCResponse
	err = json.Unmarshal(data, &decodedErrResp)
	if err != nil {
		t.Fatalf("unmarshal error response: %v", err)
	}

	if decodedErrResp.Error == nil {
		t.Errorf("expected error")
	}
	if decodedErrResp.Error.Code != -32601 {
		t.Errorf("error code mismatch")
	}

	// Test HandshakeResult
	handshake := HandshakeResult{
		ProtocolVersion:  "1.0",
		SupportedMethods: []string{"tool.name", "tool.execute"},
	}

	data, err = json.Marshal(handshake)
	if err != nil {
		t.Fatalf("marshal handshake: %v", err)
	}

	var decodedHandshake HandshakeResult
	err = json.Unmarshal(data, &decodedHandshake)
	if err != nil {
		t.Fatalf("unmarshal handshake: %v", err)
	}

	if decodedHandshake.ProtocolVersion != "1.0" {
		t.Errorf("protocol version mismatch")
	}
	if len(decodedHandshake.SupportedMethods) != 2 {
		t.Errorf("supported methods count mismatch")
	}
}
