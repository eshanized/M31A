package a11y_test

import (
	"github.com/eshanized/M31A/internal/ui/tui/a11y"
	"strings"
	"testing"
)

func TestAnnounce(t *testing.T) {
	result := a11y.Announce("Hello World")

	if !a11y.SupportsOSC1337() {
		// SGR fallback: should wrap in bold markers.
		if !strings.HasPrefix(result, "\033[1m") {
			t.Errorf("a11y.Announce() SGR fallback should start with bold, got %q", result)
		}
		if !strings.HasSuffix(result, "\033[0m") {
			t.Errorf("a11y.Announce() SGR fallback should end with reset, got %q", result)
		}
		if !strings.Contains(result, "Hello World") {
			t.Errorf("a11y.Announce() SGR fallback missing text, got %q", result)
		}
		return
	}

	if !strings.HasPrefix(result, "\x1b]1337;SetStatusMessage=") {
		t.Errorf("a11y.Announce() = %q, missing prefix", result)
	}
	if !strings.HasSuffix(result, "\x07") {
		t.Errorf("a11y.Announce() = %q, missing suffix", result)
	}
	if !strings.Contains(result, "Hello World") {
		t.Errorf("a11y.Announce() = %q, missing text", result)
	}
}

func TestAnnounce_Empty(t *testing.T) {
	result := a11y.Announce("")

	if !a11y.SupportsOSC1337() {
		// SGR fallback still wraps empty string in bold markers.
		if !strings.HasPrefix(result, "\033[1m") {
			t.Errorf("a11y.Announce(\"\") SGR fallback should start with bold, got %q", result)
		}
		return
	}

	if !strings.HasPrefix(result, "\x1b]1337;SetStatusMessage=") {
		t.Errorf("a11y.Announce() = %q, missing prefix", result)
	}
	if !strings.HasSuffix(result, "\x07") {
		t.Errorf("a11y.Announce() = %q, missing suffix", result)
	}
}

func TestDescribeElement(t *testing.T) {
	result := a11y.DescribeElement("button", "button")

	if !a11y.SupportsSemanticLabels() {
		if result != "" {
			t.Errorf("a11y.DescribeElement() should return empty on unsupported terminal, got %q", result)
		}
		return
	}

	if !strings.HasPrefix(result, "\x1b]1337;SetSemanticLabel=") {
		t.Errorf("a11y.DescribeElement() = %q, missing prefix", result)
	}
	if !strings.HasSuffix(result, "\x07") {
		t.Errorf("a11y.DescribeElement() = %q, missing suffix", result)
	}
	if !strings.Contains(result, "button:button") {
		t.Errorf("a11y.DescribeElement() = %q, missing label:role", result)
	}
}

func TestDescribeElement_DefaultRole(t *testing.T) {
	result := a11y.DescribeElement("text", "")

	if !a11y.SupportsSemanticLabels() {
		if result != "" {
			t.Errorf("a11y.DescribeElement(\"\") should return empty on unsupported terminal, got %q", result)
		}
		return
	}

	if !strings.Contains(result, "text:text") {
		t.Errorf("a11y.DescribeElement() = %q, expected default role 'text'", result)
	}
}

func TestRegionStart(t *testing.T) {
	result := a11y.RegionStart("sidebar")

	if !a11y.SupportsRegions() {
		if result != "" {
			t.Errorf("a11y.RegionStart() should return empty on unsupported terminal, got %q", result)
		}
		return
	}

	if !strings.HasPrefix(result, "\x1b]1337;PushRegion=") {
		t.Errorf("a11y.RegionStart() = %q, missing prefix", result)
	}
	if !strings.HasSuffix(result, "\x07") {
		t.Errorf("a11y.RegionStart() = %q, missing suffix", result)
	}
	if !strings.Contains(result, "sidebar") {
		t.Errorf("a11y.RegionStart() = %q, missing name", result)
	}
}

func TestRegionEnd(t *testing.T) {
	result := a11y.RegionEnd("sidebar")

	if !a11y.SupportsRegions() {
		if result != "" {
			t.Errorf("a11y.RegionEnd() should return empty on unsupported terminal, got %q", result)
		}
		return
	}

	if !strings.HasPrefix(result, "\x1b]1337;PopRegion=") {
		t.Errorf("a11y.RegionEnd() = %q, missing prefix", result)
	}
	if !strings.HasSuffix(result, "\x07") {
		t.Errorf("a11y.RegionEnd() = %q, missing suffix", result)
	}
	if !strings.Contains(result, "sidebar") {
		t.Errorf("a11y.RegionEnd() = %q, missing name", result)
	}
}

func TestRegionStartEnd_Pair(t *testing.T) {
	start := a11y.RegionStart("main")
	end := a11y.RegionEnd("main")

	if !a11y.SupportsRegions() {
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
	result := a11y.Announce("Hello <World> & \"Friends\"")

	if !a11y.SupportsOSC1337() {
		// SGR fallback preserves special characters.
		if !strings.Contains(result, "Hello <World> & \"Friends\"") {
			t.Errorf("a11y.Announce() SGR fallback missing special characters, got %q", result)
		}
		return
	}

	if !strings.Contains(result, "Hello <World> & \"Friends\"") {
		t.Errorf("a11y.Announce() = %q, missing special characters", result)
	}
}

func TestDescribeElement_SpecialCharacters(t *testing.T) {
	result := a11y.DescribeElement("my-button", "interactive")

	if !a11y.SupportsSemanticLabels() {
		if result != "" {
			t.Errorf("a11y.DescribeElement() should return empty on unsupported terminal, got %q", result)
		}
		return
	}

	if !strings.Contains(result, "interactive:my-button") {
		t.Errorf("a11y.DescribeElement() = %q, missing content", result)
	}
}

func TestRegionStart_SpecialCharacters(t *testing.T) {
	result := a11y.RegionStart("my-region-123")

	if !a11y.SupportsRegions() {
		if result != "" {
			t.Errorf("a11y.RegionStart() should return empty on unsupported terminal, got %q", result)
		}
		return
	}

	if !strings.Contains(result, "my-region-123") {
		t.Errorf("a11y.RegionStart() = %q, missing content", result)
	}
}

func TestDetectTerminal(t *testing.T) {
	terminal := a11y.DetectTerminal()
	if terminal == a11y.TermUnknown {
		t.Error("a11y.DetectTerminal() should never return a11y.TermUnknown; fallback is TermGeneric")
	}
}

func TestSupportsOSC1337(t *testing.T) {
	_ = a11y.SupportsOSC1337()
}

func TestSupportsSemanticLabels(t *testing.T) {
	_ = a11y.SupportsSemanticLabels()
}

func TestSupportsRegions(t *testing.T) {
	_ = a11y.SupportsRegions()
}

func TestSupportsSGR(t *testing.T) {
	if !a11y.SupportsSGR() {
		t.Error("a11y.SupportsSGR() should always return true")
	}
}

func TestAnnounceStreamContent(t *testing.T) {
	result := a11y.AnnounceStreamContent("Hello streaming")
	if result == "" {
		t.Error("a11y.AnnounceStreamContent() should never return empty")
	}
	if !strings.Contains(result, "Hello streaming") {
		t.Errorf("a11y.AnnounceStreamContent() = %q, missing text", result)
	}
}

func TestAnnouncePhaseChange(t *testing.T) {
	result := a11y.AnnouncePhaseChange("Execute")
	if result == "" {
		t.Error("a11y.AnnouncePhaseChange() should never return empty")
	}
	if !strings.Contains(result, "Execute") {
		t.Errorf("a11y.AnnouncePhaseChange() = %q, missing phase", result)
	}
}

func TestAnnounceError(t *testing.T) {
	result := a11y.AnnounceError("something broke")
	if result == "" {
		t.Error("a11y.AnnounceError() should never return empty")
	}
	if !strings.Contains(result, "something broke") {
		t.Errorf("a11y.AnnounceError() = %q, missing error text", result)
	}
}

func TestAnnounceSuccess(t *testing.T) {
	result := a11y.AnnounceSuccess("all done")
	if result == "" {
		t.Error("a11y.AnnounceSuccess() should never return empty")
	}
	if !strings.Contains(result, "all done") {
		t.Errorf("a11y.AnnounceSuccess() = %q, missing success text", result)
	}
}

func TestAnnounceProgress(t *testing.T) {
	result := a11y.AnnounceProgress(3, 5, "tasks")
	if result == "" {
		t.Error("a11y.AnnounceProgress() should never return empty")
	}
	if !strings.Contains(result, "3/5") {
		t.Errorf("a11y.AnnounceProgress() = %q, missing progress counts", result)
	}
	if !strings.Contains(result, "tasks") {
		t.Errorf("a11y.AnnounceProgress() = %q, missing label", result)
	}
}
