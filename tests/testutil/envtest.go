package testutil

import (
	"os"
	"path/filepath"
	"sync"
	"testing"
)

// RequireAPIKey skips the test if the named environment variable is not set
// or is empty. Use this for tests that need real API keys from .env.
//
// Example:
//
//	func TestLiveChat(t *testing.T) {
//	    key := testutil.RequireAPIKey(t, "OPENROUTER_API_KEY")
//	    // ... use key
//	}
func RequireAPIKey(t *testing.T, envVar string) string {
	t.Helper()
	if v := os.Getenv(envVar); v != "" {
		return v
	}
	t.Skipf("skipping: %s not set in environment (add to .env)", envVar)
	return ""
}

// RequireAnyAPIKey skips the test unless at least one of the named env vars
// is set. Returns the value of the first one found.
func RequireAnyAPIKey(t *testing.T, envVars ...string) string {
	t.Helper()
	for _, v := range envVars {
		if val := os.Getenv(v); val != "" {
			return val
		}
	}
	t.Skipf("skipping: none of %v set in environment", envVars)
	return ""
}

// LoadTestDotEnv loads .env.test from the project root into the process
// environment. It is safe to call multiple times — the file is only read once.
// Existing env vars are never overridden.
func LoadTestDotEnv(t *testing.T) {
	t.Helper()
	loadTestDotEnvOnce.Do(func() {
		// Walk up from cwd to find project root (contains go.mod)
		dir, err := os.Getwd()
		if err != nil {
			return
		}
		for i := 0; i < 10; i++ {
			if _, err := os.Stat(filepath.Join(dir, "go.mod")); err == nil {
				envPath := filepath.Join(dir, ".env.test")
				if _, err := os.Stat(envPath); err == nil {
					loadEnvFile(envPath)
				}
				return
			}
			dir = filepath.Dir(dir)
		}
	})
}

var loadTestDotEnvOnce sync.Once

func loadEnvFile(path string) {
	data, err := os.ReadFile(path)
	if err != nil {
		return
	}
	for _, line := range splitLines(string(data)) {
		line = trimSpace(line)
		if line == "" || line[0] == '#' {
			continue
		}
		key, val, ok := splitEnvLine(line)
		if !ok {
			continue
		}
		if _, exists := os.LookupEnv(key); !exists {
			_ = os.Setenv(key, val) //nolint:errcheck
		}
	}
}

func splitLines(s string) []string {
	var lines []string
	start := 0
	for i := 0; i < len(s); i++ {
		if s[i] == '\n' {
			lines = append(lines, s[start:i])
			start = i + 1
		}
	}
	if start < len(s) {
		lines = append(lines, s[start:])
	}
	return lines
}

func trimSpace(s string) string {
	i, j := 0, len(s)
	for i < j && (s[i] == ' ' || s[i] == '\t' || s[i] == '\r') {
		i++
	}
	for j > i && (s[j-1] == ' ' || s[j-1] == '\t' || s[j-1] == '\r') {
		j--
	}
	return s[i:j]
}

func splitEnvLine(line string) (key, val string, ok bool) {
	for i := 0; i < len(line); i++ {
		if line[i] == '=' {
			key = line[:i]
			val = line[i+1:]
			// Strip surrounding quotes
			if len(val) >= 2 {
				if (val[0] == '"' && val[len(val)-1] == '"') ||
					(val[0] == '\'' && val[len(val)-1] == '\'') {
					val = val[1 : len(val)-1]
				}
			}
			return key, val, true
		}
	}
	return "", "", false
}
