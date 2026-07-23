package components_test

import (
	"github.com/eshanized/M31A/internal/ui/tui/components"
	"strings"
	"testing"
	"time"

	"github.com/charmbracelet/lipgloss"
)

// --- Truncate tests (not covered by extra_test.go) ---

func TestTruncateWithEllipsis(t *testing.T) {
	t.Parallel()
	tests := []struct {
		name     string
		input    string
		maxWidth int
		want     string
	}{
		{"empty string", "", 10, ""},
		{"maxWidth 0", "hello", 0, ""},
		{"maxWidth negative", "hello", -1, ""},
		{"short string no truncation", "hi", 10, "hi"},
		{"exact fit", "hello", 5, "hello"},
		{"needs truncation", "hello world", 8, "hello..."},
		{"very narrow", "hello", 3, "..."},
		{"two chars fit", "ab", 5, "ab"},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := components.TruncateWithEllipsis(tt.input, tt.maxWidth)
			if got != tt.want {
				t.Errorf("components.TruncateWithEllipsis(%q, %d) = %q, want %q", tt.input, tt.maxWidth, got, tt.want)
			}
		})
	}
}

func TestTruncateMiddle(t *testing.T) {
	t.Parallel()
	tests := []struct {
		name   string
		input  string
		maxLen int
		want   string
	}{
		{"empty", "", 10, ""},
		{"maxLen 0", "hello", 0, ""},
		{"short no truncation", "hi", 10, "hi"},
		{"exact fit", "hello", 5, "hello"},
		{"needs truncation", "hello world", 7, "he...ld"},
		{"maxLen 4", "hello world", 4, "hell"},
		{"maxLen 3", "hello world", 3, "hel"},
		{"maxLen 1", "hello", 1, "h"},
		{"maxLen 2", "hello", 2, "he"},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := components.TruncateMiddle(tt.input, tt.maxLen)
			if got != tt.want {
				t.Errorf("components.TruncateMiddle(%q, %d) = %q, want %q", tt.input, tt.maxLen, got, tt.want)
			}
		})
	}
}

func TestTruncateError(t *testing.T) {
	t.Parallel()
	tests := []struct {
		name  string
		input string
	}{
		{"short message", "error occurred"},
		{"empty", ""},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := components.TruncateError(tt.input)
			if got != tt.input {
				t.Errorf("components.TruncateError(%q) = %q, want %q", tt.input, got, tt.input)
			}
		})
	}

	// Long message
	long := string(make([]rune, 250))
	got := components.TruncateError(long)
	if !strings.HasSuffix(got, "[...]") {
		t.Error("TruncateError long message should end with [...]")
	}
	if len([]rune(got)) != 205 {
		t.Errorf("TruncateError long result length = %d, want 205", len([]rune(got)))
	}

	// Exactly 200 runes
	exact := string(make([]rune, 200))
	got2 := components.TruncateError(exact)
	if got2 != exact {
		t.Error("TruncateError exact 200 runes should return unchanged")
	}
}

// --- MetricCard additional tests ---

func TestMetricCard_Render_NoTheme(t *testing.T) {
	t.Parallel()
	m := components.MetricCard{Value: "42", Label: "Tests"}
	result := m.Render()
	if result == "" {
		t.Error("MetricCard.Render() with zero-value theme returned empty string")
	}
}

func TestMetricCard_Render_CenterAlign(t *testing.T) {
	t.Parallel()
	m := components.MetricCard{Value: "10", Label: "Count", Width: 30, Align: lipgloss.Center}
	result := m.Render()
	if result == "" {
		t.Error("MetricCard.Render() center align returned empty string")
	}
}

func TestMetricCard_Render_RightAlign(t *testing.T) {
	t.Parallel()
	m := components.MetricCard{Value: "10", Label: "Count", Width: 30, Align: lipgloss.Right}
	result := m.Render()
	if result == "" {
		t.Error("MetricCard.Render() right align returned empty string")
	}
}

func TestMetricCard_Render_NegativeTrend(t *testing.T) {
	t.Parallel()
	m := components.MetricCard{Value: "5", Label: "Errors", Trend: "-3"}
	result := m.Render()
	if result == "" {
		t.Error("MetricCard.Render() with negative trend returned empty string")
	}
}

func TestMetricCard_Render_DownArrowTrend(t *testing.T) {
	t.Parallel()
	m := components.MetricCard{Value: "5", Label: "Errors", Trend: "↓ 3"}
	result := m.Render()
	if result == "" {
		t.Error("MetricCard.Render() with down-arrow trend returned empty string")
	}
}

func TestMetricRow_NilInput(t *testing.T) {
	t.Parallel()
	result := components.MetricRow(nil, 100)
	if result != "" {
		t.Errorf("components.MetricRow(nil, 100) = %q, want empty", result)
	}
}

func TestMetricRow_Single(t *testing.T) {
	t.Parallel()
	result := components.MetricRow([]components.MetricCard{{Value: "10", Label: "A"}}, 100)
	if result == "" {
		t.Error("MetricRow single metric returned empty")
	}
}

func TestMetricRow_Multiple(t *testing.T) {
	t.Parallel()
	result := components.MetricRow([]components.MetricCard{
		{Value: "10", Label: "A"},
		{Value: "20", Label: "B"},
	}, 80)
	if result == "" {
		t.Error("MetricRow multiple metrics returned empty")
	}
}

func TestMetricRow_NarrowWidth(t *testing.T) {
	t.Parallel()
	result := components.MetricRow([]components.MetricCard{{Value: "10", Label: "A"}}, 5)
	if result == "" {
		t.Error("MetricRow narrow width returned empty")
	}
}

// --- Spinner additional tests ---

func TestNewSpinner(t *testing.T) {
	t.Parallel()
	s := components.NewSpinner()
	if s.Tick() != 100*time.Millisecond {
		t.Errorf("components.NewSpinner().Tick() = %v, want 100ms", s.Tick())
	}
}

func TestNewSpinnerWithFrames(t *testing.T) {
	t.Parallel()
	frames := []string{"a", "b", "c"}
	s := components.NewSpinnerWithFrames(frames, 200*time.Millisecond)
	if s.Tick() != 200*time.Millisecond {
		t.Errorf("Tick = %v, want 200ms", s.Tick())
	}
	if s.Peek() != "a" {
		t.Errorf("Peek = %q, want a", s.Peek())
	}
}

func TestSpinner_WrapAround(t *testing.T) {
	t.Parallel()
	frames := []string{"a", "b"}
	s := components.NewSpinnerWithFrames(frames, 100*time.Millisecond)
	f1 := s.Next()
	f2 := s.Next()
	f3 := s.Next()
	if f1 != "a" || f2 != "b" || f3 != "a" {
		t.Errorf("Wrap around failed: got %q, %q, %q", f1, f2, f3)
	}
}

func TestRenderSpinner_Empty(t *testing.T) {
	t.Parallel()
	result := components.RenderSpinner("", lipgloss.NewStyle())
	if result != "" {
		t.Errorf("components.RenderSpinner(\"\") = %q, want empty", result)
	}
}

func TestSpinnerTickInterval(t *testing.T) {
	t.Parallel()
	if components.SpinnerTickInterval != 100*time.Millisecond {
		t.Errorf("components.SpinnerTickInterval = %v, want 100ms", components.SpinnerTickInterval)
	}
}

func TestOpenCodeFrames(t *testing.T) {
	t.Parallel()
	if len(components.OpenCodeFrames) != 10 {
		t.Errorf("len(components.OpenCodeFrames) = %d, want 10", len(components.OpenCodeFrames))
	}
}

func TestThinkingFrames(t *testing.T) {
	t.Parallel()
	if len(components.ThinkingFrames) != len(components.OpenCodeFrames) {
		t.Errorf("len(components.ThinkingFrames) = %d, want %d", len(components.ThinkingFrames), len(components.OpenCodeFrames))
	}
}

func TestSpinnerSets(t *testing.T) {
	t.Parallel()
	if len(components.SpinnerSets) < 7 {
		t.Errorf("len(components.SpinnerSets) = %d, want >= 7", len(components.SpinnerSets))
	}
}

// --- FilterChips additional tests ---

func TestFilterChips_Render_Empty(t *testing.T) {
	t.Parallel()
	f := components.FilterChips{}
	if f.Render() != "" {
		t.Error("Empty FilterChips.Render() should return empty")
	}
}

func TestFilterChips_Render_WithSelected(t *testing.T) {
	t.Parallel()
	f := components.FilterChips{
		Chips: []components.FilterChip{
			{Label: "A", Active: true},
			{Label: "B", Active: false},
		},
		Selected: 1,
	}
	result := f.Render()
	if result == "" {
		t.Error("FilterChips.Render() with selected returned empty")
	}
}

// --- Dropdown additional tests ---

func TestDropdown_ViewClosedNoItems(t *testing.T) {
	t.Parallel()
	dd := components.Dropdown{Items: []components.DropdownItem{}, Open: false}
	if dd.View() != "" {
		t.Error("Empty closed Dropdown.View() should return empty")
	}
}

// --- VirtualViewport tests ---

func TestVirtualViewport_TotalHeight(t *testing.T) {
	t.Parallel()
	v := components.VirtualViewport{
		Items:     []string{"line1", "line2\nline3", "line4"},
		ViewportH: 5,
		ViewportW: 80,
	}
	got := v.TotalHeight()
	if got != 4 {
		t.Errorf("TotalHeight() = %d, want 4", got)
	}
}

func TestVirtualViewport_TotalHeight_Empty(t *testing.T) {
	t.Parallel()
	v := components.VirtualViewport{}
	got := v.TotalHeight()
	if got != 0 {
		t.Errorf("TotalHeight() = %d, want 0", got)
	}
}

func TestVirtualViewport_SetScroll(t *testing.T) {
	t.Parallel()
	v := components.VirtualViewport{
		Items:     []string{"a", "b", "c", "d", "e"},
		ViewportH: 3,
		ViewportW: 80,
	}
	v.SetScroll(1)
	if v.ScrollTop != 1 {
		t.Errorf("SetScroll(1) scrollTop = %d, want 1", v.ScrollTop)
	}

	v.SetScroll(10)
	if v.ScrollTop != 2 {
		t.Errorf("SetScroll(10) scrollTop = %d, want 2", v.ScrollTop)
	}

	v.SetScroll(-5)
	if v.ScrollTop != 0 {
		t.Errorf("SetScroll(-5) scrollTop = %d, want 0", v.ScrollTop)
	}
}

func TestVirtualViewport_ScrollBy(t *testing.T) {
	t.Parallel()
	v := components.VirtualViewport{
		Items:     []string{"a", "b", "c"},
		ViewportH: 2,
		ViewportW: 80,
		ScrollTop: 0,
	}
	v.ScrollBy(1)
	if v.ScrollTop != 1 {
		t.Errorf("ScrollBy(1) scrollTop = %d, want 1", v.ScrollTop)
	}

	v.ScrollBy(-1)
	if v.ScrollTop != 0 {
		t.Errorf("ScrollBy(-1) scrollTop = %d, want 0", v.ScrollTop)
	}
}

func TestVirtualViewport_View(t *testing.T) {
	t.Parallel()
	v := components.VirtualViewport{
		Items:     []string{"line1", "line2", "line3"},
		ViewportH: 2,
		ViewportW: 20,
	}
	result := v.View()
	if result == "" {
		t.Error("VirtualViewport.View() returned empty")
	}

	// Empty items
	v2 := components.VirtualViewport{ViewportH: 5, ViewportW: 80}
	if v2.View() != "" {
		t.Error("Empty VirtualViewport.View() should return empty")
	}

	// Zero viewport height
	v3 := components.VirtualViewport{Items: []string{"a"}, ViewportH: 0}
	if v3.View() != "" {
		t.Error("Zero height VirtualViewport.View() should return empty")
	}
}

func TestVirtualViewport_ScrollRatio(t *testing.T) {
	t.Parallel()
	v := components.VirtualViewport{
		Items:     []string{"a", "b", "c", "d", "e"},
		ViewportH: 3,
		ViewportW: 80,
	}
	r := v.ScrollRatio()
	if r != 0 {
		t.Errorf("ScrollRatio() at 0 = %f, want 0", r)
	}

	v.ScrollTop = 2
	r = v.ScrollRatio()
	if r != 1.0 {
		t.Errorf("ScrollRatio() at max = %f, want 1.0", r)
	}

	// No scroll possible
	v2 := components.VirtualViewport{Items: []string{"a"}, ViewportH: 5}
	if v2.ScrollRatio() != 0 {
		t.Errorf("ScrollRatio() when no scroll = %f, want 0", v2.ScrollRatio())
	}
}

func TestVirtualViewport_ScrollToTopBottom(t *testing.T) {
	t.Parallel()
	v := components.VirtualViewport{
		Items:     []string{"a", "b", "c", "d", "e"},
		ViewportH: 2,
		ViewportW: 80,
		ScrollTop: 1,
	}
	v.ScrollToTop()
	if v.ScrollTop != 0 {
		t.Errorf("ScrollToTop() scrollTop = %d, want 0", v.ScrollTop)
	}

	v.ScrollToBottom()
	if v.ScrollTop != 3 {
		t.Errorf("ScrollToBottom() scrollTop = %d, want 3", v.ScrollTop)
	}
}

func TestVirtualViewport_InvalidateHeightCache(t *testing.T) {
	t.Parallel()
	v := components.VirtualViewport{
		Items:     []string{"a\nb", "c"},
		ViewportH: 5,
		ViewportW: 80,
	}
	h1 := v.TotalHeight() // cache it
	v.InvalidateHeightCache()
	h2 := v.TotalHeight() // should recompute
	if h1 != h2 {
		t.Errorf("TotalHeight changed after InvalidateHeightCache: %d != %d", h1, h2)
	}
}

func TestVirtualViewport_ViewWithScroll(t *testing.T) {
	t.Parallel()
	v := components.VirtualViewport{
		Items:     []string{"line1", "line2", "line3", "line4", "line5"},
		ViewportH: 2,
		ViewportW: 80,
		ScrollTop: 2,
	}
	result := v.View()
	if result == "" {
		t.Error("VirtualViewport.View() with scroll returned empty")
	}
}
