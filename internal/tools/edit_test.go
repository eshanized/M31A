package tools

import (
	"net"
	"strings"
	"testing"
)

func TestLevenshteinDistance_EmptyStrings(t *testing.T) {
	t.Parallel()
	if d := levenshteinDistance("", ""); d != 0 {
		t.Errorf("distance('', '') = %d, want 0", d)
	}
}

func TestLevenshteinDistance_OneEmpty(t *testing.T) {
	t.Parallel()
	if d := levenshteinDistance("abc", ""); d != 3 {
		t.Errorf("distance('abc', '') = %d, want 3", d)
	}
	if d := levenshteinDistance("", "abc"); d != 3 {
		t.Errorf("distance('', 'abc') = %d, want 3", d)
	}
}

func TestLevenshteinDistance_Identical(t *testing.T) {
	t.Parallel()
	if d := levenshteinDistance("hello", "hello"); d != 0 {
		t.Errorf("distance('hello', 'hello') = %d, want 0", d)
	}
}

func TestLevenshteinDistance_SingleCharDiff(t *testing.T) {
	t.Parallel()
	if d := levenshteinDistance("cat", "bat"); d != 1 {
		t.Errorf("distance('cat', 'bat') = %d, want 1", d)
	}
}

func TestLevenshteinDistance_CompleteMismatch(t *testing.T) {
	t.Parallel()
	if d := levenshteinDistance("abc", "xyz"); d != 3 {
		t.Errorf("distance('abc', 'xyz') = %d, want 3", d)
	}
}

func TestLevenshteinDistance_Insertion(t *testing.T) {
	t.Parallel()
	if d := levenshteinDistance("kitten", "sitting"); d != 3 {
		t.Errorf("distance('kitten', 'sitting') = %d, want 3", d)
	}
}

func TestLevenshteinSimilarity_Identical(t *testing.T) {
	t.Parallel()
	if s := levenshteinSimilarity("hello", "hello"); s != 1.0 {
		t.Errorf("similarity('hello', 'hello') = %f, want 1.0", s)
	}
}

func TestLevenshteinSimilarity_Empty(t *testing.T) {
	t.Parallel()
	if s := levenshteinSimilarity("", ""); s != 1.0 {
		t.Errorf("similarity('', '') = %f, want 1.0", s)
	}
	if s := levenshteinSimilarity("abc", ""); s != 0.0 {
		t.Errorf("similarity('abc', '') = %f, want 0.0", s)
	}
}

func TestLevenshteinSimilarity_Similar(t *testing.T) {
	t.Parallel()
	s := levenshteinSimilarity("kitten", "sitting")
	if s < 0.5 || s > 0.6 {
		t.Errorf("similarity('kitten', 'sitting') = %f, expected ~0.57", s)
	}
}

func TestLeadingWhitespace(t *testing.T) {
	t.Parallel()
	tests := []struct {
		input string
		want  string
	}{
		{"", ""},
		{"hello", ""},
		{"  hello", "  "},
		{"\thello", "\t"},
		{"  \thello", "  \t"},
		{"    ", "    "},
	}
	for _, tt := range tests {
		got := leadingWhitespace(tt.input)
		if got != tt.want {
			t.Errorf("leadingWhitespace(%q) = %q, want %q", tt.input, got, tt.want)
		}
	}
}

func TestDetectLineEnding(t *testing.T) {
	t.Parallel()
	tests := []struct {
		input string
		want  string
	}{
		{"hello\nworld", "\n"},
		{"hello\r\nworld", "\r\n"},
		{"no newlines", "\n"},
		{"", "\n"},
	}
	for _, tt := range tests {
		got := detectLineEnding(tt.input)
		if got != tt.want {
			t.Errorf("detectLineEnding(%q) = %q, want %q", tt.input, got, tt.want)
		}
	}
}

func TestToInt(t *testing.T) {
	t.Parallel()
	tests := []struct {
		name  string
		input any
		want  int
		ok    bool
	}{
		{"int", 42, 42, true},
		{"float64", float64(3.7), 3, true},
		{"int64", int64(100), 100, true},
		{"string", "42", 0, false},
		{"nil", nil, 0, false},
		{"bool", true, 0, false},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got, ok := toInt(tt.input)
			if ok != tt.ok {
				t.Errorf("toInt(%v) ok = %v, want %v", tt.input, ok, tt.ok)
			}
			if ok && got != tt.want {
				t.Errorf("toInt(%v) = %d, want %d", tt.input, got, tt.want)
			}
		})
	}
}

func TestGenerateDiffSummary(t *testing.T) {
	t.Parallel()
	old := "line1\nline2\nline3"
	new := "line1\nmodified\nline3"
	summary := generateDiffSummary("test.txt", old, new)
	if summary == "" {
		t.Error("expected non-empty diff summary")
	}
}

func TestReplaceByLineRange_Valid(t *testing.T) {
	t.Parallel()
	content := "line1\nline2\nline3\nline4\nline5"
	result, err := replaceByLineRange(content, 2, 3, "REPLACED")
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	expected := "line1\nREPLACED\nline4\nline5"
	if result != expected {
		t.Errorf("got %q, want %q", result, expected)
	}
}

func TestReplaceByLineRange_OutOfRange(t *testing.T) {
	t.Parallel()
	content := "line1\nline2"
	_, err := replaceByLineRange(content, 0, 1, "x")
	if err == nil {
		t.Error("expected error for start_line < 1")
	}
	_, err = replaceByLineRange(content, 1, 5, "x")
	if err == nil {
		t.Error("expected error for end_line > len(lines)")
	}
}

func TestReplaceByLineRange_SingleLine(t *testing.T) {
	t.Parallel()
	content := "line1\nline2\nline3"
	result, err := replaceByLineRange(content, 2, 2, "REPLACED")
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if result != "line1\nREPLACED\nline3" {
		t.Errorf("got %q", result)
	}
}

func TestCascadingReplace_ExactMatch(t *testing.T) {
	t.Parallel()
	content := "hello world\nfoo bar"
	result, strategy, err := cascadingReplace(content, "foo bar", "baz qux")
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if strategy != "exact-match" {
		t.Errorf("strategy = %q, want 'exact-match'", strategy)
	}
	if result != "hello world\nbaz qux" {
		t.Errorf("got %q", result)
	}
}

func TestCascadingReplace_NoMatch(t *testing.T) {
	t.Parallel()
	content := "hello world"
	_, _, err := cascadingReplace(content, "nonexistent pattern", "replacement")
	if err == nil {
		t.Error("expected error when no strategy matches")
	}
}

func TestLineTrimmedReplace_Match(t *testing.T) {
	t.Parallel()
	content := "  hello\n  world\n  foo"
	contentLines := strings.Split(content, "\n")
	result, err := lineTrimmedReplace(content, contentLines, "hello\nworld", "REPLACED")
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if result == "" {
		t.Error("expected non-empty result")
	}
}

func TestWhitespaceNormalizedReplace_Match(t *testing.T) {
	t.Parallel()
	content := "hello    world\nfoo   bar"
	contentLines := strings.Split(content, "\n")
	result, err := whitespaceNormalizedReplace(content, contentLines, "hello world\nfoo bar", "REPLACED")
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if result != "REPLACED" {
		t.Errorf("got %q, want 'REPLACED'", result)
	}
}

func TestFuzzyAnchorReplace_Match(t *testing.T) {
	t.Parallel()
	content := "first line\nsecond line\nthird line\nfourth line\nfifth line"
	oldStr := "first line\nsecond line\nthird line"
	newStr := "REPLACED"
	contentLines := strings.Split(content, "\n")
	result, err := fuzzyAnchorReplace(content, contentLines, oldStr, newStr)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if result != "REPLACED\nfourth line\nfifth line" {
		t.Errorf("got %q", result)
	}
}

func TestFuzzyAnchorReplace_TooFewLines(t *testing.T) {
	t.Parallel()
	content := "one\ntwo"
	contentLines := strings.Split(content, "\n")
	_, err := fuzzyAnchorReplace(content, contentLines, "one\ntwo", "new")
	if err == nil {
		t.Error("expected error for fewer than MinLinesForFuzzy lines")
	}
}

func TestStatusIcon(t *testing.T) {
	t.Parallel()
	tests := []struct {
		status string
		want   string
	}{
		{"completed", "[x]"},
		{"in_progress", "[~]"},
		{"cancelled", "[-]"},
		{"pending", "[ ]"},
		{"unknown", "[ ]"},
	}
	for _, tt := range tests {
		got := statusIcon(tt.status)
		if got != tt.want {
			t.Errorf("statusIcon(%q) = %q, want %q", tt.status, got, tt.want)
		}
	}
}

func TestHumanSize(t *testing.T) {
	t.Parallel()
	tests := []struct {
		bytes int64
		want  string
	}{
		{0, "0 B"},
		{500, "500 B"},
		{1024, "1.0 KB"},
		{1048576, "1.0 MB"},
		{1073741824, "1.0 GB"},
	}
	for _, tt := range tests {
		got := humanSize(tt.bytes)
		if got != tt.want {
			t.Errorf("humanSize(%d) = %q, want %q", tt.bytes, got, tt.want)
		}
	}
}

func TestIsPrivateIP(t *testing.T) {
	t.Parallel()
	tests := []struct {
		ip   string
		want bool
	}{
		{"127.0.0.1", true},
		{"10.0.0.1", true},
		{"172.16.0.1", true},
		{"192.168.1.1", true},
		{"169.254.169.254", true},
		{"8.8.8.8", false},
		{"1.1.1.1", false},
		{"0.0.0.0", true},
	}
	for _, tt := range tests {
		ip := net.ParseIP(tt.ip)
		got := isPrivateIP(ip)
		if got != tt.want {
			t.Errorf("isPrivateIP(%q) = %v, want %v", tt.ip, got, tt.want)
		}
	}
}

func TestSetVersion_GetVersion(t *testing.T) {
	SetVersion("1.2.3")
	if got := getVersion(); got != "1.2.3" {
		t.Errorf("getVersion() = %q, want '1.2.3'", got)
	}
}

func TestDecodeHTMLEntities(t *testing.T) {
	t.Parallel()
	tests := []struct {
		input string
		want  string
	}{
		{"&amp;", "&"},
		{"&lt;", "<"},
		{"&gt;", ">"},
		{"&quot;", "\""},
		{"&#39;", "'"},
		{"&nbsp;", " "},
		{"no entities", "no entities"},
		{"&amp;&lt;&gt;", "&<>"},
	}
	for _, tt := range tests {
		got := decodeHTMLEntities(tt.input)
		if got != tt.want {
			t.Errorf("decodeHTMLEntities(%q) = %q, want %q", tt.input, got, tt.want)
		}
	}
}

func TestStripAllTags(t *testing.T) {
	t.Parallel()
	tests := []struct {
		input string
		want  string
	}{
		{"<p>hello</p>", "hello"},
		{"<div><span>nested</span></div>", "nested"},
		{"no tags", "no tags"},
		{"", ""},
		{"<br/>self closing", "self closing"},
	}
	for _, tt := range tests {
		got := stripAllTags(tt.input)
		if got != tt.want {
			t.Errorf("stripAllTags(%q) = %q, want %q", tt.input, got, tt.want)
		}
	}
}

func TestNormalizeWhitespace(t *testing.T) {
	t.Parallel()
	tests := []struct {
		input string
		want  string
	}{
		{"  hello   world  ", "hello world"},
		{"a\n\n\n\nb", "a\n\nb"},
		{"\t\ttabs\t\t", "tabs"},
		{"", ""},
	}
	for _, tt := range tests {
		got := normalizeWhitespace(tt.input)
		if got != tt.want {
			t.Errorf("normalizeWhitespace(%q) = %q, want %q", tt.input, got, tt.want)
		}
	}
}
