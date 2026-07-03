package layout

import (
	"strings"
	"testing"

	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/tui/theme"
)

// --- Solver coverage: solveRow, solveColumn, alignVertical, alignHorizontal ---
// These are 0% because existing tests only call Solve with leaf boxes or
// pre-rendered content. We need boxes WITH children to exercise solver.go.

func TestSolveRow_FixedWidthChildren(t *testing.T) {
	t.Parallel()
	// Two fixed-width children in a row
	box := NewRow(
		&Box{Width: 10, Content: RenderFunc(func(w, h int) string { return "left" })},
		&Box{Width: 15, Content: RenderFunc(func(w, h int) string { return "right" })},
	)
	box.Width = 30
	box.Height = 3
	got := Solve(box)
	if got == "" {
		t.Error("solveRow with fixed children should produce output")
	}
}

func TestSolveRow_FlexChildren(t *testing.T) {
	t.Parallel()
	// Flex children share remaining space
	box := NewRow(
		&Box{Flex: 1, Content: RenderFunc(func(w, h int) string { return "a" })},
		&Box{Flex: 2, Content: RenderFunc(func(w, h int) string { return "b" })},
	)
	box.Width = 60
	box.Height = 3
	got := Solve(box)
	if got == "" {
		t.Error("solveRow with flex children should produce output")
	}
}

func TestSolveRow_FixedAndFlex(t *testing.T) {
	t.Parallel()
	// Mix of fixed and flex children
	box := NewRow(
		&Box{Width: 10, Content: RenderFunc(func(w, h int) string { return "fixed" })},
		&Box{Flex: 1, Content: RenderFunc(func(w, h int) string { return "flex" })},
	)
	box.Width = 40
	box.Height = 3
	got := Solve(box)
	if got == "" {
		t.Error("solveRow mixed should produce output")
	}
}

func TestSolveRow_WithGap(t *testing.T) {
	t.Parallel()
	box := NewRow(
		&Box{Width: 10, Content: RenderFunc(func(w, h int) string { return "a" })},
		&Box{Width: 10, Content: RenderFunc(func(w, h int) string { return "b" })},
	)
	box.Width = 40
	box.Height = 3
	box.Gap = 2
	got := Solve(box)
	if got == "" {
		t.Error("solveRow with gap should produce output")
	}
}

func TestSolveRow_WithAlign(t *testing.T) {
	t.Parallel()
	for _, align := range []Align{AlignStart, AlignCenter, AlignEnd} {
		box := NewRow(
			&Box{Width: 10, Content: RenderFunc(func(w, h int) string { return "a" })},
			&Box{Width: 10, Content: RenderFunc(func(w, h int) string { return "b" })},
		)
		box.Width = 40
		box.Height = 5
		box.Align = align
		got := Solve(box)
		if got == "" {
			t.Errorf("solveRow align=%d should produce output", align)
		}
	}
}

func TestSolveRow_SkipsZeroWidthFlex(t *testing.T) {
	t.Parallel()
	// Children with flex=0 and width=0 should be skipped
	box := NewRow(
		&Box{Flex: 0, Width: 0, Content: RenderFunc(func(w, h int) string { return "skip" })},
		&Box{Width: 10, Content: RenderFunc(func(w, h int) string { return "keep" })},
	)
	box.Width = 20
	box.Height = 3
	got := Solve(box)
	if got == "" {
		t.Error("solveRow with zero-size flex children should still produce output")
	}
}

func TestSolveRow_WithChildHeight(t *testing.T) {
	t.Parallel()
	// Children with explicit heights
	box := NewRow(
		&Box{Width: 10, Height: 3, Content: RenderFunc(func(w, h int) string { return "a" })},
		&Box{Width: 10, Height: 5, Content: RenderFunc(func(w, h int) string { return "b" })},
	)
	box.Width = 30
	box.Height = 8
	got := Solve(box)
	if got == "" {
		t.Error("solveRow with child heights should produce output")
	}
}

func TestSolveRow_FlexRemainder(t *testing.T) {
	t.Parallel()
	// Force integer truncation remainder distribution
	// 100 - 0 fixed = 100 remaining, 3 children flex=1 each -> 33 each, remainder 1
	box := NewRow(
		&Box{Flex: 1, Content: RenderFunc(func(w, h int) string { return "a" })},
		&Box{Flex: 1, Content: RenderFunc(func(w, h int) string { return "b" })},
		&Box{Flex: 1, Content: RenderFunc(func(w, h int) string { return "c" })},
	)
	box.Width = 100
	box.Height = 3
	got := Solve(box)
	if got == "" {
		t.Error("solveRow flex remainder should produce output")
	}
}

func TestSolveColumn_FixedHeightChildren(t *testing.T) {
	t.Parallel()
	box := NewColumn(
		&Box{Height: 3, Content: RenderFunc(func(w, h int) string { return "top" })},
		&Box{Height: 5, Content: RenderFunc(func(w, h int) string { return "bottom" })},
	)
	box.Width = 20
	box.Height = 10
	got := Solve(box)
	if got == "" {
		t.Error("solveColumn with fixed children should produce output")
	}
}

func TestSolveColumn_FlexChildren(t *testing.T) {
	t.Parallel()
	box := NewColumn(
		&Box{Flex: 1, Content: RenderFunc(func(w, h int) string { return "a" })},
		&Box{Flex: 3, Content: RenderFunc(func(w, h int) string { return "b" })},
	)
	box.Width = 20
	box.Height = 60
	got := Solve(box)
	if got == "" {
		t.Error("solveColumn with flex children should produce output")
	}
}

func TestSolveColumn_FixedAndFlex(t *testing.T) {
	t.Parallel()
	box := NewColumn(
		&Box{Height: 3, Content: RenderFunc(func(w, h int) string { return "fixed" })},
		&Box{Flex: 1, Content: RenderFunc(func(w, h int) string { return "flex" })},
	)
	box.Width = 20
	box.Height = 20
	got := Solve(box)
	if got == "" {
		t.Error("solveColumn mixed should produce output")
	}
}

func TestSolveColumn_WithGap(t *testing.T) {
	t.Parallel()
	box := NewColumn(
		&Box{Height: 3, Content: RenderFunc(func(w, h int) string { return "a" })},
		&Box{Height: 3, Content: RenderFunc(func(w, h int) string { return "b" })},
	)
	box.Width = 20
	box.Height = 10
	box.Gap = 2
	got := Solve(box)
	if got == "" {
		t.Error("solveColumn with gap should produce output")
	}
}

func TestSolveColumn_WithAlign(t *testing.T) {
	t.Parallel()
	for _, align := range []Align{AlignStart, AlignCenter, AlignEnd} {
		box := NewColumn(
			&Box{Height: 2, Content: RenderFunc(func(w, h int) string { return "a" })},
			&Box{Height: 2, Content: RenderFunc(func(w, h int) string { return "b" })},
		)
		box.Width = 20
		box.Height = 10
		box.Align = align
		got := Solve(box)
		if got == "" {
			t.Errorf("solveColumn align=%d should produce output", align)
		}
	}
}

func TestSolveColumn_SkipsZeroHeightFlex(t *testing.T) {
	t.Parallel()
	box := NewColumn(
		&Box{Flex: 0, Height: 0, Content: RenderFunc(func(w, h int) string { return "skip" })},
		&Box{Height: 3, Content: RenderFunc(func(w, h int) string { return "keep" })},
	)
	box.Width = 20
	box.Height = 10
	got := Solve(box)
	if got == "" {
		t.Error("solveColumn with zero-size children should still produce output")
	}
}

func TestSolveColumn_WithChildWidth(t *testing.T) {
	t.Parallel()
	box := NewColumn(
		&Box{Width: 15, Height: 3, Content: RenderFunc(func(w, h int) string { return "wide" })},
		&Box{Width: 10, Height: 3, Content: RenderFunc(func(w, h int) string { return "narrow" })},
	)
	box.Width = 30
	box.Height = 10
	got := Solve(box)
	if got == "" {
		t.Error("solveColumn with child widths should produce output")
	}
}

func TestSolveColumn_FlexRemainder(t *testing.T) {
	t.Parallel()
	// 80 height, 0 fixed, 3 flex=1 -> 26 each, remainder 2
	box := NewColumn(
		&Box{Flex: 1, Content: RenderFunc(func(w, h int) string { return "a" })},
		&Box{Flex: 1, Content: RenderFunc(func(w, h int) string { return "b" })},
		&Box{Flex: 1, Content: RenderFunc(func(w, h int) string { return "c" })},
	)
	box.Width = 20
	box.Height = 80
	got := Solve(box)
	if got == "" {
		t.Error("solveColumn flex remainder should produce output")
	}
}

// Nested row-in-column to exercise solveBox recursion
func TestSolve_NestedRowInColumn(t *testing.T) {
	t.Parallel()
	box := NewColumn(
		&Box{
			Direction: Row,
			Children: []*Box{
				{Width: 10, Content: RenderFunc(func(w, h int) string { return "a" })},
				{Width: 10, Content: RenderFunc(func(w, h int) string { return "b" })},
			},
		},
		&Box{Height: 3, Content: RenderFunc(func(w, h int) string { return "c" })},
	)
	box.Width = 30
	box.Height = 10
	got := Solve(box)
	if got == "" {
		t.Error("nested row-in-column should produce output")
	}
}

func TestSolve_NestedColumnInRow(t *testing.T) {
	t.Parallel()
	box := NewRow(
		&Box{
			Direction: Column,
			Children: []*Box{
				{Height: 2, Content: RenderFunc(func(w, h int) string { return "top" })},
				{Height: 2, Content: RenderFunc(func(w, h int) string { return "bottom" })},
			},
		},
		&Box{Width: 10, Content: RenderFunc(func(w, h int) string { return "side" })},
	)
	box.Width = 40
	box.Height = 6
	got := Solve(box)
	if got == "" {
		t.Error("nested column-in-row should produce output")
	}
}

// --- Stack coverage: RenderStack, RenderDimmed, RenderDimmedOverlay ---
// These are 0% because existing tests only test RenderModalOverlay.

func TestRenderStack_Basic(t *testing.T) {
	t.Parallel()
	layers := []StackLayer{
		{Content: "AAAA\nBBBB", X: 0, Y: 0},
		{Content: "XX", X: 2, Y: 1},
	}
	got := RenderStack(6, 2, layers)
	lines := strings.Split(got, "\n")
	if len(lines) != 2 {
		t.Fatalf("expected 2 lines, got %d", len(lines))
	}
	// First line: AAXXXA (first 6 chars of AAAA + XX overlay at col 2)
	if !strings.HasPrefix(lines[0], "AA") {
		t.Errorf("line 0 should start with AA, got %q", lines[0])
	}
	// Second line should contain BB at the start
	if !strings.HasPrefix(lines[1], "BB") {
		t.Errorf("line 1 should start with BB, got %q", lines[1])
	}
}

func TestRenderStack_SingleLayerFullSize(t *testing.T) {
	t.Parallel()
	layers := []StackLayer{
		{Content: "hello", X: 0, Y: 0},
	}
	got := RenderStack(5, 1, layers)
	if got != "hello" {
		t.Errorf("expected 'hello', got %q", got)
	}
}

func TestRenderStack_LayerOutOfBounds(t *testing.T) {
	t.Parallel()
	layers := []StackLayer{
		{Content: "X", X: 0, Y: 100}, // Y way out of bounds
	}
	got := RenderStack(5, 3, layers)
	// Should still produce grid without crashing
	lines := strings.Split(got, "\n")
	if len(lines) != 3 {
		t.Fatalf("expected 3 lines, got %d", len(lines))
	}
	for _, line := range lines {
		if strings.Contains(line, "X") {
			t.Error("out-of-bounds layer should not appear in output")
		}
	}
}

func TestRenderStack_NegativeXY(t *testing.T) {
	t.Parallel()
	layers := []StackLayer{
		{Content: "X", X: -1, Y: -1},
	}
	got := RenderStack(5, 3, layers)
	lines := strings.Split(got, "\n")
	if len(lines) != 3 {
		t.Fatalf("expected 3 lines, got %d", len(lines))
	}
}

func TestRenderStack_WideContent(t *testing.T) {
	t.Parallel()
	layers := []StackLayer{
		{Content: "ABCDEFGHIJ", X: 3, Y: 0},
	}
	got := RenderStack(5, 1, layers)
	// Content extends beyond width, should clip
	if strings.Contains(got, "J") {
		t.Error("content beyond width should be clipped")
	}
}

func TestRenderStack_Empty(t *testing.T) {
	t.Parallel()
	got := RenderStack(3, 2, nil)
	lines := strings.Split(got, "\n")
	if len(lines) != 2 {
		t.Fatalf("expected 2 lines, got %d", len(lines))
	}
	for _, line := range lines {
		if line != "   " {
			t.Errorf("empty stack should be spaces, got %q", line)
		}
	}
}

func TestRenderDimmed_Basic(t *testing.T) {
	t.Parallel()
	th := theme.Dark()
	got := RenderDimmed("hello", th)
	// Should contain ANSI faint escape code
	if !strings.Contains(got, "\x1b[2m") {
		t.Error("RenderDimmed should contain ANSI faint escape code")
	}
	if !strings.Contains(got, "hello") {
		t.Error("RenderDimmed should contain original text")
	}
}

func TestRenderDimmed_MultiLine(t *testing.T) {
	t.Parallel()
	th := theme.Dark()
	got := RenderDimmed("line1\nline2\nline3", th)
	lines := strings.Split(got, "\n")
	if len(lines) != 3 {
		t.Fatalf("expected 3 dimmed lines, got %d", len(lines))
	}
	for i, line := range lines {
		if !strings.Contains(line, "\x1b[2m") {
			t.Errorf("line %d should contain faint escape", i)
		}
	}
}

func TestRenderDimmedOverlay_Basic(t *testing.T) {
	t.Parallel()
	th := theme.Dark()
	got := RenderDimmedOverlay("base content here", "overlay", 20, 5, th)
	if got == "" {
		t.Error("RenderDimmedOverlay should produce output")
	}
	// Should contain the overlay text
	if !strings.Contains(got, "overlay") {
		t.Error("RenderDimmedOverlay output should contain overlay text")
	}
}

func TestRenderDimmedOverlay_LongBase(t *testing.T) {
	t.Parallel()
	th := theme.Dark()
	got := RenderDimmedOverlay("long base content that spans multiple words", "X", 30, 3, th)
	if got == "" {
		t.Error("RenderDimmedOverlay with long base should produce output")
	}
}

func TestRenderStack_OverlayOnTop(t *testing.T) {
	t.Parallel()
	// Two layers: second should overwrite first
	layers := []StackLayer{
		{Content: "AAAA", X: 0, Y: 0},
		{Content: "BB", X: 1, Y: 0},
	}
	got := RenderStack(4, 1, layers)
	// Expected: ABBA (A at 0, B at 1-2, A at 3)
	if got != "ABBA" {
		t.Errorf("expected 'ABBA', got %q", got)
	}
}

func TestRenderStack_MultipleRows(t *testing.T) {
	t.Parallel()
	layers := []StackLayer{
		{Content: "AAAA\nBBBB\nCCCC", X: 0, Y: 0},
		{Content: "XX\nYY", X: 1, Y: 1},
	}
	got := RenderStack(4, 3, layers)
	lines := strings.Split(got, "\n")
	if len(lines) != 3 {
		t.Fatalf("expected 3 lines, got %d", len(lines))
	}
	// Line 1 should have XX overlay
	if !strings.Contains(lines[1], "XX") {
		t.Errorf("line 1 should contain XX overlay, got %q", lines[1])
	}
}

// --- SolveBox coverage (23.1% -> higher) ---

func TestSolveBox_WithPaddingAndChildren(t *testing.T) {
	t.Parallel()
	box := NewRow(
		&Box{Width: 10, Content: RenderFunc(func(w, h int) string { return "padded" })},
	)
	box.Width = 30
	box.Height = 5
	box.Padding = UniformInsets(2)
	got := Solve(box)
	if got == "" {
		t.Error("solveBox with padding and children should produce output")
	}
}

func TestSolveBox_NegativeInnerSize(t *testing.T) {
	t.Parallel()
	// Padding exceeds available space -> inner goes to 0
	box := NewRow(
		&Box{Width: 5, Content: RenderFunc(func(w, h int) string { return "x" })},
	)
	box.Width = 5
	box.Height = 1
	box.Padding = UniformInsets(10)
	got := Solve(box)
	// Should not crash, even with negative inner dimensions
	_ = got
}

func TestSolveBox_ColumnWithPadding(t *testing.T) {
	t.Parallel()
	box := NewColumn(
		&Box{Height: 3, Content: RenderFunc(func(w, h int) string { return "a" })},
	)
	box.Width = 20
	box.Height = 10
	box.Padding = SymmetricInsets(1, 2)
	got := Solve(box)
	if got == "" {
		t.Error("solveBox column with padding should produce output")
	}
}

// --- RenderBox with background and border ---

func TestRenderBox_BackgroundOnly(t *testing.T) {
	t.Parallel()
	got := RenderBox("content", 15, 3, WithBoxBackground(lipgloss.Color("#333333")))
	if got == "" {
		t.Error("RenderBox with background only should produce output")
	}
}

func TestRenderBox_AllOptions(t *testing.T) {
	t.Parallel()
	got := RenderBox("everything", 25, 5,
		WithBoxBorder(lipgloss.DoubleBorder(), lipgloss.Color("#FF0000")),
		WithBoxPadding(1),
		WithBoxBackground(lipgloss.Color("#000000")),
	)
	if got == "" {
		t.Error("RenderBox with all options should produce output")
	}
}

// --- RenderCard variations ---

func TestRenderCard_NarrowWidth(t *testing.T) {
	t.Parallel()
	got := RenderCard("Title", "Body", 10, lipgloss.RoundedBorder(), lipgloss.Color("#FF0000"), lipgloss.Color("#000000"))
	if got == "" {
		t.Error("RenderCard narrow should produce output")
	}
}

func TestRenderCard_LongTitle(t *testing.T) {
	t.Parallel()
	got := RenderCard("A Very Long Title That Might Wrap", "body", 30, lipgloss.RoundedBorder(), lipgloss.Color("#FF0000"), lipgloss.Color("#000000"))
	if got == "" {
		t.Error("RenderCard long title should produce output")
	}
}
