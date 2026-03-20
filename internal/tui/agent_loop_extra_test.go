package tui

import (
	"encoding/json"
	"strings"
	"testing"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/eshanized/M31A/internal/types"
)

// --- agent_loop.go tests ---

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
		0: {id: "call_1", name: "Bash", args: strings.Builder{}, index: 0},
	}
	accMap[0].args.WriteString(`{"command":"ls"}`)
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
		2: {id: "c2", name: "Grep", args: strings.Builder{}, index: 2},
		0: {id: "c0", name: "Bash", args: strings.Builder{}, index: 0},
		1: {id: "c1", name: "FileRead", args: strings.Builder{}, index: 1},
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
		0: {id: "", name: "Bash", args: strings.Builder{}, index: 0},
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
		0: {id: "c1", name: "Test", args: strings.Builder{}, index: 0},
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
		0: {id: "c1", name: "Test", args: args, index: 0},
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
		if !strings.HasSuffix(messages[i].Content, "...[truncated]") {
			t.Errorf("message %d should be pruned", i)
		}
	}
	for i := 2; i < 5; i++ {
		if len(messages[i].Content) != 1000 {
			t.Errorf("message %d should not be pruned", i)
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
	// Short content (<500 chars) should not be truncated
	for i := 0; i < 2; i++ {
		if messages[i].Content != "short" {
			t.Errorf("short message %d should not be modified, got %q", i, messages[i].Content)
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
	// First two (501 chars) should be truncated to 500 + suffix
	if !strings.HasSuffix(messages[0].Content, "...[truncated]") {
		t.Error("501-char message should be truncated")
	}
	// Last two (500 chars) should be untouched
	if messages[2].Content != content500 {
		t.Error("500-char message should not be truncated")
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

// --- bisect_model.go tests ---

func testCommits(n int) []bisectCommit {
	commits := make([]bisectCommit, n)
	for i := range commits {
		commits[i] = bisectCommit{
			Hash:    strings.Repeat("a", 40),
			Message: "commit message",
			Status:  "pending",
		}
	}
	return commits
}

func TestSetCommits(t *testing.T) {
	t.Parallel()
	bm := NewBisectModel(testTheme(), 80, 24)
	commits := testCommits(10)
	bm.SetCommits(commits)
	if bm.total != 10 {
		t.Errorf("total = %d, want 10", bm.total)
	}
	if bm.current != 5 {
		t.Errorf("current = %d, want 5 (midpoint)", bm.current)
	}
	if bm.status != "testing" {
		t.Errorf("status = %q, want testing", bm.status)
	}
	if bm.commits[5].Status != "testing" {
		t.Errorf("midpoint status = %q, want testing", bm.commits[5].Status)
	}
}

func TestSetCommits_SingleCommit(t *testing.T) {
	t.Parallel()
	bm := NewBisectModel(testTheme(), 80, 24)
	bm.SetCommits(testCommits(1))
	if bm.current != 0 {
		t.Errorf("current = %d, want 0", bm.current)
	}
	if bm.commits[0].Status != "testing" {
		t.Errorf("single commit status = %q, want testing", bm.commits[0].Status)
	}
}

func TestBisectModel_MarkCurrent(t *testing.T) {
	t.Parallel()
	bm := NewBisectModel(testTheme(), 80, 24)
	bm.SetCommits(testCommits(5))
	bm.markCurrent("good")
	if bm.commits[2].Status != "good" {
		t.Errorf("commit 2 status = %q, want good", bm.commits[2].Status)
	}
}

func TestBisectModel_MarkCurrent_OutOfBounds(t *testing.T) {
	t.Parallel()
	bm := NewBisectModel(testTheme(), 80, 24)
	bm.SetCommits(testCommits(3))
	bm.current = 10 // out of bounds
	bm.markCurrent("good") // should not panic
}

func TestBisectModel_Advance_AllPending(t *testing.T) {
	t.Parallel()
	bm := NewBisectModel(testTheme(), 80, 24)
	bm.SetCommits(testCommits(5))
	// All pending, advance should go to midpoint
	bm.advance()
	if bm.current != 2 {
		t.Errorf("advance from all-pending: current = %d, want 2", bm.current)
	}
}

func TestBisectModel_Advance_FirstGood(t *testing.T) {
	t.Parallel()
	bm := NewBisectModel(testTheme(), 80, 24)
	commits := testCommits(5)
	bm.SetCommits(commits)
	bm.commits[0].Status = "good"
	bm.advance()
	// low=0 (first good at 0), high=4, midpoint = 0 + (4-0)/2 = 2
	if bm.current != 2 {
		t.Errorf("advance with first good: current = %d, want 2", bm.current)
	}
}

func TestBisectModel_Advance_Converge(t *testing.T) {
	t.Parallel()
	bm := NewBisectModel(testTheme(), 80, 24)
	commits := testCommits(5)
	bm.SetCommits(commits)
	// Simulate: good at 0, bad at 4
	bm.commits[0].Status = "good"
	bm.commits[4].Status = "bad"
	bm.advance()
	// low=0, high=4, current=2
	if bm.current != 2 {
		t.Errorf("converge: current = %d, want 2", bm.current)
	}
	// Now mark good at 2
	bm.commits[2].Status = "good"
	bm.advance()
	// low=2, high=4, current=3
	if bm.current != 3 {
		t.Errorf("converge step 2: current = %d, want 3", bm.current)
	}
	// After marking 3 as bad, low=2, high=3 → low >= high-1 → done
	bm.commits[3].Status = "bad"
	bm.advance()
	if bm.status != "done" {
		t.Errorf("status = %q, want done", bm.status)
	}
}

func TestBisectModel_Reset(t *testing.T) {
	t.Parallel()
	bm := NewBisectModel(testTheme(), 80, 24)
	bm.SetCommits(testCommits(5))
	bm.markCurrent("good")
	bm.markCurrent("bad")
	bm.reset()
	// Midpoint gets "testing", all others get "pending"
	for i, c := range bm.commits {
		if i == bm.current {
			if c.Status != "testing" {
				t.Errorf("commit %d (midpoint) status = %q, want testing", i, c.Status)
			}
		} else {
			if c.Status != "pending" {
				t.Errorf("commit %d status = %q, want pending after reset", i, c.Status)
			}
		}
	}
	if bm.status != "testing" {
		t.Errorf("status = %q, want testing after reset", bm.status)
	}
}

func TestBisectModel_UpdateWindowSize(t *testing.T) {
	t.Parallel()
	bm := NewBisectModel(testTheme(), 80, 24)
	model, _ := bm.Update(tea.WindowSizeMsg{Width: 120, Height: 40})
	bm2 := model.(*BisectModel)
	if bm2.width != 120 || bm2.height != 40 {
		t.Errorf("dimensions = %dx%d, want 120x40", bm2.width, bm2.height)
	}
}

func TestBisectModel_UpdateKey_Good(t *testing.T) {
	t.Parallel()
	bm := NewBisectModel(testTheme(), 80, 24)
	bm.SetCommits(testCommits(5))
	model, _ := bm.Update(tea.KeyMsg{Type: tea.KeyRunes, Runes: []rune{'g'}})
	bm2 := model.(*BisectModel)
	if bm2.commits[2].Status != "good" {
		t.Errorf("g key: status = %q, want good", bm2.commits[2].Status)
	}
}

func TestBisectModel_UpdateKey_Bad(t *testing.T) {
	t.Parallel()
	bm := NewBisectModel(testTheme(), 80, 24)
	bm.SetCommits(testCommits(5))
	model, _ := bm.Update(tea.KeyMsg{Type: tea.KeyRunes, Runes: []rune{'b'}})
	bm2 := model.(*BisectModel)
	if bm2.commits[2].Status != "bad" {
		t.Errorf("b key: status = %q, want bad", bm2.commits[2].Status)
	}
}

func TestBisectModel_UpdateKey_Skip(t *testing.T) {
	t.Parallel()
	bm := NewBisectModel(testTheme(), 80, 24)
	bm.SetCommits(testCommits(5))
	model, _ := bm.Update(tea.KeyMsg{Type: tea.KeyRunes, Runes: []rune{'s'}})
	bm2 := model.(*BisectModel)
	if bm2.commits[2].Status != "skip" {
		t.Errorf("s key: status = %q, want skip", bm2.commits[2].Status)
	}
}

func TestBisectModel_UpdateKey_Reset(t *testing.T) {
	t.Parallel()
	bm := NewBisectModel(testTheme(), 80, 24)
	bm.SetCommits(testCommits(5))
	bm.markCurrent("good")
	model, _ := bm.Update(tea.KeyMsg{Type: tea.KeyRunes, Runes: []rune{'r'}})
	bm2 := model.(*BisectModel)
	if bm2.commits[2].Status != "testing" {
		t.Errorf("r key: status = %q, want testing", bm2.commits[2].Status)
	}
}

func TestBisectModel_UpdateKey_Esc(t *testing.T) {
	t.Parallel()
	bm := NewBisectModel(testTheme(), 80, 24)
	_, cmd := bm.Update(tea.KeyMsg{Type: tea.KeyEscape})
	if cmd == nil {
		t.Error("esc key should return a command")
	}
}

func TestBisectModel_View_Empty(t *testing.T) {
	t.Parallel()
	bm := NewBisectModel(testTheme(), 80, 24)
	view := bm.View()
	if !strings.Contains(view, "No bisect range") {
		t.Error("empty view should contain 'No bisect range'")
	}
}

func TestBisectModel_View_WithCommits(t *testing.T) {
	t.Parallel()
	bm := NewBisectModel(testTheme(), 80, 24)
	bm.SetCommits(testCommits(5))
	view := bm.View()
	if !strings.Contains(view, "Git Bisect") {
		t.Error("view should contain 'Git Bisect'")
	}
	if !strings.Contains(view, "[g] Good") {
		t.Error("view should contain footer hints")
	}
}

func TestBisectModel_View_NarrowWidth(t *testing.T) {
	t.Parallel()
	bm := NewBisectModel(testTheme(), 10, 24) // very narrow
	bm.SetCommits(testCommits(5))
	view := bm.View()
	if view == "" {
		t.Error("narrow view should return non-empty string")
	}
}

func TestBisectModel_View_Error(t *testing.T) {
	t.Parallel()
	bm := NewBisectModel(testTheme(), 80, 24)
	bm.errMsg = "something went wrong"
	view := bm.View()
	if !strings.Contains(view, "something went wrong") {
		t.Error("view should contain error message")
	}
}

func TestBisectModel_View_LongHash(t *testing.T) {
	t.Parallel()
	bm := NewBisectModel(testTheme(), 80, 24)
	commits := testCommits(3)
	commits[0].Hash = "abcdef1234567890abcdef1234567890abcdef12"
	bm.SetCommits(commits)
	view := bm.View()
	if !strings.Contains(view, "abcdef1") {
		t.Error("view should contain first 7 chars of hash")
	}
}

func TestBisectModel_View_ScrollDown(t *testing.T) {
	t.Parallel()
	bm := NewBisectModel(testTheme(), 80, 24)
	commits := testCommits(20)
	bm.SetCommits(commits)
	// Move current past 5 to trigger scroll
	bm.current = 10
	view := bm.View()
	if view == "" {
		t.Error("scrolled view should return non-empty string")
	}
}

func TestBisectModel_View_AllStatuses(t *testing.T) {
	t.Parallel()
	bm := NewBisectModel(testTheme(), 80, 24)
	commits := testCommits(5)
	commits[0].Status = "good"
	commits[1].Status = "bad"
	commits[2].Status = "skip"
	commits[3].Status = "testing"
	commits[4].Status = "pending"
	bm.SetCommits(commits)
	bm.current = 0
	view := bm.View()
	// Check for at least some status icons (view may clip to window)
	hasIcon := false
	for _, icon := range []string{"✓", "✗", "⊘", "◐", "○"} {
		if strings.Contains(view, icon) {
			hasIcon = true
			break
		}
	}
	if !hasIcon {
		t.Error("view should contain at least one status icon")
	}
}

func TestBisectModel_View_LongMessage(t *testing.T) {
	t.Parallel()
	bm := NewBisectModel(testTheme(), 80, 24)
	commits := testCommits(3)
	commits[0].Message = strings.Repeat("very long commit message ", 10)
	bm.SetCommits(commits)
	view := bm.View()
	if view == "" {
		t.Error("view with long message should return non-empty string")
	}
}
