package layout

import (
	"strings"
	"testing"

	"github.com/eshanized/M31A/internal/tui/theme"
)

func TestDetect(t *testing.T) {
	tests := []struct {
		width int
		want  Breakpoint
	}{
		{0, UltraNarrow},
		{10, UltraNarrow},
		{39, UltraNarrow},
		{40, Compact},
		{50, Compact},
		{59, Compact},
		{60, Standard},
		{70, Standard},
		{79, Standard},
		{80, Full},
		{120, Full},
		{200, Full},
	}

	for _, tt := range tests {
		got := Detect(tt.width)
		if got != tt.want {
			t.Errorf("Detect(%d) = %d, want %d", tt.width, got, tt.want)
		}
	}
}

func TestShowSidebar(t *testing.T) {
	if ShowSidebar(79) {
		t.Error("ShowSidebar(79) should be false")
	}
	if !ShowSidebar(80) {
		t.Error("ShowSidebar(80) should be true")
	}
}

func TestShowHeaderRight(t *testing.T) {
	if ShowHeaderRight(59) {
		t.Error("ShowHeaderRight(59) should be false")
	}
	if !ShowHeaderRight(60) {
		t.Error("ShowHeaderRight(60) should be true")
	}
}

func TestShowFooterHints(t *testing.T) {
	if ShowFooterHints(59) {
		t.Error("ShowFooterHints(59) should be false")
	}
	if !ShowFooterHints(60) {
		t.Error("ShowFooterHints(60) should be true")
	}
}

func TestChromeHeight(t *testing.T) {
	if ChromeHeight != 2 {
		t.Errorf("ChromeHeight = %d, want 2", ChromeHeight)
	}
}

func TestPageChromeContentHeight(t *testing.T) {
	tests := []struct {
		height int
		want   int
	}{
		{24, 22},
		{10, 8},
		{2, 1}, // minimum
		{1, 1}, // clamped to 1
	}

	for _, tt := range tests {
		chrome := PageChrome{Width: 80, Height: tt.height}
		got := chrome.ContentHeight()
		if got != tt.want {
			t.Errorf("PageChrome{Height: %d}.ContentHeight() = %d, want %d", tt.height, got, tt.want)
		}
	}
}

func TestRenderPageRowCount(t *testing.T) {
	chrome := PageChrome{Width: 80, Height: 24}
	content := "line1\nline2\nline3"
	header := HeaderInfo{Brand: "M31A"}
	footer := FooterInfo{Cwd: "project"}
	tm := theme.Default()

	result := RenderPage(chrome, content, header, footer, tm)
	lines := strings.Split(result, "\n")

	if len(lines) != 24 {
		t.Errorf("RenderPage produced %d lines, want 24", len(lines))
	}
}

func TestRenderTooNarrow(t *testing.T) {
	tm := theme.Default()
	result := RenderTooNarrow(30, 20, tm)
	if result == "" {
		t.Error("RenderTooNarrow returned empty string")
	}
}

func TestBuildHeaderContainsBrand(t *testing.T) {
	tm := theme.Default()
	info := HeaderInfo{Brand: "M31A", Breadcrumb: "Chat"}
	header := BuildHeader(info, 80, Full, tm)
	if !strings.Contains(header, "M31A") {
		t.Error("Header does not contain brand name")
	}
}

func TestBuildFooterContainsCwd(t *testing.T) {
	tm := theme.Default()
	info := FooterInfo{Cwd: "myproject"}
	footer := BuildFooter(info, 80, Full, tm)
	if !strings.Contains(footer, "myproject") {
		t.Error("Footer does not contain cwd")
	}
}

func TestBuildHeaderCompactNoBreadcrumb(t *testing.T) {
	tm := theme.Default()
	info := HeaderInfo{Brand: "M31A", Breadcrumb: "Chat"}
	header := BuildHeader(info, 50, Compact, tm)
	if strings.Contains(header, "Chat") {
		t.Error("Compact header should not contain breadcrumb")
	}
}

func TestBuildFooterCompactNoHints(t *testing.T) {
	tm := theme.Default()
	info := FooterInfo{
		Cwd:           "project",
		KeyboardHints: []string{"ctrl+p cmds"},
	}
	footer := BuildFooter(info, 50, Compact, tm)
	if strings.Contains(footer, "ctrl+p") {
		t.Error("Compact footer should not contain keyboard hints")
	}
}
