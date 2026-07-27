package theme

import (
	"strings"
	"testing"

	"github.com/charmbracelet/lipgloss"
)

func TestGetFileTypeIcon(t *testing.T) {
	t.Parallel()
	tests := []struct {
		ext  string
		want string
	}{
		{".go", FileGo},
		{".js", FileJS},
		{".ts", FileJS},
		{".jsx", FileJS},
		{".tsx", FileJS},
		{".md", FileMD},
		{".markdown", FileMD},
		{".json", FileJSON},
		{".yaml", FileYAML},
		{".yml", FileYAML},
		{".txt", FileDefault},
		{".py", FileDefault},
		{"", FileDefault},
		{".unknown", FileDefault},
	}
	for _, tt := range tests {
		got := GetFileTypeIcon(tt.ext)
		if got != tt.want {
			t.Errorf("GetFileTypeIcon(%q) = %q, want %q", tt.ext, got, tt.want)
		}
	}
}

func TestGetPhaseIcon(t *testing.T) {
	t.Parallel()
	tests := []struct {
		phase string
		want  string
	}{
		{"initialize", PhaseInit},
		{"discuss", PhaseDiscuss},
		{"plan", PhasePlan},
		{"execute", PhaseExecute},
		{"verify", PhaseVerify},
		{"runtime", PhaseRuntime},
		{"ship", PhaseShip},
		{"unknown", PhaseInit},
		{"", PhaseInit},
	}
	for _, tt := range tests {
		got := GetPhaseIcon(tt.phase)
		if got != tt.want {
			t.Errorf("GetPhaseIcon(%q) = %q, want %q", tt.phase, got, tt.want)
		}
	}
}

func TestGetStatusIcon(t *testing.T) {
	t.Parallel()
	tests := []struct {
		status string
		want   string
	}{
		{"done", StatusPass},
		{"pass", StatusPass},
		{"complete", StatusPass},
		{"failed", StatusFail},
		{"error", StatusFail},
		{"warning", StatusWarn},
		{"skipped", StatusWarn},
		{"pending", StatusPending},
		{"running", StatusRefresh},
		{"unknown", StatusPending},
		{"", StatusPending},
	}
	for _, tt := range tests {
		got := GetStatusIcon(tt.status)
		if got != tt.want {
			t.Errorf("GetStatusIcon(%q) = %q, want %q", tt.status, got, tt.want)
		}
	}
}

func TestBorderByName(t *testing.T) {
	t.Parallel()
	tests := []struct {
		name string
	}{
		{"thin"},
		{"double"},
		{"heavy"},
		{"dashed"},
		{"none"},
		{"normal"},
		{"unknown"},
		{""},
	}
	for _, tt := range tests {
		b := BorderByName(tt.name)
		_ = b // just verify no panic, all return valid borders
	}
}

func TestBorderByName_SpecificTypes(t *testing.T) {
	t.Parallel()
	thin := BorderByName("thin")
	if thin.Top != "─" {
		t.Errorf("BorderByName('thin').Top = %q, want '─'", thin.Top)
	}
	heavy := BorderByName("heavy")
	if heavy.Top != "━" {
		t.Errorf("BorderByName('heavy').Top = %q, want '━'", heavy.Top)
	}
	dashed := BorderByName("dashed")
	if dashed.Top != "╌" {
		t.Errorf("BorderByName('dashed').Top = %q, want '╌'", dashed.Top)
	}
}

func TestWithAccent(t *testing.T) {
	t.Parallel()
	d := Dark()
	accented := d.WithAccent("#FF0000")
	if string(accented.Brand) != "#FF0000" {
		t.Errorf("WithAccent().Brand = %q, want #FF0000", string(accented.Brand))
	}
	if string(accented.Primary) != "#FF0000" {
		t.Errorf("WithAccent().Primary = %q, want #FF0000", string(accented.Primary))
	}
	if string(accented.BorderActive) != "#FF0000" {
		t.Errorf("WithAccent().BorderActive = %q, want #FF0000", string(accented.BorderActive))
	}
}

func TestWithAccent_DoesNotMutateOriginal(t *testing.T) {
	t.Parallel()
	d := Dark()
	original := string(d.Brand)
	_ = d.WithAccent("#FF0000")
	if string(d.Brand) != original {
		t.Error("WithAccent should not mutate original theme")
	}
}

func TestNormalizeTabs(t *testing.T) {
	t.Parallel()
	tests := []struct {
		s     string
		width int
		want  string
	}{
		{"hello\tworld", 4, "hello    world"},
		{"hello\tworld", 2, "hello  world"},
		{"no tabs", 4, "no tabs"},
		{"", 4, ""},
		{"\t", 0, "    "}, // default width 4
		{"\t\t", 2, "    "},
	}
	for _, tt := range tests {
		got := NormalizeTabs(tt.s, tt.width)
		if got != tt.want {
			t.Errorf("NormalizeTabs(%q, %d) = %q, want %q", tt.s, tt.width, got, tt.want)
		}
	}
}

func TestRenderWithShadow(t *testing.T) {
	t.Parallel()
	got := RenderWithShadow("hello", lipgloss.Color("#000000"), 1, 1)
	if got == "" {
		t.Error("RenderWithShadow should return non-empty string")
	}
	if !strings.Contains(got, "hello") {
		t.Error("RenderWithShadow should contain original content")
	}
}

func TestRenderWithShadow_MultipleLines(t *testing.T) {
	t.Parallel()
	got := RenderWithShadow("line1\nline2", lipgloss.Color("#000000"), 2, 2)
	if got == "" {
		t.Error("RenderWithShadow multi-line should return non-empty string")
	}
}

func TestRenderWithShadow_ZeroOffset(t *testing.T) {
	t.Parallel()
	got := RenderWithShadow("hello", lipgloss.Color("#000000"), 0, 0)
	if got != "hello" {
		t.Errorf("RenderWithShadow zero offset = %q, want 'hello'", got)
	}
}

func TestUnicodeConstants(t *testing.T) {
	t.Parallel()
	// Verify key constants are non-empty
	constants := map[string]string{
		"SectionMarker1": SectionMarker1,
		"FileGo":         FileGo,
		"PhaseInit":      PhaseInit,
		"NavCursor":      NavCursor,
		"StatusPass":     StatusPass,
		"ArrowUp":        ArrowUp,
		"LineDashed1":    LineDashed1,
		"BlockFull":      BlockFull,
		"SpinnerBraille": SpinnerBraille,
		"GradientLight":  GradientLight,
	}
	for name, val := range constants {
		if val == "" {
			t.Errorf("constant %s is empty", name)
		}
	}
}

func TestBorderVariables(t *testing.T) {
	t.Parallel()
	// Verify border variables are accessible and non-empty
	if HeavyBorder.Top == "" {
		t.Error("HeavyBorder.Top is empty")
	}
	if DashedBorder.Top == "" {
		t.Error("DashedBorder.Top is empty")
	}
}

func TestRenderWithShadow_NilShadowColor(t *testing.T) {
	t.Parallel()
	got := RenderWithShadow("test content", lipgloss.Color("#000000"), 1, 1)
	if got == "" {
		t.Error("RenderWithShadow should return non-empty string")
	}
}

func TestRenderWithShadow_WideContent(t *testing.T) {
	t.Parallel()
	got := RenderWithShadow("short", lipgloss.Color("#000000"), 5, 2)
	if got == "" {
		t.Error("RenderWithShadow wide content should return non-empty string")
	}
}

func TestRenderWithShadow_EmptyContent(t *testing.T) {
	t.Parallel()
	got := RenderWithShadow("", lipgloss.Color("#000000"), 1, 1)
	if got == "" {
		t.Error("RenderWithShadow empty content should return non-empty string")
	}
}

func TestDark_AllFieldsNonEmpty(t *testing.T) {
	t.Parallel()
	d := Dark()
	if d.Background == "" {
		t.Error("Dark().Background is empty")
	}
	if d.Surface == "" {
		t.Error("Dark().Surface is empty")
	}
	if d.Text == "" {
		t.Error("Dark().Text is empty")
	}
	if d.TextMuted == "" {
		t.Error("Dark().TextMuted is empty")
	}
	if d.Border == "" {
		t.Error("Dark().Border is empty")
	}
	if d.Brand == "" {
		t.Error("Dark().Brand is empty")
	}
	if d.Success == "" {
		t.Error("Dark().Success is empty")
	}
	if d.Error == "" {
		t.Error("Dark().Error is empty")
	}
	if d.Warning == "" {
		t.Error("Dark().Warning is empty")
	}
	if d.Thinking == "" {
		t.Error("Dark().Thinking is empty")
	}
	if d.DividerChar == "" {
		t.Error("Dark().DividerChar is empty")
	}
}

func TestLight_AllFieldsNonEmpty(t *testing.T) {
	t.Parallel()
	l := Light()
	if l.Background == "" {
		t.Error("Light().Background is empty")
	}
	if l.Brand == "" {
		t.Error("Light().Brand is empty")
	}
	if l.Text == "" {
		t.Error("Light().Text is empty")
	}
}

func TestPaletteForProfile_AllProfiles(t *testing.T) {
	t.Parallel()
	profiles := []ColorProfile{ProfileTrueColor, Profile256, Profile16}
	for _, p := range profiles {
		pal := PaletteForProfile(p)
		if pal.Background == "" {
			t.Errorf("PaletteForProfile(%d).Background is empty", p)
		}
	}
}

func TestManager_AllModes(t *testing.T) {
	t.Parallel()
	modes := []Mode{ModeDark}
	for _, mode := range modes {
		m := NewManager(mode)
		if m.Current().Background == "" {
			t.Errorf("NewManager(%d).Current().Background is empty", mode)
		}
	}
}

func TestRegistry_AllThemesHaveStyles(t *testing.T) {
	t.Parallel()
	for _, def := range Available() {
		th := def.Constructor()
		if th.Background == "" {
			t.Errorf("theme %q has empty Background", def.ID)
		}
		if th.Brand == "" {
			t.Errorf("theme %q has empty Brand", def.ID)
		}
	}
}
