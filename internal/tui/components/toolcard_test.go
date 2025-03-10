package components

import (
	"encoding/json"
	"testing"

	"github.com/eshanized/M31A/internal/tui/theme"
	"github.com/eshanized/M31A/internal/types"
)

func TestNewToolCard_Bash(t *testing.T) {
	input := json.RawMessage(`{"command": "ls -la"}`)
	call := types.ToolCall{ID: "1", Name: "Bash", Input: input}
	tc := NewToolCard(call, nil, ToolRunning, theme.Dark())
	if tc == nil {
		t.Fatal("expected non-nil ToolCard")
	}
	if tc.toolName != "Bash" {
		t.Errorf("expected toolName Bash, got %q", tc.toolName)
	}
}

func TestNewToolCard_FileRead(t *testing.T) {
	input := json.RawMessage(`"test.txt"`)
	call := types.ToolCall{ID: "2", Name: "FileRead", Input: input}
	tc := NewToolCard(call, nil, ToolRunning, theme.Dark())
	if tc.toolName != "FileRead" {
		t.Errorf("expected FileRead, got %q", tc.toolName)
	}
}

func TestNewToolCard_FileWrite(t *testing.T) {
	input := json.RawMessage(`"test.txt"`)
	call := types.ToolCall{ID: "3", Name: "FileWrite", Input: input}
	tc := NewToolCard(call, nil, ToolRunning, theme.Dark())
	if tc.toolName != "FileWrite" {
		t.Errorf("expected FileWrite, got %q", tc.toolName)
	}
}

func TestNewToolCard_Glob(t *testing.T) {
	input := json.RawMessage(`"**/*.go"`)
	call := types.ToolCall{ID: "4", Name: "Glob", Input: input}
	tc := NewToolCard(call, nil, ToolRunning, theme.Dark())
	if tc.toolName != "Glob" {
		t.Errorf("expected Glob, got %q", tc.toolName)
	}
}

func TestNewToolCard_Grep(t *testing.T) {
	input := json.RawMessage(`"pattern"`)
	call := types.ToolCall{ID: "5", Name: "Grep", Input: input}
	tc := NewToolCard(call, nil, ToolRunning, theme.Dark())
	if tc.toolName != "Grep" {
		t.Errorf("expected Grep, got %q", tc.toolName)
	}
}

func TestToolCard_RenderRunning(t *testing.T) {
	input := json.RawMessage(`"ls"`)
	call := types.ToolCall{ID: "6", Name: "Bash", Input: input}
	tc := NewToolCard(call, nil, ToolRunning, theme.Dark())
	result := tc.Render(80)
	if result == "" {
		t.Error("expected non-empty render for running state")
	}
}

func TestToolCard_RenderSuccess(t *testing.T) {
	input := json.RawMessage(`"echo hello"`)
	call := types.ToolCall{ID: "7", Name: "Bash", Input: input}
	result := &types.ToolResult{
		ToolCallID: "7",
		Output:     "hello",
		DurationMs: 120,
		Truncated:  false,
	}
	tc := NewToolCard(call, result, ToolSuccess, theme.Dark())
	rendered := tc.Render(80)
	if rendered == "" {
		t.Error("expected non-empty render")
	}
}

func TestToolCard_RenderError(t *testing.T) {
	input := json.RawMessage(`"badcommand"`)
	call := types.ToolCall{ID: "8", Name: "Bash", Input: input}
	result := &types.ToolResult{
		ToolCallID: "8",
		Output:     "",
		Error:      "command not found",
		DurationMs: 50,
	}
	tc := NewToolCard(call, result, ToolError, theme.Dark())
	rendered := tc.Render(80)
	if rendered == "" {
		t.Error("expected non-empty render for error")
	}
}

func TestToolCard_AutoCollapse_LongOutput(t *testing.T) {
	longOutput := ""
	for i := 0; i < 25; i++ {
		longOutput += "line of output\n"
	}
	input := json.RawMessage(`"cmd"`)
	call := types.ToolCall{ID: "9", Name: "Bash", Input: input}
	result := &types.ToolResult{
		ToolCallID: "9",
		Output:     longOutput,
		DurationMs: 100,
	}
	tc := NewToolCard(call, result, ToolSuccess, theme.Dark())
	if !tc.IsCollapsed() {
		t.Error("expected auto-collapse for >20 lines")
	}
}

func TestToolCard_AutoCollapse_BinaryContent(t *testing.T) {
	binaryOutput := "\x00\x01\x02\x00test\x00content"
	input := json.RawMessage(`"cat binary"`)
	call := types.ToolCall{ID: "10", Name: "Bash", Input: input}
	result := &types.ToolResult{
		ToolCallID: "10",
		Output:     binaryOutput,
		DurationMs: 50,
	}
	tc := NewToolCard(call, result, ToolSuccess, theme.Dark())
	if !tc.IsCollapsed() {
		t.Error("expected auto-collapse for binary content")
	}
}

func TestToolCard_AutoCollapse_TruncatedOutput(t *testing.T) {
	hugeOutput := string(make([]byte, types.MaxToolOutputChars+100))
	for i := range hugeOutput {
		hugeOutput = hugeOutput[:i] + "x" + hugeOutput[i+1:]
	}
	input := json.RawMessage(`"cat large"`)
	call := types.ToolCall{ID: "11", Name: "Bash", Input: input}
	result := &types.ToolResult{
		ToolCallID: "11",
		Output:     hugeOutput,
		DurationMs: 200,
		Truncated:  true,
	}
	tc := NewToolCard(call, result, ToolSuccess, theme.Dark())
	if tc.truncated != true {
		t.Error("expected truncated flag to be set")
	}
}

func TestToolCard_Toggle(t *testing.T) {
	input := json.RawMessage(`"cmd"`)
	call := types.ToolCall{ID: "12", Name: "Bash", Input: input}
	tc := NewToolCard(call, nil, ToolRunning, theme.Dark())
	initial := tc.IsCollapsed()
	tc.Toggle()
	if tc.IsCollapsed() == initial {
		t.Error("expected state change after toggle")
	}
}

func TestToolCard_HeaderColors(t *testing.T) {
	tools := []string{"Bash", "FileRead", "FileWrite", "Glob", "Grep"}
	dt := theme.Dark()
	for _, name := range tools {
		input := json.RawMessage(`"test"`)
		call := types.ToolCall{ID: name, Name: name, Input: input}
		tc := NewToolCard(call, nil, ToolRunning, dt)
		if tc.toolName != name {
			t.Errorf("expected toolName %q, got %q", name, tc.toolName)
		}
		if _, ok := dt.ToolLabel[name]; !ok {
			t.Errorf("missing ToolLabel for %q", name)
		}
	}
}
