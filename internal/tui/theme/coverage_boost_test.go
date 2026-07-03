package theme

import (
	"testing"

	"github.com/charmbracelet/lipgloss"
)

// --- NewStyleCache ---

func TestNewStyleCache_NonNil(t *testing.T) {
	t.Parallel()
	th := M31A()
	cache := NewStyleCache(th)
	if cache == nil {
		t.Fatal("NewStyleCache should return non-nil")
	}
}

func TestNewStyleCache_ThemeStored(t *testing.T) {
	t.Parallel()
	th := M31A()
	cache := NewStyleCache(th)
	if cache.Theme.Background != th.Background {
		t.Error("cache.Theme should match input theme")
	}
}

func TestNewStyleCache_TextStyles(t *testing.T) {
	t.Parallel()
	th := M31A()
	cache := NewStyleCache(th)
	// Verify text styles are non-empty by rendering
	if cache.Brand.Render("test") == "" {
		t.Error("Brand style should render non-empty")
	}
	if cache.TextPrimary.Render("test") == "" {
		t.Error("TextPrimary style should render non-empty")
	}
}

func TestNewStyleCache_SemanticStyles(t *testing.T) {
	t.Parallel()
	th := M31A()
	cache := NewStyleCache(th)
	if cache.Success.Render("ok") == "" {
		t.Error("Success style should render non-empty")
	}
	if cache.Error.Render("err") == "" {
		t.Error("Error style should render non-empty")
	}
	if cache.Warning.Render("warn") == "" {
		t.Error("Warning style should render non-empty")
	}
}

func TestNewStyleCache_BadgeStyles(t *testing.T) {
	t.Parallel()
	th := M31A()
	cache := NewStyleCache(th)
	if cache.BadgeSuccess.Render("ok") == "" {
		t.Error("BadgeSuccess should render non-empty")
	}
	if cache.BadgeError.Render("err") == "" {
		t.Error("BadgeError should render non-empty")
	}
}

func TestNewStyleCache_SemanticComponentLibrary(t *testing.T) {
	t.Parallel()
	th := M31A()
	cache := NewStyleCache(th)
	// Verify the S (SemanticStyles) is populated
	if cache.S.PageTitle.Render("test") == "" {
		t.Error("S.PageTitle should render non-empty")
	}
	if cache.S.ButtonPrimary.Render("btn") == "" {
		t.Error("S.ButtonPrimary should render non-empty")
	}
}

func TestNewStyleCache_CardStyles(t *testing.T) {
	t.Parallel()
	th := M31A()
	cache := NewStyleCache(th)
	if cache.CardBorder.Render("card") == "" {
		t.Error("CardBorder should render non-empty")
	}
}

func TestNewStyleCache_CodeStyles(t *testing.T) {
	t.Parallel()
	th := M31A()
	cache := NewStyleCache(th)
	if cache.CodeBG.Render("code") == "" {
		t.Error("CodeBG should render non-empty")
	}
}

func TestNewStyleCache_ButtonStyles(t *testing.T) {
	t.Parallel()
	th := M31A()
	cache := NewStyleCache(th)
	if cache.ButtonPrimary.Render("btn") == "" {
		t.Error("ButtonPrimary should render non-empty")
	}
}

// --- Manager methods ---

func TestManager_SetBorderStyle(t *testing.T) {
	t.Parallel()
	m := NewManager(ModeDark)
	m.SetBorderStyle("thin")
	border := m.CurrentBorder()
	if border.Top != "─" {
		t.Errorf("SetBorderStyle('thin') should produce thin border, got Top = %q", border.Top)
	}
}

func TestManager_SetBorderStyle_Unknown(t *testing.T) {
	t.Parallel()
	m := NewManager(ModeDark)
	m.SetBorderStyle("unknown")
	// Should not panic, defaults to rounded
	_ = m.CurrentBorder()
}

func TestManager_SetAccentColor(t *testing.T) {
	t.Parallel()
	m := NewManager(ModeDark)
	m.SetAccentColor("#FF5500")
	th := m.Current()
	if string(th.Brand) != "#FF5500" {
		t.Errorf("SetAccentColor should change Brand, got %q", string(th.Brand))
	}
}

func TestManager_SetAccentColor_Empty(t *testing.T) {
	t.Parallel()
	m := NewManager(ModeDark)
	original := string(m.Current().Brand)
	m.SetAccentColor("")
	th := m.Current()
	if string(th.Brand) != original {
		t.Error("Empty accent color should not change theme")
	}
}

func TestManager_CurrentBorder_AfterSet(t *testing.T) {
	t.Parallel()
	m := NewManager(ModeDark)
	m.SetBorderStyle("double")
	border := m.CurrentBorder()
	if border.Top != "═" {
		t.Errorf("CurrentBorder after SetBorderStyle('double') Top = %q, want ═", border.Top)
	}
}

func TestManager_Cycle_AlwaysDark(t *testing.T) {
	t.Parallel()
	m := NewManager(ModeDark)
	mode := m.Cycle()
	if mode != ModeDark {
		t.Errorf("Cycle should always return ModeDark, got %v", mode)
	}
}

func TestManager_Current_AfterCycle(t *testing.T) {
	t.Parallel()
	m := NewManager(ModeDark)
	m.Cycle()
	th := m.Current()
	if th.Background == "" {
		t.Error("Current after Cycle should have non-empty Background")
	}
}

// --- BuildSemanticStyles ---

func TestBuildSemanticStyles_Typography(t *testing.T) {
	t.Parallel()
	th := M31A()
	s := BuildSemanticStyles(th)
	if s.PageTitle.Render("test") == "" {
		t.Error("PageTitle should render non-empty")
	}
	if s.Heading.Render("test") == "" {
		t.Error("Heading should render non-empty")
	}
	if s.Body.Render("test") == "" {
		t.Error("Body should render non-empty")
	}
}

func TestBuildSemanticStyles_Buttons(t *testing.T) {
	t.Parallel()
	th := M31A()
	s := BuildSemanticStyles(th)
	if s.ButtonPrimary.Render("btn") == "" {
		t.Error("ButtonPrimary should render non-empty")
	}
	if s.ButtonSecondary.Render("btn") == "" {
		t.Error("ButtonSecondary should render non-empty")
	}
}

func TestBuildSemanticStyles_Cards(t *testing.T) {
	t.Parallel()
	th := M31A()
	s := BuildSemanticStyles(th)
	if s.Card.Render("card") == "" {
		t.Error("Card should render non-empty")
	}
}

func TestBuildSemanticStyles_Badges(t *testing.T) {
	t.Parallel()
	th := M31A()
	s := BuildSemanticStyles(th)
	if s.BadgeSuccess.Render("ok") == "" {
		t.Error("BadgeSuccess should render non-empty")
	}
}

func TestBuildSemanticStyles_Inputs(t *testing.T) {
	t.Parallel()
	th := M31A()
	s := BuildSemanticStyles(th)
	if s.Input.Render("input") == "" {
		t.Error("Input should render non-empty")
	}
}

func TestBuildSemanticStyles_Markdown(t *testing.T) {
	t.Parallel()
	th := M31A()
	s := BuildSemanticStyles(th)
	if s.MarkdownH1.Render("test") == "" {
		t.Error("MarkdownH1 should render non-empty")
	}
}

func TestBuildSemanticStyles_Scroll(t *testing.T) {
	t.Parallel()
	th := M31A()
	s := BuildSemanticStyles(th)
	if s.Scrollbar.Render("scroll") == "" {
		t.Error("Scrollbar should render non-empty")
	}
}

func TestBuildSemanticStyles_Overlay(t *testing.T) {
	t.Parallel()
	th := M31A()
	s := BuildSemanticStyles(th)
	if s.DimOverlay.Render("overlay") == "" {
		t.Error("DimOverlay should render non-empty")
	}
}

// --- ColorProfile ---

func TestDetectColorProfile_FromEnv(t *testing.T) {
	t.Parallel()
	// DetectColorProfile reads from env, just verify it doesn't panic
	profile := DetectColorProfile()
	if profile != ProfileTrueColor && profile != Profile256 && profile != Profile16 {
		t.Errorf("unexpected profile: %d", profile)
	}
}

func TestPaletteForProfile_TrueColor(t *testing.T) {
	t.Parallel()
	th := PaletteForProfile(ProfileTrueColor)
	if th.Background == "" {
		t.Error("PaletteForProfile(ProfileTrueColor) should have Background")
	}
}

func TestPaletteForProfile_256(t *testing.T) {
	t.Parallel()
	th := PaletteForProfile(Profile256)
	if th.Background == "" {
		t.Error("PaletteForProfile(Profile256) should have Background")
	}
}

func TestPaletteForProfile_16(t *testing.T) {
	t.Parallel()
	th := PaletteForProfile(Profile16)
	if th.Background == "" {
		t.Error("PaletteForProfile(Profile16) should have Background")
	}
	// ANSI palette should use 16-color values
	if th.Brand == "" {
		t.Error("PaletteForProfile(Profile16) should have Brand")
	}
}

// --- Theme modes ---

func TestMode_Constants(t *testing.T) {
	t.Parallel()
	if ModeDark != 0 {
		t.Errorf("ModeDark = %d, want 0", ModeDark)
	}
	if ModeLight != 1 {
		t.Errorf("ModeLight = %d, want 1", ModeLight)
	}
	if ModeAuto != 2 {
		t.Errorf("ModeAuto = %d, want 2", ModeAuto)
	}
}

// --- Border styles ---

func TestNormalBorder(t *testing.T) {
	t.Parallel()
	if NormalBorder.Top == "" {
		t.Error("NormalBorder.Top should not be empty")
	}
}

func TestThinBorder(t *testing.T) {
	t.Parallel()
	if ThinBorder.Top != "─" {
		t.Errorf("ThinBorder.Top = %q, want ─", ThinBorder.Top)
	}
}

func TestDoubleBorder(t *testing.T) {
	t.Parallel()
	if DoubleBorder.Top == "" {
		t.Error("DoubleBorder.Top should not be empty")
	}
}

func TestSplitBorder(t *testing.T) {
	t.Parallel()
	if SplitBorder.Left == "" {
		t.Error("SplitBorder.Left should not be empty")
	}
}

// --- Theme field validation ---

func TestM31A_AllRequiredFields(t *testing.T) {
	t.Parallel()
	th := M31A()
	fields := map[string]string{
		"Background":    string(th.Background),
		"Surface":       string(th.Surface),
		"Border":        string(th.Border),
		"Brand":         string(th.Brand),
		"TextPrimary":   string(th.TextPrimary),
		"TextSecondary": string(th.TextSecondary),
		"Success":       string(th.Success),
		"Error":         string(th.Error),
		"Warning":       string(th.Warning),
		"Thinking":      string(th.Thinking),
		"CodeBG":        string(th.CodeBG),
		"DividerChar":   th.DividerChar,
		"BlockFull":     th.BlockFull,
		"BlockHigh":     th.BlockHigh,
		"BlockMed":      th.BlockMed,
		"BlockLow":      th.BlockLow,
	}
	for name, val := range fields {
		if val == "" {
			t.Errorf("M31A().%s is empty", name)
		}
	}
}

func TestM31A_ToolLabels(t *testing.T) {
	t.Parallel()
	th := M31A()
	tools := []string{"Bash", "Agent", "FileRead", "FileWrite", "Edit", "Grep", "Glob"}
	for _, tool := range tools {
		if _, ok := th.ToolLabel[tool]; !ok {
			t.Errorf("ToolLabel missing entry for %q", tool)
		}
	}
}

func TestM31A_DiffColors(t *testing.T) {
	t.Parallel()
	th := M31A()
	if th.DiffAdded == "" {
		t.Error("DiffAdded should not be empty")
	}
	if th.DiffRemoved == "" {
		t.Error("DiffRemoved should not be empty")
	}
}

func TestM31A_BadgeColors(t *testing.T) {
	t.Parallel()
	th := M31A()
	if th.BadgeForeground == "" {
		t.Error("BadgeForeground should not be empty")
	}
	if th.BadgeTextLight == "" {
		t.Error("BadgeTextLight should not be empty")
	}
}

// --- WithAccent ---

func TestWithAccent_DifferentValues(t *testing.T) {
	t.Parallel()
	th := M31A()
	tests := []string{"#FF0000", "#00FF00", "#0000FF", "#FFFFFF"}
	for _, hex := range tests {
		accented := th.WithAccent(hex)
		if string(accented.Brand) != hex {
			t.Errorf("WithAccent(%q).Brand = %q", hex, string(accented.Brand))
		}
	}
}

func TestWithAccent_PreservesOtherColors(t *testing.T) {
	t.Parallel()
	th := M31A()
	accented := th.WithAccent("#FF0000")
	if accented.Background != th.Background {
		t.Error("WithAccent should not change Background")
	}
	if accented.Error != th.Error {
		t.Error("WithAccent should not change Error")
	}
}

// --- NormalizeTabs ---

func TestNormalizeTabs_MultipleTabs(t *testing.T) {
	t.Parallel()
	input := "a\tb\tc"
	got := NormalizeTabs(input, 4)
	// Each tab expands to 4 spaces
	expected := "a    b    c"
	if got != expected {
		t.Errorf("NormalizeTabs(%q, 4) = %q, want %q", input, got, expected)
	}
}

func TestNormalizeTabs_NoTabs(t *testing.T) {
	t.Parallel()
	input := "hello world"
	got := NormalizeTabs(input, 4)
	if got != input {
		t.Errorf("NormalizeTabs(%q, 4) = %q, want %q", input, got, input)
	}
}

func TestNormalizeTabs_EmptyString(t *testing.T) {
	t.Parallel()
	got := NormalizeTabs("", 4)
	if got != "" {
		t.Errorf("NormalizeTabs('', 4) = %q, want empty", got)
	}
}

// --- GetFileTypeIcon ---

func TestGetFileTypeIcon_AllKnownTypes(t *testing.T) {
	t.Parallel()
	tests := []struct {
		ext  string
		want string
	}{
		{".go", FileGo},
		{".js", FileJS},
		{".ts", FileJS},
		{".md", FileMD},
		{".json", FileJSON},
		{".yaml", FileYAML},
		{".yml", FileYAML},
		{".py", FileDefault},
		{".rs", FileDefault},
		{".java", FileDefault},
	}
	for _, tt := range tests {
		got := GetFileTypeIcon(tt.ext)
		if got != tt.want {
			t.Errorf("GetFileTypeIcon(%q) = %q, want %q", tt.ext, got, tt.want)
		}
	}
}

// --- GetPhaseIcon ---

func TestGetPhaseIcon_AllPhases(t *testing.T) {
	t.Parallel()
	phases := []string{"initialize", "discuss", "plan", "execute", "verify", "runtime", "ship"}
	for _, phase := range phases {
		got := GetPhaseIcon(phase)
		if got == "" {
			t.Errorf("GetPhaseIcon(%q) should not be empty", phase)
		}
	}
}

// --- GetStatusIcon ---

func TestGetStatusIcon_AllStatuses(t *testing.T) {
	t.Parallel()
	statuses := []string{"done", "pass", "complete", "failed", "error", "warning", "skipped", "pending", "running"}
	for _, status := range statuses {
		got := GetStatusIcon(status)
		if got == "" {
			t.Errorf("GetStatusIcon(%q) should not be empty", status)
		}
	}
}

// --- BorderByName ---

func TestBorderByName_AllNamedBorders(t *testing.T) {
	t.Parallel()
	names := []string{"thin", "double", "heavy", "dashed", "none", "normal", "rounded"}
	for _, name := range names {
		b := BorderByName(name)
		if b.Top == "" && name != "none" {
			t.Errorf("BorderByName(%q).Top should not be empty", name)
		}
	}
}

// --- RenderWithShadow ---

func TestRenderWithShadow_SingleLine(t *testing.T) {
	t.Parallel()
	got := RenderWithShadow("hello", lipgloss.Color("#000000"), 1, 1)
	if got == "" {
		t.Error("RenderWithShadow should return non-empty string")
	}
}

func TestRenderWithShadow_MultiLine(t *testing.T) {
	t.Parallel()
	got := RenderWithShadow("line1\nline2\nline3", lipgloss.Color("#000000"), 2, 2)
	if got == "" {
		t.Error("RenderWithShadow multi-line should return non-empty string")
	}
}

// --- BrandGradientStyle ---

func TestBrandGradientStyle_NonEmpty(t *testing.T) {
	t.Parallel()
	s := BrandGradientStyle()
	if s.Render("test") == "" {
		t.Error("BrandGradientStyle should render non-empty")
	}
}

// --- ThinkingGradientStyle ---

func TestThinkingGradientStyle_NonEmpty(t *testing.T) {
	t.Parallel()
	s := ThinkingGradientStyle()
	if s.Render("test") == "" {
		t.Error("ThinkingGradientStyle should render non-empty")
	}
}

// --- applyThemeStyles ---

func TestApplyThemeStyles_AllStylesSet(t *testing.T) {
	t.Parallel()
	th := M31A()
	// Verify computed styles are set by rendering
	if th.Header.Render("test") == "" {
		t.Error("Header should render non-empty")
	}
	if th.ModelBadge.Render("model") == "" {
		t.Error("ModelBadge should render non-empty")
	}
	if th.UserBubble.Render("user") == "" {
		t.Error("UserBubble should render non-empty")
	}
	if th.AssistantBubble.Render("assistant") == "" {
		t.Error("AssistantBubble should render non-empty")
	}
	if th.InputArea.Render("input") == "" {
		t.Error("InputArea should render non-empty")
	}
}

func TestApplyThemeStyles_CardBorderStyles(t *testing.T) {
	t.Parallel()
	th := M31A()
	if th.CardBorder.Render("card") == "" {
		t.Error("CardBorder should render non-empty")
	}
	if th.CardBorderActive.Render("card") == "" {
		t.Error("CardBorderActive should render non-empty")
	}
	if th.CardBorderError.Render("card") == "" {
		t.Error("CardBorderError should render non-empty")
	}
}

func TestApplyThemeStyles_PhaseStyles(t *testing.T) {
	t.Parallel()
	th := M31A()
	if th.PhaseActive.Render("active") == "" {
		t.Error("PhaseActive should render non-empty")
	}
	if th.PhasePast.Render("past") == "" {
		t.Error("PhasePast should render non-empty")
	}
}
