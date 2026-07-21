package layout

import (
	"strings"
	"testing"

	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/ui/tui/theme"
)

// ─── M1 Header Simplification Tests ────────────────────────────────────────────

func TestM1_HeaderNoLeaderDots(t *testing.T) {
	t.Parallel()
	tm := theme.Default()
	cache := theme.NewStyleCache(tm)
	info := HeaderInfo{Brand: "M31A", Breadcrumb: "Chat", ModelName: "claude-3.5-sonnet"}
	header := BuildHeader(info, 80, Full, tm, cache)

	// Leader dots should not appear as the main fill pattern
	// The header should use spacing, not leader dots for visual separation
	if strings.Contains(header, "·····") {
		t.Error("Simplified header should not contain leader dots as fill")
	}
}

func TestM1_HeaderNoProviderBadge(t *testing.T) {
	t.Parallel()
	tm := theme.Default()
	cache := theme.NewStyleCache(tm)
	info := HeaderInfo{Brand: "M31A", Breadcrumb: "Chat", Provider: "openrouter"}
	header := BuildHeader(info, 80, Full, tm, cache)

	// Provider badge [OR] should not appear
	if strings.Contains(header, "[OR]") || strings.Contains(header, "[ZEN]") {
		t.Error("Simplified header should not contain provider badge")
	}
}

func TestM1_HeaderNoContextMeter(t *testing.T) {
	t.Parallel()
	tm := theme.Default()
	cache := theme.NewStyleCache(tm)
	info := HeaderInfo{
		Brand:      "M31A",
		Breadcrumb: "Chat",
		CtxUsed:    50000,
		CtxTotal:   100000,
		CtxHistory: []int{1000, 2000, 3000},
	}
	header := BuildHeader(info, 80, Full, tm, cache)

	// Context meter bar (█░) should not appear
	if strings.Contains(header, "█") || strings.Contains(header, "░") {
		t.Error("Simplified header should not contain context meter")
	}
}

func TestM1_HeaderShowsBrandAndModel(t *testing.T) {
	t.Parallel()
	tm := theme.Default()
	cache := theme.NewStyleCache(tm)
	info := HeaderInfo{Brand: "M31A", Breadcrumb: "Chat", ModelName: "gpt-4"}
	header := BuildHeader(info, 80, Full, tm, cache)

	if !strings.Contains(header, "M31A") {
		t.Error("Header must contain brand name")
	}
	if !strings.Contains(header, "gpt-4") {
		t.Error("Header must contain model name")
	}
}

func TestM1_HeaderShowsBreadcrumb(t *testing.T) {
	t.Parallel()
	tm := theme.Default()
	cache := theme.NewStyleCache(tm)
	info := HeaderInfo{Brand: "M31A", Breadcrumb: "Planning"}
	header := BuildHeader(info, 80, Full, tm, cache)

	if !strings.Contains(header, "Planning") {
		t.Error("Header must contain breadcrumb")
	}
}

func TestM1_HeaderCompactNoBreadcrumb(t *testing.T) {
	t.Parallel()
	tm := theme.Default()
	cache := theme.NewStyleCache(tm)
	info := HeaderInfo{Brand: "M31A", Breadcrumb: "Chat"}
	header := BuildHeader(info, 50, Compact, tm, cache)

	if strings.Contains(header, "Chat") {
		t.Error("Compact header should not contain breadcrumb")
	}
}

func TestM1_HeaderWidthExact(t *testing.T) {
	t.Parallel()
	tm := theme.Default()
	cache := theme.NewStyleCache(tm)
	info := HeaderInfo{Brand: "M31A", Breadcrumb: "Chat", ModelName: "gpt-4"}
	widths := []int{40, 60, 80, 100, 120}
	for _, w := range widths {
		header := BuildHeader(info, w, Detect(w), tm, cache)
		visualW := lipgloss.Width(header)
		if visualW != w {
			t.Errorf("Header visual width = %d, want %d", visualW, w)
		}
	}
}

// ─── M1 Footer Simplification Tests ────────────────────────────────────────────

func TestM1_FooterNoCost(t *testing.T) {
	t.Parallel()
	tm := theme.Default()
	cache := theme.NewStyleCache(tm)
	info := FooterInfo{
		Cwd:           "project",
		Cost:          0.05,
		ShowCost:      true,
		TokenCount:    5000,
		KeyboardHints: []string{"ctrl+p cmds"},
	}
	footer := BuildFooter(info, 80, Full, tm, cache)

	if strings.Contains(footer, "$") {
		t.Error("Simplified footer should not contain cost")
	}
	if strings.Contains(footer, "ctx") {
		t.Error("Simplified footer should not contain token count")
	}
}

func TestM1_FooterNoContextRing(t *testing.T) {
	t.Parallel()
	tm := theme.Default()
	cache := theme.NewStyleCache(tm)
	info := FooterInfo{
		Cwd:        "project",
		TokenCount: 5000,
		ShowCost:   true,
	}
	footer := BuildFooter(info, 80, Full, tm, cache)

	// Context ring uses █ and ░ characters
	if strings.Contains(footer, "█") || strings.Contains(footer, "░") {
		t.Error("Simplified footer should not contain context ring")
	}
}

func TestM1_FooterShowsCwd(t *testing.T) {
	t.Parallel()
	tm := theme.Default()
	cache := theme.NewStyleCache(tm)
	info := FooterInfo{Cwd: "myproject"}
	footer := BuildFooter(info, 80, Full, tm, cache)

	if !strings.Contains(footer, "myproject") {
		t.Error("Footer must contain cwd")
	}
}

func TestM1_FooterShowsGitBranch(t *testing.T) {
	t.Parallel()
	tm := theme.Default()
	cache := theme.NewStyleCache(tm)
	info := FooterInfo{Cwd: "project", GitBranch: "main"}
	footer := BuildFooter(info, 80, Full, tm, cache)

	if !strings.Contains(footer, "main") {
		t.Error("Footer must contain git branch")
	}
}

func TestM1_FooterShowsHints(t *testing.T) {
	t.Parallel()
	tm := theme.Default()
	cache := theme.NewStyleCache(tm)
	info := FooterInfo{
		Cwd:           "project",
		KeyboardHints: []string{"ctrl+p cmds", "ctrl+b sidebar"},
	}
	footer := BuildFooter(info, 80, Full, tm, cache)

	if !strings.Contains(footer, "ctrl+p") {
		t.Error("Footer must contain keyboard hints")
	}
}

func TestM1_FooterCompactNoHints(t *testing.T) {
	t.Parallel()
	tm := theme.Default()
	cache := theme.NewStyleCache(tm)
	info := FooterInfo{
		Cwd:           "project",
		KeyboardHints: []string{"ctrl+p cmds"},
	}
	footer := BuildFooter(info, 50, Compact, tm, cache)

	if strings.Contains(footer, "ctrl+p") {
		t.Error("Compact footer should not contain keyboard hints")
	}
}

func TestM1_FooterWidthExact(t *testing.T) {
	t.Parallel()
	tm := theme.Default()
	cache := theme.NewStyleCache(tm)
	info := FooterInfo{Cwd: "project", GitBranch: "main"}
	widths := []int{40, 60, 80, 100}
	for _, w := range widths {
		footer := BuildFooter(info, w, Detect(w), tm, cache)
		visualW := lipgloss.Width(footer)
		if visualW != w {
			t.Errorf("Footer visual width = %d, want %d", visualW, w)
		}
	}
}

// ─── M1 UltraWide Breakpoint Tests ─────────────────────────────────────────────

func TestM1_UltraWideBreakpoint(t *testing.T) {
	t.Parallel()
	tests := []struct {
		width int
		want  Breakpoint
	}{
		{119, Full},
		{120, UltraWide},
		{150, UltraWide},
		{200, UltraWide},
	}
	for _, tt := range tests {
		got := Detect(tt.width)
		if got != tt.want {
			t.Errorf("Detect(%d) = %d, want %d", tt.width, got, tt.want)
		}
	}
}

func TestM1_UltraWideConstants(t *testing.T) {
	t.Parallel()
	if UltraWideMax != 119 {
		t.Errorf("UltraWideMax = %d, want 119", UltraWideMax)
	}
}

func TestM1_FullRangeIs80To119(t *testing.T) {
	t.Parallel()
	for w := 80; w <= 119; w++ {
		got := Detect(w)
		if got != Full {
			t.Errorf("Detect(%d) = %d, want Full (%d)", w, got, Full)
		}
	}
}

func TestM1_UltraWideRangeIs120Plus(t *testing.T) {
	t.Parallel()
	for w := 120; w <= 200; w += 10 {
		got := Detect(w)
		if got != UltraWide {
			t.Errorf("Detect(%d) = %d, want UltraWide (%d)", w, got, UltraWide)
		}
	}
}

// ─── M1 Page Layout Tests ─────────────────────────────────────────────────────

func TestM1_RenderPageRowCount(t *testing.T) {
	t.Parallel()
	tm := theme.Default()
	cache := theme.NewStyleCache(tm)
	chrome := PageChrome{Width: 80, Height: 24}
	content := "line1\nline2\nline3"
	header := HeaderInfo{Brand: "M31A"}
	footer := FooterInfo{Cwd: "project"}

	result := RenderPage(chrome, content, header, footer, tm, cache)
	lines := strings.Split(result, "\n")

	if len(lines) != 24 {
		t.Errorf("RenderPage produced %d lines, want 24", len(lines))
	}
}

func TestM1_RenderPageContentOverflow(t *testing.T) {
	t.Parallel()
	tm := theme.Default()
	cache := theme.NewStyleCache(tm)
	chrome := PageChrome{Width: 80, Height: 5}
	content := "line1\nline2\nline3\nline4\nline5\nline6\nline7\nline8"
	header := HeaderInfo{Brand: "M31A"}
	footer := FooterInfo{Cwd: "project"}

	result := RenderPage(chrome, content, header, footer, tm, cache)
	lines := strings.Split(result, "\n")

	if len(lines) != 5 {
		t.Errorf("RenderPage produced %d lines, want 5", len(lines))
	}
}

func TestM1_RenderPageMinimal(t *testing.T) {
	t.Parallel()
	tm := theme.Default()
	cache := theme.NewStyleCache(tm)
	chrome := PageChrome{Width: 80, Height: 3}
	content := ""
	header := HeaderInfo{Brand: "M31A"}
	footer := FooterInfo{Cwd: "project"}

	result := RenderPage(chrome, content, header, footer, tm, cache)
	lines := strings.Split(result, "\n")

	if len(lines) != 3 {
		t.Errorf("RenderPage produced %d lines, want 3", len(lines))
	}
}

// ─── M1 ChromeHeight Tests ────────────────────────────────────────────────────

func TestM1_ChromeHeight(t *testing.T) {
	t.Parallel()
	if ChromeHeight != 2 {
		t.Errorf("ChromeHeight = %d, want 2", ChromeHeight)
	}
}

func TestM1_ContentHeightCalculation(t *testing.T) {
	t.Parallel()
	tests := []struct {
		termHeight  int
		wantContent int
	}{
		{24, 22},
		{40, 38},
		{10, 8},
		{2, 1},
		{1, 1},
	}
	for _, tt := range tests {
		chrome := PageChrome{Width: 80, Height: tt.termHeight}
		got := chrome.ContentHeight()
		if got != tt.wantContent {
			t.Errorf("PageChrome{Height: %d}.ContentHeight() = %d, want %d", tt.termHeight, got, tt.wantContent)
		}
	}
}

// ─── M1 Responsive Behaviour Tests ────────────────────────────────────────────

func TestM1_Responsive_NarrowWidth(t *testing.T) {
	t.Parallel()
	tm := theme.Default()
	cache := theme.NewStyleCache(tm)

	// At 40 cols: UltraNarrow breakpoint, minimal chrome
	header := BuildHeader(HeaderInfo{Brand: "M31A"}, 40, UltraNarrow, tm, cache)
	footer := BuildFooter(FooterInfo{Cwd: "project"}, 40, UltraNarrow, tm, cache)

	if lipgloss.Width(header) != 40 {
		t.Errorf("Narrow header width = %d, want 40", lipgloss.Width(header))
	}
	if lipgloss.Width(footer) != 40 {
		t.Errorf("Narrow footer width = %d, want 40", lipgloss.Width(footer))
	}
}

func TestM1_Responsive_CompactWidth(t *testing.T) {
	t.Parallel()
	tm := theme.Default()
	cache := theme.NewStyleCache(tm)

	// At 59 cols: Compact breakpoint, no breadcrumb, no hints
	header := BuildHeader(HeaderInfo{Brand: "M31A", Breadcrumb: "Chat"}, 59, Compact, tm, cache)
	footer := BuildFooter(FooterInfo{Cwd: "project", KeyboardHints: []string{"ctrl+p cmds"}}, 59, Compact, tm, cache)

	if lipgloss.Width(header) != 59 {
		t.Errorf("Compact header width = %d, want 59", lipgloss.Width(header))
	}
	if lipgloss.Width(footer) != 59 {
		t.Errorf("Compact footer width = %d, want 59", lipgloss.Width(footer))
	}
	// Breadcrumb should not appear in compact header
	if strings.Contains(header, "Chat") {
		t.Error("Compact header should not contain breadcrumb")
	}
	// Hints should not appear in compact footer
	if strings.Contains(footer, "ctrl+p") {
		t.Error("Compact footer should not contain hints")
	}
}

func TestM1_Responsive_StandardWidth(t *testing.T) {
	t.Parallel()
	tm := theme.Default()
	cache := theme.NewStyleCache(tm)

	// At 80 cols: Full breakpoint, all chrome
	header := BuildHeader(HeaderInfo{Brand: "M31A", Breadcrumb: "Chat", ModelName: "gpt-4"}, 80, Full, tm, cache)
	footer := BuildFooter(FooterInfo{Cwd: "project", GitBranch: "main", KeyboardHints: []string{"ctrl+p cmds"}}, 80, Full, tm, cache)

	if lipgloss.Width(header) != 80 {
		t.Errorf("Full header width = %d, want 80", lipgloss.Width(header))
	}
	if lipgloss.Width(footer) != 80 {
		t.Errorf("Full footer width = %d, want 80", lipgloss.Width(footer))
	}
	if !strings.Contains(header, "Chat") {
		t.Error("Full header should contain breadcrumb")
	}
	if !strings.Contains(header, "gpt-4") {
		t.Error("Full header should contain model name")
	}
}

func TestM1_Responsive_UltraWide(t *testing.T) {
	t.Parallel()
	tm := theme.Default()
	cache := theme.NewStyleCache(tm)

	// At 120+ cols: UltraWide breakpoint
	header := BuildHeader(HeaderInfo{Brand: "M31A", Breadcrumb: "Chat", ModelName: "gpt-4"}, 120, UltraWide, tm, cache)
	footer := BuildFooter(FooterInfo{Cwd: "project", GitBranch: "main", KeyboardHints: []string{"ctrl+p cmds"}}, 120, UltraWide, tm, cache)

	if lipgloss.Width(header) != 120 {
		t.Errorf("UltraWide header width = %d, want 120", lipgloss.Width(header))
	}
	if lipgloss.Width(footer) != 120 {
		t.Errorf("UltraWide footer width = %d, want 120", lipgloss.Width(footer))
	}
}

func TestM1_Responsive_VeryWide(t *testing.T) {
	t.Parallel()
	tm := theme.Default()
	cache := theme.NewStyleCache(tm)

	// At 160 cols: UltraWide, no clipping
	header := BuildHeader(HeaderInfo{Brand: "M31A", Breadcrumb: "Chat", ModelName: "gpt-4"}, 160, UltraWide, tm, cache)
	footer := BuildFooter(FooterInfo{Cwd: "project", GitBranch: "main", KeyboardHints: []string{"ctrl+p cmds"}}, 160, UltraWide, tm, cache)

	if lipgloss.Width(header) != 160 {
		t.Errorf("VeryWide header width = %d, want 160", lipgloss.Width(header))
	}
	if lipgloss.Width(footer) != 160 {
		t.Errorf("VeryWide footer width = %d, want 160", lipgloss.Width(footer))
	}
}

func TestM1_Responsive_SidebarVisibility(t *testing.T) {
	t.Parallel()
	tests := []struct {
		width    int
		expected bool
	}{
		{40, false}, // Narrow: no sidebar
		{60, false}, // Compact: no sidebar
		{79, false}, // Just below threshold
		{80, true},  // Full: sidebar visible
		{120, true}, // UltraWide: sidebar visible
		{160, true}, // VeryWide: sidebar visible
	}
	for _, tt := range tests {
		got := ShowSidebar(tt.width)
		if got != tt.expected {
			t.Errorf("ShowSidebar(%d) = %v, want %v", tt.width, got, tt.expected)
		}
	}
}

func TestM1_Responsive_FooterHintsVisibility(t *testing.T) {
	t.Parallel()
	tests := []struct {
		width    int
		expected bool
	}{
		{40, false}, // Narrow: no hints
		{59, false}, // Just below threshold
		{60, true},  // Compact+: hints visible
		{80, true},  // Full: hints visible
		{120, true}, // UltraWide: hints visible
	}
	for _, tt := range tests {
		got := ShowFooterHints(tt.width)
		if got != tt.expected {
			t.Errorf("ShowFooterHints(%d) = %v, want %v", tt.width, got, tt.expected)
		}
	}
}

// ─── M1 Layout Grid Stability Tests ───────────────────────────────────────────

func TestM1_LayoutGrid_NoLayoutJumps(t *testing.T) {
	t.Parallel()

	// Verify that content height is stable across widths
	widths := []int{40, 60, 80, 120, 160}
	for _, w := range widths {
		chrome := PageChrome{Width: w, Height: 24}
		contentH := chrome.ContentHeight()
		if contentH != 22 {
			t.Errorf("Width %d: ContentHeight = %d, want 22", w, contentH)
		}
	}
}

func TestM1_LayoutGrid_ConversationOwnsLargestArea(t *testing.T) {
	t.Parallel()

	// At 80 cols, 24 rows: content should be 22 rows (91.7% of screen)
	chrome := PageChrome{Width: 80, Height: 24}
	contentH := chrome.ContentHeight()
	totalH := 24

	pct := float64(contentH) / float64(totalH) * 100
	if pct < 90 {
		t.Errorf("Content occupies %.1f%% of screen, want >= 90%%", pct)
	}
}

func TestM1_LayoutGrid_StableConversationWidth(t *testing.T) {
	t.Parallel()

	// Content width should always equal terminal width (no sidebar eating into it at Full+)
	widths := []int{80, 120, 160}
	for _, w := range widths {
		chrome := PageChrome{Width: w, Height: 24}
		contentW := chrome.ContentWidth()
		if contentW != w {
			t.Errorf("Width %d: ContentWidth = %d, want %d", w, contentW, w)
		}
	}
}

// ─── M1 Sidebar Overlay Tests ─────────────────────────────────────────────────

func TestM1_SidebarOverlay_NarrowTerminal(t *testing.T) {
	t.Parallel()
	// At narrow widths, sidebar should be overlay (not inline)
	// This is tested via ShowSidebar returning false
	if ShowSidebar(60) {
		t.Error("Sidebar should not be inline at 60 cols (should be overlay)")
	}
	if ShowSidebar(79) {
		t.Error("Sidebar should not be inline at 79 cols (should be overlay)")
	}
}

func TestM1_SidebarOverlay_WideTerminal(t *testing.T) {
	t.Parallel()
	// At wide widths, sidebar should be inline
	if !ShowSidebar(80) {
		t.Error("Sidebar should be inline at 80 cols")
	}
	if !ShowSidebar(120) {
		t.Error("Sidebar should be inline at 120 cols")
	}
}

// ─── M1 Minimum Size Tests ────────────────────────────────────────────────────

func TestM1_MinimumSize_TooNarrow(t *testing.T) {
	t.Parallel()
	tm := theme.Default()
	cache := theme.NewStyleCache(tm)

	// Below 20 cols: header should be empty spaces
	header := BuildHeader(HeaderInfo{Brand: "M31A"}, 15, UltraNarrow, tm, cache)
	if lipgloss.Width(header) != 15 {
		t.Errorf("Very narrow header width = %d, want 15", lipgloss.Width(header))
	}
}

func TestM1_MinimumSize_Footer(t *testing.T) {
	t.Parallel()
	tm := theme.Default()
	cache := theme.NewStyleCache(tm)

	// Below 10 cols: footer should be empty spaces
	footer := BuildFooter(FooterInfo{Cwd: "p"}, 5, UltraNarrow, tm, cache)
	if lipgloss.Width(footer) != 5 {
		t.Errorf("Very narrow footer width = %d, want 5", lipgloss.Width(footer))
	}
}

// ─── M1 Large Monitor Tests ───────────────────────────────────────────────────

func TestM1_LargeMonitor_256Cols(t *testing.T) {
	t.Parallel()
	tm := theme.Default()
	cache := theme.NewStyleCache(tm)

	header := BuildHeader(HeaderInfo{Brand: "M31A", Breadcrumb: "Chat", ModelName: "gpt-4"}, 256, UltraWide, tm, cache)
	footer := BuildFooter(FooterInfo{Cwd: "project", GitBranch: "main", KeyboardHints: []string{"ctrl+p cmds"}}, 256, UltraWide, tm, cache)

	if lipgloss.Width(header) != 256 {
		t.Errorf("256-col header width = %d, want 256", lipgloss.Width(header))
	}
	if lipgloss.Width(footer) != 256 {
		t.Errorf("256-col footer width = %d, want 256", lipgloss.Width(footer))
	}
}

func TestM1_LargeMonitor_ContentHeight(t *testing.T) {
	t.Parallel()
	// Large monitor: 240 rows, content should be 238
	chrome := PageChrome{Width: 256, Height: 240}
	if chrome.ContentHeight() != 238 {
		t.Errorf("240-row content height = %d, want 238", chrome.ContentHeight())
	}
}

// ─── M1 No Duplicated Information Tests ───────────────────────────────────────

func TestM1_NoDuplicatedInfo_HeaderNoFooterRepeat(t *testing.T) {
	t.Parallel()
	tm := theme.Default()
	cache := theme.NewStyleCache(tm)

	header := BuildHeader(HeaderInfo{Brand: "M31A", Breadcrumb: "Chat", ModelName: "gpt-4", Provider: "openrouter"}, 80, Full, tm, cache)
	footer := BuildFooter(FooterInfo{Cwd: "project", GitBranch: "main"}, 80, Full, tm, cache)

	// Provider should NOT appear in header (simplified chrome)
	if strings.Contains(header, "[OR]") || strings.Contains(header, "[ZEN]") {
		t.Error("Header should not contain provider badge (no duplication)")
	}

	// Cost should NOT appear in footer (simplified chrome)
	if strings.Contains(footer, "$") {
		t.Error("Footer should not contain cost (no duplication)")
	}
}
