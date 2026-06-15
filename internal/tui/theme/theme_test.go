package theme

import (
	"os"
	"testing"
)

func TestDetectColorProfile_TrueColor(t *testing.T) {
	os.Setenv("COLORTERM", "truecolor")
	defer os.Unsetenv("COLORTERM")
	if got := DetectColorProfile(); got != ProfileTrueColor {
		t.Errorf("DetectColorProfile() = %d, want ProfileTrueColor (%d)", got, ProfileTrueColor)
	}
}

func TestDetectColorProfile_24bit(t *testing.T) {
	os.Setenv("COLORTERM", "24bit")
	defer os.Unsetenv("COLORTERM")
	if got := DetectColorProfile(); got != ProfileTrueColor {
		t.Errorf("DetectColorProfile() = %d, want ProfileTrueColor (%d)", got, ProfileTrueColor)
	}
}

func TestDetectColorProfile_256(t *testing.T) {
	os.Unsetenv("COLORTERM")
	os.Setenv("TERM", "xterm-256color")
	defer os.Unsetenv("TERM")
	if got := DetectColorProfile(); got != Profile256 {
		t.Errorf("DetectColorProfile() = %d, want Profile256 (%d)", got, Profile256)
	}
}

func TestDetectColorProfile_16(t *testing.T) {
	os.Unsetenv("COLORTERM")
	os.Setenv("TERM", "xterm")
	defer os.Unsetenv("TERM")
	if got := DetectColorProfile(); got != Profile16 {
		t.Errorf("DetectColorProfile() = %d, want Profile16 (%d)", got, Profile16)
	}
}

func TestDetectColorProfile_NoEnv(t *testing.T) {
	os.Unsetenv("COLORTERM")
	os.Unsetenv("TERM")
	if got := DetectColorProfile(); got != Profile16 {
		t.Errorf("DetectColorProfile() with no env = %d, want Profile16 (%d)", got, Profile16)
	}
}

func TestPaletteForProfile_Default(t *testing.T) {
	d := PaletteForProfile(ProfileTrueColor)
	if string(d.Background) != "#0d0f1a" {
		t.Errorf("PaletteForProfile(TrueColor).Background = %q, want %q", string(d.Background), "#0d0f1a")
	}
}

func TestPaletteForProfile_256(t *testing.T) {
	d := PaletteForProfile(Profile256)
	if string(d.Background) != "#0d0f1a" {
		t.Errorf("PaletteForProfile(256).Background = %q, want %q", string(d.Background), "#0d0f1a")
	}
}

func TestPaletteForProfile_16(t *testing.T) {
	p := PaletteForProfile(Profile16)
	// ANSI palette should have ANSI color codes instead of hex
	if string(p.Brand) != "208" {
		t.Errorf("PaletteForProfile(16).Brand = %q, want %q", string(p.Brand), "208")
	}
	if string(p.Success) != "2" {
		t.Errorf("PaletteForProfile(16).Success = %q, want %q", string(p.Success), "2")
	}
	if string(p.Error) != "1" {
		t.Errorf("PaletteForProfile(16).Error = %q, want %q", string(p.Error), "1")
	}
	if string(p.Warning) != "3" {
		t.Errorf("PaletteForProfile(16).Warning = %q, want %q", string(p.Warning), "3")
	}
}

func TestManager_ProfileDetection(t *testing.T) {
	m := NewManager(ModeDark)
	// Manager should have detected a profile (any is fine as long as it's valid)
	if m.profile != ProfileTrueColor && m.profile != Profile256 && m.profile != Profile16 {
		t.Errorf("Manager profile = %d, expected one of {TrueColor, 256, 16}", m.profile)
	}
}

func TestDarkTheme_Background(t *testing.T) {
	t.Run("Background", func(t *testing.T) {
		if got := string(Dark().Background); got != "#0d0f1a" {
			t.Errorf("Dark().Background = %q, want %q", got, "#0d0f1a")
		}
	})
}

func TestDarkTheme_Brand(t *testing.T) {
	t.Run("Brand", func(t *testing.T) {
		// Dark theme uses electric indigo for modern AI aesthetic
		if got := string(Dark().Brand); got != "#7C6AF7" {
			t.Errorf("Dark().Brand = %q, want %q", got, "#7C6AF7")
		}
	})
}

func TestLightTheme_Background(t *testing.T) {
	t.Run("Background", func(t *testing.T) {
		if got := string(Light().Background); got != "#fafaf8" {
			t.Errorf("Light().Background = %q, want %q", got, "#fafaf8")
		}
	})
}

func TestLightTheme_Brand(t *testing.T) {
	t.Run("Brand", func(t *testing.T) {
		// Light theme keeps its warm brand (unchanged)
		if got := string(Light().Brand); got == "" {
			t.Errorf("Light().Brand should not be empty")
		}
	})
}

func TestManager_Cycle(t *testing.T) {
	m := NewManager(ModeDark)
	t.Run("DarkToLight", func(t *testing.T) {
		if got := m.Cycle(); got != ModeLight {
			t.Errorf("Cycle() = %d, want ModeLight", got)
		}
	})
	t.Run("LightToAuto", func(t *testing.T) {
		if got := m.Cycle(); got != ModeAuto {
			t.Errorf("Cycle() = %d, want ModeAuto", got)
		}
	})
	t.Run("AutoToDark", func(t *testing.T) {
		if got := m.Cycle(); got != ModeDark {
			t.Errorf("Cycle() = %d, want ModeDark", got)
		}
	})
}

func TestManager_Current(t *testing.T) {
	t.Run("AfterDarkConstruction", func(t *testing.T) {
		m := NewManager(ModeDark)
		if got := string(m.Current().Background); got != "#0d0f1a" {
			t.Errorf("Current().Background = %q, want %q", got, "#0d0f1a")
		}
	})
	t.Run("AfterCycle", func(t *testing.T) {
		m := NewManager(ModeDark)
		m.Cycle()
		if got := string(m.Current().Background); got != "#fafaf8" {
			t.Errorf("After cycle, Current().Background = %q, want %q", got, "#fafaf8")
		}
	})
}

func TestToolLabel_Keys(t *testing.T) {
	dt := Dark()
	keys := []string{"Bash", "FileRead", "FileWrite", "Glob", "Grep"}
	for _, k := range keys {
		if _, ok := dt.ToolLabel[k]; !ok {
			t.Errorf("Dark().ToolLabel[%q] missing", k)
		}
	}
}

func TestDefault_Dark(t *testing.T) {
	d := Default()
	if got := string(d.Background); got != "#0d0f1a" {
		t.Errorf("Default().Background = %q, want %q", got, "#0d0f1a")
	}
}
