// Package ci provides helpers for detecting CI environments in tests.
package ci

import (
	"testing"
)

func TestIsCI(t *testing.T) {

	tests := []struct {
		name       string
		envVar     string
		envValue   string
		wantResult bool
	}{
		{
			name:       "CI=true",
			envVar:     "CI",
			envValue:   "true",
			wantResult: true,
		},
		{
			name:       "GITHUB_ACTIONS=true",
			envVar:     "GITHUB_ACTIONS",
			envValue:   "true",
			wantResult: true,
		},
		{
			name:       "GITLAB_CI=true",
			envVar:     "GITLAB_CI",
			envValue:   "true",
			wantResult: true,
		},
		{
			name:       "CI_NAME=github",
			envVar:     "CI_NAME",
			envValue:   "github",
			wantResult: true,
		},
		{
			name:       "no CI vars set",
			envVar:     "",
			envValue:   "",
			wantResult: false,
		},
	}

	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			if tc.envVar != "" {
				t.Setenv(tc.envVar, tc.envValue)
			} else {
				t.Setenv("CI", "")
				t.Setenv("GITHUB_ACTIONS", "")
				t.Setenv("GITLAB_CI", "")
				t.Setenv("CI_NAME", "")
			}
			if got := IsCI(); got != tc.wantResult {
				t.Errorf("IsCI() = %v, want %v", got, tc.wantResult)
			}
		})
	}
}

func TestSkipIfCI(t *testing.T) {
	t.Setenv("CI", "")
	t.Setenv("GITHUB_ACTIONS", "")
	t.Setenv("GITLAB_CI", "")
	t.Setenv("CI_NAME", "")

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
