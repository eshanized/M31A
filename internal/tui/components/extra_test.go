package components

import (
	"encoding/json"
	"strings"
	"testing"
	"time"

	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/tui/theme"
	"github.com/eshanized/M31A/internal/types"
)

func TestTruncateEnd(t *testing.T) {
	t.Parallel()
	tests := []struct {
		s      string
		maxLen int
		want   string
	}{
		{"hello", 10, "hello"},
		{"hello", 5, "hello"},
		{"hello world", 5, "hell…"},
		{"hello", 0, ""},
		{"hello", -1, ""},
		{"", 5, ""},
		{"ab", 2, "ab"},
		{"abc", 2, "a…"},
		{"日本語テスト", 4, "日本語…"},
		{"hello", 1, "…"},
	}
	for _, tt := range tests {
		got := TruncateEnd(tt.s, tt.maxLen)
		if got != tt.want {
			t.Errorf("TruncateEnd(%q, %d) = %q, want %q", tt.s, tt.maxLen, got, tt.want)
		}
	}
}

func TestDetectLanguageFromCode(t *testing.T) {
	t.Parallel()
	got := DetectLanguage("package main\n\nimport \"fmt\"\n\nfunc main() {\n\tfmt.Println(\"hello\")\n}")
	if got != "Go" {
		t.Errorf("DetectLanguage(Go code) = %q, want \"Go\"", got)
	}
	empty := DetectLanguage("")
	if empty != "" {
		t.Errorf("DetectLanguage(empty) = %q, want empty", empty)
	}
}

func TestCalcContentWidth(t *testing.T) {
	t.Parallel()
	tests := []struct {
		width int
		want  int
	}{
		{80, 74},
		{100, 94},
		{30, 24}, // 30 - 6 = 24
		{28, 22}, // 28 - 6 = 22
		{0, 20},  // clamped to min 20
	}
	for _, tt := range tests {
		got := calcContentWidth(tt.width)
		if got != tt.want {
			t.Errorf("calcContentWidth(%d) = %d, want %d", tt.width, got, tt.want)
		}
	}
}

func TestFormatMetric(t *testing.T) {
	t.Parallel()
	tests := []struct {
		n    int
		want string
	}{
		{0, "0"},
		{1, "1"},
		{999, "999"},
		{1000, "1.0K"},
		{1500, "1.5K"},
		{999999, "1000.0K"},
		{1000000, "1.0M"},
		{1500000, "1.5M"},
		{2500000, "2.5M"},
	}
	for _, tt := range tests {
		got := FormatMetric(tt.n)
		if got != tt.want {
			t.Errorf("FormatMetric(%d) = %q, want %q", tt.n, got, tt.want)
		}
	}
}

func TestFormatCost(t *testing.T) {
	t.Parallel()
	tests := []struct {
		cost float64
		want string
	}{
		{0.0, "$0.0000"},
		{0.123456, "$0.1235"},
		{1.5, "$1.5000"},
		{100.0, "$100.0000"},
		{0.001, "$0.0010"},
	}
	for _, tt := range tests {
		got := FormatCost(tt.cost)
		if got != tt.want {
			t.Errorf("FormatCost(%f) = %q, want %q", tt.cost, got, tt.want)
		}
	}
}

func TestFormatDuration(t *testing.T) {
	t.Parallel()
	tests := []struct {
		seconds int
		want    string
	}{
		{0, "0s"},
		{30, "30s"},
		{59, "59s"},
		{60, "1m 0s"},
		{90, "1m 30s"},
		{3599, "59m 59s"},
		{3600, "1h 0m 0s"},
		{3661, "1h 1m 1s"},
		{7200, "2h 0m 0s"},
	}
	for _, tt := range tests {
		got := FormatDuration(tt.seconds)
		if got != tt.want {
			t.Errorf("FormatDuration(%d) = %q, want %q", tt.seconds, got, tt.want)
		}
	}
}

func TestSanitizeOutputCases(t *testing.T) {
	t.Parallel()
	tests := []struct {
		input string
		want  string
	}{
		{"hello world", "hello world"},
		{"\x1b[31mred\x1b[0m", "red"},
		{"\x1b[1;32mgreen\x1b[0m", "green"},
		{"normal\n\ttext", "normal\n\ttext"},
		{"", ""},
		{"no escapes here", "no escapes here"},
	}
	for _, tt := range tests {
		got := SanitizeOutput(tt.input)
		if got != tt.want {
			t.Errorf("SanitizeOutput(%q) = %q, want %q", tt.input, got, tt.want)
		}
	}
}

func TestGetSpinnerFrames(t *testing.T) {
	t.Parallel()
	tests := []struct {
		style   string
		wantLen int
	}{
		{"braille", 10},
		{"dots", 8},
		{"arc", 6},
		{"bouncing", 8},
		{"line", 4},
		{"grow", 8},
		{"pulse", 4},
		{"unknown", 10}, // default braille
		{"", 10},        // default braille
	}
	for _, tt := range tests {
		got := GetSpinnerFrames(tt.style)
		if len(got) != tt.wantLen {
			t.Errorf("GetSpinnerFrames(%q) returned %d frames, want %d", tt.style, len(got), tt.wantLen)
		}
	}
}

func TestSpinner_Next_Peek_Reset(t *testing.T) {
	t.Parallel()
	s := NewSpinner()
	f0 := s.Peek()
	f1 := s.Next()
	if f0 != f1 {
		t.Errorf("Peek and Next should return same frame initially, got Peek=%q Next=%q", f0, f1)
	}
	f2 := s.Next()
	if f1 == f2 && len(OpenCodeFrames) > 1 {
		t.Error("Next should advance to different frame")
	}
	s.Reset()
	f3 := s.Peek()
	if f3 != f0 {
		t.Errorf("Reset should return to first frame, got %q want %q", f3, f0)
	}
}

func TestSpinner_Tick(t *testing.T) {
	t.Parallel()
	s := NewSpinner()
	if s.Tick() != 100*time.Millisecond {
		t.Errorf("default tick = %v, want 100ms", s.Tick())
	}
	s2 := NewSpinnerWithFrames(OpenCodeFrames, 200*time.Millisecond)
	if s2.Tick() != 200*time.Millisecond {
		t.Errorf("custom tick = %v, want 200ms", s2.Tick())
	}
}

func TestSpinner_NextWraps(t *testing.T) {
	t.Parallel()
	s := NewSpinner()
	var first string
	for i := 0; i < len(OpenCodeFrames)+1; i++ {
		f := s.Next()
		if i == 0 {
			first = f
		}
		if i == len(OpenCodeFrames) {
			if f != first {
				t.Error("spinner should wrap to first frame")
			}
		}
	}
}

func TestRenderSpinner(t *testing.T) {
	t.Parallel()
	got := RenderSpinner("⠋", theme.Dark().Spinner)
	if got == "" {
		t.Error("RenderSpinner should return non-empty string")
	}
	got2 := RenderSpinner("", theme.Dark().Spinner)
	if got2 != "" {
		t.Error("RenderSpinner with empty frame should return empty string")
	}
}

func TestDropdown_Selected(t *testing.T) {
	t.Parallel()
	dd := &Dropdown{
		Items:  []DropdownItem{{Label: "A", Value: "a"}, {Label: "B", Value: "b"}},
		Cursor: 0,
	}
	s := dd.Selected()
	if s == nil || s.Value != "a" {
		t.Errorf("Selected() = %v, want A", s)
	}
	dd.Cursor = 1
	s = dd.Selected()
	if s == nil || s.Value != "b" {
		t.Errorf("Selected() = %v, want B", s)
	}
}

func TestDropdown_SelectedOutOfBounds(t *testing.T) {
	t.Parallel()
	dd := &Dropdown{Items: []DropdownItem{}, Cursor: 0}
	if dd.Selected() != nil {
		t.Error("Selected() should return nil for empty items")
	}
	dd2 := &Dropdown{Items: []DropdownItem{{Label: "A", Value: "a"}}, Cursor: 5}
	if dd2.Selected() != nil {
		t.Error("Selected() should return nil for out of bounds cursor")
	}
	dd3 := &Dropdown{Items: []DropdownItem{{Label: "A", Value: "a"}}, Cursor: -1}
	if dd3.Selected() != nil {
		t.Error("Selected() should return nil for negative cursor")
	}
}

func TestDropdown_MoveUp(t *testing.T) {
	t.Parallel()
	dd := &Dropdown{Items: []DropdownItem{{Label: "A"}, {Label: "B"}}, Cursor: 1}
	dd.MoveUp()
	if dd.Cursor != 0 {
		t.Errorf("MoveUp from 1: cursor = %d, want 0", dd.Cursor)
	}
	dd.MoveUp() // already at 0
	if dd.Cursor != 0 {
		t.Error("MoveUp at boundary should stay at 0")
	}
}

func TestDropdown_MoveDown(t *testing.T) {
	t.Parallel()
	dd := &Dropdown{Items: []DropdownItem{{Label: "A"}, {Label: "B"}}, Cursor: 0}
	dd.MoveDown()
	if dd.Cursor != 1 {
		t.Errorf("MoveDown from 0: cursor = %d, want 1", dd.Cursor)
	}
	dd.MoveDown() // already at end
	if dd.Cursor != 1 {
		t.Error("MoveDown at boundary should stay at end")
	}
}

func TestDropdown_Toggle(t *testing.T) {
	t.Parallel()
	dd := &Dropdown{Open: false}
	dd.Toggle()
	if !dd.Open {
		t.Error("Toggle should set Open to true")
	}
	dd.Toggle()
	if dd.Open {
		t.Error("Toggle should set Open to false")
	}
}

func TestDropdown_View(t *testing.T) {
	t.Parallel()
	dd := &Dropdown{
		Items:  []DropdownItem{{Label: "X"}, {Label: "Y"}},
		Cursor: 0,
		Theme:  theme.Dark(),
		Width:  20,
	}
	got := dd.View()
	if got == "" {
		t.Error("View should return non-empty string")
	}
	dd.Open = true
	got2 := dd.View()
	if got2 == "" {
		t.Error("View open should return non-empty string")
	}
}

func TestDropdown_ViewEmpty(t *testing.T) {
	t.Parallel()
	dd := &Dropdown{Items: []DropdownItem{}, Theme: theme.Dark()}
	got := dd.View()
	if got != "" {
		t.Error("View for empty items should return empty string")
	}
}

func TestTabBar_MoveLeft(t *testing.T) {
	t.Parallel()
	tb := &TabBar{Tabs: []string{"A", "B", "C"}, Active: 2}
	tb.MoveLeft()
	if tb.Active != 1 {
		t.Errorf("MoveLeft: Active = %d, want 1", tb.Active)
	}
	tb.MoveLeft()
	if tb.Active != 0 {
		t.Errorf("MoveLeft: Active = %d, want 0", tb.Active)
	}
	tb.MoveLeft() // at boundary
	if tb.Active != 0 {
		t.Error("MoveLeft at 0 should stay at 0")
	}
}

func TestTabBar_MoveRight(t *testing.T) {
	t.Parallel()
	tb := &TabBar{Tabs: []string{"A", "B", "C"}, Active: 0}
	tb.MoveRight()
	if tb.Active != 1 {
		t.Errorf("MoveRight: Active = %d, want 1", tb.Active)
	}
	tb.MoveRight()
	if tb.Active != 2 {
		t.Errorf("MoveRight: Active = %d, want 2", tb.Active)
	}
	tb.MoveRight() // at boundary
	if tb.Active != 2 {
		t.Error("MoveRight at end should stay at end")
	}
}

func TestTabBar_View(t *testing.T) {
	t.Parallel()
	tb := TabBar{Tabs: []string{"Tab1", "Tab2"}, Active: 0, Theme: theme.Dark(), Width: 40}
	got := tb.View()
	if got == "" {
		t.Error("TabBar View should return non-empty string")
	}
}

func TestFilterChips_ActiveCount(t *testing.T) {
	t.Parallel()
	fc := FilterChips{
		Chips: []FilterChip{
			{Label: "A", Active: true},
			{Label: "B", Active: false},
			{Label: "C", Active: true},
		},
	}
	if got := fc.ActiveCount(); got != 2 {
		t.Errorf("ActiveCount() = %d, want 2", got)
	}
}

func TestFilterChips_ActiveCountEmpty(t *testing.T) {
	t.Parallel()
	fc := FilterChips{Chips: []FilterChip{}}
	if got := fc.ActiveCount(); got != 0 {
		t.Errorf("ActiveCount() = %d, want 0", got)
	}
}

func TestFilterChips_ToggleChip(t *testing.T) {
	t.Parallel()
	fc := &FilterChips{
		Chips: []FilterChip{
			{Label: "A", Active: false},
			{Label: "B", Active: true},
		},
	}
	fc.ToggleChip(0)
	if !fc.Chips[0].Active {
		t.Error("ToggleChip(0) should set Active to true")
	}
	fc.ToggleChip(1)
	if fc.Chips[1].Active {
		t.Error("ToggleChip(1) should set Active to false")
	}
}

func TestFilterChips_ToggleChipOutOfBounds(t *testing.T) {
	t.Parallel()
	fc := &FilterChips{Chips: []FilterChip{{Label: "A", Active: false}}}
	fc.ToggleChip(-1) // should not panic
	fc.ToggleChip(5)  // should not panic
	if fc.Chips[0].Active {
		t.Error("ToggleChip OOB should not change state")
	}
}

func TestHashPosition(t *testing.T) {
	t.Parallel()
	h1 := hashPosition(42, 10, 20)
	h2 := hashPosition(42, 10, 20)
	if h1 != h2 {
		t.Error("same inputs should produce same hash")
	}
	h3 := hashPosition(42, 10, 21)
	if h1 == h3 {
		t.Error("different y should produce different hash")
	}
	h4 := hashPosition(42, 11, 20)
	if h1 == h4 {
		t.Error("different x should produce different hash")
	}
	h5 := hashPosition(99, 10, 20)
	if h1 == h5 {
		t.Error("different seed should produce different hash")
	}
}

func TestSparkline_Resample(t *testing.T) {
	t.Parallel()
	tests := []struct {
		values []int
		target int
		want   int
	}{
		{[]int{1, 2, 3, 4, 5}, 3, 3},
		{[]int{1, 2, 3, 4, 5}, 0, 5}, // target <= 0 returns original
		{[]int{1, 2, 3, 4, 5}, -1, 5},
		{[]int{1, 2, 3, 4, 5}, 5, 5},
	}
	for _, tt := range tests {
		got := resample(tt.values, tt.target)
		if len(got) != tt.want {
			t.Errorf("resample(%v, %d) returned len %d, want %d", tt.values, tt.target, len(got), tt.want)
		}
	}
}

func TestResampleFloat64(t *testing.T) {
	t.Parallel()
	tests := []struct {
		values []float64
		target int
		want   int
	}{
		{[]float64{1.0, 2.0, 3.0, 4.0, 5.0}, 3, 3},
		{[]float64{1.0, 2.0}, 0, 2}, // target <= 0
		{[]float64{1.0}, 5, 5},
	}
	for _, tt := range tests {
		got := resampleFloat64(tt.values, tt.target)
		if len(got) != tt.want {
			t.Errorf("resampleFloat64 returned len %d, want %d", len(got), tt.want)
		}
	}
}

func TestResample_Averaging(t *testing.T) {
	t.Parallel()
	// [10, 20, 30, 40] → target 2: first half avg=15, second half avg=35
	got := resample([]int{10, 20, 30, 40}, 2)
	if len(got) != 2 {
		t.Fatalf("expected 2 elements, got %d", len(got))
	}
	if got[0] != 15 {
		t.Errorf("got[0] = %d, want 15", got[0])
	}
	if got[1] != 35 {
		t.Errorf("got[1] = %d, want 35", got[1])
	}
}

func TestResampleFloat64_Averaging(t *testing.T) {
	t.Parallel()
	got := resampleFloat64([]float64{10.0, 20.0, 30.0, 40.0}, 2)
	if len(got) != 2 {
		t.Fatalf("expected 2, got %d", len(got))
	}
	if got[0] != 15.0 {
		t.Errorf("got[0] = %f, want 15.0", got[0])
	}
	if got[1] != 35.0 {
		t.Errorf("got[1] = %f, want 35.0", got[1])
	}
}

func TestIsBinaryContent(t *testing.T) {
	t.Parallel()
	tests := []struct {
		input string
		want  bool
	}{
		{"", false},
		{"hello", false},
		{"hello\x00world", true},
		{"\x00", true},
		{"abc\x00def", true},
	}
	for _, tt := range tests {
		got := isBinaryContent(tt.input)
		if got != tt.want {
			t.Errorf("isBinaryContent(%q) = %v, want %v", tt.input, got, tt.want)
		}
	}
}

func TestIsBinaryContent_LargeString(t *testing.T) {
	t.Parallel()
	// Test with >1024 bytes, only first 1024 checked
	big := strings.Repeat("a", 2048)
	if isBinaryContent(big) {
		t.Error("large string with no nulls should not be binary")
	}
	bigWithNull := strings.Repeat("a", 500) + "\x00" + strings.Repeat("b", 1548)
	if !isBinaryContent(bigWithNull) {
		t.Error("large string with null should be binary")
	}
}

func TestToolCard_ToggleCollapsed(t *testing.T) {
	t.Parallel()
	input := json.RawMessage(`{"command": "ls"}`)
	call := types.ToolCall{ID: "1", Name: "Bash", Input: input}
	tc := NewToolCard(call, nil, ToolRunning, theme.Dark())
	initial := tc.IsCollapsed()
	tc.Toggle()
	if tc.IsCollapsed() == initial {
		t.Error("Toggle should change collapsed state")
	}
	tc.Toggle()
	if tc.IsCollapsed() != initial {
		t.Error("second Toggle should restore original state")
	}
}

func TestToolCard_SetCollapsed(t *testing.T) {
	t.Parallel()
	input := json.RawMessage(`{"command": "ls"}`)
	call := types.ToolCall{ID: "1", Name: "Bash", Input: input}
	tc := NewToolCard(call, nil, ToolRunning, theme.Dark())
	tc.SetCollapsed(true)
	if !tc.IsCollapsed() {
		t.Error("SetCollapsed(true) should set collapsed")
	}
	tc.SetCollapsed(false)
	if tc.IsCollapsed() {
		t.Error("SetCollapsed(false) should unset collapsed")
	}
}

func TestToolCard_Render_Inline(t *testing.T) {
	t.Parallel()
	input := json.RawMessage(`{"command": "ls"}`)
	call := types.ToolCall{ID: "1", Name: "Bash", Input: input}
	tc := NewToolCard(call, nil, ToolRunning, theme.Dark())
	got := tc.Render(80)
	if got == "" {
		t.Error("Render should return non-empty string")
	}
}

func TestToolCard_Render_Block(t *testing.T) {
	t.Parallel()
	input := json.RawMessage(`{"command": "ls"}`)
	output := "file1.txt\nfile2.txt"
	result := &types.ToolResult{Output: output, DurationMs: 100}
	call := types.ToolCall{ID: "1", Name: "Bash", Input: input}
	tc := NewToolCard(call, result, ToolSuccess, theme.Dark())
	got := tc.Render(80)
	if got == "" {
		t.Error("Render should return non-empty string")
	}
}

func TestToolCard_Render_Error(t *testing.T) {
	t.Parallel()
	input := json.RawMessage(`{"command": "ls"}`)
	result := &types.ToolResult{Output: "error output", DurationMs: 50}
	call := types.ToolCall{ID: "1", Name: "Bash", Input: input}
	tc := NewToolCard(call, result, ToolError, theme.Dark())
	got := tc.Render(80)
	if got == "" {
		t.Error("Render should return non-empty string")
	}
}

func TestToolCard_Render_Narrow(t *testing.T) {
	t.Parallel()
	input := json.RawMessage(`{"command": "ls"}`)
	call := types.ToolCall{ID: "1", Name: "Bash", Input: input}
	tc := NewToolCard(call, nil, ToolRunning, theme.Dark())
	got := tc.Render(10) // very narrow
	if got == "" {
		t.Error("Render narrow should return non-empty string")
	}
}

func TestToolCard_UnknownTool(t *testing.T) {
	t.Parallel()
	input := json.RawMessage(`"test"`)
	call := types.ToolCall{ID: "1", Name: "UnknownTool", Input: input}
	tc := NewToolCard(call, nil, ToolRunning, theme.Dark())
	got := tc.Render(80)
	if got == "" {
		t.Error("Render for unknown tool should return non-empty string")
	}
}

func TestToolCard_WithTruncation(t *testing.T) {
	t.Parallel()
	input := json.RawMessage(`{"command": "cat file"}`)
	longOutput := strings.Repeat("x\n", 50)
	result := &types.ToolResult{Output: longOutput, DurationMs: 100, Truncated: false}
	call := types.ToolCall{ID: "1", Name: "Bash", Input: input}
	tc := NewToolCard(call, result, ToolSuccess, theme.Dark())
	got := tc.Render(80)
	if got == "" {
		t.Error("Render with long output should return non-empty string")
	}
}

func TestToolIcons(t *testing.T) {
	t.Parallel()
	tools := []string{"Bash", "Edit", "FileRead", "FileWrite", "Glob", "Grep", "TodoWrite", "AskUserQuestion"}
	for _, tool := range tools {
		if _, ok := ToolIcons[tool]; !ok {
			t.Errorf("ToolIcons missing entry for %q", tool)
		}
	}
}

func TestToolStatusIcons(t *testing.T) {
	t.Parallel()
	states := []ToolState{ToolRunning, ToolSuccess, ToolError}
	for _, state := range states {
		if _, ok := ToolStatusIcons[state]; !ok {
			t.Errorf("ToolStatusIcons missing entry for state %d", state)
		}
	}
}

func TestThinkingBlock_ID(t *testing.T) {
	t.Parallel()
	seg := types.MessageSegment{Type: "thinking"}
	tb := NewThinkingBlock(seg, theme.Dark(), false, 42)
	if tb.ID() != 42 {
		t.Errorf("ID() = %d, want 42", tb.ID())
	}
}

func TestThinkingBlock_SetFocused(t *testing.T) {
	t.Parallel()
	seg := types.MessageSegment{Type: "thinking"}
	tb := NewThinkingBlock(seg, theme.Dark(), false, 0)
	if tb.IsFocused() {
		t.Error("should not be focused initially")
	}
	tb.SetFocused(true)
	if !tb.IsFocused() {
		t.Error("should be focused after SetFocused(true)")
	}
	tb.SetFocused(false)
	if tb.IsFocused() {
		t.Error("should not be focused after SetFocused(false)")
	}
}

func TestThinkingBlock_ScrollUp(t *testing.T) {
	t.Parallel()
	seg := types.MessageSegment{Type: "thinking", Content: "line1\nline2\nline3"}
	tb := NewThinkingBlock(seg, theme.Dark(), true, 0)
	tb.ScrollUp(5)
	if tb.ScrollOffset() != 0 {
		t.Errorf("ScrollUp below 0: offset = %d, want 0", tb.ScrollOffset())
	}
}

func TestThinkingBlock_ScrollDown(t *testing.T) {
	t.Parallel()
	seg := types.MessageSegment{Type: "thinking", Content: "line1\nline2\nline3"}
	tb := NewThinkingBlock(seg, theme.Dark(), true, 0)
	tb.ScrollDown(1)
	if tb.ScrollOffset() != 0 {
		t.Errorf("ScrollDown on 3-line content: offset = %d, want 0", tb.ScrollOffset())
	}
}

func TestThinkingBlock_Duration_WithMs(t *testing.T) {
	t.Parallel()
	seg := types.MessageSegment{Type: "thinking", DurationMs: 1500}
	tb := NewThinkingBlock(seg, theme.Dark(), false, 0)
	dur := tb.Duration()
	if !strings.Contains(dur, "1.5s") {
		t.Errorf("Duration() = %q, want it to contain '1.5s'", dur)
	}
}

func TestThinkingBlock_Duration_Zero(t *testing.T) {
	t.Parallel()
	seg := types.MessageSegment{Type: "thinking"}
	tb := NewThinkingBlock(seg, theme.Dark(), false, 0)
	dur := tb.Duration()
	if dur == "" {
		t.Error("Duration() should not be empty")
	}
}

func TestThinkingBlock_Duration_Cache(t *testing.T) {
	t.Parallel()
	seg := types.MessageSegment{Type: "thinking", DurationMs: 1500}
	tb := NewThinkingBlock(seg, theme.Dark(), false, 0)
	d1 := tb.Duration()
	d2 := tb.Duration()
	if d1 != d2 {
		t.Errorf("cached Duration() mismatch: %q vs %q", d1, d2)
	}
}

func TestThinkingBlock_Render_ExpandedWithScroll(t *testing.T) {
	t.Parallel()
	lines := strings.Repeat("thinking line\n", 30)
	seg := types.MessageSegment{Type: "thinking", Content: lines}
	tb := NewThinkingBlock(seg, theme.Dark(), true, 0)
	tb.ScrollDown(5)
	got := tb.Render(80)
	if got == "" {
		t.Error("Render expanded with scroll should return non-empty string")
	}
}

func TestThinkingBlock_Header(t *testing.T) {
	t.Parallel()
	seg := types.MessageSegment{Type: "thinking", DurationMs: 1200}
	tb := NewThinkingBlock(seg, theme.Dark(), false, 0)
	h := tb.Header(80)
	if h == "" {
		t.Error("Header should return non-empty string")
	}
	// Header should contain an intent label (e.g. "Analyzing", "Thinking", etc.)
	if !strings.Contains(h, "Analyzing") && !strings.Contains(h, "Thinking") && !strings.Contains(h, "Refining") {
		t.Error("Header should contain an intent label")
	}
}

func TestThinkingBlock_Header_CompactWidth(t *testing.T) {
	t.Parallel()
	seg := types.MessageSegment{Type: "thinking", DurationMs: 1200}
	tb := NewThinkingBlock(seg, theme.Dark(), true, 0)
	h := tb.Header(10) // very narrow
	if h == "" {
		t.Error("Header narrow should return non-empty string")
	}
}

func TestThinkingBlock_Header_ScrollHint(t *testing.T) {
	t.Parallel()
	lines := strings.Repeat("line\n", 30)
	seg := types.MessageSegment{Type: "thinking", Content: lines}
	tb := NewThinkingBlock(seg, theme.Dark(), true, 0)
	tb.ScrollDown(5)
	h := tb.Header(80)
	if !strings.Contains(h, "↑↓") {
		t.Error("Header with scroll offset should contain scroll hint '↑↓'")
	}
}

func TestBreadcrumb_View(t *testing.T) {
	t.Parallel()
	bc := Breadcrumb{Parts: []string{"Home", "Settings"}, Theme: theme.Dark()}
	got := bc.View()
	if got == "" {
		t.Error("View should return non-empty string")
	}
}

func TestBreadcrumb_ViewEmpty(t *testing.T) {
	t.Parallel()
	bc := Breadcrumb{Parts: []string{}, Theme: theme.Dark()}
	got := bc.View()
	if got != "" {
		t.Error("View for empty parts should return empty string")
	}
}

func TestBreadcrumb_ViewSingle(t *testing.T) {
	t.Parallel()
	bc := Breadcrumb{Parts: []string{"Only"}, Theme: theme.Dark()}
	got := bc.View()
	if got == "" {
		t.Error("View for single part should return non-empty string")
	}
}

func TestTimelineViewEmptyEntries(t *testing.T) {
	t.Parallel()
	tv := TimelineView{Entries: []TimelineEntry{}, Height: 10, Theme: theme.Dark()}
	got := tv.View()
	if got != "" {
		t.Error("View for empty entries should return empty string")
	}
}

func TestTimelineView_WithEntries(t *testing.T) {
	t.Parallel()
	entries := []TimelineEntry{
		{Time: "10:00", Title: "Step 1", Status: "done"},
		{Time: "10:01", Title: "Step 2", Status: "active"},
		{Time: "10:02", Title: "Step 3", Status: "pending"},
	}
	tv := TimelineView{Entries: entries, Height: 10, Theme: theme.Dark()}
	got := tv.View()
	if got == "" {
		t.Error("View should return non-empty string")
	}
}

func TestTimelineView_ScrollOffset(t *testing.T) {
	t.Parallel()
	entries := []TimelineEntry{
		{Time: "10:00", Title: "Step 1", Status: "done"},
		{Time: "10:01", Title: "Step 2", Status: "active"},
	}
	tv := TimelineView{Entries: entries, Offset: 1, Height: 1, Theme: theme.Dark()}
	got := tv.View()
	if got == "" {
		t.Error("View with offset should return non-empty string")
	}
}

func TestTimelineView_WithDetail(t *testing.T) {
	t.Parallel()
	entries := []TimelineEntry{
		{Time: "10:00", Title: "Step 1", Detail: "some detail", Status: "done"},
	}
	tv := TimelineView{Entries: entries, Height: 5, Theme: theme.Dark()}
	got := tv.View()
	if !strings.Contains(got, "some detail") {
		t.Error("View should contain detail text")
	}
}

func TestStatRow_Render(t *testing.T) {
	t.Parallel()
	sr := StatRow{Label: "Tokens", Value: "1500", Theme: theme.Dark()}
	got := sr.Render()
	if got == "" {
		t.Error("Render should return non-empty string")
	}
	if !strings.Contains(got, "Tokens") {
		t.Error("Render should contain label")
	}
}

func TestStatRow_WithIcon(t *testing.T) {
	t.Parallel()
	sr := StatRow{Label: "Cost", Value: "$0.50", Icon: "$", Theme: theme.Dark()}
	got := sr.Render()
	if got == "" {
		t.Error("Render should return non-empty string")
	}
}

func TestStatRow_WithWidth(t *testing.T) {
	t.Parallel()
	sr := StatRow{Label: "X", Value: "1", Width: 30, Theme: theme.Dark()}
	got := sr.Render()
	if got == "" {
		t.Error("Render with width should return non-empty string")
	}
}

func TestStatGroup_Render(t *testing.T) {
	t.Parallel()
	sg := StatGroup{
		Stats: []StatRow{
			{Label: "A", Value: "1"},
			{Label: "B", Value: "2"},
		},
		Theme: theme.Dark(),
	}
	got := sg.Render()
	if got == "" {
		t.Error("Render should return non-empty string")
	}
}

func TestStatGroup_RenderEmpty(t *testing.T) {
	t.Parallel()
	sg := StatGroup{Stats: []StatRow{}, Theme: theme.Dark()}
	got := sg.Render()
	if got != "" {
		t.Error("Render for empty stats should return empty string")
	}
}

func TestStatGroup_WithSeparator(t *testing.T) {
	t.Parallel()
	sg := StatGroup{
		Stats: []StatRow{
			{Label: "A", Value: "1"},
			{Label: "B", Value: "2"},
		},
		Separator: true,
		Width:     40,
		Theme:     theme.Dark(),
	}
	got := sg.Render()
	if got == "" {
		t.Error("Render with separator should return non-empty string")
	}
}

func TestKeyValue_Render(t *testing.T) {
	t.Parallel()
	kv := KeyValue{Key: "model", Value: "claude-3", Theme: theme.Dark()}
	got := kv.Render()
	if got == "" {
		t.Error("Render should return non-empty string")
	}
}

func TestKeyValue_WithWidth(t *testing.T) {
	t.Parallel()
	kv := KeyValue{Key: "x", Value: "1", Width: 20, Theme: theme.Dark()}
	got := kv.Render()
	if got == "" {
		t.Error("Render with width should return non-empty string")
	}
}

func TestKeyValueGrid_Render(t *testing.T) {
	t.Parallel()
	kvg := KeyValueGrid{
		Pairs: []KeyValue{
			{Key: "A", Value: "1"},
			{Key: "B", Value: "2"},
		},
		Theme: theme.Dark(),
	}
	got := kvg.Render()
	if got == "" {
		t.Error("Render should return non-empty string")
	}
}

func TestKeyValueGrid_RenderEmpty(t *testing.T) {
	t.Parallel()
	kvg := KeyValueGrid{Pairs: []KeyValue{}, Theme: theme.Dark()}
	got := kvg.Render()
	if got != "" {
		t.Error("Render for empty pairs should return empty string")
	}
}

func TestTaskGraph_ViewEmpty(t *testing.T) {
	t.Parallel()
	tg := TaskGraph{Nodes: []TaskNode{}, Theme: theme.Dark()}
	got := tg.View()
	if got == "" {
		t.Error("View for empty nodes should return non-empty string")
	}
	if !strings.Contains(got, "No tasks") {
		t.Error("View for empty should contain 'No tasks'")
	}
}

func TestTaskGraph_ViewWithNodes(t *testing.T) {
	t.Parallel()
	nodes := []TaskNode{
		{ID: 1, Label: "Task 1", Status: "done"},
		{ID: 2, Label: "Task 2", Status: "running", Deps: []int{1}},
		{ID: 3, Label: "Task 3", Status: "failed"},
		{ID: 4, Label: "Task 4", Status: "pending"},
	}
	tg := TaskGraph{Nodes: nodes, Theme: theme.Dark()}
	got := tg.View()
	if got == "" {
		t.Error("View should return non-empty string")
	}
	if !strings.Contains(got, "Wave 1") {
		t.Error("View should contain wave headers")
	}
	if !strings.Contains(got, "Task 1") {
		t.Error("View should contain task labels")
	}
}

func TestSectionDivider_Render(t *testing.T) {
	t.Parallel()
	sd := SectionDivider{Title: "Section", Width: 40, Theme: theme.Dark()}
	got := sd.Render()
	if got == "" {
		t.Error("Render should return non-empty string")
	}
}

func TestSectionDivider_NoTitle(t *testing.T) {
	t.Parallel()
	sd := SectionDivider{Title: "", Width: 40, Theme: theme.Dark()}
	got := sd.Render()
	if got == "" {
		t.Error("Render without title should return non-empty string")
	}
}

func TestSectionDivider_TooNarrow(t *testing.T) {
	t.Parallel()
	sd := SectionDivider{Title: "X", Width: 2, Theme: theme.Dark()}
	got := sd.Render()
	if got != "" {
		t.Error("Render too narrow should return empty string")
	}
}

func TestSectionDivider_NarrowTitle(t *testing.T) {
	t.Parallel()
	sd := SectionDivider{Title: "Very Long Section Title That Exceeds Width", Width: 10, Theme: theme.Dark()}
	got := sd.Render()
	if got == "" {
		t.Error("Render with title wider than width should return label")
	}
}

func TestProgressBar_AllStyles(t *testing.T) {
	t.Parallel()
	styles := []ProgressBarStyle{ProgressBarThin, ProgressBarThick, ProgressBarBlock, ProgressBarRounded}
	for _, style := range styles {
		pb := ProgressBar{Progress: 0.5, Width: 30, ShowPct: true, Style: style, Theme: theme.Dark()}
		got := pb.Render()
		if got == "" {
			t.Errorf("Render with style %d should return non-empty string", style)
		}
	}
}

func TestProgressBar_NegativeProgress(t *testing.T) {
	t.Parallel()
	pb := ProgressBar{Progress: -0.5, Width: 30, Theme: theme.Dark()}
	got := pb.Render()
	if got == "" {
		t.Error("Render with negative progress should return non-empty string")
	}
}

func TestProgressBar_OverOneProgress(t *testing.T) {
	t.Parallel()
	pb := ProgressBar{Progress: 1.5, Width: 30, Theme: theme.Dark()}
	got := pb.Render()
	if got == "" {
		t.Error("Render with >1 progress should return non-empty string")
	}
}

func TestProgressBar_ZeroWidth(t *testing.T) {
	t.Parallel()
	pb := ProgressBar{Progress: 0.5, Width: 0, Theme: theme.Dark()}
	got := pb.Render()
	if got == "" {
		t.Error("Render with zero width should return non-empty string")
	}
}

func TestProgressBar_RenderWithLabel(t *testing.T) {
	t.Parallel()
	pb := ProgressBar{Progress: 0.5, Width: 40, Theme: theme.Dark()}
	got := pb.RenderWithLabel("Building")
	if got == "" {
		t.Error("RenderWithLabel should return non-empty string")
	}
}

func TestProgressBar_RenderWithLabel_ShortBar(t *testing.T) {
	t.Parallel()
	pb := ProgressBar{Progress: 0.5, Width: 10, Theme: theme.Dark()}
	got := pb.RenderWithLabel("LongLabel")
	if got == "" {
		t.Error("RenderWithLabel short bar should return non-empty string")
	}
}

func TestSegmentedBar_Render(t *testing.T) {
	t.Parallel()
	sb := SegmentedBar{
		Segments: []Segment{
			{Count: 5},
			{Count: 3},
		},
		Width: 40,
		Theme: theme.Dark(),
	}
	got := sb.Render()
	if got == "" {
		t.Error("Render should return non-empty string")
	}
}

func TestSegmentedBar_ZeroTotal(t *testing.T) {
	t.Parallel()
	sb := SegmentedBar{Segments: []Segment{}, Width: 40, Theme: theme.Dark()}
	got := sb.Render()
	if got == "" {
		t.Error("Render with zero total should return non-empty string")
	}
}

func TestSegmentedBar_ZeroWidth(t *testing.T) {
	t.Parallel()
	sb := SegmentedBar{Segments: []Segment{{Count: 5}}, Width: 0, Theme: theme.Dark()}
	got := sb.Render()
	if got == "" {
		t.Error("Render with zero width should return non-empty string")
	}
}

func TestCompactProgress_Render(t *testing.T) {
	t.Parallel()
	cp := CompactProgress{Progress: 0.5, ShowChars: false}
	got := cp.Render()
	if got != "[50%]" {
		t.Errorf("Render = %q, want [50%%]", got)
	}
}

func TestCompactProgress_RenderChars(t *testing.T) {
	t.Parallel()
	cp := CompactProgress{Progress: 0.5, ShowChars: true}
	got := cp.Render()
	if got == "" {
		t.Error("Render with chars should return non-empty string")
	}
}

func TestCompactProgress_Negative(t *testing.T) {
	t.Parallel()
	cp := CompactProgress{Progress: -0.5, ShowChars: false}
	got := cp.Render()
	if got != "[0%]" {
		t.Errorf("Render negative = %q, want [0%%]", got)
	}
}

func TestCompactProgress_OverOne(t *testing.T) {
	t.Parallel()
	cp := CompactProgress{Progress: 1.5, ShowChars: false}
	got := cp.Render()
	if got != "[100%]" {
		t.Errorf("Render >1 = %q, want [100%%]", got)
	}
}

func TestAnimatedProgress_UpdateProgress(t *testing.T) {
	t.Parallel()
	ap := AnimatedProgress{}
	ap.UpdateProgress(0, 5, 10)
	if ap.Current != 5 {
		t.Errorf("Current = %d, want 5", ap.Current)
	}
	if ap.Total != 10 {
		t.Errorf("Total = %d, want 10", ap.Total)
	}
	if !ap.Animating {
		t.Error("should be animating after advance")
	}
}

func TestAnimatedProgress_UpdateProgress_NoAdvance(t *testing.T) {
	t.Parallel()
	ap := AnimatedProgress{}
	ap.UpdateProgress(5, 3, 10) // no advance
	if ap.Animating {
		t.Error("should not be animating when not advancing")
	}
}

func TestAnimatedProgress_Tick(t *testing.T) {
	t.Parallel()
	ap := AnimatedProgress{}
	ap.UpdateProgress(0, 5, 10)
	// Simulate time passing
	time.Sleep(600 * time.Millisecond)
	done := ap.Tick()
	if !done {
		t.Error("Tick should be done after animation duration")
	}
	if ap.Animating {
		t.Error("should not be animating after completion")
	}
}

func TestAnimatedProgress_Progress(t *testing.T) {
	t.Parallel()
	ap := AnimatedProgress{Displayed: 5, Total: 10}
	got := ap.Progress()
	if got != 0.5 {
		t.Errorf("Progress() = %f, want 0.5", got)
	}
}

func TestAnimatedProgress_Progress_ZeroTotal(t *testing.T) {
	t.Parallel()
	ap := AnimatedProgress{Displayed: 5, Total: 0}
	got := ap.Progress()
	if got != 0 {
		t.Errorf("Progress() = %f, want 0", got)
	}
}

func TestFlashState_Tick(t *testing.T) {
	t.Parallel()
	f := &flashState{}
	f.StartFlash(lipgloss.Color("1")) // color doesn't matter
	if f.Tick() {
		t.Error("Tick should return false immediately after start")
	}
	time.Sleep(350 * time.Millisecond)
	if !f.Tick() {
		t.Error("Tick should return true after flash duration")
	}
}

func TestFlashState_TickNotActive(t *testing.T) {
	t.Parallel()
	f := &flashState{active: false}
	if !f.Tick() {
		t.Error("Tick should return true when not active")
	}
}

func TestAnimatedProgressBar_Render(t *testing.T) {
	t.Parallel()
	apb := &AnimatedProgressBar{
		Animated: AnimatedProgress{Current: 5, Total: 10, Displayed: 5},
		Width:    30,
		ShowPct:  true,
		Theme:    theme.Dark(),
	}
	got := apb.Render()
	if got == "" {
		t.Error("Render should return non-empty string")
	}
}

func TestAnimatedProgressBar_Render_ZeroWidth(t *testing.T) {
	t.Parallel()
	apb := &AnimatedProgressBar{
		Animated: AnimatedProgress{Current: 5, Total: 10, Displayed: 5},
		Width:    0,
		Theme:    theme.Dark(),
	}
	got := apb.Render()
	if got == "" {
		t.Error("Render with zero width should return non-empty string")
	}
}

func TestAnimatedProgressBar_Render_Flash(t *testing.T) {
	t.Parallel()
	apb := &AnimatedProgressBar{
		Animated: AnimatedProgress{Current: 5, Total: 10, Displayed: 5},
		Flash:    flashState{active: true, color: lipgloss.Color("1")},
		Width:    30,
		Theme:    theme.Dark(),
	}
	got := apb.Render()
	if got == "" {
		t.Error("Render with flash should return non-empty string")
	}
}

func TestThinkingBlock_Render_SmallWidth(t *testing.T) {
	t.Parallel()
	seg := types.MessageSegment{Type: "thinking", Content: "short"}
	tb := NewThinkingBlock(seg, theme.Dark(), true, 0)
	got := tb.Render(10) // very narrow
	if got == "" {
		t.Error("Render with small width should return non-empty string")
	}
}

func TestToolCard_Render_SmallWidth(t *testing.T) {
	t.Parallel()
	input := json.RawMessage(`{"command": "echo hi"}`)
	call := types.ToolCall{ID: "1", Name: "Bash", Input: input}
	tc := NewToolCard(call, nil, ToolRunning, theme.Dark())
	got := tc.Render(8) // very narrow
	if got == "" {
		t.Error("Render with small width should return non-empty string")
	}
}

func TestBaseRenderer_RenderInput(t *testing.T) {
	t.Parallel()
	r := RendererForTool("Bash", theme.Dark())
	input := json.RawMessage(`{"command": "ls -la"}`)
	call := types.ToolCall{ID: "1", Name: "Bash", Input: input}
	got := r.RenderInput(call, 40)
	if got == "" {
		t.Error("RenderInput should return non-empty string")
	}
}

func TestBaseRenderer_RenderInput_InvalidJSON(t *testing.T) {
	t.Parallel()
	r := RendererForTool("Bash", theme.Dark())
	input := json.RawMessage(`not json`)
	call := types.ToolCall{ID: "1", Name: "Bash", Input: input}
	got := r.RenderInput(call, 40)
	if got == "" {
		t.Error("RenderInput with invalid JSON should return non-empty string")
	}
}

func TestBaseRenderer_RenderOutput(t *testing.T) {
	t.Parallel()
	r := RendererForTool("Bash", theme.Dark())
	result := &types.ToolResult{Output: "hello world", DurationMs: 100}
	got := r.RenderOutput(result, ToolSuccess, 100, false, false, 40)
	if got == "" {
		t.Error("RenderOutput should return non-empty string")
	}
}

func TestBaseRenderer_RenderOutput_Error(t *testing.T) {
	t.Parallel()
	r := RendererForTool("Bash", theme.Dark())
	result := &types.ToolResult{Output: "error output", Error: "command failed", DurationMs: 50}
	got := r.RenderOutput(result, ToolError, 50, false, false, 40)
	if got == "" {
		t.Error("RenderOutput error should return non-empty string")
	}
}

func TestBaseRenderer_RenderOutput_NilResult(t *testing.T) {
	t.Parallel()
	r := RendererForTool("Bash", theme.Dark())
	got := r.RenderOutput(nil, ToolRunning, 0, false, false, 40)
	if got != "" {
		t.Error("RenderOutput nil result should return empty string")
	}
}

func TestHighlightCode_PlainText(t *testing.T) {
	t.Parallel()
	got := HighlightCode("hello world", "", theme.Dark())
	if got == "" {
		t.Error("HighlightCode should return non-empty string")
	}
}

func TestHighlightCode_GoCode(t *testing.T) {
	t.Parallel()
	code := "package main\nfunc main() {}"
	got := HighlightCode(code, "go", theme.Dark())
	if got == "" {
		t.Error("HighlightCode for Go code should return non-empty string")
	}
}

func TestBarChart_Render(t *testing.T) {
	t.Parallel()
	bc := BarChart{
		Values:   []int{10, 20, 30},
		Labels:   []string{"A", "B", "C"},
		MaxWidth: 40,
	}
	got := bc.Render()
	if got == "" {
		t.Error("Render should return non-empty string")
	}
}

func TestBarChart_RenderEmpty(t *testing.T) {
	t.Parallel()
	bc := BarChart{Values: []int{}, Labels: []string{}}
	got := bc.Render()
	if got != "" {
		t.Error("Render empty should return empty string")
	}
}

func TestBarChart_RenderMismatchedLengths(t *testing.T) {
	t.Parallel()
	bc := BarChart{Values: []int{1, 2}, Labels: []string{"A"}}
	got := bc.Render()
	if got != "" {
		t.Error("Render mismatched lengths should return empty string")
	}
}

func TestSparkline_Render(t *testing.T) {
	t.Parallel()
	sl := Sparkline{Values: []int{1, 5, 3, 8, 2}, Width: 10, Theme: theme.Dark()}
	got := sl.Render()
	if got == "" {
		t.Error("Render should return non-empty string")
	}
}

func TestSparkline_RenderEmpty(t *testing.T) {
	t.Parallel()
	sl := Sparkline{Values: []int{}, Theme: theme.Dark()}
	got := sl.Render()
	if got != "" {
		t.Error("Render empty should return empty string")
	}
}

func TestSparkline_RenderWithLabel(t *testing.T) {
	t.Parallel()
	sl := Sparkline{Values: []int{1, 2, 3}, Label: "tokens", Theme: theme.Dark()}
	got := sl.Render()
	if got == "" {
		t.Error("Render with label should return non-empty string")
	}
}

func TestSparkline_RenderResample(t *testing.T) {
	t.Parallel()
	sl := Sparkline{Values: []int{1, 2, 3, 4, 5, 6, 7, 8, 9, 10}, Width: 3, Theme: theme.Dark()}
	got := sl.Render()
	if got == "" {
		t.Error("Render resampled should return non-empty string")
	}
}

func TestSparkline_RenderPadLeft(t *testing.T) {
	t.Parallel()
	sl := Sparkline{Values: []int{1, 2}, Width: 5, Theme: theme.Dark()}
	got := sl.Render()
	if got == "" {
		t.Error("Render padded should return non-empty string")
	}
}

func TestChips_Render(t *testing.T) {
	t.Parallel()
	fc := FilterChips{
		Chips: []FilterChip{
			{Label: "All", Active: true},
			{Label: "Files", Active: false},
			{Label: "Code", Active: false},
		},
		Theme: theme.Dark(),
	}
	got := fc.Render()
	if got == "" {
		t.Error("Render should return non-empty string")
	}
}

func TestChips_RenderEmpty(t *testing.T) {
	t.Parallel()
	fc := FilterChips{Chips: []FilterChip{}, Theme: theme.Dark()}
	got := fc.Render()
	if got != "" {
		t.Error("Render empty should return empty string")
	}
}

func TestChipGroup_Render(t *testing.T) {
	t.Parallel()
	cg := ChipGroup{
		Label: "Filter",
		Chips: FilterChips{
			Chips: []FilterChip{{Label: "A", Active: true}},
			Theme: theme.Dark(),
		},
		Theme: theme.Dark(),
	}
	got := cg.Render()
	if got == "" {
		t.Error("Render should return non-empty string")
	}
}

func TestChipGroup_RenderWithSeparator(t *testing.T) {
	t.Parallel()
	cg := ChipGroup{
		Label:     "Filter",
		Separator: true,
		Chips: FilterChips{
			Chips: []FilterChip{{Label: "A", Active: true}},
			Theme: theme.Dark(),
		},
		Theme: theme.Dark(),
	}
	got := cg.Render()
	if got == "" {
		t.Error("Render with separator should return non-empty string")
	}
}

func TestChipGroup_RenderEmptyLabel(t *testing.T) {
	t.Parallel()
	cg := ChipGroup{
		Label: "",
		Chips: FilterChips{
			Chips: []FilterChip{{Label: "A", Active: true}},
			Theme: theme.Dark(),
		},
		Theme: theme.Dark(),
	}
	got := cg.Render()
	if got == "" {
		t.Error("Render with empty label should return non-empty string")
	}
}

func TestConnectorColor(t *testing.T) {
	t.Parallel()
	c := connectorColor(theme.Dark())
	_ = c // just verify no panic
}

func TestToolCard_BinaryContent(t *testing.T) {
	t.Parallel()
	input := json.RawMessage(`{"command": "cat file"}`)
	result := &types.ToolResult{Output: "text\x00binary", DurationMs: 100}
	call := types.ToolCall{ID: "1", Name: "Bash", Input: input}
	tc := NewToolCard(call, result, ToolSuccess, theme.Dark())
	got := tc.Render(80)
	if got == "" {
		t.Error("Render with binary content should return non-empty string")
	}
}

func TestToolCard_LongInput(t *testing.T) {
	t.Parallel()
	longCmd := strings.Repeat("echo ", 20)
	input, _ := json.Marshal(map[string]string{"command": longCmd})
	call := types.ToolCall{ID: "1", Name: "Bash", Input: input}
	tc := NewToolCard(call, nil, ToolRunning, theme.Dark())
	got := tc.Render(80)
	if got == "" {
		t.Error("Render with long input should return non-empty string")
	}
}

func TestThinkingBlock_FocusedRender(t *testing.T) {
	t.Parallel()
	seg := types.MessageSegment{Type: "thinking", Content: "reasoning"}
	tb := NewThinkingBlock(seg, theme.Dark(), false, 0)
	tb.SetFocused(true)
	got := tb.Render(80)
	if got == "" {
		t.Error("Render focused should return non-empty string")
	}
}

func TestFormatDuration_Large(t *testing.T) {
	t.Parallel()
	got := FormatDuration(86400) // 24 hours
	if !strings.Contains(got, "24h") {
		t.Errorf("FormatDuration(86400) = %q, want to contain '24h'", got)
	}
}

// --- FormatTimeBar ---

func TestFormatTimeBar_Zero(t *testing.T) {
	t.Parallel()
	got := FormatTimeBar(time.Time{})
	if got != "" {
		t.Errorf("FormatTimeBar(zero) = %q, want empty", got)
	}
}

func TestFormatTimeBar_Valid(t *testing.T) {
	t.Parallel()
	ts := time.Date(2025, 6, 15, 14, 30, 0, 0, time.UTC)
	got := FormatTimeBar(ts)
	if got != "14:30" {
		t.Errorf("FormatTimeBar = %q, want 14:30", got)
	}
}

func TestFormatTimeBar_Midnight(t *testing.T) {
	t.Parallel()
	ts := time.Date(2025, 1, 1, 0, 0, 0, 0, time.UTC)
	got := FormatTimeBar(ts)
	if got != "00:00" {
		t.Errorf("FormatTimeBar = %q, want 00:00", got)
	}
}

func TestFormatTimeBar_EndOfDay(t *testing.T) {
	t.Parallel()
	ts := time.Date(2025, 1, 1, 23, 59, 0, 0, time.UTC)
	got := FormatTimeBar(ts)
	if got != "23:59" {
		t.Errorf("FormatTimeBar = %q, want 23:59", got)
	}
}

// --- FormatMetric edge cases ---

func TestFormatMetric_Zero(t *testing.T) {
	t.Parallel()
	got := FormatMetric(0)
	if !strings.Contains(got, "0") {
		t.Errorf("FormatMetric(0) = %q, want to contain 0", got)
	}
}

func TestFormatMetric_Large(t *testing.T) {
	t.Parallel()
	got := FormatMetric(2_500_000)
	if !strings.Contains(got, "M") {
		t.Errorf("FormatMetric(2500000) = %q, want M suffix", got)
	}
}

func TestFormatMetric_Small(t *testing.T) {
	t.Parallel()
	got := FormatMetric(10000)
	if !strings.Contains(got, "K") {
		t.Errorf("FormatMetric(10000) = %q, want K suffix", got)
	}
}

// --- FormatCost edge cases ---

func TestFormatCost_Zero(t *testing.T) {
	t.Parallel()
	got := FormatCost(0)
	if !strings.Contains(got, "$0") {
		t.Errorf("FormatCost(0) = %q, want $0", got)
	}
}

func TestFormatCost_FreeModel(t *testing.T) {
	t.Parallel()
	got := FormatCost(-1)
	if got == "" {
		t.Error("FormatCost(-1) should return non-empty")
	}
}

// --- FormatDuration edge cases ---

func TestFormatDuration_Zero(t *testing.T) {
	t.Parallel()
	got := FormatDuration(0)
	if !strings.Contains(got, "0") {
		t.Errorf("FormatDuration(0) = %q, want 0", got)
	}
}

func TestFormatDuration_JustSeconds(t *testing.T) {
	t.Parallel()
	got := FormatDuration(59)
	if strings.Contains(got, "m") {
		t.Errorf("FormatDuration(59) should not contain m, got %q", got)
	}
}

func TestFormatDuration_JustMinutes(t *testing.T) {
	t.Parallel()
	got := FormatDuration(60)
	if !strings.Contains(got, "1m") {
		t.Errorf("FormatDuration(60) = %q, want 1m", got)
	}
}

// --- NewBadge ---

func TestNewBadge(t *testing.T) {
	t.Parallel()
	th := theme.Dark()
	b := NewBadge("v1.0", BadgeSuccessPreset, th)
	if b.Label != "v1.0" {
		t.Errorf("badge label = %q, want v1.0", b.Label)
	}
}

func TestNewBadge_Presets(t *testing.T) {
	t.Parallel()
	th := theme.Dark()
	presets := []BadgePreset{
		BadgeSuccessPreset, BadgeWarningPreset, BadgeErrorPreset,
		BadgeInfoPreset, BadgeBrandPreset, BadgeMutedPreset,
	}
	for _, p := range presets {
		b := NewBadge("test", p, th)
		if b.Label != "test" {
			t.Errorf("preset %d: label = %q, want test", p, b.Label)
		}
	}
}

// --- RenderBadges ---

func TestRenderBadges_Empty(t *testing.T) {
	t.Parallel()
	got := RenderBadges([]Badge{})
	if got != "" {
		t.Errorf("RenderBadges(empty) = %q, want empty", got)
	}
}

func TestRenderBadges_WithBadges(t *testing.T) {
	t.Parallel()
	th := theme.Dark()
	badges := []Badge{
		NewBadge("fast", BadgeSuccessPreset, th),
		NewBadge("cheap", BadgeInfoPreset, th),
	}
	got := RenderBadges(badges)
	if got == "" {
		t.Error("RenderBadges should return non-empty")
	}
}

// --- StatusBadge ---

func TestStatusBadge(t *testing.T) {
	t.Parallel()
	th := theme.Dark()
	got := StatusBadge("online", th)
	if got.Label != "online" {
		t.Errorf("StatusBadge label = %q, want online", got.Label)
	}
}

func TestStatusBadge_Empty(t *testing.T) {
	t.Parallel()
	th := theme.Dark()
	got := StatusBadge("", th)
	if got.Label != "" {
		t.Errorf("StatusBadge(empty) label = %q, want empty", got.Label)
	}
}

func TestStatusBadge_Done(t *testing.T) {
	t.Parallel()
	th := theme.Dark()
	got := StatusBadge("done", th)
	if got.Label != "done" {
		t.Errorf("StatusBadge(done) label = %q", got.Label)
	}
}

// --- CapabilityBadge ---

func TestCapabilityBadge(t *testing.T) {
	t.Parallel()
	th := theme.Dark()
	got := CapabilityBadge("reasoning", th)
	if got.Label != "reasoning" {
		t.Errorf("CapabilityBadge label = %q, want reasoning", got.Label)
	}
}

// --- MetricRow ---

func TestMetricRow_Empty(t *testing.T) {
	t.Parallel()
	got := MetricRow([]MetricCard{}, 80)
	if got != "" {
		t.Errorf("MetricRow(empty) = %q, want empty", got)
	}
}
