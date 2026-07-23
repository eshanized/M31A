package layout_test

import (
	"github.com/eshanized/M31A/internal/ui/tui/layout"
	"strings"
	"testing"

	"github.com/eshanized/M31A/internal/ui/tui/theme"
)

func TestDetect(t *testing.T) {
	tests := []struct {
		width int
		want  layout.Breakpoint
	}{
		{0, layout.UltraNarrow},
		{10, layout.UltraNarrow},
		{39, layout.UltraNarrow},
		{40, layout.Compact},
		{50, layout.Compact},
		{59, layout.Compact},
		{60, layout.Standard},
		{70, layout.Standard},
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

func TestShowSidebar(t *testing.T) {
	if layout.ShowSidebar(79) {
		t.Error("layout.ShowSidebar(79) should be false")
	}
	if !layout.ShowSidebar(80) {
		t.Error("layout.ShowSidebar(80) should be true")
	}
}

func TestShowHeaderRight(t *testing.T) {
	if layout.ShowHeaderRight(59) {
		t.Error("layout.ShowHeaderRight(59) should be false")
	}
	if !layout.ShowHeaderRight(60) {
		t.Error("layout.ShowHeaderRight(60) should be true")
	}
}

func TestShowFooterHints(t *testing.T) {
	if layout.ShowFooterHints(59) {
		t.Error("layout.ShowFooterHints(59) should be false")
	}
	if !layout.ShowFooterHints(60) {
		t.Error("layout.ShowFooterHints(60) should be true")
	}
}

func TestChromeHeight(t *testing.T) {
	if layout.ChromeHeight != 2 {
		t.Errorf("layout.ChromeHeight = %d, want 2", layout.ChromeHeight)
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
		chrome := layout.PageChrome{Width: 80, Height: tt.height}
		got := chrome.ContentHeight()
		if got != tt.want {
			t.Errorf("layout.PageChrome{Height: %d}.ContentHeight() = %d, want %d", tt.height, got, tt.want)
		}
	}
}

func TestRenderPageRowCount(t *testing.T) {
	chrome := layout.PageChrome{Width: 80, Height: 24}
	content := "line1\nline2\nline3"
	header := layout.HeaderInfo{Brand: "M31A"}
	footer := layout.FooterInfo{Cwd: "project"}
	tm := theme.Default()
	cache := theme.NewStyleCache(tm)

	result := layout.RenderPage(chrome, content, header, footer, tm, cache)
	lines := strings.Split(result, "\n")

	if len(lines) != 24 {
		t.Errorf("RenderPage produced %d lines, want 24", len(lines))
	}
}

func TestRenderTooNarrow(t *testing.T) {
	tm := theme.Default()
	result := layout.RenderTooNarrow(30, 20, tm)
	if result == "" {
		t.Error("RenderTooNarrow returned empty string")
	}
}

func TestBuildHeaderContainsBrand(t *testing.T) {
	tm := theme.Default()
	cache := theme.NewStyleCache(tm)
	info := layout.HeaderInfo{Brand: "M31A", Breadcrumb: "Chat"}
	header := layout.BuildHeader(info, 80, layout.Full, tm, cache)
	if !strings.Contains(header, "M31A") {
		t.Error("Header does not contain brand name")
	}
}

func TestBuildFooterContainsCwd(t *testing.T) {
	tm := theme.Default()
	cache := theme.NewStyleCache(tm)
	info := layout.FooterInfo{Cwd: "myproject"}
	footer := layout.BuildFooter(info, 80, layout.Full, tm, cache)
	if !strings.Contains(footer, "myproject") {
		t.Error("Footer does not contain cwd")
	}
}

func TestBuildHeaderCompactNoBreadcrumb(t *testing.T) {
	tm := theme.Default()
	cache := theme.NewStyleCache(tm)
	info := layout.HeaderInfo{Brand: "M31A", Breadcrumb: "Chat"}
	header := layout.BuildHeader(info, 50, layout.Compact, tm, cache)
	if strings.Contains(header, "Chat") {
		t.Error("layout.Compact header should not contain breadcrumb")
	}
}

func TestBuildFooterCompactNoHints(t *testing.T) {
	tm := theme.Default()
	cache := theme.NewStyleCache(tm)
	info := layout.FooterInfo{
		Cwd:           "project",
		KeyboardHints: []string{"ctrl+p cmds"},
	}
	footer := layout.BuildFooter(info, 50, layout.Compact, tm, cache)
	if strings.Contains(footer, "ctrl+p") {
		t.Error("layout.Compact footer should not contain keyboard hints")
	}
}
