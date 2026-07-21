package streaming

import (
	"encoding/json"
	"strings"
	"testing"

	"github.com/eshanized/M31A/internal/core/types"
)

func TestBuildAgentToolCalls_Empty(t *testing.T) {
	t.Parallel()
	result := buildAgentToolCalls(make(map[int]*agentToolCallAcc))
	if result != nil {
		t.Errorf("expected nil, got %v", result)
	}
}

func TestBuildAgentToolCalls_Single(t *testing.T) {
	t.Parallel()
	accMap := map[int]*agentToolCallAcc{
		0: {ID: "call_1", Name: "Bash", Args: strings.Builder{}, Index: 0},
	}
	accMap[0].Args.WriteString(`{"command":"ls"}`)
	result := buildAgentToolCalls(accMap)
	if len(result) != 1 {
		t.Fatalf("expected 1 tool call, got %d", len(result))
	}
	if result[0].ID != "call_1" {
		t.Errorf("ID = %q, want call_1", result[0].ID)
	}
	if result[0].Name != "Bash" {
		t.Errorf("Name = %q, want Bash", result[0].Name)
	}
}

func TestBuildAgentToolCalls_SortedByIndex(t *testing.T) {
	t.Parallel()
	accMap := map[int]*agentToolCallAcc{
		2: {ID: "c2", Name: "Grep", Args: strings.Builder{}, Index: 2},
		0: {ID: "c0", Name: "Bash", Args: strings.Builder{}, Index: 0},
		1: {ID: "c1", Name: "FileRead", Args: strings.Builder{}, Index: 1},
	}
	result := buildAgentToolCalls(accMap)
	if len(result) != 3 {
		t.Fatalf("expected 3, got %d", len(result))
	}
	if result[0].Name != "Bash" || result[1].Name != "FileRead" || result[2].Name != "Grep" {
		t.Errorf("wrong order: %v", result)
	}
}

func TestBuildAgentToolCalls_MissingID_FallbackToName(t *testing.T) {
	t.Parallel()
	accMap := map[int]*agentToolCallAcc{
		0: {ID: "", Name: "Bash", Args: strings.Builder{}, Index: 0},
	}
	result := buildAgentToolCalls(accMap)
	if len(result) != 1 {
		t.Fatalf("expected 1, got %d", len(result))
	}
	if result[0].ID != "Bash" {
		t.Errorf("ID fallback = %q, want Bash", result[0].ID)
	}
}

func TestBuildAgentToolCalls_EmptyArgs_DefaultJSON(t *testing.T) {
	t.Parallel()
	accMap := map[int]*agentToolCallAcc{
		0: {ID: "c1", Name: "Test", Args: strings.Builder{}, Index: 0},
	}
	result := buildAgentToolCalls(accMap)
	if len(result) != 1 {
		t.Fatalf("expected 1, got %d", len(result))
	}
	if string(result[0].Input) != "{}" {
		t.Errorf("empty args should default to {}, got %s", string(result[0].Input))
	}
}

func TestBuildAgentToolCalls_ValidJSONArgs(t *testing.T) {
	t.Parallel()
	var args strings.Builder
	args.WriteString(`{"key":"value"}`)
	accMap := map[int]*agentToolCallAcc{
		0: {ID: "c1", Name: "Test", Args: args, Index: 0},
	}
	result := buildAgentToolCalls(accMap)
	if len(result) != 1 {
		t.Fatalf("expected 1, got %d", len(result))
	}
	var m map[string]string
	if err := json.Unmarshal(result[0].Input, &m); err != nil {
		t.Fatalf("invalid JSON in Input: %v", err)
	}
	if m["key"] != "value" {
		t.Errorf("Input key = %q, want value", m["key"])
	}
}

func TestExtractAgentJSONObject_Valid(t *testing.T) {
	t.Parallel()
	tests := []struct {
		input string
		want  string
	}{
		{`{"a":1}`, `{"a":1}`},
		{`{"a":{"b":2}}`, `{"a":{"b":2}}`},
		{`{}  extra`, `{}`},
		{`not json`, ""},
		{``, ""},
		{`[1,2,3]`, "[1,2,3]"},
	}
	for _, tt := range tests {
		got := extractAgentJSONObject(tt.input)
		if got != tt.want {
			t.Errorf("extractAgentJSONObject(%q) = %q, want %q", tt.input, got, tt.want)
		}
	}
}

func TestExtractAgentJSONObject_NestedBraces(t *testing.T) {
	t.Parallel()
	got := extractAgentJSONObject(`{"a":{"b":{"c":3}}}`)
	if got != `{"a":{"b":{"c":3}}}` {
		t.Errorf("got %q", got)
	}
}

func TestParseTextToolCalls_NoToolCalls(t *testing.T) {
	t.Parallel()
	result := parseTextToolCalls("hello world, no tool calls here")
	if len(result) != 0 {
		t.Errorf("expected 0, got %d", len(result))
	}
}

func TestParseTextToolCalls_EmptyString(t *testing.T) {
	t.Parallel()
	result := parseTextToolCalls("")
	if len(result) != 0 {
		t.Errorf("expected 0, got %d", len(result))
	}
}

func TestParseTextToolCalls_ValidToolCall(t *testing.T) {
	t.Parallel()
	input := `some text {"name":"Bash","input":{"command":"ls"}} more text`
	result := parseTextToolCalls(input)
	if len(result) != 1 {
		t.Fatalf("expected 1, got %d", len(result))
	}
	if result[0].Name != "Bash" {
		t.Errorf("Name = %q, want Bash", result[0].Name)
	}
}

func TestParseTextToolCalls_ToolField(t *testing.T) {
	t.Parallel()
	input := `{"tool":"FileRead","params":{"path":"test.go"}}`
	result := parseTextToolCalls(input)
	if len(result) != 1 {
		t.Fatalf("expected 1, got %d", len(result))
	}
	if result[0].Name != "FileRead" {
		t.Errorf("Name = %q, want FileRead", result[0].Name)
	}
}

func TestParseTextToolCalls_MultipleToolCalls(t *testing.T) {
	t.Parallel()
	input := `{"name":"Bash","input":{"cmd":"a"}} then {"name":"Grep","input":{"pattern":"x"}}`
	result := parseTextToolCalls(input)
	if len(result) != 2 {
		t.Fatalf("expected 2, got %d", len(result))
	}
}

func TestParseTextToolCalls_NoNameOrTool(t *testing.T) {
	t.Parallel()
	input := `{"notAToolCall":true}`
	result := parseTextToolCalls(input)
	if len(result) != 0 {
		t.Errorf("expected 0, got %d", len(result))
	}
}

func TestParseTextToolCalls_MalformedJSON(t *testing.T) {
	t.Parallel()
	input := `{not valid json}`
	result := parseTextToolCalls(input)
	if len(result) != 0 {
		t.Errorf("expected 0, got %d", len(result))
	}
}

func TestParseTextToolCalls_EmptyNameAndTool(t *testing.T) {
	t.Parallel()
	input := `{"name":"","tool":"","input":{}}`
	result := parseTextToolCalls(input)
	if len(result) != 0 {
		t.Errorf("expected 0, got %d", len(result))
	}
}

func TestParseTextToolCalls_IDFormat(t *testing.T) {
	t.Parallel()
	input := `{"name":"Bash","input":{}}`
	result := parseTextToolCalls(input)
	if len(result) != 1 {
		t.Fatalf("expected 1, got %d", len(result))
	}
	if !strings.HasPrefix(result[0].ID, "text_Bash_") {
		t.Errorf("ID = %q, want prefix text_Bash_", result[0].ID)
	}
}

func TestPruneOldToolResults_NoToolMessages(t *testing.T) {
	t.Parallel()
	messages := []types.Message{
		{Role: "user", Content: "hello"},
		{Role: "assistant", Content: "hi"},
	}
	pruneOldToolResults(messages, nil)
	if messages[0].Content != "hello" {
		t.Error("non-tool messages should not be modified")
	}
}

func TestPruneOldToolResults_FewerThanKeep(t *testing.T) {
	t.Parallel()
	messages := []types.Message{
		{Role: "tool", Content: strings.Repeat("x", 1000)},
		{Role: "tool", Content: strings.Repeat("y", 1000)},
	}
	pruneOldToolResults(messages, nil)
	for _, m := range messages {
		if len(m.Content) != 1000 {
			t.Error("fewer than 3 tool messages should not be pruned")
		}
	}
}

func TestPruneOldToolResults_ExactlyThree(t *testing.T) {
	t.Parallel()
	messages := []types.Message{
		{Role: "tool", Content: strings.Repeat("x", 1000)},
		{Role: "tool", Content: strings.Repeat("y", 1000)},
		{Role: "tool", Content: strings.Repeat("z", 1000)},
	}
	pruneOldToolResults(messages, nil)
	for _, m := range messages {
		if len(m.Content) != 1000 {
			t.Error("exactly 3 tool messages should not be pruned")
		}
	}
}

func TestPruneOldToolResults_MoreThanThree(t *testing.T) {
	t.Parallel()
	longContent := strings.Repeat("x", 1000)
	messages := []types.Message{
		{Role: "tool", Content: longContent},
		{Role: "tool", Content: longContent},
		{Role: "tool", Content: longContent},
		{Role: "tool", Content: longContent},
		{Role: "tool", Content: longContent},
	}
	pruneOldToolResults(messages, nil)
	// First two should be pruned (indices 0,1), last 3 kept
	for i := 0; i < 2; i++ {
		if !strings.HasSuffix(messages[i].Content, "pruned to save context]") {
			t.Errorf("message %d should be pruned", i)
		}
		if !messages[i].SkipForLLM {
			t.Errorf("message %d should have SkipForLLM=true", i)
		}
	}
	for i := 2; i < 5; i++ {
		if len(messages[i].Content) != 1000 {
			t.Errorf("message %d should not be pruned", i)
		}
		if messages[i].SkipForLLM {
			t.Errorf("message %d should have SkipForLLM=false", i)
		}
	}
}

func TestPruneOldToolResults_ShortContent(t *testing.T) {
	t.Parallel()
	messages := []types.Message{
		{Role: "tool", Content: "short"},
		{Role: "tool", Content: "short"},
		{Role: "tool", Content: "short"},
		{Role: "tool", Content: "short"},
	}
	pruneOldToolResults(messages, nil)
	// Only first message should be pruned (4 messages - 3 keep = 1 pruned)
	if messages[0].Content != "[tool result pruned to save context]" {
		t.Errorf("message 0 should be pruned, got %q", messages[0].Content)
	}
	// Last 3 should be untouched
	for i := 1; i < 4; i++ {
		if messages[i].Content != "short" {
			t.Errorf("message %d should not be pruned, got %q", i, messages[i].Content)
		}
	}
}

func TestPruneOldToolResults_ExactBoundary(t *testing.T) {
	t.Parallel()
	content500 := strings.Repeat("x", 500)
	content501 := strings.Repeat("x", 501)
	messages := []types.Message{
		{Role: "tool", Content: content501},
		{Role: "tool", Content: content501},
		{Role: "tool", Content: content500},
		{Role: "tool", Content: content500},
	}
	pruneOldToolResults(messages, nil)
	// Only first message should be pruned (4 messages - 3 keep = 1 pruned)
	if messages[0].Content != "[tool result pruned to save context]" {
		t.Error("first message should be pruned")
	}
	// Last 3 should be untouched
	if messages[1].Content != content501 {
		t.Error("second message should not be pruned")
	}
	if messages[2].Content != content500 {
		t.Error("third message should not be pruned")
	}
	if messages[3].Content != content500 {
		t.Error("fourth message should not be pruned")
	}
}

func TestBuildAgentMessages_Empty(t *testing.T) {
	t.Parallel()
	result := BuildAgentMessages(nil, "hello")
	if len(result) != 1 {
		t.Fatalf("expected 1, got %d", len(result))
	}
	if result[0].Role != "user" {
		t.Errorf("Role = %q, want user", result[0].Role)
	}
	if result[0].Content != "hello" {
		t.Errorf("Content = %q, want hello", result[0].Content)
	}
}

func TestBuildAgentMessages_SkipForLLM(t *testing.T) {
	t.Parallel()
	msgs := []types.Message{
		{Role: "user", Content: "hi", SkipForLLM: true},
		{Role: "assistant", Content: "hello"},
	}
	result := BuildAgentMessages(msgs, "new input")
	// SkipForLLM message should be filtered, assistant kept, new user msg added
	for _, m := range result {
		if m.SkipForLLM {
			t.Error("SkipForLLM messages should be filtered")
		}
	}
}

func TestBuildAgentMessages_ReplaceLastUser(t *testing.T) {
	t.Parallel()
	msgs := []types.Message{
		{Role: "user", Content: "old"},
		{Role: "assistant", Content: "response"},
		{Role: "user", Content: "stale"},
	}
	result := BuildAgentMessages(msgs, "fresh")
	// Last user message should be replaced with new input
	lastUser := result[len(result)-1]
	if lastUser.Content != "fresh" {
		t.Errorf("last user Content = %q, want fresh", lastUser.Content)
	}
}

func TestBuildAgentMessages_AppendIfLastNotUser(t *testing.T) {
	t.Parallel()
	msgs := []types.Message{
		{Role: "user", Content: "old"},
		{Role: "assistant", Content: "response"},
	}
	result := BuildAgentMessages(msgs, "new")
	lastMsg := result[len(result)-1]
	if lastMsg.Content != "new" {
		t.Errorf("last Content = %q, want new", lastMsg.Content)
	}
	if lastMsg.Role != "user" {
		t.Errorf("last Role = %q, want user", lastMsg.Role)
	}
}

func TestBuildAgentMessages_OnlySkipForLLM(t *testing.T) {
	t.Parallel()
	msgs := []types.Message{
		{Role: "user", Content: "skip1", SkipForLLM: true},
		{Role: "assistant", Content: "skip2", SkipForLLM: true},
	}
	result := BuildAgentMessages(msgs, "actual")
	if len(result) != 1 {
		t.Fatalf("expected 1, got %d", len(result))
	}
	if result[0].Content != "actual" {
		t.Errorf("Content = %q, want actual", result[0].Content)
	}
}
