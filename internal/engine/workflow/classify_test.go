package workflow

import (
	"os"
	"path/filepath"
	"testing"

	m31types "github.com/eshanized/M31A/internal/core/types"
)

// TestDetectProjectType_DeterministicOnMultiFramework is a regression test
// for BUG-04: when a project has multiple framework indicators (common for
// full-stack repos), detectProjectType must return the same answer on every
// call. Before the fix, a map-based detector produced random results.
func TestDetectProjectType_DeterministicOnMultiFramework(t *testing.T) {
	dir := t.TempDir()
	for _, f := range []string{
		"go.mod", "package.json", "Cargo.toml", "pyproject.toml", "pom.xml", "Makefile",
	} {
		if err := os.WriteFile(filepath.Join(dir, f), []byte(""), 0644); err != nil {
			t.Fatal(err)
		}
	}

	// Run 100 times — a map-based implementation would produce at least two
	// distinct values over 100 iterations with overwhelming probability.
	first := detectProjectType(dir)
	for i := 0; i < 100; i++ {
		got := detectProjectType(dir)
		if got != first {
			t.Fatalf("detectProjectType not deterministic: got %q and %q on iteration %d",
				first, got, i)
		}
	}
	// go.mod has highest priority.
	if first != "go" {
		t.Errorf("priority: got %q, want 'go'", first)
	}
}

// TestDetectPackageManager_DeterministicOnMultiLock is a regression test for
// BUG-05. When multiple lock files exist, the returned package manager must
// be deterministic and respect the priority order (pnpm > yarn > bun > npm).
func TestDetectPackageManager_DeterministicOnMultiLock(t *testing.T) {
	dir := t.TempDir()
	for _, f := range []string{
		"pnpm-lock.yaml", "yarn.lock", "bun.lockb", "package-lock.json",
	} {
		if err := os.WriteFile(filepath.Join(dir, f), []byte(""), 0644); err != nil {
			t.Fatal(err)
		}
	}

	first := detectPackageManager(dir)
	for i := 0; i < 100; i++ {
		got := detectPackageManager(dir)
		if got != first {
			t.Fatalf("detectPackageManager not deterministic: got %q and %q on iteration %d",
				first, got, i)
		}
	}
	if first != "pnpm run" {
		t.Errorf("priority: got %q, want 'pnpm run'", first)
	}
}

// TestClassifyPrompt_CodeSignalsPreventTrivial is a regression test for
// BUG-06. Short verb-led goals with code-complexity signals (race, auth,
// leak, etc.) must NOT be classified as trivial — trivial mode skips Plan
// and Verify phases, which is wrong for anything touching auth/security.
func TestClassifyPrompt_CodeSignalsPreventTrivial(t *testing.T) {
	dir := t.TempDir()
	cases := []struct {
		goal string
	}{
		{"fix the data race"},
		{"add auth bypass"},
		{"fix memory leak"},
		{"add JWT support"},
		{"fix deadlock"},
		{"add oauth flow"},
		{"fix SQL injection"},
	}
	for _, tc := range cases {
		t.Run(tc.goal, func(t *testing.T) {
			got := ClassifyPrompt(tc.goal, dir)
			if got == m31types.ComplexityTrivial {
				t.Errorf("goal %q classified as trivial; expected >= moderate", tc.goal)
			}
		})
	}
}

// TestClassifyPrompt_TrueTrivial verifies that genuinely trivial goals
// (without code-complexity signals) still classify as trivial.
func TestClassifyPrompt_TrueTrivial(t *testing.T) {
	dir := t.TempDir()
	got := ClassifyPrompt("bump version", dir)
	if got != m31types.ComplexityTrivial {
		t.Errorf("got %v, want ComplexityTrivial", got)
	}
}
