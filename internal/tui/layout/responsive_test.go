package layout

import "testing"

// Additional tests beyond what page_test.go covers

func TestDetect_BoundaryValues(t *testing.T) {
	t.Parallel()
	tests := []struct {
		width int
		want  Breakpoint
	}{
		{-1, UltraNarrow},
		{39, UltraNarrow},
		{40, Compact},
		{59, Compact},
		{60, Standard},
		{79, Standard},
		{80, Full},
		{200, Full},
	}
	for _, tt := range tests {
		got := Detect(tt.width)
		if got != tt.want {
			t.Errorf("Detect(%d) = %d, want %d", tt.width, got, tt.want)
		}
	}
}

func TestShowFooterCost(t *testing.T) {
	t.Parallel()
	if ShowFooterCost(79) {
		t.Error("ShowFooterCost(79) should be false")
	}
	if !ShowFooterCost(80) {
		t.Error("ShowFooterCost(80) should be true")
	}
}

func TestBreakpointConstants(t *testing.T) {
	t.Parallel()
	if MinWidth != 40 {
		t.Errorf("MinWidth = %d, want 40", MinWidth)
	}
	if CompactMax != 59 {
		t.Errorf("CompactMax = %d, want 59", CompactMax)
	}
	if StandardMax != 79 {
		t.Errorf("StandardMax = %d, want 79", StandardMax)
	}
}

func TestShowSidebar_Boundary(t *testing.T) {
	t.Parallel()
	// MinWidth*2 = 80
	if ShowSidebar(MinWidth*2 - 1) {
		t.Errorf("ShowSidebar(%d) should be false", MinWidth*2-1)
	}
	if !ShowSidebar(MinWidth * 2) {
		t.Errorf("ShowSidebar(%d) should be true", MinWidth*2)
	}
}
