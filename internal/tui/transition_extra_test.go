package tui

import (
	"strings"
	"testing"
)

func TestPadFrameLines(t *testing.T) {
	tests := []struct {
		name    string
		lines   []string
		width   int
		height  int
		wantLen int
	}{
		{"empty", nil, 10, 5, 5},
		{"fewer lines than height", []string{"hello", "world"}, 10, 5, 5},
		{"exact lines", []string{"a", "b", "c"}, 5, 3, 3},
		{"more lines than height", []string{"a", "b", "c", "d", "e"}, 5, 3, 3},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			result := padFrameLines(tt.lines, tt.width, tt.height)
			if len(result) != tt.wantLen {
				t.Errorf("padFrameLines() length = %d, want %d", len(result), tt.wantLen)
			}
		})
	}
}

func TestPadFrameLines_Width(t *testing.T) {
	result := padFrameLines([]string{"hello"}, 10, 1)
	if len(result) != 1 {
		t.Fatalf("expected 1 line, got %d", len(result))
	}
	// Line should be padded to width
	if len(result[0]) < 10 {
		t.Errorf("line length = %d, want >= 10", len(result[0]))
	}
}

func TestSliceVisibleTail(t *testing.T) {
	tests := []struct {
		name string
		s    string
		n    int
		want string
	}{
		{"zero n", "hello", 0, ""},
		{"negative n", "hello", -1, ""},
		{"full string", "hello", 5, "hello"},
		{"shorter n", "hello", 3, "llo"},
		{"n larger than string", "hi", 5, "   hi"},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := sliceVisibleTail(tt.s, tt.n)
			if got != tt.want {
				t.Errorf("sliceVisibleTail(%q, %d) = %q, want %q", tt.s, tt.n, got, tt.want)
			}
		})
	}
}

func TestSliceVisibleHead(t *testing.T) {
	reset := "\x1b[0m"
	tests := []struct {
		name string
		s    string
		n    int
		want string
	}{
		{"zero n", "hello", 0, ""},
		{"negative n", "hello", -1, ""},
		{"full string", "hello", 5, "hello"},
		{"shorter n", "hello", 3, "hel" + reset},
		{"n larger than string", "hi", 5, "hi"},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := sliceVisibleHead(tt.s, tt.n)
			if got != tt.want {
				t.Errorf("sliceVisibleHead(%q, %d) = %q, want %q", tt.s, tt.n, got, tt.want)
			}
		})
	}
}

func TestTransitionTruncate(t *testing.T) {
	reset := "\x1b[0m"
	tests := []struct {
		name string
		s    string
		maxW int
		want string
	}{
		{"empty", "", 5, ""},
		{"short string", "hi", 5, "hi"},
		{"exact length", "hello", 5, "hello"},
		{"truncation", "hello world", 5, "hello" + reset},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := transitionTruncate(tt.s, tt.maxW)
			if got != tt.want {
				t.Errorf("transitionTruncate(%q, %d) = %q, want %q", tt.s, tt.maxW, got, tt.want)
			}
		})
	}
}

func TestTransitionTruncate_WithANSI(t *testing.T) {
	styled := "\x1b[31mhello\x1b[0m"
	got := transitionTruncate(styled, 3)
	// Should preserve ANSI codes and truncate to 3 visible chars
	if !strings.Contains(got, "hel") {
		t.Errorf("transitionTruncate() = %q, should contain 'hel'", got)
	}
	if !strings.Contains(got, "\x1b[31m") {
		t.Errorf("transitionTruncate() = %q, should preserve ANSI codes", got)
	}
}

func TestSkipVisible(t *testing.T) {
	tests := []struct {
		name string
		s    string
		n    int
		want string
	}{
		{"zero skip", "hello", 0, "hello"},
		{"skip all", "hello", 5, ""},
		{"skip partial", "hello", 2, "llo"},
		{"skip more than length", "hi", 5, ""},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := skipVisible(tt.s, tt.n)
			if got != tt.want {
				t.Errorf("skipVisible(%q, %d) = %q, want %q", tt.s, tt.n, got, tt.want)
			}
		})
	}
}

func TestSkipVisible_WithANSI(t *testing.T) {
	styled := "\x1b[31mhello\x1b[0m"
	got := skipVisible(styled, 2)
	// Should skip 2 visible chars but preserve ANSI codes from the skipped region
	if !strings.Contains(got, "llo") {
		t.Errorf("skipVisible() = %q, should contain 'llo'", got)
	}
	// The last active style from the skipped region should be applied to output
	if !strings.Contains(got, "\x1b[31m") {
		t.Errorf("skipVisible() = %q, should preserve last active ANSI style", got)
	}
}

func TestBlendLine(t *testing.T) {
	tests := []struct {
		name      string
		line      string
		blendChar rune
		width     int
	}{
		{"simple", "hello", '█', 10},
		{"with spaces", "hello world", '░', 20},
		{"empty", "", '▒', 5},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := blendLine(tt.line, tt.blendChar, tt.width)
			if got == "" {
				t.Error("blendLine() returned empty string")
			}
		})
	}
}

func TestBlendLine_PreservesSpaces(t *testing.T) {
	got := blendLine("a b", '█', 10)
	// Spaces should be preserved
	if !strings.Contains(got, " ") {
		t.Errorf("blendLine() = %q, should preserve spaces", got)
	}
}

func TestIsBackNavigation(t *testing.T) {
	tests := []struct {
		name string
		from Screen
		to   Screen
		want bool
	}{
		{"to REPL from other", ScreenSettings, ScreenREPL, true},
		{"to REPL from REPL", ScreenREPL, ScreenREPL, false},
		{"forward in workflow", ScreenREPL, ScreenPlan, false},
		{"backward in workflow", ScreenPlan, ScreenREPL, true},
		{"settings to REPL", ScreenSettings, ScreenREPL, true},
		{"REPL to settings", ScreenREPL, ScreenSettings, false},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			if got := isBackNavigation(tt.from, tt.to); got != tt.want {
				t.Errorf("isBackNavigation(%d, %d) = %v, want %v", tt.from, tt.to, got, tt.want)
			}
		})
	}
}

func TestRenderSlide(t *testing.T) {
	// Test leftward slide
	result := renderSlide("old", "new", 0.5, 10, 1, true)
	if result == "" {
		t.Error("renderSlide() returned empty string")
	}

	// Test rightward slide
	result = renderSlide("old", "new", 0.5, 10, 1, false)
	if result == "" {
		t.Error("renderSlide() returned empty string")
	}
}

func TestRenderFade(t *testing.T) {
	// Test low progress (more prev)
	result := renderFade("old", "new", 0.25, 10, 1)
	if result == "" {
		t.Error("renderFade() returned empty string at low progress")
	}

	// Test high progress (more next)
	result = renderFade("old", "new", 0.75, 10, 1)
	if result == "" {
		t.Error("renderFade() returned empty string at high progress")
	}
}
