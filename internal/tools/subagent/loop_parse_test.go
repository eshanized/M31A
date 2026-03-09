package subagent

import (
	"strings"
	"testing"
)

func TestParseToolCalls_EmptyContent(t *testing.T) {
	calls, err := parseToolCalls("", nil)
	if err != nil {
		t.Fatalf("unexpected err: %v", err)
	}
	if len(calls) != 0 {
		t.Fatalf("expected 0 calls, got %d", len(calls))
	}
}

func TestParseToolCalls_FencedBlock(t *testing.T) {
	content := "Here is a tool call:\n\n```json\n{\"name\":\"Grep\",\"input\":{\"pattern\":\"foo\"}}\n```\n"
	calls, err := parseToolCalls(content, nil)
	if err != nil {
		t.Fatalf("unexpected err: %v", err)
	}
	if len(calls) != 1 {
		t.Fatalf("expected 1 call, got %d", len(calls))
	}
	if calls[0].Name != "Grep" {
		t.Errorf("expected name=Grep, got %s", calls[0].Name)
	}
	if !strings.Contains(string(calls[0].Input), "foo") {
		t.Errorf("input should contain 'foo': %s", string(calls[0].Input))
	}
}

func TestParseToolCalls_ArrayInFence(t *testing.T) {
	content := "```\n[{\"name\":\"Glob\",\"params\":{\"pattern\":\"*.go\"}},{\"name\":\"FileRead\",\"input\":{\"path\":\"foo.go\"}}]\n```"
	calls, err := parseToolCalls(content, nil)
	if err != nil {
		t.Fatalf("unexpected err: %v", err)
	}
	if len(calls) != 2 {
		t.Fatalf("expected 2 calls, got %d", len(calls))
	}
	if calls[0].Name != "Glob" || calls[1].Name != "FileRead" {
		t.Errorf("unexpected names: %+v", calls)
	}
}

func TestParseToolCalls_InlineObject(t *testing.T) {
	content := "Just calling a tool: {\"name\":\"Bash\",\"input\":{\"command\":\"ls\"}}"
	calls, err := parseToolCalls(content, nil)
	if err != nil {
		t.Fatalf("unexpected err: %v", err)
	}
	if len(calls) != 1 {
		t.Fatalf("expected 1 call, got %d", len(calls))
	}
	if calls[0].Name != "Bash" {
		t.Errorf("expected Bash, got %s", calls[0].Name)
	}
}

func TestParseToolCalls_ToolAlias(t *testing.T) {
	content := "{\"tool\":\"Edit\",\"input\":{\"path\":\"x.go\"}}"
	calls, err := parseToolCalls(content, nil)
	if err != nil {
		t.Fatalf("unexpected err: %v", err)
	}
	if len(calls) != 1 {
		t.Fatalf("expected 1 call, got %d", len(calls))
	}
	if calls[0].Name != "Edit" {
		t.Errorf("expected Edit (from tool alias), got %s", calls[0].Name)
	}
}

func TestParseToolCalls_NoToolCalls(t *testing.T) {
	content := "Here's a summary of my findings: nothing to do."
	calls, err := parseToolCalls(content, nil)
	if err != nil {
		t.Fatalf("unexpected err: %v", err)
	}
	if len(calls) != 0 {
		t.Fatalf("expected 0 calls, got %d", len(calls))
	}
}

func TestParseToolCalls_ParamsFallback(t *testing.T) {
	content := "{\"name\":\"FileRead\",\"params\":{\"path\":\"a.go\"}}"
	calls, err := parseToolCalls(content, nil)
	if err != nil {
		t.Fatalf("unexpected err: %v", err)
	}
	if len(calls) != 1 {
		t.Fatalf("expected 1 call, got %d", len(calls))
	}
	if !strings.Contains(string(calls[0].Input), "a.go") {
		t.Errorf("expected input to carry params: %s", string(calls[0].Input))
	}
}

func TestExtractJSONObjects_Nested(t *testing.T) {
	in := `x {"a":{"b":1},"c":[2,3]} y {"d":4}`
	out := extractJSONObjects(in)
	if len(out) != 2 {
		t.Fatalf("expected 2 objects, got %d: %v", len(out), out)
	}
	if !strings.Contains(out[0], `"b":1`) {
		t.Errorf("first object missing nested: %s", out[0])
	}
}

func TestAbbreviate(t *testing.T) {
	cases := []struct {
		in   string
		n    int
		want string
	}{
		{"hello", 10, "hello"},
		{"hello world", 5, "hell…"},
		{"", 5, ""},
		{"x", 1, "x"},
	}
	for _, c := range cases {
		got := abbreviate(c.in, c.n)
		if got != c.want {
			t.Errorf("abbreviate(%q, %d) = %q; want %q", c.in, c.n, got, c.want)
		}
	}
}
