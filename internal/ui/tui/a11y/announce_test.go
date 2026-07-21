package a11y

import (
	"strings"
	"testing"
)

func TestAnnounce(t *testing.T) {
	result := Announce("Hello World")

	if !SupportsOSC1337() {
		// SGR fallback: should wrap in bold markers.
		if !strings.HasPrefix(result, "\033[1m") {
			t.Errorf("Announce() SGR fallback should start with bold, got %q", result)
		}
		if !strings.HasSuffix(result, "\033[0m") {
			t.Errorf("Announce() SGR fallback should end with reset, got %q", result)
		}
		if !strings.Contains(result, "Hello World") {
			t.Errorf("Announce() SGR fallback missing text, got %q", result)
		}
		return
	}

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

	if !SupportsOSC1337() {
		// SGR fallback still wraps empty string in bold markers.
		if !strings.HasPrefix(result, "\033[1m") {
			t.Errorf("Announce(\"\") SGR fallback should start with bold, got %q", result)
		}
		return
	}

	if !strings.HasPrefix(result, "\x1b]1337;SetStatusMessage=") {
		t.Errorf("Announce() = %q, missing prefix", result)
	}
	if !strings.HasSuffix(result, "\x07") {
		t.Errorf("Announce() = %q, missing suffix", result)
	}
}

func TestDescribeElement(t *testing.T) {
	result := DescribeElement("button", "button")

	if !SupportsSemanticLabels() {
		if result != "" {
			t.Errorf("DescribeElement() should return empty on unsupported terminal, got %q", result)
		}
		return
	}

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

	if !SupportsSemanticLabels() {
		if result != "" {
			t.Errorf("DescribeElement(\"\") should return empty on unsupported terminal, got %q", result)
		}
		return
	}

	if !strings.Contains(result, "text:text") {
		t.Errorf("DescribeElement() = %q, expected default role 'text'", result)
	}
}

func TestRegionStart(t *testing.T) {
	result := RegionStart("sidebar")

	if !SupportsRegions() {
		if result != "" {
			t.Errorf("RegionStart() should return empty on unsupported terminal, got %q", result)
		}
		return
	}

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

	if !SupportsRegions() {
		if result != "" {
			t.Errorf("RegionEnd() should return empty on unsupported terminal, got %q", result)
		}
		return
	}

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

	if !SupportsRegions() {
		if start != "" || end != "" {
			t.Error("RegionStart/RegionEnd should return empty on unsupported terminal")
		}
		return
	}

	if start == end {
		t.Error("RegionStart and RegionEnd should produce different output")
	}
}

func TestAnnounce_SpecialCharacters(t *testing.T) {
	result := Announce("Hello <World> & \"Friends\"")

	if !SupportsOSC1337() {
		// SGR fallback preserves special characters.
		if !strings.Contains(result, "Hello <World> & \"Friends\"") {
			t.Errorf("Announce() SGR fallback missing special characters, got %q", result)
		}
		return
	}

	if !strings.Contains(result, "Hello <World> & \"Friends\"") {
		t.Errorf("Announce() = %q, missing special characters", result)
	}
}

func TestDescribeElement_SpecialCharacters(t *testing.T) {
	result := DescribeElement("my-button", "interactive")

	if !SupportsSemanticLabels() {
		if result != "" {
			t.Errorf("DescribeElement() should return empty on unsupported terminal, got %q", result)
		}
		return
	}

	if !strings.Contains(result, "interactive:my-button") {
		t.Errorf("DescribeElement() = %q, missing content", result)
	}
}

func TestRegionStart_SpecialCharacters(t *testing.T) {
	result := RegionStart("my-region-123")

	if !SupportsRegions() {
		if result != "" {
			t.Errorf("RegionStart() should return empty on unsupported terminal, got %q", result)
		}
		return
	}

	if !strings.Contains(result, "my-region-123") {
		t.Errorf("RegionStart() = %q, missing content", result)
	}
}

func TestDetectTerminal(t *testing.T) {
	terminal := DetectTerminal()
	if terminal == TermUnknown {
		t.Error("DetectTerminal() should never return TermUnknown; fallback is TermGeneric")
	}
}

func TestSupportsOSC1337(t *testing.T) {
	_ = SupportsOSC1337()
}

func TestSupportsSemanticLabels(t *testing.T) {
	_ = SupportsSemanticLabels()
}

func TestSupportsRegions(t *testing.T) {
	_ = SupportsRegions()
}

func TestSupportsSGR(t *testing.T) {
	if !SupportsSGR() {
		t.Error("SupportsSGR() should always return true")
	}
}

func TestAnnounceStreamContent(t *testing.T) {
	result := AnnounceStreamContent("Hello streaming")
	if result == "" {
		t.Error("AnnounceStreamContent() should never return empty")
	}
	if !strings.Contains(result, "Hello streaming") {
		t.Errorf("AnnounceStreamContent() = %q, missing text", result)
	}
}

func TestAnnouncePhaseChange(t *testing.T) {
	result := AnnouncePhaseChange("Execute")
	if result == "" {
		t.Error("AnnouncePhaseChange() should never return empty")
	}
	if !strings.Contains(result, "Execute") {
		t.Errorf("AnnouncePhaseChange() = %q, missing phase", result)
	}
}

func TestAnnounceError(t *testing.T) {
	result := AnnounceError("something broke")
	if result == "" {
		t.Error("AnnounceError() should never return empty")
	}
	if !strings.Contains(result, "something broke") {
		t.Errorf("AnnounceError() = %q, missing error text", result)
	}
}

func TestAnnounceSuccess(t *testing.T) {
	result := AnnounceSuccess("all done")
	if result == "" {
		t.Error("AnnounceSuccess() should never return empty")
	}
	if !strings.Contains(result, "all done") {
		t.Errorf("AnnounceSuccess() = %q, missing success text", result)
	}
}

func TestAnnounceProgress(t *testing.T) {
	result := AnnounceProgress(3, 5, "tasks")
	if result == "" {
		t.Error("AnnounceProgress() should never return empty")
	}
	if !strings.Contains(result, "3/5") {
		t.Errorf("AnnounceProgress() = %q, missing progress counts", result)
	}
	if !strings.Contains(result, "tasks") {
		t.Errorf("AnnounceProgress() = %q, missing label", result)
	}
}
