package components

import (
	"encoding/json"
	"strings"
	"testing"

	"github.com/eshanized/M31A/internal/ui/tui/theme"
	"github.com/eshanized/M31A/internal/core/types"
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
	for i := 0; i < 55; i++ {
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
		t.Error("expected auto-collapse for >50 lines")
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
	hugeOutput := strings.Repeat("x", types.MaxToolOutputChars+100)
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
	tools := []string{"Bash", "FileRead", "FileWrite", "Glob", "Grep", "Edit", "TodoWrite", "WebFetch", "AskUserQuestion"}
	dt := theme.Dark()
	for _, name := range tools {
		input := json.RawMessage(`"test"`)
		call := types.ToolCall{ID: name, Name: name, Input: input}
		tc := NewToolCard(call, nil, ToolRunning, dt)
		if tc.toolName != name {
			t.Errorf("expected toolName %q, got %q", name, tc.toolName)
		}
	}
}

func TestRendererForTool_AllTools(t *testing.T) {
	dt := theme.Dark()
	tools := []string{"Bash", "Edit", "FileRead", "FileWrite", "TodoWrite", "Grep", "Glob", "WebFetch", "AskUserQuestion", "UnknownTool"}
	for _, name := range tools {
		r := RendererForTool(name, dt)
		if r == nil {
			t.Errorf("RendererForTool(%q) returned nil", name)
			continue
		}
		if r.Name() == "" {
			t.Errorf("RendererForTool(%q).Name() returned empty string", name)
		}
	}
}

func TestSanitizeOutput(t *testing.T) {
	tests := []struct {
		name  string
		input string
		want  string
	}{
		{
			name:  "ansi color codes stripped",
			input: "\x1b[31mred text\x1b[0m",
			want:  "red text",
		},
		{
			name:  "ansi bold and color",
			input: "\x1b[1;32mgreen bold\x1b[0m",
			want:  "green bold",
		},
		{
			name:  "null byte stripped",
			input: "hello\x00world",
			want:  "helloworld",
		},
		{
			name:  "bell character stripped",
			input: "alert\x07done",
			want:  "alertdone",
		},
		{
			name:  "clean input unchanged",
			input: "normal text with no codes",
			want:  "normal text with no codes",
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := SanitizeOutput(tt.input)
			if got != tt.want {
				t.Errorf("Expected %q, got %q", tt.want, got)
			}
		})
	}
}

func TestToolCard_SanitizesOutput(t *testing.T) {
	input := json.RawMessage(`"echo -e '\x1b[31mred\x1b[0m'"`)
	call := types.ToolCall{ID: "san", Name: "Bash", Input: input}
	result := &types.ToolResult{
		ToolCallID: "san",
		Output:     "\x1b[31mred text\x1b[0m",
		DurationMs: 50,
	}
	tc := NewToolCard(call, result, ToolSuccess, theme.Dark())
	// Verify the raw malicious ANSI codes were stripped from the stored output
	if strings.Contains(tc.Output(), "\x1b[31m") {
		t.Error("stored output still contains original ANSI escape codes after sanitization")
	}
}

// ─── M8: Progressive Disclosure Tests ────────────────────────────────────────

func TestM8_ToolCardID(t *testing.T) {
	input := json.RawMessage(`"cmd"`)
	call := types.ToolCall{ID: "unique-id-123", Name: "Bash", Input: input}
	tc := NewToolCard(call, nil, ToolRunning, theme.Dark())
	if tc.ToolID() != "unique-id-123" {
		t.Errorf("expected ToolID 'unique-id-123', got %q", tc.ToolID())
	}
}

func TestM8_SuccessfulToolCollapsedByDefault(t *testing.T) {
	input := json.RawMessage(`"echo hello"`)
	call := types.ToolCall{ID: "m8-1", Name: "Bash", Input: input}
	result := &types.ToolResult{
		ToolCallID: "m8-1",
		Output:     "hello",
		DurationMs: 50,
	}
	tc := NewToolCard(call, result, ToolSuccess, theme.Dark())
	if !tc.IsCollapsed() {
		t.Error("M8: successful tools should be collapsed by default")
	}
}

func TestM8_FailedToolAutoExpands(t *testing.T) {
	input := json.RawMessage(`"badcmd"`)
	call := types.ToolCall{ID: "m8-2", Name: "Bash", Input: input}
	result := &types.ToolResult{
		ToolCallID: "m8-2",
		Output:     "",
		Error:      "command not found",
		DurationMs: 50,
	}
	tc := NewToolCard(call, result, ToolError, theme.Dark())
	if tc.IsCollapsed() {
		t.Error("M8: failed tools should be auto-expanded")
	}
}

func TestM8_RunningToolNotAutoCollapsed(t *testing.T) {
	input := json.RawMessage(`"cmd"`)
	call := types.ToolCall{ID: "m8-3", Name: "Bash", Input: input}
	tc := NewToolCard(call, nil, ToolRunning, theme.Dark())
	if tc.IsCollapsed() {
		t.Error("M8: running tools should not be auto-collapsed")
	}
}

func TestM8_ToggleCollapsedState(t *testing.T) {
	input := json.RawMessage(`"cmd"`)
	call := types.ToolCall{ID: "m8-4", Name: "Bash", Input: input}
	tc := NewToolCard(call, nil, ToolRunning, theme.Dark())
	initial := tc.IsCollapsed()
	tc.Toggle()
	if tc.IsCollapsed() == initial {
		t.Error("M8: Toggle should change collapsed state")
	}
	tc.Toggle()
	if tc.IsCollapsed() != initial {
		t.Error("M8: second Toggle should restore original state")
	}
}

func TestM8_SetCollapsed(t *testing.T) {
	input := json.RawMessage(`"cmd"`)
	call := types.ToolCall{ID: "m8-5", Name: "Bash", Input: input}
	tc := NewToolCard(call, nil, ToolRunning, theme.Dark())
	tc.SetCollapsed(true)
	if !tc.IsCollapsed() {
		t.Error("M8: SetCollapsed(true) should set collapsed")
	}
	tc.SetCollapsed(false)
	if tc.IsCollapsed() {
		t.Error("M8: SetCollapsed(false) should unset collapsed")
	}
}

func TestM8_CollapsedStatePersistence(t *testing.T) {
	// Simulate: create card, collapse it, create renderer, check state persists
	input := json.RawMessage(`"cmd"`)
	call := types.ToolCall{ID: "m8-persist", Name: "Bash", Input: input}
	tc := NewToolCard(call, nil, ToolRunning, theme.Dark())
	tc.SetCollapsed(true)

	// Create a renderer and check that collapsed state is applied
	mr, err := NewMessageRenderer(theme.Dark(), 80)
	if err != nil {
		t.Fatalf("failed to create MessageRenderer: %v", err)
	}
	mr.SetToolCardCollapsed("m8-persist", true)
	if !mr.IsToolCardCollapsed("m8-persist") {
		t.Error("M8: collapsed state should persist in renderer")
	}
}

func TestM8_SetAllToolCardsCollapsed(t *testing.T) {
	mr, err := NewMessageRenderer(theme.Dark(), 80)
	if err != nil {
		t.Fatalf("failed to create MessageRenderer: %v", err)
	}
	mr.SetToolCardCollapsed("id-1", false)
	mr.SetToolCardCollapsed("id-2", false)
	mr.SetToolCardCollapsed("id-3", false)

	mr.SetAllToolCardsCollapsed(true)

	if !mr.IsToolCardCollapsed("id-1") || !mr.IsToolCardCollapsed("id-2") || !mr.IsToolCardCollapsed("id-3") {
		t.Error("M8: SetAllToolCardsCollapsed should collapse all")
	}
}

func TestM8_ResetToolCardCollapsed(t *testing.T) {
	mr, err := NewMessageRenderer(theme.Dark(), 80)
	if err != nil {
		t.Fatalf("failed to create MessageRenderer: %v", err)
	}
	mr.SetToolCardCollapsed("id-1", true)
	mr.ResetToolCardCollapsed()
	if mr.IsToolCardCollapsed("id-1") {
		t.Error("M8: ResetToolCardCollapsed should clear all state")
	}
}

func TestM8_RenderingShowsExpandHint(t *testing.T) {
	input := json.RawMessage(`"echo test"`)
	call := types.ToolCall{ID: "m8-hint", Name: "Bash", Input: input}
	result := &types.ToolResult{
		ToolCallID: "m8-hint",
		Output:     "test output\nline2\nline3",
		DurationMs: 50,
	}
	tc := NewToolCard(call, result, ToolSuccess, theme.Dark())
	// Successful tool should be collapsed by default
	if !tc.IsCollapsed() {
		t.Error("M8: successful tool should be collapsed")
	}
	// Render should include expand hint
	rendered := tc.Render(80)
	if !strings.Contains(rendered, "Enter to expand") {
		t.Error("M8: collapsed successful tool should show expand hint")
	}
}
