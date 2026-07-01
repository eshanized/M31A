package layout

import (
	"strings"
	"testing"

	"github.com/eshanized/M31A/internal/tui/theme"
)

func testCache() *theme.StyleCache {
	return theme.NewStyleCache(theme.Default())
}

func TestShortProviderName(t *testing.T) {
	t.Parallel()
	tests := []struct {
		input string
		want  string
	}{
		{"openrouter", "OR"},
		{"OpenRouter", "OR"},
		{"openrouterai", "OR"},
		{"ZenAI", "ZEN"},
		{"myzenprovider", "ZEN"},
		{"anth", "ant"},
		{"ab", "ab"},
		{"a", "a"},
		{"claude", "cla"},
		{"gpt", "gpt"},
		{"", ""},
	}
	for _, tt := range tests {
		got := shortProviderName(tt.input)
		if got != tt.want {
			t.Errorf("shortProviderName(%q) = %q, want %q", tt.input, got, tt.want)
		}
	}
}

func TestFormatTokenCount(t *testing.T) {
	t.Parallel()
	tests := []struct {
		n    int
		want string
	}{
		{0, "0 ctx"},
		{1, "1 ctx"},
		{999, "999 ctx"},
		{1000, "1K ctx"},
		{1500, "1.5K ctx"},
		{10000, "10K ctx"},
		{100000, "100K ctx"},
	}
	for _, tt := range tests {
		got := formatTokenCount(tt.n)
		if got != tt.want {
			t.Errorf("formatTokenCount(%d) = %q, want %q", tt.n, got, tt.want)
		}
	}
}

func TestFormatFloat1(t *testing.T) {
	t.Parallel()
	tests := []struct {
		f    float64
		want string
	}{
		{0.0, "0.0"},
		{1.5, "1.5"},
		{2.0, "2.0"},
		{10.3, "10.3"},
		{0.1, "0.1"},
	}
	for _, tt := range tests {
		got := formatFloat1(tt.f)
		if got != tt.want {
			t.Errorf("formatFloat1(%f) = %q, want %q", tt.f, got, tt.want)
		}
	}
}

func TestFormatCost(t *testing.T) {
	t.Parallel()
	tests := []struct {
		f    float64
		want string
	}{
		{0.0, "0.00"},
		{0.05, "0.05"},
		{0.5, "0.50"},
		{1.0, "1.00"},
		{1.25, "1.25"},
		{10.99, "10.99"},
	}
	for _, tt := range tests {
		got := formatCost(tt.f)
		if got != tt.want {
			t.Errorf("formatCost(%f) = %q, want %q", tt.f, got, tt.want)
		}
	}
}

func TestIntToStr(t *testing.T) {
	t.Parallel()
	tests := []struct {
		n    int
		want string
	}{
		{0, "0"},
		{1, "1"},
		{10, "10"},
		{123, "123"},
		{-1, "-1"},
		{-123, "-123"},
		{999999, "999999"},
	}
	for _, tt := range tests {
		got := intToStr(tt.n)
		if got != tt.want {
			t.Errorf("intToStr(%d) = %q, want %q", tt.n, got, tt.want)
		}
	}
}

func TestTruncateToWidth(t *testing.T) {
	t.Parallel()
	tests := []struct {
		s    string
		maxW int
	}{
		{"hello", 10},
		{"hello", 5},
		{"hello world", 3},
		{"", 5},
		{"hello", 0},
	}
	for _, tt := range tests {
		got := truncateToWidth(tt.s, tt.maxW)
		if got == "" && tt.s != "" {
			t.Errorf("truncateToWidth(%q, %d) returned empty string", tt.s, tt.maxW)
		}
	}
}

func TestTruncateToWidth_WithANSI(t *testing.T) {
	t.Parallel()
	colored := "\x1b[31mhello\x1b[0m"
	got := truncateToWidth(colored, 3)
	if got == "" {
		t.Error("truncateToWidth with ANSI should return non-empty string")
	}
}

func TestRenderOverlay(t *testing.T) {
	t.Parallel()
	base := "line1\nline2\nline3"
	overlay := "overlay1\noverlay2"
	got := RenderOverlay(base, overlay, 80, 3, 20, theme.Dark())
	if got == "" {
		t.Error("RenderOverlay should return non-empty string")
	}
	lines := strings.Split(got, "\n")
	if len(lines) != 3 {
		t.Errorf("RenderOverlay produced %d lines, want 3", len(lines))
	}
}

func TestRenderOverlay_BaseTooShort(t *testing.T) {
	t.Parallel()
	base := "line1"
	overlay := "overlay"
	got := RenderOverlay(base, overlay, 80, 3, 20, theme.Dark())
	if got == "" {
		t.Error("RenderOverlay with short base should return non-empty string")
	}
}

func TestRenderOverlay_BaseTooLong(t *testing.T) {
	t.Parallel()
	base := "line1\nline2\nline3\nline4\nline5"
	overlay := "ov"
	got := RenderOverlay(base, overlay, 80, 2, 20, theme.Dark())
	if got == "" {
		t.Error("RenderOverlay with long base should return non-empty string")
	}
}

func TestRenderOverlay_WideOverlay(t *testing.T) {
	t.Parallel()
	base := "content"
	overlay := "wide"
	got := RenderOverlay(base, overlay, 40, 1, 50, theme.Dark())
	if got == "" {
		t.Error("RenderOverlay with wide overlay should return non-empty string")
	}
}

func TestBuildHeader_FullWidth(t *testing.T) {
	t.Parallel()
	tm := theme.Default()
	info := HeaderInfo{Brand: "M31A", Breadcrumb: "Chat", ModelName: "claude-3", Provider: "openrouter"}
	header := BuildHeader(info, 100, Full, tm, testCache())
	if !strings.Contains(header, "M31A") {
		t.Error("Full header should contain brand")
	}
	if !strings.Contains(header, "Chat") {
		t.Error("Full header should contain breadcrumb")
	}
}

func TestBuildHeader_VeryNarrow(t *testing.T) {
	t.Parallel()
	tm := theme.Default()
	info := HeaderInfo{Brand: "M31A"}
	header := BuildHeader(info, 5, Full, tm, testCache())
	if header == "" {
		t.Error("Very narrow header should return non-empty string")
	}
}

func TestBuildFooter_FullWidth_Cost(t *testing.T) {
	t.Parallel()
	tm := theme.Default()
	info := FooterInfo{
		Cwd:           "project",
		GitBranch:     "main",
		Operation:     "thinking...",
		KeyboardHints: []string{"ctrl+p cmds"},
		TokenCount:    5000,
		Cost:          0.05,
		ShowCost:      true,
		SpinnerFrame:  "~",
	}
	footer := BuildFooter(info, 80, Full, tm, testCache())
	if !strings.Contains(footer, "project") {
		t.Error("Full footer should contain cwd")
	}
}

func TestBuildFooter_VerySmallCost(t *testing.T) {
	t.Parallel()
	tm := theme.Default()
	info := FooterInfo{
		Cwd:        "project",
		TokenCount: 100,
		Cost:       0.001,
		ShowCost:   true,
	}
	footer := BuildFooter(info, 80, Full, tm, testCache())
	// Cost is no longer displayed in simplified chrome footer
	if strings.Contains(footer, "<$0.01") || strings.Contains(footer, "$") {
		t.Error("Simplified chrome footer should not contain cost")
	}
}

func TestBuildFooter_LeaderActive(t *testing.T) {
	t.Parallel()
	tm := theme.Default()
	info := FooterInfo{
		Cwd:          "project",
		LeaderActive: true,
	}
	footer := BuildFooter(info, 80, Full, tm, testCache())
	// Footer now shows "LEADER" (not "ctrl+x") when leader key is active
	if !strings.Contains(footer, "LEADER") {
		t.Error("Leader active should show 'LEADER'")
	}
}

func TestBuildFooter_VeryNarrow(t *testing.T) {
	t.Parallel()
	tm := theme.Default()
	info := FooterInfo{Cwd: "project"}
	footer := BuildFooter(info, 5, Full, tm, testCache())
	if footer == "" {
		t.Error("Very narrow footer should return non-empty string")
	}
}

func TestBuildFooter_StandardWidth_NoCost(t *testing.T) {
	t.Parallel()
	tm := theme.Default()
	info := FooterInfo{
		Cwd:      "project",
		Cost:     5.0,
		ShowCost: true,
	}
	footer := BuildFooter(info, 70, Standard, tm, testCache())
	if !strings.Contains(footer, "project") {
		t.Error("Standard footer should contain cwd")
	}
}

func TestBuildFooter_Compact_BasenameOnly(t *testing.T) {
	t.Parallel()
	tm := theme.Default()
	info := FooterInfo{Cwd: "/home/user/project"}
	footer := BuildFooter(info, 50, Compact, tm, testCache())
	if !strings.Contains(footer, "project") {
		t.Error("Compact footer should contain basename")
	}
	if strings.Contains(footer, "/home/user/") {
		t.Error("Compact footer should not contain full path")
	}
}

func TestPageChrome_ContentWidth(t *testing.T) {
	t.Parallel()
	chrome := PageChrome{Width: 80, Height: 24}
	if got := chrome.ContentWidth(); got != 80 {
		t.Errorf("ContentWidth() = %d, want 80", got)
	}
}

func TestAssembleThreeZone_AllZones(t *testing.T) {
	t.Parallel()
	got := assembleThreeZone("left", "center", "right", 40)
	if got == "" {
		t.Error("assembleThreeZone should return non-empty string")
	}
	if !strings.Contains(got, "left") {
		t.Error("should contain left zone")
	}
}

func TestAssembleThreeZone_DropRight(t *testing.T) {
	t.Parallel()
	got := assembleThreeZone("left", "", "verylongrightstring", 10)
	if got == "" {
		t.Error("assembleThreeZone drop right should return non-empty string")
	}
}

func TestAssembleThreeZone_DropCenter(t *testing.T) {
	t.Parallel()
	got := assembleThreeZone("left", "verylongcenterstring", "", 10)
	if got == "" {
		t.Error("assembleThreeZone drop center should return non-empty string")
	}
}

func TestAssembleThreeZone_NoCenterNoRight(t *testing.T) {
	t.Parallel()
	got := assembleThreeZone("left", "", "", 30)
	if got == "" {
		t.Error("assembleThreeZone no center no right should return non-empty string")
	}
}

func TestRenderProvBadge(t *testing.T) {
	t.Parallel()
	got := renderProvBadge(theme.NewStyleCache(theme.Dark()).S, "openrouter")
	if got == "" {
		t.Error("renderProvBadge should return non-empty string")
	}
}

func TestBuildHeader_StandardWidth_WithModel(t *testing.T) {
	t.Parallel()
	tm := theme.Default()
	info := HeaderInfo{Brand: "M31A", ModelName: "claude-3", Provider: "zen"}
	header := BuildHeader(info, 80, Standard, tm, testCache())
	if !strings.Contains(header, "M31A") {
		t.Error("Standard header with model should contain brand")
	}
}

func TestBuildFooter_StandardWidth_WithOperation(t *testing.T) {
	t.Parallel()
	tm := theme.Default()
	info := FooterInfo{Cwd: "project", Operation: "responding..."}
	footer := BuildFooter(info, 65, Standard, tm, testCache())
	if !strings.Contains(footer, "responding") {
		t.Error("Standard footer should contain operation")
	}
}

func TestBuildFooter_FullWidth_NoCostShow(t *testing.T) {
	t.Parallel()
	tm := theme.Default()
	info := FooterInfo{Cwd: "project", TokenCount: 5000, ShowCost: false}
	footer := BuildFooter(info, 80, Full, tm, testCache())
	if strings.Contains(footer, "$") {
		t.Error("Full footer with ShowCost=false should not contain cost")
	}
}

func TestBuildFooter_FullWidth_ZeroTokenCount(t *testing.T) {
	t.Parallel()
	tm := theme.Default()
	info := FooterInfo{Cwd: "project", TokenCount: 0, ShowCost: true}
	footer := BuildFooter(info, 80, Full, tm, testCache())
	if strings.Contains(footer, "ctx") {
		t.Error("Full footer with 0 token count should not contain 'ctx'")
	}
}
