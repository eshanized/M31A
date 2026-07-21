package layout

import (
	"testing"

	"github.com/charmbracelet/lipgloss"
)

// --- Constraints tests ---

func TestUniformInsets(t *testing.T) {
	t.Parallel()
	insets := UniformInsets(5)
	if insets.Top != 5 || insets.Right != 5 || insets.Bottom != 5 || insets.Left != 5 {
		t.Errorf("UniformInsets(5) = %+v, want all 5", insets)
	}

	insets0 := UniformInsets(0)
	if insets0.Top != 0 || insets0.Right != 0 || insets0.Bottom != 0 || insets0.Left != 0 {
		t.Errorf("UniformInsets(0) = %+v, want all 0", insets0)
	}
}

func TestSymmetricInsets(t *testing.T) {
	t.Parallel()
	insets := SymmetricInsets(3, 7)
	if insets.Top != 3 || insets.Right != 7 || insets.Bottom != 3 || insets.Left != 7 {
		t.Errorf("SymmetricInsets(3, 7) = %+v", insets)
	}
}

func TestInsets_Horizontal(t *testing.T) {
	t.Parallel()
	insets := Insets{Left: 3, Right: 7}
	if insets.Horizontal() != 10 {
		t.Errorf("Horizontal() = %d, want 10", insets.Horizontal())
	}
}

func TestInsets_Vertical(t *testing.T) {
	t.Parallel()
	insets := Insets{Top: 2, Bottom: 8}
	if insets.Vertical() != 10 {
		t.Errorf("Vertical() = %d, want 10", insets.Vertical())
	}
}

func TestRenderFunc(t *testing.T) {
	t.Parallel()
	f := RenderFunc(func(w, h int) string {
		return "ok"
	})
	got := f.Render(10, 20)
	if got != "ok" {
		t.Errorf("RenderFunc.Render() = %q, want ok", got)
	}
}

func TestStaticRenderer(t *testing.T) {
	t.Parallel()
	s := StaticRenderer{Content: "hello"}
	got := s.Render(10, 20)
	if got != "hello" {
		t.Errorf("StaticRenderer.Render() = %q, want hello", got)
	}
}

func TestNewRow(t *testing.T) {
	t.Parallel()
	box := NewRow(&Box{Width: 10}, &Box{Width: 20})
	if box.Direction != Row {
		t.Errorf("NewRow direction = %d, want Row", box.Direction)
	}
	if len(box.Children) != 2 {
		t.Errorf("NewRow children = %d, want 2", len(box.Children))
	}
}

func TestNewColumn(t *testing.T) {
	t.Parallel()
	box := NewColumn(&Box{Width: 10}, &Box{Width: 20})
	if box.Direction != Column {
		t.Errorf("NewColumn direction = %d, want Column", box.Direction)
	}
	if len(box.Children) != 2 {
		t.Errorf("NewColumn children = %d, want 2", len(box.Children))
	}
}

func TestBox_WithContent(t *testing.T) {
	t.Parallel()
	b := &Box{}
	r := StaticRenderer{Content: "test"}
	b.WithContent(r)
	if b.Content.Render(0, 0) != "test" {
		t.Error("WithContent failed")
	}
}

func TestBox_WithFlex(t *testing.T) {
	t.Parallel()
	b := &Box{}
	b.WithFlex(3)
	if b.Flex != 3 {
		t.Errorf("WithFlex(3) = %d", b.Flex)
	}
}

func TestBox_WithSize(t *testing.T) {
	t.Parallel()
	b := &Box{}
	b.WithSize(100, 50)
	if b.Width != 100 || b.Height != 50 {
		t.Errorf("WithSize(100, 50) = (%d, %d)", b.Width, b.Height)
	}
}

func TestBox_WithPadding(t *testing.T) {
	t.Parallel()
	b := &Box{}
	b.WithPadding(5)
	if b.Padding.Top != 5 || b.Padding.Right != 5 || b.Padding.Bottom != 5 || b.Padding.Left != 5 {
		t.Errorf("WithPadding(5) = %+v", b.Padding)
	}
}

func TestBox_WithBorder(t *testing.T) {
	t.Parallel()
	b := &Box{}
	b.WithBorder(lipgloss.RoundedBorder(), lipgloss.Color("1"))
	// Just verify it doesn't panic and sets the field
	if b.BorderColor == "" {
		t.Error("WithBorder failed to set border color")
	}
}

func TestBox_WithBackground(t *testing.T) {
	t.Parallel()
	b := &Box{}
	b.WithBackground(lipgloss.Color("1"))
	if b.Background == "" {
		t.Error("WithBackground failed to set background")
	}
}

func TestBox_BuilderChain(t *testing.T) {
	t.Parallel()
	b := NewRow().
		WithSize(100, 50).
		WithFlex(2).
		WithPadding(1).
		WithBorder(lipgloss.RoundedBorder(), lipgloss.Color("1")).
		WithBackground(lipgloss.Color("2")).
		WithContent(StaticRenderer{Content: "hello"})

	if b.Width != 100 || b.Height != 50 {
		t.Errorf("chain size = (%d, %d)", b.Width, b.Height)
	}
	if b.Flex != 2 {
		t.Errorf("chain flex = %d", b.Flex)
	}
	if b.Padding.Top != 1 {
		t.Errorf("chain padding = %+v", b.Padding)
	}
}

// --- Box.go tests ---

func TestSplitHorizontal_ZeroCount(t *testing.T) {
	t.Parallel()
	result := SplitHorizontal(100, 0, 5)
	if result != nil {
		t.Errorf("SplitHorizontal(100, 0, 5) = %v, want nil", result)
	}
}

func TestSplitHorizontal_NegativeCount(t *testing.T) {
	t.Parallel()
	result := SplitHorizontal(100, -1, 5)
	if result != nil {
		t.Errorf("SplitHorizontal(100, -1, 5) = %v, want nil", result)
	}
}

func TestSplitHorizontal_NoFixed(t *testing.T) {
	t.Parallel()
	result := SplitHorizontal(100, 4, 0)
	if len(result) != 4 {
		t.Fatalf("len = %d, want 4", len(result))
	}
	for i, w := range result {
		if w != 25 {
			t.Errorf("result[%d] = %d, want 25", i, w)
		}
	}
}

func TestSplitHorizontal_WithGap(t *testing.T) {
	t.Parallel()
	// 100 width, 3 children, gap 10: totalGap=20, remaining=80
	// Each gets 26 (80/3 integer division), last gets 26+2 remainder = 28
	result := SplitHorizontal(100, 3, 10)
	if len(result) != 3 {
		t.Fatalf("len = %d, want 3", len(result))
	}
	expected := []int{26, 26, 28}
	for i, w := range result {
		if w != expected[i] {
			t.Errorf("result[%d] = %d, want %d", i, w, expected[i])
		}
	}
}

func TestSplitHorizontal_WithFixed(t *testing.T) {
	t.Parallel()
	result := SplitHorizontal(100, 3, 0, 30, 0, 0)
	if len(result) != 3 {
		t.Fatalf("len = %d, want 3", len(result))
	}
	if result[0] != 30 {
		t.Errorf("result[0] = %d, want 30 (fixed)", result[0])
	}
	// remaining = 100 - 30 = 70, flexCount = 2, each = 35
	for i := 1; i < 3; i++ {
		if result[i] != 35 {
			t.Errorf("result[%d] = %d, want 35", i, result[i])
		}
	}
}

func TestSplitHorizontal_AllFixed(t *testing.T) {
	t.Parallel()
	result := SplitHorizontal(100, 3, 0, 20, 30, 40)
	if result[0] != 20 || result[1] != 30 || result[2] != 40 {
		t.Errorf("all fixed = %v, want [20 30 40]", result)
	}
}

func TestSplitHorizontal_NegativeRemaining(t *testing.T) {
	t.Parallel()
	// Fixed exceeds total
	result := SplitHorizontal(50, 2, 0, 30, 30)
	if result[0] != 30 || result[1] != 30 {
		t.Errorf("fixed overflow = %v", result)
	}
}

func TestSplitHorizontal_SingleChild(t *testing.T) {
	t.Parallel()
	result := SplitHorizontal(100, 1, 10)
	if len(result) != 1 {
		t.Fatalf("len = %d, want 1", len(result))
	}
	if result[0] != 100 {
		t.Errorf("single child = %d, want 100", result[0])
	}
}

func TestSplitVertical_ZeroCount(t *testing.T) {
	t.Parallel()
	result := SplitVertical(100, 0, 5)
	if result != nil {
		t.Errorf("SplitVertical(100, 0, 5) = %v, want nil", result)
	}
}

func TestSplitVertical_NoFixed(t *testing.T) {
	t.Parallel()
	result := SplitVertical(100, 4, 0)
	if len(result) != 4 {
		t.Fatalf("len = %d, want 4", len(result))
	}
	for i, h := range result {
		if h != 25 {
			t.Errorf("result[%d] = %d, want 25", i, h)
		}
	}
}

func TestSplitVertical_WithFixed(t *testing.T) {
	t.Parallel()
	result := SplitVertical(100, 3, 0, 30, 0, 0)
	if result[0] != 30 {
		t.Errorf("result[0] = %d, want 30", result[0])
	}
	for i := 1; i < 3; i++ {
		if result[i] != 35 {
			t.Errorf("result[%d] = %d, want 35", i, result[i])
		}
	}
}

func TestFitContent_Empty(t *testing.T) {
	t.Parallel()
	result := FitContent("", 10, 3)
	lines := splitLines(result)
	if len(lines) != 3 {
		t.Errorf("FitContent empty: lines = %d, want 3", len(lines))
	}
	for i, line := range lines {
		if len(line) != 10 {
			t.Errorf("line %d width = %d, want 10", i, len(line))
		}
	}
}

func TestFitContent_Truncate(t *testing.T) {
	t.Parallel()
	result := FitContent("hello world", 5, 1)
	lines := splitLines(result)
	if len(lines) != 1 {
		t.Fatalf("lines = %d, want 1", len(lines))
	}
	// truncated content may contain ANSI reset, check width
	if len(lines[0]) < 5 {
		t.Errorf("truncated width = %d, want >= 5", len(lines[0]))
	}
}

func TestFitContent_Pad(t *testing.T) {
	t.Parallel()
	result := FitContent("hi", 10, 1)
	lines := splitLines(result)
	if len(lines[0]) != 10 {
		t.Errorf("padded width = %d, want 10", len(lines[0]))
	}
}

func TestFitContent_MultipleLines(t *testing.T) {
	t.Parallel()
	result := FitContent("a\nb\nc\nd", 5, 3)
	lines := splitLines(result)
	if len(lines) != 3 {
		t.Errorf("lines = %d, want 3 (clipped)", len(lines))
	}
}

func TestFitContent_PaddingLines(t *testing.T) {
	t.Parallel()
	result := FitContent("a", 5, 3)
	lines := splitLines(result)
	if len(lines) != 3 {
		t.Errorf("lines = %d, want 3 (padded)", len(lines))
	}
}

func TestRenderBox(t *testing.T) {
	t.Parallel()
	result := RenderBox("hello", 20, 5)
	if result == "" {
		t.Error("RenderBox returned empty")
	}
}

func TestRenderBox_WithBorder(t *testing.T) {
	t.Parallel()
	result := RenderBox("content", 30, 5,
		WithBoxBorder(lipgloss.RoundedBorder(), lipgloss.Color("1")),
		WithBoxPadding(1),
		WithBoxBackground(lipgloss.Color("2")),
	)
	if result == "" {
		t.Error("RenderBox with options returned empty")
	}
}

func TestRenderCard(t *testing.T) {
	t.Parallel()
	result := RenderCard("Title", "content", 30, lipgloss.RoundedBorder(), lipgloss.Color("1"), lipgloss.Color("2"))
	if result == "" {
		t.Error("RenderCard returned empty")
	}

	// No title
	result2 := RenderCard("", "content", 30, lipgloss.RoundedBorder(), lipgloss.Color("1"), lipgloss.Color("2"))
	if result2 == "" {
		t.Error("RenderCard without title returned empty")
	}
}

func TestAlignConstants(t *testing.T) {
	t.Parallel()
	if AlignStart != 0 || AlignCenter != 1 || AlignEnd != 2 {
		t.Error("Align constants have wrong values")
	}
}

func TestFlexDirectionConstants(t *testing.T) {
	t.Parallel()
	if Column != 0 || Row != 1 {
		t.Error("FlexDirection constants have wrong values")
	}
}

// helper
func splitLines(s string) []string {
	if s == "" {
		return nil
	}
	return splitString(s, "\n")
}

func splitString(s, sep string) []string {
	if s == "" {
		return nil
	}
	parts := []string{}
	start := 0
	for i := 0; i+len(sep) <= len(s); i++ {
		if s[i:i+len(sep)] == sep {
			parts = append(parts, s[start:i])
			start = i + len(sep)
			i += len(sep) - 1
		}
	}
	parts = append(parts, s[start:])
	return parts
}
