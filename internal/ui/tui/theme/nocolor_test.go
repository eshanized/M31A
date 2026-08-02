package theme

import (
	"os"
	"testing"
)

func TestNoColor_ProfileNone(t *testing.T) {
	t.Setenv("NO_COLOR", "1")
	profile := DetectColorProfile()
	if profile != ProfileNone {
		t.Errorf("expected ProfileNone when NO_COLOR=1, got %v", profile)
	}
}

func TestNoColor_NotReturnedWithoutEnv(t *testing.T) {
	// Ensure NO_COLOR is not set
	os.Unsetenv("NO_COLOR")
	profile := DetectColorProfile()
	if profile == ProfileNone {
		t.Error("ProfileNone should not be returned when NO_COLOR is not set")
	}
}

func TestNoColor_AnyNonEmptyValue(t *testing.T) {
	testCases := []string{"1", "true", "yes", "anything"}
	for _, val := range testCases {
		t.Setenv("NO_COLOR", val)
		profile := DetectColorProfile()
		if profile != ProfileNone {
			t.Errorf("expected ProfileNone when NO_COLOR=%q, got %v", val, profile)
		}
	}
}

func TestNoColor_ProfileNoneIsDefined(t *testing.T) {
	// ProfileNone should be a valid ColorProfile value (not equal to any profile)
	if ProfileNone == ProfileTrueColor || ProfileNone == Profile256 || ProfileNone == Profile16 {
		t.Error("ProfileNone should not collide with other profile constants")
	}
}

func TestNoColor_ResolveUsesAnsiPalette(t *testing.T) {
	t.Setenv("NO_COLOR", "1")
	m := NewManager(ModeDark)
	th := m.Current()
	// ProfileNone should use ANSI palette (same as Profile16)
	// Verify by checking that background uses ANSI color "0"
	if string(th.Background) != "0" {
		t.Errorf("expected ANSI palette background '0' for ProfileNone, got %q", string(th.Background))
	}
}
