package a11y_test

import (
	"github.com/eshanized/M31A/internal/ui/tui/a11y"
	"testing"
)

// --- Terminal type constants ---

func TestTerminalType_Constants(t *testing.T) {
	t.Parallel()
	if a11y.TermUnknown != 0 {
		t.Errorf("a11y.TermUnknown = %d, want 0", a11y.TermUnknown)
	}
	if a11y.TermITerm2 != 1 {
		t.Errorf("a11y.TermITerm2 = %d, want 1", a11y.TermITerm2)
	}
	if a11y.TermKitty != 2 {
		t.Errorf("a11y.TermKitty = %d, want 2", a11y.TermKitty)
	}
	if a11y.TermWezTerm != 3 {
		t.Errorf("a11y.TermWezTerm = %d, want 3", a11y.TermWezTerm)
	}
	if a11y.TermAlacritty != 4 {
		t.Errorf("a11y.TermAlacritty = %d, want 4", a11y.TermAlacritty)
	}
	if a11y.TermWindowsTerminal != 5 {
		t.Errorf("a11y.TermWindowsTerminal = %d, want 5", a11y.TermWindowsTerminal)
	}
	if a11y.TermTMux != 6 {
		t.Errorf("a11y.TermTMux = %d, want 6", a11y.TermTMux)
	}
	if a11y.TermSSH != 7 {
		t.Errorf("a11y.TermSSH = %d, want 7", a11y.TermSSH)
	}
	if a11y.TermGeneric != 8 {
		t.Errorf("a11y.TermGeneric = %d, want 8", a11y.TermGeneric)
	}
}

// --- DetectTerminal ---

func TestDetectTerminal_CachesResult(t *testing.T) {
	t.Parallel()
	// First call sets the cached value
	term1 := a11y.DetectTerminal()
	// Second call should return the same cached value
	term2 := a11y.DetectTerminal()
	if term1 != term2 {
		t.Errorf("a11y.DetectTerminal() returned different values: %d vs %d", term1, term2)
	}
}

func TestDetectTerminal_NeverUnknown(t *testing.T) {
	t.Parallel()
	term := a11y.DetectTerminal()
	if term == a11y.TermUnknown {
		t.Error("a11y.DetectTerminal() should never return a11y.TermUnknown; fallback is a11y.TermGeneric")
	}
}

// --- SupportsOSC1337 ---

func TestSupportsOSC1337_Consistent(t *testing.T) {
	t.Parallel()
	// Multiple calls should return the same value
	v1 := a11y.SupportsOSC1337()
	v2 := a11y.SupportsOSC1337()
	if v1 != v2 {
		t.Errorf("a11y.SupportsOSC1337() returned different values: %v vs %v", v1, v2)
	}
}

// --- SupportsSemanticLabels ---

func TestSupportsSemanticLabels_Consistent(t *testing.T) {
	t.Parallel()
	v1 := a11y.SupportsSemanticLabels()
	v2 := a11y.SupportsSemanticLabels()
	if v1 != v2 {
		t.Errorf("a11y.SupportsSemanticLabels() returned different values: %v vs %v", v1, v2)
	}
}

// --- SupportsRegions ---

func TestSupportsRegions_Consistent(t *testing.T) {
	t.Parallel()
	v1 := a11y.SupportsRegions()
	v2 := a11y.SupportsRegions()
	if v1 != v2 {
		t.Errorf("a11y.SupportsRegions() returned different values: %v vs %v", v1, v2)
	}
}

// --- Announce ---

func TestAnnounce_ReturnsString(t *testing.T) {
	t.Parallel()
	result := a11y.Announce("test message")
	// Result is either empty (unsupported) or a formatted string
	_ = result
}

func TestAnnounce_EmptyText(t *testing.T) {
	t.Parallel()
	result := a11y.Announce("")
	// Should not panic with empty text
	_ = result
}

func TestAnnounce_LongText(t *testing.T) {
	t.Parallel()
	longText := make([]byte, 1000)
	for i := range longText {
		longText[i] = 'a'
	}
	result := a11y.Announce(string(longText))
	// Should not panic with long text
	_ = result
}

// --- DescribeElement ---

func TestDescribeElement_ReturnsString(t *testing.T) {
	t.Parallel()
	result := a11y.DescribeElement("button", "button")
	_ = result
}

func TestDescribeElement_EmptyRole(t *testing.T) {
	t.Parallel()
	result := a11y.DescribeElement("text", "")
	// Empty role should default to "text"
	_ = result
}

func TestDescribeElement_EmptyLabel(t *testing.T) {
	t.Parallel()
	result := a11y.DescribeElement("", "button")
	// Should not panic with empty label
	_ = result
}

// --- RegionStart ---

func TestRegionStart_ReturnsString(t *testing.T) {
	t.Parallel()
	result := a11y.RegionStart("sidebar")
	_ = result
}

func TestRegionStart_EmptyName(t *testing.T) {
	t.Parallel()
	result := a11y.RegionStart("")
	// Should not panic with empty name
	_ = result
}

// --- RegionEnd ---

func TestRegionEnd_ReturnsString(t *testing.T) {
	t.Parallel()
	result := a11y.RegionEnd("sidebar")
	_ = result
}

func TestRegionEnd_EmptyName(t *testing.T) {
	t.Parallel()
	result := a11y.RegionEnd("")
	// Should not panic with empty name
	_ = result
}

// --- RegionStart/RegionEnd pair ---

func TestRegionStartEnd_MatchingNames(t *testing.T) {
	t.Parallel()
	start := a11y.RegionStart("main")
	end := a11y.RegionEnd("main")
	// Both should either be empty (unsupported) or non-empty (supported)
	if (start == "") != (end == "") {
		t.Error("RegionStart and RegionEnd should both be empty or both non-empty")
	}
}

// --- Unicode and special characters ---

func TestAnnounce_Unicode(t *testing.T) {
	t.Parallel()
	result := a11y.Announce("Hello 世界 🌍")
	// Should not panic with unicode
	_ = result
}

func TestDescribeElement_SpecialChars(t *testing.T) {
	t.Parallel()
	result := a11y.DescribeElement("my-button_123", "interactive")
	// Should not panic with special characters
	_ = result
}

func TestRegionStart_SpecialChars(t *testing.T) {
	t.Parallel()
	result := a11y.RegionStart("region-123_test")
	// Should not panic with special characters
	_ = result
}
