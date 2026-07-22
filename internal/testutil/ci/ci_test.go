// Package ci provides helpers for detecting CI environments in tests.
package ci

import (
	"os"
	"testing"
)

func TestIsCI(t *testing.T) {

	// Save original env
	origCI := os.Getenv("CI")
	origGH := os.Getenv("GITHUB_ACTIONS")
	origGL := os.Getenv("GITLAB_CI")
	origCN := os.Getenv("CI_NAME")
	defer func() {
		os.Setenv("CI", origCI)
		os.Setenv("GITHUB_ACTIONS", origGH)
		os.Setenv("GITLAB_CI", origGL)
		os.Setenv("CI_NAME", origCN)
	}()

	tests := []struct {
		name       string
		setup      func()
		wantResult bool
	}{
		{
			name: "CI=true",
			setup: func() {
				os.Setenv("CI", "true")
			},
			wantResult: true,
		},
		{
			name: "GITHUB_ACTIONS=true",
			setup: func() {
				os.Setenv("GITHUB_ACTIONS", "true")
			},
			wantResult: true,
		},
		{
			name: "GITLAB_CI=true",
			setup: func() {
				os.Setenv("GITLAB_CI", "true")
			},
			wantResult: true,
		},
		{
			name: "CI_NAME=github",
			setup: func() {
				os.Setenv("CI_NAME", "github")
			},
			wantResult: true,
		},
		{
			name: "no CI vars set",
			setup: func() {
				os.Unsetenv("CI")
				os.Unsetenv("GITHUB_ACTIONS")
				os.Unsetenv("GITLAB_CI")
				os.Unsetenv("CI_NAME")
			},
			wantResult: false,
		},
	}

	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			t.Parallel()
			tc.setup()
			if got := IsCI(); got != tc.wantResult {
				t.Errorf("IsCI() = %v, want %v", got, tc.wantResult)
			}
		})
	}
}

func TestSkipIfCI(t *testing.T) {
	t.Parallel()

	// Can't easily test Skip in parallel tests, just verify it doesn't panic outside CI
	os.Unsetenv("CI")
	os.Unsetenv("GITHUB_ACTIONS")
	os.Unsetenv("GITLAB_CI")
	os.Unsetenv("CI_NAME")

	t.Run("does not skip when not in CI", func(t *testing.T) {
		skipped := false
		t.Run("inner", func(t *testing.T) {
			SkipIfCI(t, "test reason")
			// If we reach here, not skipped
			skipped = true
		})
		if !skipped {
			t.Error("test was skipped unexpectedly")
		}
	})
}