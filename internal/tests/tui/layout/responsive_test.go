package layout_test

import (
	"testing"

	"github.com/eshanized/M31A/internal/ui/tui/layout"
)

// Additional tests beyond what page_test.go covers

func TestDetect_BoundaryValues(t *testing.T) {
	t.Parallel()
	tests := []struct {
		width int
		want  layout.Breakpoint
	}{
		{-1, layout.UltraNarrow},
		{39, layout.UltraNarrow},
		{40, layout.Compact},
		{59, layout.Compact},
		{60, layout.Standard},
		{79, layout.Standard},
		{80, layout.Full},
		{119, layout.Full},
		{120, layout.UltraWide},
		{200, layout.UltraWide},
	}
	for _, tt := range tests {
		got := layout.Detect(tt.width)
		if got != tt.want {
			t.Errorf("layout.Detect(%d) = %d, want %d", tt.width, got, tt.want)
		}
	}
}

func TestShowFooterCost(t *testing.T) {
	t.Parallel()
	if layout.ShowFooterCost(79) {
		t.Error("layout.ShowFooterCost(79) should be false")
	}
	if !layout.ShowFooterCost(80) {
		t.Error("layout.ShowFooterCost(80) should be true")
	}
}

func TestBreakpointConstants(t *testing.T) {
	t.Parallel()
	if layout.MinWidth != 40 {
		t.Errorf("layout.MinWidth = %d, want 40", layout.MinWidth)
	}
	if layout.CompactMax != 59 {
		t.Errorf("layout.CompactMax = %d, want 59", layout.CompactMax)
	}
	if layout.StandardMax != 79 {
		t.Errorf("layout.StandardMax = %d, want 79", layout.StandardMax)
	}
}

func TestShowSidebar_Boundary(t *testing.T) {
	t.Parallel()
	// layout.MinWidth*2 = 80
	if layout.ShowSidebar(layout.MinWidth*2 - 1) {
		t.Errorf("layout.ShowSidebar(%d) should be false", layout.MinWidth*2-1)
	}
	if !layout.ShowSidebar(layout.MinWidth * 2) {
		t.Errorf("layout.ShowSidebar(%d) should be true", layout.MinWidth*2)
	}
}
