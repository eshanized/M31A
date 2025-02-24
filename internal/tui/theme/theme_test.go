package theme

import "testing"

func TestDarkTheme_Background(t *testing.T) {
	t.Run("Background", func(t *testing.T) {
		if got := string(Dark().Background); got != "#0D0D0D" {
			t.Errorf("Dark().Background = %q, want %q", got, "#0D0D0D")
		}
	})
}

func TestDarkTheme_Brand(t *testing.T) {
	t.Run("Brand", func(t *testing.T) {
		if got := string(Dark().Brand); got != "#D77757" {
			t.Errorf("Dark().Brand = %q, want %q", got, "#D77757")
		}
	})
}

func TestLightTheme_Background(t *testing.T) {
	t.Run("Background", func(t *testing.T) {
		if got := string(Light().Background); got != "#FFFFFF" {
			t.Errorf("Light().Background = %q, want %q", got, "#FFFFFF")
		}
	})
}

func TestLightTheme_Brand(t *testing.T) {
	t.Run("Brand", func(t *testing.T) {
		if got := string(Light().Brand); got != "#C45C3A" {
			t.Errorf("Light().Brand = %q, want %q", got, "#C45C3A")
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
		if got := string(m.Current().Background); got != "#0D0D0D" {
			t.Errorf("Current().Background = %q, want %q", got, "#0D0D0D")
		}
	})
	t.Run("AfterCycle", func(t *testing.T) {
		m := NewManager(ModeDark)
		m.Cycle()
		if got := string(m.Current().Background); got != "#FFFFFF" {
			t.Errorf("After cycle, Current().Background = %q, want %q", got, "#FFFFFF")
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
	if got := string(d.Background); got != "#0D0D0D" {
		t.Errorf("Default().Background = %q, want %q", got, "#0D0D0D")
	}
}
