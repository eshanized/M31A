package theme

import (
	"testing"
)

func TestNormalizeTabs_WideTabs(t *testing.T) {
	t.Parallel()
	input := "hello\tworld"
	got := NormalizeTabs(input, 8)
	if got == input {
		t.Error("expected tabs to be expanded")
	}
}

func TestDefault_BackgroundNonZero(t *testing.T) {
	t.Parallel()
	th := Default()
	if string(th.Background) == "" {
		t.Error("expected non-zero background")
	}
}

func TestDark_BackgroundNonZero(t *testing.T) {
	t.Parallel()
	th := Dark()
	if th.Mode != ModeDark {
		t.Errorf("expected ModeDark, got %v", th.Mode)
	}
}
