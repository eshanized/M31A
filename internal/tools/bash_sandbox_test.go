package tools

import (
	"os"
	"os/exec"
	"strings"
	"testing"
)

func TestScrubEnvironment_RemovesAPIKeys(t *testing.T) {
	t.Parallel()

	cmd := exec.Command("echo", "test")

	// Set some sensitive env vars in the current process
	originalVars := map[string]string{
		"OPENROUTER_API_KEY": "sk-or-test-12345",
		"ZEN_API_KEY":        "zen-test-67890",
		"NVIDIA_API_KEY":     "nvapi-test-abcde",
	}

	// Save originals and set test values
	origVals := make(map[string]string)
	for k, v := range originalVars {
		origVals[k] = os.Getenv(k)
		os.Setenv(k, v)
	}
	defer func() {
		for k, v := range origVals {
			if v == "" {
				os.Unsetenv(k)
			} else {
				os.Setenv(k, v)
			}
		}
	}()

	// Also set a safe env var to verify it's kept
	os.Setenv("MY_CUSTOM_VAR", "safe-value")

	scrubEnvironment(cmd)

	// Verify sensitive vars are removed
	for _, env := range cmd.Env {
		for key := range originalVars {
			if strings.HasPrefix(env, key+"=") {
				t.Errorf("sensitive env var %s should be removed but found: %s", key, env)
			}
		}
	}

	// Verify safe var is kept
	found := false
	for _, env := range cmd.Env {
		if env == "MY_CUSTOM_VAR=safe-value" {
			found = true
			break
		}
	}
	if !found {
		t.Error("safe env var MY_CUSTOM_VAR should be preserved")
	}

	// Verify non-interactive vars are added
	foundCI := false
	foundDEBIAN := false
	for _, env := range cmd.Env {
		if env == "CI=true" {
			foundCI = true
		}
		if env == "DEBIAN_FRONTEND=noninteractive" {
			foundDEBIAN = true
		}
	}
	if !foundCI {
		t.Error("CI=true should be added")
	}
	if !foundDEBIAN {
		t.Error("DEBIAN_FRONTEND=noninteractive should be added")
	}
}

func TestScrubEnvironment_PrefixMatching(t *testing.T) {
	t.Parallel()

	cmd := exec.Command("echo", "test")

	// Set env vars with sensitive prefixes (must START with the prefix)
	os.Setenv("API_KEY_FOO", "secret123")
	os.Setenv("TOKEN_VALUE", "secret456")
	os.Setenv("SECRET_HEADER", "secret789")
	os.Setenv("PASSWORD_CONN", "secret012")
	os.Setenv("SAFE_VAR", "not-secret")
	defer func() {
		os.Unsetenv("API_KEY_FOO")
		os.Unsetenv("TOKEN_VALUE")
		os.Unsetenv("SECRET_HEADER")
		os.Unsetenv("PASSWORD_CONN")
		os.Unsetenv("SAFE_VAR")
	}()

	scrubEnvironment(cmd)

	// Verify prefix-matched vars are removed
	for _, env := range cmd.Env {
		for _, prefix := range []string{"API_KEY_FOO", "TOKEN_VALUE", "SECRET_HEADER", "PASSWORD_CONN"} {
			if strings.HasPrefix(env, prefix+"=") {
				t.Errorf("env var with prefix %s should be removed but was found: %s", prefix, env)
			}
		}
	}

	// Verify safe var is kept
	foundSafe := false
	for _, env := range cmd.Env {
		if env == "SAFE_VAR=not-secret" {
			foundSafe = true
		}
	}
	if !foundSafe {
		t.Error("SAFE_VAR should be preserved")
	}
}
