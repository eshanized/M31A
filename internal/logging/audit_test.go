package logging

import (
	"os"
	"path/filepath"
	"testing"
)

func TestRedactSecrets(t *testing.T) {
	tests := []struct {
		name     string
		input    string
		redacted bool // true if the output should differ from input
	}{
		{"openai key", "sk-abc123def456ghi789jkl012mno", true},
		{"bearer token", "Authorization: Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9", true},
		{"hex token", "token=a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0", true},
		{"plain text", "hello world", false},
		{"empty", "", false},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			result := RedactSecrets(tt.input)
			if tt.redacted && result == tt.input {
				t.Errorf("RedactSecrets did not redact %q", tt.input)
			}
			if !tt.redacted && result != tt.input {
				t.Errorf("RedactSecrets unexpectedly modified %q to %q", tt.input, result)
			}
		})
	}
}

func TestRedactSecrets_PreservesNonSecrets(t *testing.T) {
	input := "connecting to server at http://example.com:8080"
	result := RedactSecrets(input)
	if result != input {
		t.Errorf("RedactSecrets modified non-secret text: %q -> %q", input, result)
	}
}

func TestSanitizeLogValue(t *testing.T) {
	// Sensitive keys should be redacted
	if result := SanitizeLogValue("api_key", "sk-abc123"); result != "****" {
		t.Errorf("SanitizeLogValue(api_key) = %v, want ****", result)
	}
	if result := SanitizeLogValue("SECRET_TOKEN", "abc123"); result != "****" {
		t.Errorf("SanitizeLogValue(SECRET_TOKEN) = %v, want ****", result)
	}
	if result := SanitizeLogValue("password", "hunter2"); result != "****" {
		t.Errorf("SanitizeLogValue(password) = %v, want ****", result)
	}

	// Non-sensitive keys pass through
	if result := SanitizeLogValue("host", "example.com"); result != "example.com" {
		t.Errorf("SanitizeLogValue(host) = %v, want example.com", result)
	}

	// String values with secrets get redacted
	result := SanitizeLogValue("message", "sk-abc123def456ghi789jkl012mno")
	if result == "sk-abc123def456ghi789jkl012mno" {
		t.Error("SanitizeLogValue should redact secrets in string values")
	}

	// Non-string values pass through
	if result := SanitizeLogValue("count", 42); result != 42 {
		t.Errorf("SanitizeLogValue(count, 42) = %v, want 42", result)
	}
}

func TestAuditLogForSecrets(t *testing.T) {
	// Create a temp file with a suspicious log line
	dir := t.TempDir()
	path := filepath.Join(dir, "test.go")
	content := `package main

import "log/slog"

func main() {
	apiKey := "sk-abc123def456ghi789"
	slog.Info("connecting", "key", apiKey)
}
`
	if err := os.WriteFile(path, []byte(content), 0644); err != nil {
		t.Fatal(err)
	}

	issues, err := AuditLogForSecrets(path)
	if err != nil {
		t.Fatalf("AuditLogForSecrets error: %v", err)
	}
	if len(issues) == 0 {
		t.Error("AuditLogForSecrets should find issues in suspicious log line")
	}
}

func TestAuditLogForSecrets_CleanFile(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "clean.go")
	content := `package main

import "log/slog"

func main() {
	slog.Info("server started", "port", 8080)
}
`
	if err := os.WriteFile(path, []byte(content), 0644); err != nil {
		t.Fatal(err)
	}

	issues, err := AuditLogForSecrets(path)
	if err != nil {
		t.Fatalf("AuditLogForSecrets error: %v", err)
	}
	if len(issues) != 0 {
		t.Errorf("AuditLogForSecrets should find no issues, got: %v", issues)
	}
}

func TestAuditLogForSecrets_NonexistentFile(t *testing.T) {
	_, err := AuditLogForSecrets("/nonexistent/file.go")
	if err == nil {
		t.Error("AuditLogForSecrets should return error for nonexistent file")
	}
}
