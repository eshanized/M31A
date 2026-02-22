package theme

import "testing"

func TestAvailable_ReturnsThemes(t *testing.T) {
	t.Parallel()
	themes := Available()
	if len(themes) == 0 {
		t.Fatal("Available() returned empty slice")
	}
	if len(themes) < 5 {
		t.Errorf("expected at least 5 themes, got %d", len(themes))
	}
}

func TestByID_Found(t *testing.T) {
	t.Parallel()
	theme, ok := ByID("dark")
	if !ok {
		t.Fatal("ByID('dark') should return true")
	}
	_ = theme
}

func TestByID_NotFound(t *testing.T) {
	t.Parallel()
	_, ok := ByID("nonexistent-theme")
	if ok {
		t.Error("ByID('nonexistent-theme') should return false")
	}
}

func TestByID_AllRegistered(t *testing.T) {
	t.Parallel()
	for _, def := range Available() {
		t.Run(def.ID, func(t *testing.T) {
			_, ok := ByID(def.ID)
			if !ok {
				t.Errorf("ByID(%q) should return true", def.ID)
			}
		})
	}
}

func TestDark_HasRequiredFields(t *testing.T) {
	t.Parallel()
	theme := Dark()
	if theme.Background == "" {
		t.Error("Dark() Background should not be empty")
	}
	if theme.TextPrimary == "" {
		t.Error("Dark() TextPrimary should not be empty")
	}
	if theme.Brand == "" {
		t.Error("Dark() Brand should not be empty")
	}
}

func TestLight_HasRequiredFields(t *testing.T) {
	t.Parallel()
	theme := Light()
	if theme.Background == "" {
		t.Error("Light() Background should not be empty")
	}
	if theme.TextPrimary == "" {
		t.Error("Light() TextPrimary should not be empty")
	}
}

func TestCatppuccin_IsDarkMode(t *testing.T) {
	t.Parallel()
	theme := Catppuccin()
	if theme.Background == "" {
		t.Error("Catppuccin() Background should not be empty")
	}
	if theme.Mode != ModeDark {
		t.Errorf("Catppuccin() Mode = %d, want ModeDark (%d)", theme.Mode, ModeDark)
	}
}

func TestThemeDefinitions_HaveUniqueIDs(t *testing.T) {
	t.Parallel()
	seen := make(map[string]bool)
	for _, def := range Available() {
		if seen[def.ID] {
			t.Errorf("duplicate theme ID: %q", def.ID)
		}
		seen[def.ID] = true
	}
}

func TestThemeDefinitions_HaveNonEmptyNames(t *testing.T) {
	t.Parallel()
	for _, def := range Available() {
		if def.Name == "" {
			t.Errorf("theme %q has empty Name", def.ID)
		}
		if def.Description == "" {
			t.Errorf("theme %q has empty Description", def.ID)
		}
	}
}

func TestByID_DarkMatchesDefault(t *testing.T) {
	t.Parallel()
	dark, ok := ByID("dark")
	if !ok {
		t.Fatal("dark theme not found")
	}
	def := Default()
	if dark.Mode != def.Mode {
		t.Errorf("ByID('dark').Mode = %d, Default().Mode = %d", dark.Mode, def.Mode)
	}
}
