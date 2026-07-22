// Package ci provides helpers for detecting CI environments in tests.
package ci

import (
	"os"
	"testing"
)

// IsCI returns true if the current process is running in a CI environment.
// Checks common CI environment variables: CI, GITHUB_ACTIONS, GITLAB_CI, CI_NAME.
func IsCI() bool {
	return os.Getenv("CI") == "true" ||
		os.Getenv("GITHUB_ACTIONS") == "true" ||
		os.Getenv("GITLAB_CI") == "true" ||
		os.Getenv("CI_NAME") != ""
}

// SkipIfCI calls t.Skip with the given reason if running in a CI environment.
// Use for tests that require external resources (API keys, network, etc.) not available in CI.
func SkipIfCI(t *testing.T, reason string) {
	t.Helper()
	if IsCI() {
		t.Skip(reason)
	}
}