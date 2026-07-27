package tools

import (
	"context"
	"os"
	stdExec "os/exec"
	"testing"

	toolsExec "github.com/eshanized/M31A/internal/tools/exec"
)

// This test file tests the scrubEnvironment functionality which is platform-specific.
// We test by importing the exec package as a separate name to avoid conflicts.

func TestScrubEnvironment_RemovesSensitiveVars(t *testing.T) {
	t.Parallel()

	cmd := stdExec.CommandContext(context.Background(), "echo", "test")

	// Set a bunch of sensitive environment variables
	originalVars := map[string]string{
		"API_KEY":           "sk-12345",
		"SECRET_TOKEN":      "abcdef",
		"PASSWORD":          "hunter2",
		"AUTHORIZATION":     "Bearer xyz",
		"DATABASE_PASSWORD": "supersecret",
		"PRIVATE_KEY":       "-----BEGIN PRIVATE KEY-----",
	}
	for k, v := range originalVars {
		os.Setenv(k, v)
	}
	defer func() {
		for k := range originalVars {
			os.Unsetenv(k)
		}
	}()

	// Also set a safe var to verify it's kept
	os.Setenv("MY_CUSTOM_VAR", "safe-value")
	defer os.Unsetenv("MY_CUSTOM_VAR")

	toolsExec.ScrubEnvironment(cmd)

	// Verify sensitive vars are removed
	for _, env := range cmd.Env {
		for key := range originalVars {
			if len(env) > len(key)+1 && env[:len(key)+1] == key+"=" {
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

	cmd := stdExec.CommandContext(context.Background(), "echo", "test")

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

	toolsExec.ScrubEnvironment(cmd)

	// Verify prefixed vars are removed
	for _, env := range cmd.Env {
		if len(env) > 8 && env[:8] == "API_KEY=" {
			t.Errorf("API_KEY_* should be removed but found: %s", env)
		}
		if len(env) > 6 && env[:6] == "TOKEN=" {
			t.Errorf("TOKEN_* should be removed but found: %s", env)
		}
		if len(env) > 7 && env[:7] == "SECRET=" {
			t.Errorf("SECRET_* should be removed but found: %s", env)
		}
		if len(env) > 9 && env[:9] == "PASSWORD=" {
			t.Errorf("PASSWORD_* should be removed but found: %s", env)
		}
	}

	// Verify safe var is kept
	foundSafe := false
	for _, env := range cmd.Env {
		if env == "SAFE_VAR=not-secret" {
			foundSafe = true
			break
		}
	}
	if !foundSafe {
		t.Error("SAFE_VAR should be preserved")
	}
}

func TestScrubEnvironment_DoesNotRemoveNonPrefixedVars(t *testing.T) {
	t.Parallel()

	cmd := stdExec.CommandContext(context.Background(), "echo", "test")

	// These should NOT be removed (they don't contain sensitive keywords as prefix or suffix)
	os.Setenv("MY_API_SETTING", "value1")
	os.Setenv("CONFIG_NAME", "value2")
	os.Setenv("MY_COLOR_CODE", "value3")
	os.Setenv("DB_HOST_ADDRESS", "value4")
	defer func() {
		os.Unsetenv("MY_API_SETTING")
		os.Unsetenv("CONFIG_NAME")
		os.Unsetenv("MY_COLOR_CODE")
		os.Unsetenv("DB_HOST_ADDRESS")
	}()

	toolsExec.ScrubEnvironment(cmd)

	// These should be preserved since they don't contain sensitive keywords
	found := map[string]bool{}
	for _, env := range cmd.Env {
		found[env] = true
	}
	if !found["MY_API_SETTING=value1"] {
		t.Error("MY_API_SETTING should be preserved")
	}
	if !found["CONFIG_NAME=value2"] {
		t.Error("CONFIG_NAME should be preserved")
	}
	if !found["MY_COLOR_CODE=value3"] {
		t.Error("MY_COLOR_CODE should be preserved")
	}
	if !found["DB_HOST_ADDRESS=value4"] {
		t.Error("DB_HOST_ADDRESS should be preserved")
	}
}
