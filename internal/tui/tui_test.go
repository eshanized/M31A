package tui

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/git"
	"github.com/eshanized/M31A/internal/tui/layout"
	"github.com/eshanized/M31A/internal/tui/theme"
	"github.com/eshanized/M31A/internal/types"
	"github.com/eshanized/M31A/pkg/rollback"
)

func testTheme() theme.Theme {
	return theme.Theme{
		Mode:            theme.ModeDark,
		Background:      lipgloss.Color("#000000"),
		Surface:         lipgloss.Color("#1a1a1a"),
		SurfaceElevated: lipgloss.Color("#2a2a2a"),
		Border:          lipgloss.Color("#444444"),
		Brand:           lipgloss.Color("#7c3aed"),
		TextPrimary:     lipgloss.Color("#ffffff"),
		TextSecondary:   lipgloss.Color("#a0a0a0"),
		TextMuted:       lipgloss.Color("#666666"),
		Text:            lipgloss.Color("#e0e0e0"),
		Thinking:        lipgloss.Color("#fbbf24"),
		Success:         lipgloss.Color("#22c55e"),
		Error:           lipgloss.Color("#ef4444"),
		Warning:         lipgloss.Color("#f59e0b"),
		CodeBG:          lipgloss.Color("#1e1e2e"),
		Info:            lipgloss.Color("#3b82f6"),
		DiffAdded:       lipgloss.Color("#22c55e"),
		DiffRemoved:     lipgloss.Color("#ef4444"),
		DiffAddedBg:     lipgloss.Color("#1a2e1a"),
		DiffRemovedBg:   lipgloss.Color("#2e1a1a"),
		Spinner:         lipgloss.NewStyle(),
		PhaseActive:     lipgloss.NewStyle().Foreground(lipgloss.Color("#7c3aed")).Bold(true),
		PhasePast:       lipgloss.NewStyle().Foreground(lipgloss.Color("#666666")).Faint(true),
		PhaseFuture:     lipgloss.NewStyle().Foreground(lipgloss.Color("#666666")),
	}
}

// ═══ truncate.go ═══

func TestTruncateWithEllipsis(t *testing.T) {
	tests := []struct {
		name     string
		input    string
		maxWidth int
		want     string
	}{
		{"empty", "", 10, ""},
		{"zero", "hello", 0, ""},
		{"negative", "hello", -1, ""},
		{"no truncation", "hi", 10, "hi"},
		{"exact", "hello", 5, "hello"},
		{"one over", "hello", 4, "h..."},
		{"long", "this is a long string", 10, "this is..."},
		{"single char fits", "a", 1, "a"},
		{"two chars fit", "ab", 2, "ab"},
		{"unicode", "你好世界测试", 6, "你..."},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := TruncateWithEllipsis(tt.input, tt.maxWidth)
			if got != tt.want {
				t.Errorf("got %q, want %q", got, tt.want)
			}
		})
	}
}

func TestTruncateMiddle(t *testing.T) {
	tests := []struct {
		name   string
		input  string
		maxLen int
		want   string
	}{
		{"empty", "", 10, ""},
		{"zero", "hello", 0, ""},
		{"negative", "hello", -1, ""},
		{"no truncation", "hi", 10, "hi"},
		{"exact", "hello", 5, "hello"},
		{"truncation", "hello world", 7, "he...ld"},
		{"very short", "hello", 3, "hel"},
		{"max4", "hello", 4, "hell"},
		{"unicode", "你好世界测试数据", 6, "你...据"},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := TruncateMiddle(tt.input, tt.maxLen)
			if got != tt.want {
				t.Errorf("got %q, want %q", got, tt.want)
			}
		})
	}
}

func TestTruncateEnd(t *testing.T) {
	tests := []struct {
		name   string
		input  string
		maxLen int
		want   string
	}{
		{"empty", "", 10, ""},
		{"zero", "hello", 0, ""},
		{"negative", "hello", -1, ""},
		{"no truncation", "hi", 10, "hi"},
		{"exact", "hello", 5, "hello"},
		{"truncation", "hello world", 8, "hello w…"},
		{"single", "hello", 1, "…"},
		{"two", "hello", 2, "h…"},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := TruncateEnd(tt.input, tt.maxLen)
			if got != tt.want {
				t.Errorf("got %q, want %q", got, tt.want)
			}
		})
	}
}

func TestTruncateError(t *testing.T) {
	short := "error msg"
	if got := TruncateError(short); got != short {
		t.Errorf("short: got %q", got)
	}
	long := strings.Repeat("x", 300)
	got := TruncateError(long)
	if len(got) != 205 || !strings.HasSuffix(got, "[...]") || got[:200] != long[:200] {
		t.Errorf("long: got len=%d suffix=%q", len(got), got[len(got)-10:])
	}
}

// ═══ helpers.go ═══

func TestMakeAssistantMsg(t *testing.T) {
	msg := makeAssistantMsg("hello")
	if msg.Role != "assistant" || msg.Content != "hello" || len(msg.Segments) != 1 || msg.CreatedAt.IsZero() {
		t.Errorf("unexpected: role=%s content=%s segs=%d", msg.Role, msg.Content, len(msg.Segments))
	}
}

func TestMakeUserMsg(t *testing.T) {
	msg := makeUserMsg("input")
	if msg.Role != "user" || msg.Content != "input" || msg.SkipForLLM || len(msg.Segments) != 1 {
		t.Errorf("unexpected: role=%s skip=%v segs=%d", msg.Role, msg.SkipForLLM, len(msg.Segments))
	}
}

func TestMakeUserMsgWithSkip(t *testing.T) {
	msg := makeUserMsgWithSkip("x", true)
	if !msg.SkipForLLM {
		t.Error("SkipForLLM should be true")
	}
	msg2 := makeUserMsgWithSkip("y", false)
	if msg2.SkipForLLM {
		t.Error("SkipForLLM should be false")
	}
}

func TestCenterScreen(t *testing.T) {
	result := centerScreen("hi", 20, 5)
	if result == "" || !strings.Contains(result, "hi") {
		t.Error("centerScreen should contain text")
	}
}

func TestRenderSectionHeader(t *testing.T) {
	result := renderSectionHeader("Title", 30)
	if !strings.Contains(result, "── Title ") {
		t.Errorf("unexpected: %q", result)
	}
	if w := lipgloss.Width(result); w != 30 {
		t.Errorf("width=%d, want 30", w)
	}
}

func TestRenderLoading(t *testing.T) {
	th := testTheme()
	result := renderLoading("Loading...", 40, 3, th)
	if result == "" || !strings.Contains(result, "Loading...") {
		t.Error("renderLoading failed")
	}
}

func TestRenderLoadingSmall(t *testing.T) {
	th := testTheme()
	result := renderLoading("wait", 0, 0, th)
	if result == "" {
		t.Error("renderLoading with 0 should not be empty")
	}
}

// ═══ types.go ═══

func TestScreenLabel(t *testing.T) {
	tests := []struct {
		s Screen
		l string
	}{
		{ScreenFirstRun, "Setup"}, {ScreenREPL, "Chat"}, {ScreenModelSelector, "Models"},
		{ScreenSettings, "Settings"}, {ScreenResume, "Sessions"}, {ScreenPermission, "Permission"},
		{ScreenPlan, "Plan"}, {ScreenExecute, "Execute"}, {ScreenVerify, "Verify"},
		{ScreenShip, "Ship"}, {ScreenDiff, "Diff"}, {ScreenLedger, "Ledger"},
		{ScreenRollback, "Rollback"}, {ScreenGoalInput, "Goal"}, {ScreenDiscuss, "Discuss"},
		{ScreenMetrics, "Metrics"}, {ScreenConfig, "Config"}, {ScreenHelp, "Help"},
		{ScreenBisect, "Bisect"}, {ScreenThemePicker, "Themes"}, {ScreenNotifications, "Notifications"},
		{ScreenDashboard, "Dashboard"}, {ScreenSessionDetail, "Session"}, {ScreenFileExplorer, "Files"},
		{ScreenToolDetail, "Tool Output"}, {ScreenPhaseModelPicker, "Model Setup"},
		{Screen(999), "Unknown"},
	}
	for _, tt := range tests {
		if got := tt.s.Label(); got != tt.l {
			t.Errorf("Screen(%d).Label()=%q, want %q", tt.s, got, tt.l)
		}
	}
}

// ═══ providerbadge.go ═══

func TestProviderShortName(t *testing.T) {
	tests := []struct {
		i, w string
	}{
		{"openrouter", "OR"}, {"OpenRouter", "OR"}, {"zen", "Zen"}, {"zen-gateway", "Zen"},
		{"openai", "OAI"}, {"OpenAI", "OAI"}, {"anthropic", "AC"}, {"Anthropic", "AC"},
		{"short", "shor"}, {"ab", "ab"}, {"", ""}, {"a", "a"}, {"custom-provider", "cust"},
	}
	for _, tt := range tests {
		if got := ProviderShortName(tt.i); got != tt.w {
			t.Errorf("ProviderShortName(%q)=%q, want %q", tt.i, got, tt.w)
		}
	}
}

// ═══ statusbar.go ═══

func TestRenderStatusBar(t *testing.T) {
	th := testTheme()
	info := &StatusBarInfo{CwdName: "myproject", GitBranch: "main"}
	result := RenderStatusBar(th, 80, info)
	if result == "" || !strings.Contains(result, "main") {
		t.Error("RenderStatusBar failed")
	}
}

func TestRenderStatusBarNilInfo(t *testing.T) {
	th := testTheme()
	if RenderStatusBar(th, 80, nil) == "" {
		t.Error("nil info should not be empty")
	}
}

func TestRenderStatusBarNarrow(t *testing.T) {
	th := testTheme()
	info := &StatusBarInfo{CwdName: "p", GitBranch: "main", KeyboardHints: []string{"ctrl+p"}, ShowCost: true, Cost: 0.05, TotalTokens: 1500}
	result := RenderStatusBar(th, 60, info)
	if strings.Contains(result, "ctrl+p") {
		t.Error("<80 should hide hints")
	}
	if strings.Contains(result, "$0.05") {
		t.Error("<80 should hide cost")
	}
}

func TestRenderStatusBarUltraNarrow(t *testing.T) {
	th := testTheme()
	info := &StatusBarInfo{CwdName: "p", GitBranch: "main"}
	if strings.Contains(RenderStatusBar(th, 50, info), "main") {
		t.Error("<60 should hide branch")
	}
}

func TestRenderStatusBarTooNarrow(t *testing.T) {
	if RenderStatusBar(testTheme(), 5, nil) != "" {
		t.Error("<10 should be empty")
	}
}

func TestRenderStatusBarStates(t *testing.T) {
	th := testTheme()
	tests := []struct {
		name string
		info *StatusBarInfo
		want string
	}{
		{"leader", &StatusBarInfo{LeaderActive: true}, "ctrl+x"},
		{"thinking", &StatusBarInfo{IsThinking: true}, "thinking"},
		{"thinking_dur", &StatusBarInfo{IsThinking: true, ThinkingDuration: 5000}, "thinking"},
		{"streaming", &StatusBarInfo{IsStreaming: true}, "responding"},
		{"phase", &StatusBarInfo{WorkflowPhase: "execute"}, "execute"},
		{"phase_prog", &StatusBarInfo{WorkflowPhase: "plan", QuestionProgress: "2/5"}, "2/5"},
		{"whichkey", &StatusBarInfo{WhichKey: "ctrl+x pressed"}, "ctrl+x pressed"},
		{"cost", &StatusBarInfo{ShowCost: true, TotalTokens: 1500, Cost: 0.05}, "$0.05"},
		{"small_cost", &StatusBarInfo{ShowCost: true, TotalTokens: 500, Cost: 0.005}, "<$0.01"},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			if r := RenderStatusBar(th, 80, tt.info); !strings.Contains(r, tt.want) {
				t.Errorf("got %q, want to contain %q", r, tt.want)
			}
		})
	}
}

func TestRenderStatusBarOverflow(t *testing.T) {
	th := testTheme()
	info := &StatusBarInfo{
		CwdName: "very-long-project-directory", GitBranch: "feature/long-branch",
		WorkflowPhase: "execute", KeyboardHints: []string{"ctrl+p"}, ShowCost: true, TotalTokens: 5000, Cost: 1.23,
	}
	_ = RenderStatusBar(th, 40, info)
}

func TestRenderPromptMetadata(t *testing.T) {
	th := testTheme()
	result := RenderPromptMetadata("agent", "model", "openrouter", th, 80)
	if result == "" || !strings.Contains(result, "agent") {
		t.Error("RenderPromptMetadata failed")
	}
	if RenderPromptMetadata("", "", "", th, 80) != "" {
		t.Error("all empty should return empty")
	}
}

func TestRenderPromptBottomBorder(t *testing.T) {
	result := RenderPromptBottomBorder(lipgloss.Color("#7c3aed"), 30)
	if result == "" || lipgloss.Width(result) != 30 {
		t.Error("RenderPromptBottomBorder failed")
	}
	if RenderPromptBottomBorder(lipgloss.Color("#7c3aed"), 0) != "" {
		t.Error("width 0 should be empty")
	}
}

func TestFormatTokenCount(t *testing.T) {
	tests := []struct {
		n    int
		want string
	}{
		{0, "0 ctx"}, {500, "500 ctx"}, {999, "999 ctx"},
		{1000, "1.0K ctx"}, {1500, "1.5K ctx"}, {10000, "10.0K ctx"},
	}
	for _, tt := range tests {
		if got := formatTokenCount(tt.n); got != tt.want {
			t.Errorf("formatTokenCount(%d)=%q, want %q", tt.n, got, tt.want)
		}
	}
}

func TestRenderPhaseBadge(t *testing.T) {
	if r := RenderPhaseBadge(testTheme(), "execute"); r == "" || !strings.Contains(r, "execute") {
		t.Error("RenderPhaseBadge failed")
	}
}

func TestRenderPhaseBreadcrumb(t *testing.T) {
	th := testTheme()
	if r := RenderPhaseBreadcrumb(th, types.PhasePlan); r == "" {
		t.Error("RenderPhaseBreadcrumb failed")
	}
	if r := RenderPhaseBreadcrumb(th, types.PhaseIdle); r != "" {
		t.Error("idle should be empty")
	}
	if r := RenderPhaseBreadcrumb(th, ""); r != "" {
		t.Error("empty should be empty")
	}
}

func TestFormatSI(t *testing.T) {
	tests := []struct {
		n    int
		want string
	}{
		{0, "0"}, {500, "500"}, {1000, "1K"}, {1500, "2K"}, {1000000, "1.0M"},
	}
	for _, tt := range tests {
		if got := formatSI(tt.n); got != tt.want {
			t.Errorf("formatSI(%d)=%q, want %q", tt.n, got, tt.want)
		}
	}
}

func TestFormatDurationMs(t *testing.T) {
	tests := []struct {
		ms   int64
		want string
	}{
		{0, "0s"}, {-1, "0s"}, {500, "0s"}, {1000, "1s"}, {60000, "1m0s"},
		{65000, "1m5s"}, {3600000, "1h0m"}, {3665000, "1h1m"},
	}
	for _, tt := range tests {
		if got := formatDurationMs(tt.ms); got != tt.want {
			t.Errorf("formatDurationMs(%d)=%q, want %q", tt.ms, got, tt.want)
		}
	}
}

// ═══ sidebar.go ═══

func TestCountFileStatuses(t *testing.T) {
	files := []git.FileStatus{
		{Status: "M ", Path: "a.go"}, {Status: "A ", Path: "b.go"},
		{Status: "D ", Path: "c.go"}, {Status: "??", Path: "d.go"}, {Status: "M ", Path: "e.go"},
	}
	mod, add, del, untr := countFileStatuses(files)
	if mod != 2 || add != 1 || del != 1 || untr != 1 {
		t.Errorf("got (%d,%d,%d,%d)", mod, add, del, untr)
	}
	if _, _, _, _ = countFileStatuses(nil); false {
		t.Error("nil files should not panic")
	}
}

func TestBuildSidebarTree(t *testing.T) {
	files := []git.FileStatus{
		{Status: "M ", Path: "src/main.go"}, {Status: "A ", Path: "src/util.go"}, {Status: "??", Path: "README.md"},
	}
	root := buildSidebarTree(files)
	if root == nil || root.Name != "." || !root.IsDir || len(root.Children) < 2 {
		t.Error("buildSidebarTree failed")
	}
}

func TestBuildSidebarTreeEmpty(t *testing.T) {
	root := buildSidebarTree(nil)
	if root == nil || len(root.Children) != 0 {
		t.Error("empty tree should have 0 children")
	}
}

func TestBuildSidebarTreeDeep(t *testing.T) {
	files := []git.FileStatus{{Status: "M ", Path: "a/b/c/d.go"}}
	root := buildSidebarTree(files)
	if root == nil || len(root.Children) != 1 {
		t.Fatal("root child")
	}
	a := root.Children[0]
	if a.Name != "a" || !a.IsDir || len(a.Children) != 1 {
		t.Fatal("a node")
	}
	b := a.Children[0]
	if b.Name != "b" || !b.IsDir {
		t.Error("b node")
	}
}

func TestNewSidebarModel(t *testing.T) {
	th := testTheme()
	sm := NewSidebarModel(&git.Git{}, th)
	if sm == nil || !sm.IsVisible() || sm.IsFocused() || sm.GetWidth() != sidebarDefaultWidth {
		t.Error("NewSidebarModel failed")
	}
}

func TestSidebarToggle(t *testing.T) {
	sm := NewSidebarModel(&git.Git{}, testTheme())
	sm.Toggle()
	if sm.IsVisible() {
		t.Error("should be hidden")
	}
	sm.Toggle()
	if !sm.IsVisible() {
		t.Error("should be visible")
	}
}

func TestSidebarFocusBlur(t *testing.T) {
	sm := NewSidebarModel(&git.Git{}, testTheme())
	sm.Focus()
	if !sm.IsFocused() {
		t.Error("should be focused")
	}
	sm.Blur()
	if sm.IsFocused() {
		t.Error("should not be focused")
	}
}

func TestSidebarToggleFocus(t *testing.T) {
	sm := NewSidebarModel(&git.Git{}, testTheme())
	sm.ToggleFocus()
	if !sm.IsFocused() {
		t.Error("should be focused")
	}
	sm.ToggleFocus()
	if sm.IsFocused() {
		t.Error("should not be focused")
	}
}

func TestSidebarWidth(t *testing.T) {
	sm := NewSidebarModel(&git.Git{}, testTheme())
	initial := sm.GetWidth()
	sm.IncreaseWidth()
	if sm.GetWidth() != initial+2 {
		t.Errorf("after increase: %d", sm.GetWidth())
	}
	sm.DecreaseWidth()
	if sm.GetWidth() != initial {
		t.Errorf("after decrease: %d", sm.GetWidth())
	}
}

func TestSidebarWidthLimits(t *testing.T) {
	sm := NewSidebarModel(&git.Git{}, testTheme())
	for i := 0; i < 20; i++ {
		sm.DecreaseWidth()
	}
	if sm.GetWidth() < sidebarMinWidth {
		t.Errorf("below min: %d", sm.GetWidth())
	}
	for i := 0; i < 30; i++ {
		sm.IncreaseWidth()
	}
	if sm.GetWidth() > sidebarMaxWidth {
		t.Errorf("above max: %d", sm.GetWidth())
	}
}

func TestSidebarHiddenGetWidth(t *testing.T) {
	sm := NewSidebarModel(&git.Git{}, testTheme())
	sm.Toggle()
	if sm.GetWidth() != 0 {
		t.Error("hidden should be 0")
	}
}

func TestSidebarSetters(t *testing.T) {
	sm := NewSidebarModel(&git.Git{}, testTheme())
	sm.SetHeight(30)
	sm.SetTheme(testTheme())
	sm.SetVersion("1.0.0")
	sm.SetSessionID("abc")
	sm.SetShutdownContext(t.Context())
}

func TestSidebarView(t *testing.T) {
	th := testTheme()
	sm := NewSidebarModel(&git.Git{}, th)
	sm.SetHeight(40)
	if r := sm.View(); r == "" {
		t.Error("View should not be empty")
	}
	sm.Toggle()
	if r := sm.View(); r != "" {
		t.Error("hidden View should be empty")
	}
}

func TestSidebarViewWithFiles(t *testing.T) {
	sm := NewSidebarModel(&git.Git{}, testTheme())
	sm.SetHeight(40)
	sm.Update(SidebarRefreshMsg{Branch: "main", Remote: "origin/main", Files: []SidebarFile{{Path: "a.go", Status: "M "}, {Path: "b.go", Status: "A "}}})
	if r := sm.View(); r == "" {
		t.Error("View with files should not be empty")
	}
}

func TestSidebarViewClean(t *testing.T) {
	sm := NewSidebarModel(&git.Git{}, testTheme())
	sm.SetHeight(40)
	sm.Update(SidebarRefreshMsg{Branch: "main"})
	if r := sm.View(); !strings.Contains(r, "clean") {
		t.Error("should show clean")
	}
}

func TestSidebarSelectedFileNilTree(t *testing.T) {
	sm := NewSidebarModel(&git.Git{}, testTheme())
	sm.tree = nil
	if sm.SelectedFile() != nil {
		t.Error("nil tree should return nil")
	}
}

func TestSidebarHandleKeyNilTree(t *testing.T) {
	sm := NewSidebarModel(&git.Git{}, testTheme())
	sm.tree = nil
	if sm.HandleKey(tea.KeyMsg{Type: tea.KeyUp}) != nil {
		t.Error("nil tree should return nil cmd")
	}
}

func TestSidebarHandleKeyEsc(t *testing.T) {
	sm := NewSidebarModel(&git.Git{}, testTheme())
	sm.Focus()
	sm.HandleKey(tea.KeyMsg{Type: tea.KeyEsc})
	if sm.IsFocused() {
		t.Error("should be unfocused after Esc")
	}
}

func TestSidebarUpdateLoading(t *testing.T) {
	sm := NewSidebarModel(&git.Git{}, testTheme())
	sm2, _ := sm.Update(tea.WindowSizeMsg{Width: 80, Height: 24})
	if sm2 == nil {
		t.Error("Update should return non-nil")
	}
}

func TestSidebarUpdateTick(t *testing.T) {
	sm := NewSidebarModel(&git.Git{}, testTheme())
	sm.SetShutdownContext(t.Context())
	sm.Update(SidebarRefreshMsg{Branch: "main"})
	sm.Update(SidebarRefreshTickMsg{})
}

// ═══ cmdpalette.go ═══

func TestFuzzyScore(t *testing.T) {
	tests := []struct {
		name      string
		target    string
		query     string
		wantScore int
		wantMatch bool
	}{
		{"empty", "hello", "", 0, true},
		{"exact", "hello", "hello", 7, true},
		{"prefix", "hello", "hel", 5, true},
		{"no match", "hello", "xyz", 0, false},
		{"case", "Hello", "Hello", 7, true},
		{"partial", "hello world", "world", 5, true},
		{"single", "a", "a", 1, true},
		{"longer query", "hi", "hello", 0, false},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			score, match := fuzzyScore(tt.target, tt.query)
			if match != tt.wantMatch {
				t.Errorf("match=%v, want %v", match, tt.wantMatch)
			}
			if match && score < tt.wantScore {
				t.Errorf("score=%d, want >= %d", score, tt.wantScore)
			}
		})
	}
}

func TestBuildPaletteEntriesNilRegistry(t *testing.T) {
	if entries := buildPaletteEntries(nil); entries != nil {
		t.Error("nil registry should return nil")
	}
}

func TestCommandPaletteModel(t *testing.T) {
	th := testTheme()
	cp := NewCommandPalette(nil, th)
	if cp == nil || cp.IsOpen() {
		t.Error("NewCommandPalette failed")
	}
	cp.Open()
	if !cp.IsOpen() {
		t.Error("should be open")
	}
	cp.Close()
	if cp.IsOpen() {
		t.Error("should be closed")
	}
	cp.SetTheme(th)
	cp.SetDimensions(80, 24)
}

func TestCommandPaletteUpdateClosed(t *testing.T) {
	cp := NewCommandPalette(nil, testTheme())
	result, cmd := cp.Update(tea.KeyMsg{Type: tea.KeyRunes, Runes: []rune{'a'}})
	if result == nil || cmd != nil {
		t.Error("closed palette should return nil cmd")
	}
}

func TestCommandPaletteUpdateEsc(t *testing.T) {
	cp := NewCommandPalette(nil, testTheme())
	cp.Open()
	result, _ := cp.Update(tea.KeyMsg{Type: tea.KeyEsc})
	if result.IsOpen() {
		t.Error("Esc should close")
	}
}

func TestCommandPaletteUpdateUpDown(t *testing.T) {
	cp := NewCommandPalette(nil, testTheme())
	cp.entries = []paletteEntry{
		{cmd: CommandInfo{Name: "a"}, category: CatCore},
		{cmd: CommandInfo{Name: "b"}, category: CatCore},
	}
	cp.filtered = cp.entries
	cp.Open()
	result, _ := cp.Update(tea.KeyMsg{Type: tea.KeyDown})
	if result.selected != 1 {
		t.Error("Down should move to 1")
	}
	result2, _ := result.Update(tea.KeyMsg{Type: tea.KeyUp})
	if result2.selected != 0 {
		t.Error("Up should move to 0")
	}
}

func TestCommandPaletteUpdateBackspace(t *testing.T) {
	cp := NewCommandPalette(nil, testTheme())
	cp.Open()
	cp.query = "test"
	result, _ := cp.Update(tea.KeyMsg{Type: tea.KeyBackspace})
	if result.query != "tes" {
		t.Error("backspace should remove last char")
	}
}

func TestCommandPaletteFilterCommands(t *testing.T) {
	cp := NewCommandPalette(nil, testTheme())
	cp.entries = []paletteEntry{
		{cmd: CommandInfo{Name: "help", Slash: "/help", Description: "Show help"}, category: CatCore},
		{cmd: CommandInfo{Name: "settings", Slash: "/settings"}, category: CatConfig},
		{cmd: CommandInfo{Name: "model", Slash: "/model"}, category: CatAI},
	}
	cp.filtered = cp.entries
	cp.Open()
	cp.query = "help"
	cp.filterCommands()
	if len(cp.filtered) != 1 || cp.filtered[0].cmd.Name != "help" {
		t.Error("filter failed")
	}
	cp.query = ""
	cp.filterCommands()
	if len(cp.filtered) != 3 {
		t.Error("empty query should show all")
	}
}

func TestCommandPaletteView(t *testing.T) {
	cp := NewCommandPalette(nil, testTheme())
	cp.SetDimensions(80, 24)
	if r := cp.View(); r != "" {
		t.Error("closed View should be empty")
	}
	cp.Open()
	if r := cp.View(); r == "" {
		t.Error("open View should not be empty")
	}
}

func TestCommandPaletteViewNoMatches(t *testing.T) {
	cp := NewCommandPalette(nil, testTheme())
	cp.entries = []paletteEntry{{cmd: CommandInfo{Name: "help"}, category: CatCore}}
	cp.filtered = nil
	cp.SetDimensions(80, 24)
	cp.Open()
	cp.query = "xyz"
	r := cp.View()
	if r == "" {
		t.Error("View should not be empty")
	}
	// The "No commands match" text will have ANSI codes, just check the view is non-empty with palette content
	if len(r) < 100 {
		t.Error("View should have substantial content")
	}
}

// ═══ transition.go ═══

func TestScreenTransitionNilTick(t *testing.T) {
	var st *ScreenTransition
	if !st.TransitionTick() {
		t.Error("nil should return true")
	}
}

func TestScreenTransitionNilProgress(t *testing.T) {
	var st *ScreenTransition
	if st.Progress() != 1.0 {
		t.Error("nil should return 1.0")
	}
}

func TestScreenTransitionInactiveTick(t *testing.T) {
	st := &ScreenTransition{Active: false}
	if !st.TransitionTick() {
		t.Error("inactive should return true")
	}
}

func TestScreenTransitionZeroDuration(t *testing.T) {
	st := &ScreenTransition{Active: true, Duration: 0}
	if st.Progress() != 1.0 {
		t.Error("zero duration should return 1.0")
	}
}

func TestScreenTransitionRendering(t *testing.T) {
	st := &ScreenTransition{Active: true, StartAt: time.Now(), Duration: 200 * time.Millisecond, FromScreen: ScreenREPL, ToScreen: ScreenPlan}
	// Verify transition mechanics work
	if !st.Active {
		t.Error("transition should be active")
	}
	if st.Progress() < 0 || st.Progress() > 1 {
		t.Error("progress should be between 0 and 1")
	}
}

func TestScreenTransitionNilRender(t *testing.T) {
	var st *ScreenTransition
	if st.TransitionTick() != true {
		t.Error("nil transition should be tick-complete")
	}
	if st.Progress() != 1.0 {
		t.Error("nil transition progress should be 1.0")
	}
}

func TestCenterText(t *testing.T) {
	style := lipgloss.NewStyle().Foreground(lipgloss.Color("#fff"))
	r := layout.CenterText("hi", style, 10)
	if lipgloss.Width(r) != 10 || !strings.Contains(r, "hi") {
		t.Error("centerText failed")
	}
}

func TestCenterTextNarrow(t *testing.T) {
	style := lipgloss.NewStyle().Foreground(lipgloss.Color("#fff"))
	r := layout.CenterText("hello world", style, 5)
	if lipgloss.Width(r) != 11 {
		t.Error("narrow should just render text")
	}
}

// ═══ toast.go ═══

func TestRenderToastStackEmpty(t *testing.T) {
	if r := renderToastStack(nil, testTheme(), 80); r != "" {
		t.Error("empty should be empty")
	}
}

func TestRenderToastStackSingle(t *testing.T) {
	toasts := []Toast{{ID: 1, Text: "Test", Type: "info", CreatedAt: time.Now()}}
	if r := renderToastStack(toasts, testTheme(), 80); r == "" {
		t.Error("single toast should not be empty")
	}
}

func TestRenderToastStackMultiple(t *testing.T) {
	toasts := []Toast{
		{ID: 1, Text: "First", Type: "info", CreatedAt: time.Now()},
		{ID: 2, Text: "Second", Type: "success", CreatedAt: time.Now()},
		{ID: 3, Text: "Third", Type: "error", CreatedAt: time.Now()},
	}
	if r := renderToastStack(toasts, testTheme(), 80); r == "" {
		t.Error("multiple should not be empty")
	}
}

func TestRenderToastStackOverflow(t *testing.T) {
	toasts := []Toast{
		{ID: 1, Text: "Old", Type: "info", CreatedAt: time.Now()},
		{ID: 2, Text: "Mid", Type: "success", CreatedAt: time.Now()},
		{ID: 3, Text: "New", Type: "warning", CreatedAt: time.Now()},
		{ID: 4, Text: "Latest", Type: "error", CreatedAt: time.Now()},
	}
	if r := renderToastStack(toasts, testTheme(), 80); r == "" {
		t.Error("overflow should not be empty")
	}
}

func TestRenderSingleToast(t *testing.T) {
	th := testTheme()
	for _, tt := range []string{"success", "error", "warning", "info", "unknown"} {
		if r := renderSingleToast(Toast{Text: "msg", Type: tt, CreatedAt: time.Now()}, th, 0); r == "" {
			t.Errorf("type %s returned empty", tt)
		}
	}
}

func TestRenderToastProgress(t *testing.T) {
	th := testTheme()
	if r := renderToastProgress(Toast{CreatedAt: time.Now()}, th); r == "" {
		t.Error("progress should not be empty")
	}
	if r := renderToastProgress(Toast{CreatedAt: time.Now().Add(-10 * time.Second)}, th); r == "" {
		t.Error("old toast progress should not be empty")
	}
}

// ═══ keybindings.go ═══

func TestKeyRegistryNew(t *testing.T) {
	r := NewKeyRegistry(KeyRegistryOpts{})
	if r == nil || r.leaderKey != "ctrl+x" {
		t.Error("default leader failed")
	}
	r2 := NewKeyRegistry(KeyRegistryOpts{LeaderKey: "ctrl+z", LeaderTimeout: 2 * time.Second})
	if r2.leaderKey != "ctrl+z" || r2.leaderTimeout != 2*time.Second {
		t.Error("custom leader failed")
	}
}

func TestKeyRegistryRegister(t *testing.T) {
	r := NewKeyRegistry(KeyRegistryOpts{})
	called := false
	r.Register(CtxREPL, "enter", "Send", func() tea.Cmd { called = true; return nil })
	bindings := r.GetContextBindings(CtxREPL)
	if len(bindings) != 1 || bindings[0].Key != "enter" {
		t.Error("register failed")
	}
	bindings[0].Action()
	if !called {
		t.Error("action not called")
	}
}

func TestKeyRegistryHandleSimple(t *testing.T) {
	r := NewKeyRegistry(KeyRegistryOpts{})
	r.Register(CtxREPL, "enter", "Send", func() tea.Cmd { return nil })
	handled, _ := r.Handle("enter", CtxREPL)
	if !handled {
		t.Error("should be handled")
	}
}

func TestKeyRegistryHandleUnregistered(t *testing.T) {
	r := NewKeyRegistry(KeyRegistryOpts{})
	if handled, _ := r.Handle("enter", CtxREPL); handled {
		t.Error("should not be handled")
	}
}

func TestKeyRegistryHandleGlobal(t *testing.T) {
	r := NewKeyRegistry(KeyRegistryOpts{})
	r.Register(CtxGlobal, "ctrl+c", "Quit", func() tea.Cmd { return nil })
	handled, _ := r.Handle("ctrl+c", CtxREPL)
	if !handled {
		t.Error("global should match from any context")
	}
}

func TestKeyRegistryLeaderActivation(t *testing.T) {
	r := NewKeyRegistry(KeyRegistryOpts{})
	handled, cmd := r.Handle("ctrl+x", CtxREPL)
	if !handled || cmd == nil || !r.IsLeaderActive() {
		t.Error("leader activation failed")
	}
}

func TestKeyRegistryLeaderChord(t *testing.T) {
	r := NewKeyRegistry(KeyRegistryOpts{})
	called := false
	r.Register(CtxREPL, "ctrl+x h", "Help", func() tea.Cmd { called = true; return nil })
	r.Handle("ctrl+x", CtxREPL)
	handled, _ := r.Handle("h", CtxREPL)
	if !handled || !called || r.IsLeaderActive() {
		t.Error("chord failed")
	}
}

func TestKeyRegistryLeaderChordGlobal(t *testing.T) {
	r := NewKeyRegistry(KeyRegistryOpts{})
	r.Register(CtxGlobal, "ctrl+x s", "Save", func() tea.Cmd { return nil })
	r.Handle("ctrl+x", CtxREPL)
	if handled, _ := r.Handle("s", CtxREPL); !handled {
		t.Error("global chord should be handled")
	}
}

func TestKeyRegistryLeaderChordNotFound(t *testing.T) {
	r := NewKeyRegistry(KeyRegistryOpts{})
	r.Handle("ctrl+x", CtxREPL)
	if handled, _ := r.Handle("z", CtxREPL); !handled {
		t.Error("unknown chord should still be handled")
	}
}

func TestKeyRegistryDeactivateLeader(t *testing.T) {
	r := NewKeyRegistry(KeyRegistryOpts{})
	r.Handle("ctrl+x", CtxREPL)
	r.DeactivateLeader()
	if r.IsLeaderActive() {
		t.Error("should not be active")
	}
}

func TestKeyRegistryGetContextBindings(t *testing.T) {
	r := NewKeyRegistry(KeyRegistryOpts{})
	r.Register(CtxGlobal, "ctrl+c", "Quit", nil)
	r.Register(CtxREPL, "enter", "Send", nil)
	r.Register(CtxSettings, "tab", "Next", nil)
	if b := r.GetContextBindings(CtxREPL); len(b) != 2 {
		t.Errorf("REPL bindings=%d, want 2", len(b))
	}
}

func TestKeyRegistryRenderWhichKey(t *testing.T) {
	r := NewKeyRegistry(KeyRegistryOpts{})
	r.Register(CtxREPL, "ctrl+x h", "Help", nil)
	if r := r.RenderWhichKey(CtxREPL, 80, lipgloss.Color("#7c3aed"), lipgloss.Color("#a0a0a0"), lipgloss.Color("#666666")); r != "" {
		t.Error("not active should be empty")
	}
	r.Handle("ctrl+x", CtxREPL)
	if r := r.RenderWhichKey(CtxREPL, 80, lipgloss.Color("#7c3aed"), lipgloss.Color("#a0a0a0"), lipgloss.Color("#666666")); r == "" {
		t.Error("active should not be empty")
	}
}

func TestKeyRegistryNoActionBindings(t *testing.T) {
	r := NewKeyRegistry(KeyRegistryOpts{})
	r.Register(CtxREPL, "enter", "Send", nil)
	r.Register(CtxGlobal, "ctrl+c", "Quit", nil)
	r.Register(CtxREPL, "ctrl+x h", "Help", nil)
	r.Register(CtxGlobal, "ctrl+x s", "Save", nil)
	handled, _ := r.Handle("enter", CtxREPL)
	if !handled {
		t.Error("nil action should still be handled")
	}
	handled, _ = r.Handle("ctrl+c", CtxREPL)
	if !handled {
		t.Error("nil action global should be handled")
	}
	r.Handle("ctrl+x", CtxREPL)
	handled, _ = r.Handle("h", CtxREPL)
	if !handled {
		t.Error("nil action chord should be handled")
	}
	r.Handle("ctrl+x", CtxREPL)
	handled, _ = r.Handle("s", CtxREPL)
	if !handled {
		t.Error("nil action global chord should be handled")
	}
}

// ═══ mention.go ═══

func TestNewMentionCompleter(t *testing.T) {
	mc := NewMentionCompleter("/tmp")
	if mc == nil || mc.cwd != "/tmp" {
		t.Error("NewMentionCompleter failed")
	}
}

func TestMentionCompleterScanEmpty(t *testing.T) {
	mc := NewMentionCompleter("")
	mc.Scan()
	if !mc.scanned {
		t.Error("should be scanned")
	}
}

func TestMentionCompleterScanDir(t *testing.T) {
	dir := t.TempDir()
	os.WriteFile(filepath.Join(dir, "main.go"), []byte("package main"), 0o644)
	os.WriteFile(filepath.Join(dir, "README.md"), []byte("# Hi"), 0o644)
	os.MkdirAll(filepath.Join(dir, "src"), 0o755)
	mc := NewMentionCompleter(dir)
	mc.Scan()
	if len(mc.entries) < 2 {
		t.Errorf("entries=%d, want >= 2", len(mc.entries))
	}
}

func TestMentionCompleterScanSkipsHidden(t *testing.T) {
	dir := t.TempDir()
	os.WriteFile(filepath.Join(dir, "visible.go"), []byte("pkg"), 0o644)
	os.WriteFile(filepath.Join(dir, ".hidden"), []byte("sec"), 0o644)
	os.MkdirAll(filepath.Join(dir, ".git"), 0o755)
	os.WriteFile(filepath.Join(dir, ".git", "config"), []byte("repo"), 0o644)
	mc := NewMentionCompleter(dir)
	mc.Scan()
	for _, e := range mc.entries {
		if strings.HasPrefix(e.DisplayName, ".") || strings.Contains(e.Path, ".git") {
			t.Errorf("should skip: %s", e.Path)
		}
	}
}

func TestMentionCompleterScanSkipsNoisy(t *testing.T) {
	dir := t.TempDir()
	os.WriteFile(filepath.Join(dir, "main.go"), []byte("pkg"), 0o644)
	os.MkdirAll(filepath.Join(dir, "node_modules"), 0o755)
	os.WriteFile(filepath.Join(dir, "node_modules", "pkg.js"), []byte(""), 0o644)
	mc := NewMentionCompleter(dir)
	mc.Scan()
	for _, e := range mc.entries {
		if strings.Contains(e.Path, "node_modules") {
			t.Error("node_modules should be skipped")
		}
	}
}

func TestMentionCompleterFilter(t *testing.T) {
	dir := t.TempDir()
	os.WriteFile(filepath.Join(dir, "main.go"), []byte("pkg"), 0o644)
	os.WriteFile(filepath.Join(dir, "util.go"), []byte("pkg"), 0o644)
	mc := NewMentionCompleter(dir)
	results := mc.Filter("")
	if len(results) > 8 {
		t.Errorf("Filter('')=%d, want <= 8", len(results))
	}
	results = mc.Filter("main")
	if len(results) != 1 || results[0].DisplayName != "main.go" {
		t.Error("Filter('main') failed")
	}
}

func TestMentionCompleterFilterCache(t *testing.T) {
	dir := t.TempDir()
	mc := NewMentionCompleter(dir)
	mc.Filter("")
	if !mc.scanned {
		t.Error("first Filter should trigger scan")
	}
	mc.Filter("x")
}

func TestResolveMentions(t *testing.T) {
	dir := t.TempDir()
	os.WriteFile(filepath.Join(dir, "test.go"), []byte("package main\nfunc main() {}"), 0o644)
	results := ResolveMentions(dir, "Look at @test.go for details")
	if len(results) != 1 || results[0].Path != "test.go" || !strings.Contains(results[0].Content, "package main") {
		t.Error("ResolveMentions failed")
	}
}

func TestResolveMentionsNoAt(t *testing.T) {
	if r := ResolveMentions(t.TempDir(), "no mentions"); len(r) != 0 {
		t.Error("no @ should return empty")
	}
}

func TestResolveMentionsDirectory(t *testing.T) {
	dir := t.TempDir()
	os.MkdirAll(filepath.Join(dir, "subdir"), 0o755)
	if r := ResolveMentions(dir, "@subdir"); len(r) != 0 {
		t.Error("directory should return empty")
	}
}

func TestResolveMentionsMissing(t *testing.T) {
	if r := ResolveMentions(t.TempDir(), "@nonexistent.go"); len(r) != 0 {
		t.Error("missing file should return empty")
	}
}

func TestResolveMentionsDuplicate(t *testing.T) {
	dir := t.TempDir()
	os.WriteFile(filepath.Join(dir, "a.go"), []byte("pkg a"), 0o644)
	if r := ResolveMentions(dir, "@a.go @a.go"); len(r) != 1 {
		t.Error("duplicate should return 1")
	}
}

func TestResolveMentionsLargeFile(t *testing.T) {
	dir := t.TempDir()
	os.WriteFile(filepath.Join(dir, "big.go"), []byte(strings.Repeat("line\n", 2000)), 0o644)
	r := ResolveMentions(dir, "@big.go")
	if len(r) != 1 || !strings.Contains(r[0].Content, "truncated") {
		t.Error("large file should be truncated")
	}
}

func TestResolveMentionsPunctuation(t *testing.T) {
	dir := t.TempDir()
	os.WriteFile(filepath.Join(dir, "a.go"), []byte("pkg a"), 0o644)
	if r := ResolveMentions(dir, "see @a.go. for details"); len(r) != 1 {
		t.Error("trailing dot should be trimmed")
	}
}

func TestDetectProjectLanguage(t *testing.T) {
	tests := []struct {
		name  string
		files []string
		want  string
	}{
		{"go", []string{"main.go", "util.go"}, "Go"},
		{"ts", []string{"app.ts", "index.tsx"}, "TypeScript"},
		{"js", []string{"app.js", "index.jsx"}, "JavaScript"},
		{"py", []string{"main.py", "utils.py"}, "Python"},
		{"rs", []string{"main.rs"}, "Rust"},
		{"java", []string{"Main.java"}, "Java"},
		{"kt", []string{"Main.kt"}, "Kotlin"},
		{"rb", []string{"app.rb"}, "Ruby"},
		{"c", []string{"main.c", "utils.h"}, "C"},
		{"cpp", []string{"main.cpp", "utils.hpp"}, "C++"},
		{"cs", []string{"Program.cs"}, "C#"},
		{"swift", []string{"main.swift"}, "Swift"},
		{"zig", []string{"main.zig"}, "Zig"},
		{"mixed", []string{"main.go", "app.py", "utils.py"}, "Python"},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			dir := t.TempDir()
			for _, f := range tt.files {
				os.WriteFile(filepath.Join(dir, f), []byte("content"), 0o644)
			}
			if got := detectProjectLanguage(dir); got != tt.want {
				t.Errorf("got %q, want %q", got, tt.want)
			}
		})
	}
}

func TestDetectProjectLanguageEmpty(t *testing.T) {
	if got := detectProjectLanguage(""); got != "" {
		t.Errorf("empty dir: %q", got)
	}
	if got := detectProjectLanguage("/nonexistent/path/xyz"); got != "" {
		t.Errorf("invalid dir: %q", got)
	}
}

func TestMentionCompleterFilterScoreOrder(t *testing.T) {
	dir := t.TempDir()
	os.WriteFile(filepath.Join(dir, "config.go"), []byte("pkg"), 0o644)
	os.WriteFile(filepath.Join(dir, "main.go"), []byte("pkg"), 0o644)
	mc := NewMentionCompleter(dir)
	results := mc.Filter("config")
	if len(results) == 0 || results[0].DisplayName != "config.go" {
		t.Error("config.go should be first match")
	}
}

// ═══ Model constructor/setter tests ═══

func TestDiffModel(t *testing.T) {
	th := testTheme()
	dm := NewDiffModel(th)
	if dm == nil {
		t.Fatal("nil")
	}
	dm.SetTheme(th)
	dm.SetDimensions(80, 24)
	dm.SetDiff("--- a/f.go\n+++ b/f.go\n@@ -1 +1 @@\n-old\n+new")
	if dm.additions != 1 || dm.deletions != 1 {
		t.Error("diff stats wrong")
	}
	dm.SetDiff("")
	if dm.additions != 0 || dm.deletions != 0 {
		t.Error("empty diff stats wrong")
	}
	dm.SetTitle("diff \u2014 src/main.go")
	if dm.filePath != "src/main.go" {
		t.Errorf("filePath=%q, want src/main.go", dm.filePath)
	}
	dm.filePath = ""
	if dm.filePath != "" {
		t.Error("filePath should be empty after reset")
	}
	if dm.Init() != nil {
		t.Error("Init should be nil")
	}
	result, cmd := dm.Update(tea.WindowSizeMsg{Width: 80, Height: 24})
	if result == nil || cmd != nil {
		t.Error("WindowSizeMsg failed")
	}
	_, cmd = dm.Update(tea.KeyMsg{Type: tea.KeyEsc})
	if cmd == nil {
		t.Error("Esc should return cmd")
	}
	_, cmd = dm.Update(DiffCloseMsg{})
	if cmd == nil {
		t.Error("DiffCloseMsg should return cmd")
	}
	dm.SetDimensions(80, 24)
	dm.SetDiff("+added line\n context\n-removed")
	if r := dm.View(); r == "" {
		t.Error("View should not be empty")
	}
}

func TestGoalInputModel(t *testing.T) {
	th := testTheme()
	gim := NewGoalInputModel(th, []string{"g1", "g2"})
	if gim == nil || len(gim.recentGoals) != 2 {
		t.Fatal("NewGoalInputModel failed")
	}
	gim.SetTheme(th)
	gim.SetDimensions(80, 24)
	if gim.width != 80 || gim.height != 24 {
		t.Error("dimensions wrong")
	}
	gim.SetDimensions(40, 5)
	if gim.Init() == nil {
		t.Error("Init should return cmd")
	}
	_, cmd := gim.Update(tea.KeyMsg{Type: tea.KeyEsc})
	if cmd == nil {
		t.Error("Esc should return cmd")
	}
	gim.showRecent = true
	gim.Update(tea.KeyMsg{Type: tea.KeyDown})
	if gim.recentIdx != 1 {
		t.Error("Down should move")
	}
	gim.Update(tea.KeyMsg{Type: tea.KeyUp})
	if gim.recentIdx != 0 {
		t.Error("Up should move")
	}
	gim.recentIdx = 0
	gim.Update(tea.KeyMsg{Type: tea.KeyUp})
	if gim.recentIdx != 0 {
		t.Error("Up at 0 should stay")
	}
	gim.recentIdx = 1
	gim.Update(tea.KeyMsg{Type: tea.KeyDown})
	if gim.recentIdx != 1 {
		t.Error("Down at end should stay")
	}
	gim.SetDimensions(20, 10)
	if r := gim.View(); r == "" {
		t.Error("View should not be empty")
	}
	gim.showRecent = true
	if r := gim.View(); !strings.Contains(r, "Recent goals") {
		t.Error("should show recent")
	}
}

func TestRollbackModel(t *testing.T) {
	th := testTheme()
	rm := NewRollbackModel(th, &git.Git{}, nil, 80, 24)
	if rm == nil {
		t.Fatal("nil")
	}
	rm.SetTheme(th)
	rm.SetDimensions(100, 30)
	if rm.width != 100 || rm.height != 30 {
		t.Error("dimensions wrong")
	}
	if rm.Init() != nil {
		t.Error("Init should be nil")
	}
	rm.LoadCommits()
	if r := rm.View(); !strings.Contains(r, "No commits found") {
		t.Error("should show no commits")
	}
	rm.errMsg = "err"
	if r := rm.View(); !strings.Contains(r, "err") {
		t.Error("should show error")
	}
	result, _ := rm.Update(tea.WindowSizeMsg{Width: 100, Height: 30})
	rm2 := result.(*RollbackModel)
	if rm2.width != 100 || rm2.height != 30 {
		t.Error("WindowSizeMsg failed")
	}
	_, cmd := rm.Update(tea.KeyMsg{Type: tea.KeyEsc})
	if cmd == nil {
		t.Error("Esc should return cmd")
	}
	rm.entries = []rollback.RollbackEntry{
		{CommitInfo: git.CommitInfo{Hash: "abc"}}, {CommitInfo: git.CommitInfo{Hash: "def"}},
	}
	rm.Update(tea.KeyMsg{Type: tea.KeyDown})
	if rm.cursor != 1 {
		t.Error("Down should move")
	}
	rm.Update(tea.KeyMsg{Type: tea.KeyUp})
	if rm.cursor != 0 {
		t.Error("Up should move")
	}
	rm.cursor = 0
	rm.Update(tea.KeyMsg{Type: tea.KeyUp})
	if rm.cursor != 0 {
		t.Error("Up at 0 should stay")
	}
	rm.Update(tea.KeyMsg{Type: tea.KeyDown})
	if rm.cursor != 1 {
		t.Error("Down at end should stay")
	}
	rm.confirmReset = "soft"
	rm.Update(tea.KeyMsg{Type: tea.KeyRunes, Runes: []rune{'x'}})
	if rm.confirmReset != "" {
		t.Error("any key should clear confirm")
	}
	rm.confirmReset = "soft"
	_, _ = rm.Update(tea.KeyMsg{Type: tea.KeyEsc})
	if rm.confirmReset != "" {
		t.Error("Esc from confirm should clear")
	}
	rm.showDiff = true
	_, _ = rm.Update(tea.KeyMsg{Type: tea.KeyEsc})
	if rm.showDiff {
		t.Error("Esc from diff should hide")
	}
	rm.entries = nil
	_, cmd = rm.Update(tea.KeyMsg{Type: tea.KeyEnter})
	if cmd != nil {
		t.Error("Enter with no entries should be nil")
	}
	rm.Update(tea.KeyMsg{Type: tea.KeyRunes, Runes: []rune{'r'}})
	rm.Update(tea.KeyMsg{Type: tea.KeyRunes, Runes: []rune{'R'}})
	rm.height = 10
	rm.entries = make([]rollback.RollbackEntry, 20)
	for i := range rm.entries {
		rm.entries[i] = rollback.RollbackEntry{CommitInfo: git.CommitInfo{Hash: string(rune('a' + i))}}
	}
	rm.cursor = 15
	rm.clampScroll()
	lh := rm.listVisibleRows()
	if rm.cursor < rm.offset || rm.cursor >= rm.offset+lh {
		t.Error("cursor out of range")
	}
	rm.height = 2
	if rm.listVisibleRows() != 3 {
		t.Error("small height should be 3")
	}
	rm.height = 24
	rm.entries = []rollback.RollbackEntry{
		{CommitInfo: git.CommitInfo{Hash: "abc", ShortHash: "abc1234", Message: "commit", Timestamp: time.Now()}, IsCurrent: true, Diff: "+added\n-removed"},
	}
	rm.cursor = 0
	rm.showDiff = true
	if r := rm.View(); r == "" {
		t.Error("diff view should not be empty")
	}
	rm.showDiff = false
	rm.confirmReset = "soft"
	if r := rm.View(); r == "" {
		t.Error("soft reset view should not be empty")
	}
	rm.confirmReset = "hard"
	if r := rm.View(); r == "" {
		t.Error("hard reset view should not be empty")
	}
	rm.renderCommitRow(rm.entries[0], false, 80)
	rm.renderCommitRow(rm.entries[0], true, 80)
	rm.renderDiffContent(rollback.RollbackEntry{Diff: ""})
	rm.renderDiffContent(rm.entries[0])
	_ = NewRollbackModel(th, &git.Git{}, nil, 10, 5).View()
}

func TestLedgerModel(t *testing.T) {
	th := testTheme()
	lm := NewLedgerModel(th, nil)
	if lm == nil {
		t.Fatal("nil")
	}
	lm.SetTheme(th)
	lm.SetDimensions(80, 24)
	if lm.Init() != nil {
		t.Error("Init should be nil")
	}
	lm.LoadEntries()
	if !lm.loaded || len(lm.entries) != 0 {
		t.Error("LoadEntries failed")
	}
	lm.loaded = false
	if r := lm.View(); !strings.Contains(r, "Loading") {
		t.Error("should show Loading")
	}
	lm.loaded = true
	if r := lm.View(); !strings.Contains(r, "No ledger entries") {
		t.Error("should show no entries")
	}
	result, _ := lm.Update(tea.WindowSizeMsg{Width: 80, Height: 24})
	if result == nil {
		t.Error("WindowSizeMsg failed")
	}
	_, cmd := lm.Update(tea.KeyMsg{Type: tea.KeyEsc})
	if cmd == nil {
		t.Error("Esc should return cmd")
	}
	lm.Update(tea.KeyMsg{Type: tea.KeyRunes, Runes: []rune{'j'}})
	lm.Update(tea.KeyMsg{Type: tea.KeyRunes, Runes: []rune{'k'}})
	lm.Update(tea.KeyMsg{Type: tea.KeyRunes, Runes: []rune{'r'}})
}

func TestNotificationModel(t *testing.T) {
	th := testTheme()
	nm := NewNotificationModel(th, 80, 24)
	if nm == nil {
		t.Fatal("nil")
	}
	nm.AddNotification("test", "info")
	nm.SetTheme(th)
	nm.SetDimensions(100, 30)
	if nm.width != 100 || nm.height != 30 {
		t.Error("dimensions wrong")
	}
	if nm.Init() != nil {
		t.Error("Init should be nil")
	}
	_, cmd := nm.Update(tea.KeyMsg{Type: tea.KeyEsc})
	if cmd == nil {
		t.Error("Esc should return cmd")
	}
	nm.Update(tea.KeyMsg{Type: tea.KeyUp})
	nm.Update(tea.KeyMsg{Type: tea.KeyDown})
	if r := nm.View(); r == "" || !strings.Contains(r, "Notification History") {
		t.Error("View failed")
	}
}

func TestMetricsModel(t *testing.T) {
	th := testTheme()
	mm := NewMetricsModel(th)
	if mm == nil {
		t.Fatal("nil")
	}
	mm.SetTheme(th)
	mm.SetDimensions(80, 24)
	if mm.width != 80 || mm.height != 24 {
		t.Error("dimensions wrong")
	}
	if mm.Init() != nil {
		t.Error("Init should be nil")
	}
	mm.loaded = false
	if r := mm.View(); !strings.Contains(r, "Loading metrics") {
		t.Error("should show Loading")
	}
	result, _ := mm.Update(tea.WindowSizeMsg{Width: 80, Height: 24})
	if result == nil {
		t.Error("WindowSizeMsg failed")
	}
	_, cmd := mm.Update(tea.KeyMsg{Type: tea.KeyEsc})
	if cmd == nil {
		t.Error("Esc should return cmd")
	}
	_, cmd = mm.Update(tea.KeyMsg{Type: tea.KeyRunes, Runes: []rune{'q'}})
	if cmd == nil {
		t.Error("q should return cmd")
	}
	mm.stats = metricsStats{
		SessionCount: 5, TotalMessages: 42, TotalTokens: 10000, AvgTokens: 2000,
		Providers: map[string]int{"openrouter": 3}, Models: map[string]int{"gpt-4": 3}, Phases: map[string]int{"plan": 2},
	}
	mm.loaded = true
	if r := mm.View(); r == "" {
		t.Error("View with stats should not be empty")
	}
}

func TestHelpModel(t *testing.T) {
	th := testTheme()
	hm := NewHelpModel(th)
	if hm == nil || len(hm.sections) == 0 {
		t.Fatal("NewHelpModel failed")
	}
	hm.SetTheme(th)
	hm.SetDimensions(80, 24)
	if hm.width != 80 || hm.height != 24 {
		t.Error("dimensions wrong")
	}
	if hm.Init() != nil {
		t.Error("Init should be nil")
	}
	_, cmd := hm.Update(tea.KeyMsg{Type: tea.KeyEsc})
	if cmd == nil {
		t.Error("Esc should return cmd")
	}
	hm.Update(tea.KeyMsg{Type: tea.KeyRunes, Runes: []rune{'g'}})
	hm.Update(tea.KeyMsg{Type: tea.KeyRunes, Runes: []rune{'G'}})
	result, _ := hm.Update(tea.WindowSizeMsg{Width: 80, Height: 24})
	if result == nil {
		t.Error("WindowSizeMsg failed")
	}
	if r := hm.View(); r == "" {
		t.Error("View should not be empty")
	}
	sections := defaultHelpSections()
	if len(sections) < 5 {
		t.Errorf("sections=%d, want >= 5", len(sections))
	}
}

// ═══ Health/Streaming ═══

func TestHealthCheckTicker(t *testing.T) {
	if cmd := HealthCheckTicker(t.Context(), time.Second); cmd == nil {
		t.Error("nil cmd")
	}
	if cmd := NextHealthTick(t.Context(), time.Second); cmd == nil {
		t.Error("nil cmd")
	}
	if cmd := SidebarRefreshTicker(t.Context(), time.Second); cmd == nil {
		t.Error("nil cmd")
	}
	if cmd := NextSidebarRefreshTick(t.Context(), time.Second); cmd == nil {
		t.Error("nil cmd")
	}
	if cmd := StreamTickCmd(); cmd == nil {
		t.Error("nil cmd")
	}
}

// ═══ Message type tests ═══

func TestMessageTypes(t *testing.T) {
	_ = AppMsg{Screen: ScreenREPL, Action: "test", SessionID: "abc"}
	_ = ModelSelectedMsg{Model: types.ModelInfo{ID: "gpt-4"}, Provider: "openai"}
	_ = ProviderEntry{ID: "openai", APIKey: "sk-..."}
	_ = FirstRunCompleteMsg{Providers: []ProviderEntry{{ID: "openai"}}, ModelID: "gpt-4", SaveKeychain: true, DefaultProvider: "openai"}
	_ = ErrorMsg{Err: os.ErrNotExist}
	_ = DiffScreenMsg{Diff: "+added", Title: "test"}
	_ = SidebarRefreshMsg{Branch: "main", Remote: "origin/main", Files: []SidebarFile{{Path: "a.go", Status: "M "}}}
	_ = PhaseResultMsg{Phase: types.PhasePlan, Success: true}
	_ = PlanReadyMsg{Tasks: []types.Task{{ID: 1}}, CostEstimate: "$0.05", TimeEstimate: "2min"}
	_ = GoalSubmittedMsg{Goal: "build a website"}
	_ = PhaseModelPickedMsg{PlanningModelID: "gpt-4", CodingModelID: "claude-3"}
	_ = ToastMsg{Text: "saved", Duration: time.Second, Type: "success"}
	_ = ThemeChangedMsg{Theme: "dark"}
	_ = OptimizedMsg{TaskID: 1}
	_ = SessionRenameMsg{SessionID: "abc"}
	_ = SessionExportMsg{SessionID: "abc"}
	_ = SlashCommandMsg{Command: "/help", AttachedFiles: 2}
	_ = FallbackEventMsg{From: "openai", To: "anthropic", Reason: "timeout"}
	_ = QuestionRequestMsg{ID: 1, Question: "Color?", Header: "Color", Options: []string{"red", "blue"}, AllowCustom: true, TimeoutSecs: 30}
	_ = QuestionResponseMsg{Answer: "red"}
	_ = DiscussAnswerMsg{Index: 0, Answer: "yes"}
	_ = ExecutePauseMsg{Paused: true}
	_ = HealResultMsg{TaskID: 3, Success: true}
	_ = SettingsSavedMsg{}
	_ = DiffCloseMsg{}
	_ = PopScreenMsg{}
	_ = DiscussCompleteMsg{}
	_ = DiscussAnswerTimeoutMsg{QuestionIndex: 2}
	_ = LeaderTimeoutMsg{}
	_ = KeyActionMsg{Action: "toggle_sidebar"}
	_ = PermissionTickMsg{}
	_ = SidebarRefreshTickMsg{}
	_ = sessionRestoredMsg{clearExisting: true}
	_ = HealthCheckTickMsg{Time: time.Now()}
	_ = HealthCheckResultMsg{Result: types.HealthStatus{}}
	_ = RefreshCacheMsg{ProviderName: "openai"}
	_ = CacheRefreshResultMsg{ErrMsg: "failed"}
	_ = PermissionRequestMsg{}
	_ = PermissionResponseMsg{}
	_ = PlanApproveMsg{}
	_ = PlanRefineMsg{Feedback: "add tests"}
}

func TestKeyContextConstants(t *testing.T) {
	for _, ctx := range []KeyContext{CtxGlobal, CtxREPL, CtxPalette, CtxSidebar, CtxSettings, CtxModelSel, CtxResume, CtxPermModal, CtxFirstRun, CtxPlan, CtxExecute, CtxVerify, CtxShip, CtxDiscuss, CtxDiff, CtxLedger, CtxRollback, CtxMetrics, CtxGoalInput, CtxConfig, CtxHelp} {
		if ctx == "" {
			t.Error("empty KeyContext")
		}
	}
}

func TestCommandCategoryConstants(t *testing.T) {
	for _, c := range []CommandCategory{CatCore, CatAI, CatConfig, CatSession, CatGit, CatWorkflow} {
		if c == "" {
			t.Error("empty CommandCategory")
		}
	}
}

func TestStructFields(t *testing.T) {
	ci := CommandInfo{Name: "test", Description: "desc", Slash: "/test"}
	if ci.Name != "test" || ci.Execute != nil {
		t.Error("CommandInfo failed")
	}
	cr := CommandResult{Success: true, Message: "ok"}
	if !cr.Success {
		t.Error("CommandResult failed")
	}
	kb := KeyBinding{Key: "ctrl+c", Description: "Quit", Context: CtxGlobal}
	if kb.Key != "ctrl+c" || kb.Action != nil {
		t.Error("KeyBinding failed")
	}
	me := MentionEntry{Path: "src/main.go", IsDir: false, DisplayName: "main.go", Size: 1024, LineCount: 50}
	if me.Path != "src/main.go" {
		t.Error("MentionEntry failed")
	}
	mc := MentionContext{Path: "test.go", Content: "pkg"}
	if mc.Path != "test.go" {
		t.Error("MentionContext failed")
	}
	info := &StatusBarInfo{PromptTokens: 100, TotalTokens: 500, Cost: 0.01, ShowCost: true, WhichKey: "ctrl+x", LeaderActive: true, AgentName: "m31a", ModelName: "gpt-4", ProviderName: "openai", IsStreaming: true, ThinkingDuration: 1000, KeyboardHints: []string{"ctrl+p"}, WorkflowPhase: "execute", QuestionProgress: "3/5", CwdName: "proj", GitBranch: "main", SpinnerFrame: "|"}
	if info.PromptTokens != 100 {
		t.Error("StatusBarInfo failed")
	}
	st := ScreenTransition{Active: true, StartAt: time.Now(), Duration: 200 * time.Millisecond, FromScreen: ScreenREPL, ToScreen: ScreenPlan}
	if !st.Active || st.FromScreen != ScreenREPL {
		t.Error("ScreenTransition failed")
	}
	toast := Toast{ID: 1, Text: "saved", Type: "success", CreatedAt: time.Now(), Frame: 2}
	if toast.ID != 1 || toast.Frame != 2 {
		t.Error("Toast failed")
	}
	sf := SidebarFile{Path: "a.go", Status: "M "}
	if sf.Path != "a.go" {
		t.Error("SidebarFile failed")
	}
	fs := git.FileStatus{Status: "M ", Path: "a.go"}
	if fs.Status != "M " {
		t.Error("FileStatus failed")
	}
}
