package a11y

import (
	"testing"
)

// --- Terminal type constants ---

func TestTerminalType_Constants(t *testing.T) {
	t.Parallel()
	if TermUnknown != 0 {
		t.Errorf("TermUnknown = %d, want 0", TermUnknown)
	}
	if TermITerm2 != 1 {
		t.Errorf("TermITerm2 = %d, want 1", TermITerm2)
	}
	if TermKitty != 2 {
		t.Errorf("TermKitty = %d, want 2", TermKitty)
	}
	if TermWezTerm != 3 {
		t.Errorf("TermWezTerm = %d, want 3", TermWezTerm)
	}
	if TermAlacritty != 4 {
		t.Errorf("TermAlacritty = %d, want 4", TermAlacritty)
	}
	if TermWindowsTerminal != 5 {
		t.Errorf("TermWindowsTerminal = %d, want 5", TermWindowsTerminal)
	}
	if TermTMux != 6 {
		t.Errorf("TermTMux = %d, want 6", TermTMux)
	}
	if TermSSH != 7 {
		t.Errorf("TermSSH = %d, want 7", TermSSH)
	}
	if TermGeneric != 8 {
		t.Errorf("TermGeneric = %d, want 8", TermGeneric)
	}
}

// --- DetectTerminal ---

func TestDetectTerminal_CachesResult(t *testing.T) {
	t.Parallel()
	// First call sets the cached value
	term1 := DetectTerminal()
	// Second call should return the same cached value
	term2 := DetectTerminal()
	if term1 != term2 {
		t.Errorf("DetectTerminal() returned different values: %d vs %d", term1, term2)
	}
}

func TestDetectTerminal_NeverUnknown(t *testing.T) {
	t.Parallel()
	term := DetectTerminal()
	if term == TermUnknown {
		t.Error("DetectTerminal() should never return TermUnknown; fallback is TermGeneric")
	}
}

// --- SupportsOSC1337 ---

func TestSupportsOSC1337_Consistent(t *testing.T) {
	t.Parallel()
	// Multiple calls should return the same value
	v1 := SupportsOSC1337()
	v2 := SupportsOSC1337()
	if v1 != v2 {
		t.Errorf("SupportsOSC1337() returned different values: %v vs %v", v1, v2)
	}
}

// --- SupportsSemanticLabels ---

func TestSupportsSemanticLabels_Consistent(t *testing.T) {
	t.Parallel()
	v1 := SupportsSemanticLabels()
	v2 := SupportsSemanticLabels()
	if v1 != v2 {
		t.Errorf("SupportsSemanticLabels() returned different values: %v vs %v", v1, v2)
	}
}

// --- SupportsRegions ---

func TestSupportsRegions_Consistent(t *testing.T) {
	t.Parallel()
	v1 := SupportsRegions()
	v2 := SupportsRegions()
	if v1 != v2 {
		t.Errorf("SupportsRegions() returned different values: %v vs %v", v1, v2)
	}
}

// --- Announce ---

func TestAnnounce_ReturnsString(t *testing.T) {
	t.Parallel()
	result := Announce("test message")
	// Result is either empty (unsupported) or a formatted string
	_ = result
}

func TestAnnounce_EmptyText(t *testing.T) {
	t.Parallel()
	result := Announce("")
	// Should not panic with empty text
	_ = result
}

func TestAnnounce_LongText(t *testing.T) {
	t.Parallel()
	longText := make([]byte, 1000)
	for i := range longText {
		longText[i] = 'a'
	}
	result := Announce(string(longText))
	// Should not panic with long text
	_ = result
}

// --- DescribeElement ---

func TestDescribeElement_ReturnsString(t *testing.T) {
	t.Parallel()
	result := DescribeElement("button", "button")
	_ = result
}

func TestDescribeElement_EmptyRole(t *testing.T) {
	t.Parallel()
	result := DescribeElement("text", "")
	// Empty role should default to "text"
	_ = result
}

func TestDescribeElement_EmptyLabel(t *testing.T) {
	t.Parallel()
	result := DescribeElement("", "button")
	// Should not panic with empty label
	_ = result
}

// --- RegionStart ---

func TestRegionStart_ReturnsString(t *testing.T) {
	t.Parallel()
	result := RegionStart("sidebar")
	_ = result
}

func TestRegionStart_EmptyName(t *testing.T) {
	t.Parallel()
	result := RegionStart("")
	// Should not panic with empty name
	_ = result
}

// --- RegionEnd ---

func TestRegionEnd_ReturnsString(t *testing.T) {
	t.Parallel()
	result := RegionEnd("sidebar")
	_ = result
}

func TestRegionEnd_EmptyName(t *testing.T) {
	t.Parallel()
	result := RegionEnd("")
	// Should not panic with empty name
	_ = result
}

// --- RegionStart/RegionEnd pair ---

func TestRegionStartEnd_MatchingNames(t *testing.T) {
	t.Parallel()
	start := RegionStart("main")
	end := RegionEnd("main")
	// Both should either be empty (unsupported) or non-empty (supported)
	if (start == "") != (end == "") {
		t.Error("RegionStart and RegionEnd should both be empty or both non-empty")
	}
}

// --- Unicode and special characters ---

func TestAnnounce_Unicode(t *testing.T) {
	t.Parallel()
	result := Announce("Hello 世界 🌍")
	// Should not panic with unicode
	_ = result
}

func TestDescribeElement_SpecialChars(t *testing.T) {
	t.Parallel()
	result := DescribeElement("my-button_123", "interactive")
	// Should not panic with special characters
	_ = result
}

func TestRegionStart_SpecialChars(t *testing.T) {
	t.Parallel()
	result := RegionStart("region-123_test")
	// Should not panic with special characters
	_ = result
}
