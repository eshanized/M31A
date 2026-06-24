package a11y

import (
	"strings"
	"testing"
)

func TestAnnounce(t *testing.T) {
	result := Announce("Hello World")

	if !strings.HasPrefix(result, "\x1b]1337;SetStatusMessage=") {
		t.Errorf("Announce() = %q, missing prefix", result)
	}
	if !strings.HasSuffix(result, "\x07") {
		t.Errorf("Announce() = %q, missing suffix", result)
	}
	if !strings.Contains(result, "Hello World") {
		t.Errorf("Announce() = %q, missing text", result)
	}
}

func TestAnnounce_Empty(t *testing.T) {
	result := Announce("")

	if !strings.HasPrefix(result, "\x1b]1337;SetStatusMessage=") {
		t.Errorf("Announce() = %q, missing prefix", result)
	}
	if !strings.HasSuffix(result, "\x07") {
		t.Errorf("Announce() = %q, missing suffix", result)
	}
}

func TestDescribeElement(t *testing.T) {
	result := DescribeElement("button", "button")

	if !strings.HasPrefix(result, "\x1b]1337;SetSemanticLabel=") {
		t.Errorf("DescribeElement() = %q, missing prefix", result)
	}
	if !strings.HasSuffix(result, "\x07") {
		t.Errorf("DescribeElement() = %q, missing suffix", result)
	}
	if !strings.Contains(result, "button:button") {
		t.Errorf("DescribeElement() = %q, missing label:role", result)
	}
}

func TestDescribeElement_DefaultRole(t *testing.T) {
	result := DescribeElement("text", "")

	if !strings.Contains(result, "text:text") {
		t.Errorf("DescribeElement() = %q, expected default role 'text'", result)
	}
}

func TestRegionStart(t *testing.T) {
	result := RegionStart("sidebar")

	if !strings.HasPrefix(result, "\x1b]1337;PushRegion=") {
		t.Errorf("RegionStart() = %q, missing prefix", result)
	}
	if !strings.HasSuffix(result, "\x07") {
		t.Errorf("RegionStart() = %q, missing suffix", result)
	}
	if !strings.Contains(result, "sidebar") {
		t.Errorf("RegionStart() = %q, missing name", result)
	}
}

func TestRegionEnd(t *testing.T) {
	result := RegionEnd("sidebar")

	if !strings.HasPrefix(result, "\x1b]1337;PopRegion=") {
		t.Errorf("RegionEnd() = %q, missing prefix", result)
	}
	if !strings.HasSuffix(result, "\x07") {
		t.Errorf("RegionEnd() = %q, missing suffix", result)
	}
	if !strings.Contains(result, "sidebar") {
		t.Errorf("RegionEnd() = %q, missing name", result)
	}
}

func TestRegionStartEnd_Pair(t *testing.T) {
	start := RegionStart("main")
	end := RegionEnd("main")

	if start == end {
		t.Error("RegionStart and RegionEnd should produce different output")
	}
}

func TestAnnounce_SpecialCharacters(t *testing.T) {
	result := Announce("Hello <World> & \"Friends\"")

	if !strings.Contains(result, "Hello <World> & \"Friends\"") {
		t.Errorf("Announce() = %q, missing special characters", result)
	}
}

func TestDescribeElement_SpecialCharacters(t *testing.T) {
	result := DescribeElement("my-button", "interactive")

	if !strings.Contains(result, "interactive:my-button") {
		t.Errorf("DescribeElement() = %q, missing content", result)
	}
}

func TestRegionStart_SpecialCharacters(t *testing.T) {
	result := RegionStart("my-region-123")

	if !strings.Contains(result, "my-region-123") {
		t.Errorf("RegionStart() = %q, missing content", result)
	}
}
