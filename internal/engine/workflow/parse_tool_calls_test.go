package workflow

import (
	"encoding/json"
	"errors"
	"strings"
	"testing"
	"time"

	m31errors "github.com/eshanized/M31A/internal/core/errors"
)

// TestParseToolCalls_OversizedInput_ReturnsErrTooLarge verifies that
// parseToolCalls rejects inputs larger than MaxLLMResponseBytes (1 MB)
// with ErrToolInputTooLarge. Fix C-4 regression test.
func TestParseToolCalls_OversizedInput_ReturnsErrTooLarge(t *testing.T) {
	eng := &Engine{}

	// Construct a 10 MB string — well above the 1 MB cap.
	input := strings.Repeat("{", 10<<20)

	done := make(chan error, 1)
	go func() {
		_, err := eng.parseToolCalls(input)
		done <- err
	}()

	select {
	case err := <-done:
		if !errors.Is(err, m31errors.ErrToolInputTooLarge) {
			t.Fatalf("expected ErrToolInputTooLarge, got %v", err)
		}
	case <-time.After(100 * time.Millisecond):
		t.Fatal("parseToolCalls did not return within 100 ms for oversized input")
	}
}

// TestParseToolCalls_CapsToolCount verifies that parseToolCalls truncates
// results to maxToolsPerCall (16) entries. Fix C-4 regression test.
func TestParseToolCalls_CapsToolCount(t *testing.T) {
	eng := &Engine{}

	// Build a response with 20 valid tool calls.
	var calls strings.Builder
	for i := 0; i < 20; i++ {
		calls.WriteString(`{"name":"Bash","input":{"command":"echo `)
		calls.WriteString(strings.Repeat("x", 10))
		calls.WriteString(`"}}`)
	}

	result, err := eng.parseToolCalls(calls.String())
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if len(result) > maxToolsPerCall {
		t.Fatalf("expected at most %d tool calls, got %d", maxToolsPerCall, len(result))
	}
}

// TestExtractJSON_StripsComments verifies that extractJSONObject strips
// // line comments and /* */ block comments while preserving string literals.
// Fix H-5 regression test.
func TestExtractJSON_StripsComments(t *testing.T) {
	tests := []struct {
		name    string
		input   string
		wantKey string
		wantVal string
	}{
		{
			name: "line comment",
			input: `{
  // This is a hint from the model
  "name": "Bash",
  "input": {"command": "echo hello"}
}`,
			wantKey: "name",
			wantVal: "Bash",
		},
		{
			name: "block comment",
			input: `{
  /* multi-line
     hint */
  "name": "FileRead",
  "input": {"path": "/tmp/test"}
}`,
			wantKey: "name",
			wantVal: "FileRead",
		},
		{
			name: "mixed comments",
			input: `{
  // line hint
  "name": "Grep",
  /* block hint */
  "input": {"pattern": "foo"}
}`,
			wantKey: "name",
			wantVal: "Grep",
		},
		{
			name: "comment inside string literal preserved",
			input: `{
  "name": "Bash",
  "input": {"command": "echo // not a comment"}
}`,
			wantKey: "name",
			wantVal: "Bash",
		},
		{
			name: "block comment inside string literal preserved",
			input: `{
  "name": "Bash",
  "input": {"command": "echo /* not a comment */"}
}`,
			wantKey: "name",
			wantVal: "Bash",
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			result := extractJSONObject(tt.input)
			if result == "" {
				t.Fatal("extractJSONObject returned empty string")
			}

			var m map[string]any
			if err := json.Unmarshal([]byte(result), &m); err != nil {
				t.Fatalf("result is not valid JSON: %v\nraw: %s", err, result)
			}
			if got := m[tt.wantKey]; got != tt.wantVal {
				t.Errorf("want %s=%q, got %q", tt.wantKey, tt.wantVal, got)
			}
		})
	}
}

// TestExtractJSON_Negative_StringLiteralsNotStripped verifies that
// // and /* */ inside JSON string literals are NOT stripped.
func TestExtractJSON_Negative_StringLiteralsNotStripped(t *testing.T) {
	input := `{"cmd": "run // comment && echo /* block */"}`
	result := extractJSONObject(input)
	if result == "" {
		t.Fatal("extractJSONObject returned empty string")
	}

	var m map[string]any
	if err := json.Unmarshal([]byte(result), &m); err != nil {
		t.Fatalf("result is not valid JSON: %v", err)
	}
	expected := "run // comment && echo /* block */"
	if got := m["cmd"]; got != expected {
		t.Errorf("want cmd=%q, got %q", expected, got)
	}
}
