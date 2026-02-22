package types

import "testing"

func TestSkipDirsMap_ContainsExpectedDirs(t *testing.T) {
	t.Parallel()
	m := SkipDirsMap()

	for _, dir := range SkipDirs {
		if !m[dir] {
			t.Errorf("SkipDirsMap missing expected dir: %q", dir)
		}
	}
}

func TestSkipDirsMap_DoesNotContainRandomDir(t *testing.T) {
	t.Parallel()
	m := SkipDirsMap()

	if m["src"] {
		t.Error("SkipDirsMap should not contain 'src'")
	}
	if m[""] {
		t.Error("SkipDirsMap should not contain empty string")
	}
}

func TestSkipDirsMap_ReturnsSameInstance(t *testing.T) {
	t.Parallel()
	a := SkipDirsMap()
	b := SkipDirsMap()

	if &a == &b {
		t.Error("SkipDirsMap should return the same cached map pointer")
	}
	if len(a) != len(b) {
		t.Errorf("SkipDirsMap returned different lengths: %d vs %d", len(a), len(b))
	}
}

func TestSkipDirsMap_Size(t *testing.T) {
	t.Parallel()
	m := SkipDirsMap()

	if len(m) != len(SkipDirs) {
		t.Errorf("SkipDirsMap has %d entries, expected %d", len(m), len(SkipDirs))
	}
}
