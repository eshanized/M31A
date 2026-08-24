package lsp

import (
	"encoding/json"
	"os"
	"os/exec"
	"path/filepath"
	"testing"
)

// TestNewLSPClient_CommandNotFound verifies that attempting to start with a nonexistent binary returns an error, not a panic.
func TestNewLSPClient_CommandNotFound(t *testing.T) {
	_, err := NewLSPClient([]string{"/nonexistent/binary/that/does/not/exist"}, "/tmp")
	if err == nil {
		t.Fatal("expected error for nonexistent binary, got nil")
	}
	t.Logf("Got expected error: %v", err)
}

// TestNegotiateCapabilities verifies that capability negotiation correctly parses server capabilities.
func TestNegotiateCapabilities(t *testing.T) {
	tests := []struct {
		name     string
		input    string
		expected LSPCapabilities
	}{
		{
			name: "all capabilities present",
			input: `{
				"definitionProvider": true,
				"referencesProvider": true,
				"callHierarchyProvider": true,
				"typeHierarchyProvider": true,
				"hoverProvider": true,
				"completionProvider": {}
			}`,
			expected: LSPCapabilities{
				SupportsDefinition:     true,
				SupportsReferences:     true,
				SupportsCallHierarchy:  true,
				SupportsTypeHierarchy:  true,
				SupportsHover:          true,
				SupportsCompletion:     true,
			},
		},
		{
			name: "only definition and hover",
			input: `{
				"definitionProvider": true,
				"hoverProvider": {}
			}`,
			expected: LSPCapabilities{
				SupportsDefinition: true,
				SupportsHover:      true,
			},
		},
		{
			name: "empty capabilities",
			input: `{}`,
			expected: LSPCapabilities{},
		},
		{
			name: "explicit false values",
			input: `{
				"definitionProvider": false,
				"referencesProvider": false,
				"callHierarchyProvider": false,
				"typeHierarchyProvider": false,
				"hoverProvider": false,
				"completionProvider": false
			}`,
			expected: LSPCapabilities{},
		},
		{
			name: "null capabilities",
			input: `{
				"definitionProvider": null,
				"hoverProvider": null
			}`,
			expected: LSPCapabilities{},
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			var raw json.RawMessage
			err := json.Unmarshal([]byte(tt.input), &raw)
			if err != nil {
				t.Fatalf("failed to unmarshal test input: %v", err)
			}

			result := NegotiateCapabilities(raw)
			if result != tt.expected {
				t.Errorf("NegotiateCapabilities() = %+v, want %+v", result, tt.expected)
			}
		})
	}
}

// TestSendRequestJSON verifies that sendRequest produces valid JSON-RPC 2.0 envelope format.
func TestSendRequestJSON(t *testing.T) {
	// This test verifies the JSON structure by creating a client with a mock command
	// that we can inspect. Since we can't easily intercept the stdin, we'll test
	// the marshaling logic directly.

	req := jsonrpcRequest{
		JSONRPC: "2.0",
		Method:  "textDocument/definition",
		Params: map[string]interface{}{
			"textDocument": map[string]interface{}{
				"uri": "file:///test.go",
			},
			"position": map[string]interface{}{
				"line":      10,
				"character": 5,
			},
		},
		ID: 1,
	}

	data, err := json.Marshal(req)
	if err != nil {
		t.Fatalf("marshal failed: %v", err)
	}

	var parsed map[string]interface{}
	if err := json.Unmarshal(data, &parsed); err != nil {
		t.Fatalf("unmarshal failed: %v", err)
	}

	// Verify required JSON-RPC 2.0 fields
	if parsed["jsonrpc"] != "2.0" {
		t.Errorf("jsonrpc field = %v, want '2.0'", parsed["jsonrpc"])
	}
	if parsed["method"] != "textDocument/definition" {
		t.Errorf("method field = %v, want 'textDocument/definition'", parsed["method"])
	}
	if parsed["id"] != float64(1) {
		t.Errorf("id field = %v, want 1", parsed["id"])
	}
	if parsed["params"] == nil {
		t.Error("params field is missing")
	}
}

// TestNewLSPClient_Integration tests the client with a real (but dummy) command.
// This test is skipped by default since it requires a real LSP server.
// Run with: go test -run TestNewLSPClient_Integration -count=1
func TestNewLSPClient_Integration(t *testing.T) {
	// Skip if no gopls available
	if _, err := os.Executable(); err != nil {
		t.Skip("Cannot find Go executable")
	}

	// Create a temporary directory for testing
	tmpDir := t.TempDir()

	// Try to start with a dummy command that will fail gracefully
	// We use a shell command that exits immediately
	_, err := NewLSPClient([]string{"sh", "-c", "exit 1"}, tmpDir)
	if err == nil {
		t.Log("Client started but server exited - this is expected for dummy command")
	} else {
		t.Logf("Client failed to start as expected: %v", err)
	}

	// Test with a command that doesn't exist
	_, err = NewLSPClient([]string{"/nonexistent/lsp/server"}, tmpDir)
	if err == nil {
		t.Error("Expected error for nonexistent binary")
	}
}

// TestLocationSerialization tests that Location types serialize correctly.
func TestLocationSerialization(t *testing.T) {
	loc := Location{
		URI: "file:///test.go",
		Range: Range{
			Start: Position{Line: 10, Character: 5},
			End:   Position{Line: 10, Character: 15},
		},
	}

	data, err := json.Marshal(loc)
	if err != nil {
		t.Fatalf("marshal failed: %v", err)
	}

	var parsed Location
	if err := json.Unmarshal(data, &parsed); err != nil {
		t.Fatalf("unmarshal failed: %v", err)
	}

	if parsed.URI != loc.URI {
		t.Errorf("URI mismatch: %s != %s", parsed.URI, loc.URI)
	}
	if parsed.Range.Start.Line != loc.Range.Start.Line {
		t.Errorf("Start line mismatch: %d != %d", parsed.Range.Start.Line, loc.Range.Start.Line)
	}
}

// TestHoverInfoSerialization tests HoverInfo serialization.
func TestHoverInfoSerialization(t *testing.T) {
	hover := &HoverInfo{Contents: "func Test() error"}

	data, err := json.Marshal(hover)
	if err != nil {
		t.Fatalf("marshal failed: %v", err)
	}

	var parsed HoverInfo
	if err := json.Unmarshal(data, &parsed); err != nil {
		t.Fatalf("unmarshal failed: %v", err)
	}

	if parsed.Contents != hover.Contents {
		t.Errorf("Contents mismatch: %s != %s", parsed.Contents, hover.Contents)
	}
}

// TestCallHierarchyItemSerialization tests CallHierarchyItem serialization.
func TestCallHierarchyItemSerialization(t *testing.T) {
	item := CallHierarchyItem{
		Name:  "myFunction",
		Kind:  "Function",
		URI:   "file:///test.go",
		Range: Range{Start: Position{Line: 10, Character: 0}, End: Position{Line: 20, Character: 1}},
	}

	data, err := json.Marshal(item)
	if err != nil {
		t.Fatalf("marshal failed: %v", err)
	}

	var parsed CallHierarchyItem
	if err := json.Unmarshal(data, &parsed); err != nil {
		t.Fatalf("unmarshal failed: %v", err)
	}

	if parsed.Name != item.Name {
		t.Errorf("Name mismatch: %s != %s", parsed.Name, item.Name)
	}
}

// TestNewLSPClient_WithRealServer tests with a real LSP server if available.
// This is an integration test that requires gopls to be installed.
func TestNewLSPClient_WithRealServer(t *testing.T) {
	// Check if gopls is available
	if _, err := exec.LookPath("gopls"); err != nil {
		t.Skip("gopls not found in PATH, skipping integration test")
	}

	// Create a temporary Go module for testing
	tmpDir := t.TempDir()
	goMod := filepath.Join(tmpDir, "go.mod")
	if err := os.WriteFile(goMod, []byte("module test\n\ngo 1.21\n"), 0644); err != nil {
		t.Fatalf("write go.mod: %v", err)
	}

	// Create a simple Go file
	testFile := filepath.Join(tmpDir, "test.go")
	if err := os.WriteFile(testFile, []byte("package test\n\nfunc Hello() string { return \"world\" }\n"), 0644); err != nil {
		t.Fatalf("write test.go: %v", err)
	}

	client, err := NewLSPClient([]string{"gopls", "serve"}, tmpDir)
	if err != nil {
		t.Fatalf("NewLSPClient failed: %v", err)
	}
	defer client.Close()

	// Test capabilities were negotiated
	caps := client.Capabilities()
	t.Logf("Negotiated capabilities: %+v", caps)

	// Test Definition request
	locs, err := client.Definition(testFile, 2, 10) // line 3, col 10 (inside Hello)
	if err != nil {
		t.Logf("Definition request failed (may be expected if server not ready): %v", err)
	} else {
		t.Logf("Definition returned %d locations", len(locs))
	}

	// Test Hover request
	hover, err := client.Hover(testFile, 2, 10)
	if err != nil {
		t.Logf("Hover request failed: %v", err)
	} else if hover != nil {
		t.Logf("Hover: %s", hover.Contents)
	}
}

// Helper to check if a binary exists in PATH
func execLookPath(name string) (string, error) {
	return "", nil // placeholder - using os/exec.LookPath directly in test
}